use std::cmp::Ordering;

use super::{
    SemanticVersion, parse_semantic_version, parse_version_output, release_asset_name,
    release_target_from_parts, version_order,
};

#[test]
fn parses_version_from_standard_output() {
    assert_eq!(
        parse_version_output("so-context 0.0.12\n"),
        Some("0.0.12".to_string())
    );
    assert_eq!(parse_version_output("unexpected output"), None);
}

#[test]
fn parses_semantic_versions_with_prefixes_and_suffixes() {
    assert_eq!(
        parse_semantic_version("v1.2.3-beta.1"),
        Some(SemanticVersion {
            major: 1,
            minor: 2,
            patch: 3,
        })
    );
    assert_eq!(
        parse_semantic_version("0.0.12+build"),
        Some(SemanticVersion {
            major: 0,
            minor: 0,
            patch: 12,
        })
    );
}

#[test]
fn compares_versions_for_outdated_detection() {
    assert_eq!(version_order("0.0.12", "v0.0.13"), Some(Ordering::Less));
    assert_eq!(version_order("0.0.12", "0.0.12"), Some(Ordering::Equal));
    assert_eq!(
        version_order("0.1.0-dev", "0.0.12"),
        Some(Ordering::Greater)
    );
}

#[test]
fn builds_release_target_and_asset_name() {
    assert_eq!(
        release_target_from_parts("macos", "arm64").unwrap(),
        "aarch64-apple-darwin"
    );
    assert_eq!(
        release_asset_name("x86_64-unknown-linux-gnu"),
        "so-context-x86_64-unknown-linux-gnu.tar.gz"
    );
}
