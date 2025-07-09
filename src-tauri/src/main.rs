// main.rs
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::Arc;
use tauri::{ AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;

use trendline_lib::{menu, proxy};
use trendline_lib::state::capture::CaptureState;
use trendline_lib::state::proxy_register::ProxyRegister;

fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    let window = app.get_webview_window("main").unwrap();
    match event.id().as_ref() {
        "open_recording" => {
            app.dialog().file().pick_file(move |path_buf| {
                if let Some(path) = path_buf {
                    println!("File selected for opening: {}", path.to_string());
                    window.emit("file-opened", path.to_string()).unwrap();
                }
            });
        }
        "save_recording" => {
            // Using the new dialog plugin API
            app.dialog().file()
                .add_filter("Trendline Recording", &["json"])
                .set_file_name("recording.json")
                .save_file(move |path_buf| {
                    if let Some(path) = path_buf {
                        println!("File selected for saving: {}", path.to_string());
                    }
                });
        }
        // ... other handlers remain the same
        _ => {}
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let menu = menu::create_app_menu(app.handle())?;
            app.set_menu(menu)?;

            let app_handle = app.handle().clone();
            app.on_menu_event(move |_app, event| {
                handle_menu_event(&app_handle, event);
            });

            let capture = CaptureState::new();
            let registry = Arc::new(ProxyRegister::new(app.handle().clone(), capture.clone()));

            app.manage(capture);
            app.manage(registry.clone());

            proxy::discovery::spawn(registry);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            trendline_lib::commands::capture::confirm_selection,
            trendline_lib::commands::capture::get_plot_data_in_range,
            trendline_lib::commands::capture::get_latest_plot_data,
            trendline_lib::commands::capture::get_latest_fft_data,
            trendline_lib::commands::settings::get_all_devices,
            trendline_lib::commands::settings::get_port_state
            // trendline_lib::commands::settings::execute_rpc,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
