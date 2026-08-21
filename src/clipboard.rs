use anyhow::Result;
use std::sync::Arc;
use parking_lot::Mutex;

use crate::history::History;

/// 剪贴板访问抽象，便于测试与多平台适配
pub trait Clipboard: Send {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: &str) -> Result<()>;
    /// 获取当前剪贴板内容的完整备份（含非文本类型则返回 None）
    fn get_backup(&mut self) -> Option<ClipboardBackup>;
    fn restore(&mut self, backup: ClipboardBackup) -> Result<()>;
}

#[derive(Debug, Clone)]
pub enum ClipboardBackup {
    Text(String),
    Empty,
}

/// 基于 arboard 的真实剪贴板实现
pub struct ArboardClipboard {
    inner: arboard::Clipboard,
    last_text: Option<String>,
}

impl ArboardClipboard {
    pub fn new() -> Result<Self> {
        let inner = arboard::Clipboard::new()?;
        Ok(Self { inner, last_text: None })
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
        match self.inner.get_text() {
            Ok(t) => Some(ClipboardBackup::Text(t)),
            Err(_) => Some(ClipboardBackup::Empty),
        }
    }

    fn restore(&mut self, backup: ClipboardBackup) -> Result<()> {
        match backup {
            ClipboardBackup::Text(t) => self.set_text(&t),
            ClipboardBackup::Empty => {
                // 清空剪贴板：设置空字符串，失败则忽略
                let _ = self.inner.clear();
                Ok(())
            }
        }
    }
}

/// Mock 剪贴板，用于测试
#[derive(Debug, Default, Clone)]
pub struct MockClipboard {
    pub text: Option<String>,
    pub set_history: Vec<String>,
}

impl MockClipboard {
    pub fn with_text(t: &str) -> Self {
        Self { text: Some(t.to_string()), set_history: vec![] }
    }
}

impl Clipboard for MockClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.text.clone()
    }
    fn set_text(&mut self, text: &str) -> Result<()> {
        self.text = Some(text.to_string());
        self.set_history.push(text.to_string());
        Ok(())
    }
    fn get_backup(&mut self) -> Option<ClipboardBackup> {
        Some(match &self.text {
            Some(t) => ClipboardBackup::Text(t.clone()),
            None => ClipboardBackup::Empty,
        })
    }
    fn restore(&mut self, backup: ClipboardBackup) -> Result<()> {
        match backup {
            ClipboardBackup::Text(t) => {
                self.text = Some(t);
                Ok(())
            }
            ClipboardBackup::Empty => {
                self.text = None;
                Ok(())
            }
        }
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
}

impl<C: Clipboard + Send + 'static> ClipboardManager<C> {
    pub fn new(watcher: PollingWatcher<C>, history: Arc<Mutex<History>>, enabled: Arc<Mutex<bool>>) -> Self {
        Self { watcher, history, enabled }
    }

    /// 执行一次轮询检查，返回是否入队
    pub fn tick(&mut self) -> bool {
        if !*self.enabled.lock() {
            return false;
        }
        let Some(text) = self.watcher.poll() else {
            return false;
        };
        let mut h = self.history.lock();
        h.push(text)
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
        let mock = MockClipboard::with_text("hello");
        let watcher = PollingWatcher::new(mock);
        let mut mgr = ClipboardManager::new(watcher, history.clone(), enabled.clone());
        assert!(mgr.tick());
        assert_eq!(history.lock().len(), 1);
        // 未变化不入队
        assert!(!mgr.tick());
        // 停用状态不入队
        *enabled.lock() = false;
        mgr.watcher_mut().clipboard_mut().text = Some("world".into());
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 1);
        // 启用后入队
        *enabled.lock() = true;
        assert!(mgr.tick());
        assert_eq!(history.lock().len(), 2);
    }

    #[test]
    fn manager_ignores_empty_and_filters() {
        let history = Arc::new(Mutex::new(History::new(Some("secret".into()))));
        let enabled = Arc::new(Mutex::new(true));
        let mock = MockClipboard::default();
        let watcher = PollingWatcher::new(mock);
        let mut mgr = ClipboardManager::new(watcher, history.clone(), enabled);

        mgr.watcher_mut().clipboard_mut().text = Some("   ".into());
        assert!(!mgr.tick());
        mgr.watcher_mut().clipboard_mut().text = Some("my secret".into());
        assert!(!mgr.tick());
        assert_eq!(history.lock().len(), 0);
    }
}
