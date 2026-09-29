use crate::config::paths::Paths;
use crate::providers::api_client::{ApiClient, AuthMethod};
use crate::providers::oauth_device_flow::{run_device_flow, DeviceFlowConfig, RequestEncoding};
use crate::providers::openai_compatible::{
    handle_status, stream_openai_compat, stream_responses_compat,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use axum::http;
use chrono::{DateTime, Utc};
use gosling_providers::errors::ProviderError;
use gosling_providers::formats::openai::is_openai_responses_model;
use gosling_providers::images::ImageFormat;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

// Task-local so complete() and stream() can't race on the same provider instance.
tokio::task_local! {
    static IS_AGENT_CALL: bool;
}

use super::base::{
    collect_stream, Provider, ProviderDef, ProviderMetadata, DEFAULT_PROVIDER_TIMEOUT_SECS,
};
use super::openai_compatible::handle_response_openai_compat;
use super::retry::ProviderRetry;
use super::utils::get_model;
use gosling_providers::formats::openai::{create_request, get_usage, response_to_message};
use gosling_providers::formats::openai_responses::create_responses_request;

use crate::config::{Config, ConfigError};
use crate::conversation::message::{Message, MessageContent};

use crate::providers::base::{ConfigKey, MessageStream};
use futures::future::BoxFuture;
use gosling_providers::conversation::token_usage::{ProviderUsage, Usage};
use gosling_providers::model::ModelConfig;
use gosling_providers::request_log::{start_log, LoggerHandleExt};
use rmcp::model::{RawContent, Tool};
use std::ops::Deref;

const GITHUB_COPILOT_PROVIDER_NAME: &str = "github_copilot";
pub const GITHUB_COPILOT_DEFAULT_MODEL: &str = "gpt-4.1";
pub const GITHUB_COPILOT_KNOWN_MODELS: &[&str] = &[
    "claude-haiku-4.5",
    "claude-opus-4.5",
    "claude-opus-4.6",
    "claude-opus-4.7",
    "claude-sonnet-4",
    "claude-sonnet-4.5",
    "claude-sonnet-4.6",
    "gemini-2.5-pro",
    "gemini-3-flash-preview",
    "gemini-3.1-pro-preview",
    "gpt-4.1",
    "gpt-4o",
    "grok-code-fast-1",
    "gpt-5-mini",
    "gpt-5.2",
    "gpt-5.2-codex",
    "gpt-5.3-codex",
    "gpt-5.4",
    "gpt-5.4-mini",
    "gpt-5.5",
];

// Models that support streaming on the /chat/completions path.
// Models routed to /responses always stream and don't need to be listed here.
pub const GITHUB_COPILOT_STREAM_MODELS: &[&str] = &[
    "gpt-4.1",
    "gpt-4o",
    "grok-code-fast-1",
    "gemini-2.5-pro",
    "gemini-3-flash-preview",
    "gemini-3.1-pro-preview",
];

const GITHUB_COPILOT_DOC_URL: &str =
    "https://docs.github.com/en/copilot/using-github-copilot/ai-models";
const DEFAULT_GITHUB_HOST: &str = "github.com";
const DEFAULT_GITHUB_COPILOT_CLIENT_ID: &str = "Iv1.b507a08c87ecfe98";
const GITHUB_COPILOT_SIGN_IN_REQUIRED: &str =
    "GitHub Copilot sign-in required to list its models: run `gosling configure` and select GitHub Copilot";

/// Whether a token lookup may start the GitHub device flow when no sign-in is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SignIn {
    /// Real requests.
    Allowed,
    /// Model listing (which the Desktop runs in the background) and checks of an
    /// existing sign-in: never open a browser, report that a sign-in is required.
    Never,
}

/// A 401/403 from the token exchange means the stored GitHub sign-in itself is
/// no longer accepted, so retrying cannot help; only a new sign-in can.
fn rejected_sign_in_status(err: &anyhow::Error) -> Option<reqwest::StatusCode> {
    err.downcast_ref::<reqwest::Error>()
        .and_then(reqwest::Error::status)
        .filter(|status| {
            matches!(
                *status,
                reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN
            )
        })
}

fn normalize_host(host: &str) -> String {
    let host = host.trim_end_matches('/');
    let host = host.strip_prefix("https://").unwrap_or(host);
    host.to_string()
}

/// The GitHub OAuth token cached under this key is a real, long-lived
/// credential obtained from a device-code login against `host`. It must
/// not be reused against a different host - `GITHUB_COPILOT_HOST` is a
/// user-settable, non-secret config value, so anyone who can change it
/// (a malicious "recipe"/config snippet, a compromised extension) could
/// otherwise cause the real token to be replayed as a bearer credential
/// against an attacker-controlled endpoint with no re-authentication.
/// Mirrors DiskCache::new's existing per-host cache-path scoping.
fn token_secret_key(host: &str) -> String {
    if host == DEFAULT_GITHUB_HOST {
        "GITHUB_COPILOT_TOKEN".to_string()
    } else {
        let safe_host = host.replace(['/', ':', '.'], "_");
        format!("GITHUB_COPILOT_TOKEN__{}", safe_host)
    }
}

pub(crate) fn is_token_secret_key(key: &str) -> bool {
    key == "GITHUB_COPILOT_TOKEN" || key.starts_with("GITHUB_COPILOT_TOKEN__")
}

/// Whether a token is stored under the key the *currently configured*
/// `GITHUB_COPILOT_HOST` would use. The static `GITHUB_COPILOT_TOKEN`
/// `ConfigKey` this provider declares in `metadata()` can't itself be
/// host-scoped (config-key names are fixed at provider-registration time),
/// so generic "is this provider configured" checks that key it directly -
/// they'd report a provider as unconfigured after a successful non-default-host
/// login, since the token is stored under a different key. Provider
/// registration (see providers/init.rs) wires this in as the inventory
/// `configured` resolver instead of the generic per-key check.
pub fn has_configured_token() -> bool {
    let config = Config::global();
    let host = normalize_host(
        &config
            .get_param::<String>("GITHUB_COPILOT_HOST")
            .unwrap_or_else(|_| DEFAULT_GITHUB_HOST.to_string()),
    );
    config
        .get_secret::<String>(&token_secret_key(&host))
        .is_ok()
}

#[derive(Debug, Clone)]
struct GithubCopilotUrls {
    device_code_url: String,
    access_token_url: String,
    copilot_token_url: String,
}

impl GithubCopilotUrls {
    fn new(host: &str, copilot_token_url: Option<&str>) -> Self {
        if host == "github.com" {
            Self {
                device_code_url: "https://github.com/login/device/code".to_string(),
                access_token_url: "https://github.com/login/oauth/access_token".to_string(),
                copilot_token_url: "https://api.github.com/copilot_internal/v2/token".to_string(),
            }
        } else {
            let base = format!("https://{}", host);
            let copilot_token_url = copilot_token_url
                .map(|u| u.trim_end_matches('/').to_string())
                .unwrap_or_else(|| format!("https://api.{}/copilot_internal/v2/token", host));
            Self {
                device_code_url: format!("{}/login/device/code", base),
                access_token_url: format!("{}/login/oauth/access_token", base),
                copilot_token_url,
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CopilotTokenEndpoints {
    api: String,
    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)] // useful for debugging
struct CopilotTokenInfo {
    token: String,
    expires_at: i64,
    refresh_in: i64,
    endpoints: CopilotTokenEndpoints,
    #[serde(flatten)]
    _extra: HashMap<String, Value>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct CopilotState {
    expires_at: DateTime<Utc>,
    info: CopilotTokenInfo,
}

#[derive(Debug)]
struct DiskCache {
    cache_path: PathBuf,
}

impl DiskCache {
    fn new(host: &str) -> Self {
        let cache_path = if host == DEFAULT_GITHUB_HOST {
            Paths::in_config_dir("githubcopilot/info.json")
        } else {
            let safe_host = host.replace(['/', ':', '.'], "_");
            Paths::in_config_dir(&format!("githubcopilot/{}/info.json", safe_host))
        };
        Self { cache_path }
    }

    async fn load(&self) -> Option<CopilotState> {
        if let Ok(contents) = tokio::fs::read_to_string(&self.cache_path).await {
            if let Ok(info) = serde_json::from_str::<CopilotState>(&contents) {
                return Some(info);
            }
        }
        None
    }

    async fn save(&self, info: &CopilotState) -> Result<()> {
        let contents = serde_json::to_string(info)?;
        crate::config::base::write_secrets_file(&self.cache_path, &contents)?;
        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        match tokio::fs::remove_file(&self.cache_path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct GithubCopilotProvider {
    #[serde(skip)]
    client: Client,
    #[serde(skip)]
    cache: DiskCache,
    #[serde(skip)]
    mu: tokio::sync::Mutex<RefCell<Option<CopilotState>>>,
    #[serde(skip)]
    urls: GithubCopilotUrls,
    #[serde(skip)]
    client_id: String,
    #[serde(skip)]
    name: String,
    #[serde(skip)]
    tls_config: Option<crate::providers::api_client::TlsConfig>,
    #[serde(skip)]
    token_secret_key: String,
}

impl GithubCopilotProvider {
    pub async fn cleanup() -> Result<()> {
        let config = Config::global();
        let host = normalize_host(
            &config
                .get_param::<String>("GITHUB_COPILOT_HOST")
                .unwrap_or_else(|_| DEFAULT_GITHUB_HOST.to_string()),
        );
        // The disk cache only holds the short-lived Copilot API token; the
        // long-lived GitHub OAuth token lives in the secret store under a
        // host-scoped key and must be deleted too, or "remove provider"
        // leaves it behind and `has_configured_token` keeps reporting true.
        // Propagate failures instead of swallowing them: a caller like
        // provider-config deletion needs to know the token wasn't cleared.
        config.delete_secret(&token_secret_key(&host))?;
        DiskCache::new(&host).clear().await
    }

    fn messages_contain_image(messages: &[Message]) -> bool {
        messages.iter().any(|m| {
            m.content.iter().any(|c| match c {
                MessageContent::Image(_) => true,
                MessageContent::ToolResponse(resp) => resp.tool_result.as_ref().is_ok_and(|r| {
                    r.content
                        .iter()
                        .any(|item| matches!(item.deref(), RawContent::Image(_)))
                }),
                _ => false,
            })
        })
    }

    pub async fn from_env(
        tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> Result<Self> {
        let config = Config::global();
        let host = normalize_host(
            &config
                .get_param::<String>("GITHUB_COPILOT_HOST")
                .unwrap_or_else(|_| DEFAULT_GITHUB_HOST.to_string()),
        );
        let client_id: String = config
            .get_param("GITHUB_COPILOT_CLIENT_ID")
            .unwrap_or_else(|_| DEFAULT_GITHUB_COPILOT_CLIENT_ID.to_string());
        let copilot_token_url: Option<String> = config.get_param("GITHUB_COPILOT_TOKEN_URL").ok();
        let urls = GithubCopilotUrls::new(&host, copilot_token_url.as_deref());
        let client = Client::builder()
            .timeout(Duration::from_secs(DEFAULT_PROVIDER_TIMEOUT_SECS))
            .build()?;
        let cache = DiskCache::new(&host);
        let mu = tokio::sync::Mutex::new(RefCell::new(None));
        let token_secret_key = token_secret_key(&host);
        Ok(Self {
            client,
            cache,
            mu,
            urls,
            client_id,
            name: GITHUB_COPILOT_PROVIDER_NAME.to_string(),
            tls_config,
            token_secret_key,
        })
    }

    async fn post(
        &self,
        path: &str,
        is_user_initiated: bool,
        payload: &mut Value,
        has_images: bool,
    ) -> Result<Response, ProviderError> {
        let (endpoint, token) = self.get_api_info(SignIn::Allowed).await?;
        let auth = AuthMethod::BearerToken(token);
        let mut headers = self.get_github_headers();
        if has_images {
            headers.insert("Copilot-Vision-Request", "true".parse().unwrap());
        }
        let initiator = if is_user_initiated { "user" } else { "agent" };
        headers.insert("X-Initiator", initiator.parse().unwrap());
        let api_client = ApiClient::new_with_tls(endpoint.clone(), auth, self.tls_config.clone())?
            .with_request_builder(crate::session_context::session_id_request_builder())
            .with_headers(headers)?;

        api_client
            .response_post(path, payload)
            .await
            .map_err(|e| e.into())
    }

    async fn get_api_info(&self, sign_in: SignIn) -> Result<(String, String), ProviderError> {
        let guard = self.mu.lock().await;

        if let Some(state) = guard.borrow().as_ref() {
            if state.expires_at > Utc::now() {
                return Ok((state.info.endpoints.api.clone(), state.info.token.clone()));
            }
        }

        if let Some(state) = self.cache.load().await {
            if guard.borrow().is_none() {
                guard.replace(Some(state.clone()));
            }
            if state.expires_at > Utc::now() {
                return Ok((state.info.endpoints.api, state.info.token));
            }
        }

        const MAX_ATTEMPTS: i32 = 3;
        for attempt in 0..MAX_ATTEMPTS {
            tracing::trace!("attempt {} to refresh api info", attempt + 1);
            let info = match self.refresh_api_info(sign_in).await {
                Ok(data) => data,
                Err(err)
                    if sign_in == SignIn::Never
                        && matches!(
                            err.downcast_ref::<ConfigError>(),
                            Some(ConfigError::NotFound(_))
                        ) =>
                {
                    return Err(ProviderError::Authentication(
                        GITHUB_COPILOT_SIGN_IN_REQUIRED.to_string(),
                    ));
                }
                Err(err) => {
                    if let Some(status) = rejected_sign_in_status(&err) {
                        return Err(self.forget_rejected_sign_in(status).await);
                    }
                    tracing::warn!("failed to refresh api info: {}", err);
                    continue;
                }
            };
            let expires_at = Utc::now() + chrono::Duration::seconds(info.refresh_in);
            let new_state = CopilotState { info, expires_at };
            self.cache.save(&new_state).await?;
            guard.replace(Some(new_state.clone()));
            return Ok((new_state.info.endpoints.api, new_state.info.token));
        }
        Err(anyhow!("failed to get api info after 3 attempts").into())
    }

    /// A rejected GitHub sign-in cannot recover by itself, so it is removed:
    /// the next request then starts a fresh GitHub sign-in instead of
    /// replaying the dead token.
    async fn forget_rejected_sign_in(&self, status: reqwest::StatusCode) -> ProviderError {
        if let Err(error) = Config::global().delete_secret(&self.token_secret_key) {
            tracing::warn!("could not remove the rejected GitHub Copilot sign-in: {error}");
        }
        if let Err(error) = self.cache.clear().await {
            tracing::warn!("could not clear the GitHub Copilot token cache: {error}");
        }
        ProviderError::Authentication(format!(
            "GitHub Copilot rejected the saved GitHub sign-in ({status}), so it was removed; the next request will start GitHub sign-in again, or run `gosling configure` and select GitHub Copilot to sign in now"
        ))
    }

    async fn refresh_api_info(&self, sign_in: SignIn) -> Result<CopilotTokenInfo> {
        let config = Config::global();
        let token = match config.get_secret::<String>(&self.token_secret_key) {
            Ok(token) => token,
            Err(ConfigError::NotFound(_)) if sign_in == SignIn::Allowed => {
                let token = self
                    .get_access_token()
                    .await
                    .context("unable to login into github")?;
                config.set_secret(&self.token_secret_key, &token)?;
                token
            }
            Err(err) => return Err(err.into()),
        };
        let resp = self
            .client
            .get(&self.urls.copilot_token_url)
            .headers(self.get_github_headers())
            .header(http::header::AUTHORIZATION, format!("bearer {}", &token))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let info: CopilotTokenInfo = serde_json::from_str(&resp)?;
        Ok(info)
    }

    async fn get_access_token(&self) -> Result<String> {
        for attempt in 0..3 {
            tracing::trace!("attempt {} to get access token", attempt + 1);
            match self.login().await {
                Ok(token) => return Ok(token),
                Err(err) => tracing::warn!("failed to get access token: {}", err),
            }
        }
        Err(anyhow!("failed to get access token after 3 attempts"))
    }

    async fn login(&self) -> Result<String> {
        let cfg = DeviceFlowConfig {
            device_auth_url: Some(&self.urls.device_code_url),
            token_url: &self.urls.access_token_url,
            client_id: &self.client_id,
            scopes: Some("read:user"),
            extra_headers: self.get_github_headers(),
            encoding: RequestEncoding::Json,
        };
        let tokens = run_device_flow(&self.client, &cfg).await?;
        Ok(tokens.access_token)
    }

    fn get_github_headers(&self) -> http::HeaderMap {
        let mut headers = http::HeaderMap::new();
        headers.insert(http::header::ACCEPT, "application/json".parse().unwrap());
        headers.insert(
            http::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(
            http::header::USER_AGENT,
            "GithubCopilot/1.155.0".parse().unwrap(),
        );
        headers.insert("editor-version", "vscode/1.85.1".parse().unwrap());
        headers.insert("editor-plugin-version", "copilot/1.155.0".parse().unwrap());
        headers
    }

    #[allow(clippy::too_many_arguments)]
    async fn stream_responses(
        &self,
        model_config: &ModelConfig,
        is_user_initiated: bool,
        system: &str,
        messages: &[Message],
        tools: &[Tool],
        has_images: bool,
    ) -> Result<MessageStream, ProviderError> {
        let mut payload = create_responses_request(model_config, system, messages, tools)
            .map_err(|e| ProviderError::RequestFailed(e.to_string()))?;
        payload["stream"] = serde_json::Value::Bool(true);

        let mut log = start_log(model_config, &payload)?;

        let response = self
            .with_retry(|| async {
                let mut payload_clone = payload.clone();
                let resp = self
                    .post(
                        "responses",
                        is_user_initiated,
                        &mut payload_clone,
                        has_images,
                    )
                    .await?;
                handle_status(resp).await
            })
            .await
            .inspect_err(|e| {
                let _ = log.error(e);
            })?;

        stream_responses_compat(response, log)
    }

    #[allow(clippy::too_many_arguments)]
    async fn stream_chat_completions(
        &self,
        model_config: &ModelConfig,
        is_user_initiated: bool,
        system: &str,
        messages: &[Message],
        tools: &[Tool],
        has_images: bool,
    ) -> Result<MessageStream, ProviderError> {
        let supports_streaming = GITHUB_COPILOT_STREAM_MODELS
            .iter()
            .any(|prefix| model_config.model_name.starts_with(prefix));

        if supports_streaming {
            let payload = create_request(
                model_config,
                system,
                messages,
                tools,
                &ImageFormat::OpenAi,
                true,
            )?;
            let mut log = start_log(model_config, &payload)?;

            let response = self
                .with_retry(|| async {
                    let mut payload_clone = payload.clone();
                    let resp = self
                        .post(
                            "chat/completions",
                            is_user_initiated,
                            &mut payload_clone,
                            has_images,
                        )
                        .await?;
                    handle_status(resp).await
                })
                .await
                .inspect_err(|e| {
                    let _ = log.error(e);
                })?;

            stream_openai_compat(response, log)
        } else {
            let payload = create_request(
                model_config,
                system,
                messages,
                tools,
                &ImageFormat::OpenAi,
                false,
            )?;
            let mut log = start_log(model_config, &payload)?;

            let response = self
                .with_retry(|| async {
                    let mut payload_clone = payload.clone();
                    self.post(
                        "chat/completions",
                        is_user_initiated,
                        &mut payload_clone,
                        has_images,
                    )
                    .await
                })
                .await?;
            let response = handle_response_openai_compat(response).await?;

            let response = promote_tool_choice(response);

            let message = response_to_message(&response)?;
            let usage = response.get("usage").map(get_usage).unwrap_or_else(|| {
                tracing::debug!("Failed to get usage data");
                Usage::default()
            });
            let response_model = get_model(&response);
            log.write(&response, Some(&usage))?;

            Ok(super::base::stream_from_single_message(
                message,
                ProviderUsage::new(response_model, usage),
            ))
        }
    }
}

impl gosling_providers::base::ProviderDescriptor for GithubCopilotProvider {
    fn metadata() -> ProviderMetadata {
        ProviderMetadata::new(
            GITHUB_COPILOT_PROVIDER_NAME,
            "GitHub Copilot",
            "GitHub Copilot. Run `gosling configure` and select copilot to set up.",
            GITHUB_COPILOT_DEFAULT_MODEL,
            GITHUB_COPILOT_KNOWN_MODELS.to_vec(),
            GITHUB_COPILOT_DOC_URL,
            vec![
                ConfigKey::new_oauth_device_code("GITHUB_COPILOT_TOKEN", true, true, None, false),
                ConfigKey::new("GITHUB_COPILOT_HOST", false, false, None, false),
                ConfigKey::new("GITHUB_COPILOT_CLIENT_ID", false, false, None, false),
                ConfigKey::new("GITHUB_COPILOT_TOKEN_URL", false, false, None, false),
            ],
        )
    }
}

impl ProviderDef for GithubCopilotProvider {
    type Provider = Self;

    fn from_env(
        _extensions: Vec<crate::config::ExtensionConfig>,
        tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> BoxFuture<'static, Result<Self::Provider>> {
        Box::pin(Self::from_env(tls_config))
    }
}

#[async_trait]
impl Provider for GithubCopilotProvider {
    fn get_name(&self) -> &str {
        &self.name
    }

    async fn complete(
        &self,
        model_config: &ModelConfig,
        system: &str,
        messages: &[Message],
        tools: &[Tool],
    ) -> Result<(Message, ProviderUsage), ProviderError> {
        IS_AGENT_CALL
            .scope(true, async {
                collect_stream(self.stream(model_config, system, messages, tools).await?).await
            })
            .await
    }

    async fn stream(
        &self,
        model_config: &ModelConfig,
        system: &str,
        messages: &[Message],
        tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        let is_agent_call = IS_AGENT_CALL.try_with(|&v| v).unwrap_or(false);
        let last_is_tool_response = messages.last().is_some_and(|m| {
            m.content
                .iter()
                .any(|c| matches!(c, MessageContent::ToolResponse(_)))
        });
        let is_user_initiated = !is_agent_call && !last_is_tool_response;
        let has_images = Self::messages_contain_image(messages);

        if is_openai_responses_model(&model_config.model_name) {
            self.stream_responses(
                model_config,
                is_user_initiated,
                system,
                messages,
                tools,
                has_images,
            )
            .await
        } else {
            self.stream_chat_completions(
                model_config,
                is_user_initiated,
                system,
                messages,
                tools,
                has_images,
            )
            .await
        }
    }

    async fn fetch_supported_models(&self) -> Result<Vec<String>, ProviderError> {
        let (endpoint, token) = self.get_api_info(SignIn::Never).await?;
        let url = format!("{}/models", endpoint);

        let mut headers = http::HeaderMap::new();
        headers.insert(http::header::ACCEPT, "application/json".parse().unwrap());
        headers.insert(
            http::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert("Copilot-Integration-Id", "vscode-chat".parse().unwrap());
        headers.insert(
            http::header::AUTHORIZATION,
            format!("Bearer {}", token).parse().unwrap(),
        );

        let response = self.client.get(url).headers(headers).send().await?;
        let response = handle_status(response).await?;

        let json: serde_json::Value = response.json().await?;

        let arr = json.get("data").and_then(|v| v.as_array()).ok_or_else(|| {
            ProviderError::RequestFailed(
                "Missing 'data' array in GitHub Copilot models response".to_string(),
            )
        })?;
        let mut models: Vec<String> = arr
            .iter()
            .filter_map(|m| {
                if let Some(s) = m.as_str() {
                    Some(s.to_string())
                } else if let Some(obj) = m.as_object() {
                    obj.get("id").and_then(|v| v.as_str()).map(str::to_string)
                } else {
                    None
                }
            })
            .collect();
        models.sort();
        Ok(models)
    }

    async fn configure_oauth(&self) -> Result<(), ProviderError> {
        let config = Config::global();

        if config.get_secret::<String>(&self.token_secret_key).is_ok() {
            match self.refresh_api_info(SignIn::Never).await {
                Ok(_) => return Ok(()),
                Err(_) => {
                    tracing::debug!("Existing token is invalid, starting OAuth flow");
                }
            }
        }

        let token = self
            .get_access_token()
            .await
            .map_err(|e| ProviderError::Authentication(format!("OAuth flow failed: {}", e)))?;

        config
            .set_secret(&self.token_secret_key, &token)
            .map_err(|e| ProviderError::ExecutionError(format!("Failed to save token: {}", e)))?;

        Ok(())
    }
}

// Copilot sometimes returns multiple choices in a completion response for
// Claude models and places the `tool_calls` payload in a non-zero index choice.
// This function ensures the first choice contains tool metadata so the shared formatter emits a
// `ToolRequest` instead of returning only the plain-text choice.
fn promote_tool_choice(response: Value) -> Value {
    let Some(choices) = response.get("choices").and_then(|c| c.as_array()) else {
        return response;
    };

    let tool_choice_idx = choices.iter().position(|choice| {
        choice
            .get("message")
            .and_then(|m| m.get("tool_calls"))
            .and_then(|tc| tc.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false)
    });

    if let Some(idx) = tool_choice_idx {
        if idx != 0 {
            let mut new_response = response;
            if let Some(new_choices) = new_response
                .get_mut("choices")
                .and_then(|c| c.as_array_mut())
            {
                let choice = new_choices.remove(idx);
                new_choices.insert(0, choice);
            }
            return new_response;
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[cfg(unix)]
    #[tokio::test]
    async fn disk_cache_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().unwrap();
        let cache = DiskCache {
            cache_path: temp_dir.path().join("copilot.json"),
        };
        let state = CopilotState {
            expires_at: Utc::now(),
            info: CopilotTokenInfo {
                token: "secret".to_string(),
                expires_at: 1,
                refresh_in: 1,
                endpoints: CopilotTokenEndpoints {
                    api: "https://api.github.com".to_string(),
                    _extra: HashMap::new(),
                },
                _extra: HashMap::new(),
            },
        };

        cache.save(&state).await.unwrap();

        let mode = std::fs::metadata(&cache.cache_path)
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn responses_models_routed_correctly() {
        assert!(is_openai_responses_model("gpt-5.5"));
        assert!(is_openai_responses_model("gpt-5.4"));
        assert!(is_openai_responses_model("gpt-5"));
        assert!(is_openai_responses_model("gpt-5-mini"));
        assert!(is_openai_responses_model("gpt-5-codex"));
        assert!(is_openai_responses_model("o3"));
        assert!(is_openai_responses_model("o3-mini"));

        assert!(!is_openai_responses_model("gpt-4.1"));
        assert!(!is_openai_responses_model("gpt-4o"));
        assert!(!is_openai_responses_model("claude-sonnet-4"));
        assert!(!is_openai_responses_model("claude-haiku-4.5"));
        assert!(!is_openai_responses_model("gemini-2.5-pro"));
    }

    #[test]
    fn detects_images_in_messages() {
        use crate::conversation::message::Message;

        let messages_with_image = vec![Message::user()
            .with_text("describe this")
            .with_image("base64data", "image/png")];
        assert!(GithubCopilotProvider::messages_contain_image(
            &messages_with_image
        ));

        let messages_without_image = vec![Message::user().with_text("plain text")];
        assert!(!GithubCopilotProvider::messages_contain_image(
            &messages_without_image
        ));
    }

    #[test]
    fn detects_images_in_tool_responses() {
        use crate::conversation::message::{Message, MessageContent};
        use rmcp::model::{CallToolResult, Content};

        let image_content = Content::image("aW1hZ2VkYXRh".to_string(), "image/png".to_string());
        let tool_result = Ok(CallToolResult::success(vec![image_content]));

        let messages =
            vec![Message::user()
                .with_content(MessageContent::tool_response("call_123", tool_result))];
        assert!(GithubCopilotProvider::messages_contain_image(&messages));

        let text_result = Ok(CallToolResult::success(vec![Content::text("no images")]));
        let messages_text_only =
            vec![Message::user()
                .with_content(MessageContent::tool_response("call_456", text_result))];
        assert!(!GithubCopilotProvider::messages_contain_image(
            &messages_text_only
        ));
    }

    #[test]
    fn promotes_choice_with_tool_call() {
        let response = json!({
            "choices": [
                {"message": {"content": "plain text"}},
                {"message": {"tool_calls": [{"function": {"name": "foo", "arguments": "{}"}}]}}
            ]
        });

        let promoted = promote_tool_choice(response);
        assert_eq!(
            promoted
                .get("choices")
                .and_then(|c| c.as_array())
                .map(|c| c.len()),
            Some(2)
        );
        let first_choice = promoted
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|c| c.first())
            .unwrap();

        assert!(first_choice
            .get("message")
            .and_then(|m| m.get("tool_calls"))
            .is_some());
    }

    #[test]
    fn leaves_response_when_tool_choice_first() {
        let response = json!({
            "choices": [
                {"message": {"tool_calls": [{"function": {"name": "foo", "arguments": "{}"}}]}},
                {"message": {"content": "plain text"}}
            ]
        });

        let promoted = promote_tool_choice(response.clone());
        assert_eq!(promoted, response);
    }

    #[test]
    fn normalize_host_strips_prefix_and_slash() {
        assert_eq!(normalize_host("github.com"), "github.com");
        assert_eq!(normalize_host("https://github.com"), "github.com");
        assert_eq!(normalize_host("github.com/"), "github.com");
        assert_eq!(normalize_host("https://github.com/"), "github.com");
        assert_eq!(
            normalize_host("https://my-enterprise.ghe.com/"),
            "my-enterprise.ghe.com"
        );
    }

    #[test]
    fn urls_default_github_com() {
        let urls = GithubCopilotUrls::new("github.com", None);
        assert_eq!(urls.device_code_url, "https://github.com/login/device/code");
        assert_eq!(
            urls.access_token_url,
            "https://github.com/login/oauth/access_token"
        );
        assert_eq!(
            urls.copilot_token_url,
            "https://api.github.com/copilot_internal/v2/token"
        );
    }

    #[test]
    fn urls_enterprise_host() {
        let urls = GithubCopilotUrls::new("my-enterprise.ghe.com", None);
        assert_eq!(
            urls.device_code_url,
            "https://my-enterprise.ghe.com/login/device/code"
        );
        assert_eq!(
            urls.access_token_url,
            "https://my-enterprise.ghe.com/login/oauth/access_token"
        );
        assert_eq!(
            urls.copilot_token_url,
            "https://api.my-enterprise.ghe.com/copilot_internal/v2/token"
        );
    }

    const TEST_TOKEN_SECRET_KEY: &str = "GITHUB_COPILOT_TOKEN__copilot_test_host";

    fn provider_against(
        server: &wiremock::MockServer,
        cache_dir: &std::path::Path,
    ) -> GithubCopilotProvider {
        GithubCopilotProvider {
            client: Client::new(),
            cache: DiskCache {
                cache_path: cache_dir.join("info.json"),
            },
            mu: tokio::sync::Mutex::new(RefCell::new(None)),
            urls: GithubCopilotUrls {
                device_code_url: format!("{}/login/device/code", server.uri()),
                access_token_url: format!("{}/login/oauth/access_token", server.uri()),
                copilot_token_url: format!("{}/copilot_internal/v2/token", server.uri()),
            },
            client_id: "test-client".to_string(),
            name: GITHUB_COPILOT_PROVIDER_NAME.to_string(),
            tls_config: None,
            token_secret_key: TEST_TOKEN_SECRET_KEY.to_string(),
        }
    }

    // The device-code endpoint answers 500, so an attempted sign-in fails before
    // it could open a browser or touch the clipboard.
    async fn mount_failing_device_code(
        server: &wiremock::MockServer,
        expected_calls: impl Into<wiremock::Times>,
    ) {
        use wiremock::matchers::{method, path};
        wiremock::Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(wiremock::ResponseTemplate::new(500))
            .expect(expected_calls)
            .mount(server)
            .await;
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn listing_models_without_a_sign_in_never_starts_the_device_flow() {
        let root = tempfile::tempdir().unwrap();
        let root_path = root.path().to_string_lossy().to_string();
        let _env = env_lock::lock_env([("GOSLING_PATH_ROOT", Some(root_path.as_str()))]);
        let server = wiremock::MockServer::start().await;
        mount_failing_device_code(&server, 0).await;
        let provider = provider_against(&server, root.path());

        let error = provider.fetch_supported_models().await.unwrap_err();

        assert_eq!(
            error,
            ProviderError::Authentication(GITHUB_COPILOT_SIGN_IN_REQUIRED.to_string())
        );
        server.verify().await;
        assert!(Config::global()
            .get_secret::<String>(TEST_TOKEN_SECRET_KEY)
            .is_err());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn listing_models_with_a_stored_sign_in_exchanges_it_without_the_device_flow() {
        use wiremock::matchers::{header, method, path};
        let root = tempfile::tempdir().unwrap();
        let root_path = root.path().to_string_lossy().to_string();
        let _env = env_lock::lock_env([("GOSLING_PATH_ROOT", Some(root_path.as_str()))]);
        let server = wiremock::MockServer::start().await;
        mount_failing_device_code(&server, 0).await;
        wiremock::Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .and(header("authorization", "bearer ghu_stored"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "token": "copilot-api-token",
                "expires_at": 4_102_444_800_i64,
                "refresh_in": 1500,
                "endpoints": {"api": server.uri()}
            })))
            .expect(1)
            .mount(&server)
            .await;
        wiremock::Mock::given(method("GET"))
            .and(path("/models"))
            .and(header("authorization", "Bearer copilot-api-token"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(json!({
                "data": [{"id": "gpt-4o"}, {"id": "claude-sonnet-4"}]
            })))
            .mount(&server)
            .await;
        Config::global()
            .set_secret(TEST_TOKEN_SECRET_KEY, &"ghu_stored".to_string())
            .unwrap();
        let provider = provider_against(&server, root.path());

        let models = provider.fetch_supported_models().await.unwrap();

        assert_eq!(models, vec!["claude-sonnet-4", "gpt-4o"]);
        server.verify().await;
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn a_request_without_a_sign_in_still_starts_the_device_flow() {
        let root = tempfile::tempdir().unwrap();
        let root_path = root.path().to_string_lossy().to_string();
        let _env = env_lock::lock_env([("GOSLING_PATH_ROOT", Some(root_path.as_str()))]);
        let server = wiremock::MockServer::start().await;
        let provider = provider_against(&server, root.path());
        mount_failing_device_code(&server, 1..).await;

        let error = provider
            .get_api_info(SignIn::Allowed)
            .await
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("failed to get api info after 3 attempts"),
            "{error}"
        );
        server.verify().await;
    }

    async fn provider_with_stored_sign_in_and_token_status(
        server: &wiremock::MockServer,
        cache_dir: &std::path::Path,
        status: u16,
        expected_exchanges: u64,
    ) -> GithubCopilotProvider {
        use wiremock::matchers::{method, path};
        mount_failing_device_code(server, 0).await;
        wiremock::Mock::given(method("GET"))
            .and(path("/copilot_internal/v2/token"))
            .respond_with(
                wiremock::ResponseTemplate::new(status)
                    .set_body_json(json!({"message": "Bad credentials"})),
            )
            .expect(expected_exchanges)
            .mount(server)
            .await;
        Config::global()
            .set_secret(TEST_TOKEN_SECRET_KEY, &"ghu_rejected".to_string())
            .unwrap();
        provider_against(server, cache_dir)
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn a_rejected_sign_in_asks_for_reauthentication_once() {
        for (status, reason) in [(401, "401 Unauthorized"), (403, "403 Forbidden")] {
            let root = tempfile::tempdir().unwrap();
            let root_path = root.path().to_string_lossy().to_string();
            let _env = env_lock::lock_env([("GOSLING_PATH_ROOT", Some(root_path.as_str()))]);
            let server = wiremock::MockServer::start().await;
            let provider =
                provider_with_stored_sign_in_and_token_status(&server, root.path(), status, 1)
                    .await;

            let error = provider.get_api_info(SignIn::Allowed).await.unwrap_err();

            assert_eq!(
                error,
                ProviderError::Authentication(format!(
                    "GitHub Copilot rejected the saved GitHub sign-in ({reason}), so it was removed; the next request will start GitHub sign-in again, or run `gosling configure` and select GitHub Copilot to sign in now"
                ))
            );
            assert!(
                Config::global()
                    .get_secret::<String>(TEST_TOKEN_SECRET_KEY)
                    .is_err(),
                "the rejected sign-in must be removed"
            );
            server.verify().await;
        }
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn a_failing_token_exchange_is_still_retried() {
        let root = tempfile::tempdir().unwrap();
        let root_path = root.path().to_string_lossy().to_string();
        let _env = env_lock::lock_env([("GOSLING_PATH_ROOT", Some(root_path.as_str()))]);
        let server = wiremock::MockServer::start().await;
        let provider =
            provider_with_stored_sign_in_and_token_status(&server, root.path(), 500, 3).await;

        let error = provider.get_api_info(SignIn::Allowed).await.unwrap_err();

        assert_eq!(
            error,
            ProviderError::ExecutionError("failed to get api info after 3 attempts".to_string())
        );
        assert!(
            Config::global()
                .get_secret::<String>(TEST_TOKEN_SECRET_KEY)
                .is_ok(),
            "a server error must not remove the saved sign-in"
        );
        server.verify().await;
    }

    #[test]
    fn urls_enterprise_with_token_url_override() {
        let urls = GithubCopilotUrls::new(
            "my-enterprise.ghe.com",
            Some("https://my-enterprise.ghe.com/api/v3/copilot_internal/v2/token"),
        );
        assert_eq!(
            urls.copilot_token_url,
            "https://my-enterprise.ghe.com/api/v3/copilot_internal/v2/token"
        );
    }
}
