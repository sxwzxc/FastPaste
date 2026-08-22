use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

/// 配置文件一次采样：存在（含内容哈希）或缺失。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sample {
    Present(u64),
    Missing,
}

/// 轮询式配置变更探测（ADR-0005）：不引入文件监视依赖，
/// 由剪贴板轮询线程周期驱动，内部节流至约每秒一次；
/// 连续两次采样一致且不同于已应用内容才判定为稳定变更。
/// 直接以内容哈希判定变更，不依赖 mtime/size——粗粒度时间戳
/// 会漏检同尺寸的快速连写；配置文件极小，整读开销可忽略。
pub struct ConfigProbe {
    /// 最近一次已应用、或已裁定拒绝的内容基线
    last_applied: Option<Sample>,
    /// 稳定窗口内的上一拍采样
    pending: Option<Sample>,
    last_check: Option<Instant>,
}

impl Default for ConfigProbe {
    fn default() -> Self {
        Self::new()
    }
}

/// 两次探测之间的最小间隔
const CHECK_INTERVAL: Duration = Duration::from_secs(1);

impl ConfigProbe {
    pub fn new() -> Self {
        Self {
            last_applied: None,
            pending: None,
            last_check: None,
        }
    }

    /// 轮询线程周期调用，内部节流。返回 true 表示出现稳定变更，应请求主线程重载。
    pub fn probe(&mut self, path: &Path) -> bool {
        self.probe_at(path, Instant::now())
    }

    fn probe_at(&mut self, path: &Path, now: Instant) -> bool {
        if let Some(t) = self.last_check {
            if now.duration_since(t) < CHECK_INTERVAL {
                return false;
            }
        }
        self.last_check = Some(now);
        match try_sample(path) {
            Ok(s) => self.observe(s),
            // 瞬时 IO 错误（如 Windows 写入期的共享冲突）绝不视为缺失：
            // 若误判成“文件被删”，两拍之后就会触发重建默认配置，覆盖用户文件
            Err(e) => {
                log::debug!("配置采样暂不可用，沿用上一采样: {}", e);
                false
            }
        }
    }

    /// 纯状态机：首个采样作为基线直接采纳；此后连续两拍一致且异于基线才触发一次。
    /// 触发只清空待定采样、不改基线：基线的唯一记账入口是 mark_settled，
    /// 否则“触发即改基线”会让主线程的 matches_applied 把新事件误判为已应用。
    /// 结算前同一内容可能再次触发（约每两拍一次），由请求标志位合并、管线结算收尾。
    fn observe(&mut self, s: Sample) -> bool {
        if self.last_applied.is_none() {
            self.last_applied = Some(s);
            return false;
        }
        if self.last_applied.as_ref() == Some(&s) {
            self.pending = None;
            return false;
        }
        if self.pending.as_ref() != Some(&s) {
            self.pending = Some(s);
            return false;
        }
        self.pending = None;
        true
    }

    /// 重载管线处理完毕后调用（成功应用、或自动路径裁定拒绝均算“已结算”）：
    /// 以当前磁盘内容为新基线并清空待定采样，保证同一内容至多触发一次。
    pub fn mark_settled(&mut self, path: &Path) {
        match try_sample(path) {
            Ok(s) => self.last_applied = Some(s),
            Err(e) => log::warn!("记录配置基线失败: {}", e),
        }
        self.pending = None;
    }

    /// 当前磁盘内容是否等于已应用基线；主线程用于丢弃排队中的重复自动重载事件。
    pub fn matches_applied(&mut self, path: &Path) -> bool {
        match try_sample(path) {
            Ok(s) => self.last_applied.as_ref() == Some(&s),
            Err(_) => false,
        }
    }
}

fn try_sample(path: &Path) -> io::Result<Sample> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let mut hasher = DefaultHasher::new();
            hasher.write(&bytes);
            Ok(Sample::Present(hasher.finish()))
        }
        // 文件确实不存在 → 缺失态。其余错误（含写入期共享冲突等瞬时失败）
        // 向上传播，由调用方保持上一采样，绝不误判为缺失而重建默认配置
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Sample::Missing),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    /// 单调推进的测试时钟：跨调用共享同一游标，每拍推进略超 CHECK_INTERVAL。
    /// 修复点：旧实现每次调用重置 t0，导致前一调用留下的 last_check
    /// 吞掉后续首拍（节流窗口内直接跳过采样），触发断言随机失效。
    struct Ticks {
        t: Instant,
    }

    impl Ticks {
        fn new() -> Self {
            Self { t: Instant::now() }
        }

        fn run(&mut self, pr: &mut ConfigProbe, path: &Path, n: usize) -> Vec<bool> {
            (0..n).map(|_| self.step(pr, path)).collect()
        }

        fn step(&mut self, pr: &mut ConfigProbe, path: &Path) -> bool {
            self.t += CHECK_INTERVAL + Duration::from_millis(1);
            pr.probe_at(path, self.t)
        }
    }

    #[test]
    fn first_samples_adopt_baseline_without_trigger() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "a = 1").unwrap();
        let mut pr = ConfigProbe::new();
        let mut tk = Ticks::new();
        assert_eq!(tk.run(&mut pr, &path, 3), vec![false, false, false]);
    }

    #[test]
    fn edit_fires_repeatedly_until_settled_then_quiets() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "a = 1").unwrap();
        let mut pr = ConfigProbe::new();
        let mut tk = Ticks::new();
        tk.run(&mut pr, &path, 2);

        fs::write(&path, "a = 2").unwrap();
        // 第一拍看到新内容（待定），第二拍确认 → 触发；基线未结算前同一内容会再触发，
        // 现实中由主线程的请求标志位合并为一次，并在管线结束时 mark_settled 收尾
        assert_eq!(tk.run(&mut pr, &path, 4), vec![false, true, false, true]);

        // 模拟管线结算（成功应用或自动路径裁定拒绝）后归于安静
        pr.mark_settled(&path);
        assert_eq!(tk.run(&mut pr, &path, 3), vec![false, false, false]);
    }

    #[test]
    fn rapid_flapping_never_fires() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "a = 1").unwrap();
        let mut pr = ConfigProbe::new();
        let mut tk = Ticks::new();
        tk.run(&mut pr, &path, 2);

        for i in 2..8 {
            fs::write(&path, format!("a = {i}")).unwrap();
            assert!(!tk.step(&mut pr, &path), "第 {i} 次编辑不应单独触发");
        }
    }

    #[test]
    fn deletion_fires_and_recreated_file_quiets_after_mark_settled() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "original").unwrap();
        let mut pr = ConfigProbe::new();
        let mut tk = Ticks::new();
        tk.run(&mut pr, &path, 2);

        fs::remove_file(&path).unwrap();
        assert_eq!(
            tk.run(&mut pr, &path, 2),
            vec![false, true],
            "删除应作为稳定变更触发"
        );

        // 模拟主线程重载完成：load_from 已重建默认文件并 mark_settled
        fs::write(&path, "defaults").unwrap();
        pr.mark_settled(&path);
        assert_eq!(tk.run(&mut pr, &path, 3), vec![false, false, false]);
    }

    #[test]
    fn transient_read_error_is_not_treated_as_missing() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "stable").unwrap();
        let mut pr = ConfigProbe::new();
        let mut tk = Ticks::new();
        tk.run(&mut pr, &path, 2);

        // 用同名目录制造 read 失败（非 NotFound）的瞬时错误态
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        assert_eq!(
            tk.run(&mut pr, &path, 3),
            vec![false, false, false],
            "读取失败必须保持上一采样，不得当作缺失而触发重建"
        );

        // 恢复原内容后照常工作
        fs::remove_dir(&path).unwrap();
        fs::write(&path, "stable").unwrap();
        assert_eq!(tk.run(&mut pr, &path, 3), vec![false, false, false]);
    }

    #[test]
    fn matches_applied_dedupes_until_content_changes() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "v1").unwrap();
        let mut pr = ConfigProbe::new();
        let mut tk = Ticks::new();
        tk.run(&mut pr, &path, 2);
        assert!(pr.matches_applied(&path));

        fs::write(&path, "v2-longer-content").unwrap();
        assert!(!pr.matches_applied(&path));
        pr.mark_settled(&path);
        assert!(pr.matches_applied(&path));
    }
}
