use merman_theme_contract::{
    DiagramThemeSpecWireV1, ThemeDefinitionV1, ThemeRecipeV1, ThemeTokensV1,
};

#[test]
fn theme_recipe_variants_round_trip_with_closed_tags() {
    let cases = [
        ThemeRecipeV1::Definition {
            definition: ThemeDefinitionV1::new(ThemeTokensV1::default()),
        },
        ThemeRecipeV1::CompleteSpec {
            complete_spec: DiagramThemeSpecWireV1::default(),
        },
    ];

    for export in cases {
        let encoded = serde_json::to_value(&export).expect("preset export must serialize");
        assert_eq!(encoded["kind"], export.kind());
        assert_eq!(encoded["schema_version"], 1);
        let decoded: ThemeRecipeV1 =
            serde_json::from_value(encoded).expect("preset export must deserialize");
        assert_eq!(decoded, export);
    }
}

#[test]
fn theme_recipe_rejects_unknown_missing_duplicate_and_untagged_shapes() {
    let definition = serde_json::to_value(ThemeDefinitionV1::new(ThemeTokensV1::default()))
        .expect("definition must serialize");
    let complete_spec =
        serde_json::to_value(DiagramThemeSpecWireV1::default()).expect("spec must serialize");
    let invalid = [
        serde_json::json!({ "schema_version": 1, "kind": "future", "definition": definition.clone() }),
        serde_json::json!({ "schema_version": 1, "kind": "definition" }),
        serde_json::json!({
            "schema_version": 1,
            "kind": "definition",
            "definition": definition.clone(),
            "complete_spec": complete_spec.clone(),
        }),
        serde_json::json!({ "schema_version": 1, "definition": definition }),
        serde_json::json!({ "schema_version": 1, "kind": "complete-spec", "complete_spec": complete_spec }),
    ];

    for value in invalid {
        assert!(
            serde_json::from_value::<ThemeRecipeV1>(value).is_err(),
            "invalid preset export shape must fail closed"
        );
    }
}

#[test]
fn recipe_envelope_requires_the_supported_schema_version() {
    for kind in ["definition", "complete_spec"] {
        let mut valid = serde_json::json!({ "kind": kind, "schema_version": 1 });
        valid[kind] = match kind {
            "definition" => serde_json::to_value(ThemeDefinitionV1::new(ThemeTokensV1::default()))
                .expect("definition must serialize"),
            _ => serde_json::to_value(DiagramThemeSpecWireV1::default())
                .expect("spec must serialize"),
        };
        for version in [
            serde_json::Value::Null,
            serde_json::json!(0),
            serde_json::json!(2),
            serde_json::json!(1.0),
            serde_json::json!("1"),
            serde_json::json!(true),
        ] {
            let mut invalid = valid.clone();
            invalid["schema_version"] = version;
            let error = serde_json::from_value::<ThemeRecipeV1>(invalid)
                .unwrap_err()
                .to_string();
            assert!(
                error.contains("schema_version"),
                "version diagnostic must identify the field: {error}"
            );
        }
        valid.as_object_mut().unwrap().remove("schema_version");
        let error = serde_json::from_value::<ThemeRecipeV1>(valid)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("schema_version"),
            "missing version must be actionable: {error}"
        );
    }
}

#[test]
fn recipe_preserves_structured_canvas_and_ordered_effects() {
    let complete_spec: DiagramThemeSpecWireV1 = serde_json::from_value(serde_json::json!({
        "canvas": { "base": "#051423", "layers": [{
            "paint": { "kind": "pattern", "pattern": "grid", "cell_width": 40,
                "cell_height": 40, "foreground": "#00f2ff22", "background": "#00000000" },
            "opacity": 0.5, "blend_mode": "screen"
        }] },
        "effects": [{ "kind": "graph", "id": "glow", "primitives": [
            { "kind": "drop-shadow", "input": "source-graphic", "offset_x": 0,
                "offset_y": 0, "blur_radius": 2, "spread": 0, "color": "#00f2ff" },
            { "kind": "drop-shadow", "input": "previous", "offset_x": 0,
                "offset_y": 0, "blur_radius": 6, "spread": 0, "color": "#00f2ff80" }
        ] }, { "kind": "binding", "effect_id": "glow", "target": "node" }]
    }))
    .expect("structured recipe must decode");
    let recipe = ThemeRecipeV1::CompleteSpec { complete_spec };
    let wire = serde_json::to_value(&recipe).expect("recipe must serialize");
    assert_eq!(wire["schema_version"], 1);
    assert_eq!(
        serde_json::from_value::<ThemeRecipeV1>(wire).unwrap(),
        recipe
    );
}

#[test]
fn recipe_rejects_duplicate_versions_and_invalid_inner_definition_versions() {
    let duplicate =
        r#"{"schema_version":1,"schema_version":1,"kind":"complete_spec","complete_spec":{}}"#;
    assert!(serde_json::from_str::<ThemeRecipeV1>(duplicate).is_err());
    let invalid_definition = serde_json::json!({
        "schema_version": 1,
        "kind": "definition",
        "definition": { "authoring_schema_version": 2, "expansion_version": 1, "tokens": {} }
    });
    let error = serde_json::from_value::<ThemeRecipeV1>(invalid_definition)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("authoring version tuple"),
        "inner definition version must remain validated: {error}"
    );
}

#[test]
fn recipe_rejects_future_versions_with_future_payload_fields() {
    let version_first =
        r#"{"schema_version":2,"kind":"complete_spec","complete_spec":{"future":true}}"#;
    let error = serde_json::from_str::<ThemeRecipeV1>(version_first)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("schema_version"),
        "leading unsupported version must be diagnosed: {error}"
    );
    let payload_first =
        r#"{"complete_spec":{"future":true},"schema_version":2,"kind":"complete_spec"}"#;
    assert!(serde_json::from_str::<ThemeRecipeV1>(payload_first).is_err());
}
