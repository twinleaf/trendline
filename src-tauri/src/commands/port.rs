use crate::shared::UiSelectionState;
use crate::state::comm_manager::{CommManager, CommMsg};
use std::sync::Arc;
use tauri::State;
use twinleaf::tio::proto::DeviceRoute;

#[tauri::command]
pub fn connect_to_port(
    port_url: String,
    registry: State<'_, Arc<CommManager>>,
) -> Result<(), String> {
    registry
        .ask(|reply| CommMsg::Connect { port_url, reply })?
        .map(|_| ())
}

#[tauri::command]
pub fn refresh_port(port_url: String, registry: State<'_, Arc<CommManager>>) -> Result<(), String> {
    registry.ask(|reply| CommMsg::RefreshPort { port_url, reply })?
}

#[tauri::command]
pub fn set_discovery_polling(
    enabled: bool,
    registry: State<'_, Arc<CommManager>>,
) -> Result<(), String> {
    registry.dispatch(CommMsg::SetDiscoveryPolling { enabled })?;
    Ok(())
}

#[tauri::command]
pub fn disconnect_port(
    port_url: String,
    registry: State<'_, Arc<CommManager>>,
) -> Result<(), String> {
    registry.ask(|reply| CommMsg::Disconnect { port_url, reply })?
}

#[tauri::command]
pub fn confirm_selection(
    port_url: String,
    children_routes: Vec<String>,
    registry: State<Arc<CommManager>>,
) -> Result<(), String> {
    let parsed_routes: Vec<DeviceRoute> = children_routes
        .into_iter()
        .map(|route| {
            DeviceRoute::from_str(&route).map_err(|_| format!("Invalid device route '{}'.", route))
        })
        .collect::<Result<_, _>>()?;

    registry.ask(|reply| CommMsg::ConfirmSelection {
        port_url,
        children_routes: parsed_routes,
        reply,
    })?
}

#[tauri::command]
pub fn get_selection_state(registry: State<Arc<CommManager>>) -> Result<UiSelectionState, String> {
    registry
        .ask(|reply| CommMsg::GetSelectionState { reply })
        .map_err(|e| format!("Failed to query selection state: {}", e))
}
