use super::api_client::TlsConfig;
use anyhow::Result;
use futures::future::BoxFuture;
pub use gosling_providers::conversation::token_usage::{
    DraftStats, ProviderStats, ProviderUsage, Usage,
};
use serde::{Deserialize, Serialize};

/// Default provider HTTP budget. For inference calls this is a stall budget —
/// see `api_client::inference_client_builder`, which applies it as a read
/// timeout so a turn's tool execution can't run it out. One-shot auth and
/// token calls apply it as a total deadline, which is what they want.
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

    /// Structured continuity capabilities available before a live provider is created.
    /// Existing provider definitions inherit the legacy context-ownership projection;
    /// adapters with native resume/import behavior must override this contract explicitly.
    const CAPABILITIES: ProviderCapabilities =
        ProviderCapabilities::from_legacy_context_ownership(Self::MANAGES_OWN_CONTEXT);

    /// Compatibility projection of `Self::Provider`'s context ownership for a given
    /// provider type, but as an associated const readable at provider-registry
    /// registration time without constructing an instance (`Provider` is used as
    /// `dyn Provider`, so it can't carry this as an associated const itself).
    /// Keep this in sync with the corresponding `Provider` impl's
    /// `capabilities()` implementation.
    const MANAGES_OWN_CONTEXT: bool = false;

    /// Mirrors `Self::Provider`'s `Provider::executes_tools_outside_gosling()`
    /// so ACP session setup can reject or normalize modes before constructing
    /// the provider.
    const EXECUTES_TOOLS_OUTSIDE_GOSLING: bool = false;

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
