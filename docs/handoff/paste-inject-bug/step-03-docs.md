# 当前步骤 03 — 文档

## 目标

把「热键粘贴会先等修饰键松开，Windows 用虚拟键 V」写进 README 与 CHANGELOG，避免以后改回 `Unicode('v')`。

## 允许修改

- `README.md`
- `CHANGELOG.md`

## 做法

1. README「粘贴」功能条补一句：热键触发的粘贴会先等待修饰键松开，再向焦点窗口发送 Ctrl+V（Windows 使用虚拟键 `V`）。术语用「粘贴」，不要写「复制到输入框」。

2. CHANGELOG 在当前版本下增加 Fixed：热键仍按下时模拟 Ctrl+V 无法注入焦点窗口；现等待松开，Windows 改用 `Key::V`。

3. 不要改 `src/`。

## Done when

- [ ] README 写了等待松开与 Windows 虚拟键 V
- [ ] CHANGELOG 有对应 Fixed
- [ ] 未改 `src/`
