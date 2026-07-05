use crate::config::Config;

/// Runtime mode for the Context Manager, controlled by `GOSLING_CONTEXT_MANAGER`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContextManagerMode {
    /// Current Gosling behavior: the Context Manager never runs.
    #[default]
    Off,
    /// Build the `ContextPacket` and log its metadata, but leave the actual
    /// provider input untouched.
    Shadow,
    /// Build the `ContextPacket` and use it as the provider input.
    On,
}

impl ContextManagerMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContextManagerMode::Off => "off",
            ContextManagerMode::Shadow => "shadow",
            ContextManagerMode::On => "on",
        }
    }
}

/// Reads `GOSLING_CONTEXT_MANAGER` (env var or config), defaulting to `off`.
pub fn context_manager_mode() -> ContextManagerMode {
    let raw = Config::global()
        .get_param::<String>("GOSLING_CONTEXT_MANAGER")
        .unwrap_or_else(|_| "off".to_string());

    match raw.trim().to_lowercase().as_str() {
        "shadow" => ContextManagerMode::Shadow,
        "on" => ContextManagerMode::On,
        _ => ContextManagerMode::Off,
    }
}

/// How many of the most recent messages (before the required tail) are kept
/// in full as `High` priority before older messages become summarization
/// candidates. This is the baseline value for a [`BASELINE_CONTEXT_LIMIT`]
/// model; use [`recent_message_window_for`] to scale it to a provider's
/// actual context window.
pub const RECENT_MESSAGE_WINDOW: usize = 10;

/// A single tool result above this estimated token count is treated as a
/// "long tool output" summarization candidate even inside the recent window.
/// This is the baseline value for a [`BASELINE_CONTEXT_LIMIT`] model; use
/// [`long_tool_output_threshold_for`] to scale it to a provider's actual
/// context window.
pub const LONG_TOOL_OUTPUT_TOKEN_THRESHOLD: usize = 800;

/// The context window `RECENT_MESSAGE_WINDOW` and
/// `LONG_TOOL_OUTPUT_TOKEN_THRESHOLD` are tuned for. Scaling factors in
/// [`recent_message_window_for`] and [`long_tool_output_threshold_for`] are
/// relative to this baseline.
pub const BASELINE_CONTEXT_LIMIT: usize = 128_000;

/// Upper bound on how far `recent_message_window_for` will scale up, so an
/// extreme context limit can't turn every message into a `High` priority
/// block and defeat summarization entirely.
const MAX_RECENT_MESSAGE_WINDOW: usize = RECENT_MESSAGE_WINDOW * 20;

/// Scales the recent-message window with the provider's context limit.
///
/// Without this, a fixed window (tuned for a 128K model) collapses
/// everything older than the last `RECENT_MESSAGE_WINDOW` messages into a
/// single truncated summary once the budget is exceeded — regardless of how
/// much headroom a 200K or 1M context model actually has. Scaling the window
/// lets larger-context models keep proportionally more raw conversation
/// before older turns become summarization candidates.
pub fn recent_message_window_for(context_limit: usize) -> usize {
    let scaled = (context_limit as f64 / BASELINE_CONTEXT_LIMIT as f64
        * RECENT_MESSAGE_WINDOW as f64)
        .round() as usize;
    scaled.clamp(RECENT_MESSAGE_WINDOW, MAX_RECENT_MESSAGE_WINDOW)
}

/// Scales the "long tool output" token threshold with the provider's context
/// limit, so a tool result has to be proportionally larger before a
/// large-context model treats it as a summarization candidate.
pub fn long_tool_output_threshold_for(context_limit: usize) -> usize {
    let scaled = (context_limit as f64 / BASELINE_CONTEXT_LIMIT as f64
        * LONG_TOOL_OUTPUT_TOKEN_THRESHOLD as f64)
        .round() as usize;
    scaled.max(LONG_TOOL_OUTPUT_TOKEN_THRESHOLD)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn defaults_to_off() {
        std::env::remove_var("GOSLING_CONTEXT_MANAGER");
        assert_eq!(context_manager_mode(), ContextManagerMode::Off);
    }

    #[test]
    #[serial]
    fn reads_shadow_and_on() {
        std::env::set_var("GOSLING_CONTEXT_MANAGER", "shadow");
        assert_eq!(context_manager_mode(), ContextManagerMode::Shadow);

        std::env::set_var("GOSLING_CONTEXT_MANAGER", "ON");
        assert_eq!(context_manager_mode(), ContextManagerMode::On);

        std::env::remove_var("GOSLING_CONTEXT_MANAGER");
    }

    #[test]
    fn recent_message_window_scales_with_context_limit() {
        assert_eq!(recent_message_window_for(128_000), 10);
        assert_eq!(recent_message_window_for(200_000), 16);
        assert_eq!(recent_message_window_for(1_000_000), 78);
        // Never shrinks below the baseline for small-context models.
        assert_eq!(recent_message_window_for(10_000), RECENT_MESSAGE_WINDOW);
    }

    #[test]
    fn long_tool_output_threshold_scales_with_context_limit() {
        assert_eq!(long_tool_output_threshold_for(128_000), 800);
        assert_eq!(long_tool_output_threshold_for(200_000), 1250);
        assert_eq!(long_tool_output_threshold_for(1_000_000), 6250);
        // Never shrinks below the baseline for small-context models.
        assert_eq!(
            long_tool_output_threshold_for(10_000),
            LONG_TOOL_OUTPUT_TOKEN_THRESHOLD
        );
    }
}
