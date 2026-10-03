use anyhow::Result;
use gosling::agents::{extension_manager::ExtensionManager, moim::inject_moim};
use gosling::config::GoslingMode;
use gosling::conversation::{message::Message, Conversation};
use gosling::session::SessionType;

#[tokio::test]
async fn added_folders_reach_model_context_after_session_reload() -> Result<()> {
    let data = tempfile::tempdir()?;
    let primary = tempfile::tempdir()?;
    let additional = tempfile::tempdir()?;
    let extension_manager = ExtensionManager::new_without_provider(data.path().to_path_buf());
    let session = extension_manager
        .get_context()
        .session_manager
        .create_session(
            primary.path().to_path_buf(),
            "working folder recall".to_string(),
            SessionType::Hidden,
            GoslingMode::Auto,
        )
        .await?;
    let conversation =
        Conversation::new_unvalidated(vec![Message::user().with_text("My folders?")]);

    let initial = inject_moim(&session.id, &conversation, &extension_manager, 0, 100)
        .await
        .expect("turn context is available");
    assert!(!initial.messages()[0].content[0]
        .as_text()
        .unwrap()
        .contains("<additional-working-directory>"));

    extension_manager
        .get_context()
        .session_manager
        .update(&session.id)
        .additional_working_dirs(vec![additional.path().to_path_buf()])
        .apply()
        .await?;
    let live = inject_moim(&session.id, &conversation, &extension_manager, 0, 100)
        .await
        .expect("turn context refreshes during the same chat");
    let added_path = format!(
        "<additional-working-directory>{}</additional-working-directory>",
        additional.path().display()
    );
    assert!(live.messages()[0].content[0]
        .as_text()
        .unwrap()
        .contains(&added_path));
    drop(extension_manager);

    let reopened = ExtensionManager::new_without_provider(data.path().to_path_buf());
    let restored = inject_moim(&session.id, &conversation, &reopened, 0, 100)
        .await
        .expect("turn context is available after reopening");
    let context = restored.messages()[0].content[0].as_text().unwrap();
    assert!(context.contains(&format!(
        "<working-directory>{}</working-directory>",
        primary.path().display()
    )));
    assert!(context.contains(&added_path));
    assert_eq!(conversation.messages()[0].content.len(), 1);
    Ok(())
}
