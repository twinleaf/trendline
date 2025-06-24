#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod device;
mod discover;

use app_state::{AppState, SinglePlotPoint};
use tauri::{Manager, State, AppHandle};
use discover::{ FeDeviceMeta, is_process_running};
use trendline_lib::{get_valid_twinleaf_serial_urls};
use twinleaf::tio::proto::DeviceRoute;
use twinleaf::tio::proxy::{self, PortError};
use std::time::{Duration, Instant};
use std::collections::{HashMap, HashSet};



// A command to start a data collection thread which instantiates a proxy on a specified route
#[tauri::command]
async fn start_streaming(
    ports: Vec<discover::PortInfo>,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<(), String> {
    println!("Received request to stream from {} ports.", ports.len());

    let mut ports_by_url: HashMap<String, Vec<String>> = HashMap::new();
    for port in ports {
        ports_by_url.entry(port.url).or_default().push(port.id);
    }
    
    let mut backend = state.backend_state.lock().unwrap();

    for (url, stream_ids) in ports_by_url {

        let proxy = backend.proxies
            .entry(url.clone())
            .or_insert_with(|| proxy::Interface::new(&url));

        for stream_id in stream_ids {
            println!("# Spawning stream worker for route '{}' on {}", &stream_id, &url);

            let route = DeviceRoute::from_str(&stream_id).unwrap();
            
            match proxy.device_full(route) {
                Ok(worker_port) => {
                    let state_clone = state.inner().clone();
                    let handle_clone = app_handle.clone();
                    
                    device::start_stream_thread(
                        worker_port,
                        stream_id.clone(),
                        state_clone,
                        handle_clone,
                    );
                }
                Err(e) => {
                    eprintln!(
                        "! Failed to open logical route '{}' on {}: {:?}. Skipping.",
                        &stream_id, &url, e
                    );
                }
            }
        }
    }

    Ok(())
}



// A command to retrieve all data currently in the buffer.
#[tauri::command]
fn get_plot_data_in_range(
    state: State<'_, AppState>,
    series_keys: Vec<String>,
    start_time: f64,
    end_time: f64,
) -> HashMap<String, Vec<SinglePlotPoint>> {
    let mut result_map = HashMap::new();
    let buffer_lock = state.buffer.lock().unwrap();

    // Convert time range to u64 bits for BTreeMap lookup
    let start_key = start_time.to_bits();
    let end_key = end_time.to_bits();

    for key in series_keys {
        if let Some(series_data) = buffer_lock.get(&key) {
            let data: Vec<SinglePlotPoint> = series_data
                .range(start_key..=end_key) // Inclusive range query on the BTreeMap
                .map(|(timestamp_bits, value)| SinglePlotPoint {
                    x: f64::from_bits(*timestamp_bits),
                    y: *value,
                    series_key: key.clone(),
                })
                .collect();
            
            println!(
                "Retrieved {} points for series '{}' in range [{}, {}]",
                data.len(),
                &key,
                start_time,
                end_time
            );
            result_map.insert(key, data);
        } else {
            // REPLACE WITH ERROR HANDLING FOR NON-EXISTING STREAM KEY
            result_map.insert(key, Vec::new());
        }
    }
    result_map
}

// A command to return a list of unique Twinleaf serial devices, and it's children
#[tauri::command]
async fn discover_devices() -> Result<Vec<FeDeviceMeta>, String> {
    let valid_urls = get_valid_twinleaf_serial_urls();
    if valid_urls.is_empty() {
        println!("No Twinleaf devices found.");
        return Ok(vec![]);
    }

    println!("Found Twinleaf devices at: {:?}", valid_urls);

    let mut all_results: Vec<discover::MetadataWorkerMessage> = Vec::new();

    for url in &valid_urls {
        let (sender, receiver) = crossbeam::channel::unbounded();
        let proxy: proxy::Interface = proxy::Interface::new(url);

        // --- Phase 1: Discover active routes for this URL ---
        println!("# Probing {} for active routes...", url);
        let sniffer_port = match proxy.tree_probe() {
            Ok(port) => port,
            Err(e) => {
                let error_message = match e {
                    PortError::FailedNewClientSetup => {
                        if is_process_running("tio-proxy") {
                            format!(
                                "Could not access {}. \n Please close the running 'tio-proxy' process.",
                                url
                            )
                        } else {
                            format!(
                                "Serial port {} is currently in use by another application.",
                                url
                            )
                        }
                    }
                    PortError::RpcTimeoutTooShort | PortError::RpcTimeoutTooLong => format!(
                        "A timeout occurred while communicating with {}. The device may be unresponsive.",
                        url
                    ),
                };
                return Err(error_message);
            }
        };
        let mut discovered_routes = HashSet::<DeviceRoute>::new();
        let discovery_duration = Duration::from_secs(1);
        let start_time = Instant::now();

        while start_time.elapsed() < discovery_duration {
            if let Ok(pkt) = sniffer_port.receiver().recv_timeout(Duration::from_millis(50)) {
                discovered_routes.insert(pkt.routing);
            }
        }
        drop(sniffer_port);

        if discovered_routes.is_empty() {
            println!("# No active routes found on {}", url);
            continue;
        } else {
            println!("# Discovered routes on {}: {:?}", url, discovered_routes);
        }

        // --- Phase 2: Spawn a worker thread for each discovered route ---
         crossbeam::thread::scope(|s| {
            for route in discovered_routes {
                let route_str = route.to_string();
                if let Ok(worker_port) = proxy.device_full(route) {
                    println!("# Spawning worker for route '{}' on {}", route_str, url);
                    let thread_sender = sender.clone();
                    let url_clone = url.clone();
                    s.spawn(move |_| {
                        discover::metadata_worker_thread(worker_port, route_str, url_clone, thread_sender);
                    });
                }
            }
            drop(sender);
        }).map_err(|e| format!("A worker thread panicked: {:?}", e))?;
        all_results.extend(receiver.iter());
    } 

    // Partition into root devices and child devices
    let mut root_metas: HashMap<String, discover::FeDeviceMeta> = HashMap::new();
    let mut child_metas: Vec<discover::FeDeviceMeta> = Vec::new();

    for msg in all_results {
        println!("#[{}] Metadata returned", msg.route);
        let fe_meta = discover::translate_to_frontend_meta(&msg.metadata, msg.route.clone(), msg.url, None);
        if msg.route == "/" {
            root_metas.insert(fe_meta.url.clone(), fe_meta);
        } else {
            child_metas.push(fe_meta);
        }
    }

    for child in child_metas {
        if let Some(root) = root_metas.get_mut(&child.url) {
            if root.children.is_none() {
                root.children = Some(Vec::new());
            }
            root.children.as_mut().unwrap().push(child);
        }
    }

    Ok(root_metas.into_values().collect())
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            discover_devices,
            start_streaming,
            get_plot_data_in_range
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}