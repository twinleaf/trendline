use crate::state::capture::{CaptureState};
use crate::shared::{DataColumnId, DecimationMethod, DetrendMethod, PlotData, Point, PortState, StatisticSet, StreamStatistics};
use crate::state::detrend::{remove_quadratic_trend, remove_linear_trend};
use crate::util::calculate_batch_stats;
use welch_sde::{Build, SpectralDensity};
use tauri::State;
use twinleaf::tio::proto::DeviceRoute;
use std::collections::HashMap;
use std::sync::Arc;
use crate::state::proxy_register::ProxyRegister;

#[tauri::command]
pub fn get_latest_plot_data(
    keys: Vec<DataColumnId>,
    window_seconds: f64,
    num_points: usize,
    capture: State<CaptureState>,
    decimation: Option<DecimationMethod>,
) -> PlotData {
    let Some(max_time) = capture.get_latest_unified_timestamp(&keys) else {
        return PlotData::empty();
    };
    let min_time = max_time - window_seconds;
    capture.get_plot_data(&keys, min_time, max_time, Some(num_points), decimation)
}

#[tauri::command]
pub fn get_plot_data_in_range(
    keys: Vec<DataColumnId>,
    min_time: f64,
    max_time: f64,
    capture: State<CaptureState>,
    num_points: Option<usize>,
    decimation: Option<DecimationMethod>,
) -> PlotData {
    capture.get_plot_data(&keys, min_time, max_time, num_points, decimation)
}

#[tauri::command]
pub fn get_decimated_delta(
    keys: Vec<DataColumnId>,
    window_seconds: f64,
    end_timestamp: f64,
    num_points: usize,
    decimation: Option<DecimationMethod>,
    since_timestamp: f64,
    capture: State<CaptureState>,
) -> PlotData {
    let min_timestamp = end_timestamp - window_seconds;
    let full_decimated_data = capture.get_plot_data(
        &keys,
        min_timestamp,
        end_timestamp,
        Some(num_points),
        decimation,
    );

    if full_decimated_data.timestamps.is_empty() {
        return PlotData::empty();
    }
    // STUPID BUT WORKS (duplicates decimation calculation and then linear searches -- but it's over decimated vector so not as bad as it looks)
    let start_index = full_decimated_data.timestamps
        .iter()
        .position(|&t| t > since_timestamp)
        .unwrap_or(full_decimated_data.timestamps.len());
    
    let delta_timestamps = full_decimated_data.timestamps[start_index..].to_vec();
    let delta_series_data = full_decimated_data.series_data
        .iter()
        .map(|series| series[start_index..].to_vec())
        .collect();

    PlotData {
        timestamps: delta_timestamps,
        series_data: delta_series_data,
    }
}


#[tauri::command]
pub fn get_latest_fft_data(
    keys: Vec<DataColumnId>,
    window_seconds: f64,
    detrend: DetrendMethod,
    capture: State<CaptureState>,
) -> Result<PlotData, String> {
    if keys.is_empty() {
        return Ok(PlotData::empty());
    }

    let latest_time = match capture.get_latest_unified_timestamp(&keys) {
        Some(t) => t,
        None => return Ok(PlotData::empty()),
    };
    let min_time = latest_time - window_seconds;

    let mut all_asds: Vec<Vec<f64>> = Vec::with_capacity(keys.len());
    let mut frequencies: Option<Vec<f64>> = None;

    for key in &keys {

        let stitched_series_vec: Vec<Vec<Point>> = capture.get_data_across_sessions_for_keys(
            &[key.clone()],
            min_time,
            latest_time,
        );

        let points = if let Some(p) = stitched_series_vec.get(0) {
            p
        } else {
            all_asds.push(vec![]);
            continue;
        };

        if points.len() < 16 {
            all_asds.push(vec![]);
            continue;
        }

        let stream_key = key.stream_key();
        let sampling_rate = capture
            .get_effective_sampling_rate(&stream_key)
            .ok_or_else(|| format!("Effective sampling rate not found for stream: {:?}", stream_key))?;

        let y_values: Vec<f64> = points.iter().map(|p| p.y).collect();

        let signal_to_process = match detrend {
            DetrendMethod::None => {
                let mean = y_values.iter().sum::<f64>() / y_values.len() as f64;
                y_values.iter().map(|&y| y - mean).collect()
            }
            DetrendMethod::Linear => remove_linear_trend(&y_values),
            DetrendMethod::Quadratic => remove_quadratic_trend(&y_values),
        };

        let welch: SpectralDensity<f64> =
            SpectralDensity::builder(&signal_to_process, sampling_rate).build();

        let psd = welch.periodogram();
        let asd: Vec<f64> = psd.to_vec().iter().map(|&p| p.sqrt()).collect();
        all_asds.push(asd);

        if frequencies.is_none() {
            frequencies = Some(psd.frequency().to_vec());
        }
    }

    Ok(PlotData {
        timestamps: frequencies.unwrap_or_default(),
        series_data: all_asds,
    })
}

#[tauri::command]
pub fn get_interpolated_values_at_time(
    keys: Vec<DataColumnId>,
    time: f64,
    capture: State<CaptureState>,
) -> Vec<Option<f64>> {
    capture.get_interpolated_values_at_time(&keys, time)
}

#[tauri::command]
pub fn get_stream_statistics(
    keys: Vec<DataColumnId>,
    window_seconds: f64,
    capture: State<CaptureState>,
) -> Result<HashMap<String, StreamStatistics>, String> {
    if keys.is_empty() {
        return Ok(HashMap::new());
    }

    let Some(max_time) = capture.get_latest_unified_timestamp(&keys) else {
        return Ok(HashMap::new());
    };
    let min_time = max_time - window_seconds;

    let mut stats_map = HashMap::new();

    for key in keys {
        let stitched_series_vec = capture.get_data_across_sessions_for_keys(&[key.clone()], min_time, max_time);

        let (window_stats, latest_value) = if let Some(points) = stitched_series_vec.get(0) {
            if points.is_empty() {
                (StatisticSet::default(), 0.0)
            } else {
                (calculate_batch_stats(points), points.last().unwrap().y)
            }
        } else {
            (StatisticSet::default(), 0.0)
        };

        let persistent_stats = if let Some(stat_mutex) = capture.inner.persistent_stats.get(&key) {
            let stat = stat_mutex.lock().unwrap();
            StatisticSet::from(&*stat)
        } else {
            StatisticSet::default()
        };

        let stats = StreamStatistics {
            latest_value,
            persistent: persistent_stats,
            window: window_stats,
        };

        let key_str = serde_json::to_string(&key).map_err(|e| format!("Failed to serialize DataColumnId: {}", e))?;

        stats_map.insert(key_str, stats);
    }

    Ok(stats_map)
}

#[tauri::command]
pub fn confirm_selection(
    port_url: String,
    children_routes: Vec<String>,
    capture: State<CaptureState>,
    registry: State<Arc<ProxyRegister>>,
) -> Result<(), String> {
    println!(
        "[{}] Confirming selection with child routes: {:?}",
        port_url, children_routes
    );
    println!( "[{}] Getting PortManager...", port_url);
    let port_manager = match registry.get(&port_url) {
        Some(pm) => pm,
        None => return Err(format!("Could not find PortManager for URL: {}", port_url)),
    };
    println!( "[{}]   -> Obtained PortManager", port_url);


    println!( "[{}] Getting PortState...", port_url);
    let current_state = port_manager.state.lock().unwrap().clone();
    if !matches!(current_state, PortState::Streaming) {
        return Err(format!(
            "Cannot confirm selection: port '{}' is not streaming. Current state: {:?}",
            port_url, current_state
        ));
    }
    println!( "[{}]   -> Confirmed PortState is streaming", port_url);

    let mut keys_to_activate: Vec<DataColumnId> = Vec::new();
    let mut all_selected_routes = children_routes;
    all_selected_routes.push("".to_string());

    println!( "[{}] Getting HashMap over DeviceRoutes...", port_url);
    let devices_map = port_manager.devices.read()
        .map_err(|e| format!("Failed to access device list. A background task may have crashed. Details: {}", e))?;
    println!( "[{}]   -> Obtained HashMap", port_url);
    println!("[{}] Building DataColumnId(s)...", port_url);
    for route_str in all_selected_routes {
        let route = match DeviceRoute::from_str(&route_str) {
            Ok(r) => r,
            Err(_) => continue,
        };

        if let Some(device_entry) = devices_map.get(&route) {
            let device_tuple = device_entry.lock().unwrap();
            let (_device, cached_ui_device) = &*device_tuple;

            for stream in &cached_ui_device.streams {
                for column in &stream.columns {
                    let key = DataColumnId {
                        port_url: port_url.clone(),
                        device_route: route.clone(),
                        stream_id: stream.meta.stream_id,
                        column_index: column.index,
                    };
                    keys_to_activate.push(key);
                }
            }
        }
    }
    
    capture.set_active_columns_for_port(&port_url, keys_to_activate.clone());

    println!("[{}] Caching selection of {} data columns.", port_url, keys_to_activate.len());
    registry.active_selections.insert(port_url, keys_to_activate);

    Ok(())
}

#[tauri::command]
pub fn connect_to_port(
    port_url: String,
    registry: State<'_, Arc<ProxyRegister>>,
) -> Result<(), String> {
    println!("[Command] connect_to_port: Ensuring connection to '{}'", port_url);
    registry.ensure(port_url);
    Ok(())
}
