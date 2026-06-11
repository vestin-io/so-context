use super::{latest_release_asset_url, release_source_archive_url};
use crate::version::release_target_from_parts;

#[test]
fn maps_supported_targets() {
    assert_eq!(
        release_target_from_parts("macos", "arm64").unwrap(),
        "aarch64-apple-darwin"
    );
    assert_eq!(
        release_target_from_parts("linux", "x86_64").unwrap(),
        "x86_64-unknown-linux-gnu"
    );
}

#[test]
fn rejects_unsupported_targets() {
    assert!(release_target_from_parts("windows", "x86_64").is_err());
    assert!(release_target_from_parts("macos", "riscv64").is_err());
}

#[test]
fn builds_latest_release_asset_url() {
    assert_eq!(
        latest_release_asset_url("aarch64-apple-darwin"),
        "https://github.com/vestin-io/so-context/releases/latest/download/so-context-aarch64-apple-darwin.tar.gz"
    );
}

#[test]
fn builds_release_source_archive_url() {
    assert_eq!(
        release_source_archive_url("v0.0.12"),
        "https://github.com/vestin-io/so-context/archive/refs/tags/v0.0.12.tar.gz"
    );
}
