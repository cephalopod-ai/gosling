use etcetera::{choose_app_strategy, AppStrategy, AppStrategyArgs};
use once_cell::sync::Lazy;
use rmcp::{ServerHandler, ServiceExt};
use std::collections::HashMap;
use std::path::PathBuf;

// NOTE: "Block" is kept here for backwards compatibility with existing
// user config/data directories. Changing this would orphan existing installations.
pub static APP_STRATEGY: Lazy<AppStrategyArgs> = Lazy::new(|| AppStrategyArgs {
    top_level_domain: "Block".to_string(),
    author: "Block".to_string(),
    app_name: "gosling".to_string(),
});

/// Directory under gosling's cache tree, honoring `GOSLING_PATH_ROOT`.
pub(crate) fn gosling_cache_dir(subpath: &str) -> Option<PathBuf> {
    if let Ok(root) = std::env::var("GOSLING_PATH_ROOT") {
        return Some(PathBuf::from(root).join("cache").join(subpath));
    }
    choose_app_strategy(APP_STRATEGY.clone())
        .ok()
        .map(|strategy| strategy.in_cache_dir(subpath))
}

pub mod autovisualiser;
pub mod computercontroller;
pub mod mcp_server_runner;
#[cfg(target_os = "macos")]
pub mod peekaboo;
pub mod subprocess;

pub use autovisualiser::AutoVisualiserRouter;
pub use computercontroller::ComputerControllerServer;

/// Type definition for a function that spawns and serves a builtin extension server
pub type SpawnServerFn = fn(tokio::io::DuplexStream, tokio::io::DuplexStream);

fn spawn_and_serve<S>(
    name: &'static str,
    server: S,
    transport: (tokio::io::DuplexStream, tokio::io::DuplexStream),
) where
    S: ServerHandler + Send + 'static,
{
    tokio::spawn(async move {
        match server.serve(transport).await {
            Ok(running) => {
                let _ = running.waiting().await;
            }
            Err(e) => tracing::error!(builtin = name, error = %e, "server error"),
        }
    });
}

macro_rules! builtin {
    ($name:ident, $server_ty:ty) => {{
        fn spawn(r: tokio::io::DuplexStream, w: tokio::io::DuplexStream) {
            spawn_and_serve(stringify!($name), <$server_ty>::new(), (r, w));
        }
        (stringify!($name), spawn as SpawnServerFn)
    }};
}

pub static BUILTIN_EXTENSIONS: Lazy<HashMap<&'static str, SpawnServerFn>> = Lazy::new(|| {
    HashMap::from([
        builtin!(autovisualiser, AutoVisualiserRouter),
        builtin!(computercontroller, ComputerControllerServer),
    ])
});
