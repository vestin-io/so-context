use super::*;

#[test]
fn passthrough_shows_every_entry_for_small_listings() {
    let entries: Vec<String> = (0..10)
        .map(|index| format!("entry{index:02}.txt"))
        .collect();
    let summary = summarize_ls_entries(&entries, &[]);

    assert_eq!(summary.summary, "entries=10");
    assert_eq!(summary.details.len(), 10);
    assert_eq!(summary.details[0], "entry00.txt");
    assert_eq!(summary.details[9], "entry09.txt");
    assert!(!summary.details.iter().any(|line| line.starts_with('+')));
}

#[test]
fn orders_directories_before_files() {
    let entries = vec![
        "README.md".to_string(),
        "src".to_string(),
        "Cargo.toml".to_string(),
    ];
    let summary = summarize_ls_entries(&entries, &[]);

    assert_eq!(summary.details[0], "src/");
    assert_eq!(summary.details[1], "Cargo.toml");
    assert_eq!(summary.details[2], "README.md");
}

#[test]
fn lists_omitted_entry_names_instead_of_only_a_counter() {
    let entries: Vec<String> = (0..25)
        .map(|index| format!("entry{index:02}.txt"))
        .collect();
    let summary = summarize_ls_entries(&entries, &[]);

    assert_eq!(summary.details.len(), 25);
    assert_eq!(summary.details[19], "entry19.txt");
    assert_eq!(summary.details[20], "entry20.txt");
    assert_eq!(summary.details[24], "entry24.txt");
    assert!(!summary.details.iter().any(|line| line.starts_with("+ ")));
}

#[test]
fn caps_very_large_omitted_name_lists() {
    let entries: Vec<String> = (0..100)
        .map(|index| format!("entry{index:03}.txt"))
        .collect();
    let summary = summarize_ls_entries(&entries, &[]);

    assert_eq!(summary.details.len(), 36);
    assert_eq!(summary.details[19], "entry019.txt");
    assert_eq!(summary.details[20], "entry020.txt");
    assert_eq!(summary.details[34], "entry034.txt");
    assert_eq!(summary.details[35], "+ 65 more entries");
}

#[test]
fn summarizes_long_listing_as_inventory() {
    let entries = vec![
        "total 184".to_string(),
        "drwxr-xr-x  16 jiatwork  staff    512 May 31 12:37 .".to_string(),
        "drwxr-xr-x  28 jiatwork  staff    896 May 28 11:36 ..".to_string(),
        "drwxr-xr-x@ 17 jiatwork  staff    544 May 31 11:40 .git".to_string(),
        "drwxr-xr-x@  3 jiatwork  staff     96 May 29 15:25 docs".to_string(),
        "-rw-r--r--@  1 jiatwork  staff     62 May 30 22:34 .gitignore".to_string(),
        "-rw-r--r--@  1 jiatwork  staff    999 May 29 14:27 Cargo.toml".to_string(),
    ];

    let summary = summarize_ls_entries(&entries, &["-la".to_string()]);

    assert_eq!(summary.summary, "entries=4; dirs=2; files=2; hidden=2");
    assert_eq!(summary.details[0], ".git/");
    assert_eq!(summary.details[1], "docs/");
    assert_eq!(summary.details[2], ".gitignore  62B");
    assert_eq!(summary.details[3], "Cargo.toml  999B");
    assert!(!summary.details.iter().any(|line| line == "total 184"));
    assert!(!summary.details.iter().any(|line| line.contains(" .")));
    assert!(!summary.details.iter().any(|line| line.contains(" ..")));
}
