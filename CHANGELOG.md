# Changelog

所有显著变更将记录于此文件。格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，版本号遵循 [SemVer](https://semver.org/lang/zh-CN/)。

## [0.1.0] - 2026-08-21

### Added
- 初始版本：跨平台静默剪贴板管理器
- 历史：容量 10 的环形队列，去重并置顶，空/纯空白忽略，单条 5MB 上限，非文本静默忽略
- 敏感内容过滤：transient 标记与 `ignore_regex` 正则过滤
- 全局热键：`Ctrl+Shift+1..0`（1 为最新）直接粘贴到焦点窗口，支持热键冲突部分可用提示
- 粘贴策略：默认 `clipboard` 备份-粘贴-恢复（`preserve_clipboard=true`），可切换 `type` 逐字击键，失败降级为仅写剪贴板
- 托盘：唯一常驻界面，承载启用/停用、历史预览（20字符脱敏）、打开配置、重载配置、自启开关、权限诊断、退出
- 配置：`config.toml` 持久化于系统配置目录，支持热键自定义 `ctrl+shift+1` 格式，校验失败对话框提示
- 单实例：named mutex / file lock，第二实例唤醒已有实例
- 自启：Windows Task Scheduler 提权任务 / macOS LaunchAgent，托盘一键切换，默认关闭
- 权限：默认请求管理员权限启动（Windows `requireAdministrator`），macOS 引导 Accessibility 授权
- 轮询监听：500ms `arboard` 轮询，预留 `ClipboardWatcher` trait
- 引入版本号与更新日志

### ADR
- 0001 Rust + tray-icon 静默后台
- 0002 轮询式剪贴板监听
- 0003 enigo 备份-粘贴-恢复策略
- 0004 默认提权启动与 Task Scheduler 提权自启
