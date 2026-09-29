//! Ctrl-C at a cliclack prompt cancels the command instead of killing the process with the
//! cursor hidden (GSL-PT-20260927-D10, GSL-PT-20260927-A04).

use anyhow::Context;
use gosling_cli::signal::{cancellable_prompts, is_prompt_cancellation, PromptCancelled};
use std::io;

#[cfg(unix)]
#[tokio::test]
async fn sigint_raised_by_a_prompt_cancels_the_flow_instead_of_killing_the_process() {
    let result: anyhow::Result<()> = cancellable_prompts(async {
        // What `console` does when it reads Ctrl-C from the raw-mode terminal.
        unsafe {
            libc::raise(libc::SIGINT);
        }
        std::future::pending().await
    })
    .await;

    assert!(result.unwrap_err().is::<PromptCancelled>());
}

#[tokio::test]
async fn an_interrupted_prompt_becomes_a_cancellation() {
    let result: anyhow::Result<()> = cancellable_prompts(async {
        Err(io::Error::from(io::ErrorKind::Interrupted)).context("select a session")
    })
    .await;

    let err = result.unwrap_err();
    assert!(err.is::<PromptCancelled>());
    assert_eq!(err.to_string(), "Cancelled.");
}

#[tokio::test]
async fn other_outcomes_pass_through_unchanged() {
    assert_eq!(cancellable_prompts(async { Ok(7) }).await.unwrap(), 7);

    let err = cancellable_prompts(async {
        Err::<(), _>(anyhow::Error::from(io::Error::from(
            io::ErrorKind::NotConnected,
        )))
    })
    .await
    .unwrap_err();
    assert!(!is_prompt_cancellation(&err));
    assert!(!err.is::<PromptCancelled>());
}
