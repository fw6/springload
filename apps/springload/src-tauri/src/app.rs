//! 应用装配：插件注册、菜单栏托盘、窗口生命周期与命令注册。

use crate::commands;
use crate::state::AppState;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

/// 构建并运行应用。
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::load().expect("初始化数据目录与数据库");
    tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(state)
        .on_window_event(|window, event| {
            // 关闭窗口不退出进程：拦下关闭、把窗口藏起来，进程留在菜单栏。
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            setup_tray(app.handle())?;
            sync_autostart(app.handle());
            commands::market::start_scheduler(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::glossary,
            commands::app::settings_get,
            commands::app::settings_update,
            commands::market::sync_market,
            commands::market::sync_status,
            commands::market::stock_list,
            commands::market::bars_get,
        ])
        .build(tauri::generate_context!())
        .expect("构建 Tauri 应用")
        .run(|_app, event| {
            // 因窗口全部关闭而退出时拦下：托盘常驻，进程继续。
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}

/// 建菜单栏图标：左键点图标显示主窗口，菜单里可以显示主窗口或退出。
fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Springload", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))?;

    TrayIconBuilder::with_id("springload-tray")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("蓄势 Springload")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// 显示并聚焦主窗口。
fn show_main(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 按配置里的 `autostart` 把登录自启打开或关掉。
fn sync_autostart(app: &tauri::AppHandle) {
    let settings = match app.state::<AppState>().settings_get() {
        Ok(settings) => settings,
        Err(error) => {
            eprintln!("读取配置失败，跳过登录自启同步: {error}");
            return;
        }
    };
    let autolaunch = app.autolaunch();
    let enabled = autolaunch.is_enabled().unwrap_or(false);
    let outcome = if settings.autostart && !enabled {
        autolaunch.enable()
    } else if !settings.autostart && enabled {
        autolaunch.disable()
    } else {
        return;
    };
    if let Err(error) = outcome {
        eprintln!("同步登录自启失败: {error}");
    }
}
