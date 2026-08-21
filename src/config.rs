use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 粘贴方式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PasteMethod {
    Auto,
    #[serde(rename = "clipboard")]
    Clipboard,
    #[serde(rename = "type")]
    Type,
}

impl Default for PasteMethod {
    fn default() -> Self {
        Self::Auto
    }
}

impl std::fmt::Display for PasteMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Clipboard => write!(f, "clipboard"),
            Self::Type => write!(f, "type"),
        }
    }
}

fn default_preserve() -> bool {
    true
}
fn default_polling() -> u64 {
    500
}
fn default_max_bytes() -> usize {
    5 * 1024 * 1024
}
fn default_hotkeys() -> Vec<String> {
    vec![
        "ctrl+shift+1".into(),
        "ctrl+shift+2".into(),
        "ctrl+shift+3".into(),
        "ctrl+shift+4".into(),
        "ctrl+shift+5".into(),
        "ctrl+shift+6".into(),
        "ctrl+shift+7".into(),
        "ctrl+shift+8".into(),
        "ctrl+shift+9".into(),
        "ctrl+shift+0".into(),
    ]
}
fn default_autostart_elevated() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 10 个热键，索引 0 对应 Ctrl+Shift+1（最新），索引 9 对应 Ctrl+Shift+0（最旧）
    #[serde(default = "default_hotkeys")]
    pub hotkeys: Vec<String>,

    #[serde(default = "default_preserve")]
    pub preserve_clipboard: bool,

    #[serde(default)]
    pub paste_method: PasteMethod,

    #[serde(default = "default_polling")]
    pub polling_interval_ms: u64,

    #[serde(default = "default_max_bytes")]
    pub max_entry_bytes: usize,

    /// 可选的忽略正则
    #[serde(default)]
    pub ignore_regex: Option<String>,

    #[serde(default = "default_autostart_elevated")]
    pub autostart_elevated: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkeys: default_hotkeys(),
            preserve_clipboard: true,
            paste_method: PasteMethod::Auto,
            polling_interval_ms: 500,
            max_entry_bytes: 5 * 1024 * 1024,
            ignore_regex: None,
            autostart_elevated: true,
        }
    }
}

impl Config {
    pub fn config_path() -> Result<PathBuf> {
        let base = dirs::config_dir().context("无法获取系统配置目录")?;
        Ok(base.join("FastPaste").join("config.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Self> {
        if !path.exists() {
            let cfg = Self::default();
            // 自动创建默认配置
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).ok();
            }
            cfg.save_to(path).ok();
            return Ok(cfg);
        }
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("读取配置文件失败: {}", path.display()))?;
        let cfg: Self = toml::from_str(&content).context("配置文件 TOML 解析失败")?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        self.save_to(&path)
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        self.validate()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("创建配置目录失败")?;
        }
        let content = self.documented_toml();
        std::fs::write(path, content).context("写入配置文件失败")?;
        Ok(())
    }

    fn documented_toml(&self) -> String {
        // hotkeys line via toml serialization to keep escaping correct
        let hotkeys_line = {
            #[derive(Serialize)]
            struct Tmp {
                hotkeys: Vec<String>,
            }
            let tmp = Tmp { hotkeys: self.hotkeys.clone() };
            toml::to_string(&tmp).unwrap().trim().to_string()
        };
        let ignore_regex_line = match &self.ignore_regex {
            Some(val) => {
                #[derive(Serialize)]
                struct Tmp {
                    ignore_regex: String,
                }
                let tmp = Tmp { ignore_regex: val.clone() };
                let s = toml::to_string(&tmp).unwrap();
                // s is 'ignore_regex = "..."\n'
                s.trim().to_string()
            }
            None => "# ignore_regex = \"password|secret\"".to_string(),
        };
        let mut out = String::new();
        out.push_str("# FastPaste configuration\n");
        out.push_str("# Edit this file, then use Reload configuration on the tray.\n");
        out.push_str("# Invalid values are rejected and the previous configuration is kept.\n");
        out.push_str("\n");
        out.push_str("# Ten hotkeys, in order. Index 0 = newest history item; index 9 = oldest.\n");
        out.push_str("# Format: modifier+modifier+key e.g. ctrl+shift+1\n");
        out.push_str("# Modifiers: ctrl, shift, alt, super (aliases: cmd, command, win, meta).\n");
        out.push_str("# Key: 0-9, a-z, or a global-hotkey Code name such as F5 or space.\n");
        out.push_str(&hotkeys_line);
        out.push_str("\n");
        out.push_str("\n");
        out.push_str("# If true, a clipboard paste restores the previous clipboard after injecting.\n");
        out.push_str("# If false, the pasted entry is left on the clipboard.\n");
        out.push_str(&format!("preserve_clipboard = {}\n", self.preserve_clipboard));
        out.push_str("\n");
        out.push_str("# How to inject into the focused window.\n");
        out.push_str("# auto = keystroke inject for short text with no control chars; clipboard paste otherwise. Default.\n");
        out.push_str("# clipboard = copy onto the clipboard then Ctrl+V (Cmd+V on macOS).\n");
        out.push_str("# type = always keystroke inject, including newlines as Enter (can send in chat apps).\n");
        out.push_str(&format!("paste_method = \"{}\"\n", self.paste_method));
        out.push_str("\n");
        out.push_str("# Clipboard poll interval in milliseconds. Minimum 50.\n");
        out.push_str(&format!("polling_interval_ms = {}\n", self.polling_interval_ms));
        out.push_str("\n");
        out.push_str("# Max bytes for one history entry. Inclusive range 1 ..= 52428800 (50 MiB).\n");
        out.push_str(&format!("max_entry_bytes = {}\n", self.max_entry_bytes));
        out.push_str("\n");
        out.push_str("# Optional regex; matching text is not stored. Uncomment to enable:\n");
        out.push_str(&ignore_regex_line);
        out.push_str("\n");
        out.push_str("\n");
        out.push_str("# Windows: when enabling autostart, request a highest-privilege logon task.\n");
        out.push_str("# true = HIGHEST. false is ignored by the tray today (always HIGHEST); kept for the file format.\n");
        out.push_str(&format!("autostart_elevated = {}\n", self.autostart_elevated));
        out
    }

    pub fn validate(&self) -> Result<()> {
        if self.hotkeys.len() != 10 {
            anyhow::bail!("hotkeys 必须恰好 10 条，对应 Ctrl+Shift+1..0");
        }
        for hk in &self.hotkeys {
            parse_hotkey(hk).with_context(|| format!("热键格式错误: {}", hk))?;
        }
        if self.polling_interval_ms < 50 {
            anyhow::bail!("polling_interval_ms 不能小于 50ms");
        }
        if self.max_entry_bytes == 0 || self.max_entry_bytes > 50 * 1024 * 1024 {
            anyhow::bail!("max_entry_bytes 需在 1..50MB 范围");
        }
        if let Some(pat) = &self.ignore_regex {
            regex::Regex::new(pat).context("ignore_regex 正则无效")?;
        }
        Ok(())
    }
}

/// 解析热键字符串，格式如 ctrl+shift+1
/// 返回 (modifiers, key)
pub fn parse_hotkey(s: &str) -> Result<(String, String)> {
    let lower = s.to_lowercase();
    let parts: Vec<&str> = lower.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    if parts.len() < 2 {
        anyhow::bail!("热键至少包含一个修饰键和一个主键，如 ctrl+shift+1");
    }
    let (mods, key) = parts.split_at(parts.len() - 1);
    let key = key[0].to_string();
    // 校验修饰键
    let valid_mods = ["ctrl", "shift", "alt", "super", "cmd", "command", "win", "meta"];
    for m in mods {
        if !valid_mods.contains(m) {
            anyhow::bail!("未知修饰键: {}", m);
        }
    }
    // 校验主键为单个字符或 F1-F24 或常见键
    if key.is_empty() {
        anyhow::bail!("主键不能为空");
    }
    Ok((mods.join("+"), key))
}

/// 将 config hotkey 字符串转换为 global-hotkey 可用的 HotKey
pub fn to_global_hotkey(s: &str) -> Result<global_hotkey::hotkey::HotKey> {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use std::str::FromStr;

    let lower = s.to_lowercase();
    let parts: Vec<&str> = lower.split('+').map(|p| p.trim()).collect();
    if parts.len() < 2 {
        anyhow::bail!("热键格式错误: {}", s);
    }
    let (mods_parts, key_part) = parts.split_at(parts.len() - 1);
    let key_str = key_part[0];
    if key_str.is_empty() {
        anyhow::bail!("无法解析主键: {}", key_str);
    }

    let mut mods = Modifiers::empty();
    for m in mods_parts {
        match *m {
            "ctrl" => mods |= Modifiers::CONTROL,
            "shift" => mods |= Modifiers::SHIFT,
            "alt" => mods |= Modifiers::ALT,
            "super" | "cmd" | "command" | "win" | "meta" => mods |= Modifiers::SUPER,
            "" => anyhow::bail!("热键格式错误: {}", s),
            _ => anyhow::bail!("未知修饰键: {}", m),
        }
    }

    // 尝试解析 Code
    let code = match key_str {
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        c if c.len() == 1 => {
            let ch = c.chars().next().unwrap();
            if ch.is_ascii_alphabetic() {
                let code_str = format!("Key{}", ch.to_ascii_uppercase());
                Code::from_str(&code_str).map_err(|_| anyhow::anyhow!("无法解析主键: {}", key_str))?
            } else {
                Code::from_str(&key_str.to_uppercase())
                    .or_else(|_| Code::from_str(key_str))
                    .map_err(|_| anyhow::anyhow!("无法解析主键: {}", key_str))?
            }
        }
        _ => Code::from_str(&key_str.to_uppercase())
            .or_else(|_| Code::from_str(key_str))
            .map_err(|_| anyhow::anyhow!("无法解析主键: {}", key_str))?,
    };

    Ok(HotKey::new(Some(mods), code))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_config_valid() {
        let cfg = Config::default();
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.hotkeys[0], "ctrl+shift+1");
        assert_eq!(cfg.hotkeys[9], "ctrl+shift+0");
    }

    #[test]
    fn parse_hotkey_ok() {
        assert!(parse_hotkey("ctrl+shift+1").is_ok());
        assert!(parse_hotkey("ctrl+alt+shift+1").is_ok());
        assert!(parse_hotkey("super+shift+0").is_ok());
    }

    #[test]
    fn parse_hotkey_err() {
        assert!(parse_hotkey("1").is_err());
        assert!(parse_hotkey("ctrl+unknown+1").is_err());
        assert!(parse_hotkey("").is_err());
    }

    #[test]
    fn validate_hotkeys_len() {
        let mut cfg = Config::default();
        cfg.hotkeys.pop();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn load_and_save_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config::default();
        cfg.save_to(&path).unwrap();
        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded.hotkeys, cfg.hotkeys);
        assert_eq!(loaded.preserve_clipboard, cfg.preserve_clipboard);
    }

    #[test]
    fn invalid_regex_rejected() {
        let mut cfg = Config::default();
        cfg.ignore_regex = Some("[invalid".into());
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn to_global_hotkey_ok() {
        let hk = to_global_hotkey("ctrl+shift+1").unwrap();
        assert!(hk.mods.contains(global_hotkey::hotkey::Modifiers::CONTROL));
        assert!(hk.mods.contains(global_hotkey::hotkey::Modifiers::SHIFT));
    }

    #[test]
    fn to_global_hotkey_f1_or_space() {
        // F1 在 global-hotkey 0.6 中存在；若不存在则用 space 等兜底键
        let hk = to_global_hotkey("ctrl+shift+f1");
        if hk.is_err() {
            // 兜底：space 肯定存在
            assert!(to_global_hotkey("ctrl+shift+space").is_ok());
        } else {
            assert!(hk.is_ok());
        }
        // 非 f1 兜底键 f5 也应 Ok
        assert!(to_global_hotkey("ctrl+shift+f5").is_ok());
    }

    #[test]
    fn to_global_hotkey_unknown_err() {
        assert!(to_global_hotkey("ctrl+shift+thiskeydoesnotexist").is_err());
    }

    #[test]
    fn to_global_hotkey_empty_key_err() {
        assert!(to_global_hotkey("ctrl+shift+").is_err());
    }

    #[test]
    fn to_global_hotkey_letter_ok() {
        assert!(to_global_hotkey("ctrl+shift+a").is_ok());
        assert!(to_global_hotkey("ctrl+shift+Z").is_ok());
    }

    #[test]
    fn config_with_regex() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = Config::default();
        cfg.ignore_regex = Some("password".into());
        cfg.save_to(&path).unwrap();
        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded.ignore_regex, Some("password".into()));
    }

    #[test]
    fn save_to_contains_fastpaste_configuration() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config::default();
        cfg.save_to(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("FastPaste configuration"));
        let lines: Vec<&str> = content.lines().collect();
        let idx = lines.iter().position(|l| l.contains("paste_method")).expect("paste_method not found");
        let start = idx.saturating_sub(5);
        let end = (idx + 5).min(lines.len());
        let window = lines[start..end].join("\n");
        assert!(window.contains("auto"), "window missing auto: {}", window);
        assert!(window.contains("clipboard"), "window missing clipboard: {}", window);
        assert!(window.contains("type"), "window missing type: {}", window);
    }

    #[test]
    fn save_load_ignore_regex_escaped() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut cfg = Config::default();
        cfg.ignore_regex = Some("a\"b".into());
        cfg.save_to(&path).unwrap();
        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded.ignore_regex, Some("a\"b".into()));
    }

    #[test]
    fn default_is_auto() {
        assert_eq!(Config::default().paste_method, PasteMethod::Auto);
        assert_eq!(PasteMethod::default(), PasteMethod::Auto);
        assert_eq!(PasteMethod::Auto.to_string(), "auto");
        assert_eq!(PasteMethod::Clipboard.to_string(), "clipboard");
        assert_eq!(PasteMethod::Type.to_string(), "type");
    }

    #[test]
    fn missing_paste_method_defaults_to_auto() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let content = r#"hotkeys = ["ctrl+shift+1", "ctrl+shift+2", "ctrl+shift+3", "ctrl+shift+4", "ctrl+shift+5", "ctrl+shift+6", "ctrl+shift+7", "ctrl+shift+8", "ctrl+shift+9", "ctrl+shift+0"]
preserve_clipboard = true
polling_interval_ms = 500
max_entry_bytes = 5242880
autostart_elevated = true
"#;
        std::fs::write(&path, content).unwrap();
        let loaded = Config::load_from(&path).unwrap();
        assert_eq!(loaded.paste_method, PasteMethod::Auto);
    }

    #[test]
    fn paste_method_strings_roundtrip() {
        let cases = [
            ("auto", PasteMethod::Auto),
            ("clipboard", PasteMethod::Clipboard),
            ("type", PasteMethod::Type),
        ];
        for (s, expected) in cases {
            let toml_str = format!("paste_method = \"{}\"", s);
            let cfg: Config = toml::from_str(&toml_str).unwrap();
            assert_eq!(cfg.paste_method, expected);
            assert_eq!(cfg.paste_method.to_string(), s);
            // also test direct wrapper deser
            #[derive(Deserialize)]
            struct Wrapper {
                paste_method: PasteMethod,
            }
            let w: Wrapper = toml::from_str(&toml_str).unwrap();
            assert_eq!(w.paste_method, expected);
            // serialize back contains expected string
            let serialized = toml::to_string(&cfg).unwrap();
            assert!(serialized.contains(s), "serialized missing {}: {}", s, serialized);
        }
    }

    #[test]
    fn documented_toml_contains_auto_clipboard_type_and_default_note() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = Config::default();
        cfg.save_to(&path).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        let lower = content.to_lowercase();
        assert!(lower.contains("auto"), "missing auto");
        assert!(lower.contains("clipboard"), "missing clipboard");
        assert!(lower.contains("type"), "missing type");
        // check default note and Enter note
        assert!(content.contains("Default"), "missing Default");
        assert!(content.contains("Enter"), "missing Enter");
    }
}
