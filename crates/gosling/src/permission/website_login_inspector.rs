use crate::config::GoslingMode;
use crate::conversation::message::{Message, ToolRequest};
use crate::tool_inspection::{InspectionAction, InspectionResult, ToolInspector};
use crate::website_logins;
use anyhow::Result;
use async_trait::async_trait;

/// Asks before a call that will receive a saved website password, naming the
/// login's site and username. Like other advisory prompts it follows the
/// session's approval mode, so Auto mode signs in without asking.
pub struct WebsiteLoginInspector;

#[async_trait]
impl ToolInspector for WebsiteLoginInspector {
    fn name(&self) -> &'static str {
        "website_login"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn inspect(
        &self,
        _session_id: &str,
        tool_requests: &[ToolRequest],
        _messages: &[Message],
        _gosling_mode: GoslingMode,
    ) -> Result<Vec<InspectionResult>> {
        let mut results = Vec::new();
        let mut logins = None;
        for request in tool_requests {
            let Ok(tool_call) = &request.tool_call else {
                continue;
            };
            let names = website_logins::referenced_names(tool_call.arguments.as_ref());
            if names.is_empty() {
                continue;
            }
            if logins.is_none() {
                logins = Some(website_logins::list()?);
            }
            let logins = logins.as_deref().unwrap_or_default();
            let described = names
                .iter()
                .map(|name| match website_logins::find_by_name(logins, name) {
                    Some(login) => {
                        format!("\"{}\" ({} as {})", login.name, login.url, login.username)
                    }
                    None => format!("\"{name}\" (no such saved login)"),
                })
                .collect::<Vec<_>>()
                .join(", ");
            let reason = format!(
                "`{}` will receive the saved password for {described}. Approve only if this call should sign in to that site.",
                tool_call.name
            );
            results.push(InspectionResult {
                tool_request_id: request.id.clone(),
                action: InspectionAction::RequireApproval(Some(reason.clone())),
                reason,
                confidence: 1.0,
                inspector_name: self.name().to_string(),
                finding_id: None,
                metadata: None,
            });
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::CallToolRequestParams;
    use serde_json::json;

    fn request(id: &str, arguments: serde_json::Value) -> ToolRequest {
        ToolRequest {
            id: id.to_string(),
            tool_call: Ok(CallToolRequestParams::new("browser__fill")
                .with_arguments(arguments.as_object().expect("object").clone())),
            metadata: None,
            tool_meta: None,
        }
    }

    #[tokio::test]
    async fn placeholder_calls_ask_with_the_login_named() {
        let inspector = WebsiteLoginInspector;
        let requests = [
            request("plain", json!({"value": "hello"})),
            request(
                "login",
                json!({"steps": [{"value": "{{login:Nonexistent Test Login}}"}]}),
            ),
        ];

        let results = inspector
            .inspect("session", &requests, &[], GoslingMode::SmartApprove)
            .await
            .unwrap();

        assert!(inspector.auto_downgrades_require_approval());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_request_id, "login");
        assert!(matches!(
            &results[0].action,
            InspectionAction::RequireApproval(Some(message))
                if message.contains("Nonexistent Test Login")
        ));
    }
}
