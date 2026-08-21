use anyhow::Result;
use std::{thread, time::Duration};

use crate::clipboard::Clipboard;
use crate::config::PasteMethod;

pub const AUTO_INJECT_MAX_CHARS: usize = 1000;

pub fn has_c0_control(text: &str) -> bool {
    text.chars().any(|c| c < '\u{20}')
}

pub fn prefer_key_inject(text: &str) -> bool {
    text.chars().count() <= AUTO_INJECT_MAX_CHARS && !has_c0_control(text)
}

pub fn resolve_paste_method(configured: PasteMethod, text: &str) -> PasteMethod {
    match configured {
        PasteMethod::Auto => {
            if prefer_key_inject(text) {
                PasteMethod::Type
            } else {
                PasteMethod::Clipboard
            }
        }
        other => other,
    }
}

pub fn utf16_code_units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeChunk<'a> {
    Text(&'a str),
    Return,
    Tab,
}

pub fn type_chunks(text: &str) -> Result<Vec<TypeChunk<'_>>> {
    if text.contains('\0') {
        anyhow::bail!("文本包含空字节");
    }
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    while i < chars.len() {
        let (byte_idx, c) = chars[i];
        match c {
            '\r' => {
                if i + 1 < chars.len() && chars[i + 1].1 == '\n' {
                    if byte_idx > start {
                        chunks.push(TypeChunk::Text(&text[start..byte_idx]));
                    }
                    chunks.push(TypeChunk::Return);
                    let next_byte_idx = chars[i + 1].0;
                    let after = next_byte_idx + chars[i + 1].1.len_utf8();
                    start = after;
                    i += 2;
                } else {
                    if byte_idx > start {
                        chunks.push(TypeChunk::Text(&text[start..byte_idx]));
                    }
                    chunks.push(TypeChunk::Return);
                    start = byte_idx + c.len_utf8();
                    i += 1;
                }
            }
            '\n' => {
                if byte_idx > start {
                    chunks.push(TypeChunk::Text(&text[start..byte_idx]));
                }
                chunks.push(TypeChunk::Return);
                start = byte_idx + c.len_utf8();
                i += 1;
            }
            '\t' => {
                if byte_idx > start {
                    chunks.push(TypeChunk::Text(&text[start..byte_idx]));
                }
                chunks.push(TypeChunk::Tab);
                start = byte_idx + c.len_utf8();
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    if start < text.len() {
        chunks.push(TypeChunk::Text(&text[start..]));
    }
    Ok(chunks)
}

const PRE_PASTE_DELAY: Duration = Duration::from_millis(100);
const PASTE_CHORD_DELAY: Duration = Duration::from_millis(10);
const RESTORE_CLIPBOARD_DELAY: Duration = Duration::from_millis(300);

pub fn wait_for_hotkey_release(timeout: Duration) {
    wait_for_digit_release(timeout);
}

pub fn wait_for_digit_release(timeout: Duration) {
    #[cfg(windows)]
    {
        wait_for_digit_release_windows(timeout);
    }
    #[cfg(not(windows))]
    {
        // 无 GetAsyncKeyState 时退化为短睡眠，覆盖松键时间
        let cap = timeout.min(Duration::from_millis(200));
        thread::sleep(cap);
    }
}

#[cfg(windows)]
fn wait_for_digit_release_windows(timeout: Duration) {
    // GetAsyncKeyState：最高位为 1 表示当前按下，只等数字键 0-9
    extern "system" {
        fn GetAsyncKeyState(vkey: i32) -> i16;
    }
    fn down(vk: i32) -> bool {
        unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 }
    }
    let start = std::time::Instant::now();
    loop {
        let digit_down = (0x30..=0x39).any(down);
        if !digit_down {
            thread::sleep(Duration::from_millis(20));
            return;
        }
        if start.elapsed() >= timeout {
            log::warn!("等待数字键松开超时 {:?}，仍尝试粘贴", timeout);
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

// Win32 KEYEVENTF_EXTENDEDKEY / KEYEVENTF_KEYUP（与 windows 0.56 常量一致）
const KEYEVENTF_EXTENDEDKEY_BIT: u32 = 0x0001;
const KEYEVENTF_KEYUP_BIT: u32 = 0x0002;
const VK_LSHIFT: i32 = 0xA0;
const VK_RSHIFT: i32 = 0xA1;
const VK_LCONTROL: i32 = 0xA2;
const VK_RCONTROL: i32 = 0xA3;
const VK_LMENU: i32 = 0xA4;
const VK_RMENU: i32 = 0xA5;
const VK_LWIN: i32 = 0x5B;
const VK_RWIN: i32 = 0x5C;

/// 伪造修饰键 KEYUP 的 dwFlags：右 Ctrl/Alt/Win 需带 EXTENDEDKEY；左右 Shift 与左修饰键不要加。
fn modifier_keyup_flags(vk: i32) -> u32 {
    if matches!(vk, VK_RCONTROL | VK_RMENU | VK_RWIN) {
        KEYEVENTF_KEYUP_BIT | KEYEVENTF_EXTENDEDKEY_BIT
    } else {
        KEYEVENTF_KEYUP_BIT
    }
}

#[cfg(windows)]
fn release_held_modifiers_windows() -> Result<()> {
    extern "system" {
        fn GetAsyncKeyState(vkey: i32) -> i16;
    }
    fn is_down(vk: i32) -> bool {
        unsafe { GetAsyncKeyState(vk) as u16 & 0x8000 != 0 }
    }
    // LSHIFT, RSHIFT, LCONTROL, RCONTROL, LMENU, RMENU, LWIN, RWIN
    const VKS: &[i32] = &[
        VK_LSHIFT, VK_RSHIFT, VK_LCONTROL, VK_RCONTROL, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN,
    ];
    let downs: Vec<i32> = VKS.iter().copied().filter(|&vk| is_down(vk)).collect();
    if downs.is_empty() {
        return Ok(());
    }
    log::debug!("抬起仍按下的修饰键 {:?}（仅 KEYUP，不恢复）", downs);
    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, VIRTUAL_KEY,
    };
    let mut inputs: Vec<INPUT> = Vec::with_capacity(downs.len());
    for vk in downs {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk as u16),
                    wScan: 0,
                    dwFlags: KEYBD_EVENT_FLAGS(modifier_keyup_flags(vk)),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }
    let input_size = size_of::<INPUT>() as i32;
    let sent = unsafe { SendInput(&inputs, input_size) };
    if sent as usize != inputs.len() {
        let last_err = std::io::Error::last_os_error();
        anyhow::bail!(
            "抬起修饰键时 SendInput 失败：只送出 {}/{}，错误={}",
            sent,
            inputs.len(),
            last_err
        );
    }
    Ok(())
}

#[cfg(windows)]
fn send_unicode_windows(text: &str) -> Result<()> {
    if text.contains('\0') {
        anyhow::bail!("文本包含空字节");
    }
    if text.is_empty() {
        return Ok(());
    }
    let units = utf16_code_units(text);
    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE,
        VIRTUAL_KEY,
    };
    let mut inputs: Vec<INPUT> = Vec::with_capacity(units.len() * 2);
    for unit in units {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: unit,
                    dwFlags: KEYEVENTF_UNICODE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: unit,
                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        });
    }
    let input_size = size_of::<INPUT>() as i32;
    let sent = unsafe { SendInput(&inputs, input_size) };
    if sent as usize != inputs.len() {
        let last_err = std::io::Error::last_os_error();
        anyhow::bail!(
            "发送 Unicode 时 SendInput 失败：只送出 {}/{}，错误={}",
            sent,
            inputs.len(),
            last_err
        );
    }
    Ok(())
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
            thread::sleep(PASTE_CHORD_DELAY);
            self.enigo.key(Key::Unicode('v'), Direction::Click)?;
            thread::sleep(PASTE_CHORD_DELAY);
            self.enigo.key(Key::Meta, Direction::Release)?;
        }
        #[cfg(target_os = "windows")]
        {
            self.enigo.key(Key::Control, Direction::Press)?;
            thread::sleep(PASTE_CHORD_DELAY);
            self.enigo.key(Key::V, Direction::Click)?;
            thread::sleep(PASTE_CHORD_DELAY);
            self.enigo.key(Key::Control, Direction::Release)?;
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            self.enigo.key(Key::Control, Direction::Press)?;
            thread::sleep(PASTE_CHORD_DELAY);
            self.enigo.key(Key::Unicode('v'), Direction::Click)?;
            thread::sleep(PASTE_CHORD_DELAY);
            self.enigo.key(Key::Control, Direction::Release)?;
        }
        Ok(())
    }
}

impl Paster for EnigoPaster {
    fn paste_text(&mut self, text: &str) -> Result<()> {
        wait_for_hotkey_release(Duration::from_millis(800));
        if text.contains('\0') {
            anyhow::bail!("文本包含空字节");
        }
        if text.is_empty() {
            return Ok(());
        }
        let chunks = type_chunks(text)?;
        #[cfg(windows)]
        let mut released = false;
        for chunk in chunks {
            match chunk {
                TypeChunk::Text(t) => {
                    if t.is_empty() {
                        continue;
                    }
                    #[cfg(windows)]
                    {
                        send_unicode_windows(t)?;
                    }
                    #[cfg(not(windows))]
                    {
                        use enigo::Keyboard;
                        self.enigo.text(t)?;
                    }
                }
                TypeChunk::Return => {
                    #[cfg(windows)]
                    {
                        if !released {
                            release_held_modifiers_windows()?;
                            released = true;
                        }
                    }
                    use enigo::{Direction, Key, Keyboard};
                    self.enigo.key(Key::Return, Direction::Click)?;
                }
                TypeChunk::Tab => {
                    #[cfg(windows)]
                    {
                        if !released {
                            release_held_modifiers_windows()?;
                            released = true;
                        }
                    }
                    use enigo::{Direction, Key, Keyboard};
                    self.enigo.key(Key::Tab, Direction::Click)?;
                }
            }
        }
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
        thread::sleep(PRE_PASTE_DELAY);
        #[cfg(windows)]
        {
            release_held_modifiers_windows()?;
        }
        self.key_paste()?;
        thread::sleep(RESTORE_CLIPBOARD_DELAY);

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
    let method = resolve_paste_method(method, text);
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
        PasteMethod::Auto => unreachable!("resolve should have eliminated Auto"),
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
        if text.contains('\0') {
            anyhow::bail!("文本包含空字节");
        }
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

    // --- 新增 Auto 解析测试 ---
    #[test]
    fn prefer_key_inject_short_true() {
        assert!(prefer_key_inject("hello"));
    }

    #[test]
    fn prefer_key_inject_boundary() {
        let s1000 = "a".repeat(1000);
        assert!(prefer_key_inject(&s1000));
        let s1001 = "a".repeat(1001);
        assert!(!prefer_key_inject(&s1001));
    }

    #[test]
    fn prefer_key_inject_c0_false() {
        assert!(!prefer_key_inject("a\nb"));
        assert!(!prefer_key_inject("a\tb"));
        assert!(!prefer_key_inject("a\rb"));
        assert!(!prefer_key_inject("a\0b"));
    }

    #[test]
    fn resolve_auto_short_is_type() {
        assert_eq!(resolve_paste_method(PasteMethod::Auto, "hi"), PasteMethod::Type);
    }

    #[test]
    fn resolve_auto_newline_is_clipboard() {
        assert_eq!(resolve_paste_method(PasteMethod::Auto, "a\nb"), PasteMethod::Clipboard);
    }

    #[test]
    fn resolve_type_hard_override() {
        assert_eq!(resolve_paste_method(PasteMethod::Type, "a\nb"), PasteMethod::Type);
    }

    #[test]
    fn do_paste_auto_short_goes_typed() {
        let mut paster = MockPaster::default();
        let mut cb = MockClipboard::default();
        do_paste(&mut paster, &mut cb, "hi", PasteMethod::Auto, true).unwrap();
        assert_eq!(paster.typed, vec!["hi"]);
        assert!(paster.clipboard_pastes.is_empty());
    }

    #[test]
    fn do_paste_auto_newline_goes_clipboard() {
        let mut paster = MockPaster::default();
        let mut cb = MockClipboard::default();
        do_paste(&mut paster, &mut cb, "a\nb", PasteMethod::Auto, true).unwrap();
        assert_eq!(paster.clipboard_pastes.len(), 1);
        assert!(paster.typed.is_empty());
    }

    #[test]
    fn do_paste_type_with_newline_still_typed() {
        let mut paster = MockPaster::default();
        let mut cb = MockClipboard::default();
        do_paste(&mut paster, &mut cb, "a\nb", PasteMethod::Type, true).unwrap();
        assert_eq!(paster.typed, vec!["a\nb"]);
        assert!(paster.clipboard_pastes.is_empty());
    }

    // --- Windows Unicode ---
    #[test]
    fn utf16_ascii() {
        assert_eq!(utf16_code_units("ab"), vec![0x61, 0x62]);
    }

    #[test]
    fn utf16_emoji_surrogate_pair() {
        let u = utf16_code_units("😀");
        assert_eq!(u.len(), 2);
        assert_ne!(u[0], u[1]);
    }

    #[test]
    fn modifier_keyup_flags_extended_for_right_ctrl_alt_win() {
        let want = KEYEVENTF_KEYUP_BIT | KEYEVENTF_EXTENDEDKEY_BIT;
        assert_eq!(modifier_keyup_flags(VK_RCONTROL), want);
        assert_eq!(modifier_keyup_flags(VK_RMENU), want);
        assert_eq!(modifier_keyup_flags(VK_RWIN), want);
    }

    #[test]
    fn modifier_keyup_flags_plain_for_left_and_shifts() {
        assert_eq!(modifier_keyup_flags(VK_LSHIFT), KEYEVENTF_KEYUP_BIT);
        assert_eq!(modifier_keyup_flags(VK_RSHIFT), KEYEVENTF_KEYUP_BIT);
        assert_eq!(modifier_keyup_flags(VK_LCONTROL), KEYEVENTF_KEYUP_BIT);
        assert_eq!(modifier_keyup_flags(VK_LMENU), KEYEVENTF_KEYUP_BIT);
        assert_eq!(modifier_keyup_flags(VK_LWIN), KEYEVENTF_KEYUP_BIT);
    }

    #[cfg(windows)]
    #[test]
    fn modifier_keyup_flags_match_win32_and_keybdinput() {
        use windows::Win32::UI::Input::KeyboardAndMouse::{
            KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, VIRTUAL_KEY,
        };
        assert_eq!(KEYEVENTF_EXTENDEDKEY.0, KEYEVENTF_EXTENDEDKEY_BIT);
        assert_eq!(KEYEVENTF_KEYUP.0, KEYEVENTF_KEYUP_BIT);

        let ki_right = KEYBDINPUT {
            wVk: VIRTUAL_KEY(VK_RCONTROL as u16),
            wScan: 0,
            dwFlags: KEYBD_EVENT_FLAGS(modifier_keyup_flags(VK_RCONTROL)),
            time: 0,
            dwExtraInfo: 0,
        };
        assert_eq!(ki_right.dwFlags, KEYEVENTF_KEYUP | KEYEVENTF_EXTENDEDKEY);
        assert!(ki_right.dwFlags.contains(KEYEVENTF_EXTENDEDKEY));

        let ki_left = KEYBDINPUT {
            wVk: VIRTUAL_KEY(VK_LCONTROL as u16),
            wScan: 0,
            dwFlags: KEYBD_EVENT_FLAGS(modifier_keyup_flags(VK_LCONTROL)),
            time: 0,
            dwExtraInfo: 0,
        };
        assert_eq!(ki_left.dwFlags, KEYEVENTF_KEYUP);
        assert!(!ki_left.dwFlags.contains(KEYEVENTF_EXTENDEDKEY));
    }

    // --- type_chunks ---
    #[test]
    fn type_chunks_hello() {
        let chunks = type_chunks("hello").unwrap();
        assert_eq!(chunks, vec![TypeChunk::Text("hello")]);
    }

    #[test]
    fn type_chunks_a_newline_b() {
        let chunks = type_chunks("a\nb").unwrap();
        assert_eq!(chunks, vec![TypeChunk::Text("a"), TypeChunk::Return, TypeChunk::Text("b")]);
    }

    #[test]
    fn type_chunks_a_crlf_b_once() {
        let chunks = type_chunks("a\r\nb").unwrap();
        assert_eq!(chunks, vec![TypeChunk::Text("a"), TypeChunk::Return, TypeChunk::Text("b")]);
    }

    #[test]
    fn type_chunks_a_tab_b() {
        let chunks = type_chunks("a\tb").unwrap();
        assert_eq!(chunks, vec![TypeChunk::Text("a"), TypeChunk::Tab, TypeChunk::Text("b")]);
    }

    #[test]
    fn type_chunks_a_double_newline_b() {
        let chunks = type_chunks("a\n\nb").unwrap();
        assert_eq!(
            chunks,
            vec![
                TypeChunk::Text("a"),
                TypeChunk::Return,
                TypeChunk::Return,
                TypeChunk::Text("b")
            ]
        );
    }

    #[test]
    fn type_chunks_only_newline() {
        let chunks = type_chunks("\n").unwrap();
        assert_eq!(chunks, vec![TypeChunk::Return]);
    }

    #[test]
    fn type_chunks_null_err() {
        let err = type_chunks("a\0b").unwrap_err();
        assert!(
            err.to_string().contains("空字节"),
            "空字节错误应为中文，实际: {err}"
        );
    }

    #[test]
    fn has_c0_control_detects() {
        assert!(has_c0_control("\n"));
        assert!(has_c0_control("\t"));
        assert!(!has_c0_control("hello"));
        assert!(!has_c0_control("😀"));
    }
}
