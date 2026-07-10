use std::path::PathBuf;

use nvis::source::browser::{format_size, BrowserNode};

#[test]
fn format_size_units() {
    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(512), "512 B");
    assert_eq!(format_size(1024), "1.0 KB");
    assert_eq!(format_size(1536), "1.5 KB");
    assert_eq!(format_size(1048576), "1.0 MB");
    assert_eq!(format_size(1073741824), "1.0 GB");
}

#[test]
fn node_ids_are_unique() {
    let n1 = BrowserNode::Category {
        label: "Local".into(),
        id: "local".into(),
        depth: 0,
    };
    let n2 = BrowserNode::Category {
        label: "Cloud".into(),
        id: "cloud".into(),
        depth: 0,
    };
    assert_ne!(n1.id(), n2.id());
}

#[test]
fn prefix_label_strips_trailing_slash() {
    let n = BrowserNode::Prefix {
        scheme: "ks3".into(),
        bucket: "b".into(),
        prefix: "a/b/c/".into(),
        endpoint: None,
        region: None,
        depth: 3,
    };
    assert_eq!(n.display_label(), "c");
}

#[test]
fn local_dir_label_shows_basename() {
    let n = BrowserNode::LocalDir {
        path: PathBuf::from("/home/user/Documents"),
        depth: 1,
    };
    assert_eq!(n.display_label(), "Documents");
}
