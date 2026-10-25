//! SideType 协议 v1。只有类型和序列化，没有网络，也没有系统调用。
//!
//! 字段以 `protocol/v1.md` 为准。`protocol/fixtures/` 里的每份夹具都有对应测试。

use serde::{Deserialize, Serialize};

/// 单条正文的字节上限。按 UTF-8 计。
pub const TEXT_LIMIT: usize = 100_000;

/// 配对码连续错误这么多次就断开。
pub const MAX_PAIR_ATTEMPTS: u32 = 5;

/// 心跳间隔。
pub const HEARTBEAT_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

/// 这么久没有收到 pong 就断开。
pub const HEARTBEAT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

/// 配对码的有效期。
pub const PAIR_CODE_TTL: std::time::Duration = std::time::Duration::from_secs(60);

/// 协议版本。每条消息都带。
pub const VERSION: u32 = 1;

/// 手机发给电脑的消息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// 用配对码换 token。
    Pair { code: String },
    /// 用之前拿到的 token 重连。
    Auth { token: String },
    /// 把正文注入当前焦点窗口。
    Text {
        id: String,
        text: String,
        mode: Mode,
        #[serde(default)]
        enter: bool,
    },
    /// 发送一个不经过输入法的按键。
    Key { id: String, key: KeyName },
    /// 心跳。电脑回 `pong`。
    Ping { t: u64 },
    /// 对电脑心跳的回应。不回任何消息。
    Pong { t: u64 },
    /// 要一份当前可以切换的窗口。
    Windows,
    /// 把一个窗口拉到前台。`id` 来自最近一次 `windows` 的回应。
    Focus { id: String },
    /// 最小化一个窗口。
    Minimize { id: String },
    /// 最大化一个窗口。已经最大化时还原。
    Maximize { id: String },
}

/// 电脑发给手机的消息。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    /// 配对成功。`token` 是 32 字节随机数的十六进制。
    PairOk { token: String },
    /// 重连成功。`name` 是电脑名。
    AuthOk { name: String },
    /// 对一条 `text` 或 `key` 的回应。
    Ack {
        id: String,
        ok: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<ErrorCode>,
    },
    /// 电脑主动发出的心跳。手机回 `pong`。
    Ping { t: u64 },
    /// 心跳回应。
    Pong { t: u64 },
    /// 设备被撤销，连接随后关闭。
    Kick { reason: String },
    /// 握手阶段的失败。
    Error { error: ErrorCode },
    /// 当前可以切换的窗口。`id` 只在电脑这次开机里有效。
    Windows { windows: Vec<WindowInfo> },
}

/// 一个可以切换过去的窗口。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowInfo {
    /// 句柄转成的十进制字符串。
    pub id: String,
    /// 标题栏上的文字。
    pub title: String,
    /// 是否置顶。
    #[serde(default)]
    pub topmost: bool,
    /// 是否最小化到任务栏。
    #[serde(default)]
    pub minimized: bool,
    /// 是否最大化。
    #[serde(default)]
    pub maximized: bool,
    /// 程序图标的 PNG，Base64。取不到时为空。
    #[serde(default)]
    pub icon: String,
}

/// 正文的注入方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// 写入剪贴板再发送 Ctrl+V，之后还原剪贴板。
    Paste,
    /// 逐个码元发送 Unicode 按键。
    Type,
}

/// 可以单独发送的按键。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyName {
    Enter,
    Backspace,
    Escape,
    /// 复制，Ctrl+C。
    Copy,
    /// 粘贴，Ctrl+V。
    Paste,
    /// 剪切，Ctrl+X。
    Cut,
    /// 全选，Ctrl+A。
    SelectAll,
    /// 撤销，Ctrl+Z。
    Undo,
    /// 保存，Ctrl+S。
    Save,
    /// 重做，Ctrl+Y。
    Redo,
}

/// 协议里的全部错误码。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    BadCode,
    BadToken,
    Unauthorized,
    Busy,
    Integrity,
    Empty,
    TooLarge,
    InjectFailed,
    ClipboardRace,
    Unsupported,
}

/// 一条带版本号的请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// 协议版本，必须是 [`VERSION`]。
    pub v: u32,
    /// 消息体。序列化时和 `v` 平铺在同一层。
    #[serde(flatten)]
    pub message: ClientMessage,
}

/// 一条带版本号的回应。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    /// 协议版本。
    pub v: u32,
    /// 消息体。
    #[serde(flatten)]
    pub message: ServerMessage,
}

/// 解析或构造消息时的错误。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProtocolError {
    /// JSON 无法解析。
    #[error("malformed")]
    Malformed,
    /// `v` 不是本端支持的版本。
    #[error("unsupported version")]
    BadVersion,
    /// 正文超过 [`TEXT_LIMIT`]。
    #[error("text exceeds {TEXT_LIMIT} bytes")]
    TooLarge,
}

impl Request {
    /// 从一帧文本解析请求。版本不对、JSON 损坏都返回错误。
    pub fn from_frame(frame: &str) -> Result<Self, ProtocolError> {
        let request: Self = serde_json::from_str(frame).map_err(|_| ProtocolError::Malformed)?;
        if request.v != VERSION {
            return Err(ProtocolError::BadVersion);
        }
        if let ClientMessage::Text { text, .. } = &request.message {
            if text.len() > TEXT_LIMIT {
                return Err(ProtocolError::TooLarge);
            }
        }
        Ok(request)
    }

    /// 序列化成一帧，不带换行。
    pub fn to_frame(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| unreachable_frame())
    }
}

impl Response {
    /// 构造一条当前版本的回应。
    pub fn new(message: ServerMessage) -> Self {
        Self {
            v: VERSION,
            message,
        }
    }

    /// 从一帧文本解析回应。
    pub fn from_frame(frame: &str) -> Result<Self, ProtocolError> {
        let response: Self = serde_json::from_str(frame).map_err(|_| ProtocolError::Malformed)?;
        if response.v != VERSION {
            return Err(ProtocolError::BadVersion);
        }
        Ok(response)
    }

    /// 序列化成一帧，不带换行。
    pub fn to_frame(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| unreachable_frame())
    }
}

/// `Response` 的每个字段都是可序列化的，这里只是为了不在库里使用 expect。
fn unreachable_frame() -> String {
    String::from(r#"{"v":1,"type":"error","error":"inject_failed"}"#)
}

/// 成功的 ack。
pub fn ack_ok(id: impl Into<String>) -> Response {
    Response::new(ServerMessage::Ack {
        id: id.into(),
        ok: true,
        error: None,
    })
}

/// 失败的 ack。
pub fn ack_err(id: impl Into<String>, error: ErrorCode) -> Response {
    Response::new(ServerMessage::Ack {
        id: id.into(),
        ok: false,
        error: Some(error),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let path = format!("../../../protocol/fixtures/{name}.json");
        std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("fixture {name}: {err}"))
    }

    #[test]
    fn fixtures_round_trip() {
        let pair = Request::from_frame(&fixture("pair")).unwrap();
        assert_eq!(
            pair.message,
            ClientMessage::Pair {
                code: "048213".into()
            }
        );

        let text = Request::from_frame(&fixture("text")).unwrap();
        match text.message {
            ClientMessage::Text {
                mode, enter, text, ..
            } => {
                assert_eq!(mode, Mode::Paste);
                assert!(!enter);
                assert_eq!(text, "你好");
            }
            other => panic!("expected text, got {other:?}"),
        }

        let key = Request::from_frame(&fixture("key")).unwrap();
        assert!(matches!(
            key.message,
            ClientMessage::Key {
                key: KeyName::Enter,
                ..
            }
        ));

        let auth = Request::from_frame(&fixture("auth")).unwrap();
        assert!(matches!(auth.message, ClientMessage::Auth { .. }));

        let ping = Request::from_frame(&fixture("ping")).unwrap();
        assert_eq!(
            ping.message,
            ClientMessage::Ping {
                t: 1_761_321_600_000
            }
        );

        for name in ["pair_ok", "auth_ok", "ack", "pong", "kick", "error"] {
            let response = Response::from_frame(&fixture(name)).unwrap();
            let again = Response::from_frame(&response.to_frame()).unwrap();
            assert_eq!(response, again, "{name}");
        }
    }

    #[test]
    fn ack_fixture_carries_integrity() {
        let response = Response::from_frame(&fixture("ack")).unwrap();
        assert_eq!(
            response.message,
            ServerMessage::Ack {
                id: "6f1d7c3e-8a20-4c1d-9b44-2d0a0c5e7f10".into(),
                ok: false,
                error: Some(ErrorCode::Integrity),
            }
        );
    }

    #[test]
    fn every_error_code_serializes() {
        let codes = [
            ErrorCode::BadCode,
            ErrorCode::BadToken,
            ErrorCode::Unauthorized,
            ErrorCode::Busy,
            ErrorCode::Integrity,
            ErrorCode::Empty,
            ErrorCode::TooLarge,
            ErrorCode::InjectFailed,
            ErrorCode::ClipboardRace,
            ErrorCode::Unsupported,
        ];
        for code in codes {
            let frame = Response::new(ServerMessage::Error { error: code }).to_frame();
            let back = Response::from_frame(&frame).unwrap();
            assert_eq!(back.message, ServerMessage::Error { error: code });
        }
    }

    #[test]
    fn rejects_bad_version_and_oversize() {
        assert_eq!(
            Request::from_frame(r#"{"v":2,"type":"ping","t":1}"#),
            Err(ProtocolError::BadVersion)
        );
        assert_eq!(
            Request::from_frame("not json"),
            Err(ProtocolError::Malformed)
        );
        let huge = "字".repeat(TEXT_LIMIT);
        let frame = format!(r#"{{"v":1,"type":"text","id":"x","text":"{huge}","mode":"type"}}"#);
        assert_eq!(Request::from_frame(&frame), Err(ProtocolError::TooLarge));
    }

    #[test]
    fn enter_defaults_to_false() {
        let frame = r#"{"v":1,"type":"text","id":"x","text":"a","mode":"type"}"#;
        let request = Request::from_frame(frame).unwrap();
        assert!(matches!(
            request.message,
            ClientMessage::Text { enter: false, .. }
        ));
    }
}
