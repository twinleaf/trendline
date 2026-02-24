use std::sync::{Arc, Mutex};

use crate::{
    pipeline::manager::ProcessingManager,
    shared::{
        ColumnKey, ColumnStatistics, MathChannelConfig, PipelineId, PlotData, PlotId,
        SharedPlotConfig,
    },
    state::comm_manager::{CommManager, CommMsg},
    state::port_manager::PortMsg,
};
use tauri::{ipc::Channel, State};

#[tauri::command]
pub fn create_plot(manager: State<Arc<Mutex<ProcessingManager>>>) -> Result<PlotId, String> {
    Ok(manager.lock().unwrap().create_plot())
}

#[tauri::command]
pub fn destroy_plot(
    plot_id: PlotId,
    manager: State<Arc<Mutex<ProcessingManager>>>,
    registry: State<Arc<CommManager>>,
) -> Result<(), String> {
    if let Ok(Some(port_manager)) = registry.ask(|reply| CommMsg::GetActivePort { reply }) {
        let _ = port_manager.dispatch(PortMsg::ClearSnapshot {
            id: plot_id.0.to_string(),
        });
    }
    manager.lock().unwrap().destroy_plot(plot_id);
    Ok(())
}

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
    plot_id: PlotId,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<(), String> {
    manager.lock().unwrap().destroy_plot_pipelines(&plot_id);
    Ok(())
}

#[tauri::command]
pub fn create_statistics_provider(
    source_key: ColumnKey,
    window_seconds: f64,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<PipelineId, String> {
    Ok(manager
        .lock()
        .unwrap()
        .create_statistics_provider(source_key, window_seconds))
}

#[tauri::command]
pub fn create_math_channel(
    spec: MathChannelConfig,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<(PipelineId, ColumnKey), String> {
    manager.lock().unwrap().create_math_channel(spec)
}

#[tauri::command]
pub async fn listen_to_plot_data(
    plot_id: PlotId,
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
pub fn destroy_math_channel(
    output_key: ColumnKey,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<(), String> {
    // Need separate because otherwise tears down math_channel alongside visualization plots
    manager.lock().unwrap().destroy_virtual_stream(&output_key);
    Ok(())
}

#[tauri::command]
pub fn reset_by_pipeline_id(
    id: PipelineId,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<(), String> {
    manager.lock().unwrap().reset_by_pipeline_id(id)
}

#[tauri::command]
pub fn reset_by_column(
    manager: tauri::State<Arc<Mutex<ProcessingManager>>>,
    key: ColumnKey,
) -> Result<(), String> {
    manager.lock().unwrap().clear_column(&key)
}

#[tauri::command]
pub fn reset_by_stream(
    manager: tauri::State<Arc<Mutex<ProcessingManager>>>,
    key: ColumnKey,
) -> Result<(), String> {
    manager.lock().unwrap().clear_stream(&key)
}

#[tauri::command]
pub fn reset_by_device(
    manager: tauri::State<Arc<Mutex<ProcessingManager>>>,
    key: ColumnKey,
) -> Result<(), String> {
    manager.lock().unwrap().clear_device(&key)
}
