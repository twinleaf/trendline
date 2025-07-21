use tauri::{State};
use std::sync::Arc;
use serde_json::Value;

use crate::state::proxy_register::ProxyRegister;
use crate::shared::{ UiDevice, PortState };
use crate::shared::RpcError;



#[tauri::command]
pub fn get_all_devices(registry: State<Arc<ProxyRegister>>) -> Vec<UiDevice> {
    let mut all_devices = Vec::new();

    for entry in registry.ports.iter() {
        let port_manager = entry.value();
        
        let devices_map = match port_manager.devices.read() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!(
                    "[{}] Could not acquire device lock for state hydration (poisoned: {}). Skipping.",
                    port_manager.url, e
                );
                continue;
            }
        };

        
        for device_entry in devices_map.values() {
            let device_tuple = match device_entry.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    eprintln!(
                        "[{}] A device lock was poisoned during state hydration (poisoned: {}). Skipping device.",
                        port_manager.url, e
                    );
                    continue;
                }
            };

            let (_device, ui_device) = &*device_tuple;
            all_devices.push(ui_device.clone());
        }
    }
    
    println!("[Command] get_all_devices returning {} devices.", all_devices.len());
    all_devices
}

#[tauri::command]
pub fn get_port_state(
    port_url: String,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<PortState, String> {
    if let Some(port_manager) = registry.get(&port_url) {
        match port_manager.state.lock() {
            Ok(guard) => Ok(guard.clone()),
            Err(_poisoned) => {
                eprintln!("[{}] State lock was poisoned. Returning last known state.", port_url);
                Err(format!("Port '{}' is in an inconsistent state (lock poisoned).", port_url))
            }
        }
    } else {
        Err(format!("Port manager for URL '{}' not found.", port_url))
    }
}

#[tauri::command]
pub async fn execute_rpc(
    port_url: String,
    device_route: String,
    name: String,
    args: Option<Value>,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<Value, RpcError> {
    let port_manager = registry.get(&port_url)
        .ok_or_else(|| RpcError::AppLogic(format!("Port '{}' not found.", port_url)))?;

    port_manager.execute_rpc(&device_route, &name, args).await
}