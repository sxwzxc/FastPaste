# 当前步骤 10 — 权限诊断与版本号

## 目标

权限诊断写明「以管理员身份常驻会扩大剪贴板敏感内容暴露面」。关于对话框的版本号来自 `env!("CARGO_PKG_VERSION")`，不再手写 `0.1.0`。

## 允许修改

- `src/main.rs`

## 做法

1. 文件顶部或 `show_diagnosis` 旁：`const VERSION: &str = env!("CARGO_PKG_VERSION");`

2. 「关于」对话框标题用 `format!("FastPaste {VERSION}")`，正文里的版本行用同一常量。

3. `show_diagnosis` 的 Windows 文案追加一段（保持原有管理员权限/自启/热键/管理员窗口提示，只追加）：

```text
• 以管理员身份常驻会扩大剪贴板中敏感内容的暴露面。若你不需要向管理员窗口粘贴，可拒绝 UAC、以普通权限运行。
```

4. macOS 文案追加：

```text
辅助功能授权后，本应用可向其它窗口注入按键。请只在信任本应用时开启。
```

Linux 分支保持「当前平台诊断暂未实现」。

## Done when

- [ ] 关于框版本来自 `CARGO_PKG_VERSION`
- [ ] Windows 诊断含提权与敏感内容风险
- [ ] macOS 诊断含辅助功能风险说明
- [ ] `cargo test` 通过
