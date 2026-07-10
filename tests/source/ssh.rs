use nvis::source::ssh::{parse_ssh_uri, SshSource};
use nvis::source::Source;

#[test]
fn parses_colon_form_with_user() {
    let loc = parse_ssh_uri("ssh:user@host:/data/report.sqlite").unwrap();
    assert_eq!(loc.user.as_deref(), Some("user"));
    assert_eq!(loc.host, "host");
    assert_eq!(loc.port, None);
    assert_eq!(loc.path, "/data/report.sqlite");
    assert_eq!(loc.remote_spec(), "user@host:/data/report.sqlite");
}

#[test]
fn parses_colon_form_without_user() {
    let loc = parse_ssh_uri("ssh:host:/tmp/trace.json").unwrap();
    assert_eq!(loc.user, None);
    assert_eq!(loc.host, "host");
    assert_eq!(loc.path, "/tmp/trace.json");
    assert_eq!(loc.remote_spec(), "host:/tmp/trace.json");
}

#[test]
fn parses_colon_form_with_port() {
    let loc = parse_ssh_uri("ssh:user@host:2222:/data/report.csv").unwrap();
    assert_eq!(loc.user.as_deref(), Some("user"));
    assert_eq!(loc.host, "host");
    assert_eq!(loc.port, Some(2222));
    assert_eq!(loc.path, "/data/report.csv");
}

#[test]
fn parses_url_form_with_port() {
    let loc = parse_ssh_uri("ssh://user@host:2222/data/report.sqlite").unwrap();
    assert_eq!(loc.user.as_deref(), Some("user"));
    assert_eq!(loc.host, "host");
    assert_eq!(loc.port, Some(2222));
    assert_eq!(loc.path, "/data/report.sqlite");
}

#[test]
fn parses_url_form_without_user() {
    let loc = parse_ssh_uri("ssh://host/data/r.json").unwrap();
    assert_eq!(loc.user, None);
    assert_eq!(loc.host, "host");
    assert_eq!(loc.port, None);
    assert_eq!(loc.path, "/data/r.json");
}

#[test]
fn accepts_scp_prefix_alias() {
    let loc = parse_ssh_uri("scp:user@host:/data/x.csv").unwrap();
    assert_eq!(loc.host, "host");
    assert_eq!(loc.path, "/data/x.csv");
}

#[test]
fn rejects_local_paths() {
    assert!(parse_ssh_uri("/tmp/local.sqlite").is_none());
    assert!(parse_ssh_uri("./report.csv").is_none());
    assert!(parse_ssh_uri("report.json").is_none());
}

#[test]
fn remote_spec_ensures_absolute_path() {
    let loc = parse_ssh_uri("ssh:host:relative/path.csv").unwrap();
    assert_eq!(loc.path, "/relative/path.csv");
    assert_eq!(loc.remote_spec(), "host:/relative/path.csv");
}

#[test]
fn label_includes_user_when_present() {
    assert_eq!(parse_ssh_uri("ssh:a@b:/x").unwrap().label(), "a@b");
    assert_eq!(parse_ssh_uri("ssh:b:/x").unwrap().label(), "b");
}

#[test]
fn can_handle_ssh_uris_only() {
    let s = SshSource;
    assert!(s.can_handle("ssh:host:/x"));
    assert!(s.can_handle("ssh://host/x"));
    assert!(s.can_handle("scp:host:/x"));
    assert!(!s.can_handle("/local/path"));
    assert!(!s.can_handle("http://host/x"));
}
