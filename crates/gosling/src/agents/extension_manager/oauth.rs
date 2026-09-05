// Owns OAuth fallback and streamable HTTP or unix-socket MCP transport setup.
// Extension lifecycle supplies resolved configuration and receives a connected client.
// The extension_manager compatibility facade keeps transport helpers non-public.

use super::*;

fn is_oauth_auth_failure(err: &ClientInitializeError) -> bool {
    let ClientInitializeError::TransportError {
        error: DynamicTransportError { error, .. },
        ..
    } = err
    else {
        return false;
    };

    if let Some(http_err) = error.downcast_ref::<StreamableHttpError<reqwest::Error>>() {
        return match http_err {
            StreamableHttpError::AuthRequired(_) => true,
            StreamableHttpError::UnexpectedServerResponse(body) => body.starts_with("HTTP 401"),
            _ => false,
        };
    }

    #[cfg(unix)]
    if let Some(http_err) = error
        .downcast_ref::<StreamableHttpError<rmcp::transport::common::unix_socket::UnixSocketError>>(
        )
    {
        return match http_err {
            StreamableHttpError::AuthRequired(_) => true,
            StreamableHttpError::UnexpectedServerResponse(body) => body.starts_with("HTTP 401"),
            _ => false,
        };
    }

    error
        .to_string()
        .contains("unexpected server response: HTTP 401")
}

pub(super) fn should_attempt_oauth_fallback(
    res: &Result<McpClient, ClientInitializeError>,
) -> bool {
    res.as_ref().err().is_some_and(is_oauth_auth_failure)
}

/// An extension configured with its own `Authorization` header authenticates
/// with a static credential rather than delegating to OAuth, so a 401 there is
/// a credential problem no browser authorization flow can resolve.
pub(super) fn has_static_authorization_header(headers: &HashMap<String, String>) -> bool {
    headers
        .keys()
        .any(|key| key.eq_ignore_ascii_case("authorization"))
}

/// A server cannot distinguish an absent header from an empty or wrong one — all
/// three come back as the same 401 — so the message names every cause rather
/// than guessing at one.
fn static_credential_rejected(name: &str) -> ExtensionError {
    ExtensionError::ConfigError(format!(
        "extension '{name}' was rejected by the server (HTTP 401) with the Authorization \
         header it is configured to send. Check that the credential is set and current. \
         An environment variable referenced by the header that is unset in this process \
         expands to an empty value, which the server rejects exactly as it rejects a wrong \
         one. Not starting a browser OAuth flow, because this extension authenticates with \
         a static header."
    ))
}

pub(super) async fn clear_credentials_on_post_refresh_auth_failure(
    credential_store: &dyn CredentialStore,
    name: &str,
    error: &ExtensionError,
) -> bool {
    let ExtensionError::InitializeError(err) = error else {
        return false;
    };

    if !is_oauth_auth_failure(err) {
        return false;
    }

    if let Err(e) = credential_store.clear().await {
        warn!(
            "[OAuth:{}] error clearing rejected credentials: {}",
            name, e
        );
    }
    true
}

/// Merge environment variables from direct envs and keychain-stored env_keys
const GOSLING_USER_AGENT: reqwest::header::HeaderValue =
    reqwest::header::HeaderValue::from_static(concat!("gosling/", env!("CARGO_PKG_VERSION")));

#[allow(clippy::too_many_arguments)]
pub(super) async fn connect_with_auth(
    auth_manager: rmcp::transport::AuthorizationManager,
    uri: &str,
    timeout: Duration,
    headers: &HashMap<String, String>,
    provider: SharedProvider,
    client_name: String,
    capabilities: GoslingMcpClientCapabilities,
    roots_dir: &std::path::Path,
) -> ExtensionResult<Box<dyn McpClientTrait>> {
    let mut auth_headers = HeaderMap::new();
    auth_headers.insert(reqwest::header::USER_AGENT, GOSLING_USER_AGENT);
    for (key, value) in headers {
        auth_headers.insert(
            HeaderName::try_from(key)
                .map_err(|_| ExtensionError::ConfigError(format!("invalid header: {}", key)))?,
            value.parse().map_err(|_| {
                ExtensionError::ConfigError(format!("invalid header value: {}", key))
            })?,
        );
    }
    #[allow(unused_mut)]
    let mut auth_client_builder = reqwest::Client::builder().default_headers(auth_headers);
    #[cfg(target_os = "linux")]
    {
        auth_client_builder = auth_client_builder.tcp_user_timeout(Some(timeout));
    }
    let auth_http_client = auth_client_builder
        .build()
        .map_err(|_| ExtensionError::ConfigError("could not construct http client".to_string()))?;
    let auth_client = AuthClient::new(auth_http_client, auth_manager);
    let transport = StreamableHttpClientTransport::with_client(
        auth_client,
        StreamableHttpClientTransportConfig::with_uri(uri),
    );
    Ok(Box::new(
        McpClient::connect(
            transport,
            timeout,
            provider,
            client_name,
            capabilities,
            roots_dir.to_path_buf(),
        )
        .await?,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn create_streamable_http_client(
    uri: &str,
    timeout: Option<u64>,
    headers: &HashMap<String, String>,
    name: &str,
    socket: Option<&str>,
    static_oauth_client: Option<StaticOAuthClientConfig>,
    credential_store: Box<dyn CredentialStore>,
    provider: SharedProvider,
    client_name: String,
    capabilities: GoslingMcpClientCapabilities,
    roots_dir: &std::path::Path,
) -> ExtensionResult<Box<dyn McpClientTrait>> {
    #[cfg(unix)]
    if let Some(socket_path) = socket {
        return create_unix_socket_http_client(
            uri,
            timeout,
            headers,
            name,
            socket_path,
            provider,
            client_name,
            capabilities,
            roots_dir,
        )
        .await;
    }
    #[cfg(not(unix))]
    if socket.is_some() {
        return Err(ExtensionError::ConfigError(
            "Unix domain socket transport is not supported on this platform".to_string(),
        ));
    }

    let mut default_headers = HeaderMap::new();

    default_headers.insert(reqwest::header::USER_AGENT, GOSLING_USER_AGENT);

    for (key, value) in headers {
        default_headers.insert(
            HeaderName::try_from(key)
                .map_err(|_| ExtensionError::ConfigError(format!("invalid header: {}", key)))?,
            value.parse().map_err(|_| {
                ExtensionError::ConfigError(format!("invalid header value: {}", key))
            })?,
        );
    }

    let timeout_duration = Duration::from_secs(resolve_timeout(timeout));

    let http_client = build_streamable_http_client(default_headers, timeout_duration)
        .map_err(|_| ExtensionError::ConfigError("could not construct http client".to_string()))?;

    let transport = StreamableHttpClientTransport::with_client(
        http_client,
        StreamableHttpClientTransportConfig::with_uri(uri),
    );

    // If we have stored OAuth credentials, try refreshing and connecting directly.
    // This avoids the unnecessary 401 → browser re-auth cycle on every new session.
    if credential_store.load().await.is_ok_and(|c| c.is_some()) {
        match oauth_flow(
            &uri.to_string(),
            &name.to_string(),
            static_oauth_client.as_ref(),
        )
        .await
        {
            Ok(auth_manager) => {
                let auth_result = connect_with_auth(
                    auth_manager,
                    uri,
                    timeout_duration,
                    headers,
                    provider.clone(),
                    client_name.clone(),
                    capabilities.clone(),
                    roots_dir,
                )
                .await;

                if let Err(error) = &auth_result {
                    if clear_credentials_on_post_refresh_auth_failure(
                        credential_store.as_ref(),
                        name,
                        error,
                    )
                    .await
                    {
                        warn!(
                            "[OAuth:{}] Refreshed token was rejected, falling back to browser auth",
                            name
                        );
                    } else {
                        return auth_result;
                    }
                } else {
                    return auth_result;
                }
            }
            Err(e) => {
                warn!(
                    "[OAuth:{}] Proactive refresh failed: {}, falling back to unauthenticated attempt",
                    name, e
                );
            }
        }
    }

    let client_res = McpClient::connect(
        transport,
        timeout_duration,
        provider.clone(),
        client_name.clone(),
        capabilities.clone(),
        roots_dir.to_path_buf(),
    )
    .await;

    if should_attempt_oauth_fallback(&client_res) {
        if has_static_authorization_header(headers) {
            return Err(static_credential_rejected(name));
        }
        match oauth_flow(
            &uri.to_string(),
            &name.to_string(),
            static_oauth_client.as_ref(),
        )
        .await
        {
            Ok(auth_manager) => {
                connect_with_auth(
                    auth_manager,
                    uri,
                    timeout_duration,
                    headers,
                    provider,
                    client_name,
                    capabilities,
                    roots_dir,
                )
                .await
            }
            Err(_) => Ok(Box::new(client_res?)),
        }
    } else {
        Ok(Box::new(client_res?))
    }
}

/// Streamable HTTP keeps a long-lived SSE response body open to deliver server
/// messages, so a whole-request deadline would tear that stream down (and every
/// in-flight request riding on it) once the extension timeout elapsed. The
/// per-request deadline is enforced by `McpClient` instead, leaving only the
/// connect phase to bound here.
pub(super) fn build_streamable_http_client(
    default_headers: HeaderMap,
    timeout_duration: Duration,
) -> reqwest::Result<reqwest::Client> {
    #[allow(unused_mut)]
    let mut builder = reqwest::Client::builder()
        .default_headers(default_headers)
        .connect_timeout(timeout_duration);
    #[cfg(target_os = "linux")]
    {
        builder = builder.tcp_user_timeout(Some(timeout_duration));
    }
    builder.build()
}

#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
async fn create_unix_socket_http_client(
    uri: &str,
    timeout: Option<u64>,
    headers: &HashMap<String, String>,
    name: &str,
    socket_path: &str,
    provider: SharedProvider,
    client_name: String,
    capabilities: GoslingMcpClientCapabilities,
    roots_dir: &std::path::Path,
) -> ExtensionResult<Box<dyn McpClientTrait>> {
    use rmcp::transport::UnixSocketHttpClient;

    let unix_client = UnixSocketHttpClient::new(socket_path, uri);

    let mut custom_headers = std::collections::HashMap::<HeaderName, HeaderValue>::new();

    custom_headers.insert(
        HeaderName::from_static("user-agent"),
        GOSLING_USER_AGENT
            .to_str()
            .unwrap_or("gosling")
            .parse()
            .unwrap_or_else(|_| HeaderValue::from_static("gosling")),
    );

    for (key, value) in headers {
        let header_name = HeaderName::try_from(key)
            .map_err(|_| ExtensionError::ConfigError(format!("invalid header: {}", key)))?;
        let header_value = value
            .parse::<HeaderValue>()
            .map_err(|_| ExtensionError::ConfigError(format!("invalid header value: {}", key)))?;
        custom_headers.insert(header_name, header_value);
    }

    let config = StreamableHttpClientTransportConfig::with_uri(uri).custom_headers(custom_headers);
    let transport = StreamableHttpClientTransport::with_client(unix_client, config);

    let timeout_duration = Duration::from_secs(resolve_timeout(timeout));

    let client_res = McpClient::connect(
        transport,
        timeout_duration,
        provider.clone(),
        client_name.clone(),
        capabilities.clone(),
        roots_dir.to_path_buf(),
    )
    .await;

    if should_attempt_oauth_fallback(&client_res) {
        tracing::warn!(
            "Extension '{}' returned 401 over Unix domain socket transport; \
             OAuth is not supported for UDS connections",
            name,
        );
    }
    Ok(Box::new(client_res?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn a_configured_authorization_header_is_detected_whatever_its_casing() {
        assert!(has_static_authorization_header(&headers(&[(
            "Authorization",
            "Bearer t"
        )])));
        assert!(has_static_authorization_header(&headers(&[(
            "authorization",
            "Bearer t"
        )])));
        assert!(has_static_authorization_header(&headers(&[(
            "AUTHORIZATION",
            "Bearer t"
        )])));
    }

    #[test]
    fn other_headers_still_leave_oauth_available() {
        assert!(!has_static_authorization_header(&headers(&[])));
        assert!(!has_static_authorization_header(&headers(&[(
            "X-Api-Key",
            "k"
        )])));
    }

    /// An unexpanded `${VAR}` reaching the server is the failure this guard was
    /// written for, and it is still a configured header.
    #[test]
    fn an_unexpanded_placeholder_still_counts_as_a_static_credential() {
        assert!(has_static_authorization_header(&headers(&[(
            "Authorization",
            "Bearer ${MUNINN_MCP_BEARER_TOKEN}"
        )])));
        assert!(has_static_authorization_header(&headers(&[(
            "Authorization",
            "Bearer "
        )])));
    }

    #[test]
    fn the_rejection_message_names_the_extension_and_rules_out_oauth() {
        let rendered = static_credential_rejected("muninn").to_string();
        assert!(rendered.contains("muninn"));
        assert!(rendered.contains("HTTP 401"));
        assert!(rendered.contains("unset"));
        assert!(rendered.contains("Not starting a browser OAuth flow"));
    }
}
