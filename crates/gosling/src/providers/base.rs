use super::api_client::TlsConfig;
use anyhow::Result;
use futures::future::BoxFuture;
pub use gosling_providers::conversation::token_usage::{
    DraftStats, ProviderStats, ProviderUsage, Usage,
};
use serde::{Deserialize, Serialize};

pub const DEFAULT_PROVIDER_TIMEOUT_SECS: u64 = 600;

use crate::config::ExtensionConfig;
use utoipa::ToSchema;

use std::path::PathBuf;

pub use gosling_providers::base::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum ProviderType {
    Preferred,
    Builtin,
    Declarative,
    Custom,
}

pub(crate) fn current_working_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

pub trait ProviderDef: ProviderDescriptor + Send + Sync {
    type Provider: Provider + 'static;

    /// Mirrors `Self::Provider`'s `Provider::manages_own_context()` for a given
    /// provider type, but as an associated const readable at provider-registry
    /// registration time without constructing an instance (`Provider` is used as
    /// `dyn Provider`, so it can't carry this as an associated const itself).
    /// Keep this in sync with the corresponding `Provider` impl's
    /// `manages_own_context()` override.
    const MANAGES_OWN_CONTEXT: bool = false;

    fn from_env(
        extensions: Vec<ExtensionConfig>,
        tls_config: Option<TlsConfig>,
    ) -> BoxFuture<'static, Result<Self::Provider>>
    where
        Self: Sized;

    fn from_env_with_working_dir(
        extensions: Vec<ExtensionConfig>,
        _working_dir: PathBuf,
        tls_config: Option<TlsConfig>,
    ) -> BoxFuture<'static, Result<Self::Provider>>
    where
        Self: Sized,
    {
        Self::from_env(extensions, tls_config)
    }
}
