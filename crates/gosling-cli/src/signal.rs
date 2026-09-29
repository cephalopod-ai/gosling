use std::fmt;
use std::future::Future;
use std::io;
use std::pin::Pin;
use tokio::signal;

/// The user left an interactive prompt flow with Ctrl-C or Esc. `main` reports it and exits with
/// 130 instead of printing it as an error.
#[derive(Debug)]
pub struct PromptCancelled;

impl fmt::Display for PromptCancelled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Cancelled.")
    }
}

impl std::error::Error for PromptCancelled {}

pub fn is_prompt_cancellation(err: &anyhow::Error) -> bool {
    err.chain().any(|cause| {
        cause.is::<PromptCancelled>()
            || cause
                .downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::Interrupted)
    })
}

/// cliclack reads keys through `console`, which answers Ctrl-C by raising SIGINT. Under the
/// default disposition that signal kills the process before the prompt can show the cursor it
/// hid; with a handler installed the prompt cancels like Esc and returns `Interrupted`. The
/// handler is registered before `flow` first runs (hence `biased`), and a Ctrl-C that arrives
/// while `flow` awaits between prompts cancels it too.
pub async fn cancellable_prompts<T>(
    flow: impl Future<Output = anyhow::Result<T>>,
) -> anyhow::Result<T> {
    let result = tokio::select! {
        biased;
        Ok(()) = signal::ctrl_c() => Err(PromptCancelled.into()),
        result = flow => result,
    };
    result.map_err(|err| {
        if !is_prompt_cancellation(&err) {
            return err;
        }
        let stderr = console::Term::stderr();
        if stderr.is_term() {
            let _ = stderr.show_cursor();
        }
        PromptCancelled.into()
    })
}

#[cfg(unix)]
pub fn shutdown_signal() -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        #[cfg(unix)]
        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    })
}

#[cfg(not(unix))]
pub fn shutdown_signal() -> Pin<Box<dyn Future<Output = ()> + Send>> {
    Box::pin(async move {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    })
}
