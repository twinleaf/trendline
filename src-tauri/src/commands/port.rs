use crate::shared::{DataColumnId, PortState};
use crate::state::capture::CaptureState;
use crate::state::proxy_register::ProxyRegister;
use std::sync::Arc;
use tauri::State;
use twinleaf::tio::proto::DeviceRoute;

#[tauri::command]
pub fn connect_to_port(
    port_url: String,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<(), String> {
    registry.ensure(port_url.clone());
    if let Some(pm) = registry.get(&port_url) {
        if matches!(*pm.state.lock().unwrap(), PortState::Error(_)) {
            pm.connect();
        }
    }
    Ok(())
}

#[tauri::command]
pub fn refresh_port(
    port_url: String,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<(), String> {
    let pm = registry
        .get(&port_url)
        .ok_or_else(|| format!("Port '{}' not found", port_url))?;
    pm.rescan();
    Ok(())
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
    let port_manager = registry
        .get(&port_url)
        .ok_or_else(|| format!("Could not find PortManager for URL: {}", port_url))?;

    let current_state = port_manager.state.lock().unwrap().clone();
    if !matches!(current_state, PortState::Streaming) {
        return Err(format!(
            "Cannot confirm selection: port '{}' is not streaming. Current state: {:?}",
            port_url, current_state
        ));
    }

    let mut keys_to_activate: Vec<DataColumnId> = Vec::new();
    let mut all_selected_routes = children_routes;
    all_selected_routes.push("".to_string());

    let devices_map = port_manager
        .devices
        .read()
        .map_err(|e| format!("Failed to access device list: {}", e))?;

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
    capture
        .inner
        .command_tx
        .send(command)
        .map_err(|e| e.to_string())?;

    println!(
        "[{}] Sent command to activate {} data columns.",
        port_url,
        keys_to_activate.len()
    );
    registry
        .active_selections
        .insert(port_url.clone(), keys_to_activate);
    registry.set_selected_port(Some(port_url.clone()));
    registry.shutdown_all_except(&port_url);
    
    Ok(())
}
