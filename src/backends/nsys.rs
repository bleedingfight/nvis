pub mod db;
pub mod stats;
pub mod detect;
pub mod summary;

use std::any::Any;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::Result;
use rusqlite::Connection;

use crate::core::backend::ProfilerBackend;
use crate::core::session::ProfilerSession;
use crate::core::types::{ProfilerData, ViewDescriptor};

pub struct NsysBackend;

pub struct NsysSession {
    path: PathBuf,
    conn: Mutex<Connection>,
}

impl ProfilerSession for NsysSession {
    fn path(&self) -> &Path {
        &self.path
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl NsysSession {
    pub fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|e| anyhow::anyhow!("DB lock error: {}", e))
    }
}

impl ProfilerBackend for NsysBackend {
    fn name(&self) -> &str {
        "NVIDIA Nsight Systems"
    }

    fn id(&self) -> &str {
        "nsys"
    }

    fn detect(&self, path: &Path) -> Result<f64> {
        detect::detect_nsys(path)
    }

    fn open(&self, path: &Path) -> Result<Box<dyn ProfilerSession>> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        Ok(Box::new(NsysSession {
            path: path.to_path_buf(),
            conn: Mutex::new(conn),
        }))
    }

    fn list_views(&self, session: &dyn ProfilerSession) -> Result<Vec<ViewDescriptor>> {
        let nsys = session
            .as_any()
            .downcast_ref::<NsysSession>()
            .ok_or_else(|| anyhow::anyhow!("Invalid session type"))?;
        let conn = nsys.conn()?;
        db::list_views(&conn)
    }

    fn get_view_data(&self, session: &dyn ProfilerSession, view_id: &str) -> Result<ProfilerData> {
        let nsys = session
            .as_any()
            .downcast_ref::<NsysSession>()
            .ok_or_else(|| anyhow::anyhow!("Invalid session type"))?;
        let conn = nsys.conn()?;
        db::get_view_data(&conn, view_id)
    }

    fn get_stats(&self, session: &dyn ProfilerSession, view_id: &str) -> Result<Option<ProfilerData>> {
        let nsys = session
            .as_any()
            .downcast_ref::<NsysSession>()
            .ok_or_else(|| anyhow::anyhow!("Invalid session type"))?;
        let conn = nsys.conn()?;
        if view_id == "CUPTI_ACTIVITY_KIND_RUNTIME" {
            Ok(Some(stats::compute_cuda_api_aggregates(&conn, 50)?))
        } else {
            Ok(None)
        }
    }

    fn execute_sql(&self, session: &dyn ProfilerSession, sql: &str) -> Result<ProfilerData> {
        let nsys = session
            .as_any()
            .downcast_ref::<NsysSession>()
            .ok_or_else(|| anyhow::anyhow!("Invalid session type"))?;
        let conn = nsys.conn()?;
        db::execute_sql(&conn, sql)
    }
}
