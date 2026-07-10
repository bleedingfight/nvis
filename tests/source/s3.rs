use nvis::source::browser::EntryKind;
use nvis::source::s3::{
    ensure_https, parse_ks3_config_from_str, parse_ks3_uri, parse_s3_uri, S3Source,
};
use nvis::source::Source;

// ── can_handle ──────────────────────────────────────────────

#[test]
fn can_handle_s3_and_ks3_uris() {
    let s = S3Source;
    assert!(s.can_handle("s3://bucket/path/x.sqlite"));
    assert!(s.can_handle("ks3://bucket/path/x.sqlite"));
    assert!(!s.can_handle("ssh:host:/x"));
    assert!(!s.can_handle("/local/path"));
    assert!(!s.can_handle("http://host/x"));
}

// ── parse_s3_uri ────────────────────────────────────────────

#[test]
fn parses_plain_s3_uri() {
    let p = parse_s3_uri("s3://my-bucket/path/file.sqlite");
    assert_eq!(p.bucket, "my-bucket");
    assert_eq!(p.key, "path/file.sqlite");
    assert!(p.region.is_none());
    assert!(p.endpoint.is_none());
}

#[test]
fn parses_s3_uri_with_region() {
    let p = parse_s3_uri("s3://my-bucket/path/file.csv?region=ap-northeast-1");
    assert_eq!(p.bucket, "my-bucket");
    assert_eq!(p.key, "path/file.csv");
    assert_eq!(p.region.as_deref(), Some("ap-northeast-1"));
    assert!(p.endpoint.is_none());
}

#[test]
fn parses_s3_uri_with_endpoint() {
    let p = parse_s3_uri(
        "s3://my-bucket/f.json?endpoint-url=http://minio:9000&region=us-east-1",
    );
    assert_eq!(p.bucket, "my-bucket");
    assert_eq!(p.key, "f.json");
    assert_eq!(p.region.as_deref(), Some("us-east-1"));
    assert_eq!(p.endpoint.as_deref(), Some("http://minio:9000"));
}

// ── parse_ks3_uri ───────────────────────────────────────────

#[test]
fn parses_plain_ks3_uri() {
    let p = parse_ks3_uri("ks3://my-bucket/path/file.sqlite");
    assert_eq!(p.bucket, "my-bucket");
    assert_eq!(p.key, "path/file.sqlite");
    assert!(p.endpoint.is_none());
}

#[test]
fn parses_ks3_uri_with_endpoint() {
    let p = parse_ks3_uri("ks3://my-bucket/f.json?endpoint=kss-cn-beijing.ks3cloud.com");
    assert_eq!(p.bucket, "my-bucket");
    assert_eq!(p.key, "f.json");
    assert_eq!(
        p.endpoint.as_deref(),
        Some("kss-cn-beijing.ks3cloud.com")
    );
}

// ── parse_ks3_config_from_str ───────────────────────────────

#[test]
fn parses_ks3_config_string() {
    let content = "\
[Credentials]
language=CH
endpoint=ks3-cn-beijing.ksyuncs.com
accessKeyID=TEST_AK
accessKeySecret=TEST_SK
";
    let cfg = parse_ks3_config_from_str(content).unwrap();
    assert_eq!(cfg.endpoint, "ks3-cn-beijing.ksyuncs.com");
    assert_eq!(cfg.access_key_id, "TEST_AK");
    assert_eq!(cfg.access_key_secret, "TEST_SK");
}

#[test]
fn rejects_missing_credentials() {
    let content = "\
[Other]
foo=bar
";
    let result = parse_ks3_config_from_str(content);
    assert!(result.is_err());
}

// ── ensure_https ───────────────────────────────────────────

#[test]
fn ensure_https_adds_prefix() {
    assert_eq!(ensure_https("ks3-cn-beijing.ksyuncs.com"), "https://ks3-cn-beijing.ksyuncs.com");
}

#[test]
fn ensure_https_preserves_http() {
    assert_eq!(ensure_https("http://minio:9000"), "http://minio:9000");
}

#[test]
fn ensure_https_preserves_existing_https() {
    assert_eq!(ensure_https("https://ks3.example.com"), "https://ks3.example.com");
}

// ── integration: fetch a real ks3 file ─────────────────────
//
// This test requires ~/.ks3utilconfig with valid KS3 credentials.
// It's marked #[ignore] — run with:
//   cargo test -- --ignored
//
#[test]
#[ignore]
fn fetch_ks3_small_file() {
    let s = S3Source;
    let uri = "ks3://llm-perf-liushuai/analysis/3model-3x-comparison-report.md";
    let result = s.fetch(uri);
    match &result {
        Ok(fetched) => {
            assert!(fetched.local_path.exists());
            let len = fetched.local_path.metadata().unwrap().len();
            assert!(len > 0);
            assert_eq!(fetched.original_uri, uri);
            assert_eq!(fetched.source_id, "s3");
            println!("✅ ks3 fetch OK: {} bytes → {:?}", len, fetched.local_path);
        }
        Err(e) => {
            panic!("❌ ks3 fetch failed: {e:#}");
        }
    }
}

#[test]
#[ignore]
fn list_ks3_bucket() {
    let s = S3Source;
    let entries = s.list("ks3://llm-perf-liushuai/").unwrap();
    assert!(!entries.is_empty(), "bucket listing should return entries");
    // Should contain at least one directory entry
    let dirs: Vec<_> = entries.iter().filter(|e| e.kind == EntryKind::Directory).collect();
    assert!(!dirs.is_empty(), "should have directory entries");
    for e in &entries {
        println!("  {} {} {}", if e.kind == EntryKind::Directory { "D" } else { "F" }, e.name, e.uri);
    }
}

#[test]
#[ignore]
fn fetch_ks3_nsys_profile() {
    let s = S3Source;
    let uri = "ks3://llm-perf-liushuai/profile/GLM-nvfp4/nsys-profiler/nsys_profile.sqlite";
    let result = s.fetch(uri);
    match &result {
        Ok(fetched) => {
            assert!(fetched.local_path.exists());
            assert!(fetched.local_path.metadata().unwrap().len() > 0);
            assert_eq!(fetched.original_uri, uri);
            assert_eq!(fetched.source_id, "s3");
            println!(
                "✅ ks3 fetch OK: {} bytes → {:?}",
                fetched.local_path.metadata().unwrap().len(),
                fetched.local_path
            );
        }
        Err(e) => {
            panic!("❌ ks3 fetch failed: {e:#}");
        }
    }
}
