use axum::{
    extract::{ConnectInfo, DefaultBodyLimit, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::RwLock;
use uuid::Uuid;

const GUEST_HTML_TTL_SECS: u64 = 300;
const GUEST_HTML_MAX_ENTRIES: usize = 64;
const GUEST_HTML_MAX_BYTES: usize = 16 * 1024 * 1024;
const MCP_APP_PROXY_HTML: &str = include_str!("templates/mcp_app_proxy.html");

type GuestHtmlStore = Arc<RwLock<HashMap<String, GuestHtmlEntry>>>;

/// Guest CSPs the server computed at proxy-render time, keyed by the
/// single-use token handed to that proxy page.
///
/// The guest CSP used to be posted back by the proxy page, which read its own
/// `<meta>` policy and rewrote `'nonce-...'` to `'unsafe-inline'` before
/// sending it. Anything running in that frame could send a different string
/// instead, and the server installed it verbatim — the policy meant to bound
/// the guest was supplied by the very context it bounds. The server now derives
/// it from the same declared domains it already uses for the outer CSP, and the
/// page only presents a token. (SEC-GOS-002)
type ProxyCspStore = Arc<RwLock<HashMap<String, ProxyCspEntry>>>;

#[derive(Clone)]
struct ProxyCspEntry {
    guest_csp: String,
    created: Instant,
}

#[derive(Clone)]
struct GuestHtmlEntry {
    html: String,
    csp: String,
    created: Instant,
}

#[derive(Deserialize)]
struct ProxyQuery {
    connect_domains: Option<String>,
    resource_domains: Option<String>,
    frame_domains: Option<String>,
    base_uri_domains: Option<String>,
    script_domains: Option<String>,
}

#[derive(Deserialize)]
struct GuestQuery {
    nonce: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoreGuestBody {
    secret: String,
    html: String,
    /// Token minted by `/mcp-app-proxy` for this page. Names which
    /// server-computed guest CSP to apply; it does not carry a policy.
    proxy_token: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoreGuestResponse {
    nonce: String,
    guest_url: String,
}

#[derive(Clone)]
struct AppState {
    secret_key: String,
    guest_store: GuestHtmlStore,
    guest_base_url: String,
    proxy_csp_store: ProxyCspStore,
}

#[derive(Clone)]
struct GuestState {
    guest_store: GuestHtmlStore,
}

fn normalize_csp_source(source: &str) -> Option<String> {
    let source = source.trim();
    if source.is_empty()
        || source
            .chars()
            .any(|c| c.is_ascii_whitespace() || matches!(c, ';' | ',' | '"' | '\''))
    {
        return None;
    }

    if let Some((scheme, rest)) = source.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        if !matches!(scheme.as_str(), "http" | "https" | "ws" | "wss") {
            return None;
        }

        let authority = rest.split(['/', '?', '#']).next()?;
        if !is_valid_csp_host_source(authority) {
            return None;
        }

        return Some(format!("{scheme}://{}", authority.to_ascii_lowercase()));
    }

    if is_valid_csp_host_source(source) {
        return Some(source.to_ascii_lowercase());
    }

    None
}

fn is_valid_csp_host_source(source: &str) -> bool {
    if source.is_empty() || source == "*" || source.contains('@') {
        return false;
    }

    let (host, port) = split_host_and_port(source);
    if host.is_empty() {
        return false;
    }
    if port.is_some_and(|port| port.is_empty() || port.parse::<u16>().is_err()) {
        return false;
    }

    let host = host.strip_prefix("*.").unwrap_or(host);
    if host.eq_ignore_ascii_case("localhost")
        || host.parse::<std::net::Ipv4Addr>().is_ok()
        || host.parse::<std::net::Ipv6Addr>().is_ok()
    {
        return true;
    }

    !host.is_empty()
        && host.contains('.')
        && host
            .split('.')
            .all(|label| is_valid_dns_label(label) && label != "*")
}

fn split_host_and_port(source: &str) -> (&str, Option<&str>) {
    if let Some(remainder) = source.strip_prefix('[') {
        if let Some((host, tail)) = remainder.split_once(']') {
            let port = tail.strip_prefix(':');
            return (host, port);
        }
    }

    match source.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => (host, Some(port)),
        _ => (source, None),
    }
}

fn is_valid_dns_label(label: &str) -> bool {
    !label.is_empty()
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

fn peer_addr_is_loopback(peer_addr: &SocketAddr) -> bool {
    peer_addr.ip().is_loopback()
}

fn parse_domains(domains: Option<&String>) -> Vec<String> {
    domains
        .map(|domains| {
            domains
                .split(',')
                .filter_map(normalize_csp_source)
                .collect()
        })
        .unwrap_or_default()
}

fn build_outer_csp(
    connect_domains: &[String],
    resource_domains: &[String],
    frame_domains: &[String],
    base_uri_domains: &[String],
    script_domains: &[String],
    guest_origin: &str,
    script_inline_source: &str,
) -> String {
    let resources = if resource_domains.is_empty() {
        String::new()
    } else {
        format!(" {}", resource_domains.join(" "))
    };

    let scripts = if script_domains.is_empty() {
        String::new()
    } else {
        format!(" {}", script_domains.join(" "))
    };

    let connections = if connect_domains.is_empty() {
        String::new()
    } else {
        format!(" {}", connect_domains.join(" "))
    };

    let frame_src = if frame_domains.is_empty() {
        format!("frame-src 'self' {guest_origin}")
    } else {
        format!(
            "frame-src 'self' {guest_origin} {}",
            frame_domains.join(" ")
        )
    };

    let base_uris = if base_uri_domains.is_empty() {
        String::new()
    } else {
        format!(" {}", base_uri_domains.join(" "))
    };

    format!(
        "default-src 'none'; \
         script-src 'self' {script_inline_source}{resources}{scripts}; \
         script-src-elem 'self' {script_inline_source}{resources}{scripts}; \
         style-src 'self' 'unsafe-inline'{resources}; \
         style-src-elem 'self' 'unsafe-inline'{resources}; \
         connect-src 'self'{connections}; \
         img-src 'self' data: blob:{resources}; \
         font-src 'self'{resources}; \
         media-src 'self' data: blob:{resources}; \
         {frame_src}; \
         object-src 'none'; \
         base-uri 'self'{base_uris}"
    )
}

async fn mcp_app_proxy(
    State(state): State<AppState>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    Query(params): Query<ProxyQuery>,
) -> Response {
    if !peer_addr_is_loopback(&peer_addr) {
        return (
            StatusCode::BAD_REQUEST,
            "MCP app proxy is only available to loopback clients",
        )
            .into_response();
    }

    let script_nonce = Uuid::new_v4().simple().to_string();
    let script_inline_source = format!("'nonce-{script_nonce}'");

    let connect_domains = parse_domains(params.connect_domains.as_ref());
    let resource_domains = parse_domains(params.resource_domains.as_ref());
    let frame_domains = parse_domains(params.frame_domains.as_ref());
    let base_uri_domains = parse_domains(params.base_uri_domains.as_ref());
    let script_domains = parse_domains(params.script_domains.as_ref());

    let outer_csp = build_outer_csp(
        &connect_domains,
        &resource_domains,
        &frame_domains,
        &base_uri_domains,
        &script_domains,
        &state.guest_base_url,
        &script_inline_source,
    );

    // The guest runs the app's own markup and has no server-issued nonce, so
    // its policy allows inline scripts where the outer page pins a nonce.
    // Everything else is the same ceiling, derived from the same declared
    // domains — the guest cannot widen it, because it never supplies it.
    // (SEC-GOS-002)
    let guest_csp = build_outer_csp(
        &connect_domains,
        &resource_domains,
        &frame_domains,
        &base_uri_domains,
        &script_domains,
        &state.guest_base_url,
        "'unsafe-inline'",
    );

    let proxy_token = Uuid::new_v4().simple().to_string();
    {
        let mut store = state.proxy_csp_store.write().await;
        let cutoff = Instant::now() - Duration::from_secs(GUEST_HTML_TTL_SECS);
        store.retain(|_, entry| entry.created > cutoff);
        if store.len() >= GUEST_HTML_MAX_ENTRIES {
            if let Some(oldest) = store
                .iter()
                .min_by_key(|(_, entry)| entry.created)
                .map(|(key, _)| key.clone())
            {
                store.remove(&oldest);
            }
        }
        store.insert(
            proxy_token.clone(),
            ProxyCspEntry {
                guest_csp,
                created: Instant::now(),
            },
        );
    }

    let html = MCP_APP_PROXY_HTML
        .replace("{{OUTER_CSP}}", &outer_csp)
        .replace("{{SCRIPT_NONCE}}", &script_nonce)
        .replace("{{PROXY_TOKEN}}", &proxy_token);

    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (
                header::HeaderName::from_static("referrer-policy"),
                "no-referrer",
            ),
        ],
        Html(html),
    )
        .into_response()
}

async fn store_guest_html(
    State(state): State<AppState>,
    ConnectInfo(peer_addr): ConnectInfo<SocketAddr>,
    Json(body): Json<StoreGuestBody>,
) -> Response {
    // `!=` on a secret short-circuits at the first differing byte, so the
    // comparison time leaks a prefix match. The constant-time helper the ACP
    // transport already uses applies here too. (SEC-GOS-008)
    if !crate::acp::transport::auth::token_matches(Some(&body.secret), &state.secret_key) {
        return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    }
    if !peer_addr_is_loopback(&peer_addr) {
        return (
            StatusCode::BAD_REQUEST,
            "MCP app guest storage is only available to loopback clients",
        )
            .into_response();
    }

    // Consume the token minted for this proxy page and use the CSP the server
    // computed then. An unknown token means the caller never rendered through
    // `/mcp-app-proxy`, so there is no policy to apply and the request is
    // refused rather than defaulting to an empty (absent) CSP. (SEC-GOS-002)
    let csp = {
        let mut store = state.proxy_csp_store.write().await;
        let cutoff = Instant::now() - Duration::from_secs(GUEST_HTML_TTL_SECS);
        store.retain(|_, entry| entry.created > cutoff);
        match store.remove(&body.proxy_token) {
            Some(entry) => entry.guest_csp,
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    "Unknown or expired proxy token; re-render the MCP app proxy page",
                )
                    .into_response();
            }
        }
    };

    let nonce = Uuid::new_v4().to_string();
    let guest_url = format!("{}/mcp-app-guest?nonce={}", state.guest_base_url, nonce);

    {
        let mut store = state.guest_store.write().await;
        let cutoff = Instant::now() - Duration::from_secs(GUEST_HTML_TTL_SECS);
        store.retain(|_, entry| entry.created > cutoff);

        if store.len() >= GUEST_HTML_MAX_ENTRIES {
            if let Some(oldest_key) = store
                .iter()
                .min_by_key(|(_, entry)| entry.created)
                .map(|(key, _)| key.clone())
            {
                store.remove(&oldest_key);
            }
        }

        store.insert(
            nonce.clone(),
            GuestHtmlEntry {
                html: body.html,
                csp,
                created: Instant::now(),
            },
        );
    }

    (
        StatusCode::OK,
        Json(StoreGuestResponse { nonce, guest_url }),
    )
        .into_response()
}

async fn serve_guest_html(
    State(state): State<GuestState>,
    Query(params): Query<GuestQuery>,
) -> Response {
    let entry = {
        let mut store = state.guest_store.write().await;
        let cutoff = Instant::now() - Duration::from_secs(GUEST_HTML_TTL_SECS);
        store.retain(|_, entry| entry.created > cutoff);
        store.remove(&params.nonce)
    };

    match entry {
        Some(entry) => {
            let mut response = Html(entry.html).into_response();
            let headers = response.headers_mut();
            headers.insert(
                header::HeaderName::from_static("referrer-policy"),
                "strict-origin".parse().unwrap(),
            );
            if !entry.csp.is_empty() {
                match HeaderValue::from_str(&entry.csp) {
                    Ok(csp) => {
                        headers.insert(header::CONTENT_SECURITY_POLICY, csp);
                    }
                    Err(_) => return (StatusCode::BAD_REQUEST, "Invalid CSP").into_response(),
                }
            }
            response
        }
        None => (StatusCode::NOT_FOUND, "Guest content not found").into_response(),
    }
}

fn spawn_guest_server(guest_store: GuestHtmlStore) -> String {
    let listener =
        std::net::TcpListener::bind(("127.0.0.1", 0)).expect("failed to bind MCP app guest server");
    let addr = listener
        .local_addr()
        .expect("failed to read MCP app guest server address");
    listener
        .set_nonblocking(true)
        .expect("failed to configure MCP app guest server");
    let listener = tokio::net::TcpListener::from_std(listener)
        .expect("failed to create MCP app guest listener");

    let app = Router::new()
        .route("/mcp-app-guest", get(serve_guest_html))
        .with_state(GuestState { guest_store });

    tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            tracing::error!(%error, "MCP app guest server stopped");
        }
    });

    format!("http://{addr}")
}

pub(crate) fn routes(secret_key: String) -> Router {
    let guest_store = Arc::new(RwLock::new(HashMap::new()));
    let guest_base_url = spawn_guest_server(guest_store.clone());
    let state = AppState {
        secret_key,
        guest_store,
        guest_base_url,
        proxy_csp_store: Arc::new(RwLock::new(HashMap::new())),
    };

    Router::new()
        .route("/mcp-app-proxy", get(mcp_app_proxy))
        .route(
            "/mcp-app-guest",
            post(store_guest_html).layer(DefaultBodyLimit::max(GUEST_HTML_MAX_BYTES)),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::{build_outer_csp, normalize_csp_source, parse_domains, peer_addr_is_loopback};
    use std::net::SocketAddr;

    // The guest CSP used to be posted back by the proxy page, which read its
    // own <meta> policy and rewrote nonces to 'unsafe-inline' before sending
    // it — so the frame the policy bounds supplied the policy. It is now
    // derived server-side from the declared domains. (SEC-GOS-002)
    #[test]
    fn guest_csp_is_derived_from_declared_domains_not_supplied() {
        let connect = parse_domains(Some(&"https://api.example.com".to_string()));
        let guest_csp = build_outer_csp(
            &connect,
            &[],
            &[],
            &[],
            &[],
            "http://127.0.0.1:1234",
            "'unsafe-inline'",
        );

        assert!(guest_csp.starts_with("default-src 'none'"));
        assert!(guest_csp.contains("https://api.example.com"));
        assert!(guest_csp.contains("object-src 'none'"));
    }

    #[test]
    fn an_undeclared_domain_never_reaches_the_guest_policy() {
        let guest_csp = build_outer_csp(
            &parse_domains(Some(&"https://declared.example".to_string())),
            &[],
            &[],
            &[],
            &[],
            "http://127.0.0.1:1234",
            "'unsafe-inline'",
        );
        assert!(guest_csp.contains("https://declared.example"));
        assert!(
            !guest_csp.contains("https://attacker.example"),
            "only declared domains may appear in the guest policy"
        );
    }

    #[test]
    fn the_guest_policy_allows_inline_where_the_outer_page_pins_a_nonce() {
        let outer = build_outer_csp(&[], &[], &[], &[], &[], "http://127.0.0.1:1", "'nonce-abc'");
        let guest = build_outer_csp(
            &[],
            &[],
            &[],
            &[],
            &[],
            "http://127.0.0.1:1",
            "'unsafe-inline'",
        );
        assert!(outer.contains("'nonce-abc'"));
        assert!(!outer.contains("'unsafe-inline'; script-src"));
        assert!(guest.contains("'unsafe-inline'"));
    }

    #[test]
    fn normalizes_url_sources_to_origins() {
        assert_eq!(
            normalize_csp_source("https://cdn.example.com/assets/app.js"),
            Some("https://cdn.example.com".to_string())
        );
        assert_eq!(
            normalize_csp_source("wss://api.example.com/socket"),
            Some("wss://api.example.com".to_string())
        );
    }

    #[test]
    fn accepts_wildcard_and_host_sources() {
        assert_eq!(
            normalize_csp_source("https://*.cloudflare.com"),
            Some("https://*.cloudflare.com".to_string())
        );
        assert_eq!(
            normalize_csp_source("cdn.example.com"),
            Some("cdn.example.com".to_string())
        );
        assert_eq!(
            normalize_csp_source("localhost:3000"),
            Some("localhost:3000".to_string())
        );
    }

    #[test]
    fn rejects_unsafe_csp_sources() {
        assert_eq!(normalize_csp_source("*"), None);
        assert_eq!(normalize_csp_source("'unsafe-inline'"), None);
        assert_eq!(normalize_csp_source("javascript:alert(1)"), None);
        assert_eq!(normalize_csp_source("https://example.com;"), None);
        assert_eq!(normalize_csp_source("https://user@example.com"), None);
    }

    #[test]
    fn parse_domains_filters_invalid_sources() {
        let domains =
            "https://cdn.example.com/app.js, https://*.cloudflare.com, *, cdn.example.com"
                .to_string();

        assert_eq!(
            parse_domains(Some(&domains)),
            vec![
                "https://cdn.example.com".to_string(),
                "https://*.cloudflare.com".to_string(),
                "cdn.example.com".to_string(),
            ]
        );
    }

    #[test]
    fn detects_loopback_peer_addresses() {
        assert!(peer_addr_is_loopback(
            &"127.0.0.1:12345".parse::<SocketAddr>().unwrap()
        ));
        assert!(peer_addr_is_loopback(
            &"[::1]:12345".parse::<SocketAddr>().unwrap()
        ));
        assert!(!peer_addr_is_loopback(
            &"192.168.1.10:12345".parse::<SocketAddr>().unwrap()
        ));
    }
}
