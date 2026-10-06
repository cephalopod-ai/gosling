use gosling::session::export_bundle::{write_bundle, MAX_PART_BYTES};
use serde_json::json;
use std::fs;

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
