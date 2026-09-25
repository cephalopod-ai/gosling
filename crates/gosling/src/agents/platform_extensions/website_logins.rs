use crate::agents::extension::PlatformExtensionContext;
use crate::agents::mcp_client::{Error, McpClientTrait};
use crate::agents::tool_execution::ToolCallContext;
use crate::website_logins;
use anyhow::Result;
use async_trait::async_trait;
use indoc::indoc;
use rmcp::model::{
    CallToolResult, Content, Implementation, InitializeResult, JsonObject, ListToolsResult,
    ServerCapabilities, Tool, ToolAnnotations,
};
use serde_json::json;
use tokio_util::sync::CancellationToken;

pub static EXTENSION_NAME: &str = "website_logins";
const LIST_TOOL_NAME: &str = "list";

pub struct WebsiteLoginsClient {
    info: InitializeResult,
}

impl WebsiteLoginsClient {
    pub fn new(_context: PlatformExtensionContext) -> Result<Self> {
        let info = InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(EXTENSION_NAME.to_string(), "1.0.0".to_string())
                    .with_title("Website Logins"),
            )
            .with_instructions(
                indoc! {r#"
                The user can save website logins (name, website URL, username, password) in
                Gosling's credentials. List them to find the account for a site you need to
                sign in to.

                You never see a saved password. Wherever the password belongs in a tool call
                (for example the value typed into a browser password field), write the
                login's placeholder exactly, such as {{login:Work GitHub}}. Gosling inserts
                the password (asking the user first unless they are in Auto mode) and
                removes it from the tool's output. Use a placeholder only on the website it belongs to, never
                ask the user for a saved password, and never try to reveal one.
            "#}
                .to_string(),
            );
        Ok(Self { info })
    }

    fn list_logins() -> Result<Vec<Content>, String> {
        let logins = website_logins::list().map_err(|error| error.to_string())?;
        if logins.is_empty() {
            return Ok(vec![Content::text(
                "No website logins are saved. The user can add one in Settings > Credentials.",
            )]);
        }
        let with_password =
            website_logins::ids_with_password().map_err(|error| error.to_string())?;
        let entries = logins
            .iter()
            .map(|login| {
                json!({
                    "name": login.name,
                    "url": login.url,
                    "username": login.username,
                    "password_placeholder": login.placeholder(),
                    "password_saved": with_password.contains(&login.id),
                })
            })
            .collect::<Vec<_>>();
        let text = serde_json::to_string_pretty(&entries).map_err(|error| error.to_string())?;
        Ok(vec![Content::text(text)])
    }

    fn get_tools() -> Vec<Tool> {
        vec![Tool::new(
            LIST_TOOL_NAME.to_string(),
            indoc! {r#"
                List the website logins the user saved: each login's name, website URL,
                username, and the placeholder to write wherever its password is needed.
            "#}
            .to_string(),
            json!({"type": "object", "properties": {}})
                .as_object()
                .expect("schema is an object")
                .clone(),
        )
        .annotate(ToolAnnotations::from_raw(
            Some("List website logins".to_string()),
            Some(true),
            Some(false),
            Some(true),
            Some(false),
        ))]
    }
}

#[async_trait]
impl McpClientTrait for WebsiteLoginsClient {
    async fn list_tools(
        &self,
        _session_id: &str,
        _next_cursor: Option<String>,
        _cancellation_token: CancellationToken,
    ) -> Result<ListToolsResult, Error> {
        Ok(ListToolsResult {
            tools: Self::get_tools(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        _ctx: &ToolCallContext,
        name: &str,
        _arguments: Option<JsonObject>,
        _cancellation_token: CancellationToken,
    ) -> Result<CallToolResult, Error> {
        let content = match name {
            LIST_TOOL_NAME => Self::list_logins(),
            _ => Err(format!("Unknown tool: {name}")),
        };
        match content {
            Ok(content) => Ok(CallToolResult::success(content)),
            Err(error) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: {error}"
            ))])),
        }
    }

    fn get_info(&self) -> Option<&InitializeResult> {
        Some(&self.info)
    }
}
