# Rust + tray-icon 静默后台

FastPaste 需跨平台（Windows + macOS）、常驻后台、占用小、无主窗口多交互。决定采用 Rust + `tray-icon` + `global-hotkey` 构建单一可执行文件，常驻托盘为唯一界面，必要信息仅通过系统原生对话框呈现，不创建控制台或主窗口。

## Considered Options
- Tauri/Electron：自带 GUI 与 WebView，体积与内存远超“占用小”目标。
- 纯平台原生（Win32/Cocoa）：性能最优但双平台维护成本高。
- Rust `tray-icon` 生态：体积小、跨平台抽象成熟，与 `arboard`/`enigo` 适配好。

## Consequences
- 需自行处理 Windows `windows_subsystem = "windows"` 隐藏控制台与 macOS `LSUIElement` 无 Dock 图标。
- 配置与历史预览等交互受限于托盘菜单与原生对话框，复杂设置需走 `config.toml`。
