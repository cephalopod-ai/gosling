use crate::acp::server::{
    AcpProviderFactory, GoslingAcpAgent, GoslingAcpAgentOptions, PromptRunShutdown,
};
use crate::acp::shell::ShellRuntime;
use crate::agents::GoslingPlatform;
use crate::config::paths::{Paths, RuntimePaths};
use crate::source_roots::SourceRoot;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::OnceCell;
use tracing::info;

pub struct AcpServerFactoryConfig {
    pub state_dir: std::path::PathBuf,
    pub builtins: Vec<String>,
    pub data_dir: std::path::PathBuf,
    pub platform_data_dir: std::path::PathBuf,
    pub config_dir: std::path::PathBuf,
    pub gosling_platform: GoslingPlatform,
    pub additional_source_roots: Vec<SourceRoot>,
    pub shell_runtime: ShellRuntime,
}

pub struct AcpServer {
    config: AcpServerFactoryConfig,
    session_manager: Arc<crate::session::SessionManager>,
    prompt_runs: PromptRunShutdown,
    interrupted_turns_closed: OnceCell<()>,
}

impl AcpServer {
    pub fn new(config: AcpServerFactoryConfig) -> Self {
        let session_manager =
            Arc::new(crate::session::SessionManager::new(config.data_dir.clone()));
        Self {
            config,
            session_manager,
            prompt_runs: PromptRunShutdown::new(),
            interrupted_turns_closed: OnceCell::new(),
        }
    }

    /// Stops the prompts running on every connection, and any that arrive
    /// later: each answers `cancelled` and its turn is closed as interrupted.
    /// Waits up to `grace` for the answers.
    pub async fn stop_prompt_runs(&self, grace: std::time::Duration) {
        self.prompt_runs.stop_all(grace).await
    }

    pub async fn shutdown(&self) {
        self.session_manager.shutdown().await
    }

    pub async fn create_agent(&self) -> Result<Arc<GoslingAcpAgent>> {
        // Before this server starts a prompt of its own, so none of its runs
        // can be taken for one whose process died.
        self.interrupted_turns_closed
            .get_or_try_init(|| self.session_manager.close_interrupted_turns())
            .await?;
        Paths::scope(self.runtime_paths(), async {
            let config = crate::config::Config::global();
            let disable_session_naming =
                config.get_gosling_disable_session_naming().unwrap_or(false);

            let provider_factory: AcpProviderFactory =
                Arc::new(move |provider_name, extensions, working_dir| {
                    Box::pin(async move {
                        match working_dir {
                            Some(working_dir) => {
                                crate::providers::create_with_working_dir(
                                    &provider_name,
                                    extensions,
                                    working_dir,
                                )
                                .await
                            }
                            None => crate::providers::create(&provider_name, extensions).await,
                        }
                    })
                });

            let agent = GoslingAcpAgent::new(GoslingAcpAgentOptions {
                provider_factory,
                builtins: self.config.builtins.clone(),
                state_dir: self.config.state_dir.clone(),
                data_dir: self.config.data_dir.clone(),
                platform_data_dir: self.config.platform_data_dir.clone(),
                config_dir: self.config.config_dir.clone(),
                disable_session_naming,
                gosling_platform: self.config.gosling_platform.clone(),
                additional_source_roots: self.config.additional_source_roots.clone(),
                shell_runtime: self.config.shell_runtime.clone(),
                session_manager: Some(Arc::clone(&self.session_manager)),
            })
            .await?
            .with_prompt_run_shutdown(self.prompt_runs.clone());
            info!("Created new ACP agent");

            Ok(Arc::new(agent))
        })
        .await
    }
}

impl AcpServer {
    /// Session-store directory, used by the readiness probe behind `/status`.
    pub fn data_dir(&self) -> std::path::PathBuf {
        self.config.data_dir.clone()
    }

    pub fn runtime_paths(&self) -> RuntimePaths {
        RuntimePaths::new(
            self.config.config_dir.clone(),
            self.config.data_dir.clone(),
            self.config.state_dir.clone(),
        )
    }
}
