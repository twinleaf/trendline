use crate::pipeline::manager::ProcessingManager;
use crate::shared::PlotId;
use crate::state::comm_manager::{CommManager, CommMsg};
use crate::state::port_manager::PortMsg;
use std::sync::{Arc, Mutex};
use tauri::State;
use twinleaf::tio::proto::identifiers::ColumnKey as LibColumnKey;

#[tauri::command]
pub fn pause_plot(
    plot_id: PlotId,
    start_time: f64,
    end_time: f64,
    manager: State<Arc<Mutex<ProcessingManager>>>,
    registry: State<Arc<CommManager>>,
) -> Result<(), String> {
    let mg = manager.lock().unwrap();
    let plot_config = mg
        .managed_plots
        .get(&plot_id)
        .ok_or_else(|| format!("Plot {:?} not found.", plot_id))?;

    let data_keys = &plot_config.config.data_keys;
    let columns: Vec<LibColumnKey> = data_keys
        .iter()
        .map(|k| LibColumnKey::new(k.route.clone(), k.stream_id, k.column_id))
        .collect();

    let port_manager = registry
        .ask(|reply| CommMsg::GetActivePort { reply })
        .map_err(|e| format!("Failed to query port manager: {}", e))?
        .ok_or("No active port")?;

    port_manager
        .ask(|reply| PortMsg::CreateSnapshot {
            id: plot_id.0.to_string(),
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
    plot_id: PlotId,
    manager: State<Arc<Mutex<ProcessingManager>>>,
    registry: State<Arc<CommManager>>,
) -> Result<(), String> {
    let mg = manager.lock().unwrap();
    let _ = mg
        .managed_plots
        .get(&plot_id)
        .ok_or_else(|| format!("Plot {:?} not found.", plot_id))?;
    drop(mg);

    let port_manager = registry
        .ask(|reply| CommMsg::GetActivePort { reply })
        .map_err(|e| format!("Failed to query port manager: {}", e))?
        .ok_or("No active port")?;

    port_manager
        .dispatch(PortMsg::ClearSnapshot {
            id: plot_id.0.to_string(),
        })
        .map_err(|e| format!("Failed to clear snapshot: {}", e))
}
