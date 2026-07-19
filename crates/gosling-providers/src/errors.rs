use reqwest::StatusCode;
use std::time::Duration;
use thiserror::Error;

use crate::request_log::LogError;

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ProviderError {
    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Context length exceeded: {0}")]
    ContextLengthExceeded(String),

    #[error("Rate limit exceeded: {details}")]
    RateLimitExceeded {
        details: String,
        retry_delay: Option<Duration>,
    },

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Request failed: {0}")]
    RequestFailed(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Usage data error: {0}")]
    UsageError(String),

    #[error("Unsupported operation: {0}")]
    NotImplemented(String),

    #[error("Endpoint not found (404): {0}")]
    EndpointNotFound(String),

    #[error("Credits exhausted: {details}")]
    CreditsExhausted {
        details: String,
        top_up_url: Option<String>,
    },

    #[error("Provider refused request: {details}")]
    Refusal {
        details: String,
        category: Option<String>,
    },
}

impl ProviderError {
    pub fn stream_decode_error(error: impl std::fmt::Display) -> Self {
        ProviderError::NetworkError(format!("Stream decode error: {error}"))
    }

    pub fn telemetry_type(&self) -> &'static str {
        match self {
            ProviderError::Authentication(_) => "auth",
            ProviderError::ContextLengthExceeded(_) => "context_length",
            ProviderError::RateLimitExceeded { .. } => "rate_limit",
            ProviderError::ServerError(_) => "server",
            ProviderError::NetworkError(_) => "network",
            ProviderError::RequestFailed(_) => "request",
            ProviderError::ExecutionError(_) => "execution",
            ProviderError::UsageError(_) => "usage",
            ProviderError::NotImplemented(_) => "not_implemented",
            ProviderError::EndpointNotFound(_) => "endpoint_not_found",
            ProviderError::CreditsExhausted { .. } => "credits_exhausted",
            ProviderError::Refusal { .. } => "refusal",
        }
    }

    pub fn is_endpoint_not_found(&self) -> bool {
        matches!(self, ProviderError::EndpointNotFound(_))
    }

    pub fn from_models_error_payload(kind: Option<&str>, message: &str) -> Self {
        let message = message.to_string();
        let kind = kind.unwrap_or_default().to_ascii_lowercase();
        let message_lower = message.to_ascii_lowercase();

        if kind.contains("auth")
            || kind.contains("permission")
            || kind.contains("forbidden")
            || message_lower.contains("api key")
            || message_lower.contains("unauthorized")
            || message_lower.contains("authentication")
        {
            return ProviderError::Authentication(message);
        }

        if kind.contains("rate")
            || kind.contains("quota")
            || message_lower.contains("rate limit")
            || message_lower.contains("too many requests")
        {
            return ProviderError::RateLimitExceeded {
                details: message,
                retry_delay: None,
            };
        }

        if kind.contains("server")
            || kind.contains("overload")
            || kind.contains("unavailable")
            || message_lower.contains("server error")
            || message_lower.contains("overloaded")
            || message_lower.contains("unavailable")
        {
            return ProviderError::ServerError(message);
        }

        ProviderError::RequestFailed(message)
    }

    /// Recover a typed `ProviderError` from a streaming decode error, falling
    /// back to a retryable stream decode error for errors that did not
    /// originate as one.
    pub fn from_stream_error(error: anyhow::Error) -> Self {
        error
            .downcast()
            .unwrap_or_else(ProviderError::stream_decode_error)
    }
}

fn is_network_error(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout() || (err.status().is_none() && err.is_request())
}

fn provider_error_from_reqwest(error: &reqwest::Error) -> ProviderError {
    if is_network_error(error) {
        let msg = if error.is_timeout() {
            "Request timed out — check your network connection and try again.".to_string()
        } else if error.is_connect() {
            if let Some(url) = error.url() {
                if let Some(host) = url.host_str() {
                    let port_info = url.port().map(|p| format!(":{}", p)).unwrap_or_default();
                    format!(
                        "Could not connect to {}{} — check your network connection and try again.",
                        host, port_info
                    )
                } else {
                    "Could not connect to the provider — check your network connection and try again.".to_string()
                }
            } else {
                "Could not connect to the provider — check your network connection and try again."
                    .to_string()
            }
        } else {
            "Network error — check your network connection and try again.".to_string()
        };
        return ProviderError::NetworkError(msg);
    }

    if let Some(status) = error.status() {
        let details = format!("Provider request returned HTTP {status}");
        return match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                ProviderError::Authentication(details)
            }
            StatusCode::PAYMENT_REQUIRED => ProviderError::CreditsExhausted {
                details,
                top_up_url: None,
            },
            StatusCode::PAYLOAD_TOO_LARGE => ProviderError::ContextLengthExceeded(details),
            StatusCode::TOO_MANY_REQUESTS => ProviderError::RateLimitExceeded {
                details,
                retry_delay: None,
            },
            _ if status.is_server_error() => ProviderError::ServerError(details),
            _ => ProviderError::RequestFailed(details),
        };
    }

    ProviderError::RequestFailed(error.to_string())
}

impl From<anyhow::Error> for ProviderError {
    fn from(error: anyhow::Error) -> Self {
        if let Some(reqwest_err) = error.downcast_ref::<reqwest::Error>() {
            return provider_error_from_reqwest(reqwest_err);
        }
        ProviderError::ExecutionError(error.to_string())
    }
}

impl From<reqwest::Error> for ProviderError {
    fn from(error: reqwest::Error) -> Self {
        provider_error_from_reqwest(&error)
    }
}

impl From<LogError> for ProviderError {
    fn from(value: LogError) -> Self {
        ProviderError::ExecutionError(value.to_string())
    }
}

#[cfg(test)]
mod reqwest_error_tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn classified_status(status: u16) -> ProviderError {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/status"))
            .respond_with(ResponseTemplate::new(status))
            .mount(&server)
            .await;
        let error = reqwest::get(format!("{}/status", server.uri()))
            .await
            .unwrap()
            .error_for_status()
            .unwrap_err();
        error.into()
    }

    #[tokio::test]
    async fn classifies_status_errors_from_reqwest() {
        assert!(matches!(
            classified_status(401).await,
            ProviderError::Authentication(_)
        ));
        assert!(matches!(
            classified_status(429).await,
            ProviderError::RateLimitExceeded { .. }
        ));
        assert!(matches!(
            classified_status(503).await,
            ProviderError::ServerError(_)
        ));
        assert!(matches!(
            classified_status(400).await,
            ProviderError::RequestFailed(_)
        ));
    }
}

#[derive(Debug)]
pub enum GoogleErrorCode {
    BadRequest = 400,
    Unauthorized = 401,
    Forbidden = 403,
    NotFound = 404,
    TooManyRequests = 429,
    InternalServerError = 500,
    ServiceUnavailable = 503,
}

impl GoogleErrorCode {
    pub fn to_status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest => StatusCode::BAD_REQUEST,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    pub fn from_code(code: u64) -> Option<Self> {
        match code {
            400 => Some(Self::BadRequest),
            401 => Some(Self::Unauthorized),
            403 => Some(Self::Forbidden),
            404 => Some(Self::NotFound),
            429 => Some(Self::TooManyRequests),
            500 => Some(Self::InternalServerError),
            503 => Some(Self::ServiceUnavailable),
            // Unmapped codes return None so callers can distinguish a known
            // status from an unrecognized one, rather than silently coercing
            // every unknown code (incl. unmapped 4xx) into a 500.
            _ => None,
        }
    }
}
