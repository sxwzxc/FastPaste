# 当前步骤 11 — Windows requireAdministrator manifest

## 目标

Windows 构建把 `requireAdministrator` manifest 嵌进 exe，与 ADR-0004 一致。用户运行 exe 时由系统弹 UAC，而不是只靠事后 PowerShell 提权。

## 允许修改

- 新建 `app.manifest`
- 新建 `build.rs`
- `Cargo.toml`

## 做法

1. 仓库根目录新建 `app.manifest`：

```xml
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="0.1.0.0" name="FastPaste" type="win32"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
```

2. `Cargo.toml` 增加：

```toml
[build-dependencies]
embed-resource = "3"
```

若 `embed-resource` 3 的 API 与下面不符，改用 `2` 并把 `build.rs` 调成该主版本文档里的 `compile` 函数。以能编译为准。

3. 根目录 `build.rs`：

```rust
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=app.manifest");
        embed_resource::compile("app.manifest", embed_resource::NONE);
    }
}
```

若 `compile` 签名不同：打开 crates.io 上你锁进 Cargo.lock 的那个版本说明，用等价 API 嵌入**同一个** `app.manifest`。不要改 `requestedExecutionLevel`。

4. 保留步骤 08 的 `try_elevate`：无 manifest 的旧构建或从非提权父进程拉起时仍有用。

## 验证

`cargo test` 必须通过。Windows 上 `cargo build` 成功即视为嵌入流程接通（不必在提示词里用外部工具拆 exe 检查）。

## Done when

- [ ] `app.manifest` 含 `requireAdministrator`
- [ ] Windows 构建会 compile 该 manifest
- [ ] 非 Windows 构建仍成功（`build.rs` 有 OS 判断）
- [ ] `cargo test` 通过
