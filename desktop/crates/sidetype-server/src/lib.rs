//! WebSocket 服务。负责配对、token 和把协议消息交给注入层。
//!
//! 不依赖 Tauri。托盘程序和命令行都通过 [`serve`] 启动它。

mod inject;
mod pairing;
mod session;
mod store;

use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use sidetype_protocol::{
    ack_err, ack_ok, ClientMessage, ErrorCode, ProtocolError, Request, Response, ServerMessage,
    HEARTBEAT_INTERVAL, HEARTBEAT_TIMEOUT, MAX_PAIR_ATTEMPTS,
};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex};
use tokio_tungstenite::tungstenite::Message;

pub use pairing::{Pairing, PAIR_PORT};
pub use store::DeviceStore;

use inject::Injector;
use session::ConnectionState;

/// 服务的共享状态。
pub struct Server {
    pairing: Pairing,
    devices: DeviceStore,
    injector: Injector,
    /// 后连上的连接通过它通知前一条连接退出。
    kick: broadcast::Sender<u64>,
    /// 当前已鉴权连接的编号。0 表示没有。
    current: Mutex<u64>,
    next_id: std::sync::atomic::AtomicU64,
    computer_name: String,
}

impl Server {
    /// 用一份设备存储和电脑名创建服务。
    pub fn new(devices: DeviceStore, computer_name: String) -> Arc<Self> {
        let (kick, _) = broadcast::channel(16);
        Arc::new(Self {
            pairing: Pairing::new(),
            devices,
            injector: Injector::new(),
            kick,
            current: Mutex::new(0),
            next_id: std::sync::atomic::AtomicU64::new(1),
            computer_name,
        })
    }

    /// 生成一个新的配对码。旧的立即作废。
    pub fn issue_code(&self) -> String {
        self.pairing.issue()
    }

    /// 当前有效的配对码。窗口关闭或过期后是 `None`。
    pub fn current_code(&self) -> Option<String> {
        self.pairing.current()
    }

    /// 关闭配对窗口时调用，立即作废配对码。
    pub fn clear_code(&self) {
        self.pairing.clear();
    }

    /// 撤销一台设备。已连接的话会收到 `kick` 然后断开。
    pub fn revoke(&self, token_hash: &str) -> bool {
        let removed = self.devices.revoke(token_hash);
        if removed {
            let _ = self.kick.send(0);
        }
        removed
    }

    /// 已配对设备的 token 哈希列表。
    pub fn devices(&self) -> Vec<String> {
        self.devices.list()
    }
}

/// 只监听本机的调试端口，返回当前配对码。发布构建里不启用。
#[cfg(debug_assertions)]
const DEBUG_PORT: u16 = 17332;

/// 监听 `0.0.0.0:17331` 直到 `shutdown` 关闭。
pub async fn serve(
    server: Arc<Server>,
    shutdown: tokio::sync::watch::Receiver<bool>,
) -> std::io::Result<()> {
    #[cfg(debug_assertions)]
    {
        let server = Arc::clone(&server);
        tokio::spawn(async move {
            let _ = debug_code(server).await;
        });
    }
    let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], PAIR_PORT))).await?;
    tracing::info!(port = PAIR_PORT, "listening");
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, peer) = accepted?;
                let server = Arc::clone(&server);
                tokio::spawn(async move {
                    if let Err(error) = handle(server, stream).await {
                        tracing::debug!(%peer, %error, "connection ended");
                    }
                });
            }
            _ = shutdown_fired(shutdown.clone()) => break,
        }
    }
    Ok(())
}

/// 调试端口只绑定 127.0.0.1，回一行配对码，不带 token。
#[cfg(debug_assertions)]
async fn debug_code(server: Arc<Server>) -> std::io::Result<()> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], DEBUG_PORT))).await?;
    loop {
        let (mut stream, _) = listener.accept().await?;
        let code = server.current_code().unwrap_or_else(|| server.issue_code());
        let _ = tokio::io::AsyncWriteExt::write_all(&mut stream, code.as_bytes()).await;
    }
}

async fn shutdown_fired(mut shutdown: tokio::sync::watch::Receiver<bool>) {
    while !*shutdown.borrow() {
        if shutdown.changed().await.is_err() {
            return;
        }
    }
}

async fn handle(server: Arc<Server>, stream: TcpStream) -> Result<(), ConnectionError> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    let (mut write, mut read) = ws.split();
    let mut state = ConnectionState::Awaiting;
    let mut failures = 0u32;
    let mut kick = server.kick.subscribe();
    let mut last_seen = tokio::time::Instant::now();

    loop {
        // 任何入站帧都算活着，包括协议层的 ping。只认文字帧的话，手机连上不打字，
        // 15 秒后会被误判超时并断开。
        let incoming = tokio::select! {
            biased;
            frame = read.next() => frame,
            _ = tokio::time::sleep(HEARTBEAT_INTERVAL) => None,
            kicked = kick.recv() => {
                let own = match state { ConnectionState::Authed(id) => id, _ => 0 };
                let kicked = kicked.unwrap_or(0);
                if kicked != own {
                    if matches!(state, ConnectionState::Authed(_)) {
                        let notice = Response::new(ServerMessage::Kick { reason: "replaced".into() });
                        let _ = write.send(Message::Text(notice.to_frame().into())).await;
                    }
                    break;
                }
                continue;
            }
        };
        let Some(incoming) = incoming else {
            if last_seen.elapsed() > HEARTBEAT_TIMEOUT {
                break;
            }
            let ping = Response::new(ServerMessage::Ping { t: 0 });
            write.send(Message::Text(ping.to_frame().into())).await?;
            continue;
        };
        last_seen = tokio::time::Instant::now();
        let frame = match incoming {
            Ok(Message::Text(text)) => text.to_string(),
            Ok(Message::Ping(payload)) => {
                write.send(Message::Pong(payload)).await?;
                continue;
            }
            Ok(Message::Pong(_)) => continue,
            _ => break,
        };

        let response = match Request::from_frame(&frame) {
            Ok(request) => dispatch(&server, &mut state, request.message).await,
            Err(ProtocolError::TooLarge) => Some(Response::new(ServerMessage::Error {
                error: ErrorCode::TooLarge,
            })),
            Err(_) => Some(Response::new(ServerMessage::Error {
                error: ErrorCode::Unsupported,
            })),
        };
        let Some(response) = response else { continue };
        if matches!(
            response.message,
            ServerMessage::Error {
                error: ErrorCode::BadCode
            }
        ) {
            failures += 1;
        }
        write
            .send(Message::Text(response.to_frame().into()))
            .await?;
        if failures >= MAX_PAIR_ATTEMPTS {
            break;
        }
    }
    if let ConnectionState::Authed(id) = state {
        let mut current = server.current.lock().await;
        if *current == id {
            *current = 0;
        }
    }
    Ok(())
}

async fn dispatch(
    server: &Server,
    state: &mut ConnectionState,
    message: ClientMessage,
) -> Option<Response> {
    match message {
        ClientMessage::Pair { code } => Some(pair(server, state, &code).await),
        ClientMessage::Auth { token } => Some(auth(server, state, &token).await),
        ClientMessage::Ping { t } => Some(Response::new(ServerMessage::Pong { t })),
        ClientMessage::Pong { .. } => None,
        ClientMessage::Windows => {
            if !matches!(state, ConnectionState::Authed(_)) {
                return Some(Response::new(ServerMessage::Error {
                    error: ErrorCode::Unauthorized,
                }));
            }
            Some(server.injector.windows().await)
        }
        ClientMessage::Minimize { id } => {
            tracing::info!(id = %id, "minimize");
            Some(server.injector.minimize(&id).await)
        }
        ClientMessage::Maximize { id } => {
            tracing::info!(id = %id, "maximize");
            Some(server.injector.maximize(&id).await)
        }
        ClientMessage::Focus { id } => {
            if !matches!(state, ConnectionState::Authed(_)) {
                return Some(Response::new(ServerMessage::Error {
                    error: ErrorCode::Unauthorized,
                }));
            }
            Some(server.injector.focus(&id).await)
        }
        ClientMessage::Text {
            id,
            text,
            mode,
            enter,
        } => {
            if !matches!(state, ConnectionState::Authed(_)) {
                return Some(Response::new(ServerMessage::Error {
                    error: ErrorCode::Unauthorized,
                }));
            }
            Some(server.injector.inject_text(&id, &text, mode, enter).await)
        }
        ClientMessage::Key { id, key } => {
            if !matches!(state, ConnectionState::Authed(_)) {
                return Some(Response::new(ServerMessage::Error {
                    error: ErrorCode::Unauthorized,
                }));
            }
            Some(server.injector.inject_key(&id, key).await)
        }
    }
}

async fn pair(server: &Server, state: &mut ConnectionState, code: &str) -> Response {
    if !server.pairing.check(code) {
        return Response::new(ServerMessage::Error {
            error: ErrorCode::BadCode,
        });
    }
    let (token, hash) = store::new_token();
    server.devices.insert(hash);
    become_current(server, state).await;
    tracing::info!(bytes = token.len(), "paired");
    Response::new(ServerMessage::PairOk { token })
}

async fn auth(server: &Server, state: &mut ConnectionState, token: &str) -> Response {
    let hash = store::hash_token(token);
    if !server.devices.contains(&hash) {
        return Response::new(ServerMessage::Error {
            error: ErrorCode::BadToken,
        });
    }
    become_current(server, state).await;
    Response::new(ServerMessage::AuthOk {
        name: server.computer_name.clone(),
    })
}

/// 把这条连接标记为当前唯一的已鉴权连接，并踢掉之前的。
async fn become_current(server: &Server, state: &mut ConnectionState) {
    let id = server
        .next_id
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    *server.current.lock().await = id;
    *state = ConnectionState::Authed(id);
    let _ = server.kick.send(id);
}

#[derive(Debug, thiserror::Error)]
enum ConnectionError {
    #[error("websocket: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
}

/// 测试和命令行用的构造：一份只活在内存里的服务。
pub fn memory_server(name: &str) -> Arc<Server> {
    Server::new(DeviceStore::memory(), name.to_owned())
}

/// 给测试用的直接回应，不经过网络。
pub async fn handle_frame(
    server: &Server,
    state: &mut ConnectionState,
    frame: &str,
) -> Option<Response> {
    match Request::from_frame(frame) {
        Ok(request) => dispatch(server, state, request.message).await,
        Err(ProtocolError::TooLarge) => Some(ack_err(String::new(), ErrorCode::TooLarge)),
        Err(_) => Some(ack_err(String::new(), ErrorCode::Unsupported)),
    }
}

/// 避免未使用告警：成功 ack 由注入层使用。
#[allow(dead_code)]
fn _ack_ok_used() -> Response {
    ack_ok(String::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn wrong_code_is_rejected_and_right_code_issues_token() {
        let server = memory_server("test");
        let code = server.issue_code();
        let mut state = ConnectionState::Awaiting;

        let bad = handle_frame(
            &server,
            &mut state,
            r#"{"v":1,"type":"pair","code":"000000"}"#,
        )
        .await
        .unwrap();
        assert!(matches!(
            bad.message,
            ServerMessage::Error {
                error: ErrorCode::BadCode
            }
        ));
        assert!(matches!(state, ConnectionState::Awaiting));

        let frame = format!(r#"{{"v":1,"type":"pair","code":"{code}"}}"#);
        let ok = handle_frame(&server, &mut state, &frame).await.unwrap();
        let token = match ok.message {
            ServerMessage::PairOk { token } => token,
            other => panic!("{other:?}"),
        };
        assert!(matches!(state, ConnectionState::Authed(_)));

        // 重启等于一份新的连接状态，但设备还在。
        let mut again = ConnectionState::Awaiting;
        let frame = format!(r#"{{"v":1,"type":"auth","token":"{token}"}}"#);
        let authed = handle_frame(&server, &mut again, &frame).await.unwrap();
        assert!(matches!(authed.message, ServerMessage::AuthOk { .. }));
    }

    #[tokio::test]
    async fn five_failures_are_counted_by_the_caller_contract() {
        let server = memory_server("test");
        server.issue_code();
        let mut state = ConnectionState::Awaiting;
        for _ in 0..5 {
            let response = handle_frame(
                &server,
                &mut state,
                r#"{"v":1,"type":"pair","code":"999999"}"#,
            )
            .await
            .unwrap();
            assert!(matches!(
                response.message,
                ServerMessage::Error {
                    error: ErrorCode::BadCode
                }
            ));
        }
    }

    #[tokio::test]
    async fn text_before_auth_is_unauthorized() {
        let server = memory_server("test");
        let mut state = ConnectionState::Awaiting;
        let frame = r#"{"v":1,"type":"text","id":"x","text":"a","mode":"type"}"#;
        let response = handle_frame(&server, &mut state, frame).await.unwrap();
        assert!(matches!(
            response.message,
            ServerMessage::Error {
                error: ErrorCode::Unauthorized
            }
        ));
    }

    #[tokio::test]
    async fn later_connection_replaces_the_earlier_one() {
        let server = memory_server("test");
        let code = server.issue_code();
        let mut first = ConnectionState::Awaiting;
        let frame = format!(r#"{{"v":1,"type":"pair","code":"{code}"}}"#);
        handle_frame(&server, &mut first, &frame).await.unwrap();
        let first_id = match first {
            ConnectionState::Authed(id) => id,
            _ => panic!("not authed"),
        };

        let code = server.issue_code();
        let mut second = ConnectionState::Awaiting;
        let frame = format!(r#"{{"v":1,"type":"pair","code":"{code}"}}"#);
        handle_frame(&server, &mut second, &frame).await.unwrap();
        let current = *server.current.lock().await;
        let second_id = match second {
            ConnectionState::Authed(id) => id,
            _ => panic!("not authed"),
        };
        assert_eq!(current, second_id);
        assert_ne!(first_id, second_id);
    }
}
