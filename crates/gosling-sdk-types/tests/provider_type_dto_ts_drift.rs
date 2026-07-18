//! Drift guard: `ProviderTypeDto` (this crate) is hand-mirrored as
//! `ProviderType` in `ui/desktop/src/types/providers.ts`, because that
//! crate/app boundary can't share a generated type (see AGENTS.md: ui/desktop
//! must not import generated OpenAPI types). Nothing else keeps the two in
//! sync, so this test pins the exact wire strings `ProviderTypeDto` produces;
//! if it fails, update `ui/desktop/src/types/providers.ts`'s `ProviderType`
//! union to match before changing the expected set here.

use gosling_sdk_types::custom_requests::ProviderTypeDto;

const EXPECTED_TS_PROVIDER_TYPE_VALUES: &[&str] =
    &["Preferred", "Builtin", "Declarative", "Custom"];

#[test]
fn provider_type_dto_wire_values_match_ts_union() {
    let all_variants = [
        ProviderTypeDto::Preferred,
        ProviderTypeDto::Builtin,
        ProviderTypeDto::Declarative,
        ProviderTypeDto::Custom,
    ];

    assert_eq!(
        all_variants.len(),
        EXPECTED_TS_PROVIDER_TYPE_VALUES.len(),
        "a variant was added to or removed from ProviderTypeDto without updating this test \
         (and ui/desktop/src/types/providers.ts's ProviderType union)"
    );

    for (variant, expected) in all_variants.iter().zip(EXPECTED_TS_PROVIDER_TYPE_VALUES) {
        let serialized = serde_json::to_value(variant).expect("ProviderTypeDto serializes");
        assert_eq!(
            serialized.as_str(),
            Some(*expected),
            "ProviderTypeDto::{variant:?} no longer serializes to \"{expected}\" — \
             ui/desktop/src/types/providers.ts's ProviderType union mirrors this string \
             literally and must be updated to match"
        );
    }
}
