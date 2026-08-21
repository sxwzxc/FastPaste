# Changelog

所有显著变更将记录于此文件。格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，版本号遵循 [SemVer](https://semver.org/lang/zh-CN/)。

## [0.2.3] - 2026-08-21

### Changed
- 默认粘贴方式改为自动选择：短且无控制字符走击键注入，否则剪贴板粘贴（ADR-0005）
- Windows 击键注入改为 Unicode SendInput；剪贴板粘贴延迟改为 100/10/300 ms
- 热键触发只等待数字键松开；发送 Ctrl+V / Enter / Tab 前抬起修饰键且不恢复

## [0.2.2] - 2026-08-21

### Fixed
- `src/clipboard.rs:290` `tick()` 时序修复：每次 tick 先 `poll()` 再判断三态；`INGEST_CATCHUP` 时无论 `poll()` 为 `Some` 还是 `None` 都 `store(INGEST_LIVE)` 且不入队，修复 `CATCHUP` 在 `poll=None` 时卡住不回 `LIVE` 的问题
- 粘贴期间剪贴板暂停入队：`src/app.rs:15` 新增 `PasteGate` 单次飞行 + `ingest_paused` 三态（`LIVE/PAUSED/CATCHUP`），`handle_paste` 中 `PasteGuard` Drop 时 `store(CATCHUP)`，`spawn_clipboard_thread` 透传 `ingest_paused` 至 `ClipboardManager`
- 热键稳定性：`src/hotkey.rs:95` 仅处理 `Pressed` 状态，过滤 `Released`/重复事件；`src/paste.rs:7` 粘贴前 `wait_for_hotkey_release(800ms)` 等待修饰键松开，Windows 改用虚拟键 `Key::V` 注入
- `src/config.rs:135` `documented_toml` 默认写出的 `config.toml` 带英文注释，`ignore_regex` 含引号时正确转义并可往返加载

## [0.2.1] - 2026-08-21

### Added
- 图标重设计：全新剪贴板 + 闪电简洁图标，适配亮/暗任务栏，高对比度，16px 仍清晰（`assets/icon.ico` 多尺寸、`assets/icon.png`、`assets/preview.png`）
- `assets/generate_icon.py` 一键生成多尺寸图标（16/20/24/32/48/64/256/512）与 ICO
- `src/icon_data.rs` 嵌入 32x32 RGBA，`tray.rs:162` 改用新图标，告别纯蓝方块
- 默认写出的 `config.toml` 带英文注释

### Changed
- `build.rs` 同时嵌入图标与 manifest（`1 ICON` + `1 24`），`debug` 版本号同步至 0.2.1.0
- `src/main.rs` 新增 `mod icon_data`

### Fixed
- 托盘图标在亮色任务栏下可见性差、细节丢失
- 热键仍按下时模拟 Ctrl+V 无法注入焦点窗口；现等待松开，Windows 改用 `Key::V`
- 热键 Released/连发导致重复或失败粘贴，现仅处理 Pressed 且同一时刻只允许一次粘贴；粘贴期间不把剪贴板变化入队

## [0.2.0] - 2026-08-21

### Added
- 一键编译：`build.bat`/`build.ps1`（默认 `cargo build`，` -Release` 切 `release`，支持 `-Test`/`-Run`，`Bypass` 绕过执行策略）
- 一键发布：`release.bat`/`release.ps1`（`cargo test` → `cargo build --release` → 更新 `README` 顶部 `release-info` 与 `CHANGELOG` → 复制 `dist/fastpaste.exe` + `sha256`），双击即可编译，`SkipTest`/`SkipBuild` 可跳步
- `build.rs` + `app.manifest`：`release` 嵌入 `requireAdministrator`，`debug`/`test` 嵌入 `asInvoker`，避免 `cargo test` 触发 UAC

### Changed
- `.gitignore` 新增 `/dist/`，忽略发布产物
- `Cargo.toml` 仓库地址更正为 `sxwzxc/FastPaste`，补 `build-dependencies = embed-resource` 与 `clipboard-win`
- `README.md` 新增、细化一键 Release 文档与 `release-info` 自动更新
- `AGENTS.md` / `docs/agents/*` / `CONTEXT.md` 引入，明确 Issue 跟踪与领域术语
- `src/*` 细节打磨：剪贴板敏感过滤、历史预览、粘贴备份恢复、热键解析、自启/托盘/权限诊断等与文档对齐

### Fixed
- `release.ps1` 修复 `app.manifest` 版本替换误命中 `<?xml version>` 的问题

## [0.1.0] - 2026-08-21

### Added
- 初始版本：跨平台静默剪贴板管理器
- 历史：容量 10 的环形队列，去重并置顶，空/纯空白忽略，单条 5MB 上限（`max_entry_bytes` 可配置，重载配置即时生效），非文本静默忽略；历史预览约 200ms 刷新，`digit: ` + 前 20 字且换行转空格，空则 `(空)` 且不可点
- 敏感内容过滤：Windows 检测常见 transient 格式（`ExcludeClipboardContentFromMonitorProcessing` / `CanIncludeInClipboardHistory` / `ClipboardViewerIgnore`），macOS 未实现则视为非敏感，配合 `ignore_regex` 正则过滤
- 全局热键：`Ctrl+Shift+1..0`（1 为最新）直接粘贴到焦点窗口，支持热键冲突部分可用提示；未知主键报错不再回退为 `Digit1`
- 粘贴策略：默认 `clipboard` 备份-粘贴-恢复（`preserve_clipboard=true`，支持文本与图片恢复），可切换 `type` 逐字击键，粘贴失败主线程对话框提示，并写入剪贴板
- 托盘：唯一常驻界面，承载启用/停用、历史预览（20字符脱敏）、打开配置、重载配置、自启开关、权限诊断、退出；启用/停用通过 `HotkeyManager::set_enabled` 切换
- 配置：`config.toml` 持久化于系统配置目录，支持热键自定义 `ctrl+shift+1` 格式，校验失败对话框提示；重载配置走 `AppState::reload_config` 统一路径
- 单实例：named mutex / file lock，第二实例退出并提示已在运行
- 自启：Windows Task Scheduler 登录触发最高权限（`HIGHEST`）任务 / macOS LaunchAgent，托盘一键切换，开启失败不写注册表，关闭时删除任务并清理注册表残留；默认 `autostart_elevated = true`
- 权限：`cargo build --release` / 发布 exe 嵌入 `requireAdministrator` manifest（Windows），启动由系统弹 UAC；`debug` 与 `cargo test` 嵌入 `asInvoker` 不强制 UAC，未提权时仍走 PowerShell try_elevate，拒绝则原进程继续运行；以管理员身份常驻会扩大剪贴板中敏感内容的暴露面，macOS 引导 Accessibility 授权并提示风险（辅助功能授权后可向其它窗口注入按键，请只在信任时开启）
- 轮询监听：500ms `arboard` 轮询，停用期复制不补录（停用时仍 `poll` 更新 `last_seen`），预留 `ClipboardWatcher` trait
- 引入版本号与更新日志（关于对话框与诊断均使用 `env!("CARGO_PKG_VERSION")`）

### ADR
- 0001 Rust + tray-icon 静默后台
- 0002 轮询式剪贴板监听
- 0003 enigo 备份-粘贴-恢复策略
- 0004 默认提权启动与 Task Scheduler 提权自启

