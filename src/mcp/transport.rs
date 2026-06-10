use anyhow::Result;

use crate::core_metrics::{MetricsSummary, MetricsWindow};
use crate::socket::{ctrl_socket_path, socket_path};

const CTRL_QUERY_TIMEOUT_MS: u64 = 5_000;

pub async fn send_compact_reset(connection_id: &str, session_id: &str) -> Result<()> {
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "compact_reset",
        "params": {
            "connection_id": connection_id,
            "session_id":    session_id,
        }
    });
    send_ctrl_message(&msg, true).await
}

pub async fn send_ctrl_request(
    method: &str,
    path: &str,
    client: Option<&str>,
    session_id: Option<&str>,
) -> Result<()> {
    let abs_path = if std::path::Path::new(path).is_absolute() {
        path.to_string()
    } else {
        std::env::current_dir()
            .map(|d| d.join(path).to_string_lossy().to_string())
            .unwrap_or_else(|_| path.to_string())
    };

    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": {
            "path":       abs_path,
            "client":     client,
            "session_id": session_id,
        }
    });
    send_ctrl_message(&msg, false).await
}

pub async fn send_ctrl_status_request() -> Result<String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let sock = ctrl_socket_path();
    if !sock.exists() {
        return Err(anyhow::anyhow!(
            "so-context daemon is not running.\nStart it with: so-context daemon"
        ));
    }

    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "status",
        "method": "status",
        "params": {}
    });

    let mut line = serde_json::to_string(&msg)?;
    line.push('\n');

    let stream = UnixStream::connect(&sock)
        .await
        .map_err(|e| anyhow::anyhow!("connect to ctrl socket: {e}"))?;
    let (read_half, mut write_half) = stream.into_split();

    write_half
        .write_all(line.as_bytes())
        .await
        .map_err(|e| anyhow::anyhow!("write to ctrl socket: {e}"))?;

    let mut lines = BufReader::new(read_half).lines();
    let Some(response_line) = tokio::time::timeout(
        std::time::Duration::from_millis(CTRL_QUERY_TIMEOUT_MS),
        lines.next_line(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for status response from so-context daemon"))?
    .map_err(|e| anyhow::anyhow!("read ctrl response: {e}"))?
    else {
        return Err(anyhow::anyhow!(
            "so-context daemon did not respond to status request"
        ));
    };

    let response: serde_json::Value = serde_json::from_str(&response_line)?;
    Ok(response
        .pointer("/result/text")
        .and_then(|v| v.as_str())
        .unwrap_or("no projects being watched")
        .to_string())
}

pub async fn send_ctrl_metrics_request(
    window: MetricsWindow,
    project: Option<&str>,
    top: usize,
) -> Result<MetricsSummary> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    let sock = ctrl_socket_path();
    if !sock.exists() {
        return Err(anyhow::anyhow!(
            "so-context daemon is not running.\nStart it with: so-context daemon"
        ));
    }

    let normalized_project = project.map(|path| {
        if std::path::Path::new(path).is_absolute() {
            path.to_string()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path).to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string())
        }
    });

    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "metrics",
        "method": "metrics",
        "params": {
            "window": window.label(),
            "project": normalized_project,
            "top": top,
        }
    });

    let mut line = serde_json::to_string(&msg)?;
    line.push('\n');

    let stream = UnixStream::connect(&sock).await.map_err(|e| {
        if matches!(
            e.kind(),
            std::io::ErrorKind::ConnectionRefused
                | std::io::ErrorKind::NotFound
                | std::io::ErrorKind::ConnectionReset
        ) {
            anyhow::anyhow!("so-context daemon is not running.\nStart it with: so-context daemon")
        } else {
            anyhow::anyhow!("connect to ctrl socket: {e}")
        }
    })?;
    let (read_half, mut write_half) = stream.into_split();

    write_half
        .write_all(line.as_bytes())
        .await
        .map_err(|e| anyhow::anyhow!("write to ctrl socket: {e}"))?;

    let mut lines = BufReader::new(read_half).lines();
    let Some(response_line) = tokio::time::timeout(
        std::time::Duration::from_millis(CTRL_QUERY_TIMEOUT_MS),
        lines.next_line(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("timed out waiting for metrics response from so-context daemon"))?
    .map_err(|e| anyhow::anyhow!("read ctrl response: {e}"))?
    else {
        return Err(anyhow::anyhow!("empty ctrl response for metrics"));
    };

    let response: serde_json::Value = serde_json::from_str(&response_line)?;
    if let Some(message) = response.pointer("/error/message").and_then(|v| v.as_str()) {
        return Err(anyhow::anyhow!(message.to_string()));
    }

    let Some(summary) = response.get("result") else {
        return Err(anyhow::anyhow!("missing ctrl metrics result payload"));
    };

    Ok(serde_json::from_value(summary.clone())?)
}

async fn send_ctrl_message(msg: &serde_json::Value, silent_if_missing: bool) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;

    let sock = ctrl_socket_path();
    if !sock.exists() {
        if silent_if_missing {
            return Ok(());
        }
        let method = msg
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("request");
        let path = msg
            .pointer("/params/path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        eprintln!("so-context: daemon not running, skipping {method} for {path}");
        return Ok(());
    }

    let mut line = serde_json::to_string(msg)?;
    line.push('\n');

    let mut stream = UnixStream::connect(&sock)
        .await
        .map_err(|e| anyhow::anyhow!("connect to ctrl socket: {e}"))?;
    stream
        .write_all(line.as_bytes())
        .await
        .map_err(|e| anyhow::anyhow!("write to ctrl socket: {e}"))?;

    Ok(())
}

pub async fn run_mcp_bridge() -> Result<()> {
    use tokio::net::UnixStream;

    let sock = socket_path();
    let stream = {
        let mut last_err = String::new();
        let mut connected = None;
        for _ in 0..20 {
            match UnixStream::connect(&sock).await {
                Ok(s) => {
                    connected = Some(s);
                    break;
                }
                Err(e) => {
                    last_err = e.to_string();
                    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                }
            }
        }
        connected.ok_or_else(|| {
            anyhow::anyhow!(
                "so-context daemon is not running (could not connect to {}: {}).\n\
             Start it with: so-context daemon",
                sock.display(),
                last_err
            )
        })?
    };

    let (mut sock_read, mut sock_write) = tokio::io::split(stream);
    let stdin_to_sock = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let _ = tokio::io::copy(&mut stdin, &mut sock_write).await;
    });

    let sock_to_stdout = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        let _ = tokio::io::copy(&mut sock_read, &mut stdout).await;
    });

    tokio::select! {
        _ = stdin_to_sock  => {}
        _ = sock_to_stdout => {}
    }

    Ok(())
}
