// Owns background-task state, notification bridging, result collection, and cleanup.
// Extracted from `summon.rs` in a behavior-preserving modularization.
// The `summon` compatibility facade re-exports `BackgroundTask` and `CompletedTask`.

use super::*;

pub struct BackgroundTask {
    pub id: String,
    pub description: String,
    pub started_at: Instant,
    pub turns: Arc<AtomicU32>,
    pub last_activity: Arc<AtomicU64>,
    pub handle: JoinHandle<Result<String>>,
    pub cancellation_token: CancellationToken,
    pub notification_buffer: Arc<Mutex<Vec<ServerNotification>>>,
    pub(super) _slot: OwnedSemaphorePermit,
}

pub struct CompletedTask {
    pub id: String,
    pub description: String,
    pub result: Result<String, String>,
    pub turns_taken: u32,
    pub duration: Duration,
    pub completed_at: Instant,
}

/// Result from handle_load_task_result with structured metadata for the caller
#[derive(Debug)]
pub(super) struct TaskLoadResult {
    pub(super) content: Vec<Content>,
    pub(super) status: &'static str,
    pub(super) turns: Option<u32>,
    pub(super) duration_secs: Option<u64>,
}

pub(super) fn round_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", (secs / 10) * 10)
    } else {
        format!("{}m", secs / 60)
    }
}

pub(super) fn current_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Get maximum number of concurrent background tasks
pub(super) fn max_background_tasks() -> usize {
    Config::global()
        .get_param::<usize>("GOSLING_MAX_BACKGROUND_TASKS")
        .unwrap_or(5)
}

fn completed_task_ttl() -> Duration {
    let secs = Config::global()
        .get_param::<u64>("GOSLING_COMPLETED_TASK_TTL_SECS")
        .unwrap_or(600);
    Duration::from_secs(secs)
}

pub(super) fn is_session_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('_').collect();
    parts.len() == 2 && parts[0].len() == 8 && parts[0].chars().all(|c| c.is_ascii_digit())
}

impl Drop for SummonClient {
    fn drop(&mut self) {
        // Best-effort cancellation of running tasks on shutdown
        if let Ok(tasks) = self.background_tasks.try_lock() {
            for task in tasks.values() {
                task.cancellation_token.cancel();
            }
        }
    }
}

impl SummonClient {
    pub(super) fn with_background_task_limit(
        context: PlatformExtensionContext,
        max_background_tasks: usize,
    ) -> Result<Self> {
        let info = InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(EXTENSION_NAME, "1.0.0").with_title("Summon"));

        Ok(Self {
            info,
            context,
            source_cache: Mutex::new(None),
            background_task_slots: Arc::new(Semaphore::new(max_background_tasks)),
            max_background_tasks,
            background_tasks: Mutex::new(HashMap::new()),
            completed_tasks: Mutex::new(HashMap::new()),
            notification_subscribers: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub(super) fn try_reserve_background_task_slot(&self) -> Result<OwnedSemaphorePermit, String> {
        self.background_task_slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                format!(
                    "Maximum {} background tasks already running. Wait for completion or use sync mode.",
                    self.max_background_tasks
                )
            })
    }

    pub(super) fn spawn_notification_bridge(
        mut notif_rx: tokio::sync::mpsc::UnboundedReceiver<ServerNotification>,
        subscribers: Arc<Mutex<Vec<mpsc::Sender<ServerNotification>>>>,
        buffer: Arc<Mutex<Vec<ServerNotification>>>,
    ) {
        // With no subscribers the buffer holds notifications until someone
        // peeks/loads; keep only the newest so an unobserved chatty subagent
        // can't grow it for its whole run.
        const MAX_BUFFERED_NOTIFICATIONS: usize = 256;
        tokio::spawn(async move {
            while let Some(notification) = notif_rx.recv().await {
                let mut subs = subscribers.lock().await;
                if subs.is_empty() {
                    drop(subs);
                    let mut buffer = buffer.lock().await;
                    if buffer.len() >= MAX_BUFFERED_NOTIFICATIONS {
                        let excess = buffer.len() + 1 - MAX_BUFFERED_NOTIFICATIONS;
                        buffer.drain(..excess);
                    }
                    buffer.push(notification);
                } else {
                    subs.retain(|tx| match tx.try_send(notification.clone()) {
                        Ok(()) => true,
                        Err(mpsc::error::TrySendError::Full(_)) => true,
                        Err(mpsc::error::TrySendError::Closed(_)) => false,
                    });
                }
            }
        });
    }

    pub(super) async fn handle_load_task_result(
        &self,
        task_id: &str,
        cancel: bool,
        peek: bool,
    ) -> Result<TaskLoadResult, String> {
        let mut completed = self.completed_tasks.lock().await;

        let completed_entry = if peek {
            completed.get(task_id).map(|task| {
                (
                    task.result.clone(),
                    task.description.clone(),
                    task.duration,
                    task.turns_taken,
                )
            })
        } else {
            completed.remove(task_id).map(|task| {
                (
                    task.result,
                    task.description,
                    task.duration,
                    task.turns_taken,
                )
            })
        };

        if let Some((result, description, duration, turns_taken)) = completed_entry {
            let status_key = match &result {
                Ok(_) => "completed",
                Err(e) if e.starts_with("Task panicked:") => "panicked",
                Err(_) => "failed",
            };
            let status = match status_key {
                "completed" => "✓ Completed",
                "panicked" => "✗ Panicked",
                _ => "✗ Failed",
            };
            let output = match result {
                Ok(output) => output,
                Err(error) => format!("Error: {}", error),
            };
            return Ok(TaskLoadResult {
                content: vec![Content::text(format!(
                    "# Background Task Result: {}\n\n\
                     **Task:** {}\n\
                     **Status:** {}\n\
                     **Duration:** {} ({} turns)\n\n\
                     ## Output\n\n{}",
                    task_id,
                    description,
                    status,
                    round_duration(duration),
                    turns_taken,
                    output
                ))],
                status: status_key,
                turns: Some(turns_taken),
                duration_secs: Some(duration.as_secs()),
            });
        }

        drop(completed);

        let mut running = self.background_tasks.lock().await;
        if running.contains_key(task_id) {
            if peek {
                let task = running.get(task_id).unwrap();
                let elapsed = task.started_at.elapsed();
                let turns_taken = task.turns.load(Ordering::Relaxed);
                let now = current_epoch_millis();
                let idle_ms = now.saturating_sub(task.last_activity.load(Ordering::Relaxed));
                let description = task.description.clone();

                let buffered_count = task.notification_buffer.lock().await.len();

                drop(running);

                let mut output = format!(
                    "# Background Task Status: {}\n\n**Task:** {}\n**Status:** ⏳ Running\n**Elapsed:** {}\n**Turns taken:** {}\n**Idle:** {}\n**Buffered tool calls:** {}",
                    task_id,
                    description,
                    round_duration(elapsed),
                    turns_taken,
                    round_duration(Duration::from_millis(idle_ms)),
                    buffered_count,
                );

                if buffered_count == 0 && turns_taken == 0 {
                    output.push_str("\n\n_Task is initialising (no tool activity yet)._");
                }

                return Ok(TaskLoadResult {
                    content: vec![Content::text(output)],
                    status: "running",
                    turns: Some(turns_taken),
                    duration_secs: Some(elapsed.as_secs()),
                });
            }

            if cancel {
                let task = running.remove(task_id).unwrap();
                drop(running);

                task.cancellation_token.cancel();

                let duration = task.started_at.elapsed();
                let turns_taken = task.turns.load(Ordering::Relaxed);

                let mut handle = task.handle;
                let output = tokio::select! {
                    result = &mut handle => {
                        match result {
                            Ok(Ok(s)) => s,
                            Ok(Err(e)) => format!("Error: {}", e),
                            Err(e) => format!("Task panicked: {}", e),
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {
                        handle.abort();
                        "Task did not stop in time (aborted)".to_string()
                    }
                };

                return Ok(TaskLoadResult {
                    content: vec![Content::text(format!(
                        "# Background Task Result: {}\n\n\
                         **Task:** {}\n\
                         **Status:** ⊘ Cancelled\n\
                         **Duration:** {} ({} turns)\n\n\
                         ## Output\n\n{}",
                        task_id,
                        task.description,
                        round_duration(duration),
                        turns_taken,
                        output
                    ))],
                    status: "cancelled",
                    turns: Some(turns_taken),
                    duration_secs: Some(duration.as_secs()),
                });
            }

            // Wait for the running task to complete, keeping the tool call
            // alive so notifications (subagent tool calls) stream in real time.
            let mut task = running.remove(task_id).unwrap();
            drop(running);

            let buffered = {
                let mut buf = task.notification_buffer.lock().await;
                std::mem::take(&mut *buf)
            };
            if !buffered.is_empty() {
                let subs = self.notification_subscribers.lock().await;
                for notif in buffered {
                    for tx in subs.iter() {
                        let _ = tx.try_send(notif.clone());
                    }
                }
            }

            tokio::select! {
                result = &mut task.handle => {
                    let (output, status_key) = match result {
                        Ok(Ok(s)) => (s, "completed"),
                        Ok(Err(e)) => (format!("Error: {}", e), "failed"),
                        Err(e) => (format!("Task panicked: {}", e), "panicked"),
                    };

                    let turns_taken = task.turns.load(Ordering::Relaxed);
                    let elapsed = task.started_at.elapsed();
                    let status_display = match status_key {
                        "completed" => "✓ Completed",
                        "panicked" => "✗ Panicked",
                        _ => "✗ Failed",
                    };
                    return Ok(TaskLoadResult {
                        content: vec![Content::text(format!(
                            "# Background Task Result: {}\n\n\
                             **Task:** {}\n\
                             **Status:** {}\n\
                             **Duration:** {} ({} turns)\n\n\
                             ## Output\n\n{}",
                            task_id,
                            task.description,
                            status_display,
                            round_duration(elapsed),
                            turns_taken,
                            output
                        ))],
                        status: status_key,
                        turns: Some(turns_taken),
                        duration_secs: Some(elapsed.as_secs()),
                    });
                }
                _ = tokio::time::sleep(Duration::from_secs(300)) => {
                    self.background_tasks.lock().await.insert(task_id.to_string(), task);

                    return Err(format!(
                        "Task '{task_id}' is still running after waiting 5 min. \
                         Use load(source: \"{task_id}\") to wait again, or \
                         load(source: \"{task_id}\", cancel: true) to stop."
                    ));
                }
            }
        }

        Err(format!("Task '{}' not found.", task_id))
    }

    pub(super) async fn cleanup_completed_tasks(&self) {
        let finished: Vec<(String, BackgroundTask)> = {
            let mut tasks = self.background_tasks.lock().await;
            let ids: Vec<String> = tasks
                .iter()
                .filter(|(_, t)| t.handle.is_finished())
                .map(|(id, _)| id.clone())
                .collect();
            ids.into_iter()
                .filter_map(|id| tasks.remove(&id).map(|t| (id, t)))
                .collect()
        };

        let mut completed = self.completed_tasks.lock().await;

        for (id, task) in finished {
            let duration = task.started_at.elapsed();
            let turns_taken = task.turns.load(Ordering::Relaxed);

            let result = match task.handle.await {
                Ok(Ok(output)) => {
                    info!("Background task {} completed successfully", id);
                    Ok(output)
                }
                Ok(Err(e)) => {
                    warn!("Background task {} failed: {}", id, e);
                    Err(e.to_string())
                }
                Err(e) => {
                    warn!("Background task {} panicked: {}", id, e);
                    Err(format!("Task panicked: {}", e))
                }
            };

            completed.insert(
                id.clone(),
                CompletedTask {
                    id,
                    description: task.description,
                    result,
                    turns_taken,
                    duration,
                    completed_at: Instant::now(),
                },
            );
        }

        let ttl = completed_task_ttl();
        completed.retain(|_id, task| task.completed_at.elapsed() <= ttl);
    }

    pub(super) fn get_task_description(params: &DelegateParams) -> String {
        match (&params.source, &params.instructions) {
            (Some(source), Some(instructions)) => format!("{}: {}", source, instructions),
            (Some(source), None) => source.clone(),
            (None, Some(instructions)) => instructions.clone(),
            (None, None) => "Unknown task".to_string(),
        }
    }
}
