fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=app.manifest");
        println!("cargo:rerun-if-changed=assets/icon.ico");
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let profile = std::env::var("PROFILE").unwrap_or_default();
        // release 嵌入 requireAdministrator，debug/test 嵌入 asInvoker 避免 cargo test 提权失败
        let manifest_content = if profile == "release" {
            std::fs::read_to_string("app.manifest").unwrap_or_default()
        } else {
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="0.2.0.0" name="FastPaste" type="win32"/>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>"#
                .to_string()
        };
        let manifest_tmp = std::path::Path::new(&out_dir).join("app.manifest.tmp");
        std::fs::write(&manifest_tmp, manifest_content).unwrap();
        let manifest_str = manifest_tmp.display().to_string().replace('\\', "\\\\");

        // 资源脚本：同时嵌入图标与 manifest
        let rc_path = std::path::Path::new(&out_dir).join("resource.rc");
        let icon_path = std::path::Path::new("assets/icon.ico");
        if icon_path.exists() {
            let icon_canon = icon_path.canonicalize().unwrap_or_else(|_| icon_path.to_path_buf());
            let icon_str = icon_canon.display().to_string().replace('\\', "\\\\");
            // 1 ICON 为 exe 主图标，1 24 为 manifest
            std::fs::write(
                &rc_path,
                format!("1 ICON \"{}\"\n1 24 \"{}\"", icon_str, manifest_str),
            )
            .unwrap();
        } else {
            // 无图标时仅嵌入 manifest
            std::fs::write(&rc_path, format!("1 24 \"{}\"", manifest_str)).unwrap();
        }
        embed_resource::compile_for(&rc_path, &["fastpaste"], embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
