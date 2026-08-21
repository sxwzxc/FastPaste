fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=app.manifest");
        let out_dir = std::env::var("OUT_DIR").unwrap();
        let rc_path = std::path::Path::new(&out_dir).join("manifest.rc");
        let profile = std::env::var("PROFILE").unwrap_or_default();
        // release 嵌入 requireAdministrator，debug/test 嵌入 asInvoker 避免 cargo test 提权失败
        let manifest_content = if profile == "release" {
            std::fs::read_to_string("app.manifest").unwrap_or_default()
        } else {
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="0.1.0.0" name="FastPaste" type="win32"/>
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
        std::fs::write(&rc_path, format!("1 24 \"{}\"", manifest_str)).unwrap();
        embed_resource::compile_for(&rc_path, &["fastpaste"], embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}
