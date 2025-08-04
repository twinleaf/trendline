use tauri::{State, AppHandle, Emitter};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt};
use crate::shared::{DataColumnId, PlotData};
use crate::state::capture::CaptureState;
use crate::state::proxy_register::ProxyRegister;
use std::sync::Arc;
use std::collections::HashSet;
use csv::WriterBuilder;
use std::io::Cursor;
use std::fs;
use serde::Deserialize;


#[derive(Deserialize)]
pub struct FrontendPlotData {
    timestamps: Vec<f64>,
    series_data: Vec<Vec<Option<f64>>>,
}

fn format_plot_data_to_csv_string(
    plot_data: &PlotData,
    data_column_ids: &[DataColumnId],
    registry: &ProxyRegister,
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
        let port_manager = registry.get(&data_col_id.port_url).ok_or("Port not found")?;
        let devices_map = port_manager.devices.read().map_err(|e| format!("Lock error: {}", e))?;
        let device_entry = devices_map.get(&data_col_id.device_route).ok_or("Device not found")?;
        let device_tuple = device_entry.lock().unwrap();
        let (_device, ui_device) = &*device_tuple;
        let ui_stream = ui_device.streams.iter().find(|s| s.meta.stream_id == data_col_id.stream_id).ok_or("Stream not found")?;
        let column_meta = ui_stream.columns.iter().find(|c| c.index == data_col_id.column_index).ok_or("Column not found")?;
        
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
                },
                _ => { 
                    y_val.to_string()
                }
            };
            record.push(formatted_value);
        }
        writer.write_record(&record).map_err(|e| e.to_string())?;
    }

    String::from_utf8(writer.into_inner().map_err(|e| e.to_string())?.into_inner()).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_decimated_view_to_clipboard(
    app_handle: AppHandle,
    plot_data: FrontendPlotData,
    data_column_ids: Vec<DataColumnId>,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<(), String> {
    let internal_plot_data = PlotData {
        timestamps: plot_data.timestamps,
        series_data: plot_data.series_data.into_iter().map(|series| {
            series.into_iter().map(|opt_y| opt_y.unwrap_or(f64::NAN)).collect()
        }).collect(),
    };

    let csv_string = format_plot_data_to_csv_string(&internal_plot_data, &data_column_ids, &registry)?;
    
    app_handle.clipboard().write_text(csv_string)
        .map_err(|e| format!("Clipboard error: {:?}", e))?;

    app_handle.emit("csv-export-complete", "Decimated view copied to clipboard!")
        .map_err(|e| format!("Emit error: {:?}", e))?;

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
    capture_state: State<'_, CaptureState>,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<(), String> {

    let raw_plot_data = if is_paused {
        capture_state.inner.paused_snapshots.get(&plot_id)
            .map(|data_ref| data_ref.value().clone())
            .ok_or_else(|| "The snapshot for this paused plot was not found. Please un-pause and re-pause.".to_string())?
    } else {
        let raw_data_vecs = capture_state.get_data_across_sessions_for_keys(&data_column_ids, start_time, end_time);
        if raw_data_vecs.get(0).map_or(true, |v| v.is_empty()) {
            return Err("The requested time range is no longer in the live data buffer. To save raw data, please act more quickly.".to_string());
        }
        let mut individual_plot_data = Vec::with_capacity(data_column_ids.len());
        for points in raw_data_vecs {
            let timestamps: Vec<f64> = points.iter().map(|p| p.x).collect();
            let series_data: Vec<f64> = points.iter().map(|p| p.y).collect();
            individual_plot_data.push(PlotData { timestamps, series_data: vec![series_data] });
        }
        crate::util::k_way_merge_plot_data(individual_plot_data)
    };

    let csv_string = format_plot_data_to_csv_string(&raw_plot_data, &data_column_ids, &registry)?;

    let file_path_result = app_handle.dialog().file()
        .add_filter("CSV", &["csv"])
        .set_title("Save Raw Plot Data")
        .blocking_save_file();

    if let Some(file_path_enum) = file_path_result {
        let path = file_path_enum.into_path()
            .map_err(|e| format!("Failed to resolve file path: {}", e))?;

        fs::write(&path, csv_string).map_err(|e| format!("Failed to write file: {}", e))?;
        let success_msg = format!("Raw data snapshot saved to {}", path.to_string_lossy());
        app_handle.emit("csv-export-complete", success_msg).map_err(|e| format!("Emit error: {:?}", e))?;
    }
    
    Ok(())
}