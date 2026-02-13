use tauri::{
    menu::{
        AboutMetadataBuilder, CheckMenuItemBuilder, Menu, MenuBuilder, MenuItemBuilder,
        MenuItemKind, PredefinedMenuItem, SubmenuBuilder,
    },
    AppHandle, Emitter, Manager, Runtime,
};
use tauri_plugin_dialog::DialogExt;

const THEME_SYSTEM_ID: &str = "theme_mode_system";
const THEME_LIGHT_ID: &str = "theme_mode_light";
const THEME_DARK_ID: &str = "theme_mode_dark";

pub fn create_app_menu<R: Runtime>(app_handle: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let about_metadata = AboutMetadataBuilder::new()
        .name(Some("Trendline"))
        .version(Some("1.0.2"))
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

    let theme_menu = SubmenuBuilder::with_id(app_handle, "theme_menu", "Theme")
        .item(
            &CheckMenuItemBuilder::with_id(THEME_SYSTEM_ID, "System")
                .checked(true)
                .build(app_handle)?,
        )
        .item(
            &CheckMenuItemBuilder::with_id(THEME_LIGHT_ID, "Light")
                .checked(false)
                .build(app_handle)?,
        )
        .item(
            &CheckMenuItemBuilder::with_id(THEME_DARK_ID, "Dark")
                .checked(false)
                .build(app_handle)?,
        )
        .build()?;

    let view_menu = SubmenuBuilder::with_id(app_handle, "view_menu", "&View")
        .item(&theme_menu)
        .build()?;

    // Assemble the final menu
    let menu = MenuBuilder::new(app_handle)
        .item(&app_menu)
        .item(&file_menu)
        .item(&edit_menu)
        .item(&view_menu)
        .item(&device_menu)
        .build()?;

    Ok(menu)
}

fn set_theme_menu_checks(app: &AppHandle, selected_id: &str) {
    let Some(menu) = app
        .menu()
        .or_else(|| app.get_webview_window("main").and_then(|w| w.menu()))
    else {
        return;
    };
    let Some(view_submenu) = menu
        .get("view_menu")
        .and_then(|item| item.as_submenu().cloned())
    else {
        return;
    };
    let Some(theme_submenu) = view_submenu
        .get("theme_menu")
        .and_then(|item| item.as_submenu().cloned())
    else {
        return;
    };

    for (id, checked) in [
        (THEME_SYSTEM_ID, selected_id == THEME_SYSTEM_ID),
        (THEME_LIGHT_ID, selected_id == THEME_LIGHT_ID),
        (THEME_DARK_ID, selected_id == THEME_DARK_ID),
    ] {
        if let Some(MenuItemKind::Check(item)) = theme_submenu.get(id) {
            let _ = item.set_checked(checked);
        }
    }
}

pub fn handle_menu_event(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        "open_recording" => {
            let Some(window) = app.get_webview_window("main") else {
                return;
            };
            app.dialog().file().pick_file(move |path_buf| {
                if let Some(path) = path_buf {
                    println!("File selected for opening: {}", path.to_string());
                    window.emit("file-opened", path.to_string()).unwrap();
                }
            });
        }
        "save_recording" => {
            // Using the new dialog plugin API
            app.dialog()
                .file()
                .add_filter("Trendline Recording", &["json"])
                .set_file_name("recording.json")
                .save_file(move |path_buf| {
                    if let Some(path) = path_buf {
                        println!("File selected for saving: {}", path.to_string());
                    }
                });
        }
        THEME_SYSTEM_ID => {
            set_theme_menu_checks(app, THEME_SYSTEM_ID);
            let _ = app.emit("theme-mode-selected", "system");
        }
        THEME_LIGHT_ID => {
            set_theme_menu_checks(app, THEME_LIGHT_ID);
            let _ = app.emit("theme-mode-selected", "light");
        }
        THEME_DARK_ID => {
            set_theme_menu_checks(app, THEME_DARK_ID);
            let _ = app.emit("theme-mode-selected", "dark");
        }
        // ... other handlers remain the same
        _ => {}
    }
}
