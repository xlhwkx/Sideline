//! 比较前台窗口和本进程的完整性级别。
//!
//! Windows 会静默丢掉低权限进程发给高权限窗口的按键，而且 `SendInput` 的返回值
//! 经常看不出来。所以在写剪贴板之前先比较。

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Security::{
    GetSidSubAuthority, GetSidSubAuthorityCount, GetTokenInformation, TokenIntegrityLevel,
    TOKEN_MANDATORY_LABEL, TOKEN_QUERY,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

/// 读完整性级别时的失败。
#[derive(Debug, PartialEq, Eq)]
pub enum Integrity {
    /// 当前没有前台窗口。
    NoForeground,
    /// 窗口在，但打不开进程或令牌。
    Unreadable,
}

/// 前台窗口的完整性级别是否高于本进程。
pub fn foreground_is_higher() -> Result<bool, Integrity> {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.0.is_null() {
        return Err(Integrity::NoForeground);
    }
    let target = integrity_of_window(foreground).ok_or(Integrity::Unreadable)?;
    let own = integrity_of_process(unsafe { GetCurrentProcess() }).ok_or(Integrity::Unreadable)?;
    Ok(target > own)
}

fn integrity_of_window(window: windows::Win32::Foundation::HWND) -> Option<u32> {
    let mut process_id = 0u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    if process_id == 0 {
        return None;
    }
    let process =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }.ok()?;
    let result = integrity_of_process(process);
    unsafe {
        let _ = CloseHandle(process);
    }
    result
}

fn integrity_of_process(process: HANDLE) -> Option<u32> {
    let mut token = HANDLE::default();
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }.ok()?;
    let rid = token_integrity_rid(token);
    unsafe {
        let _ = CloseHandle(token);
    }
    rid
}

fn token_integrity_rid(token: HANDLE) -> Option<u32> {
    let mut size = 0u32;
    // 第一次调用只为了拿需要的字节数，失败是预期行为。
    let _ = unsafe { GetTokenInformation(token, TokenIntegrityLevel, None, 0, &mut size) };
    if size == 0 {
        return None;
    }
    let mut buffer = vec![0u8; size as usize];
    unsafe {
        GetTokenInformation(
            token,
            TokenIntegrityLevel,
            Some(buffer.as_mut_ptr().cast()),
            size,
            &mut size,
        )
    }
    .ok()?;
    let label = unsafe { &*(buffer.as_ptr().cast::<TOKEN_MANDATORY_LABEL>()) };
    let sid = label.Label.Sid;
    if sid.is_invalid() {
        return None;
    }
    let count = unsafe { *GetSidSubAuthorityCount(sid) };
    if count == 0 {
        return None;
    }
    let authority = unsafe { *GetSidSubAuthority(sid, u32::from(count - 1)) };
    Some(authority)
}

/// 前台进程的可执行文件名。只用于诊断，失败时返回空字符串。
#[allow(dead_code)]
pub fn foreground_image_name() -> String {
    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() {
        return String::new();
    }
    let mut process_id = 0u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    let Ok(process) =
        (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) })
    else {
        return String::new();
    };
    let mut buffer = [0u16; 260];
    let mut len = buffer.len() as u32;
    let ok = unsafe {
        QueryFullProcessImageNameW(
            process,
            Default::default(),
            windows::core::PWSTR(buffer.as_mut_ptr()),
            &mut len,
        )
    };
    unsafe {
        let _ = CloseHandle(process);
    }
    if ok.is_err() {
        return String::new();
    }
    String::from_utf16_lossy(&buffer[..len as usize])
}
