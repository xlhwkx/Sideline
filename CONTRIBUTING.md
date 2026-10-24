# 贡献

## 模块边界

依赖只能向下。

- `protocol/v1.md` 是两端的契约。改它必须同时改 `protocol/fixtures/`。
- `sidetype-protocol` 只有类型和序列化，没有网络，没有 Win32。
- `sidetype-inject` 只有 Win32 注入，没有网络，没有界面。
- `sidetype-server` 只有 WebSocket、配对和会话。注入通过 `sidetype-inject` 的函数完成。
- `src-tauri` 是唯一碰 Tauri 的包，只做托盘、窗口和组装。
- 安卓的 `session` 不引用界面，界面不直接发网络请求。

界面颜色来自一处定义。电脑端是 `pair.rs` 里的 CSS 变量，安卓端是 `MagpieColors.kt`。不要在界面里写死颜色。

## 提交前

```powershell
cargo fmt --manifest-path desktop/Cargo.toml --check
cargo clippy --manifest-path desktop/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path desktop/Cargo.toml
```

库代码不用 `unwrap` 和 `expect`。日志不打印正文、配对码和 token。

打进真实窗口的验证不在测试里自动跑。用 `sidetype-inject` 对记事本手动确认：中文、emoji、粘贴后原来的剪贴板还在。
