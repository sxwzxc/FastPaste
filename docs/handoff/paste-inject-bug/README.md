# Ctrl+Shift+1 只改剪贴板、不注入焦点窗口

## 症状（与代码对齐）

热键 `Ctrl+Shift+1` 触发后，系统剪贴板会出现（或保持）该历史条目，焦点窗口的输入框没有被粘贴。`cargo test` 全绿，因为现有测试用 Mock，从不调用真实 `enigo`/`SendInput`。

这不是「历史没入队」。`handle_paste` 能取到条目，并且 `clipboard.set_text(条目)` 已执行。断在 **模拟 Ctrl+V 没有变成目标窗口的粘贴**。

## 调用链

1. `global-hotkey` 收到 `ctrl+shift+1`（Windows 上 `RegisterHotKey` 会吃掉这次按键，不发给目标窗口，但物理键仍按下，直到用户松手）。
2. 事件循环调用 `handle_paste` → **立刻** `thread::spawn`。
3. 工作线程：`do_paste` → `paste_with_clipboard`：备份 → `set_text(条目)` → sleep 50ms → `key_paste` → sleep 200ms → 若 `preserve_clipboard` 则恢复备份。
4. `key_paste`（Windows）今天是：

```text
Key::Control Press
Key::Unicode('v') Click
Key::Control Release
```

默认 `preserve_clipboard = true`，且 digit `1` 就是最新复制。此时备份 ≈ 条目本身，剪贴板看起来「只是之前复制的内容」，更容易误判成「只写了剪贴板」。

## 根因（按可能性）

### H1 — 主因：热键修饰键还按着，SendInput 与物理键盘叠在一起

Win32 `SendInput` 文档原话：函数**不会**重置当前键盘状态；已经按下的键会干扰随后注入的事件。

用户按住 `Ctrl+Shift+1` 时工作线程已经 `Control+V`。物理 Shift（以及往往还按着的 Ctrl）仍在，目标进程收到的是 **Ctrl+Shift+V**，多数输入框不会当粘贴（记事本里经常什么都不插入）。

enigo 仍返回 `Ok`，所以没有「粘贴失败」对话框。这与「静默：剪贴板变了/没变、框里没字」一致。

### H2 — 次因：Windows 上 `Key::Unicode('v')` 走扫码而不是 `VK_V`

enigo 0.2.1 `keycodes.rs`：`Key::Unicode(_) => Err("Unicode must be entered via scancodes")`。`win_impl.rs` 对 Unicode 走 `get_scancode` + `KEYEVENTF_SCANCODE`，不是 `VK_V`。快捷键更稳的是虚拟键 `Key::V`。

官方 README 示例虽写 Unicode+Control，那是跨平台示意；Windows 快捷键应使用 `Key::V`。

### H3 — 次因：UIPI（只影响「往更高完整性窗口粘」）

debug/`cargo run` 嵌入 `asInvoker`。向**已提权**窗口 `SendInput` 会被 UIPI 静默丢掉。若复现在普通记事本里也失败，则不是主因；若只在管理员窗口失败，release 提权或「以管理员身份重启」才是对症。

### H4 — 加重：50ms/200ms 在未等松键时几乎无用

等的是剪贴板写入，不是热键松开。松键等待补上之后，这两段 sleep 可以保留。

## 为何测试看不出来

`MockPaster` 把 `paste_with_clipboard` 记成一次成功调用，不接触 `SendInput`。正确回归缝是：抽「等修饰键松开」为可测函数；真实按键只能人工（记事本 + Ctrl+Shift+1）。

## 修复原则（不要做的）

不要改热键绑定来「绕过」这个问题。不要把默认粘贴改成只写剪贴板。不要为了测 GUI 去 `Tray::new()`。

## 修复要做的

1. 在模拟粘贴**之前**等待 Ctrl/Shift/Alt/Win 以及数字键 0–9 松开（有超时）。
2. Windows 的粘贴组合键改成 `Key::Control` + `Key::V`（虚拟键），不要用 `Key::Unicode('v')`。
3. `paste_method = type` 的 `enigo.text()` 前同样等待松键，否则会打出 `V` 或别的组合。
4. 人工验收：焦点在普通记事本，Ctrl+Shift+1 应把最新条目插入光标处。

## 顺序（不要跳）

| 步 | 文件 | 做什么 |
|---|---|---|
| 01 | `step-01-wait-release.md` | 粘贴前等待 Ctrl/Shift 松开 |
| 02 | `step-02-vk-v.md` | Windows 用 `Key::V` |
| 03 | `step-03-docs.md` | README / CHANGELOG |

每轮用户消息 = `GLOBAL.md` + 当前 step。通过后再贴 `VERIFY.md`。

人工验收（步骤 02 之后由你做，不要让模型假装测过 GUI）：记事本输入框聚焦，复制一段字，按 Ctrl+Shift+1，字应出现在光标处；松开热键前不应要求你已经松手——程序会等。若只在「以管理员运行的窗口」失败，那是 UIPI，用 release 构建或托盘「以管理员身份重启」。

