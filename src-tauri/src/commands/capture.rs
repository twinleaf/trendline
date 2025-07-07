use crate::state::capture::{CaptureState, DataColumnId};
use crate::shared::{PlotData, UiDevice};
use tauri::State;
use twinleaf::tio::proto::DeviceRoute;
use std::sync::Arc;
use crate::state::proxy_register::ProxyRegister;



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
