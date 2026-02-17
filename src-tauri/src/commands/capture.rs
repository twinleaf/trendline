use crate::pipeline::manager::ProcessingManager;
use crate::state::comm_manager::{CommManager, CommMsg};
use crate::state::port_manager::PortMsg;
use std::sync::{Arc, Mutex};
use tauri::State;
use twinleaf::tio::proto::identifiers::ColumnKey;

#[tauri::command]
pub fn pause_plot(
    plot_id: String,
    start_time: f64,
    end_time: f64,
    manager: State<Arc<Mutex<ProcessingManager>>>,
    registry: State<Arc<CommManager>>,
) -> Result<(), String> {
    let mg = manager.lock().unwrap();
    let plot_config = mg
        .managed_plots
        .get(&plot_id)
        .ok_or_else(|| format!("Plot {} not found.", plot_id))?;

    let data_keys = &plot_config.config.data_keys;
    let port_url = data_keys
        .first()
        .ok_or("Plot has no data columns.")?
        .port_url
        .clone();

    let columns: Vec<ColumnKey> = data_keys
        .iter()
        .map(|id| ColumnKey::new(id.device_route.clone(), id.stream_id, id.column_index))
        .collect();

    let port_manager = registry
        .ask(|reply| CommMsg::GetPort { port_url, reply })
        .map_err(|e| format!("Failed to query port manager: {}", e))?
        .ok_or("Port not found")?;

    port_manager
        .ask(|reply| PortMsg::CreateSnapshot {
            id: plot_id,
            columns,
            start_time,
            end_time,
            reply,
        })
        .map_err(|e| format!("Failed to create snapshot: {}", e))?
        .map_err(|e| format!("Buffer read error: {:?}", e))
}

#[tauri::command]
pub fn unpause_plot(
    plot_id: String,
    manager: State<Arc<Mutex<ProcessingManager>>>,
    registry: State<Arc<CommManager>>,
) -> Result<(), String> {
    let mg = manager.lock().unwrap();
    let port_url = mg
        .managed_plots
        .get(&plot_id)
        .and_then(|p| p.config.data_keys.first())
        .map(|k| k.port_url.clone())
        .ok_or_else(|| format!("Plot {} not found.", plot_id))?;
    drop(mg);

    let port_manager = registry
        .ask(|reply| CommMsg::GetPort { port_url, reply })
        .map_err(|e| format!("Failed to query port manager: {}", e))?
        .ok_or("Port not found")?;

    port_manager
        .dispatch(PortMsg::ClearSnapshot { id: plot_id })
        .map_err(|e| format!("Failed to clear snapshot: {}", e))
}
