use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(prefix: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("{prefix}-{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn so_context_bin() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_so-context").map(PathBuf::from)
        && path.exists()
    {
        return path;
    }

    let test_exe = std::env::current_exe().unwrap();
    let bin = test_exe
        .parent()
        .and_then(|deps| deps.parent())
        .map(|dir| dir.join("so-context"))
        .unwrap();
    assert!(
        bin.exists(),
        "expected so-context binary at {}",
        bin.display()
    );
    bin
}

#[test]
fn shell_command_renders_compressed_summary() {
    let dir = temp_dir("so-context-shell-cli");
    fs::write(dir.join("alpha.txt"), "hello").unwrap();
    fs::write(dir.join("beta.txt"), "world").unwrap();
    fs::create_dir(dir.join("src")).unwrap();

    let output = Command::new(so_context_bin())
        .current_dir(&dir)
        .args(["shell", "ls", "-la"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("entries=3"));
    assert!(stdout.contains("- src/"));
    assert!(stdout.contains("- alpha.txt  5B"));

    let _ = fs::remove_dir_all(dir);
}

#[test]
fn shell_command_renders_full_output() {
    let output = Command::new(so_context_bin())
        .args(["shell", "--full", "sh", "-c", "printf hello"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "hello");
}

#[test]
fn shell_command_string_preserves_native_shell_expansion() {
    let dir = temp_dir("so-context-shell-cli-command");
    fs::write(dir.join("alpha.rs"), "alpha").unwrap();
    fs::write(dir.join("beta.txt"), "beta").unwrap();

    let output = Command::new(so_context_bin())
        .current_dir(&dir)
        .args(["shell", "-c", "ls *.rs"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("alpha.rs"));
    assert!(!stdout.contains("beta.txt"));

    let _ = fs::remove_dir_all(dir);
}
