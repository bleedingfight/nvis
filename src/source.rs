pub mod browser;
pub mod local;
pub mod s3;
pub mod ssh;
pub mod webdav;

use anyhow::{anyhow, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use browser::DirEntry;

/// A file resolved by a [`Source`] — either a local path or a fetched temp copy.
///
/// For remote sources (SSH, WebDAV, …) the struct owns the temp file and
/// deletes it on `Drop`. For local sources `should_cleanup` is `false` and
/// `Drop` is a no-op.
pub struct FetchedFile {
    /// Absolute or relative path on the local filesystem, ready to be opened.
    pub local_path: PathBuf,
    /// The original URI the user requested (e.g. `ssh:host:/data/x.sqlite`).
    pub original_uri: String,
    /// Identifier of the source that produced this file (e.g. `"ssh"`, `"local"`).
    pub source_id: String,
    /// Whether to delete `local_path` when this struct is dropped.
    pub should_cleanup: bool,
}

impl FetchedFile {
    /// Wrap a local path — no cleanup is performed on drop.
    pub fn local(path: PathBuf, uri: String) -> Self {
        Self {
            local_path: path,
            original_uri: uri,
            source_id: "local".to_string(),
            should_cleanup: false,
        }
    }

    /// Wrap a fetched temp file — the file is deleted on drop.
    pub fn remote(path: PathBuf, uri: String, source_id: &str) -> Self {
        Self {
            local_path: path,
            original_uri: uri,
            source_id: source_id.to_string(),
            should_cleanup: true,
        }
    }
}

impl Drop for FetchedFile {
    fn drop(&mut self) {
        if self.should_cleanup {
            log::debug!("Cleaning up temp file: {:?}", self.local_path);
            let _ = std::fs::remove_file(&self.local_path);
        }
    }
}

/// A source of profiler files.
///
/// Each implementation handles one or more URI schemes:
/// - [`local::LocalSource`] — local filesystem paths (fallback)
/// - [`ssh::SshSource`] — `ssh:`, `scp:`, `ssh://`, `scp://`
/// - [`webdav::WebDavSource`] — `webdav:`, `webdavs:`, `http://`, `https://`
/// - [`s3::S3Source`] — `s3://`
///
/// To add a new transport (e.g. S3, gcs), implement this trait and register
/// it in [`register_all`].
pub trait Source: Send + Sync {
    /// Short stable identifier (e.g. `"ssh"`, `"webdav"`).
    fn id(&self) -> &str;
    /// Human-readable name for UI messages.
    fn name(&self) -> &str;
    /// Whether this source can handle the given URI.
    fn can_handle(&self, uri: &str) -> bool;
    /// Fetch the URI and return a local path (possibly a temp file).
    fn fetch(&self, uri: &str) -> Result<FetchedFile>;

    /// Fetch the URI directly to `dest` (no temp file). Used for background
    /// downloads where the caller wants to `stat(dest)` for progress.
    /// Default: `fetch` then rename/copy to dest.
    fn fetch_to(&self, uri: &str, dest: &Path) -> Result<FetchedFile> {
        let fetched = self.fetch(uri)?;
        if fetched.local_path.as_path() == dest {
            return Ok(fetched);
        }
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::rename(&fetched.local_path, dest).is_err() {
            std::fs::copy(&fetched.local_path, dest)
                .map_err(|e| anyhow!("cannot copy to {:?}: {}", dest, e))?;
            let _ = std::fs::remove_file(&fetched.local_path);
        }
        Ok(FetchedFile::remote(dest.to_path_buf(), uri.to_string(), self.id()))
    }

    /// Best-effort total size in bytes for progress display. `None` if the
    /// source cannot determine it cheaply (callers degrade to bytes-only).
    fn head_size(&self, _uri: &str) -> Option<u64> {
        None
    }

    /// Whether this source supports directory listing.
    fn supports_listing(&self) -> bool {
        false
    }

    /// List entries under the given URI.
    /// Returns `Err` if this source does not support listing.
    fn list(&self, _uri: &str) -> Result<Vec<DirEntry>> {
        Err(anyhow!("Listing not supported by source '{}'", self.id()))
    }
}

/// Registry of available sources, queried in registration order.
///
/// The first source whose `can_handle` returns `true` wins. `LocalSource`
/// should be registered last so it acts as a fallback for unrecognized URIs.
pub struct SourceRegistry {
    sources: Vec<Arc<dyn Source>>,
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceRegistry {
    pub fn new() -> Self {
        Self { sources: Vec::new() }
    }

    pub fn register(&mut self, source: Arc<dyn Source>) {
        self.sources.push(source);
    }

    /// Pick the first source that claims to handle `uri`.
    pub fn resolve(&self, uri: &str) -> Result<Arc<dyn Source>> {
        for s in &self.sources {
            if s.can_handle(uri) {
                return Ok(Arc::clone(s));
            }
        }
        Err(anyhow!("No source can handle URI: {}", uri))
    }

    /// Convenience: resolve + fetch in one call.
    pub fn fetch(&self, uri: &str) -> Result<FetchedFile> {
        let source = self.resolve(uri)?;
        log::info!("Fetching '{}' via source '{}'", uri, source.id());
        source.fetch(uri)
    }

    /// Resolve + fetch directly to `dest` (for background downloads).
    pub fn fetch_to(&self, uri: &str, dest: &Path) -> Result<FetchedFile> {
        let source = self.resolve(uri)?;
        log::info!("Fetching '{}' -> {:?} via source '{}'", uri, dest, source.id());
        source.fetch_to(uri, dest)
    }

    /// Resolve + best-effort total size for progress.
    pub fn head_size(&self, uri: &str) -> Option<u64> {
        self.resolve(uri).ok()?.head_size(uri)
    }

    /// Convenience: resolve + list in one call.
    pub fn list(&self, uri: &str) -> Result<Vec<DirEntry>> {
        let source = self.resolve(uri)?;
        log::info!("Listing '{}' via source '{}'", uri, source.id());
        source.list(uri)
    }
}

/// Register all built-in sources. `LocalSource` is last (fallback).
pub fn register_all(registry: &mut SourceRegistry) {
    registry.register(Arc::new(ssh::SshSource));
    registry.register(Arc::new(webdav::WebDavSource));
    registry.register(Arc::new(s3::S3Source));
    registry.register(Arc::new(local::LocalSource));
}

/// Generate a unique temp path that preserves the basename (and thus the
/// extension) of `remote_name`, so backend detection still works.
pub(crate) fn temp_path_for(remote_name: &str) -> PathBuf {
    let path_part = remote_name.split('?').next().unwrap_or(remote_name);
    let basename = Path::new(path_part)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("remote-file");
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir()
        .join(format!("nvis-{}-{}-{}", pid, nanos, basename))
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

pub(crate) fn log_fetch_complete(local_path: &Path) {
    log::info!(
        "Fetch complete: {} bytes -> {:?}",
        file_size(local_path),
        local_path
    );
}
