use crate::pipeline::manager::ProcessingManager;
use crate::state::capture::{CaptureState};
use crate::shared::{DataColumnId, PlotData, Point, PortState, StreamStatistics, PipelineId};
use tauri::State;
use twinleaf::tio::proto::DeviceRoute;
use std::sync::{Arc, Mutex};
use crate::state::proxy_register::ProxyRegister;
use crate::util::{k_way_merge_plot_data, lerp};

#[tauri::command]
pub fn get_interpolated_values(
    time: f64,
    pipeline_ids: Vec<PipelineId>,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<Vec<Option<f64>>, String> {
    if pipeline_ids.is_empty() {
        return Ok(vec![]);
    }

    let manager = manager.lock().unwrap();
    let mut results = Vec::with_capacity(pipeline_ids.len());

    for id in pipeline_ids {
        let value = if let Some(pipeline_mutex) = manager.pipelines.get(&id) {
            let pipeline = pipeline_mutex.lock().unwrap();

            let plot_data = pipeline.get_output();

            if plot_data.timestamps.is_empty() || plot_data.series_data.is_empty() {
                None
            } else {
                match plot_data.timestamps.binary_search_by(|t| t.partial_cmp(&time).unwrap_or(std::cmp::Ordering::Less)) {
                    Ok(index) => plot_data.series_data[0].get(index).copied(),
                    Err(index) => {
                        if index == 0 {
                            plot_data.series_data[0].get(0).copied()
                        } else if index >= plot_data.timestamps.len() {
                            plot_data.series_data[0].last().copied()
                        } else {
                            let p1 = Point {
                                x: plot_data.timestamps[index - 1],
                                y: plot_data.series_data[0][index - 1],
                            };
                            let p2 = Point {
                                x: plot_data.timestamps[index],
                                y: plot_data.series_data[0][index],
                            };
                            Some(lerp(&p1, &p2, time))
                        }
                    }
                }
            }
        } else {
            None
        };
        results.push(value);
    }

    Ok(results)
}

#[tauri::command]
pub fn get_merged_plot_data(
    ids: Vec<PipelineId>,
    manager: State<Arc<Mutex<ProcessingManager>>>,
    capture: State<CaptureState>,
) -> Result<PlotData, String> {
    let manager = manager.lock().unwrap();
    let mut plot_data_to_merge = Vec::with_capacity(ids.len());

    for id in ids {
        if let Some(pipeline_mutex) = manager.pipelines.get(&id) {
            let mut pipeline = pipeline_mutex.lock().unwrap();
            pipeline.update(&capture);
            plot_data_to_merge.push(pipeline.get_output().clone());
        }
    }

    if plot_data_to_merge.is_empty() {
        return Ok(PlotData::empty());
    }

    Ok(k_way_merge_plot_data(plot_data_to_merge))
}

#[tauri::command]
pub fn get_statistics_data(
    id: PipelineId,
    manager: State<Arc<Mutex<ProcessingManager>>>,
    capture: State<CaptureState>,
) -> Result<StreamStatistics, String> {
    let manager = manager.lock().unwrap();
    manager.get_statistics_data(id, &capture)
        .ok_or_else(|| "Statistics provider not found".to_string())
}

#[tauri::command]
pub fn confirm_selection(
    port_url: String,
    children_routes: Vec<String>,
    capture: State<CaptureState>,
    registry: State<Arc<ProxyRegister>>,
) -> Result<(), String> {
    println!(
        "[{}] Confirming selection with child routes: {:?}",
        port_url, children_routes
    );
    println!( "[{}] Getting PortManager...", port_url);
    let port_manager = match registry.get(&port_url) {
        Some(pm) => pm,
        None => return Err(format!("Could not find PortManager for URL: {}", port_url)),
    };
    println!( "[{}]   -> Obtained PortManager", port_url);


    println!( "[{}] Getting PortState...", port_url);
    let current_state = port_manager.state.lock().unwrap().clone();
    if !matches!(current_state, PortState::Streaming) {
        return Err(format!(
            "Cannot confirm selection: port '{}' is not streaming. Current state: {:?}",
            port_url, current_state
        ));
    }
    println!( "[{}]   -> Confirmed PortState is streaming", port_url);

    let mut keys_to_activate: Vec<DataColumnId> = Vec::new();
    let mut all_selected_routes = children_routes;
    all_selected_routes.push("".to_string());

    println!( "[{}] Getting HashMap over DeviceRoutes...", port_url);
    let devices_map = port_manager.devices.read()
        .map_err(|e| format!("Failed to access device list. A background task may have crashed. Details: {}", e))?;
    println!( "[{}]   -> Obtained HashMap", port_url);
    println!("[{}] Building DataColumnId(s)...", port_url);
    for route_str in all_selected_routes {
        let route = match DeviceRoute::from_str(&route_str) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if let Some(device_entry) = devices_map.get(&route) {
            let device_tuple = device_entry.lock().unwrap();
            let (_device, cached_ui_device) = &*device_tuple;

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
    
    let command = crate::state::capture::CaptureCommand::SetActiveColumns {
        port_url: port_url.clone(),
        keys_for_port: keys_to_activate.clone(),
    };
    capture.inner.command_tx.send(command).unwrap();

    println!("[{}] Sent command to activate {} data columns.", port_url, keys_to_activate.len());
    registry.active_selections.insert(port_url, keys_to_activate);

    Ok(())
}

#[tauri::command]
pub fn connect_to_port(
    port_url: String,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<(), String> {
    println!("[Command] connect_to_port: Ensuring connection to '{}'", port_url);
    registry.ensure(port_url);
    Ok(())
}