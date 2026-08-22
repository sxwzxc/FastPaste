#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod clipboard;
mod config;
mod config_watch;
mod dialog;
mod history;
mod hotkey;
mod icon_data;
mod paste;
mod single_instance;
mod tray;

use anyhow::Result;
use global_hotkey::GlobalHotKeyEvent;
use muda::MenuEvent;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoop};

use crate::app::{handle_paste, spawn_clipboard_thread, AppState};
use crate::config::Config;
use crate::tray::{Tray, TrayEvent};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    env_logger::init();

    let _guard = match single_instance::InstanceGuard::new("fastpaste-singleton") {
        Ok(g) => g,
        Err(_) => {
            dialog::show_warning("FastPaste", "FastPaste 已在运行");
            std::process::exit(0);
        }
    };

    #[cfg(target_os = "windows")]
    {
        if !is_elevated::is_elevated() {
            log::info!("当前未提权，尝试以管理员身份重启...");
            let _ = try_elevate();
        }
    }

    let first_run = Config::config_path().map(|p| !p.exists()).unwrap_or(false);

    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            log::error!("加载配置失败: {}", e);
            dialog::show_error("FastPaste 配置错误", &format!("{}\n将使用默认配置", e));
            Config::default()
        }
    };

    let state = AppState::new(config);
    let history = state.history.clone();
    let enabled = state.enabled.clone();
    let config_arc = state.config.clone();

    spawn_clipboard_thread(state.clone());

    let event_loop = EventLoop::new().expect("创建 winit EventLoop 失败");

    let tray = Arc::new(Tray::new(enabled.clone(), history.clone()).expect("创建托盘失败"));
    // 启动时按配置决定显隐（tray-icon 0.19 无 with_visible，需 build 后 set_visible；false 时会有短暂闪现，已接受）
    if !config_arc.lock().show_tray_icon {
        let _ = tray.set_visible(false);
    }

    let hk_manager_inner = hotkey::HotkeyManager::new().expect("创建热键管理器失败");
    let hk_manager = Arc::new(parking_lot::Mutex::new(hk_manager_inner));
    {
        let mut hk = hk_manager.lock();
        let cfg = config_arc.lock().clone();
        let failures = hk.register_from_config(&cfg);
        if !failures.is_empty() {
            let msg = failures
                .iter()
                .map(|(d, s)| format!("{}: {}", d, s))
                .collect::<Vec<_>>()
                .join("\n");
            dialog::show_warning(
                "FastPaste 热键冲突",
                &format!("以下热键注册失败，将跳过：\n{}", msg),
            );
        }
    }

    if first_run {
        dialog::show_info(
            "FastPaste",
            "FastPaste 已在后台运行。\n\n• 复制任意文本自动入队（最近10条）\n• Ctrl+Shift+1..0 直接粘贴\n• 右键托盘图标可启用/停用、打开配置、重载配置、打开自启\n\n配置文件位于系统配置目录 FastPaste/config.toml",
        );
    }

    let app_state = state.clone();
    let tray_clone = tray.clone();
    let hk_clone = hk_manager.clone();
    let enabled_clone = enabled.clone();
    let config_clone = config_arc.clone();
    let pending_notice_clone = state.pending_notice.clone();

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(200),
        ));

        if let Event::AboutToWait = event {
            tray_clone.refresh_history_preview();
            // 主线程处理粘贴失败提示（工作线程通过 pending_notice 传递）
            if let Some((title, body)) = pending_notice_clone.lock().take() {
                dialog::show_warning(&title, &body);
            }
            // 自动重载：轮询线程探测到稳定变更后置位，这里统一走重载管线（ADR-0005）
            if app_state.auto_reload_pending.swap(false, Ordering::SeqCst) {
                let already_applied = Config::config_path()
                    .ok()
                    .map(|p| app_state.config_probe.lock().matches_applied(&p))
                    .unwrap_or(true);
                if already_applied {
                    log::debug!("自动重载请求已过期（内容已被应用），忽略");
                } else {
                    run_reload(
                        &app_state,
                        &hk_clone,
                        &enabled_clone,
                        &tray_clone,
                        ReloadSource::Auto,
                    );
                }
            }
            // 处理托盘菜单事件
            while let Ok(menu_event) = MenuEvent::receiver().try_recv() {
                let tray_event = tray_clone.handle_menu_event(&menu_event.id.0);
                match tray_event {
                    TrayEvent::Toggle(en) => {
                        log::info!("切换启用状态: {}", en);
                        let cfg = config_clone.lock().clone();
                        let fails = hk_clone.lock().set_enabled(en, &cfg);
                        if !fails.is_empty() {
                            let msg = fails.iter().map(|(d,s)| format!("{}: {}", d, s)).collect::<Vec<_>>().join("\n");
                            dialog::show_warning("热键冲突", &msg);
                        }
                    }
                    TrayEvent::OpenConfig => {
                        if let Ok(path) = Config::config_path() {
                            if let Err(e) = open_config_file(&path) {
                                dialog::show_error("打开配置失败", &e.to_string());
                            }
                        }
                    }
                    TrayEvent::ReloadConfig => {
                        run_reload(
                            &app_state,
                            &hk_clone,
                            &enabled_clone,
                            &tray_clone,
                            ReloadSource::Manual,
                        );
                    }
                    TrayEvent::ToggleAutostart(enable) => {
                        match autostart::set_enabled(enable, true) {
                            Ok(_) => {
                                log::info!("自启已切换: {}", enable);
                            }
                            Err(e) => {
                                dialog::show_error("自启失败", &e.to_string());
                                tray_clone.set_autostart_checked(!enable);
                            }
                        }
                    }
                    TrayEvent::ElevatedRestart => {
                        #[cfg(target_os = "windows")]
                        {
                            let _ = try_elevate();
                        }
                        #[cfg(not(target_os = "windows"))]
                        {
                            dialog::show_info("FastPaste", "当前平台无需提权重启");
                        }
                    }
                    TrayEvent::Diagnosis => {
                        show_diagnosis();
                    }
                    TrayEvent::About => {
                        dialog::show_info(
                            &format!("FastPaste {}", VERSION),
                            &format!(
                                "跨平台静默剪贴板管理器\n\n• 监听剪贴板，保留最近 10 条纯文本\n• Ctrl+Shift+1..0 直接粘贴\n• 托盘右键管理\n\n配置: FastPaste/config.toml\n版本: {}",
                                VERSION
                            ),
                        );
                    }
                    TrayEvent::Exit => {
                        elwt.exit();
                    }
                    TrayEvent::PasteHistory(digit) => {
                        handle_paste(&app_state.clone(), digit);
                    }
                    TrayEvent::None => {}
                }
            }

            // 处理热键粘贴
            while let Ok(hk_event) = GlobalHotKeyEvent::receiver().try_recv() {
                let digit_opt = {
                    let hk = hk_clone.lock();
                    hk.event_to_digit(&hk_event)
                };
                if let Some(digit) = digit_opt {
                    handle_paste(&app_state.clone(), digit);
                }
            }
        }
    })?;

    Ok(())
}

/// 重载来源：决定失败时的反馈通道（ADR-0005）
enum ReloadSource {
    /// 托盘菜单触发：成败均弹系统对话框
    Manual,
    /// 文件变更自动触发：失败静默保留旧配置，仅记日志
    Auto,
}

/// 手动与自动共用的重载管线：读取→校验→整体应用→热键重注册。
/// 成功时两者都弹“配置已重载”；热键冲突沿用警告对话框；
/// 唯一差异在加载/校验失败的反馈通道。
fn run_reload(
    state: &app::AppState,
    hk: &Arc<parking_lot::Mutex<hotkey::HotkeyManager>>,
    enabled: &Arc<parking_lot::Mutex<bool>>,
    tray: &Arc<Tray>,
    source: ReloadSource,
) {
    let old_visible = state.config.lock().show_tray_icon;
    match state.reload_config() {
        Ok(new_cfg) => {
            if *enabled.lock() {
                let mut hkm = hk.lock();
                hkm.unregister_all();
                let fails = hkm.register_from_config(&new_cfg);
                if !fails.is_empty() {
                    let msg = fails
                        .iter()
                        .map(|(d, s)| format!("{}: {}", d, s))
                        .collect::<Vec<_>>()
                        .join("\n");
                    dialog::show_warning("热键冲突", &msg);
                }
            } else {
                hk.lock().unregister_all();
            }
            if old_visible != new_cfg.show_tray_icon {
                let _ = tray.set_visible(new_cfg.show_tray_icon);
            }
            // 先结算基线再弹窗：模态框阻塞期间到达的重复事件可被 matches_applied 丢弃
            if let Ok(p) = Config::config_path() {
                state.config_probe.lock().mark_settled(&p);
            }
            dialog::show_info("FastPaste", "配置已重载");
        }
        Err(e) => match source {
            ReloadSource::Manual => dialog::show_error("重载配置失败", &e.to_string()),
            ReloadSource::Auto => {
                log::warn!("自动重载失败，保留旧配置: {}", e);
                // 结算该内容，避免对同一份坏配置反复尝试
                if let Ok(p) = Config::config_path() {
                    state.config_probe.lock().mark_settled(&p);
                }
            }
        },
    }
}

fn open_config_file(path: &std::path::Path) -> Result<()> {
    if !path.exists() {
        let cfg = Config::default();
        cfg.save_to(path)?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("notepad").arg(path).spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn powershell_literal_path(p: &std::path::Path) -> String {
    p.display().to_string().replace('\'', "''")
}

#[cfg(target_os = "windows")]
fn try_elevate() -> Result<()> {
    let exe = std::env::current_exe()?;
    let escaped = powershell_literal_path(&exe);
    let ps_cmd = format!(
        "try {{ Start-Process -FilePath '{}' -Verb RunAs -ErrorAction Stop }} catch {{ exit 1 }}",
        escaped
    );
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_cmd])
        .output()?;
    if output.status.success() {
        std::process::exit(0);
    } else {
        log::warn!(
            "提权重启失败或被拒绝: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }
}

#[cfg(all(test, target_os = "windows"))]
mod try_elevate_tests {
    use super::powershell_literal_path;
    use std::path::Path;

    #[test]
    fn escape_single_quote() {
        let p = Path::new(r"C:\a'b\c.exe");
        assert_eq!(powershell_literal_path(p), r"C:\a''b\c.exe");
    }
}

#[cfg(all(test, not(target_os = "windows")))]
mod try_elevate_tests {
    fn powershell_literal_path(p: &std::path::Path) -> String {
        p.display().to_string().replace('\'', "''")
    }

    #[test]
    fn escape_single_quote() {
        let p = std::path::Path::new(r"C:\a'b\c.exe");
        assert_eq!(powershell_literal_path(p), r"C:\a''b\c.exe");
    }
}

fn show_diagnosis() {
    #[cfg(target_os = "macos")]
    {
        dialog::show_info(
            "权限诊断",
            "macOS 需要以下权限：\n\n1. 辅助功能 (Accessibility) - 用于 enigo 模拟粘贴\n   系统设置 → 隐私与安全性 → 辅助功能 → 勾选 FastPaste\n2. 剪贴板访问 - 通常自动授权\n\n若粘贴失败，请检查辅助功能是否已授权。\n\n辅助功能授权后，本应用可向其它窗口注入按键。请只在信任本应用时开启。",
        );
    }
    #[cfg(target_os = "windows")]
    {
        let elevated = is_elevated::is_elevated();
        let autostart = autostart::is_enabled();
        dialog::show_info(
            "权限诊断",
            &format!(
                "Windows 诊断：\n\n• 管理员权限: {}\n• 自启已启用: {}\n• 若热键无效，请检查是否被其他软件占用\n• 若粘贴到管理员窗口失败，请以管理员身份重启\n• 以管理员身份常驻会扩大剪贴板中敏感内容的暴露面。若你不需要向管理员窗口粘贴，可拒绝 UAC、以普通权限运行。\n\n自启提权需在托盘勾选“开机自启”时以 Task Scheduler 方式创建（需 UAC 授权一次）。",
                if elevated { "是" } else { "否" },
                if autostart { "是" } else { "否" }
            ),
        );
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        dialog::show_info("权限诊断", "当前平台诊断暂未实现");
    }
}
