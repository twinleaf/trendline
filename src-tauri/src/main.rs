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

use trendline_lib::pipeline::manager::PipelineManager;
use trendline_lib::state::comm_manager::CommManager;
use trendline_lib::state::data_manager::DataManager;
use trendline_lib::state::port_manager::DATA_BUFFER_CAPACITY;
use trendline_lib::{commands, menu};

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

            let data_manager = DataManager::new(DATA_BUFFER_CAPACITY);
            let pipeline_manager = Arc::new(PipelineManager::new(data_manager.sender()));
            let registry = Arc::new(CommManager::new(
                app.handle().clone(),
                data_manager.sender(),
                pipeline_manager.sender(),
            ));

            app.manage(data_manager);
            app.manage(registry.clone());
            app.manage(pipeline_manager);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // --- Port Management Commands ---
            commands::port::confirm_selection,
            commands::port::connect_to_port,
            commands::port::disconnect_port,
            commands::port::get_selection_state,
            commands::port::refresh_port,
            commands::port::set_discovery_polling,
            // --- Capture Management Commands ---
            commands::capture::pause_plot,
            commands::capture::unpause_plot,
            // --- Pipeline Commands ---
            commands::pipeline::create_plot,
            commands::pipeline::destroy_plot,
            commands::pipeline::update_plot_columns,
            commands::pipeline::update_plot_view_config,
            commands::pipeline::create_statistics_provider,
            commands::pipeline::create_math_channel,
            commands::pipeline::destroy_processor,
            commands::pipeline::destroy_math_channel,
            commands::pipeline::listen_to_plot_data,
            commands::pipeline::listen_to_statistics,
            commands::pipeline::reset_plot,
            commands::pipeline::reset_by_pipeline_id,
            commands::pipeline::reset_by_column,
            commands::pipeline::reset_by_stream,
            commands::pipeline::reset_by_device,
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
