//! Error types for the LLM layer

use std::time::Duration;
use thiserror::Error;

/// Errors that can occur when using LLM providers
#[derive(Debug, Error)]
pub enum LlmError {
    /// Authentication/token issues
    #[error("Authentication error: {0}")]
    AuthenticationError(String),

    /// HTTP request failures
    #[error("HTTP error (status {status}): {body}")]
    HttpError { status: u16, body: String },

    /// SSE stream parsing failures
    #[error("Stream error: {0}")]
    StreamError(String),

    /// JSON encoding/decoding issues
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Invalid request parameters
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded (retry after {retry_after:?})")]
    RateLimitExceeded { retry_after: Option<Duration> },

    /// Provider-specific errors
    #[error("Provider error ({code}): {message}")]
    ProviderError { code: String, message: String },
}

// Implement conversion from common error types
impl From<serde_json::Error> for LlmError {
    fn from(err: serde_json::Error) -> Self {
        LlmError::SerializationError(err.to_string())
    }
}

impl From<reqwest::Error> for LlmError {
    fn from(err: reqwest::Error) -> Self {
        if let Some(status) = err.status() {
            LlmError::HttpError {
                status: status.as_u16(),
                body: err.to_string(),
            }
        } else {
            LlmError::HttpError {
                status: 0,
                body: err.to_string(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_authentication_error() {
        let err = LlmError::AuthenticationError("Invalid token".to_string());
        assert!(err.to_string().contains("Authentication error"));
        assert!(err.to_string().contains("Invalid token"));
    }

    #[test]
    fn test_http_error() {
        let err = LlmError::HttpError {
            status: 404,
            body: "Not found".to_string(),
        };
        assert!(err.to_string().contains("404"));
        assert!(err.to_string().contains("Not found"));
    }

    #[test]
    fn test_rate_limit_error() {
        let err = LlmError::RateLimitExceeded {
            retry_after: Some(Duration::from_secs(60)),
        };
        assert!(err.to_string().contains("Rate limit exceeded"));
    }

    #[test]
    fn test_provider_error() {
        let err = LlmError::ProviderError {
            code: "invalid_api_key".to_string(),
            message: "API key is invalid".to_string(),
        };
        assert!(err.to_string().contains("invalid_api_key"));
        assert!(err.to_string().contains("API key is invalid"));
    }

    #[test]
    fn test_from_serde_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let llm_err: LlmError = json_err.into();
        assert!(matches!(llm_err, LlmError::SerializationError(_)));
    }
}
