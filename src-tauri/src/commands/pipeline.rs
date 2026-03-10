use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    pipeline::manager::{PipelineManager, PipelineMsg},
    shared::{
        ColumnStatistics, MathChannelConfig, PipelineId, PlotData, PlotId, Point, UiColumnKey,
        UiPlotState, ViewConfig,
    },
    state::comm_manager::{CommManager, CommMsg},
    state::data_manager::{DataManager, DataMsg},
    state::port_manager::PortMsg,
};
use tauri::{ipc::Channel, State};

fn read_backfill(data_manager: &DataManager, key: &UiColumnKey, n: usize) -> Vec<Point> {
    data_manager
        .ask(|r| DataMsg::ReadWindow {
            columns: vec![key.clone()],
            n,
            reply: r,
        })
        .ok()
        .and_then(|res| res.ok())
        .map(|window| {
            let tl_key: twinleaf::tio::proto::identifiers::ColumnKey = key.into();
            if let Some(col_batch) = window.columns.get(&tl_key) {
                let timestamps = &window.timestamps;
                let values: Vec<f64> = match col_batch {
                    twinleaf::data::ColumnBatch::F64(v) => v.clone(),
                    twinleaf::data::ColumnBatch::I64(v) => v.iter().map(|&x| x as f64).collect(),
                    twinleaf::data::ColumnBatch::U64(v) => v.iter().map(|&x| x as f64).collect(),
                };
                timestamps
                    .iter()
                    .zip(values.iter())
                    .map(|(&t, &y)| Point { x: t, y })
                    .collect()
            } else {
                vec![]
            }
        })
        .unwrap_or_default()
}

#[tauri::command]
pub fn create_plot(manager: State<Arc<PipelineManager>>) -> Result<UiPlotState, String> {
    manager.ask(|reply| PipelineMsg::CreatePlot { reply })
}

#[tauri::command]
pub fn destroy_plot(
    plot_id: PlotId,
    manager: State<Arc<PipelineManager>>,
    registry: State<Arc<CommManager>>,
) -> Result<(), String> {
    if let Ok(Some(port_manager)) = registry.ask(|reply| CommMsg::GetActivePort { reply }) {
        let _ = port_manager.dispatch(PortMsg::ClearSnapshot {
            id: plot_id.0.to_string(),
        });
    }
    manager.dispatch(PipelineMsg::DestroyPlot { plot_id })
}

#[tauri::command]
pub fn update_plot_columns(
    plot_id: PlotId,
    data_keys: Vec<UiColumnKey>,
    view_config: ViewConfig,
    manager: State<Arc<PipelineManager>>,
    data_manager: State<DataManager>,
) -> Result<UiPlotState, String> {
    let mut rates = HashMap::new();
    let mut backfills = HashMap::new();

    for key in &data_keys {
        let rate = data_manager
            .ask(|r| DataMsg::GetSamplingRate {
                key: key.clone(),
                reply: r,
            })?
            .ok_or_else(|| {
                format!(
                    "No sampling rate for (route='{}', stream_id={}, column_id={}).",
                    key.route, key.stream_id, key.column_id
                )
            })?;

        if rate <= 0.0 || !rate.is_finite() {
            return Err(format!(
                "Invalid sampling rate ({}) for (route='{}', stream_id={}, column_id={}).",
                rate, key.route, key.stream_id, key.column_id
            ));
        }

        rates.insert(key.clone(), rate);
        backfills.insert(key.clone(), read_backfill(&data_manager, key, 65536));
    }

    let max_sampling_rate = rates.values().copied().fold(0.0_f64, f64::max);

    manager.ask(|reply| PipelineMsg::UpdatePlotColumns {
        plot_id,
        data_keys,
        max_sampling_rate,
        view_config,
        rates,
        backfills,
        reply,
    })?
}

#[tauri::command]
pub fn update_plot_view_config(
    plot_id: PlotId,
    view_config: ViewConfig,
    manager: State<Arc<PipelineManager>>,
    data_manager: State<DataManager>,
) -> Result<UiPlotState, String> {
    let data_keys = manager
        .ask(|reply| PipelineMsg::GetPlotDataKeys { plot_id, reply })?
        .ok_or("Plot not found")?;

    let mut backfills = HashMap::new();
    for key in &data_keys {
        backfills.insert(key.clone(), read_backfill(&data_manager, key, 65536));
    }

    manager.ask(|reply| PipelineMsg::UpdatePlotViewConfig {
        plot_id,
        view_config,
        backfills,
        reply,
    })?
}

#[tauri::command]
pub fn create_statistics_provider(
    source_key: UiColumnKey,
    window_seconds: f64,
    manager: State<Arc<PipelineManager>>,
) -> Result<PipelineId, String> {
    manager.ask(|reply| PipelineMsg::CreateStatisticsProvider {
        source_key,
        window_seconds,
        reply,
    })
}

#[tauri::command]
pub fn create_math_channel(
    spec: MathChannelConfig,
    manager: State<Arc<PipelineManager>>,
    data_manager: State<DataManager>,
) -> Result<(PipelineId, UiColumnKey), String> {
    let source_keys: Vec<UiColumnKey> = match &spec {
        MathChannelConfig::Arithmetic { a, b, .. } => vec![a.clone(), b.clone()],
        MathChannelConfig::Biquad { src, .. } => vec![src.clone()],
    };

    let mut input_rates = HashMap::new();
    for key in &source_keys {
        let rate = data_manager.ask(|r| DataMsg::GetSamplingRate {
            key: key.clone(),
            reply: r,
        })?;
        input_rates.insert(key.clone(), rate);
    }

    let result = manager.ask(|reply| PipelineMsg::CreateMathChannel {
        spec,
        input_rates,
        reply,
    })??;

    if let Some(rate) = result.output_rate {
        data_manager.dispatch(DataMsg::SetSamplingRate {
            key: result.output_key.clone(),
            rate,
        })?;
    }

    Ok((result.pipeline_id, result.output_key))
}

#[tauri::command]
pub async fn listen_to_plot_data(
    plot_id: PlotId,
    on_event: Channel<PlotData>,
    manager: tauri::State<'_, Arc<PipelineManager>>,
) -> Result<(), String> {
    manager.dispatch(PipelineMsg::RegisterPlotChannel {
        plot_id,
        channel: on_event,
    })
}

#[tauri::command]
pub async fn listen_to_statistics(
    id: PipelineId,
    on_event: Channel<ColumnStatistics>,
    manager: tauri::State<'_, Arc<PipelineManager>>,
) -> Result<(), String> {
    manager.dispatch(PipelineMsg::RegisterStatisticsChannel {
        provider_id: id,
        channel: on_event,
    })
}

#[tauri::command]
pub fn destroy_processor(
    id: PipelineId,
    manager: State<Arc<PipelineManager>>,
) -> Result<(), String> {
    manager.dispatch(PipelineMsg::DestroyProcessor { id })
}

#[tauri::command]
pub fn destroy_math_channel(
    output_key: UiColumnKey,
    manager: State<Arc<PipelineManager>>,
) -> Result<(), String> {
    manager.dispatch(PipelineMsg::DestroyMathChannel { output_key })
}

#[tauri::command]
pub fn reset_plot(plot_id: PlotId, manager: State<Arc<PipelineManager>>) -> Result<(), String> {
    manager.ask(|reply| PipelineMsg::ResetPlot { plot_id, reply })?
}

#[tauri::command]
pub fn reset_by_pipeline_id(
    id: PipelineId,
    manager: State<Arc<PipelineManager>>,
) -> Result<(), String> {
    manager.ask(|reply| PipelineMsg::ResetByPipelineId { id, reply })?
}

#[tauri::command]
pub fn reset_by_column(
    manager: tauri::State<Arc<PipelineManager>>,
    key: UiColumnKey,
) -> Result<(), String> {
    manager.ask(|reply| PipelineMsg::ResetByColumn { key, reply })?
}

#[tauri::command]
pub fn reset_by_stream(
    manager: tauri::State<Arc<PipelineManager>>,
    key: UiColumnKey,
) -> Result<(), String> {
    manager.ask(|reply| PipelineMsg::ResetByStream { key, reply })?
}

#[tauri::command]
pub fn reset_by_device(
    manager: tauri::State<Arc<PipelineManager>>,
    key: UiColumnKey,
) -> Result<(), String> {
    manager.ask(|reply| PipelineMsg::ResetByDevice { key, reply })?
}
