use crate::providers::base::Provider;
use rmcp::model::Tool;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

// We use double Arc here to allow easy provider swaps while sharing concurrent access
pub type SharedProvider = Arc<Mutex<Option<Arc<dyn Provider>>>>;

/// A frontend tool that will be executed by the frontend rather than an extension
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendTool {
    pub name: String,
    pub tool: Tool,
}

/// Session configuration for an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Identifier of the underlying Session
    pub id: String,
    /// Maximum number of turns (iterations) allowed without user input
    pub max_turns: Option<u32>,
    /// Whether provider context should be rebuilt from the compacted resume view.
    #[serde(default)]
    pub compacted_context: bool,
    /// Tail size to use when compacted context is enabled.
    #[serde(default)]
    pub tail_limit: Option<usize>,
}
