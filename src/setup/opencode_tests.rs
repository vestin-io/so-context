use super::{install_into_home, render_plugin_source, uninstall_from_home};
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
        r#"const NATIVE_READ_TOOL_NAMES = new Set(["Read","read","View","view","read_file"]);"#
    ));
    assert!(source.contains(
        r#"const NATIVE_SEARCH_TOOL_NAMES = new Set(["Grep","grep","rg","ripgrep","SearchFiles","search_files"]);"#
    ));
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
    assert!(source.contains(r#"hookOutput.permissionDecision === "allow""#));
    assert!(source.contains("replaceOutputArgs(output as any, hookOutput.updatedInput);"));
    assert!(source.contains(r#"hookOutput.permissionDecision === "deny""#));
    assert!(source.contains("throw new Error(String(hookOutput.permissionDecisionReason"));
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
}

#[test]
fn install_and_uninstall_opencode_files_preserve_unrelated_config() {
    let home = temp_home();
    let config_dir = home.join(".config").join("opencode");
    fs::create_dir_all(&config_dir).expect("create config dir");
    let config_path = config_dir.join("opencode.json");
    fs::write(
        &config_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "theme": "solarized",
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
    assert!(config_dir.join("plugins").join("so-context.ts").exists());

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
    assert!(!config_dir.join("plugins").join("so-context.ts").exists());

    let _ = fs::remove_dir_all(home);
}
