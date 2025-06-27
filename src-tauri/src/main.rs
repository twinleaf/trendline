// main.rs
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Arc;
use tauri::Manager;
use trendline_lib::state::proxy_register::ProxyRegister;
use trendline_lib::state::capture::CaptureState;
use trendline_lib::proxy;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let capture = CaptureState::new();
            let registry = Arc::new(ProxyRegister::new(app.handle().clone(), capture.clone()));

            app.manage(capture);
            app.manage(registry.clone());

            proxy::discovery::spawn(registry);
            Ok(())
        })
        // .invoke_handler(tauri::generate_handler![
        //     trendline_lib::commands::capture::start_capture,
        //     trendline_lib::commands::capture::stop_capture,
        //     trendline_lib::commands::capture::get_plot_data_in_range,
        //     trendline_lib::commands::settings::execute_rpc,
        // ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
