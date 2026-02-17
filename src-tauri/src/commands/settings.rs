use serde_json::Value;
use std::sync::Arc;
use tauri::State;
use twinleaf::tio::proto::DeviceRoute;

use crate::shared::RpcError;
use crate::shared::{PortState, UiDevice};
use crate::state::comm_manager::{CommManager, CommMsg};
use crate::state::port_manager::PortMsg;

#[tauri::command]
pub fn get_all_devices(registry: State<Arc<CommManager>>) -> Vec<UiDevice> {
    let mut all_devices = Vec::new();

    let port_managers = match registry.ask(|reply| CommMsg::ListPorts { reply }) {
        Ok(ports) => ports,
        Err(e) => {
            eprintln!("[Command] get_all_devices failed to query ports: {}", e);
            return all_devices;
        }
    };

    for port_manager in port_managers {
        match port_manager.ask(|reply| PortMsg::GetDevices { reply }) {
            Ok(mut devices) => all_devices.append(&mut devices),
            Err(e) => {
                eprintln!(
                    "[{}] get_all_devices failed to query devices: {}",
                    port_manager.url, e
                );
            }
        }
    }

    println!(
        "[Command] get_all_devices returning {} devices.",
        all_devices.len()
    );
    all_devices
}

#[tauri::command]
pub fn get_port_state(
    port_url: String,
    registry: State<'_, Arc<CommManager>>,
) -> Result<PortState, String> {
    let port_manager = registry
        .ask(|reply| CommMsg::GetPort {
            port_url: port_url.clone(),
            reply,
        })?
        .ok_or_else(|| format!("Port manager for URL '{}' not found.", port_url))?;

    port_manager
        .ask(|reply| PortMsg::GetState { reply })
        .map_err(|e| format!("Failed to query state for '{}': {}", port_url, e))
}

#[tauri::command]
pub async fn execute_rpc(
    port_url: String,
    device_route: String,
    name: String,
    args: Option<Value>,
    registry: State<'_, Arc<CommManager>>,
) -> Result<Value, RpcError> {
    let route = DeviceRoute::from_str(&device_route)
        .map_err(|_| RpcError::AppLogic(format!("Invalid device route '{}'.", device_route)))?;

    let port_manager = registry
        .ask(|reply| CommMsg::GetPort {
            port_url: port_url.clone(),
            reply,
        })
        .map_err(RpcError::AppLogic)?
        .ok_or_else(|| RpcError::AppLogic(format!("Port '{}' not found.", port_url)))?;

    port_manager
        .ask(|reply| PortMsg::ExecuteRpc {
            route,
            name,
            args: args.unwrap_or(Value::Null),
            reply,
        })
        .map_err(RpcError::AppLogic)?
}
