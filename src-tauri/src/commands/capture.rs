use crate::pipeline::manager::ProcessingManager;
use crate::state::capture::{CaptureCommand, CaptureState};
use std::sync::{Arc, Mutex};
use tauri::State;

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
