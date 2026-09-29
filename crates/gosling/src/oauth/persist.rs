use rmcp::transport::auth::{AuthError, CredentialStore, StoredCredentials};

use crate::config::Config;

const SECRET_KEY_PREFIX: &str = "oauth_creds_";

/// Gosling-specific credential store that uses the Config system
///
/// This implementation stores OAuth credentials in the gosling configuration
/// system, which handles secure storage (e.g., keychain integration).

#[derive(Clone)]
pub struct GoslingCredentialStore {
    name: String,
}

impl GoslingCredentialStore {
    pub fn new(name: String) -> Self {
        Self { name }
    }

    pub(crate) fn is_secret_key(key: &str) -> bool {
        key.starts_with(SECRET_KEY_PREFIX)
    }

    fn secret_key(&self) -> String {
        format!("{SECRET_KEY_PREFIX}{}", self.name)
    }
}

#[async_trait::async_trait]
impl CredentialStore for GoslingCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        let config = Config::global();
        let key = self.secret_key();

        match config.get_secret::<StoredCredentials>(&key) {
            Ok(credentials) => Ok(Some(credentials)),
            Err(_) => Ok(None), // No credentials found
        }
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        let config = Config::global();
        let key = self.secret_key();

        config
            .set_secret(&key, &credentials)
            .map_err(|e| AuthError::InternalError(format!("Failed to save credentials: {}", e)))
    }

    async fn clear(&self) -> Result<(), AuthError> {
        let config = Config::global();
        let key = self.secret_key();

        config
            .delete_secret(&key)
            .map_err(|e| AuthError::InternalError(format!("Failed to clear credentials: {}", e)))
    }
}
