use gosling::config::paths::Paths;
use gosling::providers::inventory::ProviderInventoryService;
use gosling::session::session_manager::SessionStorage;
use serial_test::serial;
use std::sync::Arc;

#[tokio::test]
#[serial]
async fn mistral_inventory_lists_mistral_large_4_and_zai_glm_5_3() {
    let root = tempfile::tempdir().unwrap();
    let root_path = root.path().to_string_lossy().to_string();
    let _env = env_lock::lock_env([
        ("GOSLING_PATH_ROOT", Some(root_path.as_str())),
        ("GOSLING_DISABLE_KEYRING", Some("1")),
        ("MISTRAL_API_KEY", None),
    ]);

    let inventory = ProviderInventoryService::new(Arc::new(SessionStorage::new(Paths::data_dir())));
    let entry = inventory
        .entry_for_provider("mistral")
        .await
        .expect("mistral inventory entry should load")
        .expect("mistral provider should be registered");

    let large_4 = entry
        .models
        .iter()
        .find(|model| model.id == "mistral-large-4")
        .expect("mistral-large-4 should be a selectable mistral model");
    assert_eq!(large_4.context_limit, Some(262_144));
    assert_eq!(large_4.family.as_deref(), Some("mistral-large"));

    let glm_5_3 = entry
        .models
        .iter()
        .find(|model| model.id == "zai-glm-5.3")
        .expect("zai-glm-5.3 should be a selectable mistral model");
    assert_eq!(glm_5_3.context_limit, Some(1_000_000));
    assert_eq!(glm_5_3.family.as_deref(), Some("glm"));
    assert_eq!(glm_5_3.reasoning, Some(true));
}
