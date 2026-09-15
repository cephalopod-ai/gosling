//! ACP admission and selected-extension dispatch for the core Recall Brief.

use super::*;
use crate::agents::extension_manager::get_tool_owner;
use crate::config::extensions::name_to_key;
use crate::recall_brief::{normalize_recall_result, synthesize_brief, unavailable_brief};
use gosling_sdk_types::recall_brief::{
    RecallArtifactKind, RecallBriefRequest, RecallBriefResponse, RecallSelectors,
};
use rmcp::model::CallToolRequestParams;
use serde_json::{Map, Value};
use std::time::Duration;

const MCP_RECALL_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_FACETS: usize = 5;

struct RecallCancellationGuard(CancellationToken);

impl Drop for RecallCancellationGuard {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

impl GoslingAcpAgent {
    pub(super) async fn on_recall_brief(
        &self,
        request: RecallBriefRequest,
    ) -> Result<RecallBriefResponse, agent_client_protocol::Error> {
        let arguments = recall_arguments(&request)?;
        self.require_normal_app_action_policy(&request.session_id, "recall_brief")
            .await?;
        let session = self
            .session_manager
            .get_session(&request.session_id, false)
            .await
            .map_err(|_| {
                agent_client_protocol::Error::resource_not_found(Some(request.session_id.clone()))
            })?;
        let extension_key = name_to_key(&request.extension_name);
        let enrolled = EnabledExtensionsState::extensions_or_default(
            Some(&session.extension_data),
            crate::config::Config::global(),
        )
        .into_iter()
        .any(|extension| {
            extension.key() == extension_key
                && matches!(
                    extension,
                    ExtensionConfig::Stdio { .. } | ExtensionConfig::StreamableHttp { .. }
                )
        });
        if !enrolled {
            return Ok(unavailable_brief(
                "unavailable",
                "unavailable",
                "selected_extension_not_enrolled",
            ));
        }
        let agent = self.get_session_agent(&request.session_id).await?;
        let provider = match agent.provider().await {
            Ok(provider) if !provider.executes_tools_outside_gosling() => provider,
            _ => {
                return Ok(unavailable_brief(
                    "unavailable",
                    "unavailable",
                    "session_provider_has_no_isolated_no_tools_completion",
                ));
            }
        };
        let model_config = match agent.model_config_for_session(&request.session_id).await {
            Ok(config) => config,
            Err(_) => {
                return Ok(unavailable_brief(
                    provider.get_name(),
                    "unavailable",
                    "session_model_unavailable",
                ));
            }
        };
        let operation_gate = self.session_operation_gate(&request.session_id).await?;
        let _guard = operation_gate
            .begin_prompt(&format!("recall_brief_{}", Uuid::new_v4().simple()))
            .await?;
        self.require_normal_app_action_policy(&request.session_id, "recall_brief")
            .await?;
        let tool_name = format!("{extension_key}__muninn_recall");
        let tools = match agent
            .list_tools(&request.session_id, Some(request.extension_name.clone()))
            .await
        {
            Ok(tools) => tools,
            Err(_) => {
                return Ok(unavailable_brief(
                    provider.get_name(),
                    &model_config.model_name,
                    "selected_extension_tool_catalog_unavailable",
                ));
            }
        };
        if !tools.iter().any(|tool| {
            tool.name.as_ref() == tool_name
                && get_tool_owner(tool).as_deref() == Some(extension_key.as_str())
        }) {
            return Ok(unavailable_brief(
                provider.get_name(),
                &model_config.model_name,
                "selected_extension_does_not_advertise_muninn_recall",
            ));
        }
        let tool_call = CallToolRequestParams::new(tool_name).with_arguments(arguments);
        let cancel_token = CancellationToken::new();
        let _cancellation_guard = RecallCancellationGuard(cancel_token.clone());
        let result = tokio::time::timeout(MCP_RECALL_TIMEOUT, async {
            let dispatched = agent
                .dispatch_app_tool_call(&request.session_id, tool_call, cancel_token.clone())
                .await?;
            dispatched.result.await
        })
        .await;
        let tool_result = match result {
            Ok(Ok(result)) => result,
            _ => {
                return Ok(unavailable_brief(
                    provider.get_name(),
                    &model_config.model_name,
                    "muninn_recall_unavailable_or_not_permitted",
                ));
            }
        };
        let bundle = match normalize_recall_result(&tool_result) {
            Ok(bundle) => bundle,
            Err(_) => {
                return Ok(unavailable_brief(
                    provider.get_name(),
                    &model_config.model_name,
                    "muninn_recall_result_unsupported",
                ));
            }
        };
        self.require_normal_app_action_policy(&request.session_id, "recall_brief_model")
            .await?;
        Ok(synthesize_brief(
            &request.session_id,
            bundle,
            provider.as_ref(),
            &model_config,
        )
        .await)
    }
}

fn recall_arguments(
    request: &RecallBriefRequest,
) -> Result<Map<String, Value>, agent_client_protocol::Error> {
    if !bounded_text(&request.session_id, 512)
        || !bounded_text(&request.extension_name, 128)
        || !bounded_text(&request.query, 2_000)
    {
        return Err(invalid_recall_request(
            "session, extension, or query is invalid",
        ));
    }
    let mut arguments =
        Map::from_iter([("query".to_string(), Value::String(request.query.clone()))]);
    if let Some(facets) = &request.facets {
        if facets.is_empty()
            || facets.len() > MAX_FACETS
            || facets.iter().any(|facet| !bounded_text(facet, 2_000))
        {
            return Err(invalid_recall_request("facets are invalid"));
        }
        arguments.insert("facets".to_string(), serde_json::to_value(facets).unwrap());
    }
    if let Some(cursor) = &request.cursor {
        if !bounded_text(cursor, 2_048) {
            return Err(invalid_recall_request("cursor is invalid"));
        }
        arguments.insert("cursor".to_string(), Value::String(cursor.clone()));
    }
    if let Some(selectors) = &request.selectors {
        insert_selectors(&mut arguments, selectors)?;
    }
    Ok(arguments)
}

fn insert_selectors(
    arguments: &mut Map<String, Value>,
    selectors: &RecallSelectors,
) -> Result<(), agent_client_protocol::Error> {
    if let Some(kind) = selectors.artifact_kind {
        let value = match kind {
            RecallArtifactKind::Chat => "chat",
            RecallArtifactKind::Code => "code",
        };
        arguments.insert(
            "artifact_kind".to_string(),
            Value::String(value.to_string()),
        );
    }
    for (key, value, maximum) in [
        ("conversation_id", &selectors.conversation_id, 512),
        ("repository", &selectors.repository, 128),
        ("repository_id", &selectors.repository_id, 128),
        (
            "repository_path_prefix",
            &selectors.repository_path_prefix,
            2_048,
        ),
        ("store_id", &selectors.store_id, 64),
    ] {
        if let Some(value) = value {
            if !bounded_text(value, maximum) {
                return Err(invalid_recall_request("selector is invalid"));
            }
            arguments.insert(key.to_string(), Value::String(value.clone()));
        }
    }
    if let Some(path) = selectors.repository_path_prefix.as_deref() {
        if !relative_selector_path(path) {
            return Err(invalid_recall_request("repository path prefix is invalid"));
        }
    }
    if let Some(context) = &selectors.retrieval_context {
        if !bounded_text(&context.active_repository, 128)
            || context
                .active_path_prefix
                .as_deref()
                .is_some_and(|path| !bounded_text(path, 2_048) || !relative_selector_path(path))
        {
            return Err(invalid_recall_request("retrieval context is invalid"));
        }
        let mut value = Map::from_iter([(
            "active_repository".to_string(),
            Value::String(context.active_repository.clone()),
        )]);
        if let Some(path) = &context.active_path_prefix {
            value.insert(
                "active_path_prefix".to_string(),
                Value::String(path.clone()),
            );
        }
        arguments.insert("retrieval_context".to_string(), Value::Object(value));
    }
    for (key, value) in [("since", &selectors.since), ("until", &selectors.until)] {
        if let Some(value) = value {
            if !bounded_text(value, 64) || chrono::DateTime::parse_from_rfc3339(value).is_err() {
                return Err(invalid_recall_request("recorded-time selector is invalid"));
            }
            arguments.insert(key.to_string(), Value::String(value.clone()));
        }
    }
    Ok(())
}

fn bounded_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= maximum * 4
        && value.chars().count() <= maximum
        && !value.chars().any(|character| character == '\0')
}

fn relative_selector_path(value: &str) -> bool {
    !value.starts_with('/')
        && !value.contains(['\\', '\n', '\r'])
        && value
            .trim_end_matches('/')
            .split('/')
            .all(|segment| !matches!(segment, "" | "." | ".."))
}

fn invalid_recall_request(message: &str) -> agent_client_protocol::Error {
    agent_client_protocol::Error::invalid_params().data(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn omitted_selectors_stay_omitted_in_muninn_arguments() {
        let request = RecallBriefRequest {
            session_id: "s1".to_string(),
            extension_name: "muninn".to_string(),
            query: "Santa belief".to_string(),
            facets: None,
            cursor: None,
            selectors: None,
        };
        assert_eq!(
            recall_arguments(&request)
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["query"]
        );
    }

    #[test]
    fn explicit_selectors_use_muninn_field_names_without_scope_widening() {
        let request = RecallBriefRequest {
            session_id: "s1".to_string(),
            extension_name: "muninn".to_string(),
            query: "decision".to_string(),
            facets: Some(vec!["earlier choice".to_string()]),
            cursor: None,
            selectors: Some(RecallSelectors {
                artifact_kind: Some(RecallArtifactKind::Chat),
                conversation_id: Some("thread-1".to_string()),
                ..Default::default()
            }),
        };
        let arguments = recall_arguments(&request).unwrap();
        assert_eq!(
            arguments.get("artifact_kind"),
            Some(&Value::String("chat".to_string()))
        );
        assert_eq!(
            arguments.get("conversation_id"),
            Some(&Value::String("thread-1".to_string()))
        );
        assert!(!arguments.contains_key("store_id"));
        assert!(!arguments.contains_key("repository"));
    }

    #[test]
    fn traversal_and_empty_facets_are_refused_before_a_tool_call() {
        let request = RecallBriefRequest {
            session_id: "s1".to_string(),
            extension_name: "muninn".to_string(),
            query: "decision".to_string(),
            facets: Some(Vec::new()),
            cursor: None,
            selectors: None,
        };
        assert!(recall_arguments(&request).is_err());
        let request = RecallBriefRequest {
            facets: None,
            selectors: Some(RecallSelectors {
                repository_path_prefix: Some("src/../private".to_string()),
                ..Default::default()
            }),
            ..request
        };
        assert!(recall_arguments(&request).is_err());
    }
}
