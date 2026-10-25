//! `SendInput` 的薄封装。不管焦点，也不碰剪贴板。

use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_RCONTROL, VK_V,
};

use crate::InjectError;

pub const VK_CONTROL_KEY: u16 = VK_CONTROL.0;
pub const VK_RETURN: u16 = windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN.0;
pub const VK_BACK: u16 = windows::Win32::UI::Input::KeyboardAndMouse::VK_BACK.0;
pub const VK_ESCAPE: u16 = windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE.0;
pub const KEY_A: u16 = windows::Win32::UI::Input::KeyboardAndMouse::VK_A.0;
pub const KEY_C: u16 = windows::Win32::UI::Input::KeyboardAndMouse::VK_C.0;
pub const KEY_S: u16 = windows::Win32::UI::Input::KeyboardAndMouse::VK_S.0;
pub const KEY_V: u16 = windows::Win32::UI::Input::KeyboardAndMouse::VK_V.0;
pub const KEY_X: u16 = windows::Win32::UI::Input::KeyboardAndMouse::VK_X.0;
pub const KEY_Y: u16 = windows::Win32::UI::Input::KeyboardAndMouse::VK_Y.0;
pub const KEY_Z: u16 = windows::Win32::UI::Input::KeyboardAndMouse::VK_Z.0;

/// 按住 Ctrl 点一个键，再把两个都抬起。
pub fn ctrl_then(vk: u16) -> Result<(), InjectError> {
    send(&[
        key_event(VK_CONTROL.0, 0, KEYBD_EVENT_FLAGS(0)),
        key_event(vk, 0, KEYBD_EVENT_FLAGS(0)),
        key_event(vk, 0, KEYEVENTF_KEYUP),
        key_event(VK_CONTROL.0, 0, KEYEVENTF_KEYUP),
    ])
}

/// Ctrl 是否处于按下状态。左右 Ctrl 都算。
pub fn ctrl_is_down() -> bool {
    key_is_down(VK_CONTROL.0) || key_is_down(VK_LCONTROL.0) || key_is_down(VK_RCONTROL.0)
}

fn key_is_down(vk: u16) -> bool {
    // 高位为 1 表示当前按下。
    (unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(i32::from(vk)) }) < 0
}

/// 按下但不抬起。和 [`key_up`] 成对使用。
pub fn tap_vk_down(vk: u16) -> Result<(), InjectError> {
    send(&[key_event(vk, 0, KEYBD_EVENT_FLAGS(0))])
}

/// 抬起一个虚拟键。
pub fn key_up(vk: u16) -> Result<(), InjectError> {
    send(&[key_event(vk, 0, KEYEVENTF_KEYUP)])
}

/// 点按一个虚拟键。
pub fn tap_vk(vk: u16) -> Result<(), InjectError> {
    send(&[
        key_event(vk, 0, KEYBD_EVENT_FLAGS(0)),
        key_event(vk, 0, KEYEVENTF_KEYUP),
    ])
}

/// 一次提交 Ctrl+V。调用前应先确认 Ctrl 没有被按住。
///
/// 用虚拟键 `VK_V`，不经过 `VkKeyScan`。大写 `V` 会被当成需要 Shift，
/// 而 Ctrl+V 跟当前键盘布局无关。
pub fn ctrl_v() -> Result<(), InjectError> {
    send(&[
        key_event(VK_CONTROL.0, 0, KEYBD_EVENT_FLAGS(0)),
        key_event(VK_V.0, 0, KEYBD_EVENT_FLAGS(0)),
        key_event(VK_V.0, 0, KEYEVENTF_KEYUP),
        key_event(VK_CONTROL.0, 0, KEYEVENTF_KEYUP),
    ])
}

/// 发送一批 UTF-16 码元。
///
/// 基本平面的字符单独按下再抬起。一个非基本平面字符的两个代理项先都按下，
/// 再按相反顺序抬起。`units` 必须是完整的 UTF-16，不能从代理对中间切开。
pub fn send_unicode(units: &[u16]) -> Result<(), InjectError> {
    let mut events = Vec::with_capacity(units.len() * 2);
    let mut index = 0;
    while index < units.len() {
        let unit = units[index];
        let is_high_surrogate = (0xD800..=0xDBFF).contains(&unit);
        if is_high_surrogate {
            let pair = units
                .get(index + 1)
                .copied()
                .ok_or(InjectError::SendInput)?;
            events.push(unicode_event(unit, false));
            events.push(unicode_event(pair, false));
            events.push(unicode_event(pair, true));
            events.push(unicode_event(unit, true));
            index += 2;
        } else {
            events.push(unicode_event(unit, false));
            events.push(unicode_event(unit, true));
            index += 1;
        }
    }
    send(&events)
}

fn unicode_event(unit: u16, up: bool) -> INPUT {
    let flags = if up {
        KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
    } else {
        KEYEVENTF_UNICODE
    };
    key_event(0, unit, flags)
}

fn key_event(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn send(events: &[INPUT]) -> Result<(), InjectError> {
    if events.is_empty() {
        return Ok(());
    }
    let size = i32::try_from(size_of::<INPUT>()).unwrap_or(0);
    let sent = unsafe { SendInput(events, size) };
    if usize::try_from(sent).ok() == Some(events.len()) {
        Ok(())
    } else {
        Err(InjectError::SendInput)
    }
}
