use axum::{
    extract::{connect_info::ConnectInfo, Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
pub use gosling::acp::transport::auth::check_acp_token;
use gosling::acp::transport::auth::token_matches;
use std::net::SocketAddr;

fn has_loopback_peer(request: &Request) -> bool {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .is_some_and(|ConnectInfo(peer)| peer.ip().is_loopback())
}

pub async fn check_token(
    State(state): State<String>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if request.uri().path() == "/status" {
        return Ok(next.run(request).await);
    }
    if matches!(request.uri().path(), "/mcp-app-proxy" | "/mcp-app-guest") {
        if has_loopback_peer(&request) {
            return Ok(next.run(request).await);
        }
        return Err(StatusCode::FORBIDDEN);
    }
    let secret_key = request
        .headers()
        .get("X-Secret-Key")
        .and_then(|value| value.to_str().ok());

    if token_matches(secret_key, &state) {
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};

    fn guest_request(peer: Option<SocketAddr>) -> Request<Body> {
        let mut request = Request::builder()
            .uri("/mcp-app-guest")
            .body(Body::empty())
            .unwrap();
        if let Some(peer) = peer {
            request.extensions_mut().insert(ConnectInfo(peer));
        }
        request
    }

    #[test]
    fn mcp_app_guest_requires_loopback_peer_metadata() {
        assert!(has_loopback_peer(&guest_request(Some(
            "127.0.0.1:1234".parse().unwrap()
        ))));
        assert!(has_loopback_peer(&guest_request(Some(
            "[::1]:1234".parse().unwrap()
        ))));
        for peer in [None, Some("192.0.2.10:1234".parse().unwrap())] {
            assert!(!has_loopback_peer(&guest_request(peer)));
        }
    }
}
