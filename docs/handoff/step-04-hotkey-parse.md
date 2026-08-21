# 当前步骤 04 — 热键解析失败要报错

## 目标

`to_global_hotkey` 遇到无法映射的主键时返回 `Err`，不再 `unwrap_or(Code::Digit1)`。

## 允许修改

- `src/config.rs`

## 做法

在 `to_global_hotkey` 的 `match key_str` 里：

- `0`–`9` 仍映射 `Code::Digit0`–`Digit9`
- 单字母：`Code::from_str(&format!("Key{}", ch.to_ascii_uppercase()))`，失败则 `bail!("无法解析主键: {}", key_str)`
- 其它：`Code::from_str` 大写、再原样各试一次，都失败则 `bail!("无法解析主键: {}", key_str)`
- 删除所有 `unwrap_or(Code::Digit1)`

`parse_hotkey` 可以保持只做修饰键校验（它本来就不是转 Code 的）。

## 测试

在现有 `to_global_hotkey_ok` 旁增加：

- `to_global_hotkey("ctrl+shift+1")` 仍 Ok
- `to_global_hotkey("ctrl+shift+f1")` 若 `Code::from_str("F1")` 在你们用的 global-hotkey 版本能成功则 Ok，否则按实际 API 选一个库里确定存在的键（例如 `f5` / `space`）；以 `cargo test` 为准
- `to_global_hotkey("ctrl+shift+thiskeydoesnotexist")` 为 Err
- `to_global_hotkey("ctrl+shift+")` 为 Err（若当前 split 逻辑会得到空主键）

## Done when

- [ ] 源码中 `to_global_hotkey` 没有 `Digit1` 回退
- [ ] 未知主键返回 Err
- [ ] `cargo test` 通过
