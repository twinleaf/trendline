// src-tauri/src/device.rs

use crate::app_state::{AppState, PlotDataPoint};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter}; // No `State` import needed here.
use twinleaf::data::Device;
use twinleaf::tio::{proxy, proto::DeviceRoute};
use twinleaf_tools::connect;
use ringbuf::traits::RingBuffer;

//
//
// ========================= THIS IS THE CRITICAL FIX =========================
// The function signature MUST take an OWNED `tauri::State<AppState>` (no lifetime <'_>),
// not a borrowed `tauri::State<'_, AppState>`. This allows it to be moved.
//
pub fn start_data_ingestion_thread(state: tauri::State<AppState>, app_handle: AppHandle) {
// ============================================================================
//
//

    std::thread::spawn(move || {
        println!("DATA THREAD: Attempting to auto-detect sensor...");
        let sensor_url = match connect::auto_detect_sensor() {
            Ok(url) => url,
            Err(e) => {
                eprintln!("DATA THREAD: Error: {:?}. Thread exiting.", e);
                app_handle.emit("device-error", format!("Failed to find sensor: {:?}", e)).unwrap();
                return;
            }
        };
        println!("DATA THREAD: Found sensor at: {}", sensor_url);

        let proxy = proxy::Interface::new(&sensor_url);
        let port = match proxy.device_full(DeviceRoute::root()) {
             Ok(p) => p,
             Err(e) => {
                eprintln!("DATA THREAD: Failed to create port: {:?}. Thread exiting.", e);
                app_handle.emit("device-error", format!("Failed to create port: {:?}", e)).unwrap();
                return;
             }
        };
        let mut device = Device::new(port);
        println!("DATA THREAD: Connection successful. Streaming data...");

        let mut last_notification_time = std::time::Instant::now();

        loop {
            let sample = device.next();

            if sample.stream.name == "imu" {
                let mut values = HashMap::new();
                for col in &sample.columns {
                    if let twinleaf::data::ColumnData::Float(val) = col.value {
                        values.insert(col.desc.name.clone(), val);
                    }
                }

                if !values.is_empty() {
                    let point = PlotDataPoint {
                        timestamp: sample.timestamp_end(),
                        values,
                    };

                    if let Ok(mut buffer) = state.buffer.lock() {
                        buffer.push_overwrite(point);
                    }
                    
                    if last_notification_time.elapsed().as_millis() > 16 {
                        app_handle.emit("new-data-available", ()).unwrap();
                        last_notification_time = std::time::Instant::now();
                    }
                }
            }
        }
    });
}