# 当前步骤 05 — 停用期复制不补录

## 目标

停用状态下复制的内容不进入历史。再启用后，不会因为「停用期间剪贴板已变」而自动入队；只有启用后再发生一次新的复制才入队。

## 允许修改

- `src/clipboard.rs`

## 原因

`ClipboardManager::tick` 在 `!*enabled` 时直接 `return false`，不调用 `poll()`。`last_seen` 停在停用前的文本。再启用时 `poll()` 看到当前剪贴板不同，会把停用期末条补进历史。这违反 `CONTEXT.md`「停用期复制不补录」。

## 做法

`tick` 改为：**无论启用与否都 `poll()`**（更新 `last_seen`）。仅当启用且 `poll()` 得到新文本时才 `history.push`。

```rust
pub fn tick(&mut self) -> bool {
    let Some(text) = self.watcher.poll() else {
        return false;
    };
    if !*self.enabled.lock() {
        return false;
    }
    self.history.lock().push(text)
}
```

## 测试

改/补 `manager_tick_respects_enabled_and_history`：

1. 启用，入队 `"hello"`。
2. 停用，把 mock 文本改成 `"secret-during-disable"`，调用 `tick()`，返回 false，历史长度仍为 1。
3. 再启用，**不改 mock 文本**，再 `tick()`：返回 false，历史仍只有 `"hello"`（不补录）。
4. 启用下把文本改成 `"after-enable"`，`tick()` 为 true，历史长度为 2。

## Done when

- [ ] 停用时仍 `poll()` 以更新 `last_seen`
- [ ] 停用时不 `push`
- [ ] 上述测试覆盖「再启用不补录」
- [ ] `cargo test` 通过
