//! 列出用户能切换到的窗口，并在点选后把它拉到前台。
//!
//! 窗口编号是句柄转成的整数，只在这次开机里有效。直接调用
//! `SetForegroundWindow` 会被系统拒绝，所以先补一次 Alt 按下。

use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::System::Threading::GetCurrentProcessId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_LCONTROL, VK_MENU, VK_RCONTROL,
};
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetAncestor, GetForegroundWindow, GetWindow, GetWindowLongPtrW,
    GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow,
    IsWindowVisible, IsZoomed, SetForegroundWindow, ShowWindow, GA_ROOT, GWL_EXSTYLE, GW_OWNER,
    SHOW_WINDOW_CMD, SW_MAXIMIZE, SW_MINIMIZE, SW_RESTORE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON};

/// 手机上能点的一个窗口。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SwitchableWindow {
    /// 句柄转成的整数。只在这次开机里有效。
    pub id: isize,
    /// 标题栏上的文字。
    pub title: String,
    /// 是否置顶。
    pub topmost: bool,
    /// 是否最小化到任务栏。
    pub minimized: bool,
    /// 是否最大化。
    pub maximized: bool,
    /// 程序图标的 PNG，Base64。取不到时为空。
    pub icon: String,
}

/// 当前前台窗口的句柄。最小化时返回空：再点一次才最小化，缩着的窗口再点应该还原。
pub fn foreground_id() -> Option<isize> {
    let window = unsafe { GetForegroundWindow() };
    if window.0.is_null() || unsafe { IsIconic(window) } == BOOL(1) {
        return None;
    }
    Some(window.0 as isize)
}

/// 当前能切换过去的顶层窗口，按句柄排序，不按 Z 序。
pub fn list_switchable_windows() -> Vec<SwitchableWindow> {
    let mut found = Vec::new();
    let pointer: *mut Vec<SwitchableWindow> = &mut found;
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM(pointer as isize));
    }
    found.sort_by_key(|window| window.id);
    found
}

/// 最小化一个窗口。
///
/// `ShowWindow(SW_MINIMIZE)` 只改状态，窗口停在后台时屏幕上看不出变化。
/// 先激活再最小化，看着 UU 的人才能看见它收进任务栏。
pub fn minimize_window(id: isize) -> bool {
    if hwnd(id).is_none() {
        return false;
    }
    activate_window(id);
    show(id, SW_MINIMIZE)
}

/// 最大化一个窗口。已经最大化时还原。
pub fn maximize_window(id: isize) -> bool {
    let window = hwnd(id);
    if window.is_none() {
        return false;
    }
    let window = window.unwrap();
    if unsafe { IsZoomed(window) } == BOOL(1) {
        unsafe {
            let _ = ShowWindow(window, SW_RESTORE);
        };
        activate_window(id);
        return true;
    }
    if unsafe { IsIconic(window) } == BOOL(1) {
        unsafe {
            let _ = ShowWindow(window, SW_RESTORE);
        };
    }
    unsafe {
        let _ = ShowWindow(window, SW_MAXIMIZE);
    };
    activate_window(id);
    true
}

fn hwnd(id: isize) -> Option<HWND> {
    let window = HWND(id as *mut core::ffi::c_void);
    if unsafe { IsWindow(Some(window)) } == BOOL(0) {
        None
    } else {
        Some(window)
    }
}

fn show(id: isize, command: SHOW_WINDOW_CMD) -> bool {
    let Some(window) = hwnd(id) else { return false };
    unsafe {
        let _ = ShowWindow(window, command);
    };
    true
}

/// 把一个窗口拉到前台。窗口已关闭或系统拒绝时返回 false。
pub fn activate_window(id: isize) -> bool {
    let window = HWND(id as *mut core::ffi::c_void);
    if unsafe { IsWindow(Some(window)) } == BOOL(0) {
        return false;
    }
    if unsafe { IsIconic(window) } == BOOL(1) {
        unsafe {
            let _ = ShowWindow(window, SW_RESTORE);
        }
    }
    // 用户正按着修饰键时不再补 Alt，避免把下一次按键变成组合键。
    if modifier_is_down() {
        return false;
    }
    let alt = VK_MENU.0;
    let _ = crate::input::tap_vk_down(alt);
    unsafe {
        let _ = SetForegroundWindow(window);
        let _ = BringWindowToTop(window);
    }
    let _ = crate::input::key_up(alt);
    let foreground = unsafe { GetForegroundWindow() };
    if foreground != window {
        return false;
    }
    // 窗口到前台之后，里面有输入框就把它点亮。控件树很大的窗口里这一步要扫好几秒，
    // 所以只等一小会儿。超时就先让切换返回，搜索在后台结束，不再占用注入锁。
    focus_editor_bounded(window, EDITOR_FOCUS_BUDGET);
    true
}

/// 找输入框最多花的时间。超过就放弃聚焦，切换本身已经完成。
const EDITOR_FOCUS_BUDGET: std::time::Duration = std::time::Duration::from_millis(300);

fn focus_editor_bounded(window: HWND, budget: std::time::Duration) {
    // HWND 里是原始指针，不能跨线程。句柄的值是整数，传过去再还原。
    let handle = window.0 as isize;
    let search = std::thread::spawn(move || focus_editor(HWND(handle as *mut core::ffi::c_void)));
    let started = std::time::Instant::now();
    loop {
        if search.is_finished() {
            break;
        }
        if started.elapsed() >= budget {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// 在一个窗口里找第一个能打字的控件并给它焦点。
///
/// 只认编辑框、组合框，以及带窗口类名的文档。Electron 会把整个窗口标成文档，
/// 但它没有类名；记事本的编辑区也标成文档，类名是 RichEdit。找不到就保持原焦点。
fn focus_editor(window: HWND) {
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, TreeScope_Descendants};

    unsafe {
        // 同一线程重复初始化会返回「已经初始化」，照样可用。
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let Ok(automation) =
            CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER)
        else {
            return;
        };
        let Ok(root) = automation.ElementFromHandle(window) else {
            return;
        };
        let Ok(condition) = editor_condition(&automation) else {
            return;
        };
        let Ok(editor) = root.FindFirst(TreeScope_Descendants, &condition) else {
            return;
        };
        // 没类名的文档是整个窗口，不是输入区。跳过它，去找后面真正的输入框。
        if element_is_bare_document(&editor) {
            let Ok(editors) = root.FindAll(TreeScope_Descendants, &condition) else {
                return;
            };
            let count = editors.Length().unwrap_or(0);
            for index in 0..count {
                let Ok(candidate) = editors.GetElement(index) else {
                    continue;
                };
                if !element_is_bare_document(&candidate) {
                    let _ = candidate.SetFocus();
                    return;
                }
            }
            return;
        }
        let _ = editor.SetFocus();
    }
}

/// 没有窗口类名的文档。Electron 用它代表整个窗口，不是一个输入区。
fn element_is_bare_document(
    element: &windows::Win32::UI::Accessibility::IUIAutomationElement,
) -> bool {
    use windows::Win32::UI::Accessibility::UIA_DocumentControlTypeId;
    unsafe {
        element.CurrentControlType().map(|kind| kind.0).unwrap_or(0) == UIA_DocumentControlTypeId.0
            && element.CurrentClassName().unwrap_or_default().is_empty()
    }
}

/// 编辑框、组合框或文档，并且可聚焦、可用、在屏幕上。
fn editor_condition(
    automation: &windows::Win32::UI::Accessibility::IUIAutomation,
) -> windows::core::Result<windows::Win32::UI::Accessibility::IUIAutomationCondition> {
    use windows::Win32::System::Variant::VARIANT;
    use windows::Win32::UI::Accessibility::{
        UIA_ComboBoxControlTypeId, UIA_ControlTypePropertyId, UIA_DocumentControlTypeId,
        UIA_EditControlTypeId, UIA_IsEnabledPropertyId, UIA_IsKeyboardFocusablePropertyId,
        UIA_IsOffscreenPropertyId,
    };

    unsafe {
        let control_type = |kind: i32| {
            automation.CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(kind))
        };
        let kinds = automation.CreateOrConditionFromNativeArray(&[
            Some(control_type(UIA_EditControlTypeId.0)?),
            Some(control_type(UIA_DocumentControlTypeId.0)?),
            Some(control_type(UIA_ComboBoxControlTypeId.0)?),
        ])?;
        let yes = VARIANT::from(true);
        let focusable =
            automation.CreatePropertyCondition(UIA_IsKeyboardFocusablePropertyId, &yes)?;
        let enabled = automation.CreatePropertyCondition(UIA_IsEnabledPropertyId, &yes)?;
        let no = VARIANT::from(false);
        let onscreen = automation.CreatePropertyCondition(UIA_IsOffscreenPropertyId, &no)?;
        automation.CreateAndConditionFromNativeArray(&[
            Some(kinds),
            Some(focusable),
            Some(enabled),
            Some(onscreen),
        ])
    }
}

unsafe extern "system" fn collect(window: HWND, data: LPARAM) -> BOOL {
    if let Some(item) = describe(window) {
        let found = unsafe { &mut *(data.0 as *mut Vec<SwitchableWindow>) };
        found.push(item);
    }
    BOOL(1)
}

fn describe(window: HWND) -> Option<SwitchableWindow> {
    unsafe {
        if IsWindowVisible(window) == BOOL(0) {
            return None;
        }
        if GetAncestor(window, GA_ROOT) != window {
            return None;
        }
        if GetWindow(window, GW_OWNER).is_ok() {
            return None;
        }
        let style = GetWindowLongPtrW(window, GWL_EXSTYLE) as u32;
        if style & (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) != 0 {
            return None;
        }
        if cloaked(window) {
            return None;
        }
        let mut process_id = 0u32;
        GetWindowThreadProcessId(window, Some(&mut process_id));
        if process_id == 0 || process_id == GetCurrentProcessId() {
            return None;
        }
        let title = title_of(window)?;
        Some(SwitchableWindow {
            id: window.0 as isize,
            title,
            topmost: style & WS_EX_TOPMOST.0 != 0,
            minimized: IsIconic(window) == BOOL(1),
            maximized: IsZoomed(window) == BOOL(1),
            icon: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                icon_of(window),
            ),
        })
    }
}

fn cloaked(window: HWND) -> bool {
    let mut cloaked = 0u32;
    let ok = unsafe {
        DwmGetWindowAttribute(
            window,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
    };
    ok.is_err() || cloaked != 0
}

/// 取窗口所属程序的图标，编码成 PNG。取不到时返回空。
fn icon_of(window: HWND) -> Vec<u8> {
    let mut process_id = 0u32;
    unsafe { GetWindowThreadProcessId(window, Some(&mut process_id)) };
    if process_id == 0 {
        return Vec::new();
    }
    let process = match unsafe {
        windows::Win32::System::Threading::OpenProcess(
            windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
            false,
            process_id,
        )
    } {
        Ok(handle) => handle,
        Err(_) => return Vec::new(),
    };
    let mut path = [0u16; 260];
    let mut size = path.len() as u32;
    let read = unsafe {
        windows::Win32::System::Threading::QueryFullProcessImageNameW(
            process,
            windows::Win32::System::Threading::PROCESS_NAME_WIN32,
            windows::core::PWSTR(path.as_mut_ptr()),
            &mut size,
        )
    };
    unsafe {
        let _ = windows::Win32::Foundation::CloseHandle(process);
    }
    if read.is_err() || size == 0 {
        return Vec::new();
    }
    let mut info = SHFILEINFOW::default();
    let got = unsafe {
        SHGetFileInfoW(
            windows::core::PCWSTR(path.as_ptr()),
            FILE_FLAGS_AND_ATTRIBUTES(0),
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if got == 0 || info.hIcon.is_invalid() {
        return Vec::new();
    }
    let bytes = icon_png(info.hIcon);
    unsafe {
        let _ = DestroyIcon(info.hIcon);
    }
    bytes
}

/// 把一个图标画成 PNG。
fn icon_png(icon: HICON) -> Vec<u8> {
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, SelectObject, BITMAP,
        BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS,
    };
    let mut icon_info = windows::Win32::UI::WindowsAndMessaging::ICONINFO::default();
    if unsafe { GetIconInfo(icon, &mut icon_info) }.is_err() {
        return Vec::new();
    }
    let mut bitmap = BITMAP::default();
    unsafe {
        GetObjectW(
            icon_info.hbmColor.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut _ as *mut _),
        )
    };
    let width = bitmap.bmWidth;
    let height = bitmap.bmHeight;
    let screen = unsafe { windows::Win32::Graphics::Gdi::GetDC(None) };
    let dc = unsafe { CreateCompatibleDC(Some(screen)) };
    unsafe { windows::Win32::Graphics::Gdi::ReleaseDC(None, screen) };
    let previous = unsafe { SelectObject(dc, icon_info.hbmColor.into()) };
    let header = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width,
        biHeight: -height,
        biPlanes: 1,
        biBitCount: 32,
        ..Default::default()
    };
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let read = unsafe {
        GetDIBits(
            dc,
            icon_info.hbmColor,
            0,
            height as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut BITMAPINFO {
                bmiHeader: header,
                ..Default::default()
            },
            DIB_RGB_COLORS,
        )
    };
    unsafe {
        SelectObject(dc, previous);
        let _ = DeleteDC(dc);
        let _ = DeleteObject(icon_info.hbmMask.into());
        let _ = DeleteObject(icon_info.hbmColor.into());
    }
    let _ = header;
    if read == 0 || width <= 0 || height <= 0 {
        return Vec::new();
    }
    for pixel in pixels.as_chunks_mut::<4>().0 {
        pixel.swap(0, 2);
    }
    let image = image::RgbaImage::from_raw(width as u32, height as u32, pixels);
    let Some(image) = image else {
        return Vec::new();
    };
    let mut bytes = std::io::Cursor::new(Vec::new());
    if image.write_to(&mut bytes, image::ImageFormat::Png).is_err() {
        return Vec::new();
    }
    bytes.into_inner()
}

fn title_of(window: HWND) -> Option<String> {
    let length = unsafe { GetWindowTextLengthW(window) };
    if length <= 0 {
        return None;
    }
    let mut buffer = vec![0u16; length as usize + 1];
    let written = unsafe { GetWindowTextW(window, &mut buffer) };
    if written <= 0 {
        return None;
    }
    let title = String::from_utf16_lossy(&buffer[..written as usize]);
    let title = title.trim();
    if title.is_empty() {
        None
    } else {
        Some(title.to_owned())
    }
}

fn modifier_is_down() -> bool {
    [VK_MENU.0, VK_CONTROL.0, VK_LCONTROL.0, VK_RCONTROL.0]
        .into_iter()
        .any(|vk| unsafe { GetAsyncKeyState(i32::from(vk)) } < 0)
}

/// 让未使用的导入在签名检查时可见。
#[allow(dead_code)]
fn _show_cmd(cmd: SHOW_WINDOW_CMD) -> SHOW_WINDOW_CMD {
    cmd
}
