# 当前步骤 03 — 配置 max_entry_bytes 生效

## 目标

`config.toml` 的 `max_entry_bytes` 真正限制条目字节上限。重载配置后新上限立刻生效。默认仍是 5MB。现有 `History::new(ignore)` 调用保持能编译。

## 允许修改

- `src/history.rs`
- `src/app.rs`
- `src/main.rs`（仅重载配置成功分支里给 History 设置新上限）

## 做法

1. `History` 增加字段 `max_bytes: usize`。`MAX_ENTRY_BYTES` 常量保留作默认值。

2. 增加而不替换构造：

```rust
pub fn new(ignore_pattern: Option<String>) -> Self {
    Self::with_limits(ignore_pattern, MAX_ENTRY_BYTES)
}

pub fn with_limits(ignore_pattern: Option<String>, max_bytes: usize) -> Self { /* ... */ }

pub fn set_max_bytes(&mut self, max_bytes: usize) {
    self.max_bytes = max_bytes.max(1);
}
```

`push` 里用 `self.max_bytes` 替代 `MAX_ENTRY_BYTES`。`with_regex` 同样给 `max_bytes: MAX_ENTRY_BYTES`。

3. `AppState::new`：`History::with_limits(ignore, config.max_entry_bytes)`。

4. `AppState::reload_config` 在 `set_ignore_regex` 旁调用 `h.set_max_bytes(new_cfg.max_entry_bytes)`。

5. `src/main.rs` 重载配置成功分支里，更新 `ignore_regex` 的同一把 `history` 锁内调用 `set_max_bytes(new_cfg.max_entry_bytes)`。

## 测试（history.rs）

- 默认 `new(None)`：长度为 `MAX_ENTRY_BYTES + 1` 的字符串不入队。
- `with_limits(None, 8)`：`"123456789"` 不入队，`"12345678"` 入队。
- `set_max_bytes(3)` 之后 `"abcd"` 不入队。

## Done when

- [ ] 入队上限来自 `self.max_bytes`，默认 5MB
- [ ] 启动与重载配置都会把 `config.max_entry_bytes` 写进 History
- [ ] 上述测试存在且 `cargo test` 通过
