use std::path::PathBuf;

use crate::credentials::Protocol;

// ── Directory entry types ─────────────────────────────────────

/// Whether a directory entry is a navigable directory or a loadable file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Directory,
    File,
}

/// A single entry in a directory listing.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub kind: EntryKind,
    /// Full URI or path that can be passed to `Source::fetch()`.
    pub uri: String,
    /// File size in bytes (0 for directories).
    pub size: u64,
}

// ── Browser tree node types ────────────────────────────────────

/// A node in the browser tree (left panel).
///
/// The tree is stored as a flat `Vec<BrowserNode>` where each node
/// carries its `depth` so indentation can be computed at render time.
/// Expanding a node inserts its children right after it; collapsing
/// removes them.
#[derive(Debug, Clone)]
pub enum BrowserNode {
    /// A top-level or sub-category label (e.g. "Local", "Cloud Storage", "S3", "KS3").
    Category {
        label: String,
        id: String,
        depth: usize,
    },
    /// A local filesystem directory.
    LocalDir {
        path: PathBuf,
        depth: usize,
    },
    /// A ".." entry pointing at a local parent directory (for navigation up).
    LocalParent {
        path: PathBuf,
        depth: usize,
    },
    /// An S3/KS3 bucket.
    Bucket {
        scheme: String, // "s3" or "ks3"
        name: String,   // bucket name
        endpoint: Option<String>,
        region: Option<String>,
        depth: usize,
    },
    /// A prefix within a bucket (directory-like).
    Prefix {
        scheme: String,
        bucket: String,
        prefix: String, // e.g. "path/to/dir/"
        endpoint: Option<String>,
        region: Option<String>,
        depth: usize,
    },
    /// A WebDAV directory (an HTTP(S) URL pointing at a collection).
    WebDavDir {
        url: String,
        depth: usize,
    },
    /// An SSH remote directory (browseable via `ssh ls`).
    SshDir {
        uri: String,
        depth: usize,
    },
    /// A saved, named cloud connection shown under its protocol category.
    /// `uri` is the resolved root URI used for listing; `id` keys on
    /// `(protocol, name)` so editing credentials doesn't break expansion.
    CloudConn {
        protocol: Protocol,
        name: String,
        uri: String,
        depth: usize,
    },
    /// The trailing "+ New connection" node under a protocol category.
    /// Not expandable — Enter opens the connection dialog instead.
    NewConn {
        protocol: Protocol,
        depth: usize,
    },
    /// A loadable file (local or remote).
    File {
        name: String,
        uri: String,
        size: u64,
        depth: usize,
    },
}

impl BrowserNode {
    /// Unique stable identifier used for tracking expansion state.
    pub fn id(&self) -> String {
        match self {
            Self::Category { id, .. } => id.clone(),
            Self::LocalDir { path, .. } => format!("local:{}", path.display()),
            Self::LocalParent { path, .. } => format!("localparent:{}", path.display()),
            Self::Bucket {
                scheme, name, endpoint, region, ..
            } => {
                let mut s = format!("{}:{}", scheme, name);
                if let Some(e) = endpoint {
                    s.push_str(&format!("?endpoint={}", e));
                }
                if let Some(r) = region {
                    s.push_str(&format!("?region={}", r));
                }
                s
            }
            Self::Prefix {
                scheme,
                bucket,
                prefix,
                endpoint,
                region,
                ..
            } => {
                let mut s = format!("{}:{}/{}", scheme, bucket, prefix);
                if let Some(e) = endpoint {
                    s.push_str(&format!("?endpoint={}", e));
                }
                if let Some(r) = region {
                    s.push_str(&format!("?region={}", r));
                }
                s
            }
            Self::File { uri, .. } => format!("file:{}", uri),
            Self::WebDavDir { url, .. } => format!("webdav:{}", url),
            Self::SshDir { uri, .. } => format!("sshdir:{}", uri),
            Self::CloudConn { protocol, name, .. } => {
                format!("cloud.conn:{}:{}", protocol.as_str(), name)
            }
            Self::NewConn { protocol, .. } => format!("cloud.new:{}", protocol.as_str()),
        }
    }

    /// Human-readable label for rendering.
    pub fn display_label(&self) -> &str {
        match self {
            Self::Category { label, .. } => label,
            Self::LocalDir { path, .. } => path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?"),
            Self::LocalParent { .. } => "..",
            Self::Bucket { name, .. } => name,
            Self::Prefix { prefix, .. } => {
                // Show only the last path component, strip trailing '/'
                prefix
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or(prefix)
            }
            Self::File { name, .. } => name,
            Self::WebDavDir { url, .. } => {
                // Show host (e.g. "webdav.123pan.cn").
                let after = url.split("://").nth(1).unwrap_or(url);
                after.split('/').next().unwrap_or(after)
            }
            Self::SshDir { uri, .. } => {
                let after = uri.split("://").nth(1).unwrap_or(uri);
                after.split('/').next().unwrap_or(after)
            }
            Self::CloudConn { name, .. } => name,
            Self::NewConn { .. } => "+ 新建连接",
        }
    }

    /// Whether this node can be expanded to reveal children.
    pub fn is_expandable(&self) -> bool {
        match self {
            Self::NewConn { .. } | Self::File { .. } => false,
            _ => true,
        }
    }

    /// Depth in the tree (used for indentation).
    pub fn depth(&self) -> usize {
        match self {
            Self::Category { depth, .. } => *depth,
            Self::LocalDir { depth, .. } => *depth,
            Self::LocalParent { depth, .. } => *depth,
            Self::Bucket { depth, .. } => *depth,
            Self::Prefix { depth, .. } => *depth,
            Self::File { depth, .. } => *depth,
            Self::WebDavDir { depth, .. } => *depth,
            Self::SshDir { depth, .. } => *depth,
            Self::CloudConn { depth, .. } => *depth,
            Self::NewConn { depth, .. } => *depth,
        }
    }

    /// Construct the URI that `Source::list()` (or `Source::fetch()`) expects.
    pub fn list_uri(&self) -> String {
        match self {
            Self::Category { .. } => String::new(),
            Self::LocalDir { path, .. } => path.to_string_lossy().to_string(),
            Self::LocalParent { path, .. } => path.to_string_lossy().to_string(),
            Self::Bucket {
                scheme, name, endpoint, region, ..
            } => {
                let mut uri = format!("{}://{}", scheme, name);
                let mut sep = '?';
                if let Some(r) = region {
                    uri.push_str(&format!("{}region={}", sep, r));
                    sep = '&';
                }
                if let Some(e) = endpoint {
                    uri.push_str(&format!("{}endpoint={}", sep, e));
                }
                uri
            }
            Self::Prefix {
                scheme,
                bucket,
                prefix,
                endpoint,
                region,
                ..
            } => {
                let mut uri = format!("{}://{}/{}", scheme, bucket, prefix);
                let mut sep = '?';
                if let Some(r) = region {
                    uri.push_str(&format!("{}region={}", sep, r));
                    sep = '&';
                }
                if let Some(e) = endpoint {
                    uri.push_str(&format!("{}endpoint={}", sep, e));
                }
                uri
            }
            Self::File { uri, .. } => uri.clone(),
            Self::WebDavDir { url, .. } => url.clone(),
            Self::SshDir { uri, .. } => uri.clone(),
            Self::CloudConn { uri, .. } => uri.clone(),
            Self::NewConn { .. } => String::new(),
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────

/// Human-readable file size.
pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
