// src-tauri/src/device.rs
use crate::app_state::{AppState, SinglePlotPoint};
use tauri::{AppHandle, Emitter};
use twinleaf::data::{self, Device};
use twinleaf::tio::proxy;
use twinleaf::tio::proto::DeviceRoute;
use std::time::Instant;

pub trait AsF64 {
    /// A helper method to convert any numeric variant into an f64.
    fn as_f64(&self) -> Option<f64>;
}

// 2. Implement OUR trait for the FOREIGN type. This is allowed!
impl AsF64 for data::ColumnData {
    fn as_f64(&self) -> Option<f64> {
        match self {
            data::ColumnData::Int(value) => Some(*value as f64),
            data::ColumnData::UInt(value) => Some(*value as f64),
            data::ColumnData::Float(value) => Some(*value),
            data::ColumnData::Unknown => None,
        }
    }
}

pub fn start_stream_thread(
    port_url: String,
    stream_id: String,
    state: AppState,
    app_handle: AppHandle,
) {
    std::thread::spawn(move || {
        println!("[{}] Connecting to stream...", stream_id);

        let proxy = proxy::Interface::new(&port_url);
        let port_result = proxy.device_full(DeviceRoute::from_str(&stream_id).unwrap());
        let port = match port_result {
            Ok(p) => p,
            Err(e) => {
                let err_msg = format!("Failed to open port {}: {:?}", port_url, e);
                eprintln!("[{}] {}", stream_id, err_msg);
                app_handle.emit("device-error", err_msg).unwrap();
                return;
            }
        };

        let mut device = Device::new(port);
        let mut point_batch_for_frontend: Vec<SinglePlotPoint> = Vec::new();

        const BATCH_INTERVAL_MS: u128 = 16;
        println!("[{}] Connection successful. Streaming data...", stream_id);
        let mut last_batch_time = Instant::now();

        loop {
            let sample = device.next();
            for col in &sample.columns {
                if let Some(value) = col.value.as_f64() {
                    let series_key = format!(
                        "{}-{}-{}",
                        sample.device.name, sample.stream.name, col.desc.name
                    );

                    let point = SinglePlotPoint {
                        x: sample.timestamp_end(), // Use `x`
                        y: value,                  // Use `y`
                        series_key,
                    };
                    point_batch_for_frontend.push(point.clone());
                    
                    // Logic for storing in the main buffer
                    let mut buffer = state.buffer.lock().unwrap();
                    buffer
                        .entry(point.series_key)
                        .or_default()
                        .insert(point.x.to_bits(), point.y);
                }
            }

            if last_batch_time.elapsed().as_millis() >= BATCH_INTERVAL_MS {
                if !point_batch_for_frontend.is_empty() {
                    app_handle
                        .emit("new-data-available", &point_batch_for_frontend)
                        .unwrap();
                    point_batch_for_frontend.clear();
                }
                last_batch_time = Instant::now();
            }
        }
    });
}