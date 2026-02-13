use crate::shared::{DataColumnId, PortState};
use crate::state::capture::{CaptureCommand, CaptureState};
use crate::state::proxy_register::ProxyRegister;
use std::sync::Arc;
use tauri::{Emitter, State};
use twinleaf::tio::proto::DeviceRoute;

#[tauri::command]
pub fn connect_to_port(
    port_url: String,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<(), String> {
    registry.allow_serial(&port_url);
    registry.set_selected_port(Some(port_url.clone()));
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
pub fn disconnect_port(
    port_url: String,
    capture: State<'_, CaptureState>,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<(), String> {
    registry.ignore_serial(&port_url);

    if let Some((_url, pm)) = registry.ports.remove(&port_url) {
        pm.shutdown();
    }

    registry.active_selections.remove(&port_url);

    if registry.selected().as_deref() == Some(port_url.as_str()) {
        registry.set_selected_port(None);
    }

    capture
        .inner
        .command_tx
        .send(CaptureCommand::SetActiveColumns {
            port_url: port_url.clone(),
            keys_for_port: Vec::new(),
        })
        .map_err(|e| format!("Failed to clear active columns for '{}': {}", port_url, e))?;

    registry
        .app_handle()
        .emit("device-removed", port_url.clone())
        .map_err(|e| format!("Failed to emit device-removed for '{}': {}", port_url, e))?;

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
    let mut selected_routes: Vec<DeviceRoute> = Vec::new();
    let mut all_selected_routes = children_routes;
    all_selected_routes.push("".to_string());

    {
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
                selected_routes.push(route.clone());
                let device_tuple = device_entry.lock().unwrap();
                for stream in &device_tuple.streams {
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

    port_manager
        .refresh_rpc_cache_for_routes(&selected_routes)
        .map_err(|e| format!("Failed to refresh RPC metadata after selection: {:?}", e))?;

    Ok(())
}
