# 当前步骤 02 — 首次运行说明

## 目标

用户第一次启动、系统配置目录里还没有 `FastPaste/config.toml` 时，弹出说明对话框。之后启动不再弹。

## 允许修改

- `src/main.rs`

## 原因

`Config::load()` → `load_from` 在文件不存在时会先 `save_to` 写出默认配置。现在的 `if !cfg_path.exists()` 写在 `load()` 之后，永远为 false。

## 做法

在调用 `Config::load()` **之前** 记下是否首次：

```rust
let first_run = Config::config_path()
    .map(|p| !p.exists())
    .unwrap_or(false);

let config = match Config::load() { /* 现有逻辑不动 */ };

// …创建 tray / 热键之后，用 first_run 替换原来的 exists() 检查：
if first_run {
    dialog::show_info("FastPaste", "……现有文案保持不变……");
}
```

删除 `load()` 之后那段 `cfg_path.exists()` 判断。对话框文案、标题与现在一致。

## Done when

- [ ] `first_run` 在 `Config::load()` 之前计算
- [ ] 欢迎框只在 `first_run == true` 时显示
- [ ] 旧的 `exists()` 检查已删除
- [ ] `cargo test` 通过
