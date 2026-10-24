//! 配对窗口。唯一的界面，关掉不退出程序。

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::AppState;

const WINDOW: &str = "pair";

/// 打开或激活配对窗口，并刷新配对码。
pub fn open(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let code = state.server.issue_code();
    let address = local_address();
    let html = page(&code, &address);

    if let Some(window) = app.get_webview_window(WINDOW) {
        let script = format!(
            "document.body.innerHTML = {} ",
            js_string(&body(&code, &address))
        );
        let _ = window.eval(script);
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let blank: tauri::Url = "about:blank".parse().expect("about:blank 是合法地址");
    let built = WebviewWindowBuilder::new(app, WINDOW, WebviewUrl::External(blank))
        .title("Sideline")
        .inner_size(420.0, 520.0)
        .resizable(false)
        .center()
        .build();
    if let Ok(window) = built {
        let script = format!(
            "document.open();document.write({});document.close();",
            js_string(&html)
        );
        let _ = window.eval(script);
        let app = app.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                if let Some(state) = app.try_state::<AppState>() {
                    state.server.clear_code();
                }
            }
        });
    }
}

fn local_address() -> String {
    let ip = local_ip_address::local_ip()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".into());
    format!("{ip}:17331")
}

fn page(code: &str, address: &str) -> String {
    format!(
        r#"<!doctype html><html lang="zh"><head><meta charset="utf-8"><title>Sideline</title><style>{style}</style></head><body>{body}</body></html>"#,
        style = STYLE,
        body = body(code, address)
    )
}

fn body(code: &str, address: &str) -> String {
    let qr = qr_data_url(&format!(
        "sidetype://pair?host={}&port=17331&code={}&name={}",
        address.split(':').next().unwrap_or(""),
        code,
        computer()
    ));
    format!(
        r#"<main class="card"><p class="status">等待连接</p><img alt="配对二维码" src="{qr}"><p class="code">{code}</p><p class="address">{address}</p><button id="copy" class="primary">复制地址</button></main><script>document.getElementById('copy').onclick=()=>navigator.clipboard.writeText('{address}')</script>"#
    )
}

fn computer() -> String {
    std::env::var("COMPUTERNAME").unwrap_or_else(|_| "SideType".into())
}

fn qr_data_url(content: &str) -> String {
    use qrcode::QrCode;
    let Ok(code) = QrCode::new(content.as_bytes()) else {
        return String::new();
    };
    let image = code
        .render::<image::Luma<u8>>()
        .min_dimensions(280, 280)
        .build();
    let mut bytes = std::io::Cursor::new(Vec::new());
    if image.write_to(&mut bytes, image::ImageFormat::Png).is_err() {
        return String::new();
    }
    format!(
        "data:image/png;base64,{}",
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, bytes.get_ref())
    )
}

fn js_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "''".into())
}

const STYLE: &str = r#"
:root { --bg:#f4f4f6; --card:#fff; --fg:#1c1c21; --muted:#85858d; --accent:#4f46e5; --accent-fg:#fff; --line:#e3e3e8; --green:#189a58; --green-soft:#e4f6ec; --font:-apple-system,BlinkMacSystemFont,"Segoe UI","Microsoft YaHei UI",system-ui,sans-serif; --code:ui-monospace,Consolas,"Microsoft YaHei UI",monospace; }
@media (prefers-color-scheme: dark) { :root { --bg:#1a1a1e; --card:#232327; --fg:#ededf1; --muted:#8b8b94; --accent:#a5b4fc; --accent-fg:#14142a; --line:#333339; --green:#6fd99b; --green-soft:#1e3a2b; } }
* { box-sizing:border-box; }
body { margin:0; background:var(--bg); color:var(--fg); font:13px/1.4 var(--font); display:flex; min-height:100vh; align-items:center; justify-content:center; }
.card { background:var(--card); border:1px solid var(--line); border-radius:10px; padding:28px; text-align:center; width:320px; }
.status { display:inline-block; background:var(--green-soft); color:var(--green); border-radius:999px; padding:2px 10px; margin:0 0 16px; }
img { width:220px; height:220px; }
.code { font:600 28px/1 var(--code); font-variant-numeric:tabular-nums; letter-spacing:.18em; margin:16px 0 4px; }
.address { color:var(--muted); font-family:var(--code); margin:0 0 16px; }
.primary { background:var(--accent); color:var(--accent-fg); border:0; border-radius:8px; height:32px; padding:0 16px; font:inherit; cursor:pointer; }
"#;
