pub mod parser;
pub mod detect;

use std::any::Any;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::core::backend::ProfilerBackend;
use crate::core::session::ProfilerSession;
use crate::core::types::{ProfilerData, ViewDescriptor};

pub struct NcuBackend;

pub struct NcuSession {
    path: PathBuf,
    data: ProfilerData,
    views: Vec<ViewDescriptor>,
}

impl ProfilerSession for NcuSession {
    fn path(&self) -> &Path {
        &self.path
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ProfilerBackend for NcuBackend {
    fn name(&self) -> &str {
        "NVIDIA Nsight Compute"
    }

    fn id(&self) -> &str {
        "ncu"
    }

    fn detect(&self, path: &Path) -> Result<f64> {
        detect::detect_ncu(path)
    }

    fn open(&self, path: &Path) -> Result<Box<dyn ProfilerSession>> {
        let (data, views) = parser::parse_csv(path)?;
        Ok(Box::new(NcuSession {
            path: path.to_path_buf(),
            data,
            views,
        }))
    }

    fn list_views(&self, session: &dyn ProfilerSession) -> Result<Vec<ViewDescriptor>> {
        let ncu = session
            .as_any()
            .downcast_ref::<NcuSession>()
            .ok_or_else(|| anyhow::anyhow!("Invalid session type"))?;
        Ok(ncu.views.clone())
    }

    fn get_view_data(&self, session: &dyn ProfilerSession, _view_id: &str) -> Result<ProfilerData> {
        let ncu = session
            .as_any()
            .downcast_ref::<NcuSession>()
            .ok_or_else(|| anyhow::anyhow!("Invalid session type"))?;
        Ok(ncu.data.clone())
    }

    fn get_stats(&self, session: &dyn ProfilerSession, _view_id: &str) -> Result<Option<ProfilerData>> {
        let ncu = session
            .as_any()
            .downcast_ref::<NcuSession>()
            .ok_or_else(|| anyhow::anyhow!("Invalid session type"))?;
        Ok(Some(parser::compute_ncu_stats(&ncu.data)?))
    }
}
