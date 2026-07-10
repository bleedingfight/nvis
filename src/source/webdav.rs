use anyhow::{anyhow, Context, Result};
use std::process::Command;

use super::{temp_path_for, log_fetch_complete, FetchedFile, Source};
use super::browser::{DirEntry, EntryKind};

/// Source for WebDAV / HTTP URLs.
///
/// Supported schemes:
/// - `webdav://host/path`  → `http://host/path`
/// - `webdavs://host/path` → `https://host/path`
/// - `http://` and `https://` URLs directly
///
/// Authentication (optional):
/// - `NVIS_WEBDAV_USER` / `NVIS_WEBDAV_PASS` environment variables
/// - `~/.netrc` (handled natively by curl)
/// - URL-embedded credentials (`https://user:pass@host/path`)
pub struct WebDavSource;

impl Source for WebDavSource {
    fn id(&self) -> &str {
        "webdav"
    }

    fn name(&self) -> &str {
        "WebDAV / HTTP (curl)"
    }

    fn can_handle(&self, uri: &str) -> bool {
        uri.starts_with("webdav://")
            || uri.starts_with("webdavs://")
            || uri.starts_with("http://")
            || uri.starts_with("https://")
    }

    fn fetch(&self, uri: &str) -> Result<FetchedFile> {
        let dest = temp_path_for(&normalize_url(uri));
        self.fetch_to(uri, &dest)
    }

    fn fetch_to(&self, uri: &str, dest: &std::path::Path) -> Result<FetchedFile> {
        let url = normalize_url(uri);
        let local_path = dest.to_path_buf();

        let mut cmd = Command::new("curl");
        cmd.arg("-sSL") // silent, show errors, follow redirects
            .arg("--fail") // non-zero exit on HTTP 4xx/5xx
            .arg("--connect-timeout")
            .arg("15")
            .arg("--max-time")
            .arg("300")
            .arg("-o")
            .arg(&local_path);

        // Optional basic auth. Precedence:
        //   1. URL-embedded credentials (handled natively by curl)
        //   2. A saved connection whose `url` is a prefix of this one
        //   3. NVIS_WEBDAV_USER / NVIS_WEBDAV_PASS environment variables
        if let Some(c) = crate::credentials::find_webdav_for_uri(&url) {
            if !c.user.is_empty() || !c.pass.is_empty() {
                cmd.arg("--user").arg(format!("{}:{}", c.user, c.pass));
            }
        } else if let (Ok(user), Ok(pass)) = (
            std::env::var("NVIS_WEBDAV_USER"),
            std::env::var("NVIS_WEBDAV_PASS"),
        ) {
            cmd.arg("--user").arg(format!("{}:{}", user, pass));
        }

        cmd.arg(&url);

        log::info!("Running curl: {:?}", cmd);

        let output = cmd
            .output()
            .with_context(|| "failed to execute `curl` — is it installed?")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let _ = std::fs::remove_file(&local_path);
            return Err(anyhow!(
                "curl exited with status {}: {}",
                output.status,
                stderr.trim()
            ));
        }

        if !local_path.exists() {
            return Err(anyhow!(
                "curl reported success but the destination file is missing"
            ));
        }

        log_fetch_complete(&local_path);
        Ok(FetchedFile::remote(local_path, uri.to_string(), self.id()))
    }

    fn head_size(&self, uri: &str) -> Option<u64> {
        let url = normalize_url(uri);
        let mut cmd = Command::new("curl");
        cmd.arg("-sI")
            .arg("--fail")
            .arg("--connect-timeout")
            .arg("15")
            .arg("--max-time")
            .arg("20");
        if let Some(c) = crate::credentials::find_webdav_for_uri(&url) {
            if !c.user.is_empty() || !c.pass.is_empty() {
                cmd.arg("--user").arg(format!("{}:{}", c.user, c.pass));
            }
        } else if let (Ok(user), Ok(pass)) = (
            std::env::var("NVIS_WEBDAV_USER"),
            std::env::var("NVIS_WEBDAV_PASS"),
        ) {
            cmd.arg("--user").arg(format!("{}:{}", user, pass));
        }
        cmd.arg(&url);
        let out = cmd.output().ok()?;
        if !out.status.success() {
            return None;
        }
        let headers = String::from_utf8_lossy(&out.stdout);
        for line in headers.lines() {
            if let Some((k, v)) = line.split_once(':') {
                if k.trim().eq_ignore_ascii_case("content-length") {
                    if let Ok(n) = v.trim().parse::<u64>() {
                        return Some(n);
                    }
                }
            }
        }
        None
    }

    fn supports_listing(&self) -> bool {
        true
    }

    fn list(&self, uri: &str) -> Result<Vec<DirEntry>> {
        list_webdav(uri)
    }
}

/// Convert `webdav://` / `webdavs://` to `http://` / `https://`.
/// Other URLs pass through unchanged.
pub fn normalize_url(uri: &str) -> String {
    if let Some(rest) = uri.strip_prefix("webdav://") {
        format!("http://{}", rest)
    } else if let Some(rest) = uri.strip_prefix("webdavs://") {
        format!("https://{}", rest)
    } else {
        uri.to_string()
    }
}

/// Split a URL into `(scheme://host, path)`, ensuring `path` ends with `/`.
fn split_url(url: &str) -> (String, String) {
    let scheme_end = url.find("://").map(|i| i + 3).unwrap_or(0);
    let after_scheme = &url[scheme_end..];
    let host_end = after_scheme
        .find('/')
        .map(|i| scheme_end + i)
        .unwrap_or(url.len());
    let scheme_host = url[..host_end].to_string();
    let path = url[host_end..].to_string();
    let path = if path.is_empty() {
        "/".to_string()
    } else if path.ends_with('/') {
        path
    } else {
        format!("{}/", path)
    };
    (scheme_host, path)
}

/// Percent-decode a path segment (e.g. `%20` → space). Best-effort.
fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
            if let Ok(b) = u8::from_str_radix(hex, 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Case-insensitive find from a byte offset, returning a byte index.
fn find_ci(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    haystack[from..]
        .to_ascii_lowercase()
        .find(needle)
        .map(|i| from + i)
}

/// List the contents of a WebDAV directory via `PROPFIND Depth: 1`.
pub fn list_webdav(uri: &str) -> Result<Vec<DirEntry>> {
    let url = normalize_url(uri);
    let list_url = if url.ends_with('/') {
        url.clone()
    } else {
        format!("{}/", url)
    };
    let (scheme_host, base_path) = split_url(&list_url);

    let mut cmd = Command::new("curl");
    cmd.arg("-s")
        .arg("--connect-timeout")
        .arg("15")
        .arg("--max-time")
        .arg("60")
        .arg("-X")
        .arg("PROPFIND")
        .arg("-H")
        .arg("Depth: 1")
        .arg("-H")
        .arg("Content-Type: application/xml");

    // Auth — same precedence as fetch(): saved-connection match → env.
    if let Some(c) = crate::credentials::find_webdav_for_uri(&list_url) {
        if !c.user.is_empty() || !c.pass.is_empty() {
            cmd.arg("--user").arg(format!("{}:{}", c.user, c.pass));
        }
    } else if let (Ok(user), Ok(pass)) = (
        std::env::var("NVIS_WEBDAV_USER"),
        std::env::var("NVIS_WEBDAV_PASS"),
    ) {
        cmd.arg("--user").arg(format!("{}:{}", user, pass));
    }
    cmd.arg(&list_url);

    log::info!("Running curl PROPFIND: {:?}", cmd);
    let out = cmd
        .output()
        .with_context(|| "failed to execute `curl` — is it installed?")?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!(
            "WebDAV listing failed (curl status {}): {}",
            out.status,
            stderr.trim()
        ));
    }
    let body = String::from_utf8_lossy(&out.stdout);
    let entries = parse_propfind(&body, &scheme_host, &base_path);
    if entries.is_empty() {
        // PROPFIND succeeded but returned no children. This usually means the
        // URL points at a *file* (not a collection) — its only response is the
        // self entry, which we skip. Tell the caller so they can guide the user
        // to use a directory URL instead.
        Err(anyhow!(
            "no children — URL points at a file, not a directory. \
             Edit the connection and use a directory URL."
        ))
    } else {
        Ok(entries)
    }
}

/// Parse a WebDAV `PROPFIND` multistatus body into directory entries.
///
/// Tolerant of the `D:` / `d:` namespace prefix. The directory itself (the
/// entry whose href equals `base_path`) is skipped so only children remain.
fn parse_propfind(body: &str, scheme_host: &str, base_path: &str) -> Vec<DirEntry> {
    let mut entries = Vec::new();
    let base_norm = base_path.trim_end_matches('/');
    let mut pos = 0;

    while let Some(open) = find_ci(body, "<d:response", pos) {
        let close = find_ci(body, "</d:response>", open).unwrap_or(body.len());
        let block = &body[open..close];
        let block_lower = block.to_ascii_lowercase();

        // Extract the (first) <d:href>...</d:href> content.
        let href = find_ci(block, "<d:href", 0).and_then(|hs| {
            let gt = block[hs..].find('>').map(|i| hs + i + 1)?;
            let he = find_ci(block, "</d:href>", gt)?;
            Some(block[gt..he].to_string())
        });

        if let Some(href) = href {
            let href_path = href.split('?').next().unwrap_or(&href);
            // Skip the directory's own self entry.
            let self_ref = href_path.trim_end_matches('/') == base_norm;
            if !self_ref {
                let is_dir = block_lower.find("<d:collection").is_some();
                let size = find_ci(block, "<d:getcontentlength", 0)
                    .and_then(|gs| {
                        let gt = block[gs..].find('>').map(|i| gs + i + 1)?;
                        let he = find_ci(block, "</d:getcontentlength>", gt)?;
                        block[gt..he].parse::<u64>().ok()
                    })
                    .unwrap_or(0);

                // Build the absolute child URL.
                let child_url = if href.starts_with("http://") || href.starts_with("https://") {
                    href.clone()
                } else if href.starts_with('/') {
                    format!("{}{}", scheme_host, href)
                } else {
                    format!("{}{}", scheme_host, base_path) + &href
                };

                // Name = last path segment, percent-decoded.
                let path_for_name = href_path.trim_end_matches('/');
                let raw_name = path_for_name.rsplit('/').next().unwrap_or(path_for_name);
                let name = percent_decode(raw_name);

                entries.push(DirEntry {
                    name,
                    kind: if is_dir { EntryKind::Directory } else { EntryKind::File },
                    uri: child_url,
                    size,
                });
            }
        }

        pos = close + 1;
    }

    entries.sort_by(|a, b| match (a.kind, b.kind) {
        (EntryKind::Directory, EntryKind::File) => std::cmp::Ordering::Less,
        (EntryKind::File, EntryKind::Directory) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    entries
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_webdav_scheme() {
        assert_eq!(normalize_url("webdav://host/path"), "http://host/path");
        assert_eq!(normalize_url("webdavs://host/path"), "https://host/path");
    }

    #[test]
    fn passes_through_http_urls() {
        assert_eq!(normalize_url("http://host/path"), "http://host/path");
        assert_eq!(normalize_url("https://host/path"), "https://host/path");
    }

    #[test]
    fn can_handle_http_schemes() {
        let s = WebDavSource;
        assert!(s.can_handle("webdav://host/x"));
        assert!(s.can_handle("webdavs://host/x"));
        assert!(s.can_handle("http://host/x"));
        assert!(s.can_handle("https://host/x"));
        assert!(!s.can_handle("ssh:host:/x"));
        assert!(!s.can_handle("/local/path"));
    }

    #[test]
    fn parses_propfind_children() {
        // Two children: a sub-dir "notes/" and a file "report.sqlite" (1234 B).
        // The self entry (base path) must be skipped.
        let body = "<?xml version=\"1.0\"?>\
<D:multistatus xmlns:D=\"DAV:\">\
<D:response><D:href>/webdav/profiler/</D:href>\
<D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop>\
<D:status>HTTP/1.1 200 OK</D:status></D:propstat></D:response>\
<D:response><D:href>/webdav/profiler/notes/</D:href>\
<D:propstat><D:prop><D:resourcetype><D:collection/></D:resourcetype></D:prop></D:propstat></D:response>\
<D:response><D:href>/webdav/profiler/report.sqlite</D:href>\
<D:propstat><D:prop><D:resourcetype/>\
<D:getcontentlength>1234</D:getcontentlength></D:prop></D:propstat></D:response>\
</D:multistatus>";
        let entries = parse_propfind(body, "https://webdav.123pan.cn", "/webdav/profiler/");
        // self entry skipped → 2 children
        assert_eq!(entries.len(), 2);
        // sorted: directories first
        assert_eq!(entries[0].name, "notes");
        assert_eq!(entries[0].kind, EntryKind::Directory);
        assert_eq!(entries[0].uri, "https://webdav.123pan.cn/webdav/profiler/notes/");
        assert_eq!(entries[1].name, "report.sqlite");
        assert_eq!(entries[1].kind, EntryKind::File);
        assert_eq!(entries[1].size, 1234);
        assert_eq!(
            entries[1].uri,
            "https://webdav.123pan.cn/webdav/profiler/report.sqlite"
        );
    }

    #[test]
    fn percent_decodes_names() {
        let body = "<D:multistatus xmlns:D=\"DAV:\">\
<D:response><D:href>/d/a%20file.sqlite</D:href><D:propstat><D:prop><D:resourcetype/>\
<D:getcontentlength>10</D:getcontentlength></D:prop></D:propstat></D:response>\
</D:multistatus>";
        let entries = parse_propfind(body, "https://h", "/d/");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "a file.sqlite");
    }

    #[test]
    fn split_url_normalizes_trailing_slash() {
        assert_eq!(split_url("https://h/webdav/p"), ("https://h".into(), "/webdav/p/".into()));
        assert_eq!(split_url("https://h/webdav/p/"), ("https://h".into(), "/webdav/p/".into()));
        assert_eq!(split_url("https://h"), ("https://h".into(), "/".into()));
    }
}
