#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;

use app_state::{AppState, PlotDataPoint};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{Manager, State, AppHandle, Emitter};
use ringbuf::{traits::*};

// A command to add a new data point to the buffer.
#[tauri::command]
fn add_dummy_data(state: State<AppState>, app_handle: AppHandle) { // <-- Add AppHandle
    let mut buffer = state.buffer.lock().unwrap();

    let mut values = HashMap::new();
    // Use the keys expected by the frontend
    values.insert("imu.accel.x".to_string(), rand::random::<f64>() * 100.0 - 50.0);
    values.insert("imu.accel.y".to_string(), rand::random::<f64>() * 100.0 - 50.0);

    let new_point = PlotDataPoint {
        timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64(),
        values,
    };

    buffer.push_overwrite(new_point);
    println!("Added data point. Buffer size: {}", buffer.write_index());
    
    // Emit an event to the frontend to notify it that new data is available.
    app_handle.emit("new-data-available", ()).unwrap();
}

// A command to retrieve all data currently in the buffer.
#[tauri::command]
fn get_plot_data(state: State<AppState>) -> Vec<PlotDataPoint> {
    let buffer = state.buffer.lock().unwrap();
    let data = buffer.iter().cloned().collect();
    println!("Retrieved {} data points.", buffer.write_index());
    data
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            app.manage(AppState::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            add_dummy_data,
            get_plot_data
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}