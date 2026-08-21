# 当前步骤 02 — Windows 用虚拟键 V 做粘贴

## 目标

Windows 上模拟粘贴改为 `Control + Key::V`（VK_V），不再用 `Key::Unicode('v')`（enigo 会走扫码，快捷键不可靠）。

## 允许修改

- `src/paste.rs` 的 `key_paste` 函数体

## 做法

依赖步骤 01 已合并。只改 `key_paste`：

```rust
fn key_paste(&mut self) -> Result<()> {
    use enigo::{Direction, Key, Keyboard};
    #[cfg(target_os = "macos")]
    {
        self.enigo.key(Key::Meta, Direction::Press)?;
        self.enigo.key(Key::Unicode('v'), Direction::Click)?;
        self.enigo.key(Key::Meta, Direction::Release)?;
    }
    #[cfg(target_os = "windows")]
    {
        self.enigo.key(Key::Control, Direction::Press)?;
        self.enigo.key(Key::V, Direction::Click)?;
        self.enigo.key(Key::Control, Direction::Release)?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        self.enigo.key(Key::Control, Direction::Press)?;
        self.enigo.key(Key::Unicode('v'), Direction::Click)?;
        self.enigo.key(Key::Control, Direction::Release)?;
    }
    Ok(())
}
```

`Key::V` 在 enigo 0.2 仅 Windows 有（`keycodes.rs` 上 `#[cfg(target_os = "windows")]`）。不要在 macOS/Linux 分支写 `Key::V`。

不要改 50ms / 200ms。不要改 `do_paste` 失败降级。

## Done when

- [ ] Windows `key_paste` 使用 `Key::V`，源码中该函数不再对 Windows 使用 `Unicode('v')`
- [ ] macOS 仍是 Meta + Unicode('v')
- [ ] `cargo test` 通过
