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
    let task_exists = std::process::Command::new("schtasks")
        .args(["/Query", "/TN", &task_name()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if task_exists {
        return true;
    }
    // 兼容旧注册表残留：若任务不存在但 Run 键存在，也视为已开启
    let reg_exists = std::process::Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "FastPaste",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    reg_exists
}

#[cfg(target_os = "windows")]
fn set_enabled_windows(enable: bool, _elevated: bool) -> Result<()> {
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
            "HIGHEST".to_string(),
            "/F".to_string(),
        ];
        let output = std::process::Command::new("schtasks")
            .args(&args)
            .output()
            .context("执行 schtasks 创建任务失败")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            anyhow::bail!("创建自启任务失败: {} {}", stdout, stderr);
        }
        Ok(())
    } else {
        let _ = std::process::Command::new("schtasks")
            .args(["/Delete", "/TN", &task_name(), "/F"])
            .output();
        // 同时清理可能残留的注册表值
        let _ = set_enabled_windows_registry(false);
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn set_enabled_windows_registry(_enable: bool) -> Result<()> {
    // 仅用于清理残留的 Run 键；开启自启不再写入注册表，关闭时只 delete
    let key = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
    let _ = std::process::Command::new("reg")
        .args(["delete", key, "/v", "FastPaste", "/f"])
        .output();
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
