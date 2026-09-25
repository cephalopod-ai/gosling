//! Durable tool inspection, dispatch, hook, and terminal-result recording.
//!
//! Maintainers: preserve begin/replay/in-doubt/complete ordering and hook fences here.
//! Clients: tool request ids, errors, and result streams remain stable.

use super::*;

impl Agent {
    pub async fn dispatch_app_tool_call(
        &self,
        session_id: &str,
        tool_call: CallToolRequestParams,
        cancellation_token: CancellationToken,
    ) -> Result<ToolCallResult, ErrorData> {
        let interaction_policy = self
            .config
            .session_manager
            .plans()
            .interaction_policy(session_id)
            .await
            .map_err(|error| ErrorData::new(ErrorCode::INTERNAL_ERROR, error.to_string(), None))?;
        crate::agents::interaction_policy::authorize_tool_execution(
            self.config.session_manager.as_ref(),
            Some(self.config.permission_manager.as_ref()),
            session_id,
            &interaction_policy,
            crate::agents::interaction_policy::DispatchOrigin::AppDirect,
            None,
            tool_call.name.as_ref(),
        )
        .await?;
        let request_id = format!("app_tool_{}", Uuid::new_v4().simple());
        let request = ToolRequest {
            id: request_id.clone(),
            tool_call: Ok(tool_call.clone()),
            metadata: None,
            tool_meta: None,
        };
        let requests = vec![request];
        let gosling_mode = self.gosling_mode().await;
        let inspection_results = self
            .tool_inspection_manager
            .inspect_tools(session_id, &requests, &[], gosling_mode)
            .await
            .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;

        let permission_result = self
            .tool_inspection_manager
            .process_inspection_results_with_permission_inspector(&requests, &inspection_results)
            .ok_or_else(|| {
                ErrorData::new(
                    ErrorCode::INTERNAL_ERROR,
                    "Tool permission inspector is unavailable".to_string(),
                    None,
                )
            })?;

        if let Some(denied) = permission_result.denied.first() {
            let tool_name = denied
                .tool_call
                .as_ref()
                .map(|call| call.name.to_string())
                .unwrap_or_else(|_| "tool".to_string());
            return Err(ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("Tool `{tool_name}` is denied by current permissions"),
                None,
            ));
        }

        if let Some(needs_approval) = permission_result.needs_approval.first() {
            let tool_name = needs_approval
                .tool_call
                .as_ref()
                .map(|call| call.name.to_string())
                .unwrap_or_else(|_| "tool".to_string());
            return Err(ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                format!("Tool `{tool_name}` requires approval before app clients can call it"),
                None,
            ));
        }

        if permission_result.approved.is_empty() {
            return Err(ErrorData::new(
                ErrorCode::INVALID_REQUEST,
                "Tool call was not approved by current permissions".to_string(),
                None,
            ));
        }

        let session = self
            .config
            .session_manager
            .get_session(session_id, false)
            .await
            .map_err(|e| ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None))?;
        let (_, result) = self
            .dispatch_tool_call_scoped(
                tool_call,
                request_id,
                Some(cancellation_token),
                &session,
                false,
                &interaction_policy,
                crate::agents::interaction_policy::DispatchOrigin::AppDirect,
                false,
            )
            .await;
        result
    }

    /// Dispatch a single tool call to the appropriate client
    #[instrument(skip(self, tool_call, request_id, cancellation_token, session), fields(input, output, session.id = %session.id))]
    pub async fn dispatch_tool_call(
        &self,
        tool_call: CallToolRequestParams,
        request_id: String,
        cancellation_token: Option<CancellationToken>,
        session: &Session,
    ) -> (String, Result<ToolCallResult, ErrorData>) {
        let interaction_policy = match self
            .config
            .session_manager
            .plans()
            .interaction_policy(&session.id)
            .await
        {
            Ok(policy) => policy,
            Err(error) => {
                return (
                    request_id,
                    Err(ErrorData::new(
                        ErrorCode::INTERNAL_ERROR,
                        error.to_string(),
                        None,
                    )),
                );
            }
        };
        self.dispatch_tool_call_scoped(
            tool_call,
            request_id,
            cancellation_token,
            session,
            false,
            &interaction_policy,
            crate::agents::interaction_policy::DispatchOrigin::AgentDirect,
            false,
        )
        .await
    }

    /// Dispatches a model-proposed call that host policy (mode, saved grants,
    /// inspectors) approved without asking the user.
    pub(crate) async fn dispatch_conversation_tool_call(
        &self,
        tool_call: CallToolRequestParams,
        request_id: String,
        cancellation_token: Option<CancellationToken>,
        session: &Session,
        interaction_policy: &crate::session::InteractionPolicy,
    ) -> (String, Result<ToolCallResult, ErrorData>) {
        self.dispatch_tool_call_scoped(
            tool_call,
            request_id,
            cancellation_token,
            session,
            true,
            interaction_policy,
            crate::agents::interaction_policy::DispatchOrigin::ModelNative,
            false,
        )
        .await
    }

    /// Dispatches a model-proposed call the user approved through the client
    /// confirmation for this exact request id. The ledger binds that id to the
    /// checkpointed payload, so the approval cannot be reused for other
    /// arguments.
    pub(crate) async fn dispatch_user_confirmed_conversation_tool_call(
        &self,
        tool_call: CallToolRequestParams,
        request_id: String,
        cancellation_token: Option<CancellationToken>,
        session: &Session,
        interaction_policy: &crate::session::InteractionPolicy,
    ) -> (String, Result<ToolCallResult, ErrorData>) {
        self.dispatch_tool_call_scoped(
            tool_call,
            request_id,
            cancellation_token,
            session,
            true,
            interaction_policy,
            crate::agents::interaction_policy::DispatchOrigin::ModelNative,
            true,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn dispatch_tool_call_scoped(
        &self,
        tool_call: CallToolRequestParams,
        request_id: String,
        cancellation_token: Option<CancellationToken>,
        session: &Session,
        conversation_bound: bool,
        interaction_policy: &crate::session::InteractionPolicy,
        dispatch_origin: crate::agents::interaction_policy::DispatchOrigin,
        user_confirmed: bool,
    ) -> (String, Result<ToolCallResult, ErrorData>) {
        let tool_name = tool_call.name.to_string();
        let is_frontend = self.is_frontend_tool(&tool_call.name).await;
        let mut resolution_error = None;
        let planning = matches!(
            interaction_policy,
            crate::session::InteractionPolicy::Planning { .. }
        );
        let resolved_tool = if is_frontend {
            None
        } else if planning {
            if matches!(
                dispatch_origin,
                crate::agents::interaction_policy::DispatchOrigin::ModelNative
            ) {
                self.extension_manager
                    .resolve_planning_tool_without_catalog(&tool_name)
                    .await
            } else {
                None
            }
        } else {
            match self
                .extension_manager
                .resolve_tool(&session.id, &tool_name)
                .await
            {
                Ok(identity) => Some(identity),
                Err(error) => {
                    resolution_error = Some(error);
                    None
                }
            }
        };
        let authorization = match crate::agents::interaction_policy::authorize_tool_execution(
            self.config.session_manager.as_ref(),
            Some(self.config.permission_manager.as_ref()),
            &session.id,
            interaction_policy,
            dispatch_origin,
            resolved_tool
                .as_ref()
                .map(|resolved| &resolved.host_identity),
            &tool_name,
        )
        .await
        {
            Ok(authorization) => authorization,
            Err(error) => return (request_id, Err(error)),
        };

        let operation_id = match self
            .config
            .session_manager
            .authorize_and_begin_tool_operation(
                &session.id,
                &request_id,
                &tool_call,
                conversation_bound,
                interaction_policy,
                matches!(
                    authorization,
                    crate::agents::interaction_policy::ExecutionAuthorization::Planning(_)
                ),
                crate::session::SkillScopeGate::Evaluate {
                    verified_non_mutating: resolved_tool.as_ref().is_some_and(|resolved| {
                        crate::agents::interaction_policy::is_verified_non_mutating(
                            &resolved.host_identity,
                        )
                    }),
                    user_approved: user_confirmed,
                },
            )
            .await
        {
            Ok(ToolOperationStart::Execute { operation_id }) => operation_id,
            Ok(ToolOperationStart::Replay { result, .. }) => {
                return (request_id, Ok(ToolCallResult::from(result)));
            }
            Ok(ToolOperationStart::InDoubt { operation_id }) => {
                return (
                    request_id,
                    Err(ErrorData::new(
                        ErrorCode::INTERNAL_ERROR,
                        "Tool execution was already durably started and its status is in doubt; Gosling will not dispatch it again automatically.".to_string(),
                        Some(serde_json::json!({
                            "tool_operation_id": operation_id,
                            "status": "in_doubt",
                            "retryable": false
                        })),
                    )),
                );
            }
            Err(error) => {
                if let Some(denied) =
                    error.downcast_ref::<crate::skills::admission::SkillCeilingDenied>()
                {
                    return (
                        request_id,
                        Err(crate::skills::admission::skill_ceiling_denial(
                            &tool_name, denied,
                        )),
                    );
                }
                if matches!(
                    interaction_policy,
                    crate::session::InteractionPolicy::Planning { .. }
                ) || error.to_string().contains("normal policy is stale")
                {
                    return (
                        request_id,
                        Err(crate::agents::interaction_policy::atomic_policy_denial(
                            &tool_name, error,
                        )),
                    );
                }
                return (
                    request_id,
                    Err(ErrorData::new(
                        ErrorCode::INTERNAL_ERROR,
                        format!("Could not durably begin tool operation: {error}"),
                        None,
                    )),
                );
            }
        };
        let mut operation_guard =
            ToolOperationGuard::new(self.config.session_manager.clone(), operation_id.clone());
        if crate::providers::utils::local_transcript_persistence_enabled() {
            let input_summary = serde_json::json!({
                "tool": tool_call.name,
                "arguments": tool_call.arguments,
            });
            tracing::Span::current().record("input", tracing::field::display(&input_summary));
        }

        if !planning
            && self
                .hook_manager
                .has_hooks(crate::hooks::HookEvent::PreToolUse)
        {
            let ctx =
                crate::hooks::HookContext::new(crate::hooks::HookEvent::PreToolUse, &session.id)
                    .with_tool(
                        tool_call.name.to_string(),
                        tool_call
                            .arguments
                            .as_ref()
                            .map(|a| serde_json::Value::Object(a.clone())),
                    )
                    .with_working_dir(session.working_dir.to_string_lossy().to_string());
            if let crate::hooks::HookDecision::Deny { reason, plugin } = self
                .hook_manager
                .emit_blocking(crate::hooks::HookEvent::PreToolUse, ctx)
                .await
            {
                let denial = ErrorData::new(
                    ErrorCode::INTERNAL_ERROR,
                    format!(
                        "Tool call denied by policy hook `{plugin}`: {reason}. \
                         Do not retry; this is a policy denial, not a transient failure."
                    ),
                    None,
                );
                if let Err(error) = self
                    .config
                    .session_manager
                    .complete_tool_operation(&operation_id, &Err(denial.clone()))
                    .await
                {
                    return (
                        request_id,
                        Err(ErrorData::new(
                            ErrorCode::INTERNAL_ERROR,
                            format!("Could not durably complete denied tool operation: {error}"),
                            None,
                        )),
                    );
                }
                operation_guard.disarm();
                return (request_id, Err(denial));
            }
        }

        let tool_input_for_extended = tool_call
            .arguments
            .as_ref()
            .map(|a| serde_json::Value::Object(a.clone()));
        if !planning {
            self.subdirectory_hint_tracker
                .lock()
                .await
                .record_tool_arguments(&tool_call.arguments, &session.working_dir);
            self.emit_pre_tool_extended_hooks(
                &tool_call.name,
                tool_input_for_extended.as_ref(),
                session,
            )
            .await;
        }

        let output_capture = self
            .config
            .session_manager
            .prepare_output_capture(session, &tool_call, &request_id)
            .await;

        let ctx = crate::agents::tool_execution::ToolCallContext::new(
            session.id.clone(),
            Some(session.working_dir.clone()),
            Some(request_id.clone()),
        )
        .with_tool_operation_id(operation_id.clone())
        .with_interaction_policy(interaction_policy.clone())
        .with_dispatch_origin(dispatch_origin);

        debug!("WAITING_TOOL_START: {}", tool_call.name);
        let result: ToolCallResult = if is_frontend {
            ToolCallResult::from(Err(ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                "Frontend tool execution required".to_string(),
                None,
            )))
        } else if let Some(error) = resolution_error {
            ToolCallResult::from(Err(error))
        } else {
            let mut dispatched_call = tool_call.clone();
            match insert_website_login_passwords(&mut dispatched_call, user_confirmed) {
                Err(error) => ToolCallResult::from(Err(error)),
                Ok(passwords) => {
                    let result = self
                        .extension_manager
                        .dispatch_authorized_tool_call(
                            &ctx,
                            resolved_tool
                                .expect("authorized non-frontend tools have a resolved owner"),
                            dispatched_call,
                            cancellation_token.unwrap_or_default(),
                        )
                        .await;
                    let result = result.unwrap_or_else(|e| {
                        #[cfg(feature = "telemetry")]
                        crate::posthog::emit_error(
                            "tool_execution_failed",
                            &format!("{}: {}", tool_call.name, e),
                        );
                        let error_data = e.downcast::<ErrorData>().unwrap_or_else(|e| {
                            ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), None)
                        });
                        ToolCallResult::from(Err(error_data))
                    });
                    redact_website_login_passwords(result, passwords)
                }
            }
        };

        debug!("WAITING_TOOL_END: {}", tool_call.name);

        let result = if planning {
            result
        } else {
            self.with_post_tool_hook(result, &tool_call, session)
        };
        let session_manager = self.config.session_manager.clone();
        let ToolCallResult {
            result,
            notification_stream,
            action_required_stream,
        } = result;
        let durable_result = async move {
            let mut terminal_result = result.await;
            if let Ok(output) = terminal_result.as_mut() {
                if output.is_error != Some(true) {
                    let captured = match output_capture {
                        Ok(Some(capture)) => {
                            session_manager.finish_output_capture(capture, output).await
                        }
                        Ok(None) => Ok(()),
                        Err(error) => Err(error),
                    };
                    if let Err(error) = captured {
                        output.content.push(Content::text(format!("The tool completed, but output history could not be fully recorded: {error}")));
                    }
                }
            }
            session_manager
                .complete_tool_operation(&operation_id, &terminal_result)
                .await
                .map_err(|error| {
                    ErrorData::new(
                        ErrorCode::INTERNAL_ERROR,
                        format!(
                            "Tool finished but its terminal result could not be durably recorded: {error}. Its status is in doubt and it must not be retried automatically."
                        ),
                        None,
                    )
                })?;
            operation_guard.disarm();
            terminal_result
        };

        (
            request_id,
            Ok(ToolCallResult {
                result: Box::new(durable_result.boxed()),
                notification_stream,
                action_required_stream,
            }),
        )
    }
}

/// Inserts saved website passwords for `{{login:NAME}}` placeholders. Only a
/// call the user confirmed for this exact request may receive one; the
/// website-login inspector makes every such call prompt, so an unconfirmed
/// call here came through a path without a prompt and is refused.
fn insert_website_login_passwords(
    tool_call: &mut CallToolRequestParams,
    user_confirmed: bool,
) -> Result<crate::website_logins::InsertedPasswords, ErrorData> {
    let Some(arguments) = tool_call.arguments.as_mut() else {
        return Ok(Default::default());
    };
    if crate::website_logins::referenced_names(Some(arguments)).is_empty() {
        return Ok(Default::default());
    }
    if !user_confirmed {
        return Err(ErrorData::new(
            ErrorCode::INVALID_REQUEST,
            "Saved website passwords are inserted only into a tool call the user approved. Make the call directly so the user is asked to approve it.".to_string(),
            None,
        ));
    }
    crate::website_logins::insert_passwords(arguments)
        .map_err(|error| ErrorData::new(ErrorCode::INVALID_PARAMS, error.to_string(), None))
}

fn redact_website_login_passwords(
    result: ToolCallResult,
    passwords: crate::website_logins::InsertedPasswords,
) -> ToolCallResult {
    if passwords.is_empty() {
        return result;
    }
    let passwords = Arc::new(passwords);
    let ToolCallResult {
        result,
        notification_stream,
        action_required_stream,
    } = result;
    let stream_passwords = passwords.clone();
    let notification_stream = notification_stream.map(|stream| {
        Box::new(stream.map(move |mut notification| {
            stream_passwords.redact_notification(&mut notification);
            notification
        })) as Box<dyn futures::Stream<Item = ServerNotification> + Send + Unpin>
    });
    let result = async move {
        let mut output = result.await;
        match output.as_mut() {
            Ok(call_result) => passwords.redact_result(call_result),
            Err(error) => passwords.redact_error(error),
        }
        output
    };
    ToolCallResult {
        result: Box::new(result.boxed()),
        notification_stream,
        action_required_stream,
    }
}

#[cfg(test)]
mod website_login_tests {
    use super::*;
    use serde_json::json;

    fn call(arguments: serde_json::Value) -> CallToolRequestParams {
        CallToolRequestParams::new("browser__fill")
            .with_arguments(arguments.as_object().expect("object").clone())
    }

    #[test]
    fn unconfirmed_call_never_receives_a_saved_password() {
        let mut tool_call = call(json!({"value": "{{login:Work GitHub}}"}));

        let error = insert_website_login_passwords(&mut tool_call, false)
            .err()
            .expect("unconfirmed placeholder must be refused");

        assert_eq!(error.code, ErrorCode::INVALID_REQUEST);
        assert_eq!(
            tool_call.arguments.unwrap()["value"],
            json!("{{login:Work GitHub}}")
        );
    }

    #[test]
    fn call_without_placeholder_is_left_untouched() {
        let mut tool_call = call(json!({"value": "plain text"}));

        let inserted = insert_website_login_passwords(&mut tool_call, false).unwrap();

        assert!(inserted.is_empty());
        assert_eq!(tool_call.arguments.unwrap()["value"], json!("plain text"));
    }
}
