use rmcp::transport::auth::{AuthError, CredentialStore, StoredCredentials};

use crate::config::Config;
use std::sync::Arc;

/// Keeps refresh and browser authorization on the same caller-selected store.
#[derive(Clone)]
pub struct SharedCredentialStore(Arc<dyn CredentialStore>);

impl SharedCredentialStore {
    pub fn new(store: Box<dyn CredentialStore>) -> Self {
        Self(Arc::from(store))
    }
}

#[async_trait::async_trait]
impl CredentialStore for SharedCredentialStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        self.0.load().await
    }
    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        self.0.save(credentials).await
    }
    async fn clear(&self) -> Result<(), AuthError> {
        self.0.clear().await
    }
}

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

    pub fn scoped(namespace: &str, uri: &str) -> Self {
        Self::new(format!(
            "scoped_{namespace}_{}",
            blake3::hash(uri.as_bytes()).to_hex()
        ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_oauth_keys_isolate_accounts_and_destinations_from_legacy_keys() {
        let first =
            GoslingCredentialStore::scoped("account-a", "https://example.test/mcp").secret_key();
        assert_ne!(
            first,
            GoslingCredentialStore::scoped("account-b", "https://example.test/mcp").secret_key()
        );
        assert_ne!(
            first,
            GoslingCredentialStore::scoped("account-a", "https://other.test/mcp").secret_key()
        );
        assert_ne!(
            first,
            GoslingCredentialStore::new("example".into()).secret_key()
        );
        assert!(GoslingCredentialStore::is_secret_key(&first));
        assert!(!first.contains("https://"));
    }

    #[tokio::test]
    async fn shared_store_clones_use_the_same_selected_account() {
        let store = SharedCredentialStore::new(Box::new(
            rmcp::transport::auth::InMemoryCredentialStore::new(),
        ));
        let copy = store.clone();
        store
            .save(StoredCredentials::new(
                "fixture-client".into(),
                None,
                vec![],
                None,
            ))
            .await
            .unwrap();
        assert!(copy.load().await.unwrap().is_some());
        copy.clear().await.unwrap();
        assert!(store.load().await.unwrap().is_none());
    }
}
