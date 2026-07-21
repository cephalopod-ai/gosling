use crate::configuration;
use crate::state;
use anyhow::Result;
use axum::middleware;
use axum_server::Handle;
use gosling::acp::server_factory::{AcpServer, AcpServerFactoryConfig};
use gosling::acp::transport::create_authenticated_acp_router;
use gosling::agents::GoslingPlatform;
use gosling::config::paths::Paths;
use gosling_server::auth::check_token;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

fn boot_marker(message: &str) {
    eprintln!("GOSLINGD_BOOT: {message}");
}

#[cfg(unix)]
async fn platform_signal_wait() {
    use tokio::signal::unix::{signal, SignalKind};

    let mut sigint = signal(SignalKind::interrupt()).expect("failed to install SIGINT handler");
    let mut sigterm = signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");

    tokio::select! {
        _ = sigint.recv() => {},
        _ = sigterm.recv() => {},
    }
}

#[cfg(not(unix))]
async fn platform_signal_wait() {
    let _ = tokio::signal::ctrl_c().await;
}

const PARENT_PID_ENV_VAR: &str = "GOSLING_SERVER__PARENT_PID";
const PARENT_LIVENESS_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);
#[cfg(any(feature = "rustls-tls", feature = "native-tls"))]
const TLS_SHUTDOWN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn supervising_parent_pid() -> Option<u32> {
    std::env::var(PARENT_PID_ENV_VAR).ok()?.parse().ok()
}

/// Waits for the desktop app that launched this process (if any) to exit.
///
/// `goslingd` is normally spawned and owned by the Electron desktop app, which
/// terminates it via SIGTERM/`taskkill` on a graceful quit. If the desktop app
/// itself dies without going through that path (force-quit, crash, OS kill),
/// this process would otherwise survive as an orphan holding an active
/// session open. `PARENT_PID_ENV_VAR` is set by the desktop app to its own
/// pid; when present, this resolves once that pid is no longer running so
/// `shutdown_signal` can fold it into the same graceful shutdown used for
/// SIGTERM. Standalone invocations (CLI, tests, other embedders) never set
/// the env var, so this is a no-op `pending()` future for them.
async fn parent_exit_wait() {
    match supervising_parent_pid() {
        Some(pid) => {
            gosling::subprocess::wait_for_process_exit(pid, PARENT_LIVENESS_POLL_INTERVAL).await
        }
        None => std::future::pending().await,
    }
}

async fn shutdown_signal() {
    tokio::select! {
        _ = platform_signal_wait() => {},
        _ = parent_exit_wait() => {
            info!("supervising desktop app process exited; shutting down gracefully");
        },
    }
}

pub async fn run() -> Result<()> {
    // Install the rustls crypto provider early, before any spawned tasks (tunnel, etc.)
    // try to open TLS connections. Both `ring` and `aws-lc-rs`
    // features are enabled on rustls (via different transitive deps), so rustls
    // cannot auto-detect a provider — we must pick one explicitly.
    #[cfg(feature = "rustls-tls")]
    let _ = rustls::crypto::ring::default_provider().install_default();

    boot_marker("main entered");
    crate::logging::setup_logging(Some("goslingd"))?;

    let settings = configuration::Settings::new()?;

    let secret_key = std::env::var("GOSLING_SERVER__SECRET_KEY")
        .unwrap_or_else(|_| hex::encode(rand::random::<[u8; 32]>()));

    boot_marker("appstate init start");
    let app_state = state::AppState::new(settings.tls).await?;
    let shutdown_state = app_state.clone();
    let shutdown_token = app_state.shutdown_token();
    tokio::spawn(async move {
        shutdown_signal().await;
        shutdown_state.shutdown().await;
    });

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // TODO(acp-migration): When ui/desktop launches `gosling serve` directly,
    // move any goslingd-only ACP setup into the gosling serve path before deleting
    // this bridge. In particular, verify everything ACP currently gets from
    // goslingd startup/AppState initialization, including builtin extension
    // registration and the desktop platform identity.
    let acp_server = Arc::new(AcpServer::new(AcpServerFactoryConfig {
        builtins: vec!["developer".to_string()],
        state_dir: Paths::state_dir(),
        data_dir: Paths::data_dir(),
        config_dir: Paths::config_dir(),
        gosling_platform: GoslingPlatform::GoslingDesktop,
        additional_source_roots: Vec::new(),
    }));

    let rest_router = crate::routes::configure(app_state.clone(), secret_key.clone())
        .layer(middleware::from_fn_with_state(
            secret_key.clone(),
            check_token,
        ))
        .layer(cors);
    let acp_router = create_authenticated_acp_router(acp_server, secret_key.clone());

    let app = rest_router.merge(acp_router);

    let addr = settings.socket_addr()?;

    if settings.tls {
        #[cfg(any(feature = "rustls-tls", feature = "native-tls"))]
        {
            boot_marker("tls setup start");
            let tls_setup = gosling::acp::transport::tls::setup_tls(
                settings.tls_cert_path.as_deref(),
                settings.tls_key_path.as_deref(),
            )
            .await?;

            let handle = Handle::new();
            let shutdown_handle = handle.clone();
            let tls_shutdown = shutdown_token.clone();
            tokio::spawn(async move {
                tls_shutdown.cancelled().await;
                shutdown_handle.graceful_shutdown(Some(TLS_SHUTDOWN_TIMEOUT));
            });

            info!("listening on https://{}", addr);
            boot_marker("listening");

            #[cfg(feature = "rustls-tls")]
            axum_server::bind_rustls(addr, tls_setup.config)
                .handle(handle)
                .serve(app.into_make_service())
                .await?;

            #[cfg(feature = "native-tls")]
            axum_server::bind_openssl(addr, tls_setup.config)
                .handle(handle)
                .serve(app.into_make_service())
                .await?;
        }

        #[cfg(not(any(feature = "rustls-tls", feature = "native-tls")))]
        {
            anyhow::bail!(
                "TLS was requested but no TLS backend is enabled. \
                 Enable the `rustls-tls` or `native-tls` feature."
            );
        }
    } else {
        boot_marker("tcp bind start");
        let listener = tokio::net::TcpListener::bind(addr).await?;

        info!("listening on http://{}", addr);
        boot_marker("listening");

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_token.cancelled_owned())
            .await?;
    }

    #[cfg(feature = "otel")]
    if gosling::otel::otlp::is_otlp_initialized() {
        gosling::otel::otlp::shutdown_otlp();
    }

    info!("server shutdown complete");
    Ok(())
}
