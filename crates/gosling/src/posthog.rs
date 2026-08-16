//! Product-telemetry seam, permanently disabled.
//!
//! Gosling does not collect product telemetry. This module used to carry the
//! full PostHog implementation — a hard-coded third-party API key, the capture
//! endpoint, a `reqwest` client, installation-ID tracking that wrote a file
//! under the config dir, platform/install-method fingerprinting, and several
//! event senders — all of it behind `is_telemetry_enabled()`, which returns a
//! constant `false`.
//!
//! That made every send path unreachable while still shipping the API key and
//! endpoint as strings in the binary and keeping ~500 lines of fingerprinting
//! code alive for readers to audit (DEAD-GSL-003, RSP-GSL-005). The seam is
//! kept because callers across the agent and the server reference it, but the
//! machinery is gone: these are no-ops, and the key, the endpoint, and the
//! installation file are no longer part of the build.

use std::collections::HashMap;

/// Legacy config key retained for compatibility with existing configuration.
/// Reading it changes nothing; telemetry is off regardless of its value.
pub const TELEMETRY_ENABLED_KEY: &str = "GOSLING_TELEMETRY_ENABLED";

/// Gosling does not permit product telemetry collection.
pub fn is_telemetry_enabled() -> bool {
    false
}

/// No-op. Retained so session setup code can record its interface without a
/// conditional.
pub fn set_session_context(_interface: &str, _is_resumed: bool) {}

/// No-op.
pub fn emit_session_started() {}

/// No-op. Errors are surfaced through `tracing` at the call site; nothing is
/// transmitted off the machine.
pub fn emit_error(_error_type: &str, _error_message: &str) {}

/// No-op. Kept `async` and fallible so the server's telemetry route compiles
/// unchanged.
pub async fn emit_event(
    _event_name: &str,
    _properties: HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn telemetry_is_off_and_has_no_toggle() {
        assert!(!is_telemetry_enabled());
    }

    #[tokio::test]
    async fn emitting_an_event_is_a_no_op_that_cannot_fail() {
        assert!(emit_event("anything", HashMap::new()).await.is_ok());
    }
}
