use crate::state::capture::{CaptureState, DataColumnId, Point};
use crate::shared::{PlotData, UiDevice};
use welch_sde::{Build, SpectralDensity};
use tauri::State;
use twinleaf::tio::proto::DeviceRoute;
use std::sync::Arc;
use crate::state::proxy_register::ProxyRegister;

struct FftStreamInfo {
    sampling_rate: f64,
    points: Vec<Point>,
}

#[tauri::command]
pub fn get_plot_data_in_range(
    keys: Vec<DataColumnId>,
    min_time: f64,
    max_time: f64,
    capture: State<CaptureState>,
    num_points: Option<usize>,
) -> PlotData {
    capture.get_data_in_range(&keys, min_time, max_time, num_points)
}

#[tauri::command]
pub fn get_latest_plot_data(
    keys: Vec<DataColumnId>,
    window_seconds: f64,
    num_points: usize,
    capture: State<CaptureState>,
) -> PlotData {

    let latest_time = capture.get_latest_timestamp_for_keys(&keys);

    if latest_time.is_none() {
        return PlotData::empty();
    }

    let max_time = latest_time.unwrap();
    let min_time = max_time - window_seconds;

    capture.get_data_in_range(&keys, min_time, max_time, Some(num_points))
}

#[tauri::command]
pub fn get_latest_fft_data(
    keys: Vec<DataColumnId>,
    window_seconds: f64, // For Welch's, a longer window (e.g., 10.0s) gives better results
    capture: State<CaptureState>,
    registry: State<Arc<ProxyRegister>>,
) -> Result<PlotData, String> {
    if keys.is_empty() {
        return Ok(PlotData::empty());
    }

    let latest_time = match capture.get_latest_timestamp_for_keys(&keys) {
        Some(t) => t,
        None => return Ok(PlotData::empty()),
    };
    let min_time = latest_time - window_seconds;

    let mut stream_infos: Vec<FftStreamInfo> = Vec::new();
    let mut first_sampling_rate: Option<f64> = None;

    for key in &keys {
        let port_manager = registry.get(&key.port_url).ok_or("Port not found")?;
        let devices = port_manager.devices.lock().unwrap();
        let (_device, ui_device) = devices.get(&key.device_route).ok_or("Device not found")?;

        let stream = ui_device
            .streams
            .iter()
            .find(|s| s.meta.stream_id == key.stream_id)
            .ok_or("Stream not found")?;

        let segment = stream
            .segment
            .as_ref()
            .ok_or("Stream segment metadata not available")?;

        let sampling_rate = segment.sampling_rate as f64; 
        let decimation = segment.decimation as f64;

        let effective_sampling_rate = if decimation > 1.0 { 
            sampling_rate / decimation
        } else {
            sampling_rate
        };


        if let Some(first_rate) = first_sampling_rate {
            if effective_sampling_rate != first_rate {
                return Err("All streams must have the same sampling rate.".to_string());
            }
        } else {
            first_sampling_rate = Some(effective_sampling_rate);
        }

        let points = capture.inner.buffers.get(key).map_or(vec![], |buffer_ref| {
            let offset = capture.inner.offsets.get(key).map_or(0.0, |off| *off.value());
            let min_key = (min_time - offset).to_bits();
            let max_key = (latest_time - offset).to_bits();
            buffer_ref
                .data
                .range(min_key..=max_key)
                .map(|(&t_bits, &y)| Point::new(f64::from_bits(t_bits) + offset, y))
                .collect()
        });

        if !points.is_empty() {
            stream_infos.push(FftStreamInfo {
                sampling_rate,
                points,
            });
        }
    }

    if stream_infos.is_empty() {
        return Ok(PlotData::empty());
    }

    let sampling_rate = first_sampling_rate.unwrap();
    let mut all_asds: Vec<Vec<f64>> = Vec::new();
    let mut frequencies: Option<Vec<f64>> = None;
    
    for info in stream_infos {
        let y_values: Vec<f64> = info.points.iter().map(|p| p.y).collect();
        if y_values.len() < 16 { 
            all_asds.push(vec![]); 
            continue;
        }

        let mean = y_values.iter().sum::<f64>() / y_values.len() as f64;
        let mean_adjusted_signal: Vec<f64> = y_values.iter().map(|&y| y - mean).collect();


        let welch: SpectralDensity<f64> =
            SpectralDensity::builder(&mean_adjusted_signal, sampling_rate).build();

        let psd = welch.periodogram();

        let asd: Vec<f64> = psd.to_vec().iter().map(|&p| p.sqrt()).collect();
        all_asds.push(asd);

        if frequencies.is_none() {
            frequencies = Some(psd.frequency().to_vec());
        }
    }

    Ok(PlotData {
        timestamps: frequencies.unwrap_or_default(),
        series_data: all_asds,
    })
}

#[tauri::command]
pub fn confirm_selection(
    port_url: String,
    children_routes: Vec<String>,
    capture: State<CaptureState>,
    registry: State<Arc<ProxyRegister>>,
) -> Result<(), String> {
    println!(
        "Confirming selection for port '{}' with child routes: {:?}",
        port_url, children_routes
    );

    let port_manager = match registry.get(&port_url) {
        Some(pm) => pm,
        None => return Err(format!("Could not find PortManager for URL: {}", port_url)),
    };

    let mut keys_to_activate: Vec<DataColumnId> = Vec::new();
    let mut all_selected_routes = children_routes;
    all_selected_routes.push("".to_string());

    // Lock the devices map for reading.
    let devices_map = port_manager.devices.lock().unwrap();

    for route_str in all_selected_routes {
        let route = match DeviceRoute::from_str(&route_str) {
            Ok(r) => r,
            Err(_) => continue,
        };

        // Get the tuple `(Device, Metadata)` from the map.
         if let Some((_device, cached_ui_device)) = devices_map.get(&route) {
            for stream in &cached_ui_device.streams {
                for column in &stream.columns {
                    let key = DataColumnId {
                        port_url: port_url.clone(),
                        device_route: route.clone(),
                        stream_id: stream.meta.stream_id,
                        column_index: column.index,
                    };
                    keys_to_activate.push(key);
                }
            }
        }
    }
    
    println!("Activating {} total data columns for port '{}'.", keys_to_activate.len(), port_url);
    capture.set_active_columns_for_port(&port_url, keys_to_activate);

    Ok(())
}

#[tauri::command]
pub fn get_all_devices(registry: State<Arc<ProxyRegister>>) -> Vec<UiDevice> {
    let mut all_devices = Vec::new();

    for entry in registry.ports.iter() {
        let port_manager = entry.value();
        
        let devices_lock = port_manager.devices.lock().unwrap();
        
        for (_route, (_device, ui_device)) in devices_lock.iter() {
            all_devices.push(ui_device.clone());
        }
    }
    
    println!("[Command] get_all_devices returning {} devices.", all_devices.len());
    all_devices
}
