use crate::io::export::{self as export_io};
use crate::shared::{PlotData, PlotId, UiColumnKey};
use crate::state::comm_manager::CommManager;
use serde::Serialize;
use std::fs;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;

#[derive(Serialize, Clone)]
struct CsvExportPayload {
    message: String,
    path: Option<String>,
}

fn emit_csv_export_complete(
    app_handle: &AppHandle,
    message: String,
    path: Option<String>,
) -> Result<(), String> {
    let payload = CsvExportPayload { message, path };
    app_handle
        .emit("csv-export-complete", payload)
        .map_err(|e| format!("Emit error: {:?}", e))
}

fn save_csv_to_file(
    app_handle: &AppHandle,
    csv_string: String,
    title: &str,
    file_name_suggestion: &str,
) -> Result<Option<String>, String> {
    let file_path_result = app_handle
        .dialog()
        .file()
        .add_filter("CSV", &["csv"])
        .set_title(title)
        .set_file_name(file_name_suggestion)
        .blocking_save_file();

    let Some(file_path_enum) = file_path_result else {
        return Ok(None);
    };

    let path_buf = file_path_enum
        .into_path()
        .map_err(|e| format!("Failed to resolve file path: {}", e))?;
    fs::write(&path_buf, csv_string).map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(Some(path_buf.to_string_lossy().to_string()))
}

#[tauri::command]
pub async fn export_plot_data_to_clipboard(
    app_handle: AppHandle,
    plot_data: PlotData,
    data_column_ids: Vec<UiColumnKey>,
    registry: State<'_, Arc<CommManager>>,
) -> Result<(), String> {
    let csv_string = export_io::format_frontend_plot_data_to_csv_string(
        &plot_data,
        &data_column_ids,
        &registry,
    )?;

    app_handle
        .clipboard()
        .write_text(csv_string)
        .map_err(|e| format!("Clipboard error: {:?}", e))?;

    emit_csv_export_complete(
        &app_handle,
        "Plotted data copied to clipboard!".to_string(),
        None,
    )?;

    Ok(())
}

#[tauri::command]
pub async fn save_plot_data_to_file(
    app_handle: AppHandle,
    plot_data: PlotData,
    data_column_ids: Vec<UiColumnKey>,
    file_name_suggestion: String,
    registry: State<'_, Arc<CommManager>>,
) -> Result<(), String> {
    let csv_string = export_io::format_frontend_plot_data_to_csv_string(
        &plot_data,
        &data_column_ids,
        &registry,
    )?;

    if let Some(path) = save_csv_to_file(
        &app_handle,
        csv_string,
        "Save Plotted Data",
        &file_name_suggestion,
    )? {
        emit_csv_export_complete(
            &app_handle,
            format!("Plotted data saved to {}", path),
            Some(path),
        )?;
    }

    Ok(())
}

#[tauri::command]
pub async fn save_raw_plot_data_to_file(
    app_handle: AppHandle,
    plot_id: PlotId,
    data_column_ids: Vec<UiColumnKey>,
    start_time: f64,
    end_time: f64,
    is_paused: bool,
    file_name_suggestion: String,
    registry: State<'_, Arc<CommManager>>,
) -> Result<(), String> {
    let raw_plot_data = if is_paused {
        export_io::read_snapshot_plot_data(&plot_id.0.to_string(), &data_column_ids, &registry)?
    } else {
        export_io::read_live_raw_plot_data(&data_column_ids, start_time, end_time, &registry)?
    };

    let csv_string =
        export_io::format_plot_data_to_csv_string(&raw_plot_data, &data_column_ids, &registry)?;

    if let Some(path) = save_csv_to_file(
        &app_handle,
        csv_string,
        "Save Raw Plot Data",
        &file_name_suggestion,
    )? {
        emit_csv_export_complete(
            &app_handle,
            format!("Raw data snapshot saved to {}", path),
            Some(path),
        )?;
    }

    Ok(())
}
