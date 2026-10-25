//! 注入命令行。不监听端口，用来在接入界面之前验证注入。
//!
//! ```text
//! sidetype-inject paste "你好"
//! sidetype-inject type "😀"
//! sidetype-inject key enter
//! ```
//!
//! 退出码 0 成功。退出码 2 是目标窗口权限更高。其余失败退出码为 1。

use std::env;
use std::process::ExitCode;

use sidetype_inject::{inject_key, inject_text, InjectError, Key, Mode};

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure::Usage(text)) => {
            eprintln!("{text}");
            ExitCode::from(1)
        }
        Err(Failure::Inject(InjectError::Integrity)) => {
            eprintln!("目标窗口以管理员运行。请同样以管理员运行 sidetype-inject。");
            ExitCode::from(2)
        }
        Err(Failure::Inject(error)) => {
            eprintln!("注入失败：{error}");
            ExitCode::from(1)
        }
    }
}

enum Failure {
    Usage(String),
    Inject(InjectError),
}

fn run(mut args: impl Iterator<Item = String>) -> Result<(), Failure> {
    let command = args.next().ok_or_else(|| Failure::Usage(usage()))?;
    match command.as_str() {
        "paste" | "type" => {
            let text = args.next().ok_or_else(|| Failure::Usage(usage()))?;
            let enter = args.next().as_deref() == Some("--enter");
            let mode = if command == "paste" {
                Mode::Paste
            } else {
                Mode::Type
            };
            inject_text(&text, mode, enter).map_err(Failure::Inject)
        }
        "key" => {
            let name = args.next().ok_or_else(|| Failure::Usage(usage()))?;
            let key = match name.as_str() {
                "enter" => Key::Enter,
                "backspace" => Key::Backspace,
                "escape" => Key::Escape,
                "copy" => Key::Copy,
                "paste-key" => Key::Paste,
                "cut" => Key::Cut,
                "select-all" => Key::SelectAll,
                "undo" => Key::Undo,
                "save" => Key::Save,
                "redo" => Key::Redo,
                _ => return Err(Failure::Usage(usage())),
            };
            inject_key(key).map_err(Failure::Inject)
        }
        _ => Err(Failure::Usage(usage())),
    }
}

fn usage() -> String {
    "用法：sidetype-inject <paste|type> <文字> [--enter] | sidetype-inject key <enter|backspace|escape|copy|paste-key|cut|select-all|undo|save|redo>".into()
}
