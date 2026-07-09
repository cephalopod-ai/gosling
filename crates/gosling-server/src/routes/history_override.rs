use gosling::conversation::message::Message;
use gosling::conversation::Conversation;
use gosling::session::{Session, SessionManager};
use gosling_providers::conversation::token_usage::Usage;
use rmcp::model::Role;
use tracing::warn;

pub(crate) struct ConversationOverrideRollback {
    restored_conversation: Conversation,
    original_usage: Usage,
}

pub(crate) async fn apply_conversation_override(
    session_manager: &SessionManager,
    session: &Session,
    user_message: &Message,
    history: Vec<Message>,
) -> Option<(Conversation, ConversationOverrideRollback)> {
    let override_conversation = Conversation::new_unvalidated(history);

    if let Err(e) = session_manager
        .replace_conversation(&session.id, &override_conversation)
        .await
    {
        warn!(
            "Failed to replace session conversation for {}: {}",
            session.id, e
        );
        return None;
    }

    if let Err(e) = session_manager
        .update(&session.id)
        .usage(Default::default())
        .apply()
        .await
    {
        warn!(
            "Failed to reset session usage after history override for {}: {}",
            session.id, e
        );
    }

    let mut restored_conversation = session.conversation.clone().unwrap_or_default();
    // Preserve the normal failed-turn behavior of retaining the user prompt
    // while removing only the override-specific history mutation on rollback.
    restored_conversation.push(user_message.clone());

    Some((
        override_conversation,
        ConversationOverrideRollback {
            restored_conversation,
            original_usage: session.usage,
        },
    ))
}

pub(crate) async fn rollback_conversation_override(
    session_manager: &SessionManager,
    session_id: &str,
    rollback: ConversationOverrideRollback,
) {
    if let Err(e) = session_manager
        .replace_conversation(session_id, &rollback.restored_conversation)
        .await
    {
        warn!(
            "Failed to restore session conversation after reply startup failure for {}: {}",
            session_id, e
        );
    }

    if let Err(e) = session_manager
        .update(session_id)
        .usage(rollback.original_usage)
        .apply()
        .await
    {
        warn!(
            "Failed to restore session usage after reply startup failure for {}: {}",
            session_id, e
        );
    }
}

pub(crate) fn is_early_provider_failure_message(message: &Message) -> bool {
    if message.role != Role::Assistant {
        return false;
    }

    let text = message.as_concat_text();
    text.starts_with("Ran into this error:")
        || text.starts_with("The provider refused this request.")
        || text.contains("Please resend your message to try again.")
}
