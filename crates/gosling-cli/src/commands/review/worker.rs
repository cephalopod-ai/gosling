use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::orchestrator::MAX_WORKERS;

#[derive(Clone)]
pub struct ReviewWorkerPool {
    repository_root: Arc<PathBuf>,
    slots: Arc<Semaphore>,
}

impl ReviewWorkerPool {
    pub fn new(repository_root: PathBuf) -> Self {
        Self::with_limit(repository_root, MAX_WORKERS)
    }

    fn with_limit(repository_root: PathBuf, limit: usize) -> Self {
        Self {
            repository_root: Arc::new(repository_root),
            slots: Arc::new(Semaphore::new(limit)),
        }
    }

    async fn acquire_slot(&self) -> OwnedSemaphorePermit {
        self.slots
            .clone()
            .acquire_owned()
            .await
            .expect("review worker semaphore is never closed")
    }

    pub async fn run(
        &self,
        prompt: &str,
        label: &str,
        provider: Option<&str>,
        model: Option<&str>,
        max_turns: Option<usize>,
    ) -> Result<Output> {
        let _permit = self.acquire_slot().await;
        let gosling_bin = std::env::current_exe().context("locate current gosling binary")?;
        let mut cmd = worker_command(&gosling_bin, &self.repository_root);

        if let Some(provider) = provider {
            cmd.arg("--provider").arg(provider);
        }
        if let Some(model) = model {
            cmd.arg("--model").arg(model);
        }
        if let Some(max_turns) = max_turns {
            cmd.arg("--max-turns").arg(max_turns.to_string());
        }

        let mut child = cmd
            .spawn()
            .with_context(|| format!("spawn subprocess for {label}"))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(prompt.as_bytes())
                .await
                .with_context(|| format!("write prompt to {label} stdin"))?;
        }

        child
            .wait_with_output()
            .await
            .with_context(|| format!("wait on {label}"))
    }
}

fn worker_command(gosling_bin: &Path, repository_root: &Path) -> Command {
    let mut cmd = Command::new(gosling_bin);
    cmd.arg("run")
        .arg("--no-session")
        .arg("--quiet")
        .arg("--no-profile")
        .arg("-i")
        .arg("-")
        .current_dir(repository_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::{watch, Barrier};
    use tokio::task::JoinSet;

    #[test]
    fn worker_command_uses_repository_root() {
        let command = worker_command(Path::new("/bin/echo"), Path::new("/repo/root"));
        assert_eq!(
            command.as_std().get_current_dir(),
            Some(Path::new("/repo/root"))
        );
    }

    #[tokio::test]
    async fn one_pool_caps_workers_across_concurrent_phases() {
        let pool = ReviewWorkerPool::with_limit(PathBuf::from("/repo/root"), 2);
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let first_batch = Arc::new(Barrier::new(3));
        let (release_tx, release_rx) = watch::channel(false);
        let mut tasks = JoinSet::new();

        for _ in 0..8 {
            let pool = pool.clone();
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            let first_batch = Arc::clone(&first_batch);
            let mut release_rx = release_rx.clone();
            tasks.spawn(async move {
                let _permit = pool.acquire_slot().await;
                let now_active = active.fetch_add(1, Ordering::SeqCst) + 1;
                peak.fetch_max(now_active, Ordering::SeqCst);
                if !*release_rx.borrow() {
                    first_batch.wait().await;
                    release_rx.changed().await.unwrap();
                }
                active.fetch_sub(1, Ordering::SeqCst);
            });
        }

        first_batch.wait().await;
        assert_eq!(active.load(Ordering::SeqCst), 2);
        release_tx.send(true).unwrap();

        while let Some(result) = tasks.join_next().await {
            result.unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }
}
