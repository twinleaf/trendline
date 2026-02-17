use crate::shared::{DataColumnId, PlotData};
use crate::state::comm_manager::{CommManager, CommMsg};
use crate::state::port_manager::PortMsg;
use csv::WriterBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::DialogExt;
use twinleaf::data::{AlignedWindow, ColumnBatch};
use twinleaf::tio::proto::identifiers::ColumnKey;

#[derive(Deserialize)]
pub struct FrontendPlotData {
    timestamps: Vec<f64>,
    series_data: Vec<Vec<Option<f64>>>,
}

#[derive(Serialize, Clone)]
struct CsvExportPayload {
    message: String,
    path: Option<String>,
}

fn data_column_id_to_column_key(id: &DataColumnId) -> ColumnKey {
    ColumnKey::new(id.device_route.clone(), id.stream_id, id.column_index)
}

fn aligned_window_to_plot_data(window: &AlignedWindow, column_ids: &[DataColumnId]) -> PlotData {
    let timestamps = window.timestamps.clone();
    let series_data = column_ids
        .iter()
        .map(|id| {
            let key = data_column_id_to_column_key(id);
            match window.columns.get(&key) {
                Some(ColumnBatch::F64(v)) => v.clone(),
                Some(ColumnBatch::I64(v)) => v.iter().map(|&x| x as f64).collect(),
                Some(ColumnBatch::U64(v)) => v.iter().map(|&x| x as f64).collect(),
                None => vec![f64::NAN; timestamps.len()],
            }
        })
        .collect();
    PlotData {
        timestamps,
        series_data,
    }
}

fn read_live_raw_plot_data(
    data_column_ids: &[DataColumnId],
    start_time: f64,
    end_time: f64,
    registry: &CommManager,
) -> Result<PlotData, String> {
    let first = data_column_ids.first().ok_or("No data columns provided.")?;

    let port_manager = registry
        .ask(|reply| CommMsg::GetPort {
            port_url: first.port_url.clone(),
            reply,
        })
        .map_err(|e| format!("Failed to query port manager: {}", e))?
        .ok_or("Port not found")?;

    let columns: Vec<ColumnKey> = data_column_ids
        .iter()
        .map(data_column_id_to_column_key)
        .collect();

    let window = port_manager
        .ask(|reply| PortMsg::ReadTimeRange {
            columns,
            start_time,
            end_time,
            reply,
        })
        .map_err(|e| format!("Failed to read time range: {}", e))?
        .map_err(|e| format!("Buffer read error: {:?}", e))?;

    Ok(aligned_window_to_plot_data(&window, data_column_ids))
}

fn read_snapshot_plot_data(
    plot_id: &str,
    data_column_ids: &[DataColumnId],
    registry: &CommManager,
) -> Result<PlotData, String> {
    let first = data_column_ids.first().ok_or("No data columns provided.")?;

    let port_manager = registry
        .ask(|reply| CommMsg::GetPort {
            port_url: first.port_url.clone(),
            reply,
        })
        .map_err(|e| format!("Failed to query port manager: {}", e))?
        .ok_or("Port not found")?;

    let window = port_manager
        .ask(|reply| PortMsg::GetSnapshot {
            id: plot_id.to_string(),
            reply,
        })
        .map_err(|e| format!("Failed to query snapshot: {}", e))?
        .ok_or("The snapshot for this paused plot was not found. Please un-pause and re-pause.")?;

    Ok(aligned_window_to_plot_data(&window, data_column_ids))
}

fn format_plot_data_to_csv_string(
    plot_data: &PlotData,
    data_column_ids: &[DataColumnId],
    registry: &CommManager,
) -> Result<String, String> {
    if data_column_ids.is_empty() {
        return Err("No data columns provided for formatting.".to_string());
    }

    let mut headers: Vec<String> = vec!["time".to_string()];
    let mut data_types: Vec<String> = Vec::new();

    let is_single_device = {
        let mut unique_device_routes = HashSet::new();
        for id in data_column_ids {
            unique_device_routes.insert(id.device_route.clone());
        }
        unique_device_routes.len() <= 1
    };

    for data_col_id in data_column_ids {
        let port_manager = registry
            .ask(|reply| CommMsg::GetPort {
                port_url: data_col_id.port_url.clone(),
                reply,
            })
            .map_err(|e| format!("Failed to query port manager: {}", e))?
            .ok_or("Port not found")?;

        let device = port_manager
            .ask(|reply| PortMsg::GetDevice {
                route: data_col_id.device_route.clone(),
                reply,
            })
            .map_err(|e| format!("Failed to query device: {}", e))?
            .ok_or("Device not found")?;

        let ui_stream = device
            .streams
            .iter()
            .find(|s| s.meta.stream_id == data_col_id.stream_id)
            .ok_or("Stream not found")?;
        let column_meta = ui_stream
            .columns
            .iter()
            .find(|c| c.index == data_col_id.column_index)
            .ok_or("Column not found")?;

        data_types.push(column_meta.data_type.clone());

        let header_name = if is_single_device {
            column_meta.name.clone()
        } else {
            format!("{}.{}", data_col_id.device_route, column_meta.name)
        };
        headers.push(header_name);
    }

    let mut writer = WriterBuilder::new().from_writer(Cursor::new(Vec::new()));
    writer.write_record(&headers).map_err(|e| e.to_string())?;

    for (row_idx, timestamp) in plot_data.timestamps.iter().enumerate() {
        let mut record: Vec<String> = Vec::with_capacity(headers.len());
        record.push(format!("{:.6}", timestamp));

        for (col_idx, series) in plot_data.series_data.iter().enumerate() {
            let y_val = series.get(row_idx).copied().unwrap_or(f64::NAN);

            if y_val.is_nan() {
                record.push("".to_string());
                continue;
            }

            let data_type = &data_types[col_idx];

            let formatted_value = match data_type.as_str() {
                "I8" | "I16" | "I32" | "I64" | "U8" | "U16" | "U32" | "U64" => {
                    (y_val as i64).to_string()
                }
                _ => y_val.to_string(),
            };
            record.push(formatted_value);
        }
        writer.write_record(&record).map_err(|e| e.to_string())?;
    }

    String::from_utf8(writer.into_inner().map_err(|e| e.to_string())?.into_inner())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_plot_data_to_clipboard(
    app_handle: AppHandle,
    plot_data: FrontendPlotData,
    data_column_ids: Vec<DataColumnId>,
    registry: State<'_, Arc<CommManager>>,
) -> Result<(), String> {
    let internal_plot_data = PlotData {
        timestamps: plot_data.timestamps,
        series_data: plot_data
            .series_data
            .into_iter()
            .map(|series| {
                series
                    .into_iter()
                    .map(|opt_y| opt_y.unwrap_or(f64::NAN))
                    .collect()
            })
            .collect(),
    };

    let csv_string =
        format_plot_data_to_csv_string(&internal_plot_data, &data_column_ids, &registry)?;

    app_handle
        .clipboard()
        .write_text(csv_string)
        .map_err(|e| format!("Clipboard error: {:?}", e))?;

    let payload = CsvExportPayload {
        message: "Plotted data copied to clipboard!".to_string(),
        path: None,
    };
    app_handle
        .emit("csv-export-complete", payload)
        .map_err(|e| format!("Emit error: {:?}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn save_plot_data_to_file(
    app_handle: AppHandle,
    plot_data: FrontendPlotData,
    data_column_ids: Vec<DataColumnId>,
    file_name_suggestion: String,
    registry: State<'_, Arc<CommManager>>,
) -> Result<(), String> {
    let internal_plot_data = PlotData {
        timestamps: plot_data.timestamps,
        series_data: plot_data
            .series_data
            .into_iter()
            .map(|series| {
                series
                    .into_iter()
                    .map(|opt_y| opt_y.unwrap_or(f64::NAN))
                    .collect()
            })
            .collect(),
    };

    let csv_string =
        format_plot_data_to_csv_string(&internal_plot_data, &data_column_ids, &registry)?;

    let file_path_result = app_handle
        .dialog()
        .file()
        .add_filter("CSV", &["csv"])
        .set_title("Save Plotted Data")
        .set_file_name(&file_name_suggestion)
        .blocking_save_file();

    if let Some(file_path_enum) = file_path_result {
        let path_buf = file_path_enum
            .into_path()
            .map_err(|e| format!("Failed to resolve file path: {}", e))?;

        fs::write(&path_buf, csv_string).map_err(|e| format!("Failed to write file: {}", e))?;

        let success_msg = format!("Plotted data saved to {}", path_buf.to_string_lossy());

        let payload = CsvExportPayload {
            message: success_msg,
            path: Some(path_buf.to_string_lossy().to_string()),
        };
        app_handle
            .emit("csv-export-complete", payload)
            .map_err(|e| format!("Emit error: {:?}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn save_raw_plot_data_to_file(
    app_handle: AppHandle,
    plot_id: String,
    data_column_ids: Vec<DataColumnId>,
    start_time: f64,
    end_time: f64,
    is_paused: bool,
    file_name_suggestion: String,
    registry: State<'_, Arc<CommManager>>,
) -> Result<(), String> {
    let raw_plot_data = if is_paused {
        read_snapshot_plot_data(&plot_id, &data_column_ids, &registry)?
    } else {
        read_live_raw_plot_data(&data_column_ids, start_time, end_time, &registry)?
    };

    let csv_string = format_plot_data_to_csv_string(&raw_plot_data, &data_column_ids, &registry)?;

    let file_path_result = app_handle
        .dialog()
        .file()
        .add_filter("CSV", &["csv"])
        .set_title("Save Raw Plot Data")
        .set_file_name(&file_name_suggestion)
        .blocking_save_file();

    if let Some(file_path_enum) = file_path_result {
        let path_buf = file_path_enum
            .into_path()
            .map_err(|e| format!("Failed to resolve file path: {}", e))?;

        fs::write(&path_buf, csv_string).map_err(|e| format!("Failed to write file: {}", e))?;

        let success_msg = format!("Raw data snapshot saved to {}", path_buf.to_string_lossy());

        let payload = CsvExportPayload {
            message: success_msg,
            path: Some(path_buf.to_string_lossy().to_string()),
        };
        app_handle
            .emit("csv-export-complete", payload)
            .map_err(|e| format!("Emit error: {:?}", e))?;
    }

    Ok(())
}
