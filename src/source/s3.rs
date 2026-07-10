use anyhow::{anyhow, Context, Result};
use awscreds::Credentials;
use awsregion::Region;
use s3::bucket::Bucket;
use s3::serde_types::ListBucketResult;

use super::browser::{DirEntry, EntryKind};
use super::{log_fetch_complete, temp_path_for, FetchedFile, Source};

// ── URI parsing ────────────────────────────────────────────────

pub struct ParsedS3Uri {
    pub bucket: String,
    pub key: String,
    pub region: Option<String>,
    pub endpoint: Option<String>,
}

pub struct ParsedKs3Uri {
    pub bucket: String,
    pub key: String,
    pub endpoint: Option<String>,
}

/// Parse `s3://bucket/path/file?region=...&endpoint-url=...` into its parts.
pub fn parse_s3_uri(uri: &str) -> ParsedS3Uri {
    let (path_part, region, endpoint) = if let Some(q) = uri.find('?') {
        let base = &uri[..q];
        let query = &uri[q + 1..];
        let r = query_param(query, "region");
        let e = query_param(query, "endpoint-url").or_else(|| query_param(query, "endpoint"));
        (base.to_string(), r, e)
    } else {
        (uri.to_string(), None, None)
    };

    // strip "s3://" and split into bucket + key
    let rest = path_part.strip_prefix("s3://").unwrap_or(&path_part);
    let (bucket, key) = match rest.find('/') {
        Some(i) => (rest[..i].to_string(), rest[i + 1..].to_string()),
        None => (rest.to_string(), String::new()),
    };

    let region = region.or_else(|| std::env::var("AWS_DEFAULT_REGION").ok());
    let endpoint = endpoint.or_else(|| std::env::var("NVIS_S3_ENDPOINT").ok());

    ParsedS3Uri {
        bucket,
        key,
        region,
        endpoint,
    }
}

/// Parse `ks3://bucket/path/file?endpoint=...` into its parts.
pub fn parse_ks3_uri(uri: &str) -> ParsedKs3Uri {
    let (path_part, endpoint) = if let Some(q) = uri.find('?') {
        let base = &uri[..q];
        let query = &uri[q + 1..];
        let e = query_param(query, "endpoint");
        (base.to_string(), e)
    } else {
        (uri.to_string(), None)
    };

    let rest = path_part.strip_prefix("ks3://").unwrap_or(&path_part);
    let (bucket, key) = match rest.find('/') {
        Some(i) => (rest[..i].to_string(), rest[i + 1..].to_string()),
        None => (rest.to_string(), String::new()),
    };

    let endpoint = endpoint.or_else(|| std::env::var("NVIS_KS3_ENDPOINT").ok());

    ParsedKs3Uri {
        bucket,
        key,
        endpoint,
    }
}

fn query_param(query: &str, key: &str) -> Option<String> {
    for pair in query.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            if k == key {
                return Some(v.to_string());
            }
        }
    }
    None
}

// ── KS3 config file ────────────────────────────────────────────

pub struct Ks3Config {
    pub endpoint: String,
    pub access_key_id: String,
    pub access_key_secret: String,
}

/// Read `~/.ks3utilconfig` and extract endpoint + credentials.
pub fn parse_ks3_config() -> Result<Ks3Config> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    let path = home.join(".ks3utilconfig");
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("failed to read {:?}", path))?;

    parse_ks3_config_from_str(&content)
}

/// Public alias used by the browser to read KS3 config.
pub fn read_ks3_config() -> Result<Ks3Config> {
    parse_ks3_config()
}

/// Parse the INI content of a ks3utilconfig file.
pub fn parse_ks3_config_from_str(content: &str) -> Result<Ks3Config> {
    let mut in_creds = false;
    let mut endpoint = None::<String>;
    let mut ak = None::<String>;
    let mut sk = None::<String>;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_creds = trimmed == "[Credentials]";
            continue;
        }
        if !in_creds {
            continue;
        }
        if let Some((k, v)) = trimmed.split_once('=') {
            match k.trim() {
                "endpoint" => endpoint = Some(v.trim().to_string()),
                "accessKeyID" => ak = Some(v.trim().to_string()),
                "accessKeySecret" => sk = Some(v.trim().to_string()),
                _ => {}
            }
        }
    }

    Ok(Ks3Config {
        endpoint: endpoint.context("missing endpoint in ~/.ks3utilconfig")?,
        access_key_id: ak.context("missing accessKeyID in ~/.ks3utilconfig")?,
        access_key_secret: sk.context("missing accessKeySecret in ~/.ks3utilconfig")?,
    })
}

// ── Bucket construction ────────────────────────────────────────

fn build_s3_bucket(parsed: &ParsedS3Uri) -> Result<Box<Bucket>> {
    let region = match (&parsed.region, &parsed.endpoint) {
        (Some(r), Some(e)) => Region::Custom {
            region: r.clone(),
            endpoint: ensure_https(e),
        },
        (None, Some(e)) => Region::Custom {
            region: "us-east-1".to_string(),
            endpoint: ensure_https(e),
        },
        (Some(r), None) => r
            .parse::<Region>()
            .map_err(|e| anyhow!("invalid AWS region '{}': {}", r, e))?,
        (None, None) => Region::from_default_env().unwrap_or_else(|_| Region::UsEast1),
    };

    // Prefer a saved connection's credentials; fall back to AWS env/credentials.
    let credentials = if let Some(c) =
        crate::credentials::find_s3_for(&parsed.bucket, parsed.endpoint.as_deref(), false)
    {
        if c.access_key.is_empty() || c.secret_key.is_empty() {
            Credentials::default()
                .map_err(|e| anyhow!("AWS credentials not found: {}", e))?
        } else {
            Credentials::new(Some(&c.access_key), Some(&c.secret_key), None, None, None)
                .map_err(|e| anyhow!("failed to create S3 credentials: {}", e))?
        }
    } else {
        Credentials::default()
            .map_err(|e| anyhow!("AWS credentials not found: {}", e))?
    };
    Ok(Bucket::new(&parsed.bucket, region, credentials)
        .map_err(|e| anyhow!("failed to create S3 bucket client: {}", e))?
        .with_request_timeout(std::time::Duration::from_secs(300))
        .map_err(|e| anyhow!("failed to set S3 request timeout: {}", e))?)
}

fn build_ks3_bucket(parsed: &ParsedKs3Uri) -> Result<Box<Bucket>> {
    // Prefer a saved connection's endpoint + credentials; fall back to config.
    let saved = crate::credentials::find_s3_for(&parsed.bucket, parsed.endpoint.as_deref(), true);

    let endpoint = match (&parsed.endpoint, &saved) {
        (Some(e), _) => ensure_https(e),
        (None, Some(c)) if !c.endpoint.is_empty() => ensure_https(&c.endpoint),
        (None, _) => {
            let config = parse_ks3_config()?;
            ensure_https(&config.endpoint)
        }
    };

    let region = Region::Custom {
        region: "cn-beijing".to_string(),
        endpoint,
    };

    let credentials = if let Some(ref c) = saved {
        if c.access_key.is_empty() || c.secret_key.is_empty() {
            let config = parse_ks3_config()?;
            ks3_credentials(&config)?
        } else {
            Credentials::new(Some(&c.access_key), Some(&c.secret_key), None, None, None)
                .map_err(|e| anyhow!("failed to create KS3 credentials: {}", e))?
        }
    } else {
        let config = parse_ks3_config()?;
        ks3_credentials(&config)?
    };

    let bucket = Bucket::new(&parsed.bucket, region, credentials)
        .map_err(|e| anyhow!("failed to create KS3 bucket client: {}", e))?
        .with_request_timeout(std::time::Duration::from_secs(300))
        .map_err(|e| anyhow!("failed to set KS3 request timeout: {}", e))?;
    Ok(bucket)
}

fn ks3_credentials(config: &Ks3Config) -> Result<Credentials> {
    Credentials::new(
        Some(&config.access_key_id),
        Some(&config.access_key_secret),
        None,
        None,
        None,
    )
    .map_err(|e| anyhow!("failed to create KS3 credentials: {}", e))
}

/// Ensure the endpoint string starts with `https://` (or `http://` if already present).
pub fn ensure_https(endpoint: &str) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint.to_string()
    } else {
        format!("https://{}", endpoint)
    }
}

// ── Source trait ────────────────────────────────────────────────

/// Source for S3-compatible URIs.
///
/// Supported schemes:
/// - `s3://bucket/path/to/file` — native S3 client, credentials from
///   AWS env vars / `~/.aws/credentials`
/// - `ks3://bucket/path/to/file` — native S3 client, credentials from
///   `~/.ks3utilconfig`
///
/// Optional query params:
/// - `?region=us-east-1`
/// - `?endpoint-url=http://minio:9000` (s3) / `?endpoint=...` (ks3)
///
/// Environment variable overrides:
/// - `NVIS_S3_ENDPOINT` — custom S3 endpoint
/// - `NVIS_KS3_ENDPOINT` — custom KS3 endpoint
pub struct S3Source;

impl Source for S3Source {
    fn id(&self) -> &str {
        "s3"
    }

    fn name(&self) -> &str {
        "S3 (native)"
    }

    fn can_handle(&self, uri: &str) -> bool {
        uri.starts_with("s3://") || uri.starts_with("ks3://")
    }

    fn fetch(&self, uri: &str) -> Result<FetchedFile> {
        if uri.starts_with("ks3://") {
            self.fetch_ks3(uri)
        } else {
            self.fetch_s3(uri)
        }
    }

    fn fetch_to(&self, uri: &str, dest: &std::path::Path) -> Result<FetchedFile> {
        if uri.starts_with("ks3://") {
            self.fetch_ks3_to(uri, dest)
        } else {
            self.fetch_s3_to(uri, dest)
        }
    }

    fn head_size(&self, uri: &str) -> Option<u64> {
        if uri.starts_with("ks3://") {
            let parsed = parse_ks3_uri(uri);
            if parsed.key.is_empty() {
                return None;
            }
            let bucket = build_ks3_bucket(&parsed).ok()?;
            s3_head_size(&bucket, &parsed.key)
        } else {
            let parsed = parse_s3_uri(uri);
            if parsed.key.is_empty() {
                return None;
            }
            let bucket = build_s3_bucket(&parsed).ok()?;
            s3_head_size(&bucket, &parsed.key)
        }
    }

    fn supports_listing(&self) -> bool {
        true
    }

    fn list(&self, uri: &str) -> Result<Vec<DirEntry>> {
        if uri.starts_with("ks3://") {
            self.list_ks3(uri)
        } else {
            self.list_s3(uri)
        }
    }
}

impl S3Source {
    fn fetch_s3(&self, uri: &str) -> Result<FetchedFile> {
        let parsed = parse_s3_uri(uri);
        if parsed.key.is_empty() {
            return Err(anyhow!("S3 URI missing object key: {}", uri));
        }

        let bucket = build_s3_bucket(&parsed)?;
        let temp_name = format!("s3://{}/{}", parsed.bucket, parsed.key);
        fetch_to_file(&bucket, &parsed.key, &temp_name, uri, self.id())
    }

    fn fetch_s3_to(&self, uri: &str, dest: &std::path::Path) -> Result<FetchedFile> {
        let parsed = parse_s3_uri(uri);
        if parsed.key.is_empty() {
            return Err(anyhow!("S3 URI missing object key: {}", uri));
        }
        let bucket = build_s3_bucket(&parsed)?;
        fetch_to_file_dest(&bucket, &parsed.key, dest, uri, self.id())
    }

    fn fetch_ks3(&self, uri: &str) -> Result<FetchedFile> {
        let parsed = parse_ks3_uri(uri);
        if parsed.key.is_empty() {
            return Err(anyhow!("KS3 URI missing object key: {}", uri));
        }

        let bucket = build_ks3_bucket(&parsed)?;
        let temp_name = format!("ks3://{}/{}", parsed.bucket, parsed.key);
        fetch_to_file(&bucket, &parsed.key, &temp_name, uri, self.id())
    }

    fn fetch_ks3_to(&self, uri: &str, dest: &std::path::Path) -> Result<FetchedFile> {
        let parsed = parse_ks3_uri(uri);
        if parsed.key.is_empty() {
            return Err(anyhow!("KS3 URI missing object key: {}", uri));
        }
        let bucket = build_ks3_bucket(&parsed)?;
        fetch_to_file_dest(&bucket, &parsed.key, dest, uri, self.id())
    }

    fn list_s3(&self, uri: &str) -> Result<Vec<DirEntry>> {
        let parsed = parse_s3_uri(uri);
        let bucket = build_s3_bucket(&parsed)?;
        let prefix = if parsed.key.is_empty() {
            String::new()
        } else if parsed.key.ends_with('/') {
            parsed.key.clone()
        } else {
            format!("{}/", parsed.key)
        };
        list_bucket(&bucket, &parsed.bucket, &prefix, "s3", &parsed.region, &parsed.endpoint)
    }

    fn list_ks3(&self, uri: &str) -> Result<Vec<DirEntry>> {
        let parsed = parse_ks3_uri(uri);
        let bucket = build_ks3_bucket(&parsed)?;
        let prefix = if parsed.key.is_empty() {
            String::new()
        } else if parsed.key.ends_with('/') {
            parsed.key.clone()
        } else {
            format!("{}/", parsed.key)
        };
        list_bucket(&bucket, &parsed.bucket, &prefix, "ks3", &None, &parsed.endpoint)
    }
}

/// Common download logic: `get_object_to_writer` → stream to temp file.
fn fetch_to_file(
    bucket: &Bucket,
    key: &str,
    temp_name: &str,
    uri: &str,
    source_id: &str,
) -> Result<FetchedFile> {
    let local_path = temp_path_for(temp_name);
    fetch_to_file_dest(bucket, key, &local_path, uri, source_id)
}

/// Download directly to `dest` (for background downloads with progress).
fn fetch_to_file_dest(
    bucket: &Bucket,
    key: &str,
    dest: &std::path::Path,
    uri: &str,
    source_id: &str,
) -> Result<FetchedFile> {
    let local_path = dest.to_path_buf();

    log::info!("Fetching {} via native S3 client -> {:?}", key, local_path);

    let mut file = std::fs::File::create(&local_path)
        .with_context(|| format!("failed to create file {:?}", local_path))?;

    let status = bucket
        .get_object_to_writer(key, &mut file)
        .map_err(|e| anyhow!("S3 GetObject failed for {}: {}", key, e))?;

    if status != 200 {
        let _ = std::fs::remove_file(&local_path);
        return Err(anyhow!("S3 GetObject for {} returned HTTP {}", key, status));
    }

    log_fetch_complete(&local_path);
    Ok(FetchedFile::remote(local_path, uri.to_string(), source_id))
}

/// Best-effort object size via `head_object`.
fn s3_head_size(bucket: &Bucket, key: &str) -> Option<u64> {
    let (head, status) = bucket.head_object(key).ok()?;
    if status != 200 {
        return None;
    }
    let size = head.content_length?;
    Some(size as u64)
}

/// List entries in an S3/KS3 bucket under a given prefix.
/// Uses `common_prefixes` for directories and `contents` for files.
fn list_bucket(
    bucket: &Bucket,
    bucket_name: &str,
    prefix: &str,
    scheme: &str,
    region: &Option<String>,
    endpoint: &Option<String>,
) -> Result<Vec<DirEntry>> {
    let results: Vec<ListBucketResult> = bucket
        .list(prefix.to_string(), Some("/".to_string()))
        .map_err(|e| anyhow!("S3 ListObjects failed for {}/{}: {}", bucket_name, prefix, e))?;

    let mut entries = Vec::new();

    for result in &results {
        // Directories from common prefixes
        if let Some(ref cps) = result.common_prefixes {
            for cp in cps {
                let p = &cp.prefix;
                // Show only the last path component as the name
                let name = p
                    .trim_end_matches('/')
                    .rsplit('/')
                    .next()
                    .unwrap_or(p)
                    .to_string();
                let mut dir_uri = format!("{}://{}/{}", scheme, bucket_name, p);
                let mut sep = '?';
                if let Some(r) = region {
                    dir_uri.push_str(&format!("{}region={}", sep, r));
                    sep = '&';
                }
                if let Some(e) = endpoint {
                    dir_uri.push_str(&format!("{}endpoint={}", sep, e));
                }
                entries.push(DirEntry {
                    name,
                    kind: EntryKind::Directory,
                    uri: dir_uri,
                    size: 0,
                });
            }
        }

        // Files from contents (skip zero-size directory markers)
        for obj in &result.contents {
            if obj.size == 0 {
                continue; // skip directory marker objects
            }
            let key = &obj.key;
            let name = key.rsplit('/').next().unwrap_or(key).to_string();
            let mut file_uri = format!("{}://{}/{}", scheme, bucket_name, key);
            let mut sep = '?';
            if let Some(r) = region {
                file_uri.push_str(&format!("{}region={}", sep, r));
                sep = '&';
            }
            if let Some(e) = endpoint {
                file_uri.push_str(&format!("{}endpoint={}", sep, e));
            }
            entries.push(DirEntry {
                name,
                kind: EntryKind::File,
                uri: file_uri,
                size: obj.size as u64,
            });
        }
    }

    // Sort: directories first, then files
    entries.sort_by(|a, b| match (a.kind, b.kind) {
        (EntryKind::Directory, EntryKind::File) => std::cmp::Ordering::Less,
        (EntryKind::File, EntryKind::Directory) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    Ok(entries)
}
