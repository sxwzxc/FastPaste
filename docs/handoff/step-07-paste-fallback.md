# 当前步骤 07 — 粘贴失败路径对齐 ADR-0003

## 目标

`paste_method = clipboard` 失败时：把条目写入剪贴板，在**主线程**提示用户手动粘贴。不再降级为 `enigo.text()` 逐字击键。工作线程里不调用 `rfd` 对话框。

## 允许修改

- `src/paste.rs`
- `src/app.rs`
- `src/main.rs`

## 做法

1. `do_paste` 的 `PasteMethod::Clipboard` 分支：`paste_with_clipboard` 失败则 `clipboard.set_text(text)?` 然后 `return Err(e)`（保留原错误）。删除「fallback to type」调用。`PasteMethod::Type` 仍只走 `paste_text`。

2. 给 `AppState` 加 `#[derive(Clone)]` 和字段：

```rust
pub pending_notice: Arc<Mutex<Option<(String, String)>>>, // (title, body)
```

`AppState::new` 里初始化为 `None`。`spawn_clipboard_thread` / `handle_paste` / `main` 里凡是手写 `AppState { history, enabled, config }` 的地方改成 `state.clone()`，或补上 `pending_notice` 字段（漏了会编不过）。

3. `handle_paste`：粘贴失败时 `set_text` 已由 `do_paste` 做过。工作线程只做：

```rust
*state.pending_notice.lock() = Some((
    "FastPaste".into(),
    "粘贴失败，已写入剪贴板，请手动粘贴".into(),
));
```

删除工作线程里的 `dialog::show_warning`。

4. `src/main.rs` 的 `AboutToWait`：在刷新托盘之后，若 `pending_notice` 有值则 `take()` 出来，主线程调 `dialog::show_warning(title, body)`。你需要把 `pending_notice` 的 Arc 克隆进 event_loop 闭包（已有其它 Arc 克隆的方式照抄）。

5. 更新 `do_paste_clipboard_fallback_to_type`：改成断言 clipboard 失败后 `typed` 仍为空，且 `cb.text` 为要粘贴的字符串；`do_paste` 返回 Err。

## Done when

- [ ] clipboard 失败不再调用 `paste_text`
- [ ] 失败会 `set_text(条目)`
- [ ] `rfd` 只在主线程 event loop 里调用
- [ ] `AppState` 可 Clone，构造点都带上 `pending_notice`
- [ ] `cargo test` 通过
