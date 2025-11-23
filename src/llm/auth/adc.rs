//! Application Default Credentials (ADC) wrapper

use gcp_auth::AuthenticationManager as GcpAuthManager;

use crate::llm::core::error::LlmError;

/// Manages GCP authentication tokens using Application Default Credentials
///
/// This wrapper around `gcp_auth::AuthenticationManager` provides token
/// management with automatic caching and refresh for GCP services.
///
/// Supports multiple credential sources:
/// - `GOOGLE_APPLICATION_CREDENTIALS` environment variable
/// - User credentials from `gcloud auth application-default login`
/// - Metadata server (Compute Engine, Cloud Run, GKE)
pub struct AuthenticationManager {
    inner: GcpAuthManager,
}

impl AuthenticationManager {
    /// Create a new authentication manager
    ///
    /// This will discover credentials using the standard ADC flow.
    ///
    /// # Errors
    /// Returns an error if no valid credentials can be found.
    pub async fn new() -> Result<Self, LlmError> {
        let inner = GcpAuthManager::new()
            .await
            .map_err(|e| LlmError::AuthenticationError(format!("Failed to initialize ADC: {}", e)))?;

        Ok(Self { inner })
    }

    /// Get an access token for the cloud platform scope
    ///
    /// The token is cached internally and automatically refreshed when expired.
    ///
    /// # Errors
    /// Returns an error if token retrieval or refresh fails.
    pub async fn get_token(&self) -> Result<String, LlmError> {
        let token = self.inner
            .get_token(&["https://www.googleapis.com/auth/cloud-platform"])
            .await
            .map_err(|e| LlmError::AuthenticationError(format!("Failed to get token: {}", e)))?;

        Ok(token.as_str().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Only run with valid credentials
    async fn test_authentication_manager_new() {
        // This test requires valid GCP credentials (ADC)
        let result = AuthenticationManager::new().await;
        assert!(
            result.is_ok(),
            "Failed to initialize AuthenticationManager with ADC: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    #[ignore] // Only run with valid credentials
    async fn test_get_token() {
        // This test requires valid GCP credentials (ADC)
        let auth = AuthenticationManager::new()
            .await
            .expect("Failed to initialize AuthenticationManager");

        let token = auth
            .get_token()
            .await
            .expect("Failed to retrieve access token");

        // Token should be non-empty
        assert!(!token.is_empty(), "Token should not be empty");

        // Token should be at least a reasonable length (GCP tokens are typically quite long)
        assert!(
            token.len() > 20,
            "Token seems too short: {} characters",
            token.len()
        );

        // Token should not contain obvious error messages
        assert!(
            !token.contains("error"),
            "Token appears to contain error text"
        );
    }
}
