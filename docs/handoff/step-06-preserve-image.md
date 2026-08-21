# 当前步骤 06 — 保留剪贴板时恢复图片

## 目标

`preserve_clipboard = true` 时，若用户原来的剪贴板是图片，粘贴文本后应恢复该图片，而不是 `clear()`。

## 允许修改

- `src/clipboard.rs`
- `src/paste.rs`（仅当 Mock 或测试需要跟着改枚举）

## 做法

1. 扩展备份：

```rust
pub enum ClipboardBackup {
    Text(String),
    Image { width: usize, height: usize, bytes: Vec<u8> },
    Empty,
}
```

2. `ArboardClipboard::get_backup`：先 `get_text()` 成功 → `Text`。否则 `get_image()` 成功 → `Image { width, height, bytes: img.bytes.into_owned() }`。都失败 → `Empty`。不要再把「get_text 失败」直接当成 Empty。

3. `restore`：`Image` 分支用 `arboard::ImageData { width, height, bytes: Cow::Owned(bytes) }` 调 `set_image`。`Empty` 仍 `clear`。

4. `MockClipboard` 增加 `image: Option<(usize, usize, Vec<u8>)>`（或等价字段）。`get_backup` / `restore` / `get_text` 行为与上面一致：有文本优先文本。测试需要能塞一张假图片。

## 测试

- 已有「preserve 恢复文本」测试仍通过。
- 新增：Mock 无文本、有 image `(1,1,vec![1,2,3,4])`；`paste_with_clipboard("new", cb, true)` 之后文本不是粘贴内容长期占用（preserve 会 restore），image 仍在。
- `do_paste` 的 clipboard + preserve 路径用 Mock 覆盖一次图片恢复。

## Done when

- [ ] `ClipboardBackup` 有 Image
- [ ] 真实剪贴板 get_text 失败会尝试 get_image
- [ ] 图片 restore 不走 clear
- [ ] `cargo test` 通过
