//! 一条连接所处的阶段。

/// 连接还没有配对，或者已经是当前唯一的已鉴权连接。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// 还不能发送文字。
    Awaiting,
    /// 已鉴权。数值是这条连接的编号，用来判断自己是不是最新的一条。
    Authed(u64),
}
