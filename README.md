# FastPaste

<!-- release-info:start -->
> **当前版本**: `v0.2.4` | **构建时间**: 2026-08-21 17:30:13 | **Commit**: `3bec06d`
<!-- release-info:end -->

跨平台静默剪贴板管理器，常驻托盘、监听剪贴板、通过全局热键将最近 10 条纯文本条目直接粘贴到焦点窗口。

## 功能

- **历史**：容量为 10 的环形队列，按复制时间倒序，`1` 为最新，`0` 为最旧。去重后置顶，空或纯空白不入队，超过 `max_entry_bytes`（默认 5MB）的条目不视为条目。
- **敏感内容**：被系统标记为瞬态（transient）或匹配 `ignore_regex` 的文本不进入历史。Windows 下检测 `ExcludeClipboardContentFromMonitorProcessing` / `CanIncludeInClipboardHistory` / `ClipboardViewerIgnore` 等格式；macOS 暂未实现则视为非敏感内容。
- **粘贴**：将指定条目直接注入到当前焦点窗口的输入焦点。默认 `paste_method = auto` 自动选择：短且无控制字符（`c < '\u{20}'`，含换行/Tab）且不超过 1000 个 Unicode 标量走击键注入，否则走剪贴板粘贴。`paste_method = clipboard` / `type` 可锁死；`type` 含换行时会发 Enter，聊天软件可能直接发送。Windows 击键注入为 Unicode 按键事件，不经剪贴板。热键触发只等数字键松开（不等待 Ctrl/Shift），Pressed 才触发，进行中的粘贴会忽略重复热键，热键打到无条目的槽位时静默忽略。剪贴板粘贴失败会写入剪贴板并在主线程对话框提示手动粘贴；击键注入失败则提示击键未成功。
- **保留剪贴板**：开关 `preserve_clipboard`，`true` 时执行备份-粘贴-恢复，支持文本与图片的恢复。

## 热键

`配置` 中 `hotkeys` 为 10 条，依次对应 `1..9,0`（`1` 最新，`0` 最旧），默认：

```
ctrl+shift+1
ctrl+shift+2
...
ctrl+shift+9
ctrl+shift+0
```

格式为 `ctrl+shift+<key>`，主键支持 `0-9`、`a-z`、`F1-F24`、`space` 等 `global-hotkey` 的 `Code`。未知主键会在启动时报错。

## 配置

持久化于系统配置目录的 `config.toml`：

- Windows: `%APPDATA%\FastPaste\config.toml`
- macOS: `~/Library/Application Support/FastPaste/config.toml`
- Linux: `~/.config/FastPaste/config.toml`

主要选项：

```toml
hotkeys = ["ctrl+shift+1", ...]
preserve_clipboard = true
paste_method = "auto" # auto | clipboard | type
polling_interval_ms = 500
max_entry_bytes = 5242880 # 5MB
ignore_regex = "password|secret"
autostart_elevated = true
```

已有 `config.toml` 若写着 `clipboard`，重载后仍是剪贴板粘贴；改成 `auto` 或删文件重建才用自动选择。`clipboard` 不会自动迁移为 `auto`。

配置文件由英文注释说明每个键与合法取值，修改保存后自动生效（约 1–2 秒内），也可随时通过托盘 **重载配置** 手动重载。粘贴进行时暂时不把剪贴板变化记入历史，避免把恢复的旧剪贴板当成新复制。

`validate` 会校验热键数量、轮询间隔、`max_entry_bytes` 范围及 `ignore_regex` 合法性。校验失败时自动重载静默保留旧配置（见日志），手动重载则弹对话框且不应用。

## 托盘

应用唯一常驻界面，承载：

- 启用状态 / 停用状态 切换（同时控制剪贴板监听与全局热键响应，历史保留，停用期复制不补录）
- 历史预览：`digit: ` + 前 20 字预览（换行变空格，超长 `...`），有条目可点并触发粘贴，无条目显示 `(空)` 且不可点，约 200ms 刷新
- 打开配置
- 重载配置：通过统一重载管线重新读取并校验，更新 `ignore_regex` 与 `max_entry_bytes`；配置文件变更亦会自动重载
- 自启开关
- 以管理员身份重启（Windows）
- 权限诊断
- 关于（显示 `CARGO_PKG_VERSION`）
- 退出

## Windows 提权与自启

- **提权**：`cargo build --release` / 发布 exe 嵌入 `requireAdministrator` manifest，启动由系统弹 UAC。`debug` 与 `cargo test` 嵌入 `asInvoker`，不强制 UAC；未提权时仍走 `powershell -NoProfile -Command "try { Start-Process -FilePath '...' -Verb RunAs -ErrorAction Stop } catch { exit 1 }"` 同步提权，拒绝则原进程继续运行，托盘仍在。日常开发用 `cargo run`；给用户的包必须用 `release`。
- **自启**：勾选自启时以 Task Scheduler 创建登录触发、最高权限（`HIGHEST`）任务 `FastPaste`，创建失败直接报错不写入 `HKCU\...\Run`。关闭时删除任务并清理可能残留的注册表 `FastPaste` 值。`is_enabled` 同时检测任务与注册表残留。默认 `autostart_elevated = true`。

`build.rs` 在 Windows 上通过 `embed-resource` 编译 `app.manifest`（`release` 为 `requireAdministrator`，`debug` 为 `asInvoker`）。
cargo build --release / 发布 exe 嵌入 requireAdministrator，启动由系统弹 UAC
debug 与 cargo test 嵌入 asInvoker，不强制 UAC；未提权时仍走 PowerShell try_elevate，拒绝则原进程继续
日常开发用 cargo run；给用户的包必须用 release

## macOS 辅助功能

`权限诊断` 中会提示在系统设置 → 隐私与安全性 → 辅助功能 中勾选 FastPaste 以允许 `enigo` 注入按键。文案中已说明：辅助功能授权后，本应用可向其它窗口注入按键，请只在信任本应用时开启。

## 权限诊断

托盘的权限诊断入口：

- Windows：展示管理员权限、自启状态、热键占用、管理员窗口粘贴提示，并提示“以管理员身份常驻会扩大剪贴板中敏感内容的暴露面。若你不需要向管理员窗口粘贴，可拒绝 UAC、以普通权限运行。”
- macOS：展示辅助功能授权说明及风险提示
- Linux：提示当前平台诊断暂未实现

## 单实例

通过 named mutex / file lock 保证单实例，第二实例退出并提示已在运行（不唤醒已有实例）。

## 构建与运行

```bash
cargo test
cargo run
cargo build --release
```

### 快捷编译（Windows 双击）

- 双击 `build.bat`：默认 `cargo build`（debug，快速）
- `build.bat -Release`：`cargo build --release`
- `build.bat -Release -Run` / `build.bat -Test` 等透传至 `build.ps1`，详见 `build.ps1 -Help`

`build.ps1` 为 PowerShell 实现，`build.bat` 以 `Bypass` 调用以规避执行策略；`target/` 已在 `.gitignore` 忽略。

### 一键 Release（Windows 双击）

双击根目录的 `release.bat` 即可一键完成：

1. 读取 `Cargo.toml` 版本（可交互式输入新版本 `0.x.y` 自动同步到 `Cargo.toml` / `app.manifest`）
2. `cargo test` → `cargo build --release`（`--release` 已启用 `lto = true`，耗时较长）
3. 自动更新 `README.md` 顶部的 `<!-- release-info -->` 版本/时间/Commit
4. 自动更新 `CHANGELOG.md` 日期与条目
5. 复制产物到 `dist/fastpaste.exe` 并生成 `dist/fastpaste.exe.sha256`
6. 输出产物大小与 `git` 提交提示

```bash
# 命令行等价用法
.\release.ps1                         # 交互式，保持当前版本
.\release.ps1 -Version 0.2.0           # bump 到 0.2.0 再编译
.\release.ps1 -SkipTest               # 跳过测试
.\release.ps1 -SkipBuild              # 仅更新文档，不编译（调试用）
release.bat 0.2.0 -SkipTest           # bat 简写同样支持
```

直接双击 `release.ps1` 可能被执行策略拦截，请双击 `release.bat`（内部以 `Bypass` 调用）。

轮询监听为 500ms `arboard` 轮询，预留 `ClipboardWatcher` trait。

## 许可

MIT

