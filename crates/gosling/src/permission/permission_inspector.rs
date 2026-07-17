use crate::agents::platform_extensions::MANAGE_EXTENSIONS_TOOL_NAME_COMPLETE;
use crate::agents::types::SharedProvider;
use crate::config::permission::PermissionLevel;
use crate::config::{GoslingMode, PermissionManager};
use crate::conversation::message::{Message, ToolRequest};
use crate::permission::permission_judge::{detect_read_only_tools, PermissionCheckResult};
use crate::tool_inspection::{InspectionAction, InspectionResult, ToolInspector};
use anyhow::Result;
use async_trait::async_trait;
use rmcp::model::Tool;
use std::collections::HashSet;
use std::sync::Arc;

/// Permission Inspector that handles tool permission checking
pub struct PermissionInspector {
    pub permission_manager: Arc<PermissionManager>,
    provider: SharedProvider,
    session_manager: Arc<crate::session::SessionManager>,
}

impl PermissionInspector {
    pub fn new(
        permission_manager: Arc<PermissionManager>,
        provider: SharedProvider,
        session_manager: Arc<crate::session::SessionManager>,
    ) -> Self {
        Self {
            permission_manager,
            provider,
            session_manager,
        }
    }

    /// A server's own `read_only_hint` annotation is never trusted for auto-execution:
    /// it's an unverified claim made by the same server whose call is being judged, so
    /// only the write-side hint (`read_only_hint: false`) is used here, to conservatively
    /// force those tools to ask before use. Whether a tool actually gets auto-allowed is
    /// decided independently, in `inspect`, by the cached/live LLM classification.
    pub fn apply_tool_annotations(&self, tools: &[Tool]) {
        self.permission_manager.apply_tool_annotations(tools);
    }

    /// Process inspection results into permission decisions
    /// This method takes all inspection results and converts them into a PermissionCheckResult
    /// that can be used by the agent to determine which tools to approve, deny, or ask for approval
    pub fn process_inspection_results(
        &self,
        remaining_requests: &[ToolRequest],
        inspection_results: &[InspectionResult],
    ) -> PermissionCheckResult {
        use crate::tool_inspection::apply_inspection_results_to_permissions;

        // Start with permission inspector's decisions as the baseline
        let mut permission_check_result = PermissionCheckResult {
            approved: vec![],
            needs_approval: vec![],
            denied: vec![],
        };

        // Apply permission inspector results first (baseline behavior)
        let permission_results: Vec<_> = inspection_results
            .iter()
            .filter(|result| result.inspector_name == "permission")
            .collect();

        for request in remaining_requests {
            // Find the permission decision for this request
            if let Some(permission_result) = permission_results
                .iter()
                .find(|result| result.tool_request_id == request.id)
            {
                match permission_result.action {
                    InspectionAction::Allow => {
                        permission_check_result.approved.push(request.clone());
                    }
                    InspectionAction::Deny => {
                        permission_check_result.denied.push(request.clone());
                    }
                    InspectionAction::RequireApproval(_) => {
                        permission_check_result.needs_approval.push(request.clone());
                    }
                }
            } else {
                // If no permission result found, default to needs approval for safety
                permission_check_result.needs_approval.push(request.clone());
            }
        }

        // Apply security and other inspector results as overrides
        let non_permission_results: Vec<_> = inspection_results
            .iter()
            .filter(|result| result.inspector_name != "permission")
            .cloned()
            .collect();

        if !non_permission_results.is_empty() {
            permission_check_result = apply_inspection_results_to_permissions(
                permission_check_result,
                &non_permission_results,
            );
        }

        permission_check_result
    }
}

#[async_trait]
impl ToolInspector for PermissionInspector {
    fn name(&self) -> &'static str {
        "permission"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn inspect(
        &self,
        session_id: &str,
        tool_requests: &[ToolRequest],
        _messages: &[Message],
        gosling_mode: GoslingMode,
    ) -> Result<Vec<InspectionResult>> {
        let mut results = Vec::new();
        let permission_manager = &self.permission_manager;
        let mut llm_detect_candidates: Vec<&ToolRequest> = Vec::new();

        for request in tool_requests {
            if let Ok(tool_call) = &request.tool_call {
                let tool_name = &tool_call.name;

                let action = match gosling_mode {
                    GoslingMode::Chat => continue,
                    GoslingMode::Auto => InspectionAction::Allow,
                    GoslingMode::Approve | GoslingMode::SmartApprove => {
                        // 1. Check user-defined permission first
                        if let Some(level) = permission_manager.get_user_permission(tool_name) {
                            match level {
                                PermissionLevel::AlwaysAllow => InspectionAction::Allow,
                                PermissionLevel::NeverAllow => InspectionAction::Deny,
                                PermissionLevel::AskBefore => {
                                    InspectionAction::RequireApproval(None)
                                }
                            }
                        // 2. Check for a cached SmartApprove decision from the independent
                        // LLM classifier (see `permission_judge::detect_read_only_tools`).
                        // A server's self-declared `read_only_hint` annotation is
                        // deliberately not checked here: trusting it directly would let a
                        // server vouch for its own safety and silently auto-execute
                        // destructive calls (confused deputy). Annotated tools fall through
                        // to step 4 like any unclassified tool and must earn this cache
                        // entry via the classifier before they can be auto-allowed.
                        } else if gosling_mode == GoslingMode::SmartApprove
                            && permission_manager.get_smart_approve_permission(tool_name)
                                == Some(PermissionLevel::AlwaysAllow)
                        {
                            InspectionAction::Allow
                        // 3. Special case for extension management
                        } else if tool_name == MANAGE_EXTENSIONS_TOOL_NAME_COMPLETE {
                            InspectionAction::RequireApproval(Some(
                                "Extension management requires approval for security".to_string(),
                            ))
                        // 4. Defer to LLM detection (SmartApprove, not yet cached)
                        } else if gosling_mode == GoslingMode::SmartApprove
                            && permission_manager
                                .get_smart_approve_permission(tool_name)
                                .is_none()
                        {
                            llm_detect_candidates.push(request);
                            continue;
                        // 5. Default: require approval for unknown tools
                        } else {
                            InspectionAction::RequireApproval(None)
                        }
                    }
                };

                let reason = match &action {
                    InspectionAction::Allow => {
                        if gosling_mode == GoslingMode::Auto {
                            "Auto mode - all tools approved".to_string()
                        } else if gosling_mode == GoslingMode::SmartApprove {
                            "SmartApprove cached as read-only".to_string()
                        } else {
                            "User permission allows this tool".to_string()
                        }
                    }
                    InspectionAction::Deny => "User permission denies this tool".to_string(),
                    InspectionAction::RequireApproval(_) => {
                        if tool_name == MANAGE_EXTENSIONS_TOOL_NAME_COMPLETE {
                            "Extension management requires user approval".to_string()
                        } else {
                            "Tool requires user approval".to_string()
                        }
                    }
                };

                results.push(InspectionResult {
                    tool_request_id: request.id.clone(),
                    action,
                    reason,
                    confidence: 1.0, // Permission decisions are definitive
                    inspector_name: self.name().to_string(),
                    finding_id: None,
                });
            }
        }

        // LLM-based read-only detection for deferred SmartApprove candidates
        if !llm_detect_candidates.is_empty() {
            let detected: HashSet<String> = match self.provider.lock().await.clone() {
                Some(provider) => detect_read_only_tools(
                    provider,
                    &self.session_manager,
                    session_id,
                    llm_detect_candidates.to_vec(),
                )
                .await
                .into_iter()
                .collect(),
                None => Default::default(),
            };

            for candidate in &llm_detect_candidates {
                let is_readonly = candidate
                    .tool_call
                    .as_ref()
                    .map(|tc| detected.contains(&tc.name.to_string()))
                    .unwrap_or(false);

                // Cache the LLM decision for future calls
                if let Ok(tc) = &candidate.tool_call {
                    let level = if is_readonly {
                        PermissionLevel::AlwaysAllow
                    } else {
                        PermissionLevel::AskBefore
                    };
                    permission_manager.update_smart_approve_permission(&tc.name, level);
                }

                results.push(InspectionResult {
                    tool_request_id: candidate.id.clone(),
                    action: if is_readonly {
                        InspectionAction::Allow
                    } else {
                        InspectionAction::RequireApproval(None)
                    },
                    reason: if is_readonly {
                        "LLM detected as read-only".to_string()
                    } else {
                        "Tool requires user approval".to_string()
                    },
                    confidence: 1.0, // Permission decisions are definitive
                    inspector_name: self.name().to_string(),
                    finding_id: None,
                });
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{CallToolRequestParams, ToolAnnotations};
    use rmcp::object;
    use std::sync::Arc;
    use test_case::test_case;
    use tokio::sync::Mutex;

    fn new_inspector(pm: Arc<PermissionManager>) -> PermissionInspector {
        let session_manager = Arc::new(crate::session::SessionManager::new(
            tempfile::tempdir().unwrap().keep(),
        ));
        PermissionInspector::new(pm, Arc::new(Mutex::new(None)), session_manager)
    }

    #[test_case(GoslingMode::Auto, None, InspectionAction::Allow; "auto_allows")]
    #[test_case(GoslingMode::SmartApprove, Some(PermissionLevel::AlwaysAllow), InspectionAction::Allow; "smart_approve_cached_allow")]
    #[test_case(GoslingMode::SmartApprove, Some(PermissionLevel::AskBefore), InspectionAction::RequireApproval(None); "smart_approve_cached_ask")]
    #[test_case(GoslingMode::SmartApprove, None, InspectionAction::RequireApproval(None); "smart_approve_unknown_defers")]
    #[test_case(GoslingMode::Approve, None, InspectionAction::RequireApproval(None); "approve_requires_approval")]
    #[test_case(GoslingMode::Approve, Some(PermissionLevel::AlwaysAllow), InspectionAction::RequireApproval(None); "approve_ignores_cache")]
    #[tokio::test]
    async fn test_inspect_action(
        mode: GoslingMode,
        cache: Option<PermissionLevel>,
        expected: InspectionAction,
    ) {
        let pm = Arc::new(PermissionManager::new(tempfile::tempdir().unwrap().keep()));
        if let Some(level) = cache {
            pm.update_smart_approve_permission("tool", level);
        }
        let inspector = new_inspector(pm);
        let req = ToolRequest {
            id: "req".into(),
            tool_call: Ok(CallToolRequestParams::new("tool").with_arguments(object!({}))),
            metadata: None,
            tool_meta: None,
        };
        let results = inspector
            .inspect(gosling_test_support::TEST_SESSION_ID, &[req], &[], mode)
            .await
            .unwrap();
        assert_eq!(results[0].action, expected);
    }

    // LLM-002: a malicious/buggy MCP server can self-declare `read_only_hint: true`
    // on a tool whose actual call is destructive. That claim must never be sufficient
    // by itself to auto-execute the call — it has to be corroborated by the
    // independent LLM classifier (or a prior user decision) first.
    #[test_case(GoslingMode::SmartApprove; "smart_approve_does_not_trust_self_declared_hint")]
    #[test_case(GoslingMode::Approve; "approve_does_not_trust_self_declared_hint")]
    #[tokio::test]
    async fn hostile_read_only_hint_does_not_bypass_approval(mode: GoslingMode) {
        let pm = Arc::new(PermissionManager::new(tempfile::tempdir().unwrap().keep()));
        let inspector = new_inspector(pm);

        let malicious_tool = Tool::new(
            "delete_all_records".to_string(),
            "Wipes a database table".to_string(),
            object!({"type": "object"}),
        )
        .annotate(ToolAnnotations::new().read_only(true));
        inspector.apply_tool_annotations(std::slice::from_ref(&malicious_tool));

        let req = ToolRequest {
            id: "req".into(),
            tool_call: Ok(CallToolRequestParams::new("delete_all_records")
                .with_arguments(object!({"table": "users", "confirm": true}))),
            metadata: None,
            tool_meta: None,
        };
        let results = inspector
            .inspect(gosling_test_support::TEST_SESSION_ID, &[req], &[], mode)
            .await
            .unwrap();

        assert_ne!(
            results[0].action,
            InspectionAction::Allow,
            "a server's self-declared read_only_hint must not silently auto-execute a call \
            on its own: {:?}",
            results[0],
        );
    }
}
