use anyhow::Result;
use std::{thread, time::Duration};

use crate::clipboard::Clipboard;
use crate::config::PasteMethod;

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

/// 粘贴器 trait，便于测试
pub trait Paster: Send {
    fn paste_text(&mut self, text: &str) -> Result<()>;
    fn paste_with_clipboard(&mut self, text: &str, clipboard: &mut dyn Clipboard, preserve: bool) -> Result<()>;
}

/// 基于 enigo 的真实粘贴实现
pub struct EnigoPaster {
    enigo: enigo::Enigo,
}

impl EnigoPaster {
    pub fn new() -> Self {
        let settings = enigo::Settings::default();
        let enigo = enigo::Enigo::new(&settings).expect("初始化 enigo 失败");
        Self { enigo }
    }

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
}

impl Paster for EnigoPaster {
    fn paste_text(&mut self, text: &str) -> Result<()> {
        wait_for_hotkey_release(Duration::from_millis(800));
        use enigo::Keyboard;
        self.enigo.text(text)?;
        Ok(())
    }

    fn paste_with_clipboard(&mut self, text: &str, clipboard: &mut dyn Clipboard, preserve: bool) -> Result<()> {
        let backup = if preserve {
            clipboard.get_backup()
        } else {
            None
        };

        wait_for_hotkey_release(Duration::from_millis(800));
        clipboard.set_text(text)?;
        // 等待剪贴板写入生效
        thread::sleep(Duration::from_millis(50));
        self.key_paste()?;
        // 等待目标应用读取
        thread::sleep(Duration::from_millis(200));

        if let Some(bk) = backup {
            let _ = clipboard.restore(bk);
        }
        Ok(())
    }
}

/// 根据配置执行粘贴
pub fn do_paste(
    paster: &mut dyn Paster,
    clipboard: &mut dyn Clipboard,
    text: &str,
    method: PasteMethod,
    preserve: bool,
) -> Result<()> {
    match method {
        PasteMethod::Clipboard => {
            match paster.paste_with_clipboard(text, clipboard, preserve) {
                Ok(_) => Ok(()),
                Err(e) => {
                    // 失败时尝试写入剪贴板以便手动粘贴；若写入也失败则返回可区分错误
                    if let Err(we) = clipboard.set_text(text) {
                        return Err(we.context("clipboard_write_failed").into());
                    }
                    Err(e)
                }
            }
        }
        PasteMethod::Type => paster.paste_text(text),
    }
}

/// Mock paster 用于测试
#[derive(Debug, Default)]
pub struct MockPaster {
    pub typed: Vec<String>,
    pub clipboard_pastes: Vec<(String, bool)>,
    pub should_fail_clipboard: bool,
}

impl Paster for MockPaster {
    fn paste_text(&mut self, text: &str) -> Result<()> {
        self.typed.push(text.to_string());
        Ok(())
    }
    fn paste_with_clipboard(&mut self, text: &str, clipboard: &mut dyn Clipboard, preserve: bool) -> Result<()> {
        if self.should_fail_clipboard {
            anyhow::bail!("mock clipboard paste fail");
        }
        let backup = if preserve { clipboard.get_backup() } else { None };
        clipboard.set_text(text)?;
        self.clipboard_pastes.push((text.to_string(), preserve));
        if let Some(bk) = backup {
            let _ = clipboard.restore(bk);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::{Clipboard, ClipboardBackup, MockClipboard};

    #[test]
    fn clipboard_preserve_restores() {
        let mut paster = MockPaster::default();
        let mut cb = MockClipboard::with_text("old");
        paster.paste_with_clipboard("new", &mut cb, true).unwrap();
        assert_eq!(cb.text, Some("old".into()));
        assert_eq!(paster.clipboard_pastes.len(), 1);
    }

    #[test]
    fn clipboard_no_preserve_overwrites() {
        let mut paster = MockPaster::default();
        let mut cb = MockClipboard::with_text("old");
        paster.paste_with_clipboard("new", &mut cb, false).unwrap();
        assert_eq!(cb.text, Some("new".into()));
    }

    #[test]
    fn clipboard_preserve_restores_image() {
        let mut paster = MockPaster::default();
        let mut cb = MockClipboard::with_image(1, 1, vec![1, 2, 3, 4]);
        assert!(cb.text.is_none());
        assert_eq!(cb.image, Some((1, 1, vec![1, 2, 3, 4])));
        paster.paste_with_clipboard("new", &mut cb, true).unwrap();
        // 保留剪贴板时，图片应被恢复，文本不应残留
        assert_eq!(cb.text, None);
        assert_eq!(cb.image, Some((1, 1, vec![1, 2, 3, 4])));
        assert_eq!(paster.clipboard_pastes.len(), 1);
    }

    #[test]
    fn do_paste_clipboard_preserve_image() {
        let mut paster = MockPaster::default();
        let mut cb = MockClipboard::with_image(1, 1, vec![5, 6, 7, 8]);
        do_paste(&mut paster, &mut cb, "hello", PasteMethod::Clipboard, true).unwrap();
        assert_eq!(paster.clipboard_pastes.len(), 1);
        assert_eq!(cb.image, Some((1, 1, vec![5, 6, 7, 8])));
        assert!(cb.text.is_none());
        assert!(paster.typed.is_empty());
    }

    #[test]
    fn do_paste_clipboard_fail_writes_text_not_type() {
        let mut paster = MockPaster { should_fail_clipboard: true, ..Default::default() };
        let mut cb = MockClipboard::with_text("old");
        let res = do_paste(&mut paster, &mut cb, "hello", PasteMethod::Clipboard, true);
        assert!(res.is_err());
        assert!(paster.typed.is_empty());
        assert_eq!(cb.text, Some("hello".into()));
    }

    #[test]
    fn do_paste_type_direct() {
        let mut paster = MockPaster::default();
        let mut cb = MockClipboard::default();
        do_paste(&mut paster, &mut cb, "hello", PasteMethod::Type, true).unwrap();
        assert_eq!(paster.typed, vec!["hello"]);
        assert!(paster.clipboard_pastes.is_empty());
    }

    // 辅助：可让 set_text 失败的 Clipboard
    struct FailingMockClipboard {
        inner: MockClipboard,
        fail_set: bool,
    }
    impl Clipboard for FailingMockClipboard {
        fn get_text(&mut self) -> Option<String> {
            self.inner.get_text()
        }
        fn set_text(&mut self, text: &str) -> anyhow::Result<()> {
            if self.fail_set {
                anyhow::bail!("mock set_text fail");
            }
            self.inner.set_text(text)
        }
        fn get_backup(&mut self) -> Option<ClipboardBackup> {
            self.inner.get_backup()
        }
        fn restore(&mut self, backup: ClipboardBackup) -> anyhow::Result<()> {
            self.inner.restore(backup)
        }
        fn is_transient(&mut self) -> bool {
            self.inner.is_transient()
        }
    }

    #[test]
    fn do_paste_clipboard_write_fail_distinguishes() {
        // set_text 成功：错误不含 clipboard_write_failed，且已写入
        let mut paster = MockPaster {
            should_fail_clipboard: true,
            ..Default::default()
        };
        let mut cb_ok = MockClipboard::with_text("old");
        let res_ok = do_paste(&mut paster, &mut cb_ok, "hello", PasteMethod::Clipboard, true);
        assert!(res_ok.is_err());
        let err_str = format!("{:?}", res_ok.unwrap_err());
        assert!(
            !err_str.contains("clipboard_write_failed"),
            "set_text 成功时不应含 clipboard_write_failed，实际: {}",
            err_str
        );
        assert_eq!(cb_ok.text, Some("hello".into()));

        // set_text 也失败：错误含 clipboard_write_failed
        let mut paster2 = MockPaster {
            should_fail_clipboard: true,
            ..Default::default()
        };
        let inner = MockClipboard::with_text("old");
        let mut cb_fail = FailingMockClipboard {
            inner,
            fail_set: true,
        };
        let res_fail = do_paste(&mut paster2, &mut cb_fail, "hello", PasteMethod::Clipboard, true);
        assert!(res_fail.is_err());
        let err_str2 = format!("{:?}", res_fail.unwrap_err());
        assert!(
            err_str2.contains("clipboard_write_failed"),
            "set_text 失败时应含 clipboard_write_failed，实际: {}",
            err_str2
        );
    }

    #[test]
    fn wait_for_hotkey_release_zero_returns_quickly() {
        let start = std::time::Instant::now();
        wait_for_hotkey_release(Duration::from_millis(0));
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "wait_for_hotkey_release(0) should return quickly"
        );
    }

    #[test]
    fn wait_for_hotkey_release_one_ms_returns_quickly() {
        let start = std::time::Instant::now();
        wait_for_hotkey_release(Duration::from_millis(1));
        assert!(
            start.elapsed() < Duration::from_millis(500),
            "wait_for_hotkey_release(1ms) should return quickly"
        );
    }
}
