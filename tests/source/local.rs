use std::path::PathBuf;

use nvis::source::browser::EntryKind;
use nvis::source::local::LocalSource;
use nvis::source::Source;

#[test]
fn strips_file_scheme() {
    let f = LocalSource.fetch("file:///data/x.sqlite").unwrap();
    assert_eq!(f.local_path, PathBuf::from("/data/x.sqlite"));
    assert!(!f.should_cleanup);
}

#[test]
fn handles_plain_path() {
    let f = LocalSource.fetch("/tmp/report.csv").unwrap();
    assert_eq!(f.local_path, PathBuf::from("/tmp/report.csv"));
    assert_eq!(f.source_id, "local");
}

#[test]
fn can_handle_anything() {
    assert!(LocalSource.can_handle("/local/path"));
    assert!(LocalSource.can_handle("anything"));
}

#[test]
fn list_temp_dir() {
    let dir = std::env::temp_dir().join("nvis_test_list");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    // Create some test entries
    std::fs::File::create(dir.join("b.csv")).unwrap();
    std::fs::File::create(dir.join("a.sqlite")).unwrap();
    std::fs::create_dir(dir.join("subdir")).unwrap();
    std::fs::File::create(dir.join(".hidden")).unwrap(); // should be skipped

    let entries = LocalSource.list(dir.to_str().unwrap()).unwrap();
    assert_eq!(entries.len(), 3);

    // Directories first
    assert_eq!(entries[0].name, "subdir");
    assert_eq!(entries[0].kind, EntryKind::Directory);

    // Then files alphabetically
    assert_eq!(entries[1].name, "a.sqlite");
    assert_eq!(entries[1].kind, EntryKind::File);
    assert_eq!(entries[2].name, "b.csv");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn list_rejects_file() {
    let result = LocalSource.list("/dev/null");
    assert!(result.is_err());
}
