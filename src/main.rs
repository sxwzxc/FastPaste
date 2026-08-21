#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod clipboard;
mod config;
mod dialog;
mod history;
mod hotkey;
mod paste;
mod single_instance;
mod tray;

use anyhow::Result;
use global_hotkey::GlobalHotKeyEvent;
use muda::MenuEvent;
use std::sync::Arc;
use winit::event::Event;
use winit::event_loop::{ControlFlow, EventLoop};

use crate::app::{handle_paste, spawn_clipboard_thread, AppState};
use crate::config::Config;
use crate::tray::{Tray, TrayEvent};

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

    spawn_clipboard_thread(AppState {
        history: history.clone(),
        enabled: enabled.clone(),
        config: config_arc.clone(),
    });

    let event_loop = EventLoop::new().expect("创建 winit EventLoop 失败");

    let tray = Arc::new(Tray::new(enabled.clone(), history.clone()).expect("创建托盘失败"));

    let hk_manager_inner = hotkey::HotkeyManager::new().expect("创建热键管理器失败");
    let hk_manager = Arc::new(parking_lot::Mutex::new(hk_manager_inner));
    {
        let mut hk = hk_manager.lock();
        let cfg = config_arc.lock().clone();
        let failures = hk.register_from_config(&cfg);
        if !failures.is_empty() {
            let msg = failures.iter().map(|(d, s)| format!("{}: {}", d, s)).collect::<Vec<_>>().join("\n");
            dialog::show_warning("FastPaste 热键冲突", &format!("以下热键注册失败，将跳过：\n{}", msg));
        }
    }

    {
        let cfg_path = Config::config_path().unwrap_or_default();
        if !cfg_path.exists() {
            dialog::show_info(
                "FastPaste",
                "FastPaste 已在后台运行。\n\n• 复制任意文本自动入队（最近10条）\n• Ctrl+Shift+1..0 直接粘贴\n• 右键托盘图标可启用/停用、打开配置、重载配置、设置自启\n\n配置文件位于系统配置目录 FastPaste/config.toml",
            );
        }
    }

    let tray_clone = tray.clone();
    let hk_clone = hk_manager.clone();
    let history_clone = history.clone();
    let enabled_clone = enabled.clone();
    let config_clone = config_arc.clone();

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        if let Event::AboutToWait = event {
            // 处理托盘菜单事件
            while let Ok(menu_event) = MenuEvent::receiver().try_recv() {
                let tray_event = tray_clone.handle_menu_event(&menu_event.id.0);
                match tray_event {
                    TrayEvent::Toggle(en) => {
                        log::info!("切换启用状态: {}", en);
                        if en {
                            let cfg = config_clone.lock().clone();
                            let mut hk = hk_clone.lock();
                            let fails = hk.register_from_config(&cfg);
                            if !fails.is_empty() {
                                let msg = fails.iter().map(|(d,s)| format!("{}: {}", d, s)).collect::<Vec<_>>().join("\n");
                                dialog::show_warning("热键冲突", &msg);
                            }
                        } else {
                            let mut hk = hk_clone.lock();
                            hk.unregister_all();
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
                        match Config::load() {
                            Ok(new_cfg) => {
                                if let Err(e) = new_cfg.validate() {
                                    dialog::show_error("配置校验失败", &e.to_string());
                                } else {
                                    {
                                        let mut h = history_clone.lock();
                                        h.set_ignore_regex(new_cfg.ignore_regex.clone());
                                    }
                                    {
                                        let mut cfg = config_clone.lock();
                                        *cfg = new_cfg.clone();
                                    }
                                    {
                                        let mut hk = hk_clone.lock();
                                        hk.unregister_all();
                                        let fails = hk.register_from_config(&new_cfg);
                                        if !fails.is_empty() {
                                            let msg = fails.iter().map(|(d,s)| format!("{}: {}", d, s)).collect::<Vec<_>>().join("\n");
                                            dialog::show_warning("热键冲突", &msg);
                                        }
                                    }
                                    dialog::show_info("FastPaste", "配置已重载");
                                }
                            }
                            Err(e) => {
                                dialog::show_error("重载配置失败", &e.to_string());
                            }
                        }
                    }
                    TrayEvent::ToggleAutostart(enable) => {
                        let elevated = {
                            let cfg = config_clone.lock();
                            cfg.autostart_elevated
                        };
                        match autostart::set_enabled(enable, elevated) {
                            Ok(_) => {
                                log::info!("自启已切换: {}", enable);
                            }
                            Err(e) => {
                                dialog::show_error("自启设置失败", &e.to_string());
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
                            "FastPaste 0.1.0",
                            "跨平台静默剪贴板管理器\n\n• 监听剪贴板，保留最近 10 条纯文本\n• Ctrl+Shift+1..0 直接粘贴\n• 托盘右键管理\n\n配置: FastPaste/config.toml\n版本: 0.1.0",
                        );
                    }
                    TrayEvent::Exit => {
                        elwt.exit();
                    }
                    TrayEvent::PasteHistory(digit) => {
                        let st = AppState {
                            history: history_clone.clone(),
                            enabled: enabled_clone.clone(),
                            config: config_clone.clone(),
                        };
                        handle_paste(&st, digit);
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
                    let st = AppState {
                        history: history_clone.clone(),
                        enabled: enabled_clone.clone(),
                        config: config_clone.clone(),
                    };
                    handle_paste(&st, digit);
                }
            }
        }
    })?;

    Ok(())
}

fn open_config_file(path: &std::path::Path) -> Result<()> {
    if !path.exists() {
        let cfg = Config::default();
        cfg.save_to(path)?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("notepad")
            .arg(path)
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn try_elevate() -> Result<()> {
    let exe = std::env::current_exe()?;
    std::process::Command::new("powershell")
        .args([
            "-Command",
            &format!("Start-Process '{}' -Verb RunAs", exe.display()),
        ])
        .spawn()?;
    std::process::exit(0);
}

fn show_diagnosis() {
    #[cfg(target_os = "macos")]
    {
        dialog::show_info(
            "权限诊断",
            "macOS 需要以下权限：\n\n1. 辅助功能 (Accessibility) - 用于 enigo 模拟粘贴\n   系统设置 → 隐私与安全性 → 辅助功能 → 勾选 FastPaste\n2. 剪贴板访问 - 通常自动授权\n\n若粘贴失败，请检查辅助功能是否已授权。",
        );
    }
    #[cfg(target_os = "windows")]
    {
        let elevated = is_elevated::is_elevated();
        let autostart = autostart::is_enabled();
        dialog::show_info(
            "权限诊断",
            &format!(
                "Windows 诊断：\n\n• 管理员权限: {}\n• 自启已启用: {}\n• 若热键无效，请检查是否被其他软件占用\n• 若粘贴到管理员窗口失败，请以管理员身份重启\n\n自启提权需在托盘勾选“开机自启”时以 Task Scheduler 方式创建（需 UAC 授权一次）。",
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
