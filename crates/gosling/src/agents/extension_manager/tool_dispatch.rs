// Owns tool-owner resolution, MCP invocation, notifications, and App hydration.
// ExtensionManager exposes the same dispatch method and typed error behavior.
// The extension_manager compatibility facade preserves the manager type and paths.

use super::*;
use std::future::Future;
use tracing::Instrument;

impl ExtensionManager {
    async fn host_identity_for_owner(
        &self,
        extension_name: &str,
        actual_tool_name: &str,
    ) -> Option<crate::agents::interaction_policy::HostToolIdentity> {
        self.extensions
            .lock()
            .await
            .get(extension_name)
            .map(|extension| {
                crate::agents::interaction_policy::HostToolIdentity::from_extension_config(
                    &extension.config,
                    actual_tool_name,
                )
            })
    }

    pub(crate) async fn host_identity_for_catalog_tool(
        &self,
        tool: &Tool,
    ) -> Option<crate::agents::interaction_policy::HostToolIdentity> {
        let owner = get_tool_owner(tool)?;
        let actual_tool_name = tool
            .name
            .strip_prefix(&format!("{owner}__"))
            .unwrap_or(tool.name.as_ref());
        self.host_identity_for_owner(&owner, actual_tool_name).await
    }

    pub(crate) async fn resolve_tool(
        &self,
        session_id: &str,
        tool_name: &str,
    ) -> Result<ResolvedTool, ErrorData> {
        let tools = self.get_all_tools_cached(session_id).await.map_err(|e| {
            ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to get tools: {}", e),
                None,
            )
        })?;

        if let Some(tool) = tools.iter().find(|t| *t.name == *tool_name) {
            let owner = get_tool_owner(tool)
                .or_else(|| {
                    tool_name
                        .split_once("__")
                        .map(|(prefix, _)| name_to_key(prefix))
                })
                .ok_or_else(|| {
                    ErrorData::new(
                        ErrorCode::RESOURCE_NOT_FOUND,
                        format!("Tool '{}' has no owner", tool_name),
                        None,
                    )
                })?;

            let actual_tool_name = tool_name
                .strip_prefix(&format!("{owner}__"))
                .unwrap_or(tool_name)
                .to_string();

            let (client, host_identity, available) = self
                .extensions
                .lock()
                .await
                .get(&owner)
                .map(|extension| {
                    (
                        extension.get_client(),
                        crate::agents::interaction_policy::HostToolIdentity::from_extension_config(
                            &extension.config,
                            &actual_tool_name,
                        ),
                        extension.config.is_tool_available(&actual_tool_name),
                    )
                })
                .ok_or_else(|| {
                    ErrorData::new(
                        ErrorCode::RESOURCE_NOT_FOUND,
                        format!("Extension '{}' not found for tool '{}'", owner, tool_name),
                        None,
                    )
                })?;
            if !available {
                return Err(ErrorData::new(
                    ErrorCode::RESOURCE_NOT_FOUND,
                    format!(
                        "Tool '{}' is not available for extension '{}'",
                        actual_tool_name, owner
                    ),
                    None,
                ));
            }

            return Ok(ResolvedTool {
                tool_name: tool.name.to_string(),
                extension_name: owner,
                actual_tool_name,
                host_identity,
                client,
                tool_meta: get_tool_meta_value(tool),
                resource_uri: get_tool_resource_uri(tool),
            });
        }

        // Platform extensions advertise their tools unprefixed, but models
        // routinely address them by owner, as `code_execution__list_functions`.
        // Resolving that form against a live extension keeps those calls working
        // instead of failing a turn on a naming convention.
        if let Some((prefix, actual)) = tool_name.split_once("__") {
            let owner = name_to_key(prefix);
            if let Some((client, host_identity, available)) =
                self.extensions.lock().await.get(&owner).map(|extension| {
                    (
                        extension.get_client(),
                        crate::agents::interaction_policy::HostToolIdentity::from_extension_config(
                            &extension.config,
                            actual,
                        ),
                        extension.config.is_tool_available(actual),
                    )
                })
            {
                if !available {
                    return Err(ErrorData::new(
                        ErrorCode::RESOURCE_NOT_FOUND,
                        format!(
                            "Tool '{}' is not available for extension '{}'",
                            actual, owner
                        ),
                        None,
                    ));
                }
                return Ok(ResolvedTool {
                    tool_name: tool_name.to_string(),
                    extension_name: owner,
                    actual_tool_name: actual.to_string(),
                    host_identity,
                    client,
                    tool_meta: None,
                    resource_uri: None,
                });
            }
        }

        let available = tools
            .iter()
            .map(|t| t.name.as_ref())
            .collect::<Vec<&str>>()
            .join(", ");

        Err(ErrorData::new(
            ErrorCode::RESOURCE_NOT_FOUND,
            format!(
                "Tool '{}' not found. Available tools: [{}]",
                tool_name, available
            ),
            None,
        ))
    }

    pub(crate) async fn resolve_planning_tool_without_catalog(
        &self,
        tool_name: &str,
    ) -> Option<ResolvedTool> {
        let (_, extension_name, actual_tool_name) =
            crate::agents::interaction_policy::PlanningCapability::fixed_v1_capabilities().find(
                |(_, registered_extension, registered_tool)| {
                    *registered_tool == tool_name
                        || tool_name.strip_prefix(&format!("{registered_extension}__"))
                            == Some(*registered_tool)
                },
            )?;
        let extension_name = name_to_key(extension_name);
        let (client, host_identity, available) = self
            .extensions
            .lock()
            .await
            .get(&extension_name)
            .map(|extension| {
                (
                    extension.get_client(),
                    crate::agents::interaction_policy::HostToolIdentity::from_extension_config(
                        &extension.config,
                        actual_tool_name,
                    ),
                    extension.config.is_tool_available(actual_tool_name),
                )
            })?;
        available.then(|| ResolvedTool {
            tool_name: tool_name.to_string(),
            extension_name,
            actual_tool_name: actual_tool_name.to_string(),
            host_identity,
            client,
            tool_meta: None,
            resource_uri: None,
        })
    }

    pub async fn dispatch_tool_call(
        &self,
        ctx: &ToolCallContext,
        tool_call: CallToolRequestParams,
        cancellation_token: CancellationToken,
    ) -> Result<ToolCallResult> {
        let tool_name_str = tool_call.name.to_string();
        let interaction_policy = match &ctx.interaction_policy {
            Some(policy) => policy.clone(),
            None => self
                .context
                .session_manager
                .plans()
                .interaction_policy(&ctx.session_id)
                .await
                .map_err(|error| {
                    ErrorData::new(ErrorCode::INTERNAL_ERROR, error.to_string(), None)
                })?,
        };

        // Planning must never enumerate an external catalog merely to discover
        // that a direct, nested, or unknown call is forbidden. Resolve the
        // fixed host-owned capability set from the live host registry only;
        // full authorization below still revalidates durable plan state.
        let resolved = match &interaction_policy {
            crate::session::InteractionPolicy::Planning { .. } => {
                self.resolve_planning_tool_without_catalog(&tool_name_str)
                    .await
            }
            crate::session::InteractionPolicy::Normal => {
                Some(self.resolve_tool(&ctx.session_id, &tool_name_str).await?)
            }
        };
        crate::agents::interaction_policy::authorize_tool_execution(
            self.context.session_manager.as_ref(),
            None,
            &ctx.session_id,
            &interaction_policy,
            ctx.dispatch_origin,
            resolved.as_ref().map(|tool| &tool.host_identity),
            &tool_name_str,
        )
        .await?;

        let resolved = resolved.expect(
            "planning authorization cannot succeed without a fixed live host capability identity",
        );

        self.dispatch_authorized_tool_call(ctx, resolved, tool_call, cancellation_token)
            .await
    }

    pub(crate) async fn dispatch_authorized_tool_call(
        &self,
        ctx: &ToolCallContext,
        resolved: ResolvedTool,
        tool_call: CallToolRequestParams,
        cancellation_token: CancellationToken,
    ) -> Result<ToolCallResult> {
        let arguments = tool_call.arguments.clone();
        let client = resolved.client.clone();
        let hydration_client = client.clone();
        let notifications_receiver = client.subscribe().await;
        let session_id = ctx.session_id.clone();
        let action_required_tool_call_request_id = ctx.tool_call_request_id.clone();
        let action_required_receiver =
            if let Some(tool_call_request_id) = action_required_tool_call_request_id.clone() {
                if ActionRequiredManager::global()
                    .has_action_required_stream(&session_id, &tool_call_request_id)
                    .await
                {
                    None
                } else {
                    let registered_tool_call_request_id = tool_call_request_id.clone();
                    let receiver = ActionRequiredManager::global()
                        .register_action_required_stream(session_id.clone(), tool_call_request_id)
                        .await;
                    Some((
                        receiver,
                        session_id.clone(),
                        registered_tool_call_request_id,
                    ))
                }
            } else {
                None
            };
        let actual_tool_name = resolved.actual_tool_name.clone();
        let resolved_tool = resolved;
        let should_hydrate_mcp_app = self.host_supports_mcp_apps();
        let read_cancellation_token = cancellation_token.clone();
        let mut owned_ctx = ToolCallContext::new(
            ctx.session_id.clone(),
            ctx.working_dir.clone(),
            ctx.tool_call_request_id.clone(),
        );
        if let Some(operation_id) = &ctx.tool_operation_id {
            owned_ctx = owned_ctx.with_tool_operation_id(operation_id.clone());
        }
        if let Some(interaction_policy) = &ctx.interaction_policy {
            owned_ctx = owned_ctx.with_interaction_policy(interaction_policy.clone());
        }
        owned_ctx = owned_ctx.with_dispatch_origin(ctx.dispatch_origin);

        let fut = async move {
            tracing::debug!(
                "dispatch_tool_call: calling client.call_tool tool={} session_id={} working_dir={:?}",
                actual_tool_name,
                owned_ctx.session_id,
                owned_ctx.working_dir,
            );
            let call_result = client
                .call_tool(&owned_ctx, &actual_tool_name, arguments, cancellation_token)
                .await
                .map_err(|e| match e {
                    ServiceError::McpError(error_data) => error_data,
                    _ => {
                        ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), e.maybe_to_value())
                    }
                });

            let mut result = call_result?;

            remove_untrusted_mcp_app_meta(&mut result);

            if should_hydrate_mcp_app && result.is_error != Some(true) {
                if let Some(attachment) = Self::hydrate_mcp_app_attachment(
                    &hydration_client,
                    &session_id,
                    &resolved_tool,
                    read_cancellation_token,
                )
                .await
                {
                    insert_trusted_tool_update_meta(&mut result, &attachment);
                }
            }

            Ok(result)
        };

        Ok(ToolCallResult {
            result: Box::new(run_on_own_task(fut).boxed()),
            notification_stream: Some(Box::new(ReceiverStream::new(notifications_receiver))),
            action_required_stream: action_required_receiver.map(
                |(rx, session_id, tool_call_request_id)| {
                    Box::new(ActionRequiredStream::new(
                        rx,
                        session_id,
                        tool_call_request_id,
                    )) as _
                },
            ),
        })
    }
}

/// Runs a tool body on its own task.
///
/// The reply stream multiplexes a batch's tool futures with `select_all` on
/// its task and, between polls, persists sibling results through the session
/// store's write gate. A tool body polled inline that parks while holding that
/// gate (a delegated subagent creating its session, `todo` merging extension
/// state) is then never polled again once the stream waits for the gate, and
/// the whole session deadlocks on one task. The spawn is deferred to the first
/// poll so a tool still starts only once the batch's approval prompts are
/// answered, and the task is aborted when the caller drops the future so
/// cancellation behaves as before.
async fn run_on_own_task<F>(fut: F) -> Result<CallToolResult, ErrorData>
where
    F: Future<Output = Result<CallToolResult, ErrorData>> + Send + 'static,
{
    let session_id = crate::session_context::current_session_id();
    let task = AbortOnDrop(tokio::spawn(
        crate::session_context::with_session_id(session_id, fut)
            .instrument(tracing::Span::current()),
    ));
    match task.await {
        Ok(result) => result,
        Err(join_error) if join_error.is_panic() => Err(ErrorData::new(
            ErrorCode::INTERNAL_ERROR,
            format!("tool task panicked: {join_error}"),
            None,
        )),
        Err(_) => Err(ErrorData::new(
            ErrorCode::INTERNAL_ERROR,
            "tool task was cancelled before it produced a result".to_string(),
            None,
        )),
    }
}

struct AbortOnDrop<T>(tokio::task::JoinHandle<T>);

impl<T> Future for AbortOnDrop<T> {
    type Output = Result<T, tokio::task::JoinError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.0).poll(cx)
    }
}

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}
