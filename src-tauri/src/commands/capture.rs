use crate::state::capture::{CaptureState, DataColumnId};
use crate::shared::PlotData;
use tauri::State;

// This command takes a list of keys and returns all the buffered data for them.
#[tauri::command]
pub fn get_buffered_plot_data(
    keys: Vec<DataColumnId>,
    capture: State<CaptureState>,
) -> PlotData {
    capture.get_all_data_for_keys(&keys)
}