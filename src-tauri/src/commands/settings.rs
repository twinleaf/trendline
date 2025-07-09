use tauri::State;
use std::sync::Arc;
use crate::state::proxy_register::ProxyRegister;
use crate::shared::{ UiDevice, PortState };

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