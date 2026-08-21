# 当前步骤 12 — 敏感内容 transient

## 目标

被系统标为瞬态的剪贴板文本不进入历史（`CONTEXT.md` 敏感内容）。`Clipboard` 增加检测缝；轮询在入队前询问。Windows 检测常见「不要进剪贴板历史」格式。macOS 先识别 Concealed/Transient 类型名；拿不到 API 则返回 false，不要假装成功。

## 允许修改

- `src/clipboard.rs`
- `Cargo.toml`（仅当必须把已有传递依赖 `clipboard-win` 写成直接依赖）

## 做法

1. trait 增加：`fn is_transient(&mut self) -> bool { false }`（默认 false，Mock 可覆盖）。

2. `PollingWatcher::poll`：取出 `current` 后、更新 `last_seen` 之前，若 `self.clipboard.is_transient()` 则：仍然把 `last_seen = Some(current.clone())`（避免以后反复看到同一瞬态文本），然后 `return None`。这样瞬态既不入队，也不会在标记消失后把同一串当新复制补进去……等等，若 last_seen 设为瞬态文本，用户稍后**正常**再复制同一密码，也不会入队。更好：

   - 瞬态：`last_seen = Some(current.clone())` 且 `return None`。同一内容稍后作为非瞬态再出现时，`Some(&current) == last_seen` 会挡住。密码管理器通常复制后清空或改成别的。接受这个权衡，并在注释里写一句。

3. `MockClipboard` 增加 `pub transient: bool`，`is_transient` 返回它。

4. **Windows** `ArboardClipboard::is_transient`：打开剪贴板，枚举格式名。任一格式名等于下列之一则 true：

   - `ExcludeClipboardContentFromMonitorProcessing`
   - `CanIncludeInClipboardHistory`（若能读到该格式的数据且为 32-bit 0，也视为 transient；若枚举实现太绕，**仅按格式名是否存在**即可，并在注释写明）
   - `ClipboardViewerIgnore`

   优先用已在 `Cargo.lock` 里的 `clipboard-win`：在 `Cargo.toml` 对 Windows 加直接依赖 `clipboard-win`（版本与 lock 里主要那条接近，如 `"5"`）。用它的 raw API 枚举格式。打开/关闭剪贴板要配对。失败则 `false`。

5. **macOS**：若能用现有依赖读 NSPasteboard 类型，存在 `org.nspasteboard.TransientType` 或 `org.nspasteboard.ConcealedType` 则 true。否则 `is_transient` 保持 false，加 `log::debug!("macOS transient 检测未实现")` 一次即可（不要每次 poll 刷屏：用 `Once` 或只在 debug 且检测到 get_text 时偶尔打）。小模型若 30 分钟内做不完 macOS，允许 Windows 实现 + macOS 返回 false，并在 `clipboard.rs` 顶部用一行注释标明。

## 测试

- Mock `transient = true`、文本 `"pw"`：`PollingWatcher::poll` 第一次返回 None，且之后相同文本仍 None。
- `transient = false` 时行为与现在一致，`"hello"` 能 poll 出来。
- `ClipboardManager`：transient mock 不增加历史长度。

## Done when

- [ ] `Clipboard::is_transient` 存在
- [ ] watcher 对 transient 不向调用方返回文本（仍更新 last_seen）
- [ ] Windows 实现按格式名检测（或你在回复里写明编译期做不到的原因与降级）
- [ ] 测试覆盖 mock transient
- [ ] `cargo test` 通过
