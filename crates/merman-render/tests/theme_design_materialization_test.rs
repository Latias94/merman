use merman_render::diagram_theme::{inspect_theme_design, materialize_theme_design};
use merman_theme_contract::{
    SpecifiedWireV1, ThemeCanvasPaintObjectWireV1, ThemeCanvasPaintWireV1, ThemeDesignV1,
    ThemeRuleSetWireV1,
};

fn design(json: &str) -> ThemeDesignV1 {
    serde_json::from_str(json).unwrap()
}

#[test]
fn shared_color_reaches_body_label_gradient_and_shadow_but_not_equal_literals() {
    let mut document = design(
        r##"{
      "tokens": {
        "ink": {"type":"color","value":"#112233"},
        "alias": {"type":"color","value":{"ref":"ink"}},
        "raised": {"type":"effect","value":{"kind":"shadow-stack","composition":"independent","shadows":[
          {"offset_x":6,"offset_y":6,"std_deviation":0,"color":{"ref":"alias"},"opacity":0.5}
        ]}}
      },
      "styles": {
        "node": {"fill":"#112233","stroke":{"paint":{"kind":"solid","color":{"ref":"ink"}}},"effect":{"ref":"raised"}},
        "node-label":{"fill":{"kind":"solid","color":{"ref":"alias"}}}
      },
      "canvas":{"base":{"kind":"linear-gradient","angle_degrees":45,"stops":[
        {"offset":0,"color":{"ref":"ink"}},{"offset":1,"color":"#ffffff"}
      ]}}
    }"##,
    );
    let original = materialize_theme_design(&document).unwrap();
    document.tokens.get_mut("ink").unwrap().value =
        merman_theme_contract::ThemeDesignTokenValueV1::Color(
            merman_theme_contract::ThemeValueV1::Literal("#abcdef".to_owned()),
        );
    let edited = materialize_theme_design(&document).unwrap();
    let ThemeRuleSetWireV1::Rule { style, .. } = &edited.styles.as_ref().unwrap()[0] else {
        panic!()
    };
    assert_eq!(
        style.fill,
        SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color("#112233".to_owned()))
    );
    assert!(serde_json::to_string(&edited).unwrap().contains("#abcdef"));
    assert!(
        serde_json::to_string(&original)
            .unwrap()
            .contains("#112233")
    );
    let Some(ThemeCanvasPaintWireV1::Structured(ThemeCanvasPaintObjectWireV1::LinearGradient {
        stops,
        ..
    })) = edited
        .canvas
        .as_ref()
        .and_then(|canvas| canvas.base.as_ref())
    else {
        panic!()
    };
    assert_eq!(stops[0].color, "#abcdef");
    let inspection = inspect_theme_design(&document).unwrap();
    assert!(
        serde_json::to_string(&inspection)
            .unwrap()
            .contains("/styles/node/stroke/paint/color")
    );
}

#[test]
fn missing_cycles_wrong_types_and_illegal_units_have_authoring_paths() {
    for (json, expected_path) in [
        (
            r#"{"styles":{"node":{"radius":{"ref":"missing"}}}}"#,
            "/styles/node/radius",
        ),
        (
            r#"{"tokens":{"a":{"type":"length","value":{"ref":"b"}},"b":{"type":"length","value":{"ref":"a"}}}}"#,
            "/tokens/",
        ),
        (
            r#"{"tokens":{"n":{"type":"number","value":2}},"styles":{"node":{"radius":{"ref":"n"}}}}"#,
            "/styles/node/radius",
        ),
        (
            r#"{"styles":{"node":{"stroke":{"width":{"percent":20}}}}}"#,
            "/styles/node/stroke/width",
        ),
    ] {
        let error = materialize_theme_design(&design(json)).unwrap_err();
        assert!(error.to_string().contains(expected_path), "{error}");
    }
}

#[test]
fn direct_values_need_no_tokens_and_exceptions_preserve_authored_order() {
    let document = design(
        r##"{
      "styles":{"node":{"fill":"#ffffff"}},
      "rules":[
        {"target":"node","family":"flowchart","style":{"radius":3}},
        {"target":"node","family":"flowchart","style":{"radius":null}}
      ]
    }"##,
    );
    let spec = materialize_theme_design(&document).unwrap();
    assert_eq!(spec.styles.as_ref().unwrap().len(), 3);
    let ThemeRuleSetWireV1::Rule { style, .. } = &spec.styles.as_ref().unwrap()[2] else {
        panic!()
    };
    assert_eq!(style.radius, SpecifiedWireV1::Clear);
}

#[test]
fn inspection_exposes_consumer_choices_for_shared_strings() {
    let document = design(
        r#"{
      "tokens":{"cap":{"type":"string","value":"round"}},
      "styles":{"edge":{"stroke":{"linecap":{"ref":"cap"}}}}
    }"#,
    );
    let inspection = serde_json::to_value(inspect_theme_design(&document).unwrap()).unwrap();
    let json = inspection.to_string();
    assert!(json.contains("butt") && json.contains("round") && json.contains("square"));
    assert!(json.contains("/styles/edge/stroke/linecap"));
}

#[test]
fn inspection_propagates_consumer_domains_through_alias_chains() {
    let document = design(
        r#"{
      "tokens": {
        "amount":{"type":"number","value":0.5},
        "alias":{"type":"number","value":{"ref":"amount"}},
        "alias2":{"type":"number","value":{"ref":"alias"}}
      },
      "styles":{"node":{"opacity":{"ref":"alias2"}}}
    }"#,
    );
    let inspection = inspect_theme_design(&document).unwrap();
    let use_of_amount = inspection
        .references
        .iter()
        .find(|entry| entry.token == "amount")
        .unwrap();
    assert_eq!(use_of_amount.domain.min, Some(0.0));
    assert_eq!(use_of_amount.domain.max, Some(1.0));
}

#[test]
fn design_recipe_compiles_through_the_real_compiler_and_metadata_is_not_render_identity() {
    use merman_render::diagram_theme::DiagramThemeCompiler;
    use merman_theme_contract::ThemeRecipeV1;
    let mut document = design(
        r##"{"tokens":{"ink":{"type":"color","value":"#112233","label":"Ink"}},"styles":{"node":{"fill":{"kind":"solid","color":{"ref":"ink"}}}}}"##,
    );
    let compiler = DiagramThemeCompiler::new();
    let first = compiler
        .compile_recipe(ThemeRecipeV1::Design {
            design: document.clone(),
        })
        .unwrap();
    document.tokens.get_mut("ink").unwrap().label = Some("Renamed display label".to_owned());
    let second = compiler
        .compile_recipe(ThemeRecipeV1::Design { design: document })
        .unwrap();
    assert_eq!(first.recipe_fingerprint(), second.recipe_fingerprint());
}
