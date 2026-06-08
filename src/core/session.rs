use anyhow::Result;
use std::any::Any;
use std::path::Path;

pub trait ProfilerSession: Send + Sync {
    fn path(&self) -> &Path;
    fn as_any(&self) -> &dyn Any;
    fn close(&self) -> Result<()> {
        Ok(())
    }
}
