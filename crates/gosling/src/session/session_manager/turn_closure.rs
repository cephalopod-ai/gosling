//! Closing a turn that stopped before it finished.
//!
//! A turn can stop without reaching its reply: the user cancels it, the client
//! disconnects or quits while an approval is pending, the turn lease is lost,
//! or the process is killed. Its prompt, unanswered tool calls or cut-off reply
//! are then the tail of the history. `fix_conversation` would drop the
//! unanswered calls and merge the stopped prompt into the next one, so the
//! model would run the stopped request as live instruction. Closing the turn
//! answers its unanswered tool calls as not run and appends a notice, so the
//! stopped request stays history and is never resubmitted.

use super::SessionStorage;
use crate::conversation::message::{Message, MessageContent, ProviderMetadata};
use crate::session::extension_data::{AcpPromptRunState, ExtensionData, ExtensionState};
use anyhow::Result;
use futures::TryStreamExt;
use rmcp::model::Role;
use std::collections::HashSet;

pub(crate) const CANCELLED_TURN_NOTICE: &str = "Run cancelled by user before completion.";
pub(crate) const INTERRUPTED_TURN_NOTICE: &str = "Run interrupted before completion.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TurnClosureTrigger {
    /// A client or the CLI reopening the session. Another process or
    /// connection may still be running the turn, so it is left alone while a
    /// live turn lease exists. An in-progress ACP run is then known to have
    /// stopped, and is recorded as interrupted.
    Reopen,
    /// A new turn that already holds the session's turn lease.
    NextTurn,
    /// The caller's own cancelled turn, closed while it still holds the lease.
    Cancel,
}

#[derive(Debug, PartialEq)]
struct UnfinishedTurn {
    unanswered_tool_requests: Vec<(String, Option<ProviderMetadata>)>,
    /// A reply cut off mid-stream. Like a reply interrupted by a provider
    /// failure, it stays for the user but is no longer sent to the model.
    cut_off_reply: Option<Message>,
}

/// The trailing turn of `messages` (oldest first) when it never reached its
/// reply. Trailing tool results only count as unfinished when the caller knows
/// the turn stopped (`turn_known_stopped`): a planner turn legitimately ends
/// on its review tool's result.
fn unfinished_turn(messages: &[Message], turn_known_stopped: bool) -> Option<UnfinishedTurn> {
    let visible: Vec<&Message> = messages
        .iter()
        .filter(|message| message.is_agent_visible())
        .collect();
    let last = *visible.last()?;
    // Imported history is quoted per message and never merged with live
    // input; a handoff checkpoint is meant to be sent with the next prompt.
    if last.metadata.imported_untrusted || crate::session::handoff::is_handoff_checkpoint(last) {
        return None;
    }
    let turn_start = visible
        .iter()
        .rposition(|message| crate::context_mgmt::is_turn_start(message))
        .unwrap_or(0);
    let turn = &visible[turn_start..];
    let answered: HashSet<&str> = turn
        .iter()
        .flat_map(|message| message.content.iter())
        .filter_map(MessageContent::as_tool_response)
        .map(|response| response.id.as_str())
        .collect();
    let unanswered_tool_requests: Vec<_> = turn
        .iter()
        .filter(|message| message.role == Role::Assistant)
        .flat_map(|message| message.content.iter())
        .filter_map(MessageContent::as_tool_request)
        .filter(|request| !answered.contains(request.id.as_str()))
        .map(|request| (request.id.clone(), request.metadata.clone()))
        .collect();
    let ends_on_tool_results = last
        .content
        .iter()
        .any(|content| matches!(content, MessageContent::ToolResponse(_)));
    let cut_off_reply =
        (last.role == Role::Assistant && last.metadata.incomplete).then(|| last.clone());
    let unfinished = !unanswered_tool_requests.is_empty()
        || cut_off_reply.is_some()
        || (last.role == Role::User && (!ends_on_tool_results || turn_known_stopped));
    unfinished.then_some(UnfinishedTurn {
        unanswered_tool_requests,
        cut_off_reply,
    })
}

impl SessionStorage {
    /// Closes the session's trailing turn if it never finished. Returns whether
    /// a closure notice was written.
    pub(super) async fn close_unfinished_turn(
        &self,
        session_id: &str,
        trigger: TurnClosureTrigger,
    ) -> Result<bool> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

        if trigger == TurnClosureTrigger::Reopen
            && self.live_turn_owner(&mut tx, session_id).await?.is_some()
        {
            tx.rollback().await?;
            return Ok(false);
        }

        let extension_data = match trigger {
            TurnClosureTrigger::Reopen => {
                let stored = sqlx::query_scalar::<_, String>(
                    "SELECT extension_data FROM sessions WHERE id = ?",
                )
                .bind(session_id)
                .fetch_optional(&mut *tx)
                .await?;
                stored.map(|json| serde_json::from_str::<ExtensionData>(&json).unwrap_or_default())
            }
            TurnClosureTrigger::NextTurn | TurnClosureTrigger::Cancel => None,
        };
        // Only ACP prompts record a run state, so a terminal value can predate
        // later CLI turns; an in-progress one always describes the last turn.
        let run_in_progress = extension_data
            .as_ref()
            .and_then(AcpPromptRunState::from_extension_data)
            == Some(AcpPromptRunState::InProgress);
        let (notice, turn_known_stopped) = match trigger {
            TurnClosureTrigger::Cancel => (CANCELLED_TURN_NOTICE, true),
            TurnClosureTrigger::Reopen => (INTERRUPTED_TURN_NOTICE, run_in_progress),
            TurnClosureTrigger::NextTurn => (INTERRUPTED_TURN_NOTICE, false),
        };

        let trailing = Self::trailing_turn_in_tx(&mut tx, session_id).await?;
        let mut closed = false;
        let mut plan_staled = false;
        if let Some(turn) = unfinished_turn(&trailing, turn_known_stopped) {
            let dispatched = sqlx::query_scalar::<_, String>(
                "SELECT tool_request_id FROM tool_operations WHERE session_id = ? AND conversation_bound = TRUE",
            )
            .bind(session_id)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .collect::<HashSet<_>>();
            // A dispatched call without a result is either still running or
            // waiting for `recover_tool_operations` to record it as in doubt;
            // saying it did not run could be false, so the turn is closed on a
            // later pass instead.
            let awaiting_recovery = turn
                .unanswered_tool_requests
                .iter()
                .any(|(request_id, _)| dispatched.contains(request_id));
            if !awaiting_recovery {
                if let Some(reply) = turn.cut_off_reply {
                    let user_visible = reply.metadata.user_visible;
                    plan_staled |= self
                        .upsert_message_in_tx(
                            &mut tx,
                            session_id,
                            &reply.with_visibility(user_visible, false),
                        )
                        .await?;
                }
                for (request_id, request_metadata) in &turn.unanswered_tool_requests {
                    let mut response = Message::user().with_generated_id();
                    response.add_tool_response_with_metadata(
                        request_id.clone(),
                        Err(super::tool_operations::cancelled_before_dispatch()),
                        request_metadata.as_ref(),
                    );
                    self.upsert_message_in_tx(&mut tx, session_id, &response)
                        .await?;
                }
                let notice = Message::assistant().with_text(notice).with_generated_id();
                self.upsert_message_in_tx(&mut tx, session_id, &notice)
                    .await?;
                closed = true;
            }
        }

        if let Some(mut extension_data) = extension_data.filter(|_| run_in_progress) {
            AcpPromptRunState::Interrupted.to_extension_data(&mut extension_data)?;
            sqlx::query("UPDATE sessions SET extension_data = ? WHERE id = ?")
                .bind(serde_json::to_string(&extension_data)?)
                .bind(session_id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        self.publish_stale_plan_update(session_id, plan_staled)
            .await;
        Ok(closed)
    }

    /// The stored messages from the last agent-visible turn start to the end,
    /// oldest first, read newest first so long histories are not loaded.
    async fn trailing_turn_in_tx(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        session_id: &str,
    ) -> Result<Vec<Message>> {
        let mut rows = sqlx::query_as::<_, (String, String, i64, Option<String>, Option<String>)>(
            "SELECT role, content_json, created_timestamp, metadata_json, message_id FROM messages WHERE session_id = ? ORDER BY id DESC",
        )
        .bind(session_id)
        .fetch(&mut **tx);
        let mut trailing = Vec::new();
        while let Some((role, content_json, created, metadata_json, message_id)) =
            rows.try_next().await?
        {
            let Some(message) =
                Self::row_to_message(role, content_json, created, metadata_json, message_id)?
            else {
                continue;
            };
            let starts_turn = crate::context_mgmt::is_turn_start(&message);
            trailing.push(message);
            if starts_turn {
                break;
            }
        }
        trailing.reverse();
        Ok(trailing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GoslingMode;
    use crate::session::session_manager::{SessionManager, SessionType, ToolOperationStart};
    use rmcp::model::{CallToolRequestParams, CallToolResult, Content};
    use std::path::PathBuf;
    use tempfile::TempDir;

    async fn session_with(messages: Vec<Message>) -> (TempDir, SessionManager, String) {
        let temp_dir = TempDir::new().unwrap();
        let sm = SessionManager::new(temp_dir.path().to_path_buf());
        let session = sm
            .create_session(
                PathBuf::from("/tmp/test"),
                "Turn closure".to_string(),
                SessionType::User,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        for message in messages {
            sm.add_message(&session.id, &message.with_generated_id())
                .await
                .unwrap();
        }
        (temp_dir, sm, session.id)
    }

    async fn set_run_state(sm: &SessionManager, session_id: &str, state: AcpPromptRunState) {
        sm.merge_extension_state(session_id, "acp_prompt_run.v1", state.to_value().unwrap())
            .await
            .unwrap();
    }

    async fn run_state(sm: &SessionManager, session_id: &str) -> Option<AcpPromptRunState> {
        let session = sm.get_session(session_id, false).await.unwrap();
        AcpPromptRunState::from_extension_data(&session.extension_data)
    }

    async fn stored(sm: &SessionManager, session_id: &str) -> Vec<Message> {
        sm.get_session(session_id, true)
            .await
            .unwrap()
            .conversation
            .unwrap()
            .messages()
            .clone()
    }

    async fn texts(sm: &SessionManager, session_id: &str) -> Vec<String> {
        stored(sm, session_id)
            .await
            .iter()
            .map(Message::as_concat_text)
            .collect()
    }

    #[tokio::test]
    async fn reopening_leaves_a_turn_that_still_holds_its_lease_alone() {
        let (_temp, sm, id) = session_with(vec![Message::user().with_text("run it")]).await;
        set_run_state(&sm, &id, AcpPromptRunState::InProgress).await;
        let lease = sm.acquire_session_turn_lease(&id, None).await.unwrap();

        assert!(!sm.close_interrupted_turn(&id).await.unwrap());
        assert_eq!(texts(&sm, &id).await, vec!["run it"]);
        assert_eq!(
            run_state(&sm, &id).await,
            Some(AcpPromptRunState::InProgress)
        );

        lease.release().await.unwrap();
        assert!(sm.close_interrupted_turn(&id).await.unwrap());
        assert_eq!(
            texts(&sm, &id).await,
            vec!["run it", INTERRUPTED_TURN_NOTICE]
        );
        assert_eq!(
            run_state(&sm, &id).await,
            Some(AcpPromptRunState::Interrupted)
        );
    }

    #[tokio::test]
    async fn a_dispatched_call_without_a_result_is_left_for_recovery() {
        let (_temp, sm, id) = session_with(vec![
            Message::user().with_text("run it"),
            tool_request("dispatched"),
        ])
        .await;
        assert!(matches!(
            sm.begin_tool_operation(
                &id,
                "dispatched",
                &CallToolRequestParams::new("shell"),
                true
            )
            .await
            .unwrap(),
            ToolOperationStart::Execute { .. }
        ));

        assert!(!sm.close_interrupted_turn(&id).await.unwrap());
        assert!(!sm.close_unfinished_turn(&id).await.unwrap());
        let stored = stored(&sm, &id).await;
        assert_eq!(stored.len(), 2);
        assert!(stored
            .iter()
            .flat_map(|message| message.content.iter())
            .all(|content| !matches!(content, MessageContent::ToolResponse(_))));
    }

    #[tokio::test]
    async fn a_cancelled_turn_is_closed_after_its_tool_results() {
        let (_temp, sm, id) = session_with(vec![
            Message::user().with_text("run it"),
            tool_request("done"),
            tool_response("done"),
        ])
        .await;

        assert!(!sm.close_unfinished_turn(&id).await.unwrap());
        assert!(sm.close_cancelled_turn(&id).await.unwrap());
        assert_eq!(
            texts(&sm, &id).await.last().map(String::as_str),
            Some(CANCELLED_TURN_NOTICE)
        );
        assert!(!sm.close_cancelled_turn(&id).await.unwrap());
    }

    #[tokio::test]
    async fn reopening_answers_a_call_left_waiting_for_approval() {
        let (_temp, sm, id) = session_with(vec![
            Message::user().with_text("run it"),
            tool_request("awaiting-approval"),
        ])
        .await;
        set_run_state(&sm, &id, AcpPromptRunState::Cancelled).await;

        assert!(sm.close_interrupted_turn(&id).await.unwrap());
        let stored = stored(&sm, &id).await;
        let response = stored[2].content[0]
            .as_tool_response()
            .expect("the pending call is answered");
        assert_eq!(response.id, "awaiting-approval");
        assert!(response.tool_result.is_err());
        assert_eq!(stored[3].as_concat_text(), INTERRUPTED_TURN_NOTICE);
        assert_eq!(
            run_state(&sm, &id).await,
            Some(AcpPromptRunState::Cancelled)
        );
    }

    #[tokio::test]
    async fn only_an_in_progress_run_makes_trailing_tool_results_unfinished_on_reopen() {
        let (_temp, sm, id) = session_with(vec![
            Message::user().with_text("plan it"),
            tool_request("review"),
            tool_response("review"),
        ])
        .await;
        // A terminal state may predate later CLI turns, which record none.
        set_run_state(&sm, &id, AcpPromptRunState::Interrupted).await;
        assert!(!sm.close_interrupted_turn(&id).await.unwrap());
        assert_eq!(texts(&sm, &id).await.len(), 3);

        set_run_state(&sm, &id, AcpPromptRunState::InProgress).await;
        assert!(sm.close_interrupted_turn(&id).await.unwrap());
        assert_eq!(
            texts(&sm, &id).await.last().map(String::as_str),
            Some(INTERRUPTED_TURN_NOTICE)
        );
    }

    #[tokio::test]
    async fn reopening_records_a_stale_in_progress_run_even_when_its_history_is_closed() {
        let (_temp, sm, id) = session_with(vec![
            Message::user().with_text("run it"),
            Message::assistant().with_text("done"),
        ])
        .await;
        set_run_state(&sm, &id, AcpPromptRunState::InProgress).await;

        assert!(!sm.close_interrupted_turn(&id).await.unwrap());
        assert_eq!(texts(&sm, &id).await, vec!["run it", "done"]);
        assert_eq!(
            run_state(&sm, &id).await,
            Some(AcpPromptRunState::Interrupted)
        );

        set_run_state(&sm, &id, AcpPromptRunState::Completed).await;
        assert!(!sm.close_interrupted_turn(&id).await.unwrap());
        assert_eq!(
            run_state(&sm, &id).await,
            Some(AcpPromptRunState::Completed)
        );
    }

    fn tool_request(id: &str) -> Message {
        Message::assistant()
            .with_generated_id()
            .with_tool_request(id, Ok(CallToolRequestParams::new("shell")))
    }

    fn tool_response(id: &str) -> Message {
        Message::user()
            .with_generated_id()
            .with_tool_response(id, Ok(CallToolResult::success(vec![Content::text("done")])))
    }

    fn unanswered(turn: Option<UnfinishedTurn>) -> Option<Vec<String>> {
        turn.map(|turn| {
            turn.unanswered_tool_requests
                .into_iter()
                .map(|(id, _)| id)
                .collect()
        })
    }

    #[test]
    fn a_prompt_without_a_reply_is_unfinished() {
        let messages = [
            Message::user().with_text("earlier"),
            Message::assistant().with_text("answer"),
            Message::user().with_text("run it"),
        ];
        assert_eq!(
            unanswered(unfinished_turn(&messages, false)),
            Some(Vec::new())
        );
        assert_eq!(
            unanswered(unfinished_turn(
                &[
                    Message::user().with_text("run it"),
                    Message::assistant().with_text("calling a tool"),
                    Message::user().with_text("tool output"),
                ],
                false
            )),
            Some(Vec::new())
        );
    }

    #[test]
    fn unanswered_tool_calls_leave_the_turn_unfinished() {
        let messages = [Message::user().with_text("run it"), tool_request("pending")];
        assert_eq!(
            unanswered(unfinished_turn(&messages, false)),
            Some(vec!["pending".to_string()])
        );

        let parallel = [
            Message::user().with_text("run both"),
            Message::assistant()
                .with_generated_id()
                .with_tool_request("first", Ok(CallToolRequestParams::new("shell")))
                .with_tool_request("second", Ok(CallToolRequestParams::new("shell"))),
            tool_response("first"),
        ];
        assert_eq!(
            unanswered(unfinished_turn(&parallel, false)),
            Some(vec!["second".to_string()])
        );
    }

    #[test]
    fn a_reply_saved_mid_stream_is_unfinished() {
        let messages = [
            Message::user().with_text("long answer please"),
            Message::assistant().with_text("partial").with_incomplete(),
        ];
        let turn = unfinished_turn(&messages, false).expect("the cut-off reply is unfinished");
        assert!(turn.unanswered_tool_requests.is_empty());
        assert_eq!(
            turn.cut_off_reply.map(|reply| reply.as_concat_text()),
            Some("partial".to_string())
        );
    }

    #[test]
    fn trailing_tool_results_are_unfinished_only_when_the_turn_is_known_to_have_stopped() {
        let messages = [
            Message::user().with_text("plan it"),
            tool_request("review"),
            tool_response("review"),
        ];
        assert_eq!(unfinished_turn(&messages, false), None);
        assert_eq!(
            unanswered(unfinished_turn(&messages, true)),
            Some(Vec::new())
        );
    }

    #[test]
    fn finished_and_closed_turns_are_left_alone() {
        assert_eq!(unfinished_turn(&[], true), None);
        for tail in [
            Message::assistant().with_text("partial"),
            Message::assistant().with_text(CANCELLED_TURN_NOTICE),
            Message::assistant()
                .with_text("Run ended by a provider error before completion.")
                .agent_only(),
        ] {
            let messages = [Message::user().with_text("run it"), tail];
            assert_eq!(unfinished_turn(&messages, true), None);
        }
        let hidden_reply = [
            Message::user().with_text("run it"),
            Message::assistant().with_text("done"),
            Message::assistant()
                .with_text("user-only status")
                .user_only(),
        ];
        assert_eq!(unfinished_turn(&hidden_reply, true), None);
    }

    #[test]
    fn imported_history_and_handoff_checkpoints_are_left_alone() {
        let imported = [Message::user().with_text("foreign prompt").with_metadata(
            crate::conversation::message::MessageMetadata::default().with_imported_untrusted(),
        )];
        assert_eq!(unfinished_turn(&imported, true), None);

        let checkpoint = [
            Message::user().with_text("before").agent_only(),
            Message::user()
                .with_id("handoff_snapshot_abc")
                .with_text("# Gosling session checkpoint")
                .with_visibility(false, true),
        ];
        assert_eq!(unfinished_turn(&checkpoint, true), None);
    }
}
