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

#[test]
fn projected_font_bytes_are_checked_before_full_payload_decode() {
    for (format, payload, limit, expected_phase, expected_id) in [
        (
            "woff2",
            "d09GMgAA",
            ThemeResourceLimitId::MaxFontAssetCompressedBytes,
            ThemeResourceLimitPhase::FontDecode,
            "max_font_asset_compressed_bytes",
        ),
        (
            "truetype",
            "AAEAAAAA",
            ThemeResourceLimitId::MaxFontAssetDecodedBytes,
            ThemeResourceLimitPhase::FontDecode,
            "max_font_asset_decoded_bytes",
        ),
        (
            "truetype",
            "AAEAAAAA",
            ThemeResourceLimitId::MaxFontCatalogDecodedBytes,
            ThemeResourceLimitPhase::FontCatalog,
            "max_font_catalog_decoded_bytes",
        ),
    ] {
        let policy = ThemeResourcePolicy::interactive()
            .with_limit(limit, 5)
            .expect("valid projected-font test ceiling");

        assert!(matches!(
            DiagramThemeCompiler::new()
                .with_resource_policy(policy)
                .compile_spec_wire(font_spec("font", format, payload)),
            Err(ThemeCompileError::ResourceLimit(
                ThemeResourceLimitExceeded {
                    phase,
                    limit,
                    actual: 6,
                    max: 5,
                    ..
                }
            )) if phase == expected_phase && limit == expected_id
        ));
    }
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

#[test]
fn every_core_family_id_round_trips_distinct_rule_and_typography_scopes() {
    use merman_core::DiagramFamilyId;
    use merman_render::diagram_theme::{ThemeTextStyle, TypographySpec};
    use merman_theme_contract::{ThemeTextStyleWireV1, ThemeTypographySpecWireV1};

    let compiler = DiagramThemeCompiler::new();
    let paint = CanvasPaint::solid("#2563eb").unwrap();
    for rule_scope in [true, false] {
        let mut fingerprints = Vec::new();
        for &family in DiagramFamilyId::all() {
            let spec = DiagramThemeSpecWireV1 {
                styles: rule_scope.then(|| {
                    vec![ThemeRuleSetWireV1::Rule {
                        target: "text".to_owned(),
                        family: Some(family.as_str().to_owned()),
                        variant: None,
                        ordinal: None,
                        style: ThemeStylePatchWireV1 {
                            fill: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Color(
                                "#2563eb".to_owned(),
                            )),
                            ..Default::default()
                        },
                    }]
                }),
                typography: (!rule_scope).then(|| ThemeTypographySpecWireV1 {
                    families: Some(
                        [(
                            family.as_str().to_owned(),
                            ThemeTextStyleWireV1 {
                                font_size_px: Some(23.0),
                                ..Default::default()
                            },
                        )]
                        .into_iter()
                        .collect(),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let wire = compiler
                .compile_spec_wire(
                    serde_json::from_slice(&serde_json::to_vec(&spec).unwrap()).unwrap(),
                )
                .unwrap_or_else(|error| panic!("{} wire scope: {error}", family.as_str()));
            let typed_spec = if rule_scope {
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default().with_fill(paint.clone()),
                        )
                        .for_family(family),
                    ),
                )
            } else {
                DiagramThemeSpec::new().with_typography(
                    TypographySpec::default().with_family_style(
                        family,
                        ThemeTextStyle::default().with_font_size_px(23.0).unwrap(),
                    ),
                )
            };
            let typed = compiler.compile(typed_spec).unwrap();
            assert_eq!(
                wire.recipe_fingerprint(),
                typed.recipe_fingerprint(),
                "{} scope",
                family.as_str()
            );
            for fingerprint in &fingerprints {
                assert_ne!(
                    wire.recipe_fingerprint(),
                    *fingerprint,
                    "logical family scopes must stay distinct"
                );
            }
            fingerprints.push(wire.recipe_fingerprint());
        }
    }
}

#[test]
fn parser_aliases_and_unknown_ids_cannot_be_used_as_logical_theme_scopes() {
    use merman_core::{DiagramFamilyId, diagram_family_capabilities};
    for id in diagram_family_capabilities()
        .iter()
        .map(|fact| fact.diagram_type)
        .filter(|id| DiagramFamilyId::from_id(id).is_none())
        .chain(["future-family"])
    {
        for (spec, field) in [
            (
                serde_json::json!({"styles":[{"kind":"rule","target":"text","family":id,"style":{}}]}),
                "styles.rule.family",
            ),
            (
                serde_json::json!({"typography":{"families":{(id):{"font_size_px":23}}}}),
                "typography.families",
            ),
        ] {
            let error = DiagramThemeCompiler::new()
                .compile_spec_wire(serde_json::from_value(spec).unwrap())
                .unwrap_err();
            assert!(
                matches!(error, ThemeCompileError::Validation(ThemeCompileValidationError::UnknownId {field: actual}) if actual == field),
                "{id} must not become a logical scope at {field}: {error:?}"
            );
        }
    }
}
