use gosling_sdk_types::custom_requests::ProviderTypeDto;

#[test]
fn provider_type_dto_wire_format_is_pinned() {
    assert_eq!(
        serde_json::to_value(ProviderTypeDto::Preferred).unwrap(),
        serde_json::json!("Preferred")
    );
    assert_eq!(
        serde_json::to_value(ProviderTypeDto::Builtin).unwrap(),
        serde_json::json!("Builtin")
    );
    assert_eq!(
        serde_json::to_value(ProviderTypeDto::Declarative).unwrap(),
        serde_json::json!("Declarative")
    );
    assert_eq!(
        serde_json::to_value(ProviderTypeDto::Custom).unwrap(),
        serde_json::json!("Custom")
    );
}

#[test]
fn provider_type_dto_deserializes_known_values() {
    assert_eq!(
        serde_json::from_value::<ProviderTypeDto>(serde_json::json!("Preferred")).unwrap(),
        ProviderTypeDto::Preferred
    );
    assert_eq!(
        serde_json::from_value::<ProviderTypeDto>(serde_json::json!("Builtin")).unwrap(),
        ProviderTypeDto::Builtin
    );
    assert_eq!(
        serde_json::from_value::<ProviderTypeDto>(serde_json::json!("Declarative")).unwrap(),
        ProviderTypeDto::Declarative
    );
    assert_eq!(
        serde_json::from_value::<ProviderTypeDto>(serde_json::json!("Custom")).unwrap(),
        ProviderTypeDto::Custom
    );
}

#[test]
fn provider_type_dto_rejects_unknown_or_mismatched_case_values() {
    assert!(serde_json::from_value::<ProviderTypeDto>(serde_json::json!("preferred")).is_err());
    assert!(serde_json::from_value::<ProviderTypeDto>(serde_json::json!("Unknown")).is_err());
}
