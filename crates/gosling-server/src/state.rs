use axum::http::StatusCode;
use futures::future::{BoxFuture, Shared};
use futures::FutureExt;
use gosling::builtin_extension::register_builtin_extensions;
use gosling::config::Config;
use gosling::execution::manager::{AgentManager, DEFAULT_MAX_SESSION};
use gosling::session::SessionManager;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::{AbortHandle, JoinError, JoinHandle};
use tokio_util::sync::CancellationToken;

use crate::session_event_bus::SessionEventBus;
use gosling::agents::ExtensionLoadResult;

type ExtensionLoadingResult = Result<Vec<ExtensionLoadResult>, Arc<JoinError>>;
type ExtensionLoadingFuture = Shared<BoxFuture<'static, ExtensionLoadingResult>>;

#[derive(Clone)]
struct ExtensionLoadingTask {
    abort_handle: AbortHandle,
    result: ExtensionLoadingFuture,
}

impl ExtensionLoadingTask {
    fn new(handle: JoinHandle<Vec<ExtensionLoadResult>>) -> Self {
        let abort_handle = handle.abort_handle();
        let result = async move { handle.await.map_err(Arc::new) }
            .boxed()
            .shared();
        Self {
            abort_handle,
            result,
        }
    }

    fn abort(self) {
        self.abort_handle.abort();
    }
}

#[derive(Clone, Default)]
struct ExtensionLoadingTasks {
    tasks: Arc<Mutex<HashMap<String, ExtensionLoadingTask>>>,
}

impl ExtensionLoadingTasks {
    async fn replace(&self, session_id: String, handle: JoinHandle<Vec<ExtensionLoadResult>>) {
        let replaced = self
            .tasks
            .lock()
            .await
            .insert(session_id, ExtensionLoadingTask::new(handle));
        if let Some(replaced) = replaced {
            replaced.abort();
        }
    }

    async fn spawn_if_absent<F>(&self, session_id: String, spawn: F) -> bool
    where
        F: FnOnce() -> JoinHandle<Vec<ExtensionLoadResult>>,
    {
        let mut tasks = self.tasks.lock().await;
        if tasks.contains_key(&session_id) {
            return false;
        }
        tasks.insert(session_id, ExtensionLoadingTask::new(spawn()));
        true
    }

    async fn wait(
        &self,
        session_id: &str,
    ) -> Result<Option<Vec<ExtensionLoadResult>>, Arc<JoinError>> {
        let result = self
            .tasks
            .lock()
            .await
            .get(session_id)
            .map(|task| task.result.clone());
        match result {
            Some(result) => result.await.map(Some),
            None => Ok(None),
        }
    }

    async fn remove(&self, session_id: &str) -> bool {
        let task = self.tasks.lock().await.remove(session_id);
        if let Some(task) = task {
            task.abort();
            true
        } else {
            false
        }
    }

    async fn abort_all(&self) {
        let tasks = std::mem::take(&mut *self.tasks.lock().await);
        for (_, task) in tasks {
            task.abort();
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub(crate) agent_manager: Arc<AgentManager>,
    extension_loading_tasks: ExtensionLoadingTasks,
    /// Bounded like the agent LRU: SSE clients rarely call `stop_agent`, so
    /// without eviction each abandoned session would pin its bus (and up to
    /// 8 MiB of replay buffer) for the lifetime of the server. In-flight
    /// subscribers hold their own `Arc` clone and are unaffected by eviction.
    session_buses: Arc<Mutex<LruCache<String, Arc<SessionEventBus>>>>,
    shutdown: CancellationToken,
}

impl AppState {
    pub async fn new(_tls: bool) -> anyhow::Result<Arc<AppState>> {
        register_builtin_extensions(gosling_mcp::BUILTIN_EXTENSIONS.clone());

        let agent_manager = AgentManager::instance().await?;
        let bus_capacity = Config::global()
            .get_gosling_max_active_agents()
            .ok()
            .and_then(NonZeroUsize::new)
            .unwrap_or_else(|| NonZeroUsize::new(DEFAULT_MAX_SESSION).unwrap());
        Ok(Arc::new(Self {
            agent_manager,
            extension_loading_tasks: ExtensionLoadingTasks::default(),
            session_buses: Arc::new(Mutex::new(LruCache::new(bus_capacity))),
            shutdown: CancellationToken::new(),
        }))
    }

    pub async fn set_extension_loading_task(
        &self,
        session_id: String,
        task: JoinHandle<Vec<ExtensionLoadResult>>,
    ) {
        self.extension_loading_tasks.replace(session_id, task).await;
    }

    pub async fn spawn_extension_loading_task_if_absent<F>(
        &self,
        session_id: String,
        spawn: F,
    ) -> bool
    where
        F: FnOnce() -> JoinHandle<Vec<ExtensionLoadResult>>,
    {
        self.extension_loading_tasks
            .spawn_if_absent(session_id, spawn)
            .await
    }

    pub async fn take_extension_loading_task(
        &self,
        session_id: &str,
    ) -> Result<Option<Vec<ExtensionLoadResult>>, Arc<JoinError>> {
        self.extension_loading_tasks.wait(session_id).await
    }

    pub async fn remove_extension_loading_task(&self, session_id: &str) {
        self.extension_loading_tasks.remove(session_id).await;
    }

    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    pub async fn shutdown(&self) {
        self.extension_loading_tasks.abort_all().await;
        self.agent_manager.shutdown().await;
        self.shutdown.cancel();
    }

    pub fn session_manager(&self) -> &SessionManager {
        self.agent_manager.session_manager()
    }

    pub async fn get_or_create_event_bus(&self, session_id: &str) -> Arc<SessionEventBus> {
        let mut buses = self.session_buses.lock().await;
        buses
            .get_or_insert(session_id.to_string(), || Arc::new(SessionEventBus::new()))
            .clone()
    }

    /// Get an existing event bus for a session without creating one.
    pub async fn get_event_bus(&self, session_id: &str) -> Option<Arc<SessionEventBus>> {
        let mut buses = self.session_buses.lock().await;
        buses.get(session_id).cloned()
    }

    /// Drop a session's event bus when the session is stopped so buses (each of
    /// which retains a replay buffer of events) do not accumulate for the whole
    /// lifetime of the server. Active SSE subscribers hold their own `Arc` clone,
    /// so an in-flight stream keeps working until it finishes. Not called on
    /// restart, where the same session and its subscribers persist.
    pub async fn remove_event_bus(&self, session_id: &str) {
        let mut buses = self.session_buses.lock().await;
        buses.pop(session_id);
    }

    pub async fn get_agent(
        &self,
        session_id: String,
    ) -> anyhow::Result<Arc<gosling::agents::Agent>> {
        self.agent_manager.get_or_create_agent(session_id).await
    }

    pub async fn get_agent_for_route(
        &self,
        session_id: String,
    ) -> Result<Arc<gosling::agents::Agent>, StatusCode> {
        self.get_agent(session_id).await.map_err(|e| {
            tracing::error!("Failed to get agent: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    async fn wait_for_flag(flag: &AtomicBool) {
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while !flag.load(Ordering::SeqCst) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn replacement_aborts_previous_extension_loader() {
        let tasks = ExtensionLoadingTasks::default();
        let started = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));
        let started_for_task = started.clone();
        let dropped_for_task = dropped.clone();
        tasks
            .replace(
                "session".to_string(),
                tokio::spawn(async move {
                    let _drop_flag = DropFlag(dropped_for_task);
                    started_for_task.store(true, Ordering::SeqCst);
                    std::future::pending::<Vec<ExtensionLoadResult>>().await
                }),
            )
            .await;
        wait_for_flag(&started).await;

        tasks
            .replace("session".to_string(), tokio::spawn(async { Vec::new() }))
            .await;

        wait_for_flag(&dropped).await;
        tasks.remove("session").await;
    }

    #[tokio::test]
    async fn removal_aborts_extension_loader() {
        let tasks = ExtensionLoadingTasks::default();
        let started = Arc::new(AtomicBool::new(false));
        let dropped = Arc::new(AtomicBool::new(false));
        let started_for_task = started.clone();
        let dropped_for_task = dropped.clone();
        tasks
            .replace(
                "session".to_string(),
                tokio::spawn(async move {
                    let _drop_flag = DropFlag(dropped_for_task);
                    started_for_task.store(true, Ordering::SeqCst);
                    std::future::pending::<Vec<ExtensionLoadResult>>().await
                }),
            )
            .await;
        wait_for_flag(&started).await;

        assert!(tasks.remove("session").await);
        wait_for_flag(&dropped).await;
    }

    #[tokio::test]
    async fn concurrent_registration_spawns_one_extension_loader() {
        let tasks = ExtensionLoadingTasks::default();
        let spawn_count = Arc::new(AtomicUsize::new(0));
        let left_tasks = tasks.clone();
        let right_tasks = tasks.clone();
        let left_count = spawn_count.clone();
        let right_count = spawn_count.clone();

        let (left, right) = tokio::join!(
            left_tasks.spawn_if_absent("session".to_string(), move || {
                left_count.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(std::future::pending::<Vec<ExtensionLoadResult>>())
            }),
            right_tasks.spawn_if_absent("session".to_string(), move || {
                right_count.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(std::future::pending::<Vec<ExtensionLoadResult>>())
            })
        );

        assert_ne!(left, right);
        assert_eq!(spawn_count.load(Ordering::SeqCst), 1);
        tasks.remove("session").await;
    }
}
