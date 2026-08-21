use anyhow::{Context, Result};
use std::path::PathBuf;

/// 检查是否已开启自启
pub fn is_enabled() -> bool {
    #[cfg(target_os = "windows")]
    {
        is_enabled_windows()
    }
    #[cfg(target_os = "macos")]
    {
        is_enabled_macos()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        false
    }
}

pub fn set_enabled(enable: bool, elevated: bool) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        set_enabled_windows(enable, elevated)
    }
    #[cfg(target_os = "macos")]
    {
        set_enabled_macos(enable)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (enable, elevated);
        anyhow::bail!("当前平台不支持自启")
    }
}

fn exe_path() -> Result<PathBuf> {
    std::env::current_exe().context("获取当前可执行路径失败")
}

#[cfg(target_os = "windows")]
fn task_name() -> String {
    "FastPaste".to_string()
}

#[cfg(target_os = "windows")]
fn is_enabled_windows() -> bool {
    let output = std::process::Command::new("schtasks")
        .args(["/Query", "/TN", &task_name()])
        .output();
    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

#[cfg(target_os = "windows")]
fn set_enabled_windows(enable: bool, elevated: bool) -> Result<()> {
    let exe = exe_path()?;
    let exe_str = exe.display().to_string();
    if enable {
        // 先删除旧任务（忽略错误）
        let _ = std::process::Command::new("schtasks")
            .args(["/Delete", "/TN", &task_name(), "/F"])
            .output();
        let args = vec![
            "/Create".to_string(),
            "/TN".to_string(),
            task_name(),
            "/TR".to_string(),
            format!("\"{}\"", exe_str),
            "/SC".to_string(),
            "ONLOGON".to_string(),
            "/RL".to_string(),
            if elevated { "HIGHEST".to_string() } else { "LIMITED".to_string() },
            "/F".to_string(),
        ];
        // 若 elevated，需要以管理员身份创建，schtasks 会自动触发 UAC（若当前非提权会失败）
        let output = std::process::Command::new("schtasks")
            .args(&args)
            .output()
            .context("执行 schtasks 创建任务失败")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            // 回退到注册表方式（非提权）
            if elevated {
                log::warn!("提权任务创建失败，回退到普通自启: {} {}", stdout, stderr);
                return set_enabled_windows_registry(true);
            }
            anyhow::bail!("创建自启任务失败: {} {}", stdout, stderr);
        }
        Ok(())
    } else {
        let _ = std::process::Command::new("schtasks")
            .args(["/Delete", "/TN", &task_name(), "/F"])
            .output();
        // 同时清理注册表残留
        let _ = set_enabled_windows_registry(false);
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn set_enabled_windows_registry(enable: bool) -> Result<()> {
    // 使用 reg 命令操作 HKCU\Software\Microsoft\Windows\CurrentVersion\Run
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    if enable {
        let exe = exe_path()?;
        let output = std::process::Command::new("reg")
            .args(["add", key, "/v", "FastPaste", "/t", "REG_SZ", "/d", &exe.display().to_string(), "/f"])
            .output()
            .context("reg add 失败")?;
        if !output.status.success() {
            anyhow::bail!("注册表自启设置失败");
        }
    } else {
        let _ = std::process::Command::new("reg")
            .args(["delete", key, "/v", "FastPaste", "/f"])
            .output();
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn is_enabled_macos() -> bool {
    if let Some(home) = dirs::home_dir() {
        let plist = home.join("Library/LaunchAgents/com.fastpaste.plist");
        return plist.exists();
    }
    false
}

#[cfg(target_os = "macos")]
fn set_enabled_macos(enable: bool) -> Result<()> {
    let home = dirs::home_dir().context("无法获取 home 目录")?;
    let plist_path = home.join("Library/LaunchAgents/com.fastpaste.plist");
    if enable {
        let exe = exe_path()?;
        let plist_content = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key><string>com.fastpaste</string>
    <key>ProgramArguments</key><array><string>{}</string></array>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><false/>
</dict>
</plist>"#,
            exe.display()
        );
        if let Some(parent) = plist_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&plist_path, plist_content)?;
        // 加载
        let _ = std::process::Command::new("launchctl")
            .args(["load", "-w", &plist_path.display().to_string()])
            .output();
    } else {
        let _ = std::process::Command::new("launchctl")
            .args(["unload", &plist_path.display().to_string()])
            .output();
        let _ = std::fs::remove_file(&plist_path);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exe_path_ok() {
        let p = exe_path();
        // 在测试环境中 current_exe 指向 test binary，仍应成功
        assert!(p.is_ok());
    }
}
