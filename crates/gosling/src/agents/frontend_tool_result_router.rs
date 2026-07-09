use std::collections::HashMap;

use rmcp::model::CallToolResult;
use tokio::sync::{oneshot, Mutex};
use tracing::warn;

use crate::mcp_utils::ToolResult;

pub(crate) enum FrontendToolResultRegistration {
    Ready(ToolResult<CallToolResult>),
    Pending(oneshot::Receiver<ToolResult<CallToolResult>>),
}

#[derive(Default)]
struct FrontendToolResultState {
    pending_results: HashMap<String, ToolResult<CallToolResult>>,
    pending_waiters: HashMap<String, oneshot::Sender<ToolResult<CallToolResult>>>,
}

pub(crate) struct FrontendToolResultRouter {
    state: Mutex<FrontendToolResultState>,
}

impl FrontendToolResultRouter {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(FrontendToolResultState::default()),
        }
    }

    pub(crate) async fn register(&self, request_id: String) -> FrontendToolResultRegistration {
        let (tx, rx) = oneshot::channel();
        let mut state = self.state.lock().await;
        state
            .pending_waiters
            .retain(|_, sender| !sender.is_closed());

        if let Some(result) = state.pending_results.remove(&request_id) {
            return FrontendToolResultRegistration::Ready(result);
        }

        if state
            .pending_waiters
            .insert(request_id.clone(), tx)
            .is_some()
        {
            warn!(
                request_id = %request_id,
                "Replacing an existing frontend tool result waiter"
            );
        }

        FrontendToolResultRegistration::Pending(rx)
    }

    pub(crate) async fn deliver(&self, request_id: String, result: ToolResult<CallToolResult>) {
        let mut state = self.state.lock().await;

        if let Some(waiter) = state.pending_waiters.remove(&request_id) {
            if let Err(result) = waiter.send(result) {
                warn!(
                    request_id = %request_id,
                    "Frontend tool result receiver was dropped; storing result for a future waiter"
                );
                state.pending_results.insert(request_id, result);
            }
            return;
        }

        if state
            .pending_results
            .insert(request_id.clone(), result)
            .is_some()
        {
            warn!(
                request_id = %request_id,
                "Replacing a previously buffered frontend tool result"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::Content;

    fn tool_result(text: &str) -> ToolResult<CallToolResult> {
        Ok(CallToolResult::success(vec![Content::text(text)]))
    }

    fn tool_result_text(result: ToolResult<CallToolResult>) -> String {
        let result = result.expect("tool result should be ok");
        result.content[0]
            .as_text()
            .map(|text| text.text.clone())
            .expect("first content item should be text")
    }

    #[tokio::test]
    async fn test_register_then_deliver() {
        let router = FrontendToolResultRouter::new();

        let rx = match router.register("req_1".to_string()).await {
            FrontendToolResultRegistration::Pending(rx) => rx,
            FrontendToolResultRegistration::Ready(_) => panic!("expected pending registration"),
        };

        router
            .deliver("req_1".to_string(), tool_result("one"))
            .await;

        assert_eq!(tool_result_text(rx.await.unwrap()), "one");
    }

    #[tokio::test]
    async fn test_deliver_before_register() {
        let router = FrontendToolResultRouter::new();
        router
            .deliver("req_1".to_string(), tool_result("one"))
            .await;

        let result = match router.register("req_1".to_string()).await {
            FrontendToolResultRegistration::Ready(result) => result,
            FrontendToolResultRegistration::Pending(_) => panic!("expected buffered result"),
        };

        assert_eq!(tool_result_text(result), "one");
    }

    #[tokio::test]
    async fn test_cancelled_receiver_buffers_result_for_retry() {
        let router = FrontendToolResultRouter::new();

        let rx = match router.register("req_1".to_string()).await {
            FrontendToolResultRegistration::Pending(rx) => rx,
            FrontendToolResultRegistration::Ready(_) => panic!("expected pending registration"),
        };
        drop(rx);

        router
            .deliver("req_1".to_string(), tool_result("one"))
            .await;

        let result = match router.register("req_1".to_string()).await {
            FrontendToolResultRegistration::Ready(result) => result,
            FrontendToolResultRegistration::Pending(_) => panic!("expected buffered result"),
        };

        assert_eq!(tool_result_text(result), "one");
    }

    #[tokio::test]
    async fn test_concurrent_requests_do_not_cross_bind() {
        let router = FrontendToolResultRouter::new();

        let rx1 = match router.register("req_1".to_string()).await {
            FrontendToolResultRegistration::Pending(rx) => rx,
            FrontendToolResultRegistration::Ready(_) => panic!("expected pending registration"),
        };
        let rx2 = match router.register("req_2".to_string()).await {
            FrontendToolResultRegistration::Pending(rx) => rx,
            FrontendToolResultRegistration::Ready(_) => panic!("expected pending registration"),
        };

        router
            .deliver("req_2".to_string(), tool_result("two"))
            .await;
        router
            .deliver("req_1".to_string(), tool_result("one"))
            .await;

        assert_eq!(tool_result_text(rx1.await.unwrap()), "one");
        assert_eq!(tool_result_text(rx2.await.unwrap()), "two");
    }
}
