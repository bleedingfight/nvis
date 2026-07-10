use anyhow::{anyhow, Context, Result};
use std::path::PathBuf;
use std::process::Command;

use super::browser::{DirEntry, EntryKind};
use super::{log_fetch_complete, FetchedFile, Source};

/// A parsed SSH remote file location.
#[derive(Debug, Clone)]
pub struct SshLocation {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    pub path: String,
}

impl SshLocation {
    /// Format as an scp-style remote argument: `[user@]host:/path`.
    pub fn remote_spec(&self) -> String {
        let mut s = String::new();
        if let Some(ref u) = self.user {
            s.push_str(u);
            s.push('@');
        }
        s.push_str(&self.host);
        s.push(':');
        if !self.path.starts_with('/') {
            s.push('/');
        }
        s.push_str(&self.path);
        s
    }

    /// `[user@]host` (no path), used as the ssh target argument.
    fn target(&self) -> String {
        match &self.user {
            Some(u) => format!("{}@{}", u, self.host),
            None => self.host.clone(),
        }
    }

    /// Format as a `ssh://[user@]host[:port]/path` URI (for child entries).
    pub fn uri(&self) -> String {
        let mut s = String::from("ssh://");
        if let Some(ref u) = self.user {
            s.push_str(u);
            s.push('@');
        }
        s.push_str(&self.host);
        if let Some(p) = self.port {
            s.push_str(&format!(":{}", p));
        }
        if !self.path.starts_with('/') {
            s.push('/');
        }
        s.push_str(&self.path);
        s
    }

    /// Short human-readable label for UI messages (e.g. `user@host`).
    pub fn label(&self) -> String {
        match &self.user {
            Some(u) => format!("{}@{}", u, self.host),
            None => self.host.clone(),
        }
    }
}

/// Source for SSH/SCP URIs.
///
/// Supported formats:
/// - `ssh://[user@]host[:port]/path`   (standard URL form)
/// - `ssh:[user@]host:/path`            (colon form, port 22)
/// - `ssh:[user@]host:port:/path`       (colon form with port)
/// - `scp:` accepted as an alias of `ssh:`
pub struct SshSource;

impl Source for SshSource {
    fn id(&self) -> &str {
        "ssh"
    }

    fn name(&self) -> &str {
        "SSH (scp)"
    }

    fn can_handle(&self, uri: &str) -> bool {
        parse_ssh_uri(uri).is_some()
    }

    fn supports_listing(&self) -> bool {
        true
    }

    fn list(&self, uri: &str) -> Result<Vec<DirEntry>> {
        let loc = parse_ssh_uri(uri)
            .ok_or_else(|| anyhow!("Invalid SSH URI: {}", uri))?;
        list_ssh_dir(&loc)
    }

    fn fetch(&self, uri: &str) -> Result<FetchedFile> {
        let loc = parse_ssh_uri(uri)
            .ok_or_else(|| anyhow!("Invalid SSH URI: {}", uri))?;
        let dest = super::temp_path_for(&loc.path);
        self.fetch_to(uri, &dest)
    }

    fn fetch_to(&self, uri: &str, dest: &std::path::Path) -> Result<FetchedFile> {
        let loc = parse_ssh_uri(uri)
            .ok_or_else(|| anyhow!("Invalid SSH URI: {}", uri))?;
        let local_path = fetch_to_dest(&loc, dest)?;
        Ok(FetchedFile::remote(local_path, uri.to_string(), self.id()))
    }

    fn head_size(&self, uri: &str) -> Option<u64> {
        let loc = parse_ssh_uri(uri)?;
        ssh_size(&loc)
    }
}

/// Parse an SSH URI. Returns `None` when the input is not an SSH URI.
pub fn parse_ssh_uri(input: &str) -> Option<SshLocation> {
    let input = input.trim();

    if let Some(rest) = input
        .strip_prefix("ssh://")
        .or_else(|| input.strip_prefix("scp://"))
    {
        return parse_url_form(rest);
    }

    if let Some(rest) = input
        .strip_prefix("ssh:")
        .or_else(|| input.strip_prefix("scp:"))
    {
        return parse_colon_form(rest);
    }

    None
}

fn parse_url_form(rest: &str) -> Option<SshLocation> {
    // rest = [user@]host[:port]/path
    let slash_idx = rest.find('/')?;
    if slash_idx == 0 {
        return None; // missing authority
    }
    let authority = &rest[..slash_idx];
    let path = &rest[slash_idx + 1..];

    let (user, host_port) = match authority.rfind('@') {
        Some(i) => (Some(authority[..i].to_string()), &authority[i + 1..]),
        None => (None, authority),
    };

    let (host, port) = split_host_port(host_port)?;

    if host.is_empty() || path.is_empty() {
        return None;
    }

    Some(SshLocation {
        user,
        host,
        port,
        path: format!("/{}", path),
    })
}

fn parse_colon_form(rest: &str) -> Option<SshLocation> {
    // rest = [user@]host:[port:]path
    let (user, host_rest) = match rest.rfind('@') {
        Some(i) => (Some(rest[..i].to_string()), &rest[i + 1..]),
        None => (None, rest),
    };

    let first_colon = host_rest.find(':')?;
    let host = &host_rest[..first_colon];
    if host.is_empty() {
        return None;
    }
    let after_host = &host_rest[first_colon + 1..];

    // Try to interpret `port:/path` form: digits followed by `:`.
    let (port, path_raw) = if let Some(second_colon) = after_host.find(':') {
        let maybe_port = &after_host[..second_colon];
        if let Ok(p) = maybe_port.parse::<u16>() {
            (Some(p), after_host[second_colon + 1..].to_string())
        } else {
            (None, after_host.to_string())
        }
    } else {
        (None, after_host.to_string())
    };

    if path_raw.is_empty() {
        return None;
    }

    let path = if path_raw.starts_with('/') {
        path_raw
    } else {
        format!("/{}", path_raw)
    };

    Some(SshLocation {
        user,
        host: host.to_string(),
        port,
        path,
    })
}

fn split_host_port(host_port: &str) -> Option<(String, Option<u16>)> {
    match host_port.rfind(':') {
        Some(i) => {
            let port: u16 = host_port[i + 1..].parse().ok()?;
            Some((host_port[..i].to_string(), Some(port)))
        }
        None => Some((host_port.to_string(), None)),
    }
}

/// Look up a saved SSH connection matching this location and return its
/// password (if any). Used to decide key vs `sshpass` auth.
fn saved_password(loc: &SshLocation) -> Option<String> {
    crate::credentials::find_ssh_for(&loc.host, loc.user.as_deref().unwrap_or(""), loc.port)
        .filter(|c| !c.pass.is_empty())
        .map(|c| c.pass)
}

/// Best-effort remote file size via `ssh stat -c%s`. Returns None on any error.
fn ssh_size(loc: &SshLocation) -> Option<u64> {
    let password = saved_password(loc);
    let mut cmd = if password.is_some() {
        match sshpass_cmd("ssh") {
            Ok(c) => c,
            Err(_) => return None,
        }
    } else {
        Command::new("ssh")
    };
    if let Some(ref pass) = password {
        cmd.env("SSHPASS", pass);
    }
    if let Some(port) = loc.port {
        cmd.arg("-p").arg(port.to_string());
    }
    cmd.arg("-o").arg("ConnectTimeout=10");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    cmd.arg(loc.target());
    cmd.arg("stat").arg("-c%s").arg(&loc.path);
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout).trim().parse::<u64>().ok()
}

/// Fetch a remote file via `scp` into `dest`.
///
/// Auth: a saved SSH connection with a password uses `sshpass -e scp`
/// (the password is passed via the `SSHPASS` env var, never argv). Without
/// a saved password, `scp -B` (batch mode) is used for key/agent auth.
pub fn fetch_to_dest(loc: &SshLocation, dest: &std::path::Path) -> Result<PathBuf> {
    let local_path = dest.to_path_buf();
    let password = saved_password(loc);

    let mut cmd = if password.is_some() {
        sshpass_cmd("scp")?
    } else {
        Command::new("scp")
    };
    if password.is_none() {
        cmd.arg("-B"); // batch mode — no interactive prompts (key/agent auth)
    }
    cmd.arg("-q"); // quiet — no progress bar
    if let Some(ref pass) = password {
        cmd.env("SSHPASS", pass);
    }
    cmd.arg("-o").arg("ConnectTimeout=10");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    if let Some(port) = loc.port {
        cmd.arg("-P").arg(port.to_string());
    }
    cmd.arg(loc.remote_spec()).arg(&local_path);

    log::info!("Running scp: {:?}", cmd);

    let output = cmd
        .output()
        .with_context(|| "failed to execute `scp` — is it installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let _ = std::fs::remove_file(&local_path);
        return Err(anyhow!(
            "scp exited with status {}: {}{}",
            output.status,
            stderr.trim(),
            if stdout.trim().is_empty() {
                String::new()
            } else {
                format!("\nstdout: {}", stdout.trim())
            }
        ));
    }

    if !local_path.exists() {
        return Err(anyhow!(
            "scp reported success but the destination file is missing"
        ));
    }

    log_fetch_complete(&local_path);
    Ok(local_path)
}

/// Build an `sshpass -e <program>` command, with a clear error if sshpass
/// is not installed.
fn sshpass_cmd(program: &str) -> Result<Command> {
    // Probe presence with `sshpass -V` (prints version + exits 0).
    match Command::new("sshpass").arg("-V").output() {
        Ok(o) if o.status.success() => {}
        _ => {
            return Err(anyhow!(
                "sshpass not found — install it for SSH password auth \
                 (`brew install hudochenkov/sshpass/sshpass` on macOS). \
                 Key/agent SSH auth needs no sshpass."
            ))
        }
    }
    let mut c = Command::new("sshpass");
    c.arg("-e").arg(program);
    Ok(c)
}

/// List a remote directory via `ssh ls -1A -p`. Returns children as DirEntries
/// whose URIs round-trip through `parse_ssh_uri`.
pub fn list_ssh_dir(loc: &SshLocation) -> Result<Vec<DirEntry>> {
    let password = saved_password(loc);

    let mut cmd = if password.is_some() {
        sshpass_cmd("ssh")?
    } else {
        Command::new("ssh")
    };
    if let Some(ref pass) = password {
        cmd.env("SSHPASS", pass);
    }
    if let Some(port) = loc.port {
        cmd.arg("-p").arg(port.to_string());
    }
    cmd.arg("-o").arg("ConnectTimeout=10");
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
    cmd.arg(loc.target());
    // `ls -1A -p`: one per line, show hidden except . and .., append '/' to dirs.
    cmd.arg("ls").arg("-1Ap");
    cmd.arg(&loc.path);

    log::info!("Running ssh ls: {:?}", cmd);
    let out = cmd
        .output()
        .with_context(|| "failed to execute `ssh` — is it installed?")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(anyhow!(
            "ssh ls exited with status {}: {}",
            out.status,
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(parse_ls_output(&stdout, loc))
}

/// Parse `ls -1p` output (one entry per line, trailing '/' marks a dir) into
/// DirEntries with absolute child URIs.
fn parse_ls_output(stdout: &str, loc: &SshLocation) -> Vec<DirEntry> {
    let mut entries = Vec::new();
    let base = loc.path.trim_end_matches('/');
    for line in stdout.lines() {
        let name = line.trim_end();
        if name.is_empty() || name == "." || name == ".." {
            continue;
        }
        let is_dir = name.ends_with('/');
        let name = name.trim_end_matches('/').to_string();
        if name.is_empty() {
            continue;
        }
        let child_path = if base.is_empty() {
            format!("/{}", name)
        } else {
            format!("{}/{}", base, name)
        };
        let child_uri = format!("ssh://{}{}{}", loc.uri_authority(), child_path, if is_dir { "/" } else { "" });
        entries.push(DirEntry {
            name,
            kind: if is_dir { EntryKind::Directory } else { EntryKind::File },
            uri: child_uri,
            size: 0,
        });
    }
    entries.sort_by(|a, b| match (a.kind, b.kind) {
        (EntryKind::Directory, EntryKind::File) => std::cmp::Ordering::Less,
        (EntryKind::File, EntryKind::Directory) => std::cmp::Ordering::Greater,
        _ => a.name.cmp(&b.name),
    });
    entries
}

impl SshLocation {
    /// `user@host[:port]` authority (no scheme, no leading slashes).
    fn uri_authority(&self) -> String {
        let mut s = String::new();
        if let Some(ref u) = self.user {
            s.push_str(u);
            s.push('@');
        }
        s.push_str(&self.host);
        if let Some(p) = self.port {
            s.push_str(&format!(":{}", p));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ls_distinguishes_dirs() {
        let loc = parse_ssh_uri("ssh://user@host:2222/data/reports").unwrap();
        let out = "nsys.sqlite\ntraces/\n.hidden\n";
        let entries = parse_ls_output(out, &loc);
        assert_eq!(entries.len(), 3);
        // sorted: dirs first
        assert_eq!(entries[0].name, "traces");
        assert_eq!(entries[0].kind, EntryKind::Directory);
        assert_eq!(entries[0].uri, "ssh://user@host:2222/data/reports/traces/");
        assert_eq!(entries[1].name, ".hidden");
        assert_eq!(entries[1].kind, EntryKind::File);
        assert_eq!(entries[2].name, "nsys.sqlite");
        assert_eq!(entries[2].kind, EntryKind::File);
        assert_eq!(entries[2].uri, "ssh://user@host:2222/data/reports/nsys.sqlite");
    }

    #[test]
    fn parse_ls_skips_dotentries() {
        let loc = parse_ssh_uri("ssh://h/x").unwrap();
        let entries = parse_ls_output(".\n..\nfile\n", &loc);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "file");
    }

    #[test]
    fn child_uris_round_trip() {
        let loc = parse_ssh_uri("ssh://host:2222/data/r").unwrap();
        let entries = parse_ls_output("a\nb/\n", &loc);
        for e in &entries {
            assert!(parse_ssh_uri(&e.uri).is_some(), "{} should re-parse", e.uri);
        }
    }
}
