use crate::pipeline::manager::{PipelineManager, PipelineMsg};
use crate::shared::PlotId;
use crate::state::comm_manager::{CommManager, CommMsg};
use crate::state::port_manager::PortMsg;
use std::sync::Arc;
use tauri::State;

#[tauri::command]
pub fn pause_plot(
    plot_id: PlotId,
    start_time: f64,
    end_time: f64,
    manager: State<Arc<PipelineManager>>,
    registry: State<Arc<CommManager>>,
) -> Result<(), String> {
    let data_keys = manager
        .ask(|reply| PipelineMsg::GetPlotDataKeys { plot_id, reply })?
        .ok_or_else(|| format!("Plot {:?} not found.", plot_id))?;

    let port_manager = registry
        .ask(|reply| CommMsg::GetActivePort { reply })
        .map_err(|e| format!("Failed to query port manager: {}", e))?
        .ok_or("No active port")?;

    port_manager
        .ask(|reply| PortMsg::CreateSnapshot {
            id: plot_id.0.to_string(),
            columns: data_keys,
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
    manager: State<Arc<PipelineManager>>,
    registry: State<Arc<CommManager>>,
) -> Result<(), String> {
    // Verify plot exists
    manager
        .ask(|reply| PipelineMsg::GetPlotDataKeys { plot_id, reply })?
        .ok_or_else(|| format!("Plot {:?} not found.", plot_id))?;

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
