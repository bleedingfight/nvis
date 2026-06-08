pub mod parser;
pub mod detect;

use std::any::Any;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::core::backend::ProfilerBackend;
use crate::core::session::ProfilerSession;
use crate::core::types::{ProfilerData, ViewDescriptor};

pub struct TorchBackend;

pub struct TorchSession {
    path: PathBuf,
    data: ProfilerData,
    views: Vec<ViewDescriptor>,
}

impl ProfilerSession for TorchSession {
    fn path(&self) -> &Path {
        &self.path
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ProfilerBackend for TorchBackend {
    fn name(&self) -> &str {
        "PyTorch Profiler"
    }

    fn id(&self) -> &str {
        "torch"
    }

    fn detect(&self, path: &Path) -> Result<f64> {
        detect::detect_torch(path)
    }

    fn open(&self, path: &Path) -> Result<Box<dyn ProfilerSession>> {
        let (data, views) = parser::parse_trace(path)?;
        Ok(Box::new(TorchSession {
            path: path.to_path_buf(),
            data,
            views,
        }))
    }

    fn list_views(&self, session: &dyn ProfilerSession) -> Result<Vec<ViewDescriptor>> {
        let torch = session
            .as_any()
            .downcast_ref::<TorchSession>()
            .ok_or_else(|| anyhow::anyhow!("Invalid session type"))?;
        Ok(torch.views.clone())
    }

    fn get_view_data(&self, session: &dyn ProfilerSession, _view_id: &str) -> Result<ProfilerData> {
        let torch = session
            .as_any()
            .downcast_ref::<TorchSession>()
            .ok_or_else(|| anyhow::anyhow!("Invalid session type"))?;
        Ok(torch.data.clone())
    }

    fn get_stats(&self, session: &dyn ProfilerSession, _view_id: &str) -> Result<Option<ProfilerData>> {
        let torch = session
            .as_any()
            .downcast_ref::<TorchSession>()
            .ok_or_else(|| anyhow::anyhow!("Invalid session type"))?;
        Ok(Some(parser::compute_torch_stats(&torch.data)?))
    }
}
