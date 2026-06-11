use std::fs::{self, OpenOptions};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use tokio::net::UnixStream;
use tokio::time::sleep;

use crate::socket::{ctrl_socket_path, log_path, pid_path, socket_path};
use crate::version::binary_version;

const START_TIMEOUT: Duration = Duration::from_secs(5);
const STOP_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

pub async fn start_background(binary: &str) -> Result<()> {
    if daemon_is_running().await? {
        println!("so-context daemon already running");
        println!("ctrl socket: {}", ctrl_socket_path().display());
        println!("pid file: {}", pid_path().display());
        return Ok(());
    }

    cleanup_stale_runtime_files()?;

    if let Some(parent) = log_path().parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create daemon runtime dir {}", parent.display()))?;
    }

    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path())
        .with_context(|| format!("open daemon log {}", log_path().display()))?;
    let stderr = stdout
        .try_clone()
        .with_context(|| format!("clone daemon log handle {}", log_path().display()))?;

    let mut child = Command::new(binary)
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .with_context(|| format!("spawn background daemon from {binary}"))?;

    wait_for_daemon_ready(&mut child).await?;

    println!("so-context daemon started in background");
    println!("ctrl socket: {}", ctrl_socket_path().display());
    println!("log file: {}", log_path().display());
    Ok(())
}

pub async fn stop_background() -> Result<()> {
    let pid = read_pid_file()?;
    if let Some(pid) = pid {
        if is_pid_running(pid)? {
            if !process_matches_daemon(pid)? {
                bail!(
                    "pid file points to pid {pid}, but that process is not `so-context daemon`.\nStop it manually if needed, then remove stale runtime files."
                );
            }
            signal_pid(pid, libc::SIGTERM)?;
            wait_for_daemon_stop(pid).await?;
            cleanup_stale_runtime_files()?;
            println!("so-context daemon stopped");
            return Ok(());
        }

        cleanup_stale_runtime_files()?;
        println!("so-context daemon was not running; removed stale runtime files");
        return Ok(());
    }

    if daemon_is_running().await? {
        bail!(
            "so-context daemon is running but no pid file was found.\nStop it manually, then remove stale runtime files if needed."
        );
    }

    cleanup_stale_runtime_files()?;
    println!("so-context daemon is not running");
    Ok(())
}

pub async fn restart_background(binary: &str) -> Result<()> {
    stop_background().await?;
    start_background(binary).await
}

pub async fn daemon_version_mismatch(binary: &str) -> Result<Option<DaemonVersionMismatch>> {
    if !daemon_is_running().await? {
        return Ok(None);
    }

    let Some(info) = running_daemon_info()? else {
        return Ok(None);
    };

    let current_version = binary_version(Path::new(binary))?;
    let Some(daemon_version) = info.version else {
        return Ok(None);
    };

    if daemon_version == current_version {
        return Ok(None);
    }

    Ok(Some(DaemonVersionMismatch {
        daemon_version,
        session_version: current_version,
        daemon_binary: info.binary,
        session_binary: PathBuf::from(binary),
    }))
}

pub(crate) async fn ensure_no_active_daemon() -> Result<()> {
    if daemon_is_running().await? {
        bail!(
            "so-context daemon is already running.\nUse `so-context stop` before starting another instance."
        );
    }
    Ok(())
}

pub(crate) fn write_pid_file_for_current_process() -> Result<PidFileGuard> {
    let path = pid_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("create daemon runtime dir {}", parent.display()))?;
    }

    fs::write(&path, std::process::id().to_string())
        .with_context(|| format!("write daemon pid file {}", path.display()))?;
    Ok(PidFileGuard { path })
}

pub(crate) fn cleanup_stale_runtime_files() -> Result<()> {
    remove_if_exists(&socket_path())?;
    remove_if_exists(&ctrl_socket_path())?;
    remove_if_exists(&pid_path())?;
    Ok(())
}

async fn daemon_is_running() -> Result<bool> {
    let sock = ctrl_socket_path();
    if !sock.exists() {
        return Ok(false);
    }

    match UnixStream::connect(&sock).await {
        Ok(_) => Ok(true),
        Err(err)
            if matches!(
                err.kind(),
                ErrorKind::ConnectionRefused | ErrorKind::NotFound | ErrorKind::ConnectionReset
            ) =>
        {
            Ok(false)
        }
        Err(err) => Err(anyhow!("connect to ctrl socket {}: {err}", sock.display())),
    }
}

async fn wait_for_daemon_ready(child: &mut Child) -> Result<()> {
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        if daemon_is_running().await? {
            return Ok(());
        }

        if let Some(status) = child
            .try_wait()
            .context("check background daemon process status")?
        {
            bail!(
                "so-context daemon exited before becoming ready (status: {status}).\nSee log: {}",
                log_path().display()
            );
        }

        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!(
                "timed out waiting for so-context daemon to become ready.\nSee log: {}",
                log_path().display()
            );
        }

        sleep(POLL_INTERVAL).await;
    }
}

async fn wait_for_daemon_stop(pid: u32) -> Result<()> {
    let deadline = Instant::now() + STOP_TIMEOUT;
    loop {
        if !is_pid_running(pid)? && !ctrl_socket_path().exists() && !socket_path().exists() {
            return Ok(());
        }
        if !is_pid_running(pid)? && !daemon_is_running().await? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            bail!("timed out waiting for so-context daemon to stop");
        }
        sleep(POLL_INTERVAL).await;
    }
}

fn read_pid_file() -> Result<Option<u32>> {
    let path = pid_path();
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("read daemon pid file {}", path.display()))?;
    let pid = raw
        .trim()
        .parse::<u32>()
        .with_context(|| format!("parse daemon pid file {}", path.display()))?;
    Ok(Some(pid))
}

fn is_pid_running(pid: u32) -> Result<bool> {
    let rc = unsafe { libc::kill(pid as i32, 0) };
    if rc == 0 {
        return Ok(true);
    }
    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err(anyhow!("probe daemon pid {pid}: {err}")),
    }
}

fn signal_pid(pid: u32, signal: i32) -> Result<()> {
    let rc = unsafe { libc::kill(pid as i32, signal) };
    if rc == 0 {
        return Ok(());
    }
    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(libc::ESRCH) => Ok(()),
        _ => Err(anyhow!("signal daemon pid {pid}: {err}")),
    }
}

fn process_matches_daemon(pid: u32) -> Result<bool> {
    Ok(running_daemon_info_for_pid(pid)?.is_some())
}

fn command_line_matches_daemon(command: &str) -> bool {
    let mut saw_binary = false;
    for token in command.split_whitespace() {
        if !saw_binary && token.contains("so-context") {
            saw_binary = true;
            continue;
        }
        if saw_binary && token == "daemon" {
            return true;
        }
    }
    false
}

pub struct DaemonVersionMismatch {
    pub daemon_version: String,
    pub session_version: String,
    pub daemon_binary: PathBuf,
    pub session_binary: PathBuf,
}

struct RunningDaemonInfo {
    binary: PathBuf,
    version: Option<String>,
}

fn running_daemon_info() -> Result<Option<RunningDaemonInfo>> {
    let Some(pid) = read_pid_file()? else {
        return Ok(None);
    };
    running_daemon_info_for_pid(pid)
}

fn running_daemon_info_for_pid(pid: u32) -> Result<Option<RunningDaemonInfo>> {
    let output = Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "command="])
        .output()
        .with_context(|| format!("inspect daemon pid {pid} with ps"))?;
    if !output.status.success() {
        return Ok(None);
    }

    let command = String::from_utf8_lossy(&output.stdout);
    let command = command.trim();
    if !command_line_matches_daemon(command) {
        return Ok(None);
    }

    let Some(binary) = daemon_binary_from_command_line(command) else {
        return Ok(None);
    };

    Ok(Some(RunningDaemonInfo {
        version: binary_version(&binary).ok(),
        binary,
    }))
}

fn daemon_binary_from_command_line(command: &str) -> Option<PathBuf> {
    command
        .split_whitespace()
        .next()
        .filter(|token| token.contains("so-context"))
        .map(PathBuf::from)
}

fn remove_if_exists(path: &std::path::Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(anyhow!("remove {}: {err}", path.display())),
    }
}

pub(crate) struct PidFileGuard {
    path: std::path::PathBuf,
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod control_tests;
