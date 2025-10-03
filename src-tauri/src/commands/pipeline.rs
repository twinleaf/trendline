use std::sync::{Arc, Mutex};

use crate::{
    pipeline::manager::ProcessingManager,
    shared::{DataColumnId, PipelineId, PlotData, SharedPlotConfig, ColumnStatistics},
};
use tauri::{ipc::Channel, State};

#[tauri::command]
pub fn update_plot_pipeline(
    config: SharedPlotConfig,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<(), String> {
    manager.lock().unwrap().apply_plot_config(config)?;
    Ok(())
}

#[tauri::command]
pub fn destroy_plot_pipeline(
    plot_id: String,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<(), String> {
    manager.lock().unwrap().destroy_plot_pipelines(&plot_id);
    Ok(())
}

#[tauri::command]
pub fn create_statistics_provider(
    source_key: DataColumnId,
    window_seconds: f64,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<PipelineId, String> {
    Ok(manager
        .lock()
        .unwrap()
        .create_statistics_provider(source_key, window_seconds))
}

#[tauri::command]
pub async fn listen_to_plot_data(
    plot_id: String,
    on_event: Channel<PlotData>,
    manager: tauri::State<'_, Arc<Mutex<ProcessingManager>>>,
) -> Result<(), String> {
    manager
        .lock()
        .unwrap()
        .register_plot_channel(plot_id, on_event);
    Ok(())
}

#[tauri::command]
pub async fn listen_to_statistics(
    id: PipelineId,
    on_event: Channel<ColumnStatistics>,
    manager: tauri::State<'_, Arc<Mutex<ProcessingManager>>>,
) -> Result<(), String> {
    manager
        .lock()
        .unwrap()
        .register_statistics_channel(id, on_event);
    Ok(())
}

#[tauri::command]
pub fn destroy_processor(id: PipelineId, manager: State<Arc<Mutex<ProcessingManager>>>) {
    manager.lock().unwrap().destroy(id);
}

#[tauri::command]
pub fn reset_by_pipeline_id(
    id: PipelineId,
    reset_capture_state: bool,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<(), String> {
    manager.lock().unwrap().reset_by_pipeline_id(id, reset_capture_state)
}

#[tauri::command]
pub fn reset_by_column(
    manager: tauri::State<Arc<Mutex<ProcessingManager>>>,
    key: DataColumnId,
    reset_capture_state: bool,
) -> Result<(), String> {
    manager.lock().unwrap().clear_column(&key, reset_capture_state)
}

#[tauri::command]
pub fn reset_by_stream(
    manager: tauri::State<Arc<Mutex<ProcessingManager>>>,
    key: DataColumnId,
    reset_capture_state: bool,
) -> Result<(), String> {
    manager.lock().unwrap().clear_stream(&key, reset_capture_state)
}

#[tauri::command]
pub fn reset_by_device(
    manager: tauri::State<Arc<Mutex<ProcessingManager>>>,
    key: DataColumnId,
    reset_capture_state: bool,
) -> Result<(), String> {
    manager.lock().unwrap().clear_device(&key, reset_capture_state)
}