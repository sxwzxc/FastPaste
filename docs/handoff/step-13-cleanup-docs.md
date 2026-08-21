# 当前步骤 13 — 收尾：死代码、文档、CHANGELOG

## 目标

主路径不再留未调用的重复逻辑；文档和 CHANGELOG 与代码一致。

## 允许修改

- `src/app.rs`
- `src/main.rs`
- `src/dialog.rs`
- `src/hotkey.rs`
- `src/clipboard.rs`（只删 `ArboardClipboard.last_text` 若仍未使用）
- `Cargo.toml`
- `CHANGELOG.md`
- 新建 `README.md`

## 做法

1. **重载配置单一路径**：让 `AppState::reload_config` 真正执行：load+validate、`set_ignore_regex`、`set_max_bytes`、写回 `self.config`，并返回 `Ok(new_cfg)`（把返回类型改成 `Result<Config>` 比空 `Vec` 有用）。`src/main.rs` 的 `TrayEvent::ReloadConfig` 调用它，再用返回的 `Config` 去注销/注册热键。校验失败仍对话框且不应用（保持现有行为）。

2. **热键启用/停用**：`TrayEvent::Toggle` 改为调用已有的 `HotkeyManager::set_enabled(en, &cfg)`，删掉主函数里手写的 register/unregister 重复块（若 `set_enabled` 与当前行为一致：enable 时 register_from_config，disable 时 unregister_all）。

3. 删除确认无引用的：`dialog::notify_bubble`（步骤 07 已改用 `pending_notice`）、`ArboardClipboard.last_text`。`show_confirm` 若无调用者一并删除。

4. `Cargo.toml` 的 `repository` 改为 `https://github.com/sxwzxc/FastPaste`（与 `git remote` 一致）。

5. 新建根目录 `README.md`，用 `CONTEXT.md` 的术语，写清：做什么、热键、配置路径、托盘项、Windows 提权/自启、macOS 辅助功能。不要编未实现的功能（第二实例唤醒、macOS Dock 隐藏、安装器都不要写成已有）。

6. 改 `CHANGELOG.md` `[0.1.0]`：删掉或改写与代码不符的句子：
   - transient：改成「Windows 检测常见 transient 格式；macOS 未实现则写未实现」
   - 历史预览：保留，因为步骤 01 已做
   - 「第二实例唤醒已有实例」改为「第二实例退出并提示已在运行」
   - requireAdministrator：若步骤 11 完成则保留
   - 托盘气泡：改为「粘贴失败主线程对话框提示，并写入剪贴板」

## Done when

- [ ] 重载配置走 `AppState::reload_config`
- [ ] 启用状态切换走 `HotkeyManager::set_enabled`
- [ ] 无引用函数/字段已删
- [ ] repository URL 正确
- [ ] README 存在且不声称未做功能
- [ ] CHANGELOG 与实现一致
- [ ] `cargo test` 通过
