use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use subtle::ConstantTimeEq;

pub fn token_matches(candidate: Option<&str>, expected: &str) -> bool {
    candidate
        .map(|key| bool::from(key.as_bytes().ct_eq(expected.as_bytes())))
        .unwrap_or(false)
}

/// Subprotocol prefix carrying the shared secret on a WebSocket upgrade.
///
/// Browser `WebSocket` cannot set request headers, which is why the secret was
/// originally passed as `?token=`. A URL query string lands in access logs,
/// `ps` output, crash reports, and `Referer` headers, so the secret is now
/// carried in `Sec-WebSocket-Protocol` instead — still attacker-visible only
/// with the connection itself, but no longer written to any of those sinks.
/// (SEC-GOS-001)
pub const ACP_TOKEN_SUBPROTOCOL_PREFIX: &str = "gosling.token.";

fn subprotocol_token(request: &Request) -> Option<String> {
    request
        .headers()
        .get(axum::http::header::SEC_WEBSOCKET_PROTOCOL)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| {
            value
                .split(',')
                .map(str::trim)
                .find_map(|proto| proto.strip_prefix(ACP_TOKEN_SUBPROTOCOL_PREFIX))
                .map(str::to_owned)
        })
}

pub async fn check_acp_token(
    State(state): State<String>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let header_token = request
        .headers()
        .get("X-Secret-Key")
        .and_then(|value| value.to_str().ok());

    let offered_subprotocol = subprotocol_token(&request);

    if !token_matches(header_token, &state)
        && !token_matches(offered_subprotocol.as_deref(), &state)
    {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut response = next.run(request).await;

    // A client that offers a subprotocol aborts the connection unless the
    // server selects one. The upstream ACP WebSocket server does not handle
    // `Sec-WebSocket-Protocol`, so the selection is echoed here.
    if let Some(token) = offered_subprotocol {
        if response.status() == StatusCode::SWITCHING_PROTOCOLS {
            if let Ok(value) =
                axum::http::HeaderValue::from_str(&format!("{ACP_TOKEN_SUBPROTOCOL_PREFIX}{token}"))
            {
                response
                    .headers_mut()
                    .insert(axum::http::header::SEC_WEBSOCKET_PROTOCOL, value);
            }
        }
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::SEC_WEBSOCKET_PROTOCOL;

    fn request_with_subprotocol(value: &str) -> Request {
        Request::builder()
            .header(SEC_WEBSOCKET_PROTOCOL, value)
            .body(axum::body::Body::empty())
            .unwrap()
    }

    #[test]
    fn extracts_token_from_a_single_subprotocol() {
        let request = request_with_subprotocol("gosling.token.s3cret");
        assert_eq!(subprotocol_token(&request).as_deref(), Some("s3cret"));
    }

    #[test]
    fn extracts_token_when_offered_alongside_other_subprotocols() {
        let request = request_with_subprotocol("acp.v1, gosling.token.s3cret");
        assert_eq!(subprotocol_token(&request).as_deref(), Some("s3cret"));
    }

    #[test]
    fn absent_or_unprefixed_subprotocol_yields_no_token() {
        assert!(subprotocol_token(&request_with_subprotocol("acp.v1")).is_none());
        assert!(subprotocol_token(&Request::new(axum::body::Body::empty())).is_none());
    }

    #[test]
    fn query_string_token_is_no_longer_accepted() {
        let request = Request::builder()
            .uri("/acp?token=s3cret")
            .body(axum::body::Body::empty())
            .unwrap();
        assert!(subprotocol_token(&request).is_none());
    }
}
