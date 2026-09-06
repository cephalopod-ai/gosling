//! ACP multi-tool chain summary generation and persistence.
//!
//! Maintainers: keep completion detection, retry policy, and persistence ordering together.
//! Clients: chain summary timing, fallback behavior, and metadata remain stable.

use super::*;

impl GoslingAcpAgent {
    /// If `tool_call_id` belongs to a multi-tool chain and every step in that
    /// chain has now had its response processed, spawn a single LLM
    /// summarization task that persists the chain summary on the first tool
    /// request and notifies the client. Idempotent — fires at most once per
    /// chain.
    pub(super) fn maybe_summarize_chain(
        &self,
        tool_call_id: &str,
        session_id: &SessionId,
        _session_id_str: &str,
        session: &mut GoslingAcpSession,
        cx: &ConnectionTo<Client>,
    ) {
        let Some(chain) = session.chain_membership.get(tool_call_id).cloned() else {
            // A single tool call has no chain; this is the common case, not a fault.
            debug!(
                "tool chain summary: skipped — no chain registered for tool_call_id {tool_call_id}",
            );
            return;
        };
        if !chain
            .ids
            .iter()
            .all(|id| session.responded_tool_ids.contains(id))
        {
            let total = chain.ids.len();
            let responded = chain
                .ids
                .iter()
                .filter(|id| session.responded_tool_ids.contains(*id))
                .count();
            let missing: Vec<&String> = chain
                .ids
                .iter()
                .filter(|id| !session.responded_tool_ids.contains(*id))
                .collect();
            warn!(
                "tool chain summary: waiting on {pending}/{total} responses for chain anchored at {anchor:?} (missing: {missing:?})",
                pending = total - responded,
                anchor = chain.ids.first(),
            );
            return;
        }
        let Some(first_id) = chain.ids.first() else {
            warn!("tool chain summary: skipped — empty chain.ids for tool_call_id {tool_call_id}");
            return;
        };
        if !session.summarized_chains.insert(first_id.clone()) {
            debug!("tool chain summary: chain anchored at {first_id} already summarized; skipping");
            return;
        }

        let agent = session.agent.clone();

        // Snapshot (name, args_json) for each step in document order.
        let steps: Vec<(String, String)> = chain
            .ids
            .iter()
            .filter_map(|id| {
                let req = session.tool_requests.get(id)?;
                let tool_call = req.tool_call.as_ref().ok()?;
                let name = tool_call.name.to_string();
                let args = tool_call
                    .arguments
                    .as_ref()
                    .map(|a| serde_json::to_string(a).unwrap_or_default())
                    .unwrap_or_default();
                let args = if args.len() > 200 {
                    format!("{}…", crate::utils::safe_truncate(&args, 200))
                } else {
                    args
                };
                Some((name, args))
            })
            .collect();
        if steps.len() < 2 {
            return;
        }

        let identity_meta = session
            .tool_requests
            .get(first_id)
            .and_then(tool_call_identity_meta);

        let sid = session_id.clone();
        let chain_for_task = chain.clone();
        let cx = cx.clone();
        let session_manager = self.session_manager.clone();

        let first_id = first_id.clone();
        tokio::spawn(async move {
            let provider = match agent.provider().await {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        "tool chain summary: failed to get provider for chain anchored at {first_id}: {e}",
                    );
                    return;
                }
            };
            if provider.manages_own_context() {
                warn!(
                    "tool chain summary: provider manages own context; skipping chain anchored at {first_id}",
                );
                return;
            }

            let system = "Summarize this sequence of tool calls in a short lowercase phrase \
                 (3-8 words). No punctuation. No quotes. \
                 Examples: applied dark mode polish, scanned for security issues, \
                 refactored config loading";

            let mut user_text = String::from("Tool call sequence:\n");
            for (i, (name, args)) in steps.iter().enumerate() {
                user_text.push_str(&format!("Step {}: {} {}\n", i + 1, name, args));
            }
            let message = Message::user().with_text(&user_text);
            let model_config = match agent.model_config_for_session(&sid.0).await {
                Ok(config) => config,
                Err(_) => return,
            };
            let fast_model_config =
                match crate::model_config::get_fast_model(provider.get_name(), &model_config).await
                {
                    Ok(config) => config,
                    Err(_) => return,
                };

            // Match the per-tool retry policy: one retry on empty/error keeps
            // the chain header reliable when the fast model is rate-limited or
            // momentarily flaky, without escalating to the regular model.
            let mut summary: Option<String> = None;
            for attempt in 0..2 {
                match crate::session_context::with_session_id(
                    Some(sid.0.to_string()),
                    provider.complete(
                        &fast_model_config,
                        system,
                        std::slice::from_ref(&message),
                        &[],
                    ),
                )
                .await
                {
                    Ok((response, _)) => {
                        let s = response
                            .content
                            .iter()
                            .filter_map(|c: &MessageContent| c.as_text())
                            .collect::<String>()
                            .trim()
                            .to_string();
                        if !s.is_empty() {
                            summary = Some(s);
                            break;
                        }
                        if attempt == 0 {
                            warn!(
                                "tool chain summary: fast_complete returned empty for chain anchored at {first_id} ({} steps), retrying once",
                                steps.len(),
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                        }
                    }
                    Err(e) => {
                        if attempt == 0 {
                            warn!(
                                "tool chain summary: fast_complete errored for chain anchored at {first_id}: {e}, retrying once",
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                        } else {
                            warn!(
                                "tool chain summary: fast_complete errored for chain anchored at {first_id} after retry: {e}",
                            );
                        }
                    }
                }
            }
            let Some(summary) = summary else {
                warn!(
                    "tool chain summary: no LLM summary produced for chain anchored at {first_id} — replay will fall back to the deterministic phrase",
                );
                return;
            };
            let summary = presentation::project_tool_chain_summary(&summary);

            let count = chain_for_task.ids.len();
            let patch = serde_json::json!({
                crate::conversation::message::TOOL_META_CHAIN_SUMMARY_KEY: {
                    "summary": &summary,
                    "count": count,
                },
            });
            if let Err(e) = session_manager
                .update_tool_request_meta(&sid.0, &chain_for_task.message_id, &first_id, patch)
                .await
            {
                warn!(
                    "tool chain summary: persist failed for chain anchored at {first_id} in {}: {e}",
                    chain_for_task.message_id,
                );
            }

            let meta = with_tool_chain_summary_meta(identity_meta, &summary, count);
            let fields = ToolCallUpdateFields::new();
            let _ = cx.send_notification(SessionNotification::new(
                sid,
                SessionUpdate::ToolCallUpdate(
                    ToolCallUpdate::new(
                        ToolCallId::new(presentation::project_identifier(&first_id)),
                        fields,
                    )
                    .meta(meta),
                ),
            ));
        });
    }
}
