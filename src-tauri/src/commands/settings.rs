use tauri::{async_runtime, State, Emitter};
use twinleaf::Device;
use std::sync::Arc;
use serde_json::Value;
use twinleaf::tio::proto::DeviceRoute;

use crate::state::proxy_register::ProxyRegister;
use crate::shared::{ UiDevice, PortState };
use crate::shared::RpcError;
use crate::util;



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

#[tauri::command]
pub fn get_port_state(
    port_url: String,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<PortState, String> {
    if let Some(port_manager) = registry.get(&port_url) {
        let state = port_manager.state.lock().unwrap().clone();
        Ok(state)
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

    let proxy_if = port_manager.proxy.lock().unwrap().clone()
        .ok_or_else(|| RpcError::AppLogic("Proxy interface not available.".to_string()))?;

    let route = DeviceRoute::from_str(&device_route)
        .map_err(|_| RpcError::AppLogic(format!("Invalid device route string: '{}'", device_route)))?;

    let rpc_meta = {
        let devices = port_manager.devices.lock().unwrap();
        let (_data_device, ui_device) = devices.get(&route)
            .ok_or_else(|| RpcError::AppLogic(format!("Device '{}' not found in cache.", route)))?;
        
        ui_device.rpcs.iter().find(|r| r.name == name)
            .cloned()
            .ok_or_else(|| RpcError::AppLogic(format!("RPC '{}' not found.", name)))?
    };

    let route_clone = route.clone();
    let name_clone = name.clone();
    let args_clone = args.clone();
    let rpc_meta_clone = rpc_meta.clone();

     let rpc_task_result = async_runtime::spawn_blocking(move || -> Result<Value, RpcError> {
        let rpc_port = proxy_if.device_rpc(route_clone)? ;
        let mut device = Device::new(rpc_port);
        let arg_bytes = util::json_to_bytes(args_clone, &rpc_meta_clone.arg_type)?;
        let reply_bytes = device.raw_rpc(&name_clone, &arg_bytes)?;
        util::bytes_to_json_value(&reply_bytes, &rpc_meta_clone.arg_type)
            .ok_or_else(|| RpcError::AppLogic("Failed to parse RPC reply".to_string()))
    }).await;

    match rpc_task_result {
        Ok(Ok(rpc_result)) => {
            let new_value_to_cache = if rpc_meta.writable && args.is_some() {
                args
            } else if rpc_meta.readable {
                Some(rpc_result.clone())
            } else {
                None
            };

            if let Some(new_val) = new_value_to_cache {
                let mut devices = port_manager.devices.lock().unwrap();
                if let Some((_, ui_device)) = devices.get_mut(&route) {
                    if let Some(cached_rpc) = ui_device.rpcs.iter_mut().find(|r| r.name == name) {
                        cached_rpc.value = Some(new_val);
                        port_manager.app.emit("device-metadata-updated", ui_device.clone()).unwrap();
                    }
                }
            }
            Ok(rpc_result)
        },
        Ok(Err(rpc_error)) => {
            eprintln!("[{}] RPC for '{}' failed: {:?}", port_manager.url, name, rpc_error);
            Err(rpc_error)
        },
        Err(join_error) => {
            eprintln!("[{}] RPC task for '{}' panicked: {}", port_manager.url, name, join_error);
            Err(RpcError::AppLogic(format!("RPC task panicked: {}", join_error)))
        }
    }
}