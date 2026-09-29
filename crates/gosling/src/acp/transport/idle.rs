//! Reaps Streamable-HTTP ACP connections their clients abandoned.
//!
//! The upstream `agent-client-protocol-http` registry keeps a connection, and
//! the agent behind it, until the client sends `DELETE` or the agent exits. A
//! client that initializes and then goes away without either would otherwise
//! hold its agent for the life of the server.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Once, PoisonError, Weak};
use std::task::{Context, Poll};
use std::time::Duration;

use axum::body::{Body, BodyDataStream};
use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use futures::Stream;
use tokio::time::Instant;

use crate::acp::server::AgentConnectionControl;

pub(super) const HTTP_CONNECTION_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

const CONNECTION_ID_HEADER: &str = "acp-connection-id";

tokio::task_local! {
    static INITIALIZING: Arc<AgentConnectionControl>;
}

/// The control for the connection an `initialize` request is creating, or a
/// fresh one for connections this module does not track (WebSockets).
pub(super) fn connection_control() -> Arc<AgentConnectionControl> {
    INITIALIZING.try_with(Arc::clone).unwrap_or_default()
}

struct TrackedConnection {
    control: Arc<AgentConnectionControl>,
    open_requests: usize,
    last_activity: Instant,
}

pub(super) struct IdleHttpConnections {
    idle_timeout: Duration,
    connections: Mutex<HashMap<String, TrackedConnection>>,
    sweeper: Once,
}

impl IdleHttpConnections {
    pub(super) fn new(idle_timeout: Duration) -> Arc<Self> {
        Arc::new(Self {
            idle_timeout,
            connections: Mutex::default(),
            sweeper: Once::new(),
        })
    }

    fn connections(&self) -> std::sync::MutexGuard<'_, HashMap<String, TrackedConnection>> {
        self.connections
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn register(self: &Arc<Self>, connection_id: String, control: Arc<AgentConnectionControl>) {
        self.connections().insert(
            connection_id,
            TrackedConnection {
                control,
                open_requests: 0,
                last_activity: Instant::now(),
            },
        );
        self.sweeper.call_once(|| {
            tokio::spawn(sweep(Arc::downgrade(self)));
        });
    }

    fn begin_request(self: &Arc<Self>, connection_id: &str) -> Option<OpenRequest> {
        let mut connections = self.connections();
        let connection = connections.get_mut(connection_id)?;
        connection.open_requests += 1;
        connection.last_activity = Instant::now();
        Some(OpenRequest {
            tracker: Arc::clone(self),
            connection_id: connection_id.to_string(),
        })
    }

    fn end_request(&self, connection_id: &str) {
        if let Some(connection) = self.connections().get_mut(connection_id) {
            connection.open_requests -= 1;
            connection.last_activity = Instant::now();
        }
    }

    fn forget(&self, connection_id: &str) {
        self.connections().remove(connection_id);
    }

    /// Closes every connection with no open request or stream, no running
    /// prompt, and no activity for the idle timeout. Closing happens under the
    /// lock, so a request for the connection cannot slip in between the check
    /// and the close.
    fn reap_idle(&self) {
        let idle_timeout = self.idle_timeout;
        self.connections().retain(|_, connection| {
            let idle = connection.open_requests == 0
                && connection.last_activity.elapsed() >= idle_timeout
                && !connection.control.is_running_prompt();
            if idle {
                connection.control.close();
            }
            !idle
        });
    }
}

async fn sweep(tracker: Weak<IdleHttpConnections>) {
    let Some(period) = tracker.upgrade().map(|tracker| tracker.idle_timeout / 4) else {
        return;
    };
    loop {
        tokio::time::sleep(period).await;
        let Some(tracker) = tracker.upgrade() else {
            return;
        };
        tracker.reap_idle();
    }
}

/// Keeps a connection from being reaped while a request to it is being
/// handled or, for `GET`, while its SSE stream is open.
struct OpenRequest {
    tracker: Arc<IdleHttpConnections>,
    connection_id: String,
}

impl Drop for OpenRequest {
    fn drop(&mut self) {
        self.tracker.end_request(&self.connection_id);
    }
}

struct StreamWithOpenRequest {
    stream: BodyDataStream,
    _open: OpenRequest,
}

impl Stream for StreamWithOpenRequest {
    type Item = <BodyDataStream as Stream>::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.stream).poll_next(cx)
    }
}

pub(super) async fn track_http_connections(
    State(tracker): State<Arc<IdleHttpConnections>>,
    request: Request,
    next: Next,
) -> Response {
    let connection_id = request
        .headers()
        .get(CONNECTION_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let method = request.method().clone();

    let Some(connection_id) = connection_id else {
        if method != Method::POST {
            return next.run(request).await;
        }
        let control = Arc::new(AgentConnectionControl::default());
        let response = INITIALIZING
            .scope(Arc::clone(&control), next.run(request))
            .await;
        if let Some(connection_id) = response
            .headers()
            .get(CONNECTION_ID_HEADER)
            .and_then(|value| value.to_str().ok())
        {
            tracker.register(connection_id.to_string(), control);
        }
        return response;
    };

    let Some(open) = tracker.begin_request(&connection_id) else {
        return next.run(request).await;
    };
    let response = next.run(request).await;
    if method == Method::DELETE && response.status().is_success() {
        tracker.forget(&connection_id);
    } else if method == Method::GET && response.status().is_success() {
        return response.map(|body| {
            Body::from_stream(StreamWithOpenRequest {
                stream: body.into_data_stream(),
                _open: open,
            })
        });
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::server_factory::{AcpServer, AcpServerFactoryConfig};
    use crate::agents::GoslingPlatform;
    use axum::http::{header, StatusCode};
    use axum::Router;
    use tower::ServiceExt;

    const TEST_IDLE_TIMEOUT: Duration = Duration::from_secs(1);
    const PAST_IDLE_TIMEOUT: Duration = Duration::from_millis(2500);

    fn test_router(dir: &tempfile::TempDir) -> Router {
        let server = Arc::new(AcpServer::new(AcpServerFactoryConfig {
            builtins: vec![],
            state_dir: dir.path().join("state"),
            data_dir: dir.path().join("data"),
            platform_data_dir: dir.path().join("data"),
            config_dir: dir.path().join("config"),
            gosling_platform: GoslingPlatform::GoslingCli,
            additional_source_roots: Vec::new(),
            shell_runtime: Default::default(),
        }));
        super::super::acp_http_router(
            server,
            super::super::AcpOriginPolicy::loopback(),
            TEST_IDLE_TIMEOUT,
        )
    }

    async fn initialize(router: &Router) -> String {
        let request = Request::builder()
            .method(Method::POST)
            .uri("/acp")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":1}}"#,
            ))
            .unwrap();
        let response = router.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        response
            .headers()
            .get(CONNECTION_ID_HEADER)
            .expect("initialize returns a connection id")
            .to_str()
            .unwrap()
            .to_string()
    }

    async fn open_stream(router: &Router, connection_id: &str) -> Response {
        let request = Request::builder()
            .method(Method::GET)
            .uri("/acp")
            .header(header::ACCEPT, "text/event-stream")
            .header(CONNECTION_ID_HEADER, connection_id)
            .body(Body::empty())
            .unwrap();
        router.clone().oneshot(request).await.unwrap()
    }

    #[tokio::test]
    async fn abandoned_http_connection_is_reaped_after_idle_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let router = test_router(&dir);
        let connection_id = initialize(&router).await;

        tokio::time::sleep(PAST_IDLE_TIMEOUT).await;

        let status = open_stream(&router, &connection_id).await.status();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn http_connection_with_open_stream_is_kept_until_the_stream_closes() {
        let dir = tempfile::tempdir().unwrap();
        let router = test_router(&dir);
        let connection_id = initialize(&router).await;
        let stream = open_stream(&router, &connection_id).await;
        assert_eq!(stream.status(), StatusCode::OK);

        tokio::time::sleep(PAST_IDLE_TIMEOUT).await;

        let second = open_stream(&router, &connection_id).await;
        assert_eq!(second.status(), StatusCode::OK);

        drop(stream);
        drop(second);
        tokio::time::sleep(PAST_IDLE_TIMEOUT).await;

        let status = open_stream(&router, &connection_id).await.status();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}
