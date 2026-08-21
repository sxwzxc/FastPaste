use anyhow::Result;
use parking_lot::Mutex;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::clipboard::{ArboardClipboard, Clipboard, ClipboardManager, PollingWatcher};
use crate::config::Config;
use crate::history::History;
use crate::paste::{do_paste, EnigoPaster};

pub struct AppState {
    pub history: Arc<Mutex<History>>,
    pub enabled: Arc<Mutex<bool>>,
    pub config: Arc<Mutex<Config>>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let ignore = config.ignore_regex.clone();
        let history = Arc::new(Mutex::new(History::new(ignore)));
        let enabled = Arc::new(Mutex::new(true));
        let config = Arc::new(Mutex::new(config));
        Self { history, enabled, config }
    }

    pub fn push_history(&self, text: String) -> bool {
        let mut h = self.history.lock();
        h.push(text)
    }

    pub fn get_history_entry(&self, digit: u8) -> Option<String> {
        let h = self.history.lock();
        h.get_by_hotkey_digit(digit).cloned()
    }

    pub fn is_enabled(&self) -> bool {
        *self.enabled.lock()
    }

    pub fn set_enabled(&self, v: bool) {
        *self.enabled.lock() = v;
    }

    pub fn reload_config(&self) -> Result<Vec<(u8, String)>> {
        let new_cfg = Config::load()?;
        new_cfg.validate()?;
        let mut cfg = self.config.lock();
        // 更新历史的 ignore_regex
        {
            let mut h = self.history.lock();
            h.set_ignore_regex(new_cfg.ignore_regex.clone());
        }
        *cfg = new_cfg;
        Ok(vec![])
    }
}

/// 启动剪贴板轮询线程
pub fn spawn_clipboard_thread(state: AppState) {
    let history = state.history.clone();
    let enabled = state.enabled.clone();
    let config = state.config.clone();

    thread::spawn(move || {
        let clipboard = match ArboardClipboard::new() {
            Ok(cb) => cb,
            Err(e) => {
                log::error!("初始化剪贴板失败: {}", e);
                return;
            }
        };
        let watcher = PollingWatcher::new(clipboard);
        let mut manager = ClipboardManager::new(watcher, history, enabled);
        loop {
            let interval = {
                let cfg = config.lock();
                cfg.polling_interval_ms
            };
            thread::sleep(Duration::from_millis(interval));
            if manager.tick() {
                log::debug!("新条目已入队");
            }
        }
    });
}

/// 执行粘贴（由热键或托盘历史点击触发）
pub fn handle_paste(state: &AppState, digit: u8) {
    if !state.is_enabled() {
        log::info!("当前为停用状态，忽略粘贴请求 digit={}", digit);
        return;
    }
    let Some(text) = state.get_history_entry(digit) else {
        log::info!("历史为空或索引超出: digit={}", digit);
        crate::dialog::show_warning("FastPaste", &format!("历史 {} 为空", digit));
        return;
    };
    let (method, preserve) = {
        let cfg = state.config.lock();
        (cfg.paste_method, cfg.preserve_clipboard)
    };

    // 剪贴板与粘贴需在主线程或短期线程中执行
    thread::spawn(move || {
        let mut clipboard = match ArboardClipboard::new() {
            Ok(cb) => cb,
            Err(e) => {
                log::error!("剪贴板初始化失败: {}", e);
                return;
            }
        };
        let mut paster = EnigoPaster::new();
        let clipboard_ref: &mut dyn Clipboard = &mut clipboard;
        let paster_ref: &mut dyn crate::paste::Paster = &mut paster;
        match do_paste(paster_ref, clipboard_ref, &text, method, preserve) {
            Ok(_) => log::info!("粘贴成功 digit={} len={}", digit, text.len()),
            Err(e) => {
                log::error!("粘贴失败: {}", e);
                // 降级提示
                let _ = clipboard_ref.set_text(&text);
                crate::dialog::show_warning("FastPaste", "粘贴失败，已复制到剪贴板，请手动 Ctrl+V");
            }
        }
    });
}
