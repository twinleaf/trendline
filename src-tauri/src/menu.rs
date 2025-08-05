use tauri::{
    menu::{
        AboutMetadataBuilder, Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem,
        SubmenuBuilder,
    },
    AppHandle, Runtime,
};

pub fn create_app_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let about_metadata = AboutMetadataBuilder::new()
        .name(Some("Trendline"))
        .version(Some("0.1.0")) // Example: added version
        .authors(Some(vec!["Steven Nguyen".to_string()]))
        .website(Some("https://twinleaf.com"))
        .build();

    let app_menu = SubmenuBuilder::new(app_handle, "App")
        .item(&PredefinedMenuItem::about(
            app_handle,
            None,
            Some(about_metadata),
        )?)
        .separator()
        .item(&PredefinedMenuItem::services(app_handle, None)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app_handle, None)?)
        .item(&PredefinedMenuItem::hide_others(app_handle, None)?)
        .item(&PredefinedMenuItem::show_all(app_handle, None)?)
        .separator()
        .item(&PredefinedMenuItem::quit(app_handle, None)?)
        .build()?;

    let file_menu = SubmenuBuilder::with_id(app_handle, "file_menu", "&File")
        .item(
            &MenuItemBuilder::with_id("open_recording", "Open Recording...")
                .accelerator("CmdOrCtrl+O")
                .build(app_handle)?,
        )
        .item(
            &MenuItemBuilder::with_id("save_recording", "Save Recording As...")
                .accelerator("CmdOrCtrl+S")
                .build(app_handle)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("preferences", "Preferences...").build(app_handle)?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app_handle, Some("Exit"))?)
        .build()?;

    // Edit Submenu
    let edit_menu = SubmenuBuilder::with_id(app_handle, "edit_menu", "&Edit")
        .item(
            &MenuItemBuilder::with_id("copy_screenshot", "Copy Screenshot")
                .accelerator("Shift+CmdOrCtrl+C")
                .build(app_handle)?,
        )
        .item(&MenuItemBuilder::with_id("copy_data", "Copy Data (CSV)").build(app_handle)?)
        .separator()
        .item(&MenuItemBuilder::with_id("clear_session", "Clear Session").build(app_handle)?)
        .build()?;

    // Device Submenu
    let device_menu = SubmenuBuilder::with_id(app_handle, "device_menu", "&Device")
        .item(
            &MenuItemBuilder::with_id("toggle_logging", "Start Logging")
                .accelerator("CmdOrCtrl+Space")
                .build(app_handle)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id("connect_device", "Connect Device...").build(app_handle)?)
        .item(&MenuItemBuilder::with_id("rpc_settings", "RPC Settings...").build(app_handle)?)
        .build()?;

    // Assemble the final menu
    let menu = MenuBuilder::new(app_handle)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&device_menu)
        .build()?;

    Ok(menu)
}
