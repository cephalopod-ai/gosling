use crate::session::SESSION_ID_HEADER;
use crate::ExpectedSessionId;
use rmcp::model::{
    CallToolResult, ClientNotification, ClientRequest, Content, ErrorCode, Implementation,
    InitializeResult, Meta, ProtocolVersion, ServerCapabilities, ServerInfo,
};
use rmcp::service::{DynService, NotificationContext, RequestContext, ServiceExt, ServiceRole};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
};
use rmcp::{
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler, Service,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::task::JoinHandle;

pub const FAKE_CODE: &str = "test-uuid-12345-67890";

pub const TEST_IMAGE_B64: &str = include_str!("test_assets/test_image.b64").trim_ascii_end();

pub trait HasMeta {
    fn meta(&self) -> &Meta;
}

impl<R: ServiceRole> HasMeta for RequestContext<R> {
    fn meta(&self) -> &Meta {
        &self.meta
    }
}

impl<R: ServiceRole> HasMeta for NotificationContext<R> {
    fn meta(&self) -> &Meta {
        &self.meta
    }
}

struct ValidatingService<S> {
    inner: S,
    expected_session_id: Arc<dyn ExpectedSessionId>,
    list_tools_calls: Arc<AtomicUsize>,
}

impl<S> ValidatingService<S> {
    fn new(
        inner: S,
        expected_session_id: Arc<dyn ExpectedSessionId>,
        list_tools_calls: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            inner,
            expected_session_id,
            list_tools_calls,
        }
    }

    fn validate<C: HasMeta>(&self, context: &C) -> Result<(), McpError> {
        let actual = context
            .meta()
            .0
            .get(SESSION_ID_HEADER)
            .and_then(|v| v.as_str());
        self.expected_session_id
            .validate(actual)
            .map_err(|e| McpError::new(ErrorCode::INVALID_REQUEST, e, None))
    }
}

impl<S: Service<RoleServer>> Service<RoleServer> for ValidatingService<S> {
    async fn handle_request(
        &self,
        request: ClientRequest,
        context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::ServerResult, McpError> {
        if matches!(&request, ClientRequest::ListToolsRequest(_)) {
            self.list_tools_calls.fetch_add(1, Ordering::SeqCst);
        }
        if !matches!(request, ClientRequest::InitializeRequest(_)) {
            self.validate(&context)?;
        }
        self.inner.handle_request(request, context).await
    }

    async fn handle_notification(
        &self,
        notification: ClientNotification,
        context: NotificationContext<RoleServer>,
    ) -> Result<(), McpError> {
        if !matches!(notification, ClientNotification::InitializedNotification(_)) {
            self.validate(&context).ok();
        }
        self.inner.handle_notification(notification, context).await
    }

    fn get_info(&self) -> ServerInfo {
        self.inner.get_info()
    }
}

#[derive(Clone, Default)]
pub struct McpFixtureServer;

#[tool_router]
impl McpFixtureServer {
    pub fn new() -> Self {
        Self
    }

    #[tool(description = "Get the code", annotations(read_only_hint = true))]
    fn get_code(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(FAKE_CODE)]))
    }

    #[tool(description = "Get an image")]
    fn get_image(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::image(
            TEST_IMAGE_B64,
            "image/png",
        )]))
    }
}

#[tool_handler]
impl ServerHandler for McpFixtureServer {
    fn get_info(&self) -> ServerInfo {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2025_03_26)
            .with_server_info(Implementation::new("mcp-fixture", "1.0.0"))
            .with_instructions("Test server with get_code and get_image tools.")
    }
}

pub struct McpFixture {
    pub url: String,
    list_tools_calls: Arc<AtomicUsize>,
    handle: JoinHandle<()>,
}

impl Drop for McpFixture {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

type McpServiceFactory =
    Box<dyn Fn() -> Result<Box<dyn DynService<RoleServer>>, std::io::Error> + Send + Sync>;

impl McpFixture {
    pub async fn new(expected_session_id: Arc<dyn ExpectedSessionId>) -> Self {
        let list_tools_calls = Arc::new(AtomicUsize::new(0));
        let service_list_tools_calls = Arc::clone(&list_tools_calls);
        let service_factory: McpServiceFactory = Box::new(move || {
            Ok(ValidatingService::new(
                McpFixtureServer::new(),
                expected_session_id.clone(),
                Arc::clone(&service_list_tools_calls),
            )
            .into_dyn())
        });

        let service = StreamableHttpService::new(
            service_factory,
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default(),
        );
        let router = axum::Router::new().nest_service("/mcp", service);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/mcp");

        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });

        Self {
            url,
            list_tools_calls,
            handle,
        }
    }

    pub fn list_tools_call_count(&self) -> usize {
        self.list_tools_calls.load(Ordering::SeqCst)
    }
}

#[derive(Clone)]
struct MuninnRecallFixtureServer {
    payload: serde_json::Value,
    call_count: Arc<AtomicUsize>,
}

#[tool_router]
impl MuninnRecallFixtureServer {
    #[tool(
        description = "Read-only Muninn recall fixture",
        annotations(read_only_hint = true)
    )]
    fn muninn_recall(&self) -> Result<CallToolResult, McpError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        Ok(CallToolResult::structured(self.payload.clone()))
    }
}

#[tool_handler]
impl ServerHandler for MuninnRecallFixtureServer {
    fn get_info(&self) -> ServerInfo {
        InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2025_03_26)
            .with_server_info(Implementation::new("muninn-recall-fixture", "1.0.0"))
    }
}

pub struct MuninnRecallFixture {
    pub url: String,
    call_count: Arc<AtomicUsize>,
    list_tools_calls: Arc<AtomicUsize>,
    handle: JoinHandle<()>,
}

impl Drop for MuninnRecallFixture {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl MuninnRecallFixture {
    pub async fn new(
        payload: serde_json::Value,
        expected_session_id: Arc<dyn ExpectedSessionId>,
    ) -> Self {
        let call_count = Arc::new(AtomicUsize::new(0));
        let list_tools_calls = Arc::new(AtomicUsize::new(0));
        let service_call_count = Arc::clone(&call_count);
        let service_list_tools_calls = Arc::clone(&list_tools_calls);
        let service_factory: McpServiceFactory = Box::new(move || {
            Ok(ValidatingService::new(
                MuninnRecallFixtureServer {
                    payload: payload.clone(),
                    call_count: Arc::clone(&service_call_count),
                },
                expected_session_id.clone(),
                Arc::clone(&service_list_tools_calls),
            )
            .into_dyn())
        });
        let service = StreamableHttpService::new(
            service_factory,
            LocalSessionManager::default().into(),
            StreamableHttpServerConfig::default(),
        );
        let router = axum::Router::new().nest_service("/mcp", service);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let url = format!("http://{addr}/mcp");
        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.unwrap();
        });
        Self {
            url,
            call_count,
            list_tools_calls,
            handle,
        }
    }

    pub fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }

    pub fn list_tools_call_count(&self) -> usize {
        self.list_tools_calls.load(Ordering::SeqCst)
    }
}
