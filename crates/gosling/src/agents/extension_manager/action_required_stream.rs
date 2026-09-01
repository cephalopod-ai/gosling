// Owns action-required stream polling and unregister-on-drop cleanup.
// Tool dispatch receives a Stream while registration bookkeeping stays encapsulated.
// The extension_manager compatibility facade keeps the stream type private.

use super::*;

pub(super) struct ActionRequiredStream {
    inner: ReceiverStream<crate::conversation::message::Message>,
    session_id: String,
    tool_call_request_id: String,
}

impl ActionRequiredStream {
    pub(super) fn new(
        receiver: tokio::sync::mpsc::Receiver<crate::conversation::message::Message>,
        session_id: String,
        tool_call_request_id: String,
    ) -> Self {
        Self {
            inner: ReceiverStream::new(receiver),
            session_id,
            tool_call_request_id,
        }
    }
}

impl Stream for ActionRequiredStream {
    type Item = crate::conversation::message::Message;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for ActionRequiredStream {
    fn drop(&mut self) {
        let session_id = self.session_id.clone();
        let tool_call_request_id = self.tool_call_request_id.clone();
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        handle.spawn(async move {
            ActionRequiredManager::global()
                .unregister_action_required_stream(&session_id, &tool_call_request_id)
                .await;
        });
    }
}
