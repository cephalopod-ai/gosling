use gosling::authentication::{
    session_authentication, AuthenticationSettings, ExtensionAuthentication,
    SESSION_AUTHENTICATION_KEY,
};
use gosling::config::GoslingMode;
use gosling::session::{SessionManager, SessionType};
use std::collections::BTreeMap;

fn disconnected_settings() -> AuthenticationSettings {
    AuthenticationSettings {
        provider_disconnected: true,
        extensions: BTreeMap::from([(
            "example".into(),
            ExtensionAuthentication {
                disconnected: true,
                ..Default::default()
            },
        )]),
    }
}

#[tokio::test]
async fn disconnect_survives_reload_and_keeps_other_chats_and_host_state() {
    let directory = tempfile::tempdir().unwrap();
    let sessions = SessionManager::new(directory.path().to_path_buf());
    let selected = sessions
        .create_session(
            directory.path().into(),
            "Selected".into(),
            SessionType::User,
            GoslingMode::Approve,
        )
        .await
        .unwrap();
    let other = sessions
        .create_session(
            directory.path().into(),
            "Other".into(),
            SessionType::User,
            GoslingMode::Approve,
        )
        .await
        .unwrap();
    sessions
        .merge_extension_state(
            &selected.id,
            "test-host.v1",
            serde_json::json!({"keep": true}),
        )
        .await
        .unwrap();
    sessions
        .set_authentication_profile(&selected.id, None, None, &disconnected_settings())
        .await
        .unwrap();
    drop(sessions);

    let reopened = SessionManager::new(directory.path().to_path_buf());
    let selected = reopened.get_session(&selected.id, false).await.unwrap();
    assert_eq!(
        session_authentication(&selected.extension_data).unwrap(),
        disconnected_settings()
    );
    assert_eq!(
        selected.extension_data.extension_states["test-host.v1"],
        serde_json::json!({"keep": true})
    );
    assert!(selected.credential_profile_id.is_none());
    assert!(
        !session_authentication(
            &reopened
                .get_session(&other.id, false)
                .await
                .unwrap()
                .extension_data
        )
        .unwrap()
        .provider_disconnected
    );
}

#[tokio::test]
async fn profile_and_disconnection_update_together_and_copy_keeps_the_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let sessions = SessionManager::new(directory.path().to_path_buf());
    let original = sessions
        .create_session(
            directory.path().into(),
            "Original".into(),
            SessionType::User,
            GoslingMode::Approve,
        )
        .await
        .unwrap();
    sessions
        .set_authentication_profile(&original.id, None, None, &disconnected_settings())
        .await
        .unwrap();
    let mut connected = disconnected_settings();
    connected.provider_disconnected = false;
    sessions
        .set_authentication_profile(
            &original.id,
            Some("fixture-profile"),
            Some("Fixture"),
            &connected,
        )
        .await
        .unwrap();
    let copy = sessions
        .copy_session(&original.id, "Copy".into())
        .await
        .unwrap();
    sessions
        .set_authentication_profile(&original.id, None, None, &disconnected_settings())
        .await
        .unwrap();
    let copy = sessions.get_session(&copy.id, false).await.unwrap();
    assert_eq!(
        copy.credential_profile_id.as_deref(),
        Some("fixture-profile")
    );
    assert_eq!(
        session_authentication(&copy.extension_data).unwrap(),
        connected
    );
}

#[test]
fn malformed_authentication_fails_instead_of_selecting_global_credentials() {
    let mut data = gosling::session::ExtensionData::default();
    assert_eq!(
        session_authentication(&data).unwrap(),
        AuthenticationSettings::default()
    );
    data.extension_states.insert(
        SESSION_AUTHENTICATION_KEY.into(),
        serde_json::json!({ "providerDisconnected": "true" }),
    );
    assert!(session_authentication(&data).is_err());
    data.extension_states.insert(SESSION_AUTHENTICATION_KEY.into(), serde_json::json!({ "extensions": { "example": { "credentialNamespace": "not-an-account" } } }));
    assert!(session_authentication(&data).is_err());
}

#[tokio::test]
async fn untrusted_import_does_not_reuse_local_authentication_references() {
    let directory = tempfile::tempdir().unwrap();
    let sessions = SessionManager::new(directory.path().to_path_buf());
    let original = sessions
        .create_session(
            directory.path().into(),
            "Original".into(),
            SessionType::User,
            GoslingMode::Approve,
        )
        .await
        .unwrap();
    sessions
        .set_authentication_profile(
            &original.id,
            Some("fixture-profile"),
            Some("Fixture"),
            &disconnected_settings(),
        )
        .await
        .unwrap();
    let document = sessions.export_session(&original.id).await.unwrap();
    let outcome = sessions
        .import_session(
            &document,
            None,
            directory.path().into(),
            gosling::session::import_formats::SessionImportTransport::Json,
        )
        .await
        .unwrap();
    let gosling::session::SessionImportOutcome::Imported(imported) = outcome else {
        panic!("expected new import")
    };
    assert!(imported.credential_profile_id.is_none());
    assert!(!imported
        .extension_data
        .extension_states
        .contains_key(SESSION_AUTHENTICATION_KEY));
}
