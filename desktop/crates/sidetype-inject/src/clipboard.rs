//! 只处理文本剪贴板。图片和文件不保存，也不还原。

use clipboard_win::raw::seq_num;
use clipboard_win::{get_clipboard_string, set_clipboard_string};

use crate::InjectError;

/// 注入前抓到的文本剪贴板。
pub struct ClipboardText {
    /// 原来的文本。没有文本时是 `None`，还原时清空剪贴板。
    previous: Option<String>,
}

impl ClipboardText {
    /// 读取当前的 Unicode 文本。剪贴板打不开时返回错误。
    pub fn capture() -> Result<Self, InjectError> {
        // 剪贴板里没有文本时读会失败，这是正常情况，记成「原先无文本」。
        let previous = get_clipboard_string().ok();
        Ok(Self { previous })
    }

    /// 写入要粘贴的正文，返回写入后的剪贴板序号。
    pub fn write(&self, text: &str) -> Result<u32, InjectError> {
        set_clipboard_string(text).map_err(|_| InjectError::ClipboardOpen)?;
        Ok(sequence())
    }

    /// 序号没变才把原来的文本写回去。
    ///
    /// 序号变了说明别的程序（或用户）改过剪贴板。此时不覆盖，避免毁掉那份新内容。
    pub fn restore_if_unchanged(&self, written_at: u32) -> Result<(), InjectError> {
        if sequence() != written_at {
            return Err(InjectError::ClipboardRace);
        }
        match &self.previous {
            Some(text) => set_clipboard_string(text).map_err(|_| InjectError::ClipboardRace),
            None => clear().map_err(|_| InjectError::ClipboardRace),
        }
    }
}

fn sequence() -> u32 {
    seq_num().map(|value| value.get()).unwrap_or(0)
}

fn clear() -> Result<(), clipboard_win::ErrorCode> {
    let _clip = clipboard_win::Clipboard::new_attempts(10)?;
    clipboard_win::raw::empty()
}
