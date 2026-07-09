use serde::{Deserialize, Serialize};
use strum::{Display, EnumMessage, EnumString, IntoStaticStr, VariantNames};
use utoipa::ToSchema;

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
    PartialEq,
    Serialize,
    Deserialize,
    Display,
    EnumMessage,
    EnumString,
    IntoStaticStr,
    VariantNames,
    ToSchema,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum GoslingMode {
    #[strum(message = "Automatically approve tool calls")]
    Auto,
    #[strum(message = "Ask only for sensitive tool calls")]
    #[default]
    SmartApprove,
    #[strum(message = "Ask before every tool call")]
    Approve,
    #[strum(message = "Chat only, no tool calls")]
    Chat,
}

#[cfg(test)]
mod tests {
    use super::GoslingMode;

    #[test]
    fn default_mode_is_smart_approve() {
        assert_eq!(GoslingMode::default(), GoslingMode::SmartApprove);
    }
}
