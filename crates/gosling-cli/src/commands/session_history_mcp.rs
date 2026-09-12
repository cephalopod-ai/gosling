use anyhow::Result;
use gosling::agents::mcp_client::McpClientTrait;
use gosling::agents::platform_extensions::{
    session_history::SessionHistoryClient, PlatformExtensionContext,
};
use gosling::agents::ToolCallContext;
use gosling::config::CodeExecutionRuntime;
use gosling::session::SessionManager;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, ErrorData, ListToolsResult, PaginatedRequestParams,
    ServerInfo,
};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, ServerHandler};
use std::path::PathBuf;
use std::sync::Arc;

struct SessionHistoryServer {
    client: SessionHistoryClient,
    session_id: String,
    working_dir: PathBuf,
}

impl ServerHandler for SessionHistoryServer {
    fn get_info(&self) -> ServerInfo {
        self.client.get_info().cloned().unwrap_or_default()
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        self.client
            .list_tools(
                &self.session_id,
                request.and_then(|request| request.cursor),
                context.ct,
            )
            .await
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        self.client
            .call_tool(
                &ToolCallContext::new(
                    self.session_id.clone(),
                    Some(self.working_dir.clone()),
                    None,
                ),
                &request.name,
                request.arguments,
                context.ct,
            )
            .await
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))
    }
}

pub async fn serve(session_id: String, data_dir: PathBuf) -> Result<()> {
    let session_manager = Arc::new(SessionManager::new(data_dir));
    let session = session_manager.get_session(&session_id, false).await?;
    let client = SessionHistoryClient::new(PlatformExtensionContext {
        extension_manager: None,
        session_manager,
        session: Some(Arc::new(session.clone())),
        use_login_shell_path: false,
        code_execution_runtime: CodeExecutionRuntime::Disabled,
    });
    gosling_mcp::mcp_server_runner::serve(SessionHistoryServer {
        client,
        session_id,
        working_dir: session.working_dir,
    })
    .await
}
