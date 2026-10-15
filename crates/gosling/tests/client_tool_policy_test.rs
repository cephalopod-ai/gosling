use gosling::config::GoslingMode;
use gosling::conversation::message::ToolRequest;
use gosling::permission::client_tool_inspector::ClientToolInspector;
use gosling::session::{client_tool_policy, SessionManager, SessionType};
use gosling::tool_inspection::{InspectionAction, ToolInspectionManager, ToolInspector};
use rmcp::model::CallToolRequestParams;
use std::sync::Arc;

#[tokio::test]
async fn bounded_policy_survives_reload_and_requires_approval_in_auto() {
    let root = tempfile::tempdir().unwrap();
    let manager = Arc::new(SessionManager::new(root.path().into()));
    let session = manager
        .create_session(
            root.path().into(),
            "synthetic".into(),
            SessionType::Acp,
            GoslingMode::Auto,
        )
        .await
        .unwrap();
    let mut data = session.extension_data.clone();
    data.extension_states.insert(
        client_tool_policy::KEY.into(),
        serde_json::json!({"allowedExtensions": ["trilium"]}),
    );
    manager
        .update(&session.id)
        .extension_data(data)
        .apply()
        .await
        .unwrap();
    let reloaded = manager.get_session(&session.id, false).await.unwrap();
    assert!(client_tool_policy::is_bounded(&reloaded));
    assert!(client_tool_policy::allows_extension(&reloaded, "trilium"));
    assert!(!client_tool_policy::allows_extension(
        &reloaded,
        "developer"
    ));
    let inspector = ClientToolInspector(manager.clone());
    assert!(!inspector.auto_downgrades_require_approval());
    let requests = [ToolRequest {
        id: "read".into(),
        tool_call: Ok(CallToolRequestParams::new("trilium__get_note")),
        metadata: None,
        tool_meta: None,
    }];
    let mut inspections = ToolInspectionManager::new();
    inspections.add_inspector(Box::new(inspector));
    let results = inspections
        .inspect_tools(&session.id, &requests, &[], GoslingMode::Auto)
        .await
        .unwrap();
    assert_eq!(results[0].action, InspectionAction::RequireApproval(None));
}
