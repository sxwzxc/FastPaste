use anyhow::Result;
use std::{thread, time::Duration};

use crate::clipboard::Clipboard;
use crate::config::PasteMethod;

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
        #[cfg(not(target_os = "macos"))]
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
            // 尝试 clipboard 方式，失败则降级为 type
            match paster.paste_with_clipboard(text, clipboard, preserve) {
                Ok(_) => Ok(()),
                Err(e) => {
                    log::warn!("clipboard paste failed, fallback to type: {}", e);
                    paster.paste_text(text)
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
    use crate::clipboard::MockClipboard;

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
    fn do_paste_clipboard_fallback_to_type() {
        let mut paster = MockPaster { should_fail_clipboard: true, ..Default::default() };
        let mut cb = MockClipboard::with_text("old");
        do_paste(&mut paster, &mut cb, "hello", PasteMethod::Clipboard, true).unwrap();
        assert_eq!(paster.typed, vec!["hello"]);
    }

    #[test]
    fn do_paste_type_direct() {
        let mut paster = MockPaster::default();
        let mut cb = MockClipboard::default();
        do_paste(&mut paster, &mut cb, "hello", PasteMethod::Type, true).unwrap();
        assert_eq!(paster.typed, vec!["hello"]);
        assert!(paster.clipboard_pastes.is_empty());
    }
}
