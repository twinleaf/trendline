// main.rs
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use std::sync::Arc;
use tauri::Manager;

use trendline_lib::pipeline::manager::ProcessingManager;
use trendline_lib::state::capture::CaptureState;
use trendline_lib::state::proxy_register::ProxyRegister;
use trendline_lib::{commands, menu, proxy};


fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_prevent_default::debug())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let app_handle = app.handle().clone();

            let menu = menu::create_app_menu(app.handle())?;
            app.set_menu(menu)?;
            app.on_menu_event(move |_app, event| {
                menu::handle_menu_event(&app_handle, event);
            });

            let capture = CaptureState::new();
            let registry = Arc::new(ProxyRegister::new(app.handle().clone(), capture.clone()));
            let processing_manager = ProcessingManager::new_with_ticker(capture.clone());

            app.manage(capture);
            app.manage(registry.clone());
            app.manage(processing_manager);

            proxy::discovery::spawn(registry);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // --- Port Management Commands ---
            commands::port::confirm_selection,
            commands::port::connect_to_port,
            commands::port::refresh_port,
            // --- Capture Management Commands ---
            commands::capture::pause_plot,
            commands::capture::unpause_plot,
            // --- Pipeline Commands ---
            commands::pipeline::update_plot_pipeline,
            commands::pipeline::destroy_plot_pipeline,
            commands::pipeline::create_statistics_provider,
            commands::pipeline::destroy_processor,
            commands::pipeline::listen_to_plot_data,
            commands::pipeline::listen_to_statistics,
            commands::pipeline::reset_statistics_provider,
            // --- Pipeline Management Commands ---
            commands::settings::get_all_devices,
            commands::settings::get_port_state,
            commands::settings::execute_rpc,
            // ---  Export Command ---
            commands::export::export_plot_data_to_clipboard,
            commands::export::save_plot_data_to_file,
            commands::export::save_raw_plot_data_to_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
