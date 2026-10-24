//! 把文字送进当前前台窗口。
//!
//! 没有网络，也没有界面。两种方式：[`Mode::Paste`] 走剪贴板和 Ctrl+V，
//! [`Mode::Type`] 逐个码元发送 Unicode 按键。整段操作由一把锁串行化。

mod clipboard;
mod input;
mod integrity;
mod windows;

pub use windows::{
    activate_window, foreground_id, list_switchable_windows, maximize_window, minimize_window,
    SwitchableWindow,
};

use std::sync::{Mutex, TryLockError};

use clipboard::ClipboardText;
pub use integrity::Integrity;

/// 全局注入锁。拿不到就说明上一发还没结束。
static INJECT_LOCK: Mutex<()> = Mutex::new(());

/// 一次粘贴后等待目标程序读取剪贴板的时间。
const PASTE_SETTLE: std::time::Duration = std::time::Duration::from_millis(40);

/// 一批 Unicode 按键最多包含的 UTF-16 码元数。
const TYPE_BATCH: usize = 50;

/// 正文怎么送进目标窗口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 写入剪贴板，发送 Ctrl+V，再按序号决定是否还原。
    Paste,
    /// 逐个码元发送 Unicode 按键，不碰剪贴板。
    Type,
}

/// 可以单独发送的按键。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Enter,
    Backspace,
    Escape,
}

/// 注入失败的原因。调用方把它映射成协议错误码。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum InjectError {
    /// 上一发还没结束。
    #[error("busy")]
    Busy,
    /// 目标窗口的完整性级别高于本进程。
    #[error("foreground integrity is higher than this process")]
    Integrity,
    /// 没有前台窗口，或者读不到它的完整性级别。
    #[error("foreground window is unavailable")]
    NoForeground,
    /// 剪贴板打不开。
    #[error("clipboard is busy")]
    ClipboardOpen,
    /// 正文已经发出，但剪贴板在中途被改过，因此没有还原。
    #[error("clipboard changed before it could be restored")]
    ClipboardRace,
    /// `SendInput` 没有全部送出。
    #[error("sendinput failed")]
    SendInput,
    /// 正文为空，也没有要求补回车。
    #[error("empty")]
    Empty,
}

/// 把正文注入当前前台窗口。
///
/// `enter` 为真时，正文处理完再补一次回车。粘贴模式下，回车在剪贴板还原之后发送。
/// 不改变前台窗口。焦点在中途被用户点走时，按键仍发往当时的前台，并返回成功。
pub fn inject_text(text: &str, mode: Mode, enter: bool) -> Result<(), InjectError> {
    if text.is_empty() && !enter {
        return Err(InjectError::Empty);
    }
    let _guard = lock()?;
    ensure_integrity()?;
    release_ctrl_if_held()?;
    if !text.is_empty() {
        match mode {
            Mode::Paste => paste(text)?,
            Mode::Type => type_unicode(text)?,
        }
    }
    if enter {
        tap(Key::Enter)?;
    }
    Ok(())
}

/// 向当前前台窗口发送一个按键。
pub fn inject_key(key: Key) -> Result<(), InjectError> {
    let _guard = lock()?;
    ensure_integrity()?;
    if key == Key::Enter {
        release_ctrl_if_held()?;
    }
    tap(key)
}

fn lock() -> Result<std::sync::MutexGuard<'static, ()>, InjectError> {
    match INJECT_LOCK.try_lock() {
        Ok(guard) => Ok(guard),
        Err(TryLockError::WouldBlock) => Err(InjectError::Busy),
        Err(TryLockError::Poisoned(poisoned)) => Ok(poisoned.into_inner()),
    }
}

fn ensure_integrity() -> Result<(), InjectError> {
    match integrity::foreground_is_higher() {
        Ok(true) => Err(InjectError::Integrity),
        Ok(false) => Ok(()),
        Err(Integrity::NoForeground) => Err(InjectError::NoForeground),
        Err(Integrity::Unreadable) => Err(InjectError::NoForeground),
    }
}

/// 用户正按着 Ctrl 时先补一次抬起，避免 Ctrl+V 变成别的组合键。
/// 不动 Shift、Alt 和 Win。
fn release_ctrl_if_held() -> Result<(), InjectError> {
    if input::ctrl_is_down() {
        input::key_up(input::VK_CONTROL_KEY)?;
    }
    Ok(())
}

fn paste(text: &str) -> Result<(), InjectError> {
    let saved = ClipboardText::capture()?;
    let written = saved.write(text)?;
    let pasted = input::ctrl_v();
    std::thread::sleep(PASTE_SETTLE);
    if pasted.is_err() {
        // 按键没送出去，剪贴板按序号能还原就还原。
        let _ = saved.restore_if_unchanged(written);
        return pasted;
    }
    saved.restore_if_unchanged(written)
}

fn type_unicode(text: &str) -> Result<(), InjectError> {
    let units: Vec<u16> = text.encode_utf16().collect();
    for batch in units.chunks(TYPE_BATCH) {
        input::send_unicode(batch)?;
    }
    Ok(())
}

fn tap(key: Key) -> Result<(), InjectError> {
    let vk = match key {
        Key::Enter => input::VK_RETURN,
        Key::Backspace => input::VK_BACK,
        Key::Escape => input::VK_ESCAPE,
    };
    input::tap_vk(vk)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_rejected_before_any_injection() {
        assert_eq!(inject_text("", Mode::Paste, false), Err(InjectError::Empty));
        assert_eq!(inject_text("", Mode::Type, false), Err(InjectError::Empty));
    }
}
