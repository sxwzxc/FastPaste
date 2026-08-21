use anyhow::Result;
use std::borrow::Cow;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;

pub const INGEST_LIVE: u8 = 0;
pub const INGEST_PAUSED: u8 = 1;
pub const INGEST_CATCHUP: u8 = 2;

use crate::history::History;

// 注意：macOS transient 检测当前未完整实现，仅 Windows 实现了格式名检测，macOS 保持 false

/// 剪贴板访问抽象，便于测试与多平台适配
pub trait Clipboard: Send {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: &str) -> Result<()>;
    /// 获取当前剪贴板内容的完整备份（含非文本类型则返回 None）
    fn get_backup(&mut self) -> Option<ClipboardBackup>;
    fn restore(&mut self, backup: ClipboardBackup) -> Result<()>;
    /// 是否为敏感内容的瞬态标记（transient），默认 false
    fn is_transient(&mut self) -> bool {
        false
    }
}

#[derive(Debug, Clone)]
pub enum ClipboardBackup {
    Text(String),
    Image { width: usize, height: usize, bytes: Vec<u8> },
    Empty,
}

/// 基于 arboard 的真实剪贴板实现
pub struct ArboardClipboard {
    inner: arboard::Clipboard,
}

impl ArboardClipboard {
    pub fn new() -> Result<Self> {
        let inner = arboard::Clipboard::new()?;
        Ok(Self { inner })
    }
}

impl Clipboard for ArboardClipboard {
    fn get_text(&mut self) -> Option<String> {
        match self.inner.get_text() {
            Ok(t) => Some(t),
            Err(_) => None,
        }
    }

    fn set_text(&mut self, text: &str) -> Result<()> {
        self.inner.set_text(text.to_string())?;
        Ok(())
    }

    fn get_backup(&mut self) -> Option<ClipboardBackup> {
        if let Ok(t) = self.inner.get_text() {
            return Some(ClipboardBackup::Text(t));
        }
        if let Ok(img) = self.inner.get_image() {
            return Some(ClipboardBackup::Image {
                width: img.width,
                height: img.height,
                bytes: img.bytes.into_owned(),
            });
        }
        Some(ClipboardBackup::Empty)
    }

    fn restore(&mut self, backup: ClipboardBackup) -> Result<()> {
        match backup {
            ClipboardBackup::Text(t) => self.set_text(&t),
            ClipboardBackup::Image { width, height, bytes } => {
                let img = arboard::ImageData {
                    width,
                    height,
                    bytes: Cow::Owned(bytes),
                };
                self.inner.set_image(img)?;
                Ok(())
            }
            ClipboardBackup::Empty => {
                // 清空剪贴板：设置空字符串，失败则忽略
                let _ = self.inner.clear();
                Ok(())
            }
        }
    }

    fn is_transient(&mut self) -> bool {
        #[cfg(target_os = "windows")]
        {
            return is_transient_windows();
        }
        #[cfg(target_os = "macos")]
        {
            // macOS 检测未完整实现，返回 false
            // 仅在 debug 下偶尔提示，避免刷屏
            use std::sync::Once;
            static ONCE: Once = Once::new();
            ONCE.call_once(|| {
                log::debug!("macOS transient 检测未实现");
            });
            return false;
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            false
        }
    }
}

#[cfg(target_os = "windows")]
fn is_transient_windows() -> bool {
    // 枚举剪贴板格式，检测常见 transient 标记
    // 仅按格式名是否存在判断，并在注释说明 CanIncludeInClipboardHistory 的 0 值分支简化
    // 若枚举实现太绕，仅按格式名是否存在即可
    use clipboard_win::raw::{close, format_name_big, is_format_avail, open, EnumFormats};
    // 尝试打开剪贴板，失败则认为非 transient
    if open().is_err() {
        return false;
    }
    let mut is_transient = false;
    for fmt in EnumFormats::new() {
        if let Some(name) = format_name_big(fmt) {
            if name == "ExcludeClipboardContentFromMonitorProcessing"
                || name == "ClipboardViewerIgnore"
                || name == "CanIncludeInClipboardHistory"
            {
                // 对于 CanIncludeInClipboardHistory，若能读取到该格式数据且为 32-bit 0 也视为 transient
                // 简化：仅按格式名存在即视为 transient，如需更精确可在后续扩展
                is_transient = true;
                break;
            }
        } else {
            // fallback：尝试通过 is_format_avail 检测自定义格式？已覆盖
        }
    }
    let _ = close();
    // 未能通过枚举拿到时，也尝试直接通过已知名称注册的格式 ID 检测（避免枚举遗漏）
    if !is_transient {
        // 将常用名称转换为格式 ID 再检测是否可用，作为兜底
        if let Some(fmt) = clipboard_win::raw::register_format("ExcludeClipboardContentFromMonitorProcessing") {
            if is_format_avail(fmt.get() as u32) {
                is_transient = true;
            }
        }
        if !is_transient {
            if let Some(fmt) = clipboard_win::raw::register_format("CanIncludeInClipboardHistory") {
                if is_format_avail(fmt.get() as u32) {
                    is_transient = true;
                }
            }
        }
        if !is_transient {
            if let Some(fmt) = clipboard_win::raw::register_format("ClipboardViewerIgnore") {
                if is_format_avail(fmt.get() as u32) {
                    is_transient = true;
                }
            }
        }
    }
    is_transient
}

/// Mock 剪贴板，用于测试
#[derive(Debug, Default, Clone)]
pub struct MockClipboard {
    pub text: Option<String>,
    pub image: Option<(usize, usize, Vec<u8>)>,
    pub transient: bool,
    pub set_history: Vec<String>,
}

impl MockClipboard {
    pub fn with_text(t: &str) -> Self {
        Self { text: Some(t.to_string()), image: None, transient: false, set_history: vec![] }
    }

    pub fn with_image(width: usize, height: usize, bytes: Vec<u8>) -> Self {
        Self { text: None, image: Some((width, height, bytes)), transient: false, set_history: vec![] }
    }
}

impl Clipboard for MockClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.text.clone()
    }
    fn set_text(&mut self, text: &str) -> Result<()> {
        self.text = Some(text.to_string());
        self.image = None;
        self.set_history.push(text.to_string());
        Ok(())
    }
    fn get_backup(&mut self) -> Option<ClipboardBackup> {
        if let Some(t) = &self.text {
            return Some(ClipboardBackup::Text(t.clone()));
        }
        if let Some((w, h, bytes)) = &self.image {
            return Some(ClipboardBackup::Image {
                width: *w,
                height: *h,
                bytes: bytes.clone(),
            });
        }
        Some(ClipboardBackup::Empty)
    }
    fn restore(&mut self, backup: ClipboardBackup) -> Result<()> {
        match backup {
            ClipboardBackup::Text(t) => {
                self.text = Some(t);
                self.image = None;
                Ok(())
            }
            ClipboardBackup::Image { width, height, bytes } => {
                self.image = Some((width, height, bytes));
                self.text = None;
                Ok(())
            }
            ClipboardBackup::Empty => {
                self.text = None;
                self.image = None;
                Ok(())
            }
        }
    }

    fn is_transient(&mut self) -> bool {
        self.transient
    }
}

/// 剪贴板监听器 trait
pub trait ClipboardWatcher: Send {
    fn poll(&mut self) -> Option<String>;
}

/// 轮询式监听器
pub struct PollingWatcher<C: Clipboard> {
    clipboard: C,
    last_seen: Option<String>,
}

impl<C: Clipboard> PollingWatcher<C> {
    pub fn new(clipboard: C) -> Self {
        Self { clipboard, last_seen: None }
    }

    pub fn clipboard_mut(&mut self) -> &mut C {
        &mut self.clipboard
    }
}

impl<C: Clipboard + Send> ClipboardWatcher for PollingWatcher<C> {
    fn poll(&mut self) -> Option<String> {
        let current = self.clipboard.get_text()?;
        if self.clipboard.is_transient() {
            // 仍更新 last_seen，避免以后反复看到同一瞬态文本，但不向调用方返回
            // 同一内容稍后作为非瞬态再出现也会被挡住，接受该权衡（密码管理器通常复制后清空）
            self.last_seen = Some(current.clone());
            return None;
        }
        if Some(&current) == self.last_seen.as_ref() {
            return None;
        }
        self.last_seen = Some(current.clone());
        Some(current)
    }
}

/// 高层管理器：负责轮询、过滤、入队历史
pub struct ClipboardManager<C: Clipboard> {
    watcher: PollingWatcher<C>,
    history: Arc<Mutex<History>>,
    enabled: Arc<Mutex<bool>>,
    ingest_paused: Arc<AtomicU8>,
}

impl<C: Clipboard + Send + 'static> ClipboardManager<C> {
    pub fn new(watcher: PollingWatcher<C>, history: Arc<Mutex<History>>, enabled: Arc<Mutex<bool>>, ingest_paused: Arc<AtomicU8>) -> Self {
        Self { watcher, history, enabled, ingest_paused }
    }

    /// 执行一次轮询检查，返回是否入队
    /// 停用状态下仍 poll 以更新 last_seen，但不入队，避免再启用时把停用期最后一次复制补录
    pub fn tick(&mut self) -> bool {
        let polled = self.watcher.poll();
        match self.ingest_paused.load(Ordering::SeqCst) {
            INGEST_PAUSED => return false,
            INGEST_CATCHUP => {
                self.ingest_paused.store(INGEST_LIVE, Ordering::SeqCst);
                return false;
            }
            _ => {}
        }
        let Some(text) = polled else {
            return false;
        };
        if !*self.enabled.lock() {
            return false;
        }
        self.history.lock().push(text)
    }

    pub fn watcher_mut(&mut self) -> &mut PollingWatcher<C> {
        &mut self.watcher
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::History;

    #[test]
    fn polling_watcher_detects_change() {
        let mock = MockClipboard::with_text("hello");
        let mut watcher = PollingWatcher::new(mock);
        assert_eq!(watcher.poll(), Some("hello".into()));
        // 相同内容不再触发
        assert_eq!(watcher.poll(), None);
        // 改变内容
        watcher.clipboard_mut().text = Some("world".into());
        assert_eq!(watcher.poll(), Some("world".into()));
    }

    #[test]
    fn manager_tick_respects_enabled_and_history() {
        let history = Arc::new(Mutex::new(History::default()));
        let enabled = Arc::new(Mutex::new(true));
        let ingest_paused = Arc::new(AtomicU8::new(INGEST_LIVE));
        let mock = MockClipboard::with_text("hello");
        let watcher = PollingWatcher::new(mock);
        let mut mgr = ClipboardManager::new(watcher, history.clone(), enabled.clone(), ingest_paused);
        // 启用，入队 "hello"
        assert!(mgr.tick());
        assert_eq!(history.lock().len(), 1);
        assert_eq!(history.lock().get(0).unwrap(), "hello");
        // 未变化不入队
        assert!(!mgr.tick());
        // 停用，把 mock 文本改成 "secret-during-disable"，调用 tick()，返回 false，历史长度仍为 1
        *enabled.lock() = false;
        mgr.watcher_mut().clipboard_mut().text = Some("secret-during-disable".into());
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 1);
        // 再启用，不改 mock 文本，再 tick()：返回 false，历史仍只有 "hello"（不补录）
        *enabled.lock() = true;
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 1);
        assert_eq!(history.lock().get(0).unwrap(), "hello");
        // 启用下把文本改成 "after-enable"，tick() 为 true，历史长度为 2
        mgr.watcher_mut().clipboard_mut().text = Some("after-enable".into());
        assert!(mgr.tick());
        assert_eq!(history.lock().len(), 2);
        assert_eq!(history.lock().get(0).unwrap(), "after-enable");
    }

    #[test]
    fn manager_ignores_empty_and_filters() {
        let history = Arc::new(Mutex::new(History::new(Some("secret".into()))));
        let enabled = Arc::new(Mutex::new(true));
        let ingest_paused = Arc::new(AtomicU8::new(INGEST_LIVE));
        let mock = MockClipboard::default();
        let watcher = PollingWatcher::new(mock);
        let mut mgr = ClipboardManager::new(watcher, history.clone(), enabled, ingest_paused);

        mgr.watcher_mut().clipboard_mut().text = Some("   ".into());
        assert!(!mgr.tick());
        mgr.watcher_mut().clipboard_mut().text = Some("my secret".into());
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 0);
    }

    #[test]
    fn polling_watcher_transient_blocks() {
        let mut mock = MockClipboard::with_text("pw");
        mock.transient = true;
        let mut watcher = PollingWatcher::new(mock);
        assert_eq!(watcher.poll(), None);
        // 相同文本仍 None（已更新 last_seen）
        assert_eq!(watcher.poll(), None);
        // 改为非瞬态但相同文本仍被挡
        watcher.clipboard_mut().transient = false;
        assert_eq!(watcher.poll(), None);
        // 新文本且非瞬态则通过
        watcher.clipboard_mut().text = Some("hello".into());
        assert_eq!(watcher.poll(), Some("hello".into()));
    }

    #[test]
    fn polling_watcher_non_transient_ok() {
        let mock = MockClipboard::with_text("hello");
        let mut watcher = PollingWatcher::new(mock);
        // transient = false 时行为与现在一致
        assert_eq!(watcher.poll(), Some("hello".into()));
    }

    #[test]
    fn manager_transient_not_in_history() {
        let history = Arc::new(Mutex::new(History::default()));
        let enabled = Arc::new(Mutex::new(true));
        let ingest_paused = Arc::new(AtomicU8::new(INGEST_LIVE));
        let mut mock = MockClipboard::with_text("secret-pw");
        mock.transient = true;
        let watcher = PollingWatcher::new(mock);
        let mut mgr = ClipboardManager::new(watcher, history.clone(), enabled, ingest_paused);
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 0);
        // 之后改为非瞬态但相同文本仍不入队（已更新 last_seen）
        mgr.watcher_mut().clipboard_mut().transient = false;
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 0);
        // 新内容非瞬态则入队
        mgr.watcher_mut().clipboard_mut().text = Some("normal".into());
        assert!(mgr.tick());
        assert_eq!(history.lock().len(), 1);
    }

    #[test]
    fn manager_tick_respects_ingest_paused() {
        let history = Arc::new(Mutex::new(History::default()));
        let enabled = Arc::new(Mutex::new(true));
        let ingest_paused = Arc::new(AtomicU8::new(INGEST_LIVE));
        let mock = MockClipboard::with_text("hello");
        let watcher = PollingWatcher::new(mock);
        let mut mgr = ClipboardManager::new(watcher, history.clone(), enabled.clone(), ingest_paused.clone());
        // 启用，入队 "hello"
        assert!(mgr.tick());
        assert_eq!(history.lock().len(), 1);
        assert_eq!(history.lock().get(0).unwrap(), "hello");
        // ingest_paused.store(PAUSED)，mock 改为 "during-paste"，tick() 为 false，历史长度仍 1
        ingest_paused.store(INGEST_PAUSED, Ordering::SeqCst);
        mgr.watcher_mut().clipboard_mut().text = Some("during-paste".into());
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 1);
        // ingest_paused.store(LIVE)，不改 mock，再 tick()：false，历史仍 "hello"（pause 期间 poll 已更新 last_seen）
        ingest_paused.store(INGEST_LIVE, Ordering::SeqCst);
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 1);
        assert_eq!(history.lock().get(0).unwrap(), "hello");
        // mock 改为 "after"，tick true，长度 2
        mgr.watcher_mut().clipboard_mut().text = Some("after".into());
        assert!(mgr.tick());
        assert_eq!(history.lock().len(), 2);
        assert_eq!(history.lock().get(0).unwrap(), "after");
    }

    #[test]
    fn manager_tick_catchup_skips_restored() {
        let history = Arc::new(Mutex::new(History::default()));
        let enabled = Arc::new(Mutex::new(true));
        let ingest_paused = Arc::new(AtomicU8::new(INGEST_LIVE));
        let mock = MockClipboard::with_text("hello");
        let watcher = PollingWatcher::new(mock);
        let mut mgr = ClipboardManager::new(watcher, history.clone(), enabled.clone(), ingest_paused.clone());
        // hello 入队
        assert!(mgr.tick());
        assert_eq!(history.lock().len(), 1);
        // 进入 PAUSED，mock 改为 "pasted-entry" 并 tick（不入队）
        ingest_paused.store(INGEST_PAUSED, Ordering::SeqCst);
        mgr.watcher_mut().clipboard_mut().text = Some("pasted-entry".into());
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 1);
        // 再改为 "restored-old"，切 CATCHUP，tick 返回 false、历史仍只有 hello
        mgr.watcher_mut().clipboard_mut().text = Some("restored-old".into());
        ingest_paused.store(INGEST_CATCHUP, Ordering::SeqCst);
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 1);
        // CATCHUP 的 tick 已把状态写回 LIVE，不要人工再 store LIVE
        assert_eq!(ingest_paused.load(Ordering::SeqCst), INGEST_LIVE);
        // 不改 mock 再 tick，仍 false
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 1);
        assert_eq!(history.lock().get(0).unwrap(), "hello");
        // 最后 mock 改为 "after"，tick true，长度 2
        mgr.watcher_mut().clipboard_mut().text = Some("after".into());
        assert!(mgr.tick());
        assert_eq!(history.lock().len(), 2);
        assert_eq!(history.lock().get(0).unwrap(), "after");
    }

    #[test]
    fn manager_tick_catchup_poll_none_transitions_to_live() {
        let history = Arc::new(Mutex::new(History::default()));
        let enabled = Arc::new(Mutex::new(true));
        let ingest_paused = Arc::new(AtomicU8::new(INGEST_LIVE));
        let mock = MockClipboard::with_text("hello");
        let watcher = PollingWatcher::new(mock);
        let mut mgr = ClipboardManager::new(watcher, history.clone(), enabled.clone(), ingest_paused.clone());
        // 1. hello 入队
        assert!(mgr.tick());
        assert_eq!(history.lock().len(), 1);
        assert_eq!(history.lock().get(0).unwrap(), "hello");
        // 2. store(PAUSED)，不改 mock，tick 为 false（poll 为 None）
        ingest_paused.store(INGEST_PAUSED, Ordering::SeqCst);
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 1);
        // 3. store(CATCHUP)，仍不改 mock，tick 为 false，且 ingest 已是 INGEST_LIVE
        ingest_paused.store(INGEST_CATCHUP, Ordering::SeqCst);
        assert!(!mgr.tick());
        assert_eq!(ingest_paused.load(Ordering::SeqCst), INGEST_LIVE);
        assert_eq!(history.lock().len(), 1);
        // 4. 不改 mock 再 tick，仍 false，历史仍 1 条 hello
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 1);
        assert_eq!(history.lock().get(0).unwrap(), "hello");
        // 5. mock 改为 "after"，tick true，长度 2
        mgr.watcher_mut().clipboard_mut().text = Some("after".into());
        assert!(mgr.tick());
        assert_eq!(history.lock().len(), 2);
        assert_eq!(history.lock().get(0).unwrap(), "after");
    }
}
