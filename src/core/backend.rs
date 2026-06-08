use anyhow::Result;
use std::path::Path;

use super::types::{ViewDescriptor, ProfilerData};
use super::session::ProfilerSession;

pub trait ProfilerBackend: Send + Sync {
    fn name(&self) -> &str;
    fn id(&self) -> &str;
    fn detect(&self, path: &Path) -> Result<f64>;
    fn open(&self, path: &Path) -> Result<Box<dyn ProfilerSession>>;
    fn list_views(&self, session: &dyn ProfilerSession) -> Result<Vec<ViewDescriptor>>;
    fn get_view_data(&self, session: &dyn ProfilerSession, view_id: &str) -> Result<ProfilerData>;
    fn get_stats(&self, session: &dyn ProfilerSession, view_id: &str) -> Result<Option<ProfilerData>>;
}
