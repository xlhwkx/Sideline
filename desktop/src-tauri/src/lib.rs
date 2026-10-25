//! 托盘程序。没有主窗口，关闭配对窗口不退出。

mod pair;

use std::sync::Arc;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Manager, RunEvent};

use sidetype_server::{serve, DeviceStore, Server};

/// 托盘、服务和配对窗口共用的状态。
struct AppState {
    server: Arc<Server>,
}

/// 启动托盘程序。
pub fn run() {
    // 管理员窗口会把普通进程的按键和窗口操作直接丢掉。不是管理员就重新申请一次。
    if !running_elevated() {
        if relaunch_elevated() {
            std::process::exit(0);
        }
        tracing::warn!("没有拿到管理员权限，管理员窗口将无法切换和输入");
    }

    // 托盘程序没有控制台，日志写到文件，方便看每次窗口操作的耗时。
    let log_dir = std::env::var("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join("Sideline");
    let _ = std::fs::create_dir_all(&log_dir);
    let writer = tracing_appender::rolling::never(&log_dir, "sideline.log");
    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(writer)
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("pair") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            let store = DeviceStore::load(device_path(app)?);
            let server = Server::new(store, computer_name());
            // 调试构建启动时就发一个配对码，方便没有界面地点对点验证。
            #[cfg(debug_assertions)]
            server.issue_code();
            start_server(Arc::clone(&server));
            app.manage(AppState { server });
            build_tray(app.handle())?;
            // 托盘图标的消息窗口是个 14 像素的透明窗口，在这台机器上会显示在屏幕左上角。
            // 它在事件循环启动后才被显示，所以晚一点再藏。
            std::thread::spawn(|| {
                // 窗口在创建之后才被设成可见，等它出现再藏。
                std::thread::sleep(std::time::Duration::from_millis(300));
                hide_tray_message_window();
            });
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("启动失败")
        .run(|app, event| {
            if let RunEvent::ExitRequested { api, code, .. } = event {
                // 关闭配对窗口时 Tauri 会请求退出。只有菜单里的退出才真的退出。
                if code.is_none() {
                    api.prevent_exit();
                    let _ = app;
                }
            }
        });
}

fn start_server(server: Arc<Server>) {
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::error!(%error, "runtime");
                return;
            }
        };
        let (_tx, rx) = tokio::sync::watch::channel(false);
        runtime.block_on(async move {
            if let Err(error) = serve(server, rx).await {
                tracing::error!(%error, "server stopped");
            }
        });
    });
}

fn build_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let pair = MenuItem::with_id(app, "pair", "配对", true, None::<&str>)?;
    let autostart = MenuItem::with_id(app, "autostart", "开机自启", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&pair, &autostart, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder
        .tooltip("Sideline")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "pair" => pair::open(app),
            "autostart" => toggle_autostart(app),
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
                pair::open(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn toggle_autostart(app: &tauri::AppHandle) {
    use tauri_plugin_autostart::ManagerExt;
    let manager = app.autolaunch();
    let enabled = manager.is_enabled().unwrap_or(false);
    let result = if enabled {
        manager.disable()
    } else {
        manager.enable()
    };
    if let Err(error) = result {
        tracing::info!(%error, "autostart");
    }
}

fn device_path(app: &tauri::App) -> tauri::Result<std::path::PathBuf> {
    let dir = app.path().app_config_dir()?;
    Ok(dir.join("devices.json"))
}

fn computer_name() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "SideType".into())
}

/// 藏起托盘图标用来接收消息的那个小窗口。它不属于任何 WebView 窗口。
fn hide_tray_message_window() {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowExW, GetWindowLongPtrW, GetWindowThreadProcessId, SetWindowLongPtrW, ShowWindow,
        GWL_STYLE, SW_HIDE,
    };

    // 单实例插件的消息窗口。它靠 WM_COPYDATA 收第二个实例的参数，不需要可见，
    // 但插件把窗口样式设成了 WS_VISIBLE，于是左上角多出一个小窗口。
    for class_name in ["app.sidetype.desktop-sic", "Tao Thread Event Target"] {
        hide_class(class_name);
    }

    fn hide_class(class_name: &str) {
    let class_name: Vec<u16> = class_name.encode_utf16().chain(Some(0)).collect();
    let mut window = HWND::default();
    loop {
        let found = unsafe {
            FindWindowExW(
                None,
                Some(window),
                windows::core::PCWSTR(class_name.as_ptr()),
                None,
            )
        };
        window = match found {
            Ok(found) => found,
            Err(_) => break,
        };
        if window.0.is_null() {
            break;
        }
        let mut owner = 0u32;
        unsafe { GetWindowThreadProcessId(window, Some(&mut owner)) };
        if owner == std::process::id() {
            unsafe {
                // 去掉 WS_VISIBLE。只调 ShowWindow 会被上面设好的样式盖回去。
                let style = GetWindowLongPtrW(window, GWL_STYLE);
                let _ = SetWindowLongPtrW(window, GWL_STYLE, style & !0x1000_0000);
                let _ = ShowWindow(window, SW_HIDE);
            }
        }
    }
    }
}

/// 当前进程是否已经以管理员运行。
fn running_elevated() -> bool {
    use windows::Win32::UI::Shell::IsUserAnAdmin;
    unsafe { IsUserAnAdmin().as_bool() }
}

/// 用同一份程序重新启动，并弹出管理员确认。成功启动后调用方应该退出。
fn relaunch_elevated() -> bool {
    use std::os::windows::process::CommandExt;

    let Ok(exe) = std::env::current_exe() else {
        return false;
    };
    // CREATE_NO_WINDOW：提权后的程序自己没有控制台。
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = format!(
        "Start-Process -FilePath '{}' -Verb RunAs",
        exe.display().to_string().replace('\'', "''")
    );
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .is_ok()
}
