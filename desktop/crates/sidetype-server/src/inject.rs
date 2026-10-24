//! 把协议里的注入请求交给 [`sidetype_inject`]，并把错误映射成错误码。
//!
//! 注入是同步的系统调用，放在 `spawn_blocking` 里，避免卡住 WebSocket 的心跳。

use sidetype_inject::{
    activate_window, inject_key, inject_text, list_switchable_windows, maximize_window,
    minimize_window, InjectError, Key, Mode as InjectMode,
};
use sidetype_protocol::{
    ack_err, ack_ok, ErrorCode, KeyName, Mode, Response, ServerMessage, WindowInfo,
};

/// 注入的入口。真正的互斥在 `sidetype_inject` 里。
pub struct Injector;

impl Injector {
    /// 新建。没有自己的状态。
    pub fn new() -> Self {
        Self
    }

    /// 注入一段正文。日志只记长度和结果，不记内容。
    pub async fn inject_text(&self, id: &str, text: &str, mode: Mode, enter: bool) -> Response {
        let bytes = text.len();
        let owned = text.to_owned();
        let id_owned = id.to_owned();
        let started = std::time::Instant::now();
        let result =
            tokio::task::spawn_blocking(move || inject_text(&owned, to_mode(mode), enter)).await;
        let result = match result {
            Ok(inner) => inner,
            Err(_) => Err(InjectError::SendInput),
        };
        log_result(bytes, started, &result);
        to_response(&id_owned, result)
    }

    /// 当前可以切换的窗口。
    pub async fn windows(&self) -> Response {
        let windows = tokio::task::spawn_blocking(list_switchable_windows)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|window| WindowInfo {
                id: window.id.to_string(),
                title: window.title,
                topmost: window.topmost,
                minimized: window.minimized,
                maximized: window.maximized,
                icon: window.icon,
            })
            .collect();
        Response::new(ServerMessage::Windows { windows })
    }

    /// 把一个窗口拉到前台。编号对不上就回失败。
    pub async fn focus(&self, id: &str) -> Response {
        let parsed = id.parse::<isize>().ok();
        let id_owned = id.to_owned();
        let switched = match parsed {
            Some(value) => tokio::task::spawn_blocking(move || {
                // 列表按句柄排序，第一项不是前台窗口。再点一次最小化只对真正在前台的那个成立。
                if sidetype_inject::foreground_id() == Some(value) {
                    minimize_window(value)
                } else {
                    activate_window(value)
                }
            })
            .await
            .unwrap_or(false),
            None => false,
        };
        if switched {
            ack_ok(id_owned)
        } else {
            ack_err(id_owned, ErrorCode::InjectFailed)
        }
    }

    /// 最小化一个窗口。
    pub async fn minimize(&self, id: &str) -> Response {
        window_action(id, minimize_window).await
    }

    /// 最大化一个窗口。已经最大化时还原。
    pub async fn maximize(&self, id: &str) -> Response {
        window_action(id, maximize_window).await
    }

    /// 发送一个单独按键。
    pub async fn inject_key(&self, id: &str, key: KeyName) -> Response {
        let id_owned = id.to_owned();
        let result = tokio::task::spawn_blocking(move || inject_key(to_key(key))).await;
        let result = match result {
            Ok(inner) => inner,
            Err(_) => Err(InjectError::SendInput),
        };
        to_response(&id_owned, result)
    }
}

async fn window_action(id: &str, action: fn(isize) -> bool) -> Response {
    let id_owned = id.to_owned();
    let done = match id.parse::<isize>() {
        Ok(value) => tokio::task::spawn_blocking(move || action(value))
            .await
            .unwrap_or(false),
        Err(_) => false,
    };
    if done {
        ack_ok(id_owned)
    } else {
        ack_err(id_owned, ErrorCode::InjectFailed)
    }
}

impl Default for Injector {
    fn default() -> Self {
        Self::new()
    }
}

fn to_mode(mode: Mode) -> InjectMode {
    match mode {
        Mode::Paste => InjectMode::Paste,
        Mode::Type => InjectMode::Type,
    }
}

fn to_key(key: KeyName) -> Key {
    match key {
        KeyName::Enter => Key::Enter,
        KeyName::Backspace => Key::Backspace,
        KeyName::Escape => Key::Escape,
    }
}

fn to_response(id: &str, result: Result<(), InjectError>) -> Response {
    match result {
        Ok(()) => ack_ok(id),
        Err(error) => ack_err(id, map_error(error)),
    }
}

fn map_error(error: InjectError) -> ErrorCode {
    match error {
        InjectError::Busy => ErrorCode::Busy,
        InjectError::Integrity => ErrorCode::Integrity,
        InjectError::NoForeground => ErrorCode::InjectFailed,
        InjectError::ClipboardOpen => ErrorCode::InjectFailed,
        InjectError::ClipboardRace => ErrorCode::ClipboardRace,
        InjectError::SendInput => ErrorCode::InjectFailed,
        InjectError::Empty => ErrorCode::Empty,
    }
}

fn log_result(bytes: usize, started: std::time::Instant, result: &Result<(), InjectError>) {
    match result {
        Ok(()) => tracing::info!(
            bytes,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "injected"
        ),
        Err(error) => tracing::info!(bytes, error = %error, "inject rejected"),
    }
}
