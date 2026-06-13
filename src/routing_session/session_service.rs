use serde_json::Value;

use crate::host_adapter::{HostKind, capability_profile};
use crate::routing_session::IdentityContext;

pub struct SessionService;

impl SessionService {
    pub fn new() -> Self {
        Self
    }

    pub fn identity_from_hook_input(&self, host_kind: HostKind, input: &Value) -> IdentityContext {
        let profile = capability_profile(host_kind);

        IdentityContext {
            session_id: Self::read_first_string_path(
                input,
                profile.identity_paths.session_id_paths,
            ),
            agent_id: Self::read_first_string_path(input, profile.identity_paths.agent_id_paths),
            connection_id: Self::read_first_string_path(
                input,
                profile.identity_paths.connection_id_paths,
            ),
        }
    }

    pub fn compact_reset_ids(
        &self,
        host_kind: HostKind,
        input: &Value,
    ) -> Option<(String, String)> {
        let profile = capability_profile(host_kind);
        let session_id =
            Self::read_first_string_path(input, profile.identity_paths.session_id_paths);
        let connection_id =
            Self::read_first_string_path(input, profile.identity_paths.connection_id_paths)
                .or_else(|| session_id.clone());

        match (connection_id, session_id) {
            (Some(connection_id), Some(session_id)) => Some((connection_id, session_id)),
            _ => None,
        }
    }

    fn read_first_string_path(input: &Value, paths: &[&str]) -> Option<String> {
        paths
            .iter()
            .find_map(|path| Self::read_string_path(input, path))
            .filter(|value| !value.is_empty())
    }

    fn read_string_path(input: &Value, path: &str) -> Option<String> {
        let value = path
            .split('.')
            .try_fold(input, |current, segment| current.get(segment))?;
        value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    }
}

#[cfg(test)]
#[path = "session_service_tests.rs"]
mod tests;
