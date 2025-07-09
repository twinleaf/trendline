use tauri::{State, Emitter};
use std::sync::Arc;
use serde_json::Value;
use tokio::sync::oneshot;
use twinleaf::tio::proto::DeviceRoute;


use crate::state::proxy_register::ProxyRegister;
use crate::shared::{ UiDevice, PortState };
use crate::proxy::port_manager::PortCommand;
use crate::shared::RpcError;



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
    
    let route = DeviceRoute::from_str(&device_route)
        .map_err(|_| RpcError::AppLogic(format!("Invalid device route string: '{}'", device_route)))?;
    
    let rpc_meta = {
        let devices = port_manager.devices.lock().unwrap();
        let (_device, ui_device) = devices.get(&route)
            .ok_or_else(|| RpcError::AppLogic("Device not found while fetching RPC meta.".to_string()))?;
        ui_device.rpcs.iter().find(|r| r.name == name)
            .cloned() // Clone the metadata so we can drop the lock
            .ok_or_else(|| RpcError::AppLogic(format!("RPC '{}' not found in metadata.", name)))?
    };

    let (tx, rx) = oneshot::channel();

    let cmd = PortCommand::ExecuteRpc {
        route: route.clone(),
        name: name.clone(),
        args: args.clone(),
        responder: tx,
    };

    port_manager.command_tx.send(cmd)
        .map_err(|_| RpcError::AppLogic("Command channel to port manager is closed.".to_string()))?;
        
    let result_value = rx.await.unwrap()?;

    if args.is_some() {
        if rpc_meta.readable {
            let (read_tx, read_rx) = oneshot::channel();
            let read_cmd = PortCommand::ExecuteRpc {
                route: route.clone(),
                name: name.clone(),
                args: None,
                responder: read_tx,
            };
            port_manager.command_tx.send(read_cmd).map_err(|_| RpcError::AppLogic("Read-back command failed.".to_string()))?;

            if let Ok(Ok(new_value)) = read_rx.await {
                 let mut devices = port_manager.devices.lock().unwrap();
                 if let Some((_d, ui_d)) = devices.get_mut(&route) {
                     if let Some(rpc) = ui_d.rpcs.iter_mut().find(|r| r.name == name) {
                         rpc.value = Some(new_value.clone());
                         port_manager.app.emit("rpc-value-updated", (
                             port_url.clone(), device_route.clone(), name.clone(), new_value
                         )).unwrap();
                     }
                 }
            }
        } else {
            port_manager.app.emit("rpc-write-success", (
                port_url.clone(),
                device_route.clone(),
                name.clone(),
            )).unwrap();
        }
    }
    
    Ok(result_value)
}