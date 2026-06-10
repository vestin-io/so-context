use super::*;

fn make_cache() -> FileVisitCache {
    FileVisitCache::new()
}

#[test]
fn add_and_query_file() {
    let cache = make_cache();
    cache.add_file("conn-1", "cw-1", "/src/main.rs", 120, "hash1".to_string());
    assert!(cache.is_visited("conn-1", "cw-1", "/src/main.rs"));
    assert!(!cache.is_visited("conn-1", "cw-1", "/src/lib.rs"));
}

#[test]
fn add_updates_token_count_and_last_visit() {
    let cache = make_cache();
    cache.add_file("conn-1", "cw-1", "/src/main.rs", 100, "hash-a".to_string());
    let first = cache.file("conn-1", "cw-1", "/src/main.rs").unwrap();

    cache.add_file("conn-1", "cw-1", "/src/main.rs", 200, "hash-b".to_string());
    let second = cache.file("conn-1", "cw-1", "/src/main.rs").unwrap();

    assert_eq!(second.token_count, 200);
    assert_eq!(second.content_hash, "hash-b");
    assert!(second.last_visit >= first.last_visit);
}

#[test]
fn expire_file() {
    let cache = make_cache();
    cache.add_file("conn-1", "cw-1", "/src/main.rs", 120, "h".to_string());
    assert!(cache.expire_file("conn-1", "cw-1", "/src/main.rs"));
    assert!(!cache.is_visited("conn-1", "cw-1", "/src/main.rs"));
    assert!(!cache.expire_file("conn-1", "cw-1", "/src/main.rs"));
}

#[test]
fn delete_context_window() {
    let cache = make_cache();
    cache.add_file("conn-1", "cw-1", "/src/a.rs", 10, "h1".to_string());
    cache.add_file("conn-1", "cw-1", "/src/b.rs", 20, "h2".to_string());
    cache.add_file("conn-1", "cw-2", "/src/c.rs", 30, "h3".to_string());

    assert!(cache.delete_context_window("conn-1", "cw-1"));
    assert!(!cache.is_visited("conn-1", "cw-1", "/src/a.rs"));
    assert!(!cache.is_visited("conn-1", "cw-1", "/src/b.rs"));
    assert!(cache.is_visited("conn-1", "cw-2", "/src/c.rs"));
}

#[test]
fn delete_context_window_globally() {
    let cache = make_cache();
    cache.add_file("conn-1", "cw-1", "/src/a.rs", 10, "h1".to_string());
    cache.add_file("conn-2", "cw-1", "/src/b.rs", 20, "h2".to_string());
    cache.add_file("conn-2", "cw-2", "/src/c.rs", 30, "h3".to_string());

    assert!(cache.delete_context_window_globally("cw-1"));
    assert!(!cache.is_visited("conn-1", "cw-1", "/src/a.rs"));
    assert!(!cache.is_visited("conn-2", "cw-1", "/src/b.rs"));
    assert!(cache.is_visited("conn-2", "cw-2", "/src/c.rs"));
    assert!(!cache.delete_context_window_globally("missing"));
}

#[test]
fn delete_connection() {
    let cache = make_cache();
    cache.add_file("conn-1", "cw-1", "/src/a.rs", 10, "h1".to_string());
    cache.add_file("conn-1", "cw-2", "/src/b.rs", 20, "h2".to_string());
    cache.add_file("conn-2", "cw-1", "/src/c.rs", 30, "h3".to_string());

    assert!(cache.delete_connection("conn-1"));
    assert!(!cache.is_visited("conn-1", "cw-1", "/src/a.rs"));
    assert!(!cache.is_visited("conn-1", "cw-2", "/src/b.rs"));
    assert!(cache.is_visited("conn-2", "cw-1", "/src/c.rs"));
}

#[test]
fn list_files() {
    let cache = make_cache();
    cache.add_file("conn-1", "cw-1", "/a.rs", 1, "h1".to_string());
    cache.add_file("conn-1", "cw-1", "/b.rs", 2, "h2".to_string());

    let mut files: Vec<String> = cache
        .list_files("conn-1", "cw-1")
        .into_iter()
        .map(|e| e.file_path)
        .collect();
    files.sort();
    assert_eq!(files, vec!["/a.rs", "/b.rs"]);
}

#[test]
fn list_context_windows() {
    let cache = make_cache();
    cache.add_file("conn-1", "cw-a", "/a.rs", 1, "h1".to_string());
    cache.add_file("conn-1", "cw-b", "/b.rs", 2, "h2".to_string());

    let mut windows = cache.list_context_windows("conn-1");
    windows.sort();
    assert_eq!(windows, vec!["cw-a", "cw-b"]);
}

#[test]
fn len_and_is_empty() {
    let cache = make_cache();
    assert!(cache.is_empty());

    cache.add_file("conn-1", "cw-1", "/a.rs", 1, "h1".to_string());
    cache.add_file("conn-1", "cw-2", "/b.rs", 2, "h2".to_string());
    assert_eq!(cache.len(), 2);

    cache.delete_connection("conn-1");
    assert!(cache.is_empty());
}
