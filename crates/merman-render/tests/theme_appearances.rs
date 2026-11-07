use merman_render::diagram_theme::{
    DiagramThemeCompiler, ThemeAppearance, ThemeDesignCompileError, ThemeRecipeV1,
    ThemeResourceLimitId, ThemeResourcePolicy, materialize_theme_design,
};
use serde_json::{Value, json};

fn reading_recipe() -> Value {
    json!({
        "schema_version":1,"kind":"design",
        "design":{
            "tokens":{"accent":{"type":"paint","value":"#3567aa"}},
            "styles":{"node":{"fill":{"ref":"surface"}},"node-label":{"fill":{"ref":"ink"}}},
            "rules":[{"target":"node","style":{"stroke":{"paint":{"ref":"accent"}}}}],
            "canvas":{"base":{"ref":"surface"}},
            "appearances":{
            "light":{"tokens":{
                "surface":{"type":"paint","value":"#fafbfc"},
                "ink":{"type":"paint","value":"#20242c"}
            }},
            "dark":{"tokens":{
                "surface":{"type":"paint","value":"#20242c"},
                "ink":{"type":"paint","value":"#fafbfc"}
            }}
            },
            "default_appearance":"light"
        }
    })
}

fn decode(input: Value) -> ThemeRecipeV1 {
    serde_json::from_value(input).unwrap()
}

#[test]
fn complete_pair_inspection_admits_common_references_supplied_by_each_appearance() {
    let compiler = DiagramThemeCompiler::new();
    let recipe = decode(reading_recipe());
    let inspected = compiler.inspect_document(&recipe).unwrap();
    assert_eq!(inspected.schema_version(), 1);
    assert_eq!(inspected.supported_appearances, ThemeAppearance::ALL);
    assert_eq!(inspected.default_appearance, Some(ThemeAppearance::Light));
    for entry in &inspected.appearances {
        let appearance = entry.appearance.unwrap();
        let surface = entry
            .references
            .iter()
            .find(|entry| entry.token == "surface" && entry.path == "/design/canvas/base")
            .unwrap();
        assert_eq!(surface.path, "/design/canvas/base");
        assert_eq!(
            surface.token_path,
            format!("/design/appearances/{appearance}/tokens/surface")
        );
    }
    for appearance in ThemeAppearance::ALL {
        let materialized = compiler
            .materialize_document(&recipe, Some(appearance))
            .unwrap();
        assert_eq!(materialized.effective_appearance, Some(appearance));
        assert_eq!(
            compiler
                .compile_document(recipe.clone(), Some(appearance))
                .unwrap()
                .recipe_fingerprint(),
            compiler
                .compile_spec_wire(materialized.spec)
                .unwrap()
                .recipe_fingerprint()
        );
    }
    assert_eq!(
        compiler
            .compile_document(recipe.clone(), None)
            .unwrap()
            .recipe_fingerprint(),
        compiler
            .compile_document(recipe, Some(ThemeAppearance::Light))
            .unwrap()
            .recipe_fingerprint()
    );
}

#[test]
fn concrete_document_reference_locations_resolve_against_the_complete_recipe() {
    let input = json!({
        "schema_version":1,"kind":"design","design":{
            "tokens":{"ink":{"type":"paint","value":"#123456"}},
            "styles":{"node":{"fill":{"ref":"ink"}}}
        }
    });
    let compiler = DiagramThemeCompiler::new();
    let recipe = decode(input.clone());
    let inspected = compiler.inspect_document(&recipe).unwrap();
    let materialized = compiler.materialize_document(&recipe, None).unwrap();
    assert_eq!(inspected.appearances[0].references, materialized.references);
    let reference = &materialized.references[0];
    assert_eq!(reference.path, "/design/styles/node/fill");
    assert_eq!(reference.token_path, "/design/tokens/ink");
    assert_eq!(input.pointer(&reference.path), Some(&json!({"ref":"ink"})));
    assert_eq!(
        input.pointer(&reference.token_path),
        Some(&json!({"type":"paint","value":"#123456"}))
    );
}

#[test]
fn an_invalid_inactive_appearance_rejects_inspect_materialize_and_compile() {
    let compiler = DiagramThemeCompiler::new();
    let mut input = reading_recipe();
    input["design"]["appearances"]["dark"]["tokens"]["surface"]["value"] = json!({"ref":"missing"});
    let recipe = decode(input);
    let results = [
        compiler.inspect_document(&recipe).map(|_| ()),
        compiler
            .materialize_document(&recipe, Some(ThemeAppearance::Light))
            .map(|_| ()),
        compiler
            .compile_document(recipe.clone(), Some(ThemeAppearance::Light))
            .map(|_| ()),
    ];
    for result in results {
        let ThemeDesignCompileError::Materialization(error) = result.unwrap_err() else {
            panic!("a reference error retains its authoring diagnostic")
        };
        assert_eq!(
            error.diagnostic().path(),
            "/design/appearances/dark/tokens/surface/value"
        );
    }
}

#[test]
fn local_semantic_styles_follow_shared_rules_and_local_rules_follow_local_styles() {
    let compiler = DiagramThemeCompiler::new();
    let mut input = reading_recipe();
    input["design"]["rules"] = json!([{"target":"node","style":{"fill":"#ee3300","radius":3}}]);
    input["design"]["appearances"]["dark"]["styles"] = json!({"node":{"fill":"#3355ee"}});
    input["design"]["appearances"]["dark"]["rules"] =
        json!([{"target":"node","style":{"radius":7}}]);
    let actual = compiler
        .materialize_document(&decode(input), Some(ThemeAppearance::Dark))
        .unwrap();
    let expected = materialize_theme_design(
        &serde_json::from_value(json!({
            "tokens":{
                "accent":{"type":"paint","value":"#3567aa"},
                "surface":{"type":"paint","value":"#20242c"},
                "ink":{"type":"paint","value":"#fafbfc"}
            },
            "styles":{"node":{"fill":{"ref":"surface"}},"node-label":{"fill":{"ref":"ink"}}},
            "rules":[
                {"target":"node","style":{"fill":"#ee3300","radius":3}},
                {"target":"node","style":{"fill":"#3355ee"}},
                {"target":"node","style":{"radius":7}}
            ],
            "canvas":{"base":{"ref":"surface"}}
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual.spec, expected);
}

#[test]
fn local_style_and_token_diagnostics_point_into_the_editable_appearance() {
    let compiler = DiagramThemeCompiler::new();
    let mut input = reading_recipe();
    input["design"]["appearances"]["dark"]["styles"] = json!({"node":{"fill":{"ref":"unknown"}}});
    let error = compiler.inspect_document(&decode(input)).unwrap_err();
    let ThemeDesignCompileError::Materialization(error) = error else {
        panic!("local style reference error")
    };
    assert_eq!(
        error.diagnostic().path(),
        "/design/appearances/dark/styles/node/fill"
    );
}

#[test]
fn compiler_diagnostics_retain_shared_and_local_field_ownership() {
    let mut shared_style = reading_recipe();
    shared_style["design"]["styles"]["unknown-target"] = json!({});
    let mut shared_mermaid = reading_recipe();
    shared_mermaid["design"]["mermaid"] = json!({"theme":"unknown"});
    shared_mermaid["design"]["appearances"]["light"]["mermaid"] = json!({"dark_mode":false});
    let mut shared_variable = reading_recipe();
    shared_variable["design"]["mermaid"] = json!({"variables":{"darkMode":"invalid"}});
    shared_variable["design"]["appearances"]["light"]["mermaid"] = json!({"theme":"base"});
    let mut local_mermaid = reading_recipe();
    local_mermaid["design"]["appearances"]["dark"]["mermaid"] = json!({"theme":"unknown"});
    let mut local_rule = reading_recipe();
    local_rule["design"]["appearances"]["dark"]["rules"] = json!([{
        "target":"actor","family":"flowchart","style":{"fill":"red"}
    }]);
    for (input, path) in [
        (shared_style, "/design/styles/unknown-target"),
        (shared_mermaid, "/design/mermaid/theme"),
        (shared_variable, "/design/mermaid/variables/darkMode"),
        (local_mermaid, "/design/appearances/dark/mermaid/theme"),
        (local_rule, "/design/appearances/dark/rules/0/style"),
    ] {
        let recipe = decode(input.clone());
        let ThemeDesignCompileError::Materialization(error) = DiagramThemeCompiler::new()
            .inspect_document(&recipe)
            .unwrap_err()
        else {
            panic!("compiler errors retain authoring provenance")
        };
        assert_eq!(error.diagnostic().path(), path);
        assert!(input.pointer(path).is_some());
    }
}

#[test]
fn single_appearance_and_unclassified_recipes_have_explicit_selection_contracts() {
    let compiler = DiagramThemeCompiler::new();
    let mut input = reading_recipe();
    input["design"]["appearances"]
        .as_object_mut()
        .unwrap()
        .remove("dark");
    let single = decode(input);
    assert!(compiler.compile_document(single.clone(), None).is_ok());
    assert!(
        compiler
            .compile_document(single, Some(ThemeAppearance::Dark))
            .is_err()
    );
    let concrete = decode(json!({"schema_version":1,"kind":"design","design":{}}));
    assert!(compiler.compile_document(concrete.clone(), None).is_ok());
    assert!(
        compiler
            .compile_document(concrete, Some(ThemeAppearance::Light))
            .is_err()
    );
    for input in [
        json!({"schema_version":1,"kind":"design","design":{"default_appearance":"light"}}),
        json!({"schema_version":1,"kind":"design","design":{"appearances":{},"default_appearance":"light"}}),
        json!({"schema_version":1,"kind":"design","design":{"appearances":{"light":{}}}}),
        json!({"schema_version":1,"kind":"design","design":{"appearances":{"light":{}},"default_appearance":"dark"}}),
    ] {
        assert!(compiler.inspect_document(&decode(input)).is_err());
    }
}

#[test]
fn inactive_expansion_and_full_document_bytes_are_global_admission_gates() {
    let mut input = reading_recipe();
    input["design"]["appearances"]["dark"]["tokens"]["large"] = json!({
        "type":"paint","value":{
            "kind":"linear-gradient","angle_degrees":90,
            "stops":[{"offset":0,"color":"#112233"},{"offset":1,"color":"#ddeeff"}]
        }
    });
    let mut rules = Vec::new();
    for radius in 0..30 {
        rules.push(json!({"target":"node","style":{"fill":{"ref":"large"},"radius":radius}}));
    }
    input["design"]["appearances"]["dark"]["rules"] = json!(rules);
    let recipe = decode(input);
    let size = serde_json::to_vec(&recipe).unwrap().len();
    let compiler = DiagramThemeCompiler::new().with_resource_policy(
        ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, size)
            .unwrap(),
    );
    assert!(
        compiler
            .materialize_document(&recipe, Some(ThemeAppearance::Light))
            .is_err()
    );
    let too_small = DiagramThemeCompiler::new().with_resource_policy(
        ThemeResourcePolicy::interactive()
            .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, size - 1)
            .unwrap(),
    );
    assert!(too_small.inspect_document(&recipe).is_err());
}
