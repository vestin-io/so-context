use super::super::{install_all_for_home, uninstall_all_for_home};
use super::{all_renderers, all_renderers_for_home};
use crate::host_adapter::HostKind;
use std::fs;
use std::path::PathBuf;

fn temp_home() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "so-context-renderer-tests-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).expect("create temp home");
    dir
}

#[test]
fn renderer_list_covers_supported_hosts() {
    let hosts = all_renderers("/tmp/so-context")
        .into_iter()
        .map(|renderer| renderer.host_kind())
        .collect::<Vec<_>>();

    assert_eq!(
        hosts,
        vec![HostKind::Claude, HostKind::OpenCode, HostKind::Codex]
    );
}

#[test]
fn renderers_install_and_uninstall_real_setup_files() {
    let home = temp_home();
    let binary = "/tmp/ctx-custom";

    let hosts = all_renderers_for_home(&home, binary)
        .into_iter()
        .map(|renderer| renderer.host_kind())
        .collect::<Vec<_>>();
    assert_eq!(
        hosts,
        vec![HostKind::Claude, HostKind::OpenCode, HostKind::Codex]
    );

    install_all_for_home(&home, binary).expect("install all renderers");

    assert!(home.join(".claude").join("settings.json").exists());
    assert!(home.join(".claude").join("CLAUDE.md").exists());
    assert!(
        home.join(".claude")
            .join("rules")
            .join("so-context.md")
            .exists()
    );
    assert!(home.join(".codex").join("config.toml").exists());
    assert!(home.join(".codex").join("AGENTS.md").exists());
    assert!(home.join(".codex").join("SO-CONTEXT.md").exists());
    assert!(
        home.join(".config")
            .join("opencode")
            .join("opencode.json")
            .exists()
    );
    assert!(
        home.join(".config")
            .join("opencode")
            .join("plugins")
            .join("so-context.ts")
            .exists()
    );

    uninstall_all_for_home(&home, binary).expect("uninstall all renderers");

    let claude_settings = fs::read_to_string(home.join(".claude").join("settings.json"))
        .expect("read claude settings");
    let codex_config =
        fs::read_to_string(home.join(".codex").join("config.toml")).expect("read codex config");
    let opencode_config =
        fs::read_to_string(home.join(".config").join("opencode").join("opencode.json"))
            .expect("read opencode config");
    assert!(!claude_settings.contains("so-context"));
    assert!(!codex_config.contains("so-context"));
    assert!(!opencode_config.contains("so-context.ts"));
    assert!(
        !home
            .join(".claude")
            .join("rules")
            .join("so-context.md")
            .exists()
    );
    assert!(!home.join(".codex").join("SO-CONTEXT.md").exists());
    assert!(
        !home
            .join(".config")
            .join("opencode")
            .join("plugins")
            .join("so-context.ts")
            .exists()
    );

    let _ = fs::remove_dir_all(home);
}
