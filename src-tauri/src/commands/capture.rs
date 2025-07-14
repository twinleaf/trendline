use crate::state::capture::{CaptureState, DataColumnId, Point};
use crate::shared::{PlotData, PortState};
use welch_sde::{Build, SpectralDensity};
use tauri::State;
use twinleaf::tio::proto::DeviceRoute;
use std::sync::Arc;
use crate::state::proxy_register::ProxyRegister;

#[tauri::command]
pub fn get_plot_data_in_range(
    keys: Vec<DataColumnId>,
    min_time: f64,
    max_time: f64,
    capture: State<CaptureState>,
    num_points: Option<usize>,
) -> PlotData {
    capture.get_data_in_range(&keys, min_time, max_time, num_points)
}

#[tauri::command]
pub fn get_latest_plot_data(
    keys: Vec<DataColumnId>,
    window_seconds: f64,
    num_points: usize,
    capture: State<CaptureState>,
) -> PlotData {

    let latest_time = capture.get_latest_timestamp_for_keys(&keys);

    if latest_time.is_none() {
        return PlotData::empty();
    }

    let max_time = latest_time.unwrap();
    let min_time = max_time - window_seconds;

    capture.get_data_in_range(&keys, min_time, max_time, Some(num_points))
}

#[tauri::command]
pub fn get_latest_fft_data(
    keys: Vec<DataColumnId>,
    window_seconds: f64,
    capture: State<CaptureState>,
) -> Result<PlotData, String> {
    if keys.is_empty() {
        return Ok(PlotData::empty());
    }

    let latest_time = match capture.get_latest_timestamp_for_keys(&keys) {
        Some(t) => t,
        None => return Ok(PlotData::empty()),
    };
    let min_time = latest_time - window_seconds;

    let mut all_series_points: Vec<Vec<Point>> = Vec::new();
    let mut first_sampling_rate: Option<f64> = None;

    for key in &keys {
        let stream_key = key.stream_key();
        let effective_sampling_rate = capture.get_effective_sampling_rate(&stream_key)
            .ok_or_else(|| format!("Effective sampling rate not found for stream: {:?}", stream_key))?;

        if let Some(first_rate) = first_sampling_rate {
            if (effective_sampling_rate - first_rate).abs() > 1e-6 { // Floating point comparison
                return Err("All streams must have the same effective sampling rate.".to_string());
            }
        } else {
            first_sampling_rate = Some(effective_sampling_rate);
        }

        let points = capture.inner.buffers.get(key).map_or(vec![], |buffer_ref| {
            let offset = capture.inner.offsets.get(key).map_or(0.0, |off| *off.value());
            let min_key = (min_time - offset).to_bits();
            let max_key = (latest_time - offset).to_bits();
            buffer_ref
                .data
                .range(min_key..=max_key)
                .map(|(&t_bits, &y)| Point::new(f64::from_bits(t_bits) + offset, y))
                .collect()
        });

        if !points.is_empty() {
            all_series_points.push(points);
        }
    }

    if all_series_points.is_empty() {
        return Ok(PlotData::empty());
    }

    let sampling_rate = first_sampling_rate.ok_or("Could not determine sampling rate.")?;
    let mut all_asds: Vec<Vec<f64>> = Vec::new();
    let mut frequencies: Option<Vec<f64>> = None;

    for points in all_series_points {
        let y_values: Vec<f64> = points.iter().map(|p| p.y).collect();
        if y_values.len() < 16 { // Welch's method needs a minimum number of points
            all_asds.push(vec![]);
            continue;
        }

        let mean = y_values.iter().sum::<f64>() / y_values.len() as f64;
        let mean_adjusted_signal: Vec<f64> = y_values.iter().map(|&y| y - mean).collect();

        let welch: SpectralDensity<f64> =
            SpectralDensity::builder(&mean_adjusted_signal, sampling_rate).build();

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
pub fn confirm_selection(
    port_url: String,
    children_routes: Vec<String>,
    capture: State<CaptureState>,
    registry: State<Arc<ProxyRegister>>,
) -> Result<(), String> {
    println!(
        "Confirming selection for port '{}' with child routes: {:?}",
        port_url, children_routes
    );

    let port_manager = match registry.get(&port_url) {
        Some(pm) => pm,
        None => return Err(format!("Could not find PortManager for URL: {}", port_url)),
    };

    let current_state = port_manager.state.lock().unwrap().clone();
    if !matches!(current_state, PortState::Streaming) {
        return Err(format!(
            "Cannot confirm selection: port '{}' is not streaming. Current state: {:?}",
            port_url, current_state
        ));
    }

    let mut keys_to_activate: Vec<DataColumnId> = Vec::new();
    let mut all_selected_routes = children_routes;
    all_selected_routes.push("".to_string());

    let devices_map = port_manager.devices.read()
        .map_err(|e| format!("Failed to access device list. A background task may have crashed. Details: {}", e))?;

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
    
    println!("Activating {} total data columns for port '{}'.", keys_to_activate.len(), port_url);
    capture.set_active_columns_for_port(&port_url, keys_to_activate);

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
