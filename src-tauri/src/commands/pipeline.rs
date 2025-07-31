use std::sync::{Arc, Mutex};

use crate::{pipeline::{manager::ProcessingManager}, 
            shared::{DataColumnId, DetrendMethod, PipelineId}};
use tauri::{State};


#[tauri::command]
pub fn create_fpcs_pipeline(
    source_key: DataColumnId,
    ratio: usize,
    window_seconds: f64,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<PipelineId, String> {
    Ok(manager.lock().unwrap().create_fpcs_pipeline(source_key, ratio, window_seconds))
}


#[tauri::command]
pub fn create_detrend_pipeline(
    source_key: DataColumnId,
    window_seconds: f64,
    method: DetrendMethod,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<PipelineId, String> {
    Ok(manager.lock().unwrap().create_detrend_pipeline(source_key, window_seconds, method))
}

#[tauri::command]
pub fn create_passthrough_pipeline(
    source_key: DataColumnId,
    window_seconds: f64,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<PipelineId, String> {
    Ok(manager.lock().unwrap().create_passthrough_pipeline(source_key, window_seconds))
}

#[tauri::command]
pub fn create_fft_pipeline_from_source(
    source_pipeline_id: PipelineId,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<PipelineId, String> {
    manager.lock().unwrap().create_fft_pipeline_from_source(source_pipeline_id)
}

#[tauri::command]
pub fn create_statistics_provider(
    source_key: DataColumnId,
    window_seconds: f64,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<PipelineId, String> {
    Ok(manager.lock().unwrap().create_statistics_provider(source_key, window_seconds))
}


#[tauri::command]
pub fn destroy_processor(
    id: PipelineId,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) {
    manager.lock().unwrap().destroy(id);
}

#[tauri::command]
pub fn reset_statistics_provider(
    id: PipelineId,
    manager: State<Arc<Mutex<ProcessingManager>>>,
) -> Result<(), String> {
    let manager = manager.lock().unwrap();
    if let Some(provider_mutex) = manager.stat_providers.get(&id) {
        let mut provider = provider_mutex.lock().unwrap();
        provider.reset();
        Ok(())
    } else {
        Err("Statistics provider not found".to_string())
    }
}