use anyhow::{Context, Result};
use global_hotkey::{
    hotkey::HotKey,
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use std::collections::HashMap;

use crate::config::Config;

/// 热键管理器：注册 10 个热键并处理冲突
pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    /// digit 1..9,0 -> HotKey
    hotkeys: HashMap<u8, HotKey>,
    /// digit -> hotkey id
    id_to_digit: HashMap<u32, u8>,
    enabled: bool,
}

impl HotkeyManager {
    pub fn new() -> Result<Self> {
        let manager = GlobalHotKeyManager::new().context("创建全局热键管理器失败")?;
        Ok(Self {
            manager,
            hotkeys: HashMap::new(),
            id_to_digit: HashMap::new(),
            enabled: true,
        })
    }

    /// 根据配置注册全部 10 个热键，返回失败的条目
    pub fn register_from_config(&mut self, config: &Config) -> Vec<(u8, String)> {
        self.unregister_all();
        let mut failures = Vec::new();
        for (idx, hk_str) in config.hotkeys.iter().enumerate() {
            let digit = match idx {
                0..=8 => (idx + 1) as u8,
                9 => 0,
                _ => continue,
            };
            match crate::config::to_global_hotkey(hk_str) {
                Ok(hk) => {
                    match self.manager.register(hk) {
                        Ok(_) => {
                            self.hotkeys.insert(digit, hk);
                            self.id_to_digit.insert(hk.id(), digit);
                        }
                        Err(e) => {
                            log::warn!("热键注册失败 {} ({}): {}", hk_str, digit, e);
                            failures.push((digit, format!("{}: {}", hk_str, e)));
                        }
                    }
                }
                Err(e) => {
                    log::warn!("热键解析失败 {}: {}", hk_str, e);
                    failures.push((digit, format!("{}: {}", hk_str, e)));
                }
            }
        }
        failures
    }

    pub fn unregister_all(&mut self) {
        for hk in self.hotkeys.values() {
            let _ = self.manager.unregister(*hk);
        }
        self.hotkeys.clear();
        self.id_to_digit.clear();
    }

    pub fn set_enabled(&mut self, enabled: bool, config: &Config) -> Vec<(u8, String)> {
        if enabled == self.enabled {
            return vec![];
        }
        self.enabled = enabled;
        if enabled {
            self.register_from_config(config)
        } else {
            self.unregister_all();
            vec![]
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// 将 GlobalHotKeyEvent 映射为 digit
    pub fn event_to_digit(&self, event: &GlobalHotKeyEvent) -> Option<u8> {
        self.id_to_digit.get(&event.id).copied()
    }

    pub fn hotkey_count(&self) -> usize {
        self.hotkeys.len()
    }
}

/// 解析 digit 的辅助，与 History::get_by_hotkey_digit 保持一致
pub fn digit_to_history_index(digit: u8) -> Option<usize> {
    match digit {
        1..=9 => Some((digit - 1) as usize),
        0 => Some(9),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digit_to_index() {
        assert_eq!(digit_to_history_index(1), Some(0));
        assert_eq!(digit_to_history_index(9), Some(8));
        assert_eq!(digit_to_history_index(0), Some(9));
        assert_eq!(digit_to_history_index(10), None);
    }

    #[test]
    fn config_to_hotkey_parse() {
        let cfg = crate::config::Config::default();
        for hk in &cfg.hotkeys {
            assert!(crate::config::to_global_hotkey(hk).is_ok(), "{}", hk);
        }
    }
}
