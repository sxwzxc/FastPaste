# enigo 备份-粘贴-恢复的粘贴策略

热键触发的“粘贴”需直接注入焦点窗口。决定默认 `paste_method = "clipboard"`：`备份旧剪贴板 → set_text(条目) → 50ms 后 enigo 模拟 Ctrl+V/Cmd+V → 200ms 后恢复旧剪贴板`，由 `preserve_clipboard` 开关控制是否恢复。另提供 `paste_method = "type"` 以 `enigo.text()` 逐字击键实现零污染路径供极端场景切换，失败时降级为仅写剪贴板并托盘气泡提示。
