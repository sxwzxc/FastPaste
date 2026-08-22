# 托盘图标显隐开关

新增 `show_tray_icon` 布尔配置（默认 `true`）控制托盘图标是否显示，`false` 时通过 `tray-icon` 的 `set_visible(false)` 隐藏图标但保持热键与剪贴板监听运行，需手改配置文件恢复。启动时 `Tray::new` 后按值 `set_visible`（`tray-icon` 无 `with_visible`，避免闪现已接受）；运行期在统一重载管线中对比旧值变更时调用。自动重载静默失败语义不变，拼写错误等整文件解析失败会保留显隐旧值直至手动重载。
