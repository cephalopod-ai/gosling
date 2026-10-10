use gosling::session::export_bundle::{write_bundle, MAX_PART_BYTES};
use serde_json::json;
use std::fs;

#[tokio::test]
async fn native_large_session_uses_bundle_without_weakening_ordinary_export() {
    use gosling::config::GoslingMode;
    use gosling::conversation::message::Message;
    use gosling::session::{SessionManager, SessionType};

    let temporary = tempfile::tempdir().unwrap();
    let manager = SessionManager::new(temporary.path().join("data"));
    let session = manager
        .create_session(
            temporary.path().to_path_buf(),
            "large native export".to_string(),
            SessionType::User,
            GoslingMode::default(),
        )
        .await
        .unwrap();
    let text = "🐦".repeat(4_300_000);
    manager
        .add_message(&session.id, &Message::user().with_text(&text))
        .await
        .unwrap();

    assert!(manager.export_session(&session.id).await.is_err());
    let snapshot = manager
        .export_session_snapshot_for_bundle(&session.id)
        .await
        .unwrap();
    assert!(snapshot.len() > 16 * 1024 * 1024);
    assert!(snapshot.contains(&text));
    let directory = temporary.path().join("bundle");
    let manifest = write_bundle(&directory, &session.id, &snapshot, true).unwrap();
    let mut assembled = Vec::new();
    for part in &manifest.parts {
        assembled.extend(fs::read(directory.join(&part.file)).unwrap());
    }
    assert!(assembled == snapshot.as_bytes());
}

#[test]
fn large_native_snapshot_roundtrips_with_plan_history_and_private_parts() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("bundle");
    let snapshot = serde_json::to_string(&json!({
        "id": "session-test", "conversation": {"messages": [
            {"role": "user", "content": "🐦".repeat(4_300_000)}
        ]},
        "plan_history_v1": {"plans": [{"title": "keep historical plan", "revisions": [1, 2]}]}
    }))
    .unwrap();
    assert!(snapshot.len() > 16 * 1024 * 1024);
    let manifest = write_bundle(&directory, "session-test", &snapshot, true).unwrap();
    let mut assembled = Vec::new();
    for part in &manifest.parts {
        let bytes = fs::read(directory.join(&part.file)).unwrap();
        assert!(bytes.len() <= MAX_PART_BYTES);
        assert_eq!(assembled.len(), part.offset);
        assert!(std::str::from_utf8(&bytes).is_ok());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(directory.join(&part.file))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        assembled.extend(bytes);
    }
    assert_eq!(assembled, snapshot.as_bytes());
    assert_eq!(
        write_bundle(&directory, "session-test", &snapshot, true).unwrap(),
        manifest
    );
}

#[test]
fn interrupted_bundle_resumes_without_replacing_completed_parts() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("bundle");
    let snapshot = serde_json::to_string(
        &json!({"id": "session-test", "content": "x".repeat(MAX_PART_BYTES * 2)}),
    )
    .unwrap();
    let manifest = write_bundle(&directory, "session-test", &snapshot, true).unwrap();
    let first_before = fs::metadata(directory.join(&manifest.parts[0].file))
        .unwrap()
        .modified()
        .unwrap();
    fs::remove_file(directory.join("manifest.json")).unwrap();
    fs::remove_file(directory.join(&manifest.parts[1].file)).unwrap();
    assert_eq!(
        write_bundle(&directory, "session-test", &snapshot, true).unwrap(),
        manifest
    );
    assert_eq!(
        fs::metadata(directory.join(&manifest.parts[0].file))
            .unwrap()
            .modified()
            .unwrap(),
        first_before
    );
}

#[test]
fn snapshot_change_or_corrupt_part_never_overwrites_user_data() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("bundle");
    let snapshot = r#"{"id":"session-test","content":"first"}"#;
    let manifest = write_bundle(&directory, "session-test", snapshot, true).unwrap();
    assert!(write_bundle(
        &directory,
        "session-test",
        r#"{"id":"session-test","content":"second"}"#,
        true
    )
    .is_err());
    let part = directory.join(&manifest.parts[0].file);
    fs::write(&part, b"damaged").unwrap();
    assert!(write_bundle(&directory, "session-test", snapshot, true).is_err());
    assert_eq!(fs::read(part).unwrap(), b"damaged");
}

#[cfg(unix)]
#[test]
fn symlink_output_is_refused() {
    let temporary = tempfile::tempdir().unwrap();
    let actual = temporary.path().join("actual");
    fs::create_dir(&actual).unwrap();
    let link = temporary.path().join("link");
    std::os::unix::fs::symlink(&actual, &link).unwrap();
    assert!(write_bundle(&link, "session-test", r#"{"id":"session-test"}"#, true).is_err());
    assert!(fs::read_dir(actual).unwrap().next().is_none());
}

#[test]
fn orphaned_atomic_write_does_not_prevent_resume() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("bundle");
    let snapshot = r#"{"id":"session-test","content":"same snapshot"}"#;
    let manifest = write_bundle(&directory, "session-test", snapshot, true).unwrap();
    fs::remove_file(directory.join("manifest.json")).unwrap();
    fs::remove_file(directory.join(&manifest.parts[0].file)).unwrap();
    fs::write(
        directory.join(".staging/interrupted-write"),
        b"partial bytes",
    )
    .unwrap();
    assert_eq!(
        write_bundle(&directory, "session-test", snapshot, true).unwrap(),
        manifest
    );
}

#[cfg(unix)]
#[test]
fn dangling_manifest_and_completed_staging_symlinks_are_refused() {
    let temporary = tempfile::tempdir().unwrap();
    let directory = temporary.path().join("bundle");
    fs::create_dir(&directory).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
    std::os::unix::fs::symlink("missing", directory.join("manifest.json")).unwrap();
    let snapshot = r#"{"id":"session-test"}"#;
    assert!(write_bundle(&directory, "session-test", snapshot, true).is_err());
    assert!(!directory.join("intent.json").exists());
    fs::remove_file(directory.join("manifest.json")).unwrap();
    write_bundle(&directory, "session-test", snapshot, true).unwrap();
    fs::remove_dir(directory.join(".staging")).unwrap();
    std::os::unix::fs::symlink(temporary.path(), directory.join(".staging")).unwrap();
    assert!(write_bundle(&directory, "session-test", snapshot, true).is_err());
}
