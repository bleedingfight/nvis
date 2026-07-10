//! Encrypted local connection store.
//!
//! Cloud connections (SSH / WebDAV / S3 / KS3) entered in the connection
//! dialog are encrypted and persisted locally as named shortcuts so the user
//! does not have to re-enter them every session. On startup the saved
//! connections are listed under their protocol category node in the browser
//! tree; selecting one expands its remote directory for browsing.
//!
//! ## Threat model & honesty
//!
//! The encryption key is derived from a constant baked into the binary, so
//! this is **obfuscation-grade** protection, not a real secret store: it
//! keeps credentials out of plaintext on disk and defeats casual snooping
//! (`grep`, accidental backups, screen-sharing), but a determined attacker
//! who has both the binary and the connection file can recover them. For
//! high-value secrets prefer the OS keychain. We accept this trade-off
//! because the goal here is zero-friction auto-fill, which rules out a
//! master password prompt.
//!
//! ## Scheme
//!
//! - 16 random bytes from `/dev/urandom` are used as a per-record salt.
//! - A keystream is produced by chaining `SHA256(salt || APP_SECRET || counter)`
//!   in 32-byte blocks (a CTR-style hash stream cipher).
//! - `ciphertext = plaintext XOR keystream`.
//! - On disk we store `base64(salt || ciphertext)`.
//!
//! The plaintext is a length-prefixed blob: a record count followed by one
//! block per `CloudConnection` (each field `len(u32) + bytes`), so no extra
//! serialization dependency is needed.

use anyhow::{Context, Result};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::PathBuf;

/// Secret baked into the binary to derive the keystream.
const APP_SECRET: &[u8] = b"nvis-credential-store-v1";

/// Salt length in bytes.
const SALT_LEN: usize = 16;

/// Which cloud protocol a connection uses. Stored as the first field of a
/// `CloudConnection` and used to tag browser tree nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
    Ssh,
    WebDav,
    S3,
    Ks3,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ssh => "ssh",
            Self::WebDav => "webdav",
            Self::S3 => "s3",
            Self::Ks3 => "ks3",
        }
    }

    /// Map a category node id suffix (`cloud.s3` → `s3`) to a protocol.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "ssh" => Some(Self::Ssh),
            "webdav" => Some(Self::WebDav),
            "s3" => Some(Self::S3),
            "ks3" => Some(Self::Ks3),
            _ => None,
        }
    }

    /// Human-readable connection title, e.g. "SSH Connection".
    pub fn title(self) -> &'static str {
        match self {
            Self::Ssh => "SSH Connection",
            Self::WebDav => "WebDAV Connection",
            Self::S3 => "S3 Connection",
            Self::Ks3 => "KS3 Connection",
        }
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A saved, named cloud connection. A flat struct where each protocol uses a
/// subset of the fields; unused fields are stored empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloudConnection {
    pub protocol_tag: String, // Protocol::as_str; stored as string for the blob
    pub name: String,
    // WebDAV
    pub url: String,
    pub user: String,
    pub pass: String,
    // SSH
    pub host: String,
    pub port: String,
    pub path: String,
    // S3 / KS3
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
}

impl CloudConnection {
    pub fn protocol(&self) -> Option<Protocol> {
        match self.protocol_tag.as_str() {
            "ssh" => Some(Protocol::Ssh),
            "webdav" => Some(Protocol::WebDav),
            "s3" => Some(Protocol::S3),
            "ks3" => Some(Protocol::Ks3),
            _ => None,
        }
    }

    /// Build a connection from a protocol + its field values.
    pub fn from_protocol(
        protocol: Protocol,
        name: &str,
        fields: &ConnectionFields,
    ) -> Self {
        Self {
            protocol_tag: protocol.as_str().to_string(),
            name: name.to_string(),
            url: fields.url.clone(),
            user: fields.user.clone(),
            pass: fields.pass.clone(),
            host: fields.host.clone(),
            port: fields.port.clone(),
            path: fields.path.clone(),
            bucket: fields.bucket.clone(),
            region: fields.region.clone(),
            endpoint: fields.endpoint.clone(),
            access_key: fields.access_key.clone(),
            secret_key: fields.secret_key.clone(),
        }
    }
}

/// Field values for a dialog; `from_protocol` consumes the relevant subset.
#[derive(Debug, Clone, Default)]
pub struct ConnectionFields {
    pub url: String,
    pub user: String,
    pub pass: String,
    pub host: String,
    pub port: String,
    pub path: String,
    pub bucket: String,
    pub region: String,
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
}

impl From<&CloudConnection> for ConnectionFields {
    fn from(c: &CloudConnection) -> Self {
        Self {
            url: c.url.clone(),
            user: c.user.clone(),
            pass: c.pass.clone(),
            host: c.host.clone(),
            port: c.port.clone(),
            path: c.path.clone(),
            bucket: c.bucket.clone(),
            region: c.region.clone(),
            endpoint: c.endpoint.clone(),
            access_key: c.access_key.clone(),
            secret_key: c.secret_key.clone(),
        }
    }
}

/// The on-disk field order for a single `CloudConnection`. Any change here
/// breaks older files, so bump `APP_SECRET` if you reorder.
const RECORD_FIELDS: [&str; 13] = [
    "protocol_tag", "name", "url", "user", "pass", "host", "port", "path",
    "bucket", "region", "endpoint", "access_key", "secret_key",
];

fn field_of<'a>(c: &'a CloudConnection, name: &str) -> &'a str {
    match name {
        "protocol_tag" => &c.protocol_tag,
        "name" => &c.name,
        "url" => &c.url,
        "user" => &c.user,
        "pass" => &c.pass,
        "host" => &c.host,
        "port" => &c.port,
        "path" => &c.path,
        "bucket" => &c.bucket,
        "region" => &c.region,
        "endpoint" => &c.endpoint,
        "access_key" => &c.access_key,
        "secret_key" => &c.secret_key,
        _ => "",
    }
}

fn set_field(c: &mut CloudConnection, name: &str, val: String) {
    match name {
        "protocol_tag" => c.protocol_tag = val,
        "name" => c.name = val,
        "url" => c.url = val,
        "user" => c.user = val,
        "pass" => c.pass = val,
        "host" => c.host = val,
        "port" => c.port = val,
        "path" => c.path = val,
        "bucket" => c.bucket = val,
        "region" => c.region = val,
        "endpoint" => c.endpoint = val,
        "access_key" => c.access_key = val,
        "secret_key" => c.secret_key = val,
        _ => {}
    }
}

/// Path of the encrypted connection store: `<config_dir>/nvis/connections.dat`.
fn connections_path() -> Result<PathBuf> {
    let dir = dirs::config_dir().context("cannot determine config directory")?;
    Ok(dir.join("nvis").join("connections.dat"))
}

/// Legacy single-WebDAV-cred file (pre-multi-connection). Used for migration.
fn legacy_webdav_path() -> Result<PathBuf> {
    let dir = dirs::config_dir().context("cannot determine config directory")?;
    Ok(dir.join("nvis").join("webdav.dat"))
}

/// Read `n` cryptographically random bytes from `/dev/urandom`.
fn random_bytes(n: usize) -> Result<Vec<u8>> {
    let mut f = std::fs::File::open("/dev/urandom").context("cannot open /dev/urandom")?;
    let mut buf = vec![0u8; n];
    f.read_exact(&mut buf).context("cannot read /dev/urandom")?;
    Ok(buf)
}

/// One 32-byte keystream block: `SHA256(salt || APP_SECRET || counter_le)`.
fn keystream_block(salt: &[u8], counter: u32) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(salt);
    h.update(APP_SECRET);
    h.update(counter.to_le_bytes());
    h.finalize().into()
}

/// XOR `data` with the keystream derived from `salt`.
fn xor_keystream(salt: &[u8], data: &mut [u8]) {
    let mut counter = 0u32;
    let mut pos = 0;
    while pos < data.len() {
        let block = keystream_block(salt, counter);
        counter += 1;
        let take = std::cmp::min(block.len(), data.len() - pos);
        for (i, b) in block[..take].iter().enumerate() {
            data[pos + i] ^= b;
        }
        pos += take;
    }
}

/// Serialize a list of connections as a length-prefixed blob:
/// `record_count(u32)` + per record, each field `len(u32) + bytes`.
fn encode_list(conns: &[CloudConnection]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(conns.len() as u32).to_le_bytes());
    for c in conns {
        for fname in RECORD_FIELDS.iter() {
            let val = field_of(c, fname).as_bytes();
            out.extend_from_slice(&(val.len() as u32).to_le_bytes());
            out.extend_from_slice(val);
        }
    }
    out
}

/// Deserialize a length-prefixed blob back into a list of connections.
fn decode_list(blob: &[u8]) -> Result<Vec<CloudConnection>> {
    let mut cur = 0usize;
    if cur + 4 > blob.len() {
        anyhow::bail!("connections blob truncated (record count)");
    }
    let count = u32::from_le_bytes([blob[cur], blob[cur + 1], blob[cur + 2], blob[cur + 3]]) as usize;
    cur += 4;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let mut c = CloudConnection::default();
        for fname in RECORD_FIELDS.iter() {
            if cur + 4 > blob.len() {
                anyhow::bail!("connections blob truncated (field len)");
            }
            let len = u32::from_le_bytes([
                blob[cur], blob[cur + 1], blob[cur + 2], blob[cur + 3],
            ]) as usize;
            cur += 4;
            if cur + len > blob.len() {
                anyhow::bail!("connections blob field overruns buffer");
            }
            let val = String::from_utf8(blob[cur..cur + len].to_vec())
                .context("connections blob is not valid UTF-8")?;
            set_field(&mut c, fname, val);
            cur += len;
        }
        out.push(c);
    }
    Ok(out)
}

/// Legacy 3-field (url, user, pass) decoder for migrating old `webdav.dat`.
fn decode_legacy_webdav(blob: &[u8]) -> Result<LegacyWebDavCreds> {
    let mut cur = 0usize;
    let mut strs: [String; 3] = Default::default();
    for slot in strs.iter_mut() {
        if cur + 4 > blob.len() {
            anyhow::bail!("legacy webdav blob truncated");
        }
        let len = u32::from_le_bytes([blob[cur], blob[cur + 1], blob[cur + 2], blob[cur + 3]]) as usize;
        cur += 4;
        if cur + len > blob.len() {
            anyhow::bail!("legacy webdav blob field overruns buffer");
        }
        *slot = String::from_utf8(blob[cur..cur + len].to_vec())
            .context("legacy webdav blob is not valid UTF-8")?;
        cur += len;
    }
    Ok(LegacyWebDavCreds {
        url: strs[0].clone(),
        user: strs[1].clone(),
        pass: strs[2].clone(),
    })
}

struct LegacyWebDavCreds {
    url: String,
    user: String,
    pass: String,
}

/// Encrypt a plaintext blob → `salt || ciphertext`, then base64-encode it.
fn encrypt_base64(plaintext: &[u8]) -> Result<String> {
    let salt = random_bytes(SALT_LEN)?;
    let mut buf = Vec::with_capacity(SALT_LEN + plaintext.len());
    buf.extend_from_slice(&salt);
    buf.extend_from_slice(plaintext);
    xor_keystream(&salt, &mut buf[SALT_LEN..]);
    Ok(base64::engine::general_purpose::STANDARD.encode(&buf))
}

/// Decrypt a base64-encoded `salt || ciphertext` back to the plaintext blob.
fn decrypt_base64(stored: &str) -> Result<Vec<u8>> {
    let buf = base64::engine::general_purpose::STANDARD
        .decode(stored.trim())
        .context("connection file is not valid base64")?;
    if buf.len() < SALT_LEN {
        anyhow::bail!("connection payload too small");
    }
    let (salt, payload) = buf.split_at(SALT_LEN);
    let salt = salt.to_vec();
    let mut payload = payload.to_vec();
    xor_keystream(&salt, &mut payload);
    Ok(payload)
}

/// Derive a default connection name from a WebDAV URL's host.
fn name_from_url(url: &str) -> String {
    let after = url.split("://").nth(1).unwrap_or(url);
    let host = after.split('/').next().unwrap_or(after);
    if host.is_empty() {
        "webdav".to_string()
    } else {
        host.to_string()
    }
}

/// Load all saved connections, migrating the legacy single-WebDAV file on
/// first run. Returns an empty vec when nothing is stored yet.
pub fn load_connections() -> Vec<CloudConnection> {
    let path = match connections_path() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    if let Ok(stored) = std::fs::read_to_string(&path) {
        match decrypt_base64(&stored).and_then(|v| decode_list(&v)) {
            Ok(list) => return list,
            Err(e) => log::warn!("Failed to read connections.dat: {}", e),
        }
    }

    // No (or unreadable) connections.dat — try migrating the legacy file.
    let legacy = match legacy_webdav_path().ok().filter(|p| p.exists()) {
        Some(p) => p,
        None => return Vec::new(),
    };
    let stored = match std::fs::read_to_string(&legacy) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let creds = match decrypt_base64(&stored).and_then(|v| decode_legacy_webdav(&v)) {
        Ok(c) if !c.url.is_empty() || !c.user.is_empty() || !c.pass.is_empty() => c,
        Ok(_) => return Vec::new(),
        Err(e) => {
            log::warn!("Failed to migrate legacy webdav.dat: {}", e);
            return Vec::new();
        }
    };
    let conn = CloudConnection {
        protocol_tag: Protocol::WebDav.as_str().to_string(),
        name: name_from_url(&creds.url),
        url: creds.url,
        user: creds.user,
        pass: creds.pass,
        ..Default::default()
    };
    let migrated = vec![conn];
    if let Err(e) = save_connections(&migrated) {
        log::warn!("Failed to persist migrated connections: {}", e);
    } else {
        // Keep the legacy file as a .bak backup rather than deleting it.
        let _ = std::fs::rename(&legacy, format!("{}.bak", legacy.display()));
        log::info!("Migrated legacy webdav.dat → connections.dat");
    }
    migrated
}

/// Persist the connection list, encrypted, with an atomic write.
pub fn save_connections(conns: &[CloudConnection]) -> Result<()> {
    let path = connections_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create connections dir {:?}", parent))?;
    }
    let blob = encode_list(conns);
    let stored = encrypt_base64(&blob)?;
    let tmp = path.with_extension("dat.tmp");
    std::fs::write(&tmp, &stored)
        .with_context(|| format!("cannot write connections tmp file {:?}", tmp))?;
    std::fs::rename(&tmp, &path)
        .with_context(|| format!("cannot rename connections tmp file → {:?}", path))?;
    log::info!("Saved {} connection(s) to {:?}", conns.len(), path);
    Ok(())
}

/// Add a connection. Rejects if `(protocol, name)` already exists.
pub fn add_connection(conn: &CloudConnection) -> Result<()> {
    let mut list = load_connections();
    if find_index(&list, conn.protocol(), &conn.name).is_some() {
        anyhow::bail!("connection '{}' already exists", conn.name);
    }
    list.push(conn.clone());
    save_connections(&list)
}

/// Update the connection identified by `(protocol, old_name)`. If
/// `new_name` differs and already exists, reject.
pub fn update_connection(
    protocol: Protocol,
    old_name: &str,
    new_name: &str,
    conn: &CloudConnection,
) -> Result<()> {
    let mut list = load_connections();
    let idx = find_index(&list, Some(protocol), old_name)
        .ok_or_else(|| anyhow::anyhow!("connection '{}' not found", old_name))?;
    if new_name != old_name {
        if let Some(other) = find_index(&list, Some(protocol), new_name) {
            if other != idx {
                anyhow::bail!("connection '{}' already exists", new_name);
            }
        }
    }
    list[idx] = conn.clone();
    save_connections(&list)
}

/// Remove the connection identified by `(protocol, name)`.
pub fn remove_connection(protocol: Protocol, name: &str) -> Result<()> {
    let mut list = load_connections();
    let idx = find_index(&list, Some(protocol), name)
        .ok_or_else(|| anyhow::anyhow!("connection '{}' not found", name))?;
    list.remove(idx);
    save_connections(&list)
}

/// Find a saved connection by protocol + name.
pub fn find_connection(protocol: Protocol, name: &str) -> Option<CloudConnection> {
    load_connections()
        .into_iter()
        .find(|c| c.protocol() == Some(protocol) && c.name == name)
}

/// Look up the WebDAV connection whose `url` is the longest prefix of `uri`.
pub fn find_webdav_for_uri(uri: &str) -> Option<CloudConnection> {
    load_connections()
        .into_iter()
        .filter(|c| c.protocol() == Some(Protocol::WebDav))
        .filter(|c| !c.url.is_empty() && uri.starts_with(&c.url))
        .max_by_key(|c| c.url.len())
}

/// Look up an SSH connection by host + user (+ port).
pub fn find_ssh_for(host: &str, user: &str, port: Option<u16>) -> Option<CloudConnection> {
    load_connections().into_iter().find(|c| {
        c.protocol() == Some(Protocol::Ssh)
            && c.host == host
            && (user.is_empty() || c.user == user)
            && port.map_or(true, |p| c.port.parse::<u16>().ok() == Some(p))
    })
}

/// Look up an S3/KS3 connection by bucket (+ endpoint).
pub fn find_s3_for(bucket: &str, endpoint: Option<&str>, ks3: bool) -> Option<CloudConnection> {
    let proto = if ks3 { Protocol::Ks3 } else { Protocol::S3 };
    load_connections().into_iter().find(|c| {
        c.protocol() == Some(proto)
            && c.bucket == bucket
            && endpoint.map_or(true, |e| c.endpoint == e || c.endpoint.is_empty())
    })
}

fn find_index(list: &[CloudConnection], protocol: Option<Protocol>, name: &str) -> Option<usize> {
    list.iter().position(|c| c.protocol() == protocol && c.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(protocol: Protocol, name: &str) -> CloudConnection {
        CloudConnection::from_protocol(
            protocol,
            name,
            &ConnectionFields {
                url: "https://h/d/".into(),
                user: "u".into(),
                pass: "p".into(),
                host: "h".into(),
                port: "22".into(),
                path: "/x".into(),
                bucket: "b".into(),
                region: "us-east-1".into(),
                endpoint: "e".into(),
                access_key: "ak".into(),
                secret_key: "sk".into(),
            },
        )
    }

    #[test]
    fn roundtrip_multi_record() {
        let list = vec![
            sample(Protocol::WebDav, "123pan"),
            sample(Protocol::Ssh, "gpu-box"),
            sample(Protocol::Ks3, "中文连接"),
        ];
        let blob = encode_list(&list);
        let stored = encrypt_base64(&blob).unwrap();
        let back = decode_list(&decrypt_base64(&stored).unwrap()).unwrap();
        assert_eq!(back.len(), 3);
        assert_eq!(back[0].name, "123pan");
        assert_eq!(back[0].protocol(), Some(Protocol::WebDav));
        assert_eq!(back[2].name, "中文连接");
        assert_eq!(back[2].secret_key, "sk");
    }

    #[test]
    fn roundtrip_empty_fields() {
        let mut c = sample(Protocol::S3, "empty");
        c.access_key.clear();
        c.secret_key.clear();
        c.region.clear();
        let list = vec![c];
        let back = decode_list(&decrypt_base64(&encrypt_base64(&encode_list(&list)).unwrap()).unwrap()).unwrap();
        assert_eq!(back[0].access_key, "");
        assert_eq!(back[0].region, "");
        assert_eq!(back[0].bucket, "b");
    }

    #[test]
    fn ciphertext_no_plaintext() {
        let list = vec![sample(Protocol::WebDav, "secret-name")];
        let stored = encrypt_base64(&encode_list(&list)).unwrap();
        assert!(!stored.contains("secret-name"));
        assert!(!stored.contains("123pan"));
    }

    #[test]
    fn migrates_legacy_webdav() {
        // Encode an old 3-field (url,user,pass) blob with the legacy layout.
        let mut blob = Vec::new();
        for field in [
            "https://webdav.123pan.cn/webdav/profiler/",
            "18581298733",
            "gg7qfuim",
        ] {
            blob.extend_from_slice(&(field.len() as u32).to_le_bytes());
            blob.extend_from_slice(field.as_bytes());
        }
        let stored = encrypt_base64(&blob).unwrap();
        let creds = decode_legacy_webdav(&decrypt_base64(&stored).unwrap()).unwrap();
        assert_eq!(creds.user, "18581298733");
        assert_eq!(creds.pass, "gg7qfuim");
        assert_eq!(name_from_url(&creds.url), "webdav.123pan.cn");
    }

    #[test]
    fn protocol_roundtrip_string() {
        assert_eq!(Protocol::from_id("ks3"), Some(Protocol::Ks3));
        assert_eq!(Protocol::from_id("nope"), None);
        assert_eq!(Protocol::Ssh.as_str(), "ssh");
    }

    #[test]
    fn name_from_url_extracts_host() {
        assert_eq!(name_from_url("https://webdav.123pan.cn/webdav/p/"), "webdav.123pan.cn");
        assert_eq!(name_from_url("webdav.123pan.cn/x"), "webdav.123pan.cn");
        assert_eq!(name_from_url("not-a-url"), "not-a-url");
        assert_eq!(name_from_url("://"), "webdav");
    }
}
