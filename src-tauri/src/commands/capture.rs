use crate::pipeline::manager::ProcessingManager;
use crate::shared::{DataColumnId, PortState};
use crate::state::capture::{CaptureCommand, CaptureState};
use crate::state::proxy_register::ProxyRegister;
use std::sync::{Arc, Mutex};
use tauri::State;
use twinleaf::tio::proto::DeviceRoute;

#[tauri::command]
pub fn pause_plot(
    plot_id: String,
    start_time: f64,
    end_time: f64,
    manager: State<Arc<Mutex<ProcessingManager>>>,
    capture_state: State<CaptureState>,
) -> Result<(), String> {
    let mg = manager.lock().unwrap();
    let plot_config = mg
        .managed_plots
        .get(&plot_id)
        .ok_or_else(|| format!("Plot {} not found.", plot_id))?;

    let command = CaptureCommand::CreateSnapshot {
        plot_id,
        keys: plot_config.config.data_keys.clone(),
        start_time,
        end_time,
    };
    capture_state
        .inner
        .command_tx
        .send(command)
        .map_err(|e| format!("Failed to send snapshot command: {}", e))
}

#[tauri::command]
pub fn unpause_plot(plot_id: String, capture_state: State<CaptureState>) -> Result<(), String> {
    let command = CaptureCommand::ClearSnapshot { plot_id };
    capture_state
        .inner
        .command_tx
        .send(command)
        .map_err(|e| format!("Failed to send clear snapshot command: {}", e))
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
    println!("[{}] Getting PortManager...", port_url);
    let port_manager = match registry.get(&port_url) {
        Some(pm) => pm,
        None => return Err(format!("Could not find PortManager for URL: {}", port_url)),
    };
    println!("[{}]   -> Obtained PortManager", port_url);

    println!("[{}] Getting PortState...", port_url);
    let current_state = port_manager.state.lock().unwrap().clone();
    if !matches!(current_state, PortState::Streaming) {
        return Err(format!(
            "Cannot confirm selection: port '{}' is not streaming. Current state: {:?}",
            port_url, current_state
        ));
    }
    println!("[{}]   -> Confirmed PortState is streaming", port_url);

    let mut keys_to_activate: Vec<DataColumnId> = Vec::new();
    let mut all_selected_routes = children_routes;
    all_selected_routes.push("".to_string());

    println!("[{}] Getting HashMap over DeviceRoutes...", port_url);
    let devices_map = port_manager.devices.read().map_err(|e| {
        format!(
            "Failed to access device list. A background task may have crashed. Details: {}",
            e
        )
    })?;
    println!("[{}]   -> Obtained HashMap", port_url);
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

    let command = CaptureCommand::SetActiveColumns {
        port_url: port_url.clone(),
        keys_for_port: keys_to_activate.clone(),
    };
    capture.inner.command_tx.send(command).unwrap();

    println!(
        "[{}] Sent command to activate {} data columns.",
        port_url,
        keys_to_activate.len()
    );
    registry
        .active_selections
        .insert(port_url, keys_to_activate);

    Ok(())
}

#[tauri::command]
pub fn connect_to_port(
    port_url: String,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<(), String> {
    println!(
        "[Command] connect_to_port: Ensuring connection to '{}'",
        port_url
    );
    registry.ensure(port_url);
    Ok(())
}
