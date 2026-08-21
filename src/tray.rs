use anyhow::Result;
use muda::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use parking_lot::Mutex;
use std::sync::Arc;
use tray_icon::{TrayIcon, TrayIconBuilder, Icon};

use crate::history::History;

/// 托盘菜单 ID 常量
pub const ID_TOGGLE: &str = "toggle";
pub const ID_OPEN_CONFIG: &str = "open_config";
pub const ID_RELOAD_CONFIG: &str = "reload_config";
pub const ID_AUTOSTART: &str = "autostart";
pub const ID_ELEVATED_RESTART: &str = "elevated_restart";
pub const ID_ABOUT: &str = "about";
pub const ID_DIAGNOSIS: &str = "diagnosis";
pub const ID_EXIT: &str = "exit";
pub const ID_HISTORY_BASE: &str = "history_";

pub struct Tray {
    _tray: TrayIcon,
    menu: Menu,
    toggle_item: CheckMenuItem,
    autostart_item: CheckMenuItem,
    history_items: Vec<MenuItem>,
    enabled: Arc<Mutex<bool>>,
    history: Arc<Mutex<History>>,
}

impl Tray {
    pub fn new(enabled: Arc<Mutex<bool>>, history: Arc<Mutex<History>>) -> Result<Self> {
        let toggle_item = CheckMenuItem::with_id(ID_TOGGLE, "启用", true, *enabled.lock(), None);
        let autostart_item = CheckMenuItem::with_id(ID_AUTOSTART, "开机自启", true, crate::autostart::is_enabled(), None);

        let history_submenu = Submenu::with_id("history_submenu", "历史预览", true);
        let mut history_items = Vec::new();
        for i in 0..10 {
            let digit = if i == 9 { 0 } else { (i + 1) as u8 };
            let label = format!("{}: (空)", digit);
            let item = MenuItem::with_id(format!("{}{}", ID_HISTORY_BASE, digit), label, false, None);
            history_submenu.append(&item).ok();
            history_items.push(item);
        }

        let open_config = MenuItem::with_id(ID_OPEN_CONFIG, "打开配置文件", true, None);
        let reload_config = MenuItem::with_id(ID_RELOAD_CONFIG, "重载配置", true, None);
        let elevated_restart = MenuItem::with_id(ID_ELEVATED_RESTART, "以管理员身份重启", true, None);
        let diagnosis = MenuItem::with_id(ID_DIAGNOSIS, "权限诊断", true, None);
        let about = MenuItem::with_id(ID_ABOUT, "关于", true, None);
        let exit = MenuItem::with_id(ID_EXIT, "退出", true, None);

        let menu = Menu::with_items(&[
            &toggle_item,
            &PredefinedMenuItem::separator(),
            &history_submenu,
            &PredefinedMenuItem::separator(),
            &open_config,
            &reload_config,
            &autostart_item,
            &elevated_restart,
            &PredefinedMenuItem::separator(),
            &diagnosis,
            &about,
            &PredefinedMenuItem::separator(),
            &exit,
        ])?;

        let icon = load_icon();
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu.clone()))
            .with_tooltip("FastPaste - 剪贴板管理器")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            _tray: tray,
            menu,
            toggle_item,
            autostart_item,
            history_items,
            enabled,
            history,
        })
    }

    /// 刷新历史预览
    pub fn refresh_history_preview(&self) {
        let h = self.history.lock();
        for (i, item) in self.history_items.iter().enumerate() {
            let digit = if i == 9 { 0 } else { (i + 1) as u8 };
            let (text, enabled) = history_item_label(digit, h.get(i).map(|s| s.as_str()));
            item.set_text(&text);
            item.set_enabled(enabled);
        }
    }

    pub fn set_enabled_checked(&self, enabled: bool) {
        self.toggle_item.set_checked(enabled);
    }

    pub fn set_autostart_checked(&self, enabled: bool) {
        self.autostart_item.set_checked(enabled);
    }

    pub fn handle_menu_event(&self, id: &str) -> TrayEvent {
        match id {
            ID_TOGGLE => {
                let mut en = self.enabled.lock();
                *en = !*en;
                self.toggle_item.set_checked(*en);
                TrayEvent::Toggle(*en)
            }
            ID_OPEN_CONFIG => TrayEvent::OpenConfig,
            ID_RELOAD_CONFIG => TrayEvent::ReloadConfig,
            ID_AUTOSTART => {
                let new_state = !self.autostart_item.is_checked();
                self.autostart_item.set_checked(new_state);
                TrayEvent::ToggleAutostart(new_state)
            }
            ID_ELEVATED_RESTART => TrayEvent::ElevatedRestart,
            ID_DIAGNOSIS => TrayEvent::Diagnosis,
            ID_ABOUT => TrayEvent::About,
            ID_EXIT => TrayEvent::Exit,
            other if other.starts_with(ID_HISTORY_BASE) => {
                let digit_str = &other[ID_HISTORY_BASE.len()..];
                if let Ok(d) = digit_str.parse::<u8>() {
                    TrayEvent::PasteHistory(d)
                } else {
                    TrayEvent::None
                }
            }
            _ => TrayEvent::None,
        }
    }
}

/// 历史项标签纯函数，便于测试（不依赖 GUI）
pub fn history_item_label(digit: u8, entry: Option<&str>) -> (String, bool) {
    match entry {
        Some(s) => {
            let preview = History::format_preview(s, 20);
            (format!("{}: {}", digit, preview), true)
        }
        None => (format!("{}: (空)", digit), false),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrayEvent {
    Toggle(bool),
    OpenConfig,
    ReloadConfig,
    ToggleAutostart(bool),
    ElevatedRestart,
    Diagnosis,
    About,
    Exit,
    PasteHistory(u8),
    None,
}

fn load_icon() -> Icon {
    // 使用嵌入的 FastPaste 图标：白底剪贴板 + 深灰描边 + 顶部黑色夹子 + 灰色文本线 + 右下角蓝色闪电
    // 设计：高对比度，适配亮/暗任务栏，16px 仍可辨识，详见 assets/generate_icon_v2.py
    let rgba = crate::icon_data::TRAY_ICON_RGBA.to_vec();
    let size = crate::icon_data::TRAY_ICON_SIZE;
    Icon::from_rgba(rgba, size, size).unwrap_or_else(|_| {
        // 回退：1x1 透明
        Icon::from_rgba(vec![0, 0, 0, 0], 1, 1).unwrap()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::History;

    #[test]
    fn tray_event_parse_history() {
        let _enabled = Arc::new(Mutex::new(true));
        let _history = Arc::new(Mutex::new(History::default()));
        // 构造 Tray 需要图形环境，测试中仅验证事件解析逻辑
        let id = "history_1";
        assert!(id.starts_with(ID_HISTORY_BASE));
        let digit: u8 = id[ID_HISTORY_BASE.len()..].parse().unwrap();
        assert_eq!(digit, 1);
    }

    #[test]
    fn history_item_label_with_entry() {
        let (text, enabled) = history_item_label(1, Some("hello world"));
        assert_eq!(text, "1: hello world");
        assert!(enabled);
    }

    #[test]
    fn history_item_label_empty() {
        let (text, enabled) = history_item_label(0, None);
        assert_eq!(text, "0: (空)");
        assert!(!enabled);
    }

    #[test]
    fn history_item_label_truncates_20() {
        let long = "a".repeat(25);
        let (text, enabled) = history_item_label(2, Some(&long));
        let expected_preview = History::format_preview(&long, 20);
        assert_eq!(text, format!("2: {}", expected_preview));
        assert!(enabled);
        assert!(expected_preview.ends_with("..."));
    }

    #[test]
    fn history_item_label_newline_sanitized() {
        let (text, _) = history_item_label(3, Some("a\nb\nc"));
        assert_eq!(text, "3: a b c");
    }
}
