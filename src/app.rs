use anyhow::Result;
use parking_lot::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::clipboard::{INGEST_CATCHUP, INGEST_LIVE, INGEST_PAUSED};

use crate::clipboard::{ArboardClipboard, Clipboard, ClipboardManager, PollingWatcher};
use crate::config::Config;
use crate::history::History;
use crate::paste::{do_paste, EnigoPaster};

pub struct PasteGate {
    busy: AtomicBool,
}

impl PasteGate {
    pub fn new() -> Self {
        Self { busy: AtomicBool::new(false) }
    }
    pub fn try_begin(&self) -> bool {
        self.busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }
    pub fn end(&self) {
        self.busy.store(false, Ordering::SeqCst);
    }
}

#[derive(Clone)]
pub struct AppState {
    pub history: Arc<Mutex<History>>,
    pub enabled: Arc<Mutex<bool>>,
    pub config: Arc<Mutex<Config>>,
    pub pending_notice: Arc<Mutex<Option<(String, String)>>>,
    pub paste_gate: Arc<PasteGate>,
    pub ingest_paused: Arc<AtomicU8>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let ignore = config.ignore_regex.clone();
        let history = Arc::new(Mutex::new(History::with_limits(
            ignore,
            config.max_entry_bytes,
        )));
        let enabled = Arc::new(Mutex::new(true));
        let config = Arc::new(Mutex::new(config));
        let pending_notice = Arc::new(Mutex::new(None));
        let paste_gate = Arc::new(PasteGate::new());
        let ingest_paused = Arc::new(AtomicU8::new(INGEST_LIVE));
        Self { history, enabled, config, pending_notice, paste_gate, ingest_paused }
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

    pub fn reload_config(&self) -> Result<Config> {
        let new_cfg = Config::load()?;
        new_cfg.validate()?;
        {
            let mut h = self.history.lock();
            h.set_ignore_regex(new_cfg.ignore_regex.clone());
            h.set_max_bytes(new_cfg.max_entry_bytes);
        }
        *self.config.lock() = new_cfg.clone();
        Ok(new_cfg)
    }
}

/// 启动剪贴板轮询线程
pub fn spawn_clipboard_thread(state: AppState) {
    let history = state.history.clone();
    let enabled = state.enabled.clone();
    let config = state.config.clone();
    let ingest_paused = state.ingest_paused.clone();

    thread::spawn(move || {
        let clipboard = match ArboardClipboard::new() {
            Ok(cb) => cb,
            Err(e) => {
                log::error!("初始化剪贴板失败: {}", e);
                return;
            }
        };
        let watcher = PollingWatcher::new(clipboard);
        let mut manager = ClipboardManager::new(watcher, history, enabled, ingest_paused);
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
    let method = crate::paste::resolve_paste_method(method, &text);

    if !state.paste_gate.try_begin() {
        log::info!("粘贴进行中，忽略重复热键 digit={}", digit);
        return;
    }

    // 剪贴板与粘贴需在短期线程中执行，失败时通过 pending_notice 让主线程弹窗（避免在工作线程直接弹 rfd）
    let state_clone = state.clone();
    thread::spawn(move || {
        struct PasteGuard {
            gate: Arc<PasteGate>,
            ingest: Arc<AtomicU8>,
        }
        impl Drop for PasteGuard {
            fn drop(&mut self) {
                self.ingest.store(INGEST_CATCHUP, Ordering::SeqCst);
                self.gate.end();
            }
        }
        let _guard = PasteGuard {
            gate: state_clone.paste_gate.clone(),
            ingest: state_clone.ingest_paused.clone(),
        };
        state_clone.ingest_paused.store(INGEST_PAUSED, Ordering::SeqCst);
        let mut clipboard = match ArboardClipboard::new() {
            Ok(cb) => cb,
            Err(e) => {
                log::error!("剪贴板初始化失败: {}", e);
                *state_clone.pending_notice.lock() = Some((
                    "FastPaste".into(),
                    "粘贴失败，无法访问剪贴板".into(),
                ));
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
                let body = if method == crate::config::PasteMethod::Type {
                    "粘贴失败（击键注入未成功）"
                } else {
                    let err_str = format!("{:?}", e);
                    if err_str.contains("clipboard_write_failed") {
                        "粘贴失败，且未能把条目放到系统剪贴板"
                    } else {
                        "粘贴失败，已写入剪贴板，请手动粘贴"
                    }
                };
                *state_clone.pending_notice.lock() =
                    Some(("FastPaste".into(), body.into()));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paste_gate_single_flight() {
        let g = PasteGate::new();
        assert!(g.try_begin());
        assert!(!g.try_begin());
        g.end();
        assert!(g.try_begin());
    }
}
