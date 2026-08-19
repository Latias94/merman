use merman_theme_contract::{
    DiagramThemeSpecWireV1, PresetExportV1, ThemeDefinitionV1, ThemeTokensV1,
};

#[test]
fn preset_export_variants_round_trip_with_closed_tags() {
    let cases = [
        PresetExportV1::Definition {
            definition: ThemeDefinitionV1::new(ThemeTokensV1::default()),
        },
        PresetExportV1::CompleteSpec {
            complete_spec: DiagramThemeSpecWireV1::default(),
        },
    ];

    for export in cases {
        let encoded = serde_json::to_value(&export).expect("preset export must serialize");
        assert_eq!(encoded["kind"], export.kind());
        let decoded: PresetExportV1 =
            serde_json::from_value(encoded).expect("preset export must deserialize");
        assert_eq!(decoded, export);
    }
}

#[test]
fn preset_export_rejects_unknown_missing_duplicate_and_untagged_shapes() {
    let definition = serde_json::to_value(ThemeDefinitionV1::new(ThemeTokensV1::default()))
        .expect("definition must serialize");
    let complete_spec =
        serde_json::to_value(DiagramThemeSpecWireV1::default()).expect("spec must serialize");
    let invalid = [
        serde_json::json!({ "kind": "future", "definition": definition.clone() }),
        serde_json::json!({ "kind": "definition" }),
        serde_json::json!({
            "kind": "definition",
            "definition": definition.clone(),
            "complete_spec": complete_spec.clone(),
        }),
        serde_json::json!({ "definition": definition }),
        serde_json::json!({ "kind": "complete-spec", "complete_spec": complete_spec }),
    ];

    for value in invalid {
        assert!(
            serde_json::from_value::<PresetExportV1>(value).is_err(),
            "invalid preset export shape must fail closed"
        );
    }
}
