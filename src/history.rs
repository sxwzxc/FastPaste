use regex::Regex;

/// 单条条目字节上限 5MB
pub const MAX_ENTRY_BYTES: usize = 5 * 1024 * 1024;
/// 历史容量
pub const HISTORY_CAPACITY: usize = 10;

/// 历史：环形队列，去重后置顶，空或纯空白不入队
#[derive(Debug, Clone)]
pub struct History {
    entries: Vec<String>,
    ignore_regex: Option<Regex>,
    max_bytes: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new(None)
    }
}

impl History {
    pub fn new(ignore_pattern: Option<String>) -> Self {
        Self::with_limits(ignore_pattern, MAX_ENTRY_BYTES)
    }

    pub fn with_limits(ignore_pattern: Option<String>, max_bytes: usize) -> Self {
        let regex = ignore_pattern
            .as_deref()
            .and_then(|p| Regex::new(p).ok());
        Self {
            entries: Vec::with_capacity(HISTORY_CAPACITY),
            ignore_regex: regex,
            max_bytes: max_bytes.max(1),
        }
    }

    pub fn with_regex(regex: Option<Regex>) -> Self {
        Self {
            entries: Vec::with_capacity(HISTORY_CAPACITY),
            ignore_regex: regex,
            max_bytes: MAX_ENTRY_BYTES,
        }
    }

    pub fn set_max_bytes(&mut self, max_bytes: usize) {
        self.max_bytes = max_bytes.max(1);
    }

    /// 尝试将文本推入历史
    /// 返回 true 表示已入队（或去重后置顶），false 表示被忽略
    pub fn push(&mut self, text: String) -> bool {
        // 空或纯空白不入队
        if text.trim().is_empty() {
            return false;
        }
        // 超长不入队（由调用方决定是否气泡提示，这里静默忽略）
        if text.as_bytes().len() > self.max_bytes {
            return false;
        }
        // 敏感内容过滤
        if let Some(re) = &self.ignore_regex {
            if re.is_match(&text) {
                return false;
            }
        }
        // 去重后置顶
        if let Some(pos) = self.entries.iter().position(|e| e == &text) {
            self.entries.remove(pos);
            self.entries.insert(0, text);
            return true;
        }
        // 新条目插入队首
        self.entries.insert(0, text);
        if self.entries.len() > HISTORY_CAPACITY {
            self.entries.pop();
        }
        true
    }

    /// 按内部索引获取：0 = 最新
    pub fn get(&self, index: usize) -> Option<&String> {
        self.entries.get(index)
    }

    /// 按热键数字获取：1..9,0 其中 1=最新, 0=最旧(第10条)
    pub fn get_by_hotkey_digit(&self, digit: u8) -> Option<&String> {
        let idx = match digit {
            1..=9 => (digit - 1) as usize,
            0 => 9,
            _ => return None,
        };
        self.get(idx)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// 预览：前 n 字符脱敏显示，超长以 ... 截断，换行转空格
    pub fn preview(&self, index: usize, max_chars: usize) -> Option<String> {
        self.get(index).map(|s| Self::format_preview(s, max_chars))
    }

    pub fn format_preview(s: &str, max_chars: usize) -> String {
        let sanitized: String = s.chars().map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect();
        let truncated: String = sanitized.chars().take(max_chars).collect();
        if sanitized.chars().count() > max_chars {
            format!("{}...", truncated)
        } else {
            truncated
        }
    }

    pub fn set_ignore_regex(&mut self, pattern: Option<String>) {
        self.ignore_regex = pattern.as_deref().and_then(|p| Regex::new(p).ok());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_basic_fifo() {
        let mut h = History::default();
        for i in 0..10 {
            assert!(h.push(format!("text {}", i)));
        }
        assert_eq!(h.len(), 10);
        assert_eq!(h.get(0).unwrap(), "text 9");
        assert_eq!(h.get(9).unwrap(), "text 0");
        // 第11条，FIFO 丢弃最旧
        h.push("text 10".to_string());
        assert_eq!(h.len(), 10);
        assert_eq!(h.get(0).unwrap(), "text 10");
        assert_eq!(h.get(9).unwrap(), "text 1");
    }

    #[test]
    fn dedup_moves_to_front() {
        let mut h = History::default();
        h.push("a".into());
        h.push("b".into());
        h.push("c".into());
        assert_eq!(h.entries(), &["c", "b", "a"]);
        // 重复 b，应置顶
        assert!(h.push("b".into()));
        assert_eq!(h.entries(), &["b", "c", "a"]);
        assert_eq!(h.len(), 3);
    }

    #[test]
    fn dedup_same_continuous_single_entry() {
        let mut h = History::default();
        for _ in 0..10 {
            h.push("same".into());
        }
        assert_eq!(h.len(), 1);
        assert_eq!(h.get(0).unwrap(), "same");
    }

    #[test]
    fn ignore_empty_and_whitespace() {
        let mut h = History::default();
        assert!(!h.push("".into()));
        assert!(!h.push("   ".into()));
        assert!(!h.push("\n\t".into()));
        assert_eq!(h.len(), 0);
        assert!(h.push(" hello ".into()));
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn ignore_over_5mb() {
        let mut h = History::default();
        let large = "a".repeat(MAX_ENTRY_BYTES + 1);
        assert!(!h.push(large));
        assert_eq!(h.len(), 0);
        let ok = "a".repeat(MAX_ENTRY_BYTES);
        assert!(h.push(ok));
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn ignore_regex() {
        let mut h = History::new(Some(r"password|secret".into()));
        assert!(!h.push("my password is 123".into()));
        assert!(!h.push("secret token".into()));
        assert!(h.push("hello world".into()));
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn hotkey_digit_mapping() {
        let mut h = History::default();
        for i in 1..=10 {
            h.push(format!("item{}", i));
        }
        // h: [item10, item9, ..., item1]
        assert_eq!(h.get_by_hotkey_digit(1).unwrap(), "item10");
        assert_eq!(h.get_by_hotkey_digit(2).unwrap(), "item9");
        assert_eq!(h.get_by_hotkey_digit(9).unwrap(), "item2");
        assert_eq!(h.get_by_hotkey_digit(0).unwrap(), "item1");
        assert!(h.get_by_hotkey_digit(11).is_none());
    }

    #[test]
    fn preview_truncation() {
        let s = "hello\nworld this is a long text";
        let p = History::format_preview(s, 5);
        assert_eq!(p, "hello...");
        let p2 = History::format_preview("short", 10);
        assert_eq!(p2, "short");
        let p3 = History::format_preview("a\nb\r\nc", 10);
        assert_eq!(p3, "a b  c");
    }

    #[test]
    fn default_max_bytes_limits() {
        let mut h = History::new(None);
        let large = "a".repeat(MAX_ENTRY_BYTES + 1);
        assert!(!h.push(large));
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn with_limits_custom() {
        let mut h = History::with_limits(None, 8);
        assert!(!h.push("123456789".into()));
        assert_eq!(h.len(), 0);
        assert!(h.push("12345678".into()));
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn set_max_bytes_updates() {
        let mut h = History::new(None);
        h.set_max_bytes(3);
        assert!(!h.push("abcd".into()));
        assert_eq!(h.len(), 0);
        assert!(h.push("abc".into()));
        assert_eq!(h.len(), 1);
    }
}
