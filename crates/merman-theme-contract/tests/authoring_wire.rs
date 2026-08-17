use merman_theme_contract::{
    CanonicalJsonErrorKind, SpecifiedWireV1, ThemeAuthoringTypographyV1,
    ThemeCanvasPaintObjectWireV1, ThemeCanvasPaintWireV1, ThemeColorTokenV1, ThemeDefinitionV1,
    ThemeGradientStopWireV1, ThemeOrdinalCycleWireV1, ThemeOrdinalSelectorWireV1,
    ThemeRuleSetWireV1, ThemeStylePatchWireV1, ThemeTokensV1,
};

#[test]
fn minimal_definition_decodes_and_rust_construction_selects_version_one() {
    let decoded: ThemeDefinitionV1 =
        serde_json::from_str(r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{}}"#)
            .expect("the minimal persisted definition should decode");
    assert_eq!(decoded.authoring_schema_version(), 1);
    assert_eq!(decoded.expansion_version(), 1);
    assert!(decoded.styles().is_empty());

    let constructed = ThemeDefinitionV1::new(ThemeTokensV1::default());
    assert_eq!(constructed.authoring_schema_version(), 1);
    assert_eq!(constructed.expansion_version(), 1);
}

#[test]
fn persisted_definitions_require_the_complete_supported_version_tuple() {
    for json in [
        r#"{"expansion_version":1,"tokens":{}}"#,
        r#"{"authoring_schema_version":1,"tokens":{}}"#,
        r#"{"authoring_schema_version":2,"expansion_version":1,"tokens":{}}"#,
        r#"{"authoring_schema_version":1,"expansion_version":2,"tokens":{}}"#,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{},"extra":true}"#,
    ] {
        assert!(
            serde_json::from_str::<ThemeDefinitionV1>(json).is_err(),
            "{json}"
        );
    }

    let encoded = serde_json::to_value(ThemeDefinitionV1::new(ThemeTokensV1::default()))
        .expect("a valid definition should serialize");
    assert_eq!(encoded["authoring_schema_version"], 1);
    assert_eq!(encoded["expansion_version"], 1);
    assert_eq!(encoded["tokens"], serde_json::json!({}));
}

#[test]
fn top_level_and_token_nulls_are_rejected() {
    for json in [
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":null}"#,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"canvas":null}}"#,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"series":null}}"#,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"typography":null}}"#,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"typography":{"font_stack":null}}}"#,
    ] {
        assert!(
            serde_json::from_str::<ThemeDefinitionV1>(json).is_err(),
            "{json}"
        );
    }
}

#[test]
fn token_omission_and_explicit_empty_series_survive_round_trip() {
    let omitted: ThemeDefinitionV1 = serde_json::from_str(
        r##"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"text":"#123456","typography":{}}}"##,
    )
    .expect("optional tokens should decode");
    assert_eq!(
        omitted.tokens().color(ThemeColorTokenV1::Text),
        Some("#123456")
    );
    assert_eq!(omitted.tokens().color(ThemeColorTokenV1::Canvas), None);
    assert!(omitted.tokens().typography().is_some());
    assert_eq!(omitted.tokens().series(), None);

    let empty_series: ThemeDefinitionV1 = serde_json::from_str(
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"series":[]}}"#,
    )
    .expect("wire decoding leaves empty-series validation to materialization");
    assert_eq!(empty_series.tokens().series(), Some(&[] as &[String]));
    assert_eq!(
        serde_json::to_value(empty_series).expect("the typed wire should serialize")["tokens"]["series"],
        serde_json::json!([])
    );
}

#[test]
fn styles_omission_and_empty_array_decode_as_empty_but_null_is_rejected() {
    for json in [
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{}}"#,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{},"styles":[]}"#,
    ] {
        let definition: ThemeDefinitionV1 =
            serde_json::from_str(json).expect("styles are optional");
        assert!(definition.styles().is_empty());
    }

    assert!(
        serde_json::from_str::<ThemeDefinitionV1>(
            r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{},"styles":null}"#,
        )
        .is_err()
    );
}

#[test]
fn removed_family_specific_tokens_and_nested_unknown_fields_are_rejected() {
    for json in [
        r##"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"actor_background":"#fff"}}"##,
        r##"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"note_text":"#111"}}"##,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"typography":{"font_style":"italic"}}}"#,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{},"styles":[{"kind":"rule","target":"node","style":{"unknown":1}}]}"#,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"typography":{"line_height":{"px":20,"extra":1}}}}"#,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{},"styles":[{"kind":"rule","target":"node","family":null,"style":{}}]}"#,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{},"styles":[{"kind":"rule","target":"node","ordinal":{"exact":1,"extra":1},"style":{}}]}"#,
        r#"{"authoring_schema_version":1,"expansion_version":1,"tokens":{},"styles":[{"kind":"ordinal-palette","target":"node","colors":null}]}"#,
    ] {
        assert!(
            serde_json::from_str::<ThemeDefinitionV1>(json).is_err(),
            "{json}"
        );
    }
}

#[test]
fn style_facets_preserve_unspecified_clear_and_value_states() {
    let json = r##"{
        "authoring_schema_version": 1,
        "expansion_version": 1,
        "tokens": {},
        "styles": [{
            "kind": "rule",
            "target": "node",
            "style": {
                "opacity": null,
                "fill": "#abcdef",
                "stroke": {"paint": null, "width": 2.5},
                "effect": "drop-shadow"
            }
        }]
    }"##;
    let definition: ThemeDefinitionV1 = serde_json::from_str(json).expect("rule should decode");
    let ThemeRuleSetWireV1::Rule { style, .. } = &definition.styles()[0] else {
        panic!("expected a rule entry");
    };
    assert!(matches!(style.radius, SpecifiedWireV1::Unspecified));
    assert!(matches!(style.opacity, SpecifiedWireV1::Clear));
    assert!(matches!(style.fill, SpecifiedWireV1::Value(_)));
    assert!(matches!(style.effect, SpecifiedWireV1::Value(ref id) if id == "drop-shadow"));

    let encoded = serde_json::to_value(definition).expect("rule should re-encode");
    let style = &encoded["styles"][0]["style"];
    assert_eq!(style["opacity"], serde_json::Value::Null);
    assert_eq!(style["fill"], "#abcdef");
    assert_eq!(style["stroke"]["paint"], serde_json::Value::Null);
    assert_eq!(style["stroke"]["width"], 2.5);
    assert_eq!(style["effect"], "drop-shadow");
    assert!(style.get("radius").is_none());
}

#[test]
fn complete_rule_support_wire_round_trips_without_changing_shape() {
    let value = serde_json::json!({
        "authoring_schema_version": 1,
        "expansion_version": 1,
        "tokens": {
            "typography": {
                "font_stack": ["Inter", "sans-serif"],
                "font_size_px": 16,
                "font_weight": 450,
                "line_height": {"px": 24}
            }
        },
        "styles": [
            {
                "kind": "rule",
                "target": "node",
                "family": "flowchart-v2",
                "variant": "primary",
                "ordinal": {"cycle": {"period": 4, "offset": 1}},
                "style": {
                    "fill": {
                        "kind": "linear-gradient",
                        "angle_degrees": 45,
                        "stops": [
                            {"offset": 0, "color": "#000000"},
                            {"offset": 1, "color": "#ffffff"}
                        ],
                        "repetition": {"kind": "repeating", "period_px": 20}
                    },
                    "fill_opacity": 0.75,
                    "stroke": {
                        "paint": {"kind": "solid", "color": "#123456"},
                        "width": 2,
                        "dasharray": [3, 2],
                        "linecap": "round",
                        "linejoin": "bevel",
                        "opacity": 0.5
                    },
                    "radius": 6,
                    "padding": {"top": 1, "right": 2, "bottom": 3, "left": 4},
                    "typography": {
                        "font_stack": ["Mono"],
                        "font_size_px": 14,
                        "font_weight": 600,
                        "font_style": "italic",
                        "line_height": "normal",
                        "letter_spacing_px": 0.5,
                        "word_spacing_px": 1,
                        "transform": "uppercase",
                        "decoration": "underline",
                        "text_align": "center",
                        "white_space": "normal",
                        "wrap": "word"
                    }
                }
            },
            {"kind": "ordinal-palette", "target": "node", "colors": ["#f00", "#0f0"]}
        ]
    });

    let definition: ThemeDefinitionV1 =
        serde_json::from_value(value.clone()).expect("the complete support wire should decode");
    let canonical = definition
        .canonical_json_bytes()
        .expect("the complete support wire should canonicalize");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&canonical)
            .expect("canonical JSON should decode for comparison"),
        value
    );
}

#[test]
fn ordinal_wire_uses_one_based_fixed_width_values() {
    let definition = ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![
        ThemeRuleSetWireV1::Rule {
            target: "node".to_owned(),
            family: None,
            variant: None,
            ordinal: Some(ThemeOrdinalSelectorWireV1::Exact { exact: 1 }),
            style: ThemeStylePatchWireV1::default(),
        },
        ThemeRuleSetWireV1::Rule {
            target: "node".to_owned(),
            family: None,
            variant: None,
            ordinal: Some(ThemeOrdinalSelectorWireV1::Cycle {
                cycle: ThemeOrdinalCycleWireV1 {
                    period: u32::MAX,
                    offset: u32::MAX - 1,
                },
            }),
            style: ThemeStylePatchWireV1::default(),
        },
    ]);

    let bytes = definition
        .canonical_json_bytes()
        .expect("fixed-width ordinals should canonicalize exactly");
    let decoded: ThemeDefinitionV1 =
        serde_json::from_slice(&bytes).expect("canonical ordinal bytes should replay");
    assert_eq!(decoded, definition);
}

#[test]
fn typed_canonical_json_orders_keys_and_normalizes_negative_zero() {
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_color(ThemeColorTokenV1::Text, "#123456")
            .with_typography(ThemeAuthoringTypographyV1::default().with_font_size_px(-0.0)),
    );

    let json = String::from_utf8(
        definition
            .canonical_json_bytes()
            .expect("finite typed wire should canonicalize"),
    )
    .expect("canonical JSON is UTF-8");
    assert_eq!(
        json,
        r##"{"authoring_schema_version":1,"expansion_version":1,"tokens":{"text":"#123456","typography":{"font_size_px":0}}}"##
    );
}

#[test]
fn typed_canonical_json_rejects_rust_constructed_non_finite_numbers() {
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_typography(ThemeAuthoringTypographyV1::default().with_font_size_px(f32::NAN)),
    );
    assert_eq!(
        definition
            .canonical_json_bytes()
            .expect_err("non-finite token typography must fail")
            .kind(),
        CanonicalJsonErrorKind::InvalidInput
    );
    assert!(serde_json::to_vec(&definition).is_err());

    let style = ThemeStylePatchWireV1 {
        opacity: SpecifiedWireV1::Value(f32::INFINITY),
        ..ThemeStylePatchWireV1::default()
    };
    let definition = ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![
        ThemeRuleSetWireV1::Rule {
            target: "node".to_owned(),
            family: None,
            variant: None,
            ordinal: None,
            style,
        },
    ]);
    assert_eq!(
        definition
            .canonical_json_bytes()
            .expect_err("non-finite style facets must fail")
            .kind(),
        CanonicalJsonErrorKind::InvalidInput
    );

    let style = ThemeStylePatchWireV1 {
        fill: SpecifiedWireV1::Value(ThemeCanvasPaintWireV1::Structured(
            ThemeCanvasPaintObjectWireV1::LinearGradient {
                angle_degrees: 45.0,
                stops: vec![ThemeGradientStopWireV1 {
                    offset: f32::NAN,
                    color: "#000000".to_owned(),
                }],
                repetition: None,
            },
        )),
        ..ThemeStylePatchWireV1::default()
    };
    let definition = ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![
        ThemeRuleSetWireV1::Rule {
            target: "node".to_owned(),
            family: None,
            variant: None,
            ordinal: None,
            style,
        },
    ]);
    assert_eq!(
        definition
            .canonical_json_bytes()
            .expect_err("non-finite nested gradient values must fail")
            .kind(),
        CanonicalJsonErrorKind::InvalidInput
    );
    assert!(serde_json::to_vec(&definition).is_err());
}

#[test]
fn named_token_setters_are_the_same_shareable_wire_value_as_generic_construction() {
    let named = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_canvas("#0f172a")
            .with_surface("#111827")
            .with_surface_alt("#1f2937")
            .with_surface_muted("#334155")
            .with_text("#e5e7eb")
            .with_subtle_text("#cbd5e1")
            .with_border("#475569")
            .with_line("#94a3b8")
            .with_accent("#60a5fa")
            .with_error("#ef4444")
            .with_warning("#f59e0b")
            .with_success("#34d399"),
    );
    let generic = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_color(ThemeColorTokenV1::Canvas, "#0f172a")
            .with_color(ThemeColorTokenV1::Surface, "#111827")
            .with_color(ThemeColorTokenV1::SurfaceAlt, "#1f2937")
            .with_color(ThemeColorTokenV1::SurfaceMuted, "#334155")
            .with_color(ThemeColorTokenV1::Text, "#e5e7eb")
            .with_color(ThemeColorTokenV1::SubtleText, "#cbd5e1")
            .with_color(ThemeColorTokenV1::Border, "#475569")
            .with_color(ThemeColorTokenV1::Line, "#94a3b8")
            .with_color(ThemeColorTokenV1::Accent, "#60a5fa")
            .with_color(ThemeColorTokenV1::Error, "#ef4444")
            .with_color(ThemeColorTokenV1::Warning, "#f59e0b")
            .with_color(ThemeColorTokenV1::Success, "#34d399"),
    );

    assert_eq!(
        named
            .canonical_json_bytes()
            .expect("named token construction should canonicalize"),
        generic
            .canonical_json_bytes()
            .expect("generic token construction should canonicalize")
    );
}
