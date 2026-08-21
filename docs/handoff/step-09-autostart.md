# 当前步骤 09 — 自启对齐 ADR-0004

## 目标

托盘打开自启时创建 **Task Scheduler、登录触发、最高权限（HIGHEST）** 任务。创建失败就返回错误（托盘已有失败回滚勾选），**不要**写入 `HKCU\...\Run`。关闭自启时仍删除任务，并清理可能残留的注册表值。

## 允许修改

- `src/autostart.rs`
- `src/config.rs`（只改 `autostart_elevated` 默认值）
- `src/main.rs`（托盘切换自启时传入的 `elevated` 参数）

## 做法

1. `default_autostart_elevated()` 改为返回 `true`。`Config::default()` 跟着变成 `true`。

2. `src/main.rs` 里 `TrayEvent::ToggleAutostart`：调用 `autostart::set_enabled(enable, true)`（始终提权任务）。可以仍读取 config，但 **enable=true 时 elevated 必须为 true**。推荐直接传 `true`，少一个出错点。

3. `set_enabled_windows(true, _)`：`schtasks /Create ... /RL HIGHEST`（忽略函数的 `elevated` 参数，或 assert 后仍写 HIGHEST）。成功则 `Ok(())`。失败则 `bail!` 带上 stdout/stderr。**删除**「失败则 `set_enabled_windows_registry(true)`」分支。

4. `set_enabled_windows(false, _)`：删除任务 + `set_enabled_windows_registry(false)` 清理残留。保留这个清理。

5. `is_enabled_windows`：任务存在 **或** 注册表 `FastPaste` 值存在都算已开启（这样才能关掉旧的注册表残留）。检测注册表可用 `reg query HKCU\Software\Microsoft\Windows\CurrentVersion\Run /v FastPaste`。

`set_enabled_windows_registry(true)` 若已无调用者，可删「写入」分支，只留 delete；或整函数只用于 delete。

## 测试

`autostart.rs` 现有 `exe_path_ok` 保留。不必在 CI 里真建任务。

## Done when

- [ ] 开启自启只走 `schtasks` + `HIGHEST`
- [ ] 开启失败不写 Run 键
- [ ] 关闭时删任务并清 Run 键
- [ ] 默认 `autostart_elevated == true`
- [ ] `cargo test` 通过
