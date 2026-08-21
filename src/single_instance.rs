use anyhow::Result;
use single_instance::SingleInstance;

/// 单实例守卫
pub struct InstanceGuard {
    _instance: SingleInstance,
}

impl InstanceGuard {
    pub fn new(id: &str) -> Result<Self> {
        let instance = SingleInstance::new(id)?;
        if !instance.is_single() {
            anyhow::bail!("FastPaste 已在运行");
        }
        Ok(Self { _instance: instance })
    }
}
