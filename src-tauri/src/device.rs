// src-tauri/src/device.rs
use crate::app_state::{AppState};
use tauri::{AppHandle};
use std::time::{Duration, Instant};
use twinleaf::data::{self, Device};
use twinleaf::tio::proxy;

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
    port: proxy::Port,
    stream_id: String,
    state: AppState,
    _app_handle: AppHandle, 
) {
    std::thread::spawn(move || {
        println!("[{}] Stream thread started.", stream_id);

        let mut device = Device::new(port);

        // --- Add periodic logging state ---
        let mut last_log_time = Instant::now();
        let mut samples_since_last_log = 0u64;
        let log_interval = Duration::from_secs(5);
        // ---

        loop {
            let sample = device.next();
            samples_since_last_log += 1;

            for col in &sample.columns {
                if let Some(value) = col.value.as_f64() {
                    let series_key = format!(
                        "{}-{}-{}",
                        stream_id, sample.stream.name, col.desc.name
                    );

                    let mut buffer = state.buffer.lock().unwrap();
                    buffer
                        .entry(series_key)
                        .or_default()
                        .insert(sample.timestamp_end().to_bits(), value);
                }
            }

            // --- Check if it's time to log a summary ---
            if last_log_time.elapsed() >= log_interval {
                println!(
                    "[{}] Processed {} samples in the last {:.2}s.",
                    stream_id,
                    samples_since_last_log,
                    last_log_time.elapsed().as_secs_f32()
                );
                last_log_time = Instant::now();
                samples_since_last_log = 0;
            }
            // ---
        }
    });
}