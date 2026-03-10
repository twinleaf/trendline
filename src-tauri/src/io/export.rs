use crate::shared::{PlotData, UiColumnKey};
use crate::state::comm_manager::{CommManager, CommMsg};
use crate::state::port_manager::{PortManager, PortMsg};
use csv::WriterBuilder;
use std::collections::HashSet;
use std::io::Cursor;
use std::sync::Arc;
use twinleaf::data::{AlignedWindow, ColumnBatch};

struct CsvColumns {
    headers: Vec<String>,
    data_types: Vec<String>,
}

fn get_active_port_manager(registry: &CommManager) -> Result<Arc<PortManager>, String> {
    registry
        .ask(|reply| CommMsg::GetActivePort { reply })
        .map_err(|e| format!("Failed to query port manager: {}", e))?
        .ok_or("No active port".to_string())
}

fn resolve_column_headers(
    data_column_ids: &[UiColumnKey],
    port_manager: &PortManager,
) -> Result<CsvColumns, String> {
    let is_single_device = {
        let mut unique_device_routes = HashSet::new();
        for id in data_column_ids {
            unique_device_routes.insert(id.route.clone());
        }
        unique_device_routes.len() <= 1
    };

    let mut headers = Vec::with_capacity(data_column_ids.len());
    let mut data_types = Vec::with_capacity(data_column_ids.len());

    for data_col_id in data_column_ids {
        let device = port_manager
            .ask(|reply| PortMsg::GetDevice {
                route: data_col_id.route.clone(),
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
            .find(|c| c.index == data_col_id.column_id)
            .ok_or("Column not found")?;

        data_types.push(column_meta.data_type.clone());

        let header_name = if is_single_device {
            column_meta.name.clone()
        } else {
            format!("{}.{}", data_col_id.route, column_meta.name)
        };
        headers.push(header_name);
    }

    Ok(CsvColumns {
        headers,
        data_types,
    })
}

fn aligned_window_to_plot_data(window: &AlignedWindow, column_ids: &[UiColumnKey]) -> PlotData {
    let timestamps = window.timestamps.clone();
    let series_data = column_ids
        .iter()
        .map(|id| match window.columns.get(&id.into()) {
            Some(ColumnBatch::F64(v)) => v.iter().map(|&x| Some(x)).collect(),
            Some(ColumnBatch::I64(v)) => v.iter().map(|&x| Some(x as f64)).collect(),
            Some(ColumnBatch::U64(v)) => v.iter().map(|&x| Some(x as f64)).collect(),
            None => vec![None; timestamps.len()],
        })
        .collect();
    PlotData {
        timestamps,
        series_data,
    }
}

pub fn read_live_raw_plot_data(
    data_column_ids: &[UiColumnKey],
    start_time: f64,
    end_time: f64,
    registry: &CommManager,
) -> Result<PlotData, String> {
    let port_manager = get_active_port_manager(registry)?;

    let window = port_manager
        .ask(|reply| PortMsg::ReadTimeRange {
            columns: data_column_ids.to_vec(),
            start_time,
            end_time,
            reply,
        })
        .map_err(|e| format!("Failed to read time range: {}", e))?
        .map_err(|e| format!("Buffer read error: {:?}", e))?;

    Ok(aligned_window_to_plot_data(&window, data_column_ids))
}

pub fn read_snapshot_plot_data(
    plot_id: &str,
    data_column_ids: &[UiColumnKey],
    registry: &CommManager,
) -> Result<PlotData, String> {
    let port_manager = get_active_port_manager(registry)?;

    let window = port_manager
        .ask(|reply| PortMsg::GetSnapshot {
            id: plot_id.to_string(),
            reply,
        })
        .map_err(|e| format!("Failed to query snapshot: {}", e))?
        .ok_or("The snapshot for this paused plot was not found. Please un-pause and re-pause.")?;

    Ok(aligned_window_to_plot_data(&window, data_column_ids))
}

pub fn format_plot_data_to_csv_string(
    plot_data: &PlotData,
    data_column_ids: &[UiColumnKey],
    registry: &CommManager,
) -> Result<String, String> {
    if data_column_ids.is_empty() {
        return Err("No data columns provided for formatting.".to_string());
    }

    let port_manager = get_active_port_manager(registry)?;
    let resolved_columns = resolve_column_headers(data_column_ids, &port_manager)?;
    let mut headers: Vec<String> = vec!["time".to_string()];
    headers.extend(resolved_columns.headers);

    let mut writer = WriterBuilder::new().from_writer(Cursor::new(Vec::new()));
    writer.write_record(&headers).map_err(|e| e.to_string())?;

    for (row_idx, timestamp) in plot_data.timestamps.iter().enumerate() {
        let mut record: Vec<String> = Vec::with_capacity(headers.len());
        record.push(format!("{:.6}", timestamp));

        for (col_idx, series) in plot_data.series_data.iter().enumerate() {
            let y_opt = series.get(row_idx).copied().flatten();

            let y_val = match y_opt {
                Some(v) if v.is_finite() => v,
                _ => {
                    record.push("".to_string());
                    continue;
                }
            };

            let data_type = &resolved_columns.data_types[col_idx];

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

pub fn format_frontend_plot_data_to_csv_string(
    plot_data: &PlotData,
    data_column_ids: &[UiColumnKey],
    registry: &CommManager,
) -> Result<String, String> {
    format_plot_data_to_csv_string(plot_data, data_column_ids, registry)
}
