# 当前步骤 01 — 粘贴前等待热键松开

## 目标

在调用 enigo 模拟粘贴或逐字击键之前，等待物理修饰键与数字键松开，避免 `SendInput` 与仍按下的 `Ctrl+Shift+1` 叠成 Ctrl+Shift+V。

## 允许修改

- `src/paste.rs` 仅新增等待函数及其测试；本步不要改 `key_paste` 用的键（那是步骤 02）。

## 做法

1. 在 `src/paste.rs` 增加：

```rust
pub fn wait_for_hotkey_release(timeout: Duration) {
    #[cfg(windows)]
    {
        wait_for_hotkey_release_windows(timeout);
    }
    #[cfg(not(windows))]
    {
        // 无 GetAsyncKeyState 时退化为短睡眠，覆盖松键时间
        let cap = timeout.min(Duration::from_millis(200));
        thread::sleep(cap);
    }
}

#[cfg(windows)]
fn wait_for_hotkey_release_windows(timeout: Duration) {
    // GetAsyncKeyState：最高位为 1 表示当前按下
    extern "system" {
        fn GetAsyncKeyState(vkey: i32) -> i16;
    }
    const VK_SHIFT: i32 = 0x10;
    const VK_CONTROL: i32 = 0x11;
    const VK_MENU: i32 = 0x12; // Alt
    const VK_LWIN: i32 = 0x5B;
    const VK_RWIN: i32 = 0x5C;
    fn down(vk: i32) -> bool {
        unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 }
    }
    let start = std::time::Instant::now();
    loop {
        let digit_down = (0x30..=0x39).any(down); // '0'..'9'
        let busy = down(VK_SHIFT)
            || down(VK_CONTROL)
            || down(VK_MENU)
            || down(VK_LWIN)
            || down(VK_RWIN)
            || digit_down;
        if !busy {
            thread::sleep(Duration::from_millis(20));
            return;
        }
        if start.elapsed() >= timeout {
            log::warn!("等待热键松开超时 {:?}，仍尝试粘贴", timeout);
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}
```

超时默认 **800ms**。

2. 在 `paste_with_clipboard` 里，`clipboard.set_text` **之前**调用 `wait_for_hotkey_release(Duration::from_millis(800))`。必须在写剪贴板前等：若先写再等，用户松键慢时剪贴板已变，但叠键问题仍在；先等再写顺序也对，且松键后再注入。

   推荐顺序：等待松开 → `set_text` → 50ms → `key_paste` → 200ms → restore。把 wait 放在函数开头（backup 之后或之前均可；推荐 backup 之后、set_text 之前）。

3. `Paster::paste_text`（type 路径）开头同样 `wait_for_hotkey_release(800ms)`。

## 测试

- 增加 `wait_for_hotkey_release(Duration::from_millis(0))` 或 1ms：必须立即返回，不永久阻塞。Windows 上若此刻没有键按下，应马上返回。
- 不要在测试里依赖真实按着的键。

## Done when

- [ ] `wait_for_hotkey_release` 存在；Windows 用 `GetAsyncKeyState`，其它平台短 sleep
- [ ] `paste_with_clipboard` 与 `paste_text` 都会等待
- [ ] 超时不会卡住线程（有 timeout 分支）
- [ ] `cargo test` 通过
- [ ] 未改 `key_paste` 的按键种类
