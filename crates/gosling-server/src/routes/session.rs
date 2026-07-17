use crate::routes::errors::ErrorResponse;
use crate::state::AppState;
use axum::extract::State;
use axum::routing::post;
use axum::{
    extract::Path,
    http::StatusCode,
    routing::{get, put},
    Json, Router,
};
use gosling::agents::ExtensionConfig;
use gosling::session::{EnabledExtensionsState, Session};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSessionNameRequest {
    /// Updated name for the session (max 200 characters)
    name: String,
}

#[derive(Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ForkRequest {
    timestamp: Option<i64>,
    truncate: bool,
    copy: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ForkResponse {
    session_id: String,
}

const MAX_NAME_LENGTH: usize = 200;

#[utoipa::path(
    get,
    path = "/sessions/{session_id}",
    params(
        ("session_id" = String, Path, description = "Unique identifier for the session")
    ),
    responses(
        (status = 200, description = "Session history retrieved successfully", body = Session),
        (status = 401, description = "Unauthorized - Invalid or missing API key"),
        (status = 404, description = "Session not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("api_key" = [])
    ),
    tag = "Session Management"
)]
async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<Session>, StatusCode> {
    let session = state
        .session_manager()
        .get_session(&session_id, true)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(session))
}

#[utoipa::path(
    put,
    path = "/sessions/{session_id}/name",
    request_body = UpdateSessionNameRequest,
    params(
        ("session_id" = String, Path, description = "Unique identifier for the session")
    ),
    responses(
        (status = 200, description = "Session name updated successfully"),
        (status = 400, description = "Bad request - Name too long (max 200 characters)"),
        (status = 401, description = "Unauthorized - Invalid or missing API key"),
        (status = 404, description = "Session not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("api_key" = [])
    ),
    tag = "Session Management"
)]
async fn update_session_name(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<UpdateSessionNameRequest>,
) -> Result<StatusCode, ErrorResponse> {
    let name = request.name.trim();
    if name.is_empty() {
        return Err(ErrorResponse::bad_request("Session name cannot be empty"));
    }
    if name.len() > MAX_NAME_LENGTH {
        return Err(ErrorResponse::bad_request(format!(
            "Session name too long (max {} characters)",
            MAX_NAME_LENGTH
        )));
    }

    state
        .session_manager()
        .update(&session_id)
        .user_provided_name(name.to_string())
        .apply()
        .await
        .map_err(|e| {
            tracing::error!("Failed to update session name for {}: {}", session_id, e);
            ErrorResponse {
                message: format!("Failed to update session name: {}", e),
                status: StatusCode::INTERNAL_SERVER_ERROR,
            }
        })?;

    Ok(StatusCode::OK)
}

#[utoipa::path(
    post,
    path = "/sessions/{session_id}/fork",
    request_body = ForkRequest,
    params(
        ("session_id" = String, Path, description = "Unique identifier for the session")
    ),
    responses(
        (status = 200, description = "Session forked successfully", body = ForkResponse),
        (status = 400, description = "Bad request - truncate=true requires timestamp"),
        (status = 401, description = "Unauthorized - Invalid or missing API key"),
        (status = 404, description = "Session not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("api_key" = [])
    ),
    tag = "Session Management"
)]
async fn fork_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<ForkRequest>,
) -> Result<Json<ForkResponse>, ErrorResponse> {
    if request.truncate && request.timestamp.is_none() {
        return Err(ErrorResponse {
            message: "truncate=true requires a timestamp".to_string(),
            status: StatusCode::BAD_REQUEST,
        });
    }

    let session_manager = state.session_manager();

    let target_session_id = if request.copy {
        let original = session_manager
            .get_session(&session_id, false)
            .await
            .map_err(|e| {
                tracing::error!("Failed to get session: {}", e);
                #[cfg(feature = "telemetry")]
                gosling::posthog::emit_error("session_get_failed", &e.to_string());
                ErrorResponse {
                    message: if e.to_string().contains("not found") {
                        format!("Session {} not found", session_id)
                    } else {
                        format!("Failed to get session: {}", e)
                    },
                    status: if e.to_string().contains("not found") {
                        StatusCode::NOT_FOUND
                    } else {
                        StatusCode::INTERNAL_SERVER_ERROR
                    },
                }
            })?;

        let copied = session_manager
            .copy_session(&session_id, original.name)
            .await
            .map_err(|e| {
                tracing::error!("Failed to copy session: {}", e);
                #[cfg(feature = "telemetry")]
                gosling::posthog::emit_error("session_copy_failed", &e.to_string());
                ErrorResponse {
                    message: format!("Failed to copy session: {}", e),
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                }
            })?;

        copied.id
    } else {
        session_id.clone()
    };

    if request.truncate {
        session_manager
            .truncate_conversation(&target_session_id, request.timestamp.unwrap_or(0))
            .await
            .map_err(|e| {
                tracing::error!("Failed to truncate conversation: {}", e);
                #[cfg(feature = "telemetry")]
                gosling::posthog::emit_error("session_truncate_failed", &e.to_string());
                ErrorResponse {
                    message: format!("Failed to truncate conversation: {}", e),
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                }
            })?;
    }

    Ok(Json(ForkResponse {
        session_id: target_session_id,
    }))
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionExtensionsResponse {
    extensions: Vec<ExtensionConfig>,
}

#[utoipa::path(
    get,
    path = "/sessions/{session_id}/extensions",
    params(
        ("session_id" = String, Path, description = "Unique identifier for the session")
    ),
    responses(
        (status = 200, description = "Session extensions retrieved successfully", body = SessionExtensionsResponse),
        (status = 401, description = "Unauthorized - Invalid or missing API key"),
        (status = 404, description = "Session not found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("api_key" = [])
    ),
    tag = "Session Management"
)]
async fn get_session_extensions(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionExtensionsResponse>, StatusCode> {
    let session = state
        .session_manager()
        .get_session(&session_id, false)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let extensions = EnabledExtensionsState::extensions_or_default(
        Some(&session.extension_data),
        gosling::config::Config::global(),
    );

    Ok(Json(SessionExtensionsResponse { extensions }))
}

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/sessions/{session_id}", get(get_session))
        .route("/sessions/{session_id}/name", put(update_session_name))
        .route("/sessions/{session_id}/fork", post(fork_session))
        .route(
            "/sessions/{session_id}/extensions",
            get(get_session_extensions),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use tower::ServiceExt;

    /// Regression test for OPS-004: before the fix, a failed session mutation
    /// mapped its error to a bare `StatusCode`, discarding the root cause, so
    /// the client received a 500 with an empty body and nothing was logged
    /// server-side beyond the status code itself.
    #[tokio::test(flavor = "multi_thread")]
    async fn update_session_name_reports_failure_reason_for_missing_session() {
        let state = AppState::new(true).await.unwrap();
        let app = routes(state);

        let request = Request::builder()
            .uri("/sessions/session-that-does-not-exist/name")
            .method("PUT")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({ "name": "new name" })).unwrap(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_text = String::from_utf8(body.to_vec()).unwrap();
        assert!(
            !body_text.is_empty(),
            "expected a JSON error body instead of an empty 500 response"
        );
        assert!(
            body_text.contains("session-that-does-not-exist"),
            "expected the underlying failure reason in the response body, got: {body_text}"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn update_session_name_still_succeeds_for_a_real_session() {
        let state = AppState::new(true).await.unwrap();
        let session = state
            .session_manager()
            .create_session(
                std::path::PathBuf::from("/tmp"),
                "ops-004-rename-test".to_string(),
                gosling::session::session_manager::SessionType::Hidden,
                gosling::config::GoslingMode::default(),
            )
            .await
            .unwrap();

        let app = routes(state);
        let request = Request::builder()
            .uri(format!("/sessions/{}/name", session.id))
            .method("PUT")
            .header("content-type", "application/json")
            .body(Body::from(
                serde_json::to_string(&serde_json::json!({ "name": "renamed" })).unwrap(),
            ))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert!(
            body.is_empty(),
            "success path must not gain a response body"
        );
    }
}
