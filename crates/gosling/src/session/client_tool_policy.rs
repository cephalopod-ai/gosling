//! Persisted opt-in authority for clients that expose a bounded tool catalog.
use super::Session;
use serde::{Deserialize, Serialize};

pub const KEY: &str = "client_tool_policy.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientToolPolicy {
    pub allowed_extensions: Vec<String>,
}

pub fn is_bounded(session: &Session) -> bool {
    session.extension_data.extension_states.contains_key(KEY)
}

pub fn allows_extension(session: &Session, name: &str) -> bool {
    session
        .extension_data
        .extension_states
        .get(KEY)
        .and_then(|value| serde_json::from_value::<ClientToolPolicy>(value.clone()).ok())
        .is_some_and(|policy| {
            policy
                .allowed_extensions
                .iter()
                .any(|allowed| allowed == name)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_policy_does_not_grant_extensions() {
        let mut session = Session::default();
        session
            .extension_data
            .extension_states
            .insert(KEY.into(), serde_json::json!({}));
        assert!(is_bounded(&session));
        assert!(!allows_extension(&session, "developer"));
    }
}
