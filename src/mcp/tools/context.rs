use std::path::PathBuf;

use rmcp::ErrorData;
use serde_json::Map;
use serde_json::Value;

use crate::daemon::watch_manager::ProjectStatus;

pub(crate) fn infer_connection_project_root(
    statuses: &[ProjectStatus],
    client: &str,
    connection_id: &str,
) -> Result<PathBuf, ErrorData> {
    let mut matches: Vec<PathBuf> = statuses
        .iter()
        .filter(|status| {
            status
                .consumers
                .iter()
                .any(|consumer| consumer.client == client && consumer.session_id == connection_id)
        })
        .map(|status| status.path.clone())
        .collect();

    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => Err(ErrorData::invalid_params(
            "could not infer project root from watched projects for this MCP connection; pass path explicitly",
            None,
        )),
        _ => Err(ErrorData::invalid_params(
            "multiple watched projects are active for this MCP connection; pass path explicitly",
            None,
        )),
    }
}

pub(crate) fn resolve_project_path_arg(
    args: &Map<String, Value>,
    arg_name: &str,
    client: &Option<String>,
    connection_id: &str,
    statuses: &[ProjectStatus],
) -> Result<PathBuf, ErrorData> {
    if let Some(path) = args.get(arg_name).and_then(Value::as_str) {
        if path.trim().is_empty() {
            return Err(ErrorData::invalid_params(
                format!("{arg_name} must not be empty when provided"),
                None,
            ));
        }

        let path_buf = PathBuf::from(path);
        if path_buf.is_absolute() {
            return Ok(path_buf);
        }

        let project_root = infer_connection_project_root(
            statuses,
            client.as_deref().unwrap_or("unknown"),
            connection_id,
        )?;
        return Ok(project_root.join(path_buf));
    }

    infer_connection_project_root(
        statuses,
        client.as_deref().unwrap_or("unknown"),
        connection_id,
    )
}

pub(crate) fn infer_connection_project_for_path(
    statuses: &[ProjectStatus],
    client: &str,
    connection_id: &str,
    path: &str,
) -> Option<PathBuf> {
    let path = PathBuf::from(path);
    let mut matches: Vec<PathBuf> =
        statuses
            .iter()
            .filter(|status| {
                status.consumers.iter().any(|consumer| {
                    consumer.client == client && consumer.session_id == connection_id
                }) && path.starts_with(&status.path)
            })
            .map(|status| status.path.clone())
            .collect();

    matches.sort_by_key(|candidate| std::cmp::Reverse(candidate.components().count()));
    matches.into_iter().next()
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
