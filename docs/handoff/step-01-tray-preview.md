# 当前步骤 01 — 托盘历史预览

## 目标

托盘「历史预览」随历史变化更新：有条目则显示 `digit: ` + 前 20 字（换行变空格，超长 `...`），该项可点并触发粘贴；无条目则 `digit: (空)` 且不可点。

## 允许修改

- `src/tray.rs`
- `src/main.rs`

## 做法

1. 在 `src/tray.rs` 的 `refresh_history_preview` 里，对每个 `MenuItem` 调用 `set_text` 和 `set_enabled`。`muda::MenuItem` 有 `set_text(&str)`、`set_enabled(bool)`。有条目：`enabled = true`；无条目：`enabled = false`。删掉「无法更新标题」的注释和 `let _ = text`。

2. 在 `src/main.rs` 事件循环：把 `ControlFlow::Wait` 改成每隔 200ms 醒来一次，并在 `AboutToWait` 里调用 `tray_clone.refresh_history_preview()`。需要 `std::time::{Duration, Instant}`：

```rust
elwt.set_control_flow(ControlFlow::WaitUntil(
    Instant::now() + Duration::from_millis(200),
));
```

放在 `event_loop.run` 闭包开头（每次 event 都设置）。`AboutToWait` 分支里先 `refresh_history_preview()`，再处理菜单/热键。

3. `src/tray.rs` 里创建历史项时仍可用 `enabled: false` 作为初始值，刷新后会改。

## 测试

在 `src/tray.rs` 的 `#[cfg(test)]` 增加：用 `History::format_preview` 断言 20 字截断格式（已有类似测试则补一条带 digit 前缀的纯函数测试即可，例如抽一小段 `fn history_item_label(digit: u8, preview: Option<&str>) -> (String, bool)` 并测它）。不要为了测 GUI 去 `Tray::new()`。

## Done when

- [ ] `refresh_history_preview` 会 `set_text` / `set_enabled`
- [ ] 事件循环 `WaitUntil(200ms)` 且每次 `AboutToWait` 刷新预览
- [ ] `cargo test` 通过
- [ ] 未改其它文件
