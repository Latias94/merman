use merman_render::diagram_theme::{
    DiagramThemeCompiler, ThemeResourceLimitId, ThemeResourcePolicy, materialize_theme_design,
    materialize_theme_design_json_with_resource_policy,
    materialize_theme_design_with_resource_policy,
};
use merman_theme_contract::{ThemeDesignV1, ThemeRecipeV1};
use serde_json::json;

fn decode(input: serde_json::Value) -> ThemeDesignV1 {
    serde_json::from_value(input).expect("typed theme design")
}

#[test]
fn typed_json_and_policy_operations_materialize_the_same_explicit_design() {
    let design = decode(json!({
        "tokens": {
            "color.ink": {"type":"color","value":"#123456"},
            "paint.node": {"type":"paint","value":{"kind":"solid","color":{"ref":"color.ink"}}}
        },
        "styles": {"node":{"fill":{"ref":"paint.node"}}},
        "canvas": {"base":"#ffffff"}
    }));
    let before = design.clone();
    let resources = ThemeResourcePolicy::interactive();
    let typed = materialize_theme_design(&design).expect("materialize typed design");
    let raw = design
        .canonical_json_bytes()
        .expect("canonical design JSON");
    assert_eq!(
        materialize_theme_design_with_resource_policy(&design, &resources).unwrap(),
        typed
    );
    assert_eq!(
        materialize_theme_design_json_with_resource_policy(&raw, &resources).unwrap(),
        typed
    );
    assert_eq!(
        design, before,
        "materialization must preserve editable bindings"
    );
}

#[test]
fn materialization_does_not_eagerly_compile_semantic_targets() {
    let design = decode(json!({"styles":{"future-node":{"fill":"#abcdef"}}}));
    materialize_theme_design(&design).expect("materialization resolves values");
    assert!(
        DiagramThemeCompiler::new()
            .compile_recipe(ThemeRecipeV1::Design { design })
            .is_err()
    );
}

#[test]
fn unresolved_reference_errors_identify_the_original_consumer() {
    let design = decode(json!({"styles":{"node":{"fill":{"ref":"missing.paint"}}}}));
    let typed = materialize_theme_design(&design).expect_err("unknown reference");
    let raw = serde_json::to_vec(&design).unwrap();
    let decoded = materialize_theme_design_json_with_resource_policy(
        &raw,
        &ThemeResourcePolicy::interactive(),
    )
    .unwrap_err();
    assert_eq!(typed, decoded);
    assert_eq!(typed.diagnostic().path(), "/styles/node/fill");
}

#[test]
fn raw_design_input_rejects_duplicate_keys_before_map_normalization() {
    let raw = br##"{"tokens":{"ink":{"type":"color","value":"#000","value":"#fff"}}}"##;
    let error = materialize_theme_design_json_with_resource_policy(
        raw,
        &ThemeResourcePolicy::interactive(),
    )
    .unwrap_err();
    assert_eq!(error.diagnostic().reason_id(), Some("duplicate-object-key"));
}

#[test]
fn raw_resource_paths_escape_keys_and_preserve_complete_bounded_ancestors() {
    for (key, expected_path) in [
        ("a/b~c".to_owned(), "/future/a~1b~0c/0".to_owned()),
        ("界".repeat(160), format!("/future/{}/0", "界".repeat(160))),
        ("x".repeat(502), format!("/future/{}/0", "x".repeat(502))),
        ("x".repeat(503), format!("/future/{}", "x".repeat(503))),
        ("x".repeat(505), "/future".to_owned()),
    ] {
        let input = json!({"future":{key:["x".repeat(65_537)]}});
        let error = materialize_theme_design_json_with_resource_policy(
            &serde_json::to_vec(&input).unwrap(),
            &ThemeResourcePolicy::interactive(),
        )
        .unwrap_err();
        assert_eq!(error.diagnostic().path(), expected_path);
        assert!(input.pointer(error.diagnostic().path()).is_some());
        assert_eq!(
            error.diagnostic().limit_id(),
            Some("max_theme_definition_string_bytes")
        );
    }
}

#[test]
fn caller_owned_encoded_byte_limits_apply_before_resolution_for_both_entry_points() {
    let design = decode(json!({"styles":{"node":{"fill":"#abcdef"}}}));
    let resources = ThemeResourcePolicy::default()
        .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
        .unwrap();
    let typed = materialize_theme_design_with_resource_policy(&design, &resources).unwrap_err();
    let raw = design.canonical_json_bytes().unwrap();
    let decoded = materialize_theme_design_json_with_resource_policy(&raw, &resources).unwrap_err();
    assert_eq!(
        typed.diagnostic().code(),
        "theme-authoring.resource-limit-exceeded"
    );
    assert_eq!(
        typed.diagnostic().limit_id(),
        Some("max_theme_encoded_bytes")
    );
    assert_eq!(typed.diagnostic().actual(), Some(2));
    assert_eq!(typed.diagnostic().max(), Some(1));
    assert_eq!(typed, decoded);
}

#[test]
fn current_token_library_limit_is_shared_by_typed_and_json_inputs() {
    let tokens = (0..256)
        .map(|index| {
            (
                format!("color.{index}"),
                json!({"type":"color","value":"#123456"}),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let mut input = json!({"tokens":tokens});
    let design = decode(input.clone());
    materialize_theme_design(&design).expect("256 reusable values");
    materialize_theme_design_json_with_resource_policy(
        &serde_json::to_vec(&input).unwrap(),
        &ThemeResourcePolicy::interactive(),
    )
    .expect("raw design accepts 256 reusable values");
    input["tokens"]["color.excess"] = json!({"type":"color","value":"#ffffff"});
    let design = decode(input.clone());
    let typed = materialize_theme_design(&design).unwrap_err();
    let raw = materialize_theme_design_json_with_resource_policy(
        &serde_json::to_vec(&input).unwrap(),
        &ThemeResourcePolicy::interactive(),
    )
    .unwrap_err();
    assert_eq!(typed, raw);
    assert_eq!(typed.diagnostic().path(), "/tokens");
}

#[test]
fn malformed_or_obsolete_compact_shapes_fail_closed() {
    for raw in [
        r##"{"tokens":"#fff"}"##,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{}}"#,
        r#"{"styles":[{"kind":"rule","target":"node","style":{}}]}"#,
        r##"{"tokens":{"ink":{"type":"color","value":"#000"}}} trailing"##,
    ] {
        assert!(
            materialize_theme_design_json_with_resource_policy(
                raw.as_bytes(),
                &ThemeResourcePolicy::interactive()
            )
            .is_err(),
            "{raw}"
        );
    }
}

#[test]
fn shared_value_amplification_is_stopped_before_later_consumers_are_resolved() {
    let color = format!("{}red", " ".repeat(1024));
    let stops = (0..64)
        .map(|index| json!({"offset":index as f32 / 63.0,"color":{"ref":"ink"}}))
        .collect::<Vec<_>>();
    let mut rules = (0..100)
        .map(|_| json!({"target":"node","style":{"fill":{"ref":"gradient"}}}))
        .collect::<Vec<_>>();
    // Resolving the whole document before admission would reach this missing value instead.
    rules.push(json!({"target":"node","style":{"fill":{"ref":"missing"}}}));
    let design = decode(json!({"tokens":{
        "ink":{"type":"color","value":color},
        "gradient":{"type":"paint","value":{"kind":"linear-gradient","angle_degrees":0,"stops":stops}}
    },"rules":rules}));
    let policy = ThemeResourcePolicy::interactive()
        .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 256 * 1024)
        .unwrap();
    let error = materialize_theme_design_with_resource_policy(&design, &policy).unwrap_err();
    assert_eq!(
        error.diagnostic().limit_id(),
        Some("max_theme_encoded_bytes")
    );
    assert_eq!(error.diagnostic().actual(), Some(256 * 1024 + 1));
}
