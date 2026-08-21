# 轮询式剪贴板监听

首版采用 `arboard` 每 500ms 轮询 `get_text()` 检测变化，仅文本差异才入队，CPU <0.1%。预留 `ClipboardWatcher` trait 以便后续无缝替换为系统事件驱动（Windows `WM_CLIPBOARDUPDATE` / macOS `NSPasteboardDidChange`），首版规避隐藏窗口与 RunLoop 的平台坑。
