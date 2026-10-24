# Sideline

平板用网易 UU 远程看和点 Windows 电脑时，打字会弹出 UU 的软键盘挡住画面。Sideline 让一部安卓手机专门当键盘：用手机自己的输入法打字，点发送，文字出现在电脑当前的窗口里。平板继续负责看和点。

手机上也能看到电脑打开的窗口，点一下就切过去，也可以最小化和最大化。切到一个窗口后，Sideline 会找到里面的输入框并聚焦，所以不用先在平板上点一下输入位置。

## 组成

```
手机（Kotlin，系统输入法）
        │  ws://电脑:17331/v1
        ▼
电脑（Tauri 2 托盘程序）
        ├─ 剪贴板 + Ctrl+V，粘贴后按序号还原
        └─ 逐个字符发送 Unicode 按键
```

协议在 [`protocol/v1.md`](protocol/v1.md)。电脑端不记录发送的内容。

## 现在能用的部分

电脑端和安卓端都可以用。电脑端是托盘程序，安卓端是一个输入界面。

每个版本的发布页附带两个安装包：`Sideline_<版本>_x64-setup.exe` 是电脑端，装完托盘里出现图标；`Sideline_<版本>.apk` 是手机端，下载后直接安装。手机需要允许安装未知来源的应用。

协议是公开的，任何 WebSocket 客户端都可以按协议发送。

自己编译电脑端：

```powershell
cargo build --manifest-path desktop/Cargo.toml -p sidetype
desktop\target\debug\sidetype.exe
```

托盘图标左键打开配对窗口，里面有 6 位配对码和二维码。配对码 60 秒过期，关掉窗口立即作废。

只想验证注入、不启动托盘时：

```powershell
cargo build --manifest-path desktop/Cargo.toml -p sidetype-inject-cli
# 先点一下目标窗口，再运行
desktop\target\debug\sidetype-inject.exe paste "你好"
desktop\target\debug\sidetype-inject.exe type "😀"
```

## 外出使用

不提供中转服务器。两端安装 [Tailscale](https://tailscale.com/)，加入同一个网络，手机填写电脑的 Tailscale 地址。协议不变。

不要把 17331 端口映射到公网。如果 Windows 防火墙拦截了局域网连接，在确认是自己的网络之后执行：

```powershell
netsh advfirewall firewall add rule name="Sideline" dir=in action=allow protocol=TCP localport=17331 profile=private
```

## 已知限制

- 目标窗口以管理员身份运行时，普通权限的程序发过去的按键会被 Windows 丢掉。这时 Sideline 会返回 `integrity`，不会静默失败。把 Sideline 也以管理员身份运行即可。
- 粘贴期间如果别的程序改了剪贴板（UU 远程的剪贴板同步会这么做），Sideline 不会覆盖那份新内容，并返回 `clipboard_race`。正文通常已经粘贴成功。这种情况改用逐字模式。
- 发送过程中如果焦点被点到别的窗口，文字会进到新的焦点窗口，不会追回原来的窗口。
- 只还原文本剪贴板。原来剪贴板里是图片或文件时，粘贴之后会被清空。

## 开发

```powershell
cargo fmt --manifest-path desktop/Cargo.toml --check
cargo clippy --manifest-path desktop/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path desktop/Cargo.toml
```

模块边界和这些命令写在 [`CONTRIBUTING.md`](CONTRIBUTING.md)。

## 安全

- 服务监听局域网。不要把 17331 端口映射到公网，也不要把它暴露在不受信任的网络上。
- 配对码 60 秒过期，关掉配对窗口立即作废。连续 5 次配对失败会断开连接。
- 配对得到的 token 在电脑上只保存 SHA-256，手机保存在加密存储里。托盘菜单可以撤销已配对的设备。
- 电脑端不记录发送的文字、配对码和 token。
- 目标窗口权限更高时，Sideline 拒绝发送并返回 `integrity`，不会用驱动或其他方式绕过系统的权限隔离。
- 调试端口只在调试构建里打开，并且只监听本机。

## 许可

[MIT](LICENSE)
