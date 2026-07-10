use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;

use super::browser::{DirEntry, EntryKind};
use super::{FetchedFile, Source};

/// Source for local filesystem paths.
///
/// Acts as a fallback — `can_handle` returns `true` for anything not claimed
/// by a more specific source, so it should be registered last.
pub struct LocalSource;

impl Source for LocalSource {
    fn id(&self) -> &str {
        "local"
    }

    fn name(&self) -> &str {
        "Local filesystem"
    }

    fn can_handle(&self, _uri: &str) -> bool {
        // Fallback: claims everything not handled by a more specific source.
        true
    }

    fn fetch(&self, uri: &str) -> Result<FetchedFile> {
        let path = uri
            .strip_prefix("file://")
            .or_else(|| uri.strip_prefix("file:"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(uri));
        Ok(FetchedFile::local(path, uri.to_string()))
    }

    /// Local files are already local — ignore `dest`, return the path as-is.
    fn fetch_to(&self, uri: &str, _dest: &std::path::Path) -> Result<FetchedFile> {
        self.fetch(uri)
    }

    fn head_size(&self, uri: &str) -> Option<u64> {
        let path = uri
            .strip_prefix("file://")
            .or_else(|| uri.strip_prefix("file:"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(uri));
        std::fs::metadata(&path).ok().map(|m| m.len())
    }

    fn supports_listing(&self) -> bool {
        true
    }

    fn list(&self, uri: &str) -> Result<Vec<DirEntry>> {
        let path = uri
            .strip_prefix("file://")
            .or_else(|| uri.strip_prefix("file:"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(uri));

        if !path.is_dir() {
            return Err(anyhow!("Not a directory: {:?}", path));
        }

        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&path)
            .with_context(|| format!("failed to read dir {:?}", path))?
        {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden entries (starting with '.')
            if name.starts_with('.') {
                continue;
            }
            let metadata = entry.metadata()?;
            let kind = if metadata.is_dir() {
                EntryKind::Directory
            } else {
                EntryKind::File
            };
            let full_uri = entry.path().to_string_lossy().to_string();
            entries.push(DirEntry {
                name,
                kind,
                uri: full_uri,
                size: metadata.len(),
            });
        }

        // Sort: directories first, then files; alphabetically within each group.
        entries.sort_by(|a, b| match (a.kind, b.kind) {
            (EntryKind::Directory, EntryKind::File) => std::cmp::Ordering::Less,
            (EntryKind::File, EntryKind::Directory) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });

        Ok(entries)
    }
}
