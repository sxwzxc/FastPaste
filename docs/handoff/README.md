# FastPaste 缺口修复 — 给人看的操作手册

把下面的步骤**一次只喂一步**给小模型。不要把本目录整包丢进同一轮对话。

## 怎么喂

每一轮用户消息 = `GLOBAL.md` 全文 + 空一行 + `step-XX-....md` 全文。

模型回复「Done when 全勾、`cargo test` 绿」之后，再发 `VERIFY.md` 做验收。验收通过才进入下一步。

卡住（编译红、胡乱改文件、编造 API）时发 `STUCK.md`，不要直接进下一步。

新开对话时：把 `GLOBAL.md` + 当前 step 再贴一次，并写「仓库路径：D:\MyLearn\FastPaste」。不要依赖上一轮记忆。

### 第一轮可直接复制

把下面三段中间换成当前步骤文件全文：

```
仓库路径：D:\MyLearn\FastPaste

<粘贴 GLOBAL.md>

<粘贴 step-01-tray-preview.md>
```

## 顺序（有依赖，不要跳）

| 步 | 文件 | 做什么 | 依赖 |
|---|---|---|---|
| 01 | `step-01-tray-preview.md` | 托盘历史预览真正刷新 | 无 |
| 02 | `step-02-first-run.md` | 首次运行说明对话框 | 无 |
| 03 | `step-03-max-entry-bytes.md` | `max_entry_bytes` 真正限制条目大小 | 无 |
| 04 | `step-04-hotkey-parse.md` | 未知热键报错，不再变成 Digit1 | 无 |
| 05 | `step-05-no-catchup.md` | 停用期复制不补录 | 无 |
| 06 | `step-06-preserve-image.md` | 保留剪贴板时能恢复图片 | 无 |
| 07 | `step-07-paste-fallback.md` | 粘贴失败路径对齐 ADR-0003；提示回主线程 | 01（都改 event loop） |
| 08 | `step-08-elevate.md` | UAC 拒绝后原进程继续跑 | 无 |
| 09 | `step-09-autostart.md` | 自启走提权任务，创建失败不写注册表 | 无 |
| 10 | `step-10-diagnosis-version.md` | 权限诊断补风险说明；版本号走 Cargo | 无 |
| 11 | `step-11-win-manifest.md` | 嵌入 requireAdministrator manifest | 无 |
| 12 | `step-12-transient.md` | 敏感内容：transient 检测缝 + Windows 实现 | 05（都改 watcher） |
| 13 | `step-13-cleanup-docs.md` | 死代码、仓库 URL、CHANGELOG、README | 01–12 都做完 |

01 和 02、07 都动 `src/main.rs`。按序做即可，不要并行开三个对话改同一文件。

## 本包刻意不做（留给以后）

- 第二实例唤醒已有实例（需要 IPC）
- macOS `.app` + `LSUIElement`（cargo 直接跑 exe 无效）
- NSIS/MSI 安装器
- 系统事件驱动剪贴板（ADR-0002 允许首版轮询）
- 真正的托盘气泡（用主线程对话框代替）

## 全局完成标准

全部 13 步结束后：

- `cargo test` 全绿
- 托盘「历史预览」复制文本后最多约 200ms 出现 20 字预览，可点击粘贴
- 删掉配置文件再启动会弹出首次说明
- 停用后再启用，不会把停用期间最后一次复制补进历史（除非再复制一次）
- 粘贴失败时焦点窗口外弹对话框，程序不卡死在工作线程
- Windows 非提权启动：UAC 点否后原窗口/托盘还在
