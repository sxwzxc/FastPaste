# 默认提权启动与 Task Scheduler 提权自启

为解决 Windows 上 `enigo` 向管理员窗口模拟按键被 UAC 隔离的问题，决定：1) 主程序默认以管理员身份启动（Windows 通过 `requireAdministrator` manifest / macOS 接受复杂性），2) 托盘“自启”勾选时不写入注册表 `Run`，而由安装/首次授权时创建 Task Scheduler 提权任务（最高权限、登录触发），实现后续开机无感提权。

## Considered Options
- 始终普通权限 + 失败降级：最安全但管理员窗口无法一键粘贴。
- 双进程（用户托盘 + Privileged Helper / Service）：可兼顾安全与提权，但剪贴板与输入模拟必须在用户会话内，Service 隔离导致需 IPC，复杂度高。
- 安装时一次授权创建提权任务：兼顾“无感”与“仍在用户会话”，符合现有软件惯例。

## Consequences
- 便携单 exe 模式被打破，需提供安装器在首次创建任务时弹一次 UAC；绿色免安装运行仍会每次启动弹 UAC。
- macOS 提权对 `Accessibility` 无帮助，仍需用户手动在系统设置中授权，且 `LaunchDaemon` 无法访问用户 Pasteboard，实际仍依赖 `LaunchAgent` + 辅助功能授权。
- 以高权限常驻提升敏感数据风险，需在权限诊断中明确提示。
