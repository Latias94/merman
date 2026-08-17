use merman_render::diagram_theme::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, Specified, TextStylePatch,
    ThemeCompileError, ThemeCompileValidationError, ThemeResourceLimitExceeded,
    ThemeResourceLimitId, ThemeResourceLimitPhase, ThemeResourcePolicy, ThemeRule, ThemeRuleSet,
    ThemeStrokePatch, ThemeStylePatch, ThemeTarget,
};
use merman_theme_contract::{
    DiagramThemeSpecWireV1, SpecifiedWireV1, ThemeAssetsWireV1, ThemeCanvasPaintWireV1,
    ThemeFontAssetWireV1, ThemeRuleSetWireV1, ThemeStylePatchWireV1,
};

#[test]
fn spec_wire_compiles_to_the_equivalent_typed_recipe() {
    let wire = DiagramThemeSpecWireV1 {
        styles: Some(vec![ThemeRuleSetWireV1::Rule {
            target: "node".to_string(),
            family: None,
            variant: None,
            ordinal: None,
            style: ThemeStylePatchWireV1 {
                fill: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color("#2563eb".to_string())),
                ..Default::default()
            },
        }]),
        ..Default::default()
    };
    let typed = DiagramThemeSpec::new().with_styles(
        ThemeRuleSet::default().with_rule(ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#2563eb").expect("valid test color")),
        )),
    );
    let compiler = DiagramThemeCompiler::new();

    let wire = compiler
        .compile_spec_wire(wire)
        .expect("wire recipe should compile");
    let typed = compiler
        .compile(typed)
        .expect("typed recipe should compile");

    assert_eq!(wire.recipe_fingerprint(), typed.recipe_fingerprint());
}

#[test]
fn nested_style_clear_facets_lower_to_the_equivalent_typed_recipe() {
    let wire = serde_json::from_str::<DiagramThemeSpecWireV1>(
        r#"{"styles":[{"kind":"rule","target":"node","style":{"stroke":{"paint":null},"typography":{"font_size_px":null}}}]}"#,
    )
    .expect("nested style facets should preserve clear semantics");
    let typed =
        DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch {
                stroke: ThemeStrokePatch {
                    paint: Specified::Clear,
                    ..Default::default()
                },
                typography: TextStylePatch {
                    font_size_px: Specified::Clear,
                    ..Default::default()
                },
                ..Default::default()
            },
        )));
    let compiler = DiagramThemeCompiler::new();

    let wire = compiler
        .compile_spec_wire(wire)
        .expect("nested clear facets should compile");
    let typed = compiler
        .compile(typed)
        .expect("typed clear facets should compile");

    assert_eq!(wire.recipe_fingerprint(), typed.recipe_fingerprint());
}

#[test]
fn unknown_semantic_id_fails_closed() {
    let spec = DiagramThemeSpecWireV1 {
        styles: Some(vec![ThemeRuleSetWireV1::Rule {
            target: "future-node".to_string(),
            family: None,
            variant: None,
            ordinal: None,
            style: ThemeStylePatchWireV1::default(),
        }]),
        ..Default::default()
    };

    assert!(matches!(
        DiagramThemeCompiler::new().compile_spec_wire(spec),
        Err(ThemeCompileError::Validation(
            ThemeCompileValidationError::UnknownId {
                field: "styles.rule.target"
            }
        ))
    ));
}

#[test]
fn base64_resource_limit_precedes_invalid_base64() {
    let policy = ThemeResourcePolicy::interactive()
        .with_limit(ThemeResourceLimitId::MaxThemeBase64Bytes, 3)
        .expect("valid test ceiling");

    assert!(matches!(
        DiagramThemeCompiler::new()
            .with_resource_policy(policy)
            .compile_spec_wire(font_spec("font", "woff2", "!!!!")),
        Err(ThemeCompileError::ResourceLimit(
            ThemeResourceLimitExceeded {
                phase: ThemeResourceLimitPhase::ThemeInput,
                limit: "max_theme_base64_bytes",
                actual: 4,
                max: 3,
                ..
            }
        ))
    ));
}

#[test]
fn declared_font_container_must_match_the_payload_before_font_compilation() {
    assert!(matches!(
        DiagramThemeCompiler::new().compile_spec_wire(font_spec(
            "excalifont",
            "truetype",
            "d09GMg==",
        )),
        Err(ThemeCompileError::Validation(
            ThemeCompileValidationError::InvalidValue {
                field: "assets.fonts.format"
            }
        ))
    ));
}

fn font_spec(id: &str, format: &str, data_base64: &str) -> DiagramThemeSpecWireV1 {
    DiagramThemeSpecWireV1 {
        assets: Some(ThemeAssetsWireV1 {
            fonts: Some(vec![ThemeFontAssetWireV1 {
                id: id.to_string(),
                format: format.to_string(),
                data_base64: data_base64.to_string(),
            }]),
            ..Default::default()
        }),
        ..Default::default()
    }
}
