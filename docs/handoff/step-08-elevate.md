# 当前步骤 08 — UAC 拒绝后进程继续运行

## 目标

Windows 下当前未提权时尝试提权重启：只有 PowerShell/`Start-Process -Verb RunAs` **成功返回**后才 `process::exit(0)`。用户拒绝 UAC 或命令失败时，现有进程继续跑（托盘还在）。

## 允许修改

- `src/main.rs` 里的 `try_elevate`（以及它的调用方式：调用处保持 `let _ = try_elevate();`，由函数内部决定是否退出）

## 做法

重写 `try_elevate`（仅 `cfg(windows)`）：

1. `std::env::current_exe()` 得到路径。
2. 为 PowerShell 单引号转义：把路径里的 `'` 换成 `''`。
3. 用 `Command::new("powershell")` 执行 **同步** `.output()`（不是 `.spawn()`）：

```text
-NoProfile -Command "try { Start-Process -FilePath 'PATH' -Verb RunAs -ErrorAction Stop } catch { exit 1 }"
```

4. `status.success()` 为 true → `std::process::exit(0)`。
5. 否则 `log::warn!(...)` 并 `return Ok(())`（进程继续）。

托盘「以管理员身份重启」走同一函数。

路径含空格必须用上面的单引号 FilePath 形式，不要字符串拼接进未加引号的 `Start-Process 'path'` 且无 `-FilePath`。

## 测试

单元测试很难覆盖 UAC。加一个小测试只测转义：若你把 `fn powershell_literal_path(p: &Path) -> String` 抽出来，测 `C:\a'b\c.exe` 变成 `C:\a''b\c.exe`。抽不出来也可以不加测试，但 `cargo test` 必须绿。

## Done when

- [ ] 使用 `.output()` 等待 PowerShell 结束
- [ ] 成功才 `exit(0)`；失败/拒绝则函数返回、进程不退出
- [ ] 路径按 PowerShell 单引号规则转义
- [ ] `cargo test` 通过
