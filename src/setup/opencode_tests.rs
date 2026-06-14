use super::{install_into_home, render_plugin_source, uninstall_from_home};
use crate::setup::instructions::opencode_instructions_body;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

fn temp_home() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "so-context-opencode-tests-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir_all(&dir).expect("create temp home");
    dir
}

#[test]
fn generated_plugin_uses_shared_runtime_contract_for_opencode() {
    let source = render_plugin_source("/opt/bin/so-context").expect("plugin source");

    assert!(source.contains(r#"const SO_CONTEXT_TOOL_PREFIX = "so-context_";"#));
    assert!(source.contains(r#"const SESSION_ID_PATHS: string[] = ["session.id","sessionID"];"#));
    assert!(source.contains(
        r#"const NATIVE_SHELL_TOOL_NAMES = new Set(["Bash","bash","Shell","shell","runTerminalCommand","runInTerminal","run_in_terminal","terminal","shell_command","exec_command","local_shell","run_shell_command"]);"#
    ));
    assert!(source.contains("if (!shouldDelegateToSharedHook(toolName)) return;"));
    assert!(source.contains("const isSelfTool = toolName.startsWith(SO_CONTEXT_TOOL_PREFIX);"));
    assert!(source.contains(r#"const sessionId = isSelfTool ? resolveSessionId(input) : "";"#));
    assert!(source.contains(r#"tool_name: String(input.tool ?? "")"#));
    assert!(source.contains("tool_input: args"));
    assert!(source.contains("hook pre-tool --host opencode"));
    assert!(
        source.contains("function hookFailureMessage(toolName: string, message: string): string {")
    );
    assert!(source.contains(
        "return `[so-context] shared pre-tool hook failed for ${toolName}: ${message}`;"
    ));
    assert!(source.contains("const message = hookFailureMessage(toolName, hookResult.message);"));
    assert!(source.contains("if (isSelfTool) {"));
    assert!(source.contains("console.warn(message);"));
    assert!(source.contains("if (sessionId) fallbackInjectSessionId(output as any, sessionId);"));
    assert!(source.contains("fallbackInjectSessionId(output as any, sessionId);"));
    assert!(source.contains("throw new Error(`${message}. Native reroute was not bypassed.`);"));
    assert!(source.contains("if (!hookOutput) {"));
    assert!(source.contains(
        r#"if (hookOutput.updatedInput && typeof hookOutput.updatedInput === "object") {"#
    ));
    assert!(source.contains("replaceOutputArgs(output as any, hookOutput.updatedInput);"));
    assert!(source.contains("function applyInputKeyOrder("));
    assert!(source.contains("if (Array.isArray(hookOutput.inputKeyOrder)) {"));
    assert!(source.contains("applyInputKeyOrder(output as any, hookOutput.inputKeyOrder);"));
    assert!(source.contains("hook post-compact --host opencode"));
    assert!(source.contains("session: { id: sessionId }"));
    assert!(source.contains("sessionID: sessionId"));
    assert!(
        source
            .contains("console.warn(`[so-context] shared post-compact hook failed: ${message}`);")
    );
    assert!(!source.contains(r#"import net from "node:net";"#));
    assert!(!source.contains(r#"method: "pre_tool_decision""#));
    assert!(!source.contains("const socket = ctrlSocketPath();"));
    assert!(!source.contains("const daemonResult = await queryPreToolDecisionViaDaemon("));
    assert!(!source.contains(r#""permission.ask": async (input, output) => {"#));
    assert!(!source.contains(r#"hookOutput.permissionDecision === "deny""#));
    assert!(!source.contains("function deriveShellDisplayCommand("));
    assert!(!source.contains("function reorderSoShellArgs("));
    assert!(!source.contains("function normalizeSelfToolArgs("));
}

#[test]
fn install_and_uninstall_opencode_files_preserve_unrelated_config() {
    let managed_instructions = opencode_instructions_body();
    let home = temp_home();
    let config_dir = home.join(".config").join("opencode");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let config_path = config_dir.join("opencode.json");
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "theme": "solarized",
            "permission": {
                "bash": "deny"
            },
            "instructions": [
                "## so-context\n\nlegacy managed instructions",
                "keep this user instruction"
            ],
            "mcp": {
                "other": {
                    "type": "local",
                    "command": ["/usr/bin/other", "mcp"],
                    "enabled": true
                }
            }
        }))
        .expect("serialize config"),
    )
    .expect("write seed config");

    install_into_home(&home, "/opt/bin/so-context").expect("install");

    let installed: Value =
        serde_json::from_str(&fs::read_to_string(&config_path).expect("read config"))
            .expect("parse config");
    assert_eq!(installed["theme"].as_str(), Some("solarized"));
    assert_eq!(
        installed["mcp"]["other"]["command"][0].as_str(),
        Some("/usr/bin/other")
    );
    assert_eq!(
        installed["mcp"]["so-context"]["command"][0].as_str(),
        Some("/opt/bin/so-context")
    );
    assert_eq!(installed["permission"]["bash"].as_str(), Some("deny"));
    assert!(installed["permission"].get("read").is_none());
    assert!(installed["permission"].get("grep").is_none());
    let instructions = installed["instructions"]
        .as_array()
        .expect("instructions array");
    assert_eq!(
        instructions
            .iter()
            .filter(|item| item
                .as_str()
                .map(|value| value.starts_with("## so-context\n\n"))
                .unwrap_or(false))
            .count(),
        1
    );
    assert!(
        instructions
            .iter()
            .any(|item| item.as_str() == Some(managed_instructions.as_str()))
    );
    assert!(
        instructions
            .iter()
            .any(|item| item.as_str() == Some("keep this user instruction"))
    );
    let plugin_path = config_dir.join("plugins").join("so-context.ts");
    let plugin_source = fs::read_to_string(&plugin_path).expect("read plugin source");
    assert!(!plugin_source.contains("__SO_CONTEXT_PERMISSION_BACKUP__="));
    assert!(plugin_path.exists());

    uninstall_from_home(&home).expect("uninstall");

    let uninstalled: Value = serde_json::from_str(
        &fs::read_to_string(&config_path).expect("read config after uninstall"),
    )
    .expect("parse config after uninstall");
    assert_eq!(uninstalled["theme"].as_str(), Some("solarized"));
    assert!(uninstalled["mcp"].get("so-context").is_none());
    assert_eq!(
        uninstalled["mcp"]["other"]["command"][0].as_str(),
        Some("/usr/bin/other")
    );
    assert_eq!(uninstalled["permission"]["bash"].as_str(), Some("deny"));
    assert!(uninstalled["permission"].get("read").is_none());
    assert!(uninstalled["permission"].get("grep").is_none());
    let instructions = uninstalled["instructions"]
        .as_array()
        .expect("instructions array after uninstall");
    assert!(
        !instructions
            .iter()
            .any(|item| item.as_str() == Some(managed_instructions.as_str()))
    );
    assert!(!instructions.iter().any(|item| {
        item.as_str()
            .map(|value| value.starts_with("## so-context\n\n"))
            .unwrap_or(false)
    }));
    assert!(
        instructions
            .iter()
            .any(|item| item.as_str() == Some("keep this user instruction"))
    );
    assert!(!config_dir.join("plugins").join("so-context.ts").exists());

    let _ = fs::remove_dir_all(home);
}

#[test]
fn install_cleans_legacy_managed_opencode_permissions_from_previous_plugin() {
    let home = temp_home();
    let config_dir = home.join(".config").join("opencode");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let config_path = config_dir.join("opencode.json");
    let plugin_dir = config_dir.join("plugins");
    fs::create_dir_all(&plugin_dir).expect("create plugin dir");
    let plugin_path = plugin_dir.join("so-context.ts");
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "permission": {
                "bash": "deny",
                "read": "ask",
                "grep": "ask"
            }
        }))
        .expect("serialize config"),
    )
    .expect("write seed config");
    fs::write(
        &plugin_path,
        r#"// __SO_CONTEXT_PERMISSION_BACKUP__={"bash":"deny"}"#,
    )
    .expect("write legacy plugin marker");

    install_into_home(&home, "/opt/bin/so-context").expect("install");

    let installed: Value =
        serde_json::from_str(&fs::read_to_string(&config_path).expect("read config"))
            .expect("parse config");
    assert_eq!(installed["permission"]["bash"].as_str(), Some("deny"));
    assert!(installed["permission"].get("read").is_none());
    assert!(installed["permission"].get("grep").is_none());

    let _ = fs::remove_dir_all(home);
}

#[test]
fn install_cleans_pure_legacy_managed_opencode_permissions_without_plugin_marker() {
    let home = temp_home();
    let config_dir = home.join(".config").join("opencode");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let config_path = config_dir.join("opencode.json");
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "permission": {
                "read": "ask",
                "grep": "ask"
            }
        }))
        .expect("serialize config"),
    )
    .expect("write seed config");

    install_into_home(&home, "/opt/bin/so-context").expect("install");

    let installed: Value =
        serde_json::from_str(&fs::read_to_string(&config_path).expect("read config"))
            .expect("parse config");
    assert!(installed.get("permission").is_none());

    let _ = fs::remove_dir_all(home);
}
