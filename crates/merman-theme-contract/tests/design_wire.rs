use merman_theme_contract::{
    CanonicalJsonErrorKind, SpecifiedWireV1, ThemeDesignCanvasLayerV1, ThemeDesignCanvasV1,
    ThemeDesignEffectV1, ThemeDesignGradientStopV1, ThemeDesignPaintObjectV1, ThemeDesignPaintV1,
    ThemeDesignShadowV1, ThemeDesignStyleV1, ThemeDesignTokenV1, ThemeDesignTokenValueV1,
    ThemeDesignV1, ThemeInsetsWireV1, ThemeLengthWireV1, ThemeLineHeightWireV1, ThemeReferenceV1,
    ThemeShadowCompositionV1, ThemeValueV1,
};

#[test]
fn current_recipe_envelope_requires_its_explicit_version_and_kind() {
    use merman_theme_contract::ThemeRecipeV1;
    let input =
        json!({"schema_version":1,"kind":"design","design":{"styles":{"node":{"fill":"#ffffff"}}}});
    let recipe: ThemeRecipeV1 = serde_json::from_value(input.clone()).unwrap();
    assert_eq!(serde_json::to_value(recipe).unwrap(), input);
    for invalid in [
        json!({"schema_version":2,"kind":"design","design":{}}),
        json!({"schema_version":1,"kind":"definition","definition":{}}),
        json!({"schema_version":1,"kind":"design","design":{},"complete_spec":{}}),
    ] {
        assert!(serde_json::from_value::<ThemeRecipeV1>(invalid).is_err());
    }
}
use serde_json::json;

#[test]
fn single_element_lists_are_literals_and_references_require_objects() {
    let design: ThemeDesignV1 = serde_json::from_value(json!({"tokens": {
        "colors": {"type":"palette","value":["orange"]},
        "fonts": {"type":"font-stack","value":["sans-serif"]}
    }}))
    .unwrap();
    assert!(matches!(
        &design.tokens["colors"].value,
        ThemeDesignTokenValueV1::Palette(ThemeValueV1::Literal(_))
    ));
    assert!(matches!(
        &design.tokens["fonts"].value,
        ThemeDesignTokenValueV1::FontStack(ThemeValueV1::Literal(_))
    ));
    assert!(serde_json::from_value::<ThemeReferenceV1>(json!(["orange"])).is_err());
    assert!(
        serde_json::from_value::<ThemeReferenceV1>(json!({"ref":"colors","extra":true})).is_err()
    );
}

#[test]
fn a_direct_semantic_style_needs_no_token_library() {
    let input = json!({"styles": {"node": {"fill": "#f0f4f8"}}});
    let design: ThemeDesignV1 = serde_json::from_value(input.clone()).expect("direct style");
    assert!(design.tokens.is_empty());
    assert_eq!(
        design.styles["node"].fill,
        SpecifiedWireV1::Value(ThemeValueV1::Literal(ThemeDesignPaintV1::Color(
            "#f0f4f8".into()
        )))
    );
    assert_eq!(serde_json::to_value(design).expect("serialize"), input);
    assert_eq!(
        serde_json::to_value(ThemeDesignV1::default()).expect("empty design"),
        json!({})
    );
}

#[test]
fn aliases_composites_and_presentation_metadata_survive_round_trip() {
    let input = json!({
        "tokens": {
            "color.ink": {"type": "color", "value": "#171717", "label": "Ink", "group": "Colors"},
            "color.shadow": {"type": "color", "value": {"ref": "color.ink"}},
            "size.offset": {"type": "length", "value": {"px": 6}, "description": "Shadow offset"},
            "space.body": {"type": "insets", "value": {"top": 8, "right": 12, "bottom": 8, "left": 12}},
            "space.body.alias": {"type": "insets", "value": {"ref": "space.body"}},
            "leading.body": {"type": "line-height", "value": 1.5},
            "leading.alias": {"type": "line-height", "value": {"ref": "leading.body"}},
            "number.softness": {"type": "number", "value": 0.25},
            "choice.cap": {"type": "string", "value": "round"},
            "font.body": {"type": "font-stack", "value": ["Inter", "sans-serif"]},
            "palette.series": {"type": "palette", "value": [{"ref": "color.ink"}, "#ffffff"]},
            "paint.surface": {"type": "paint", "value": {
                "kind": "linear-gradient", "angle_degrees": 45,
                "stops": [
                    {"offset": 0, "color": {"ref": "color.ink"}, "opacity": {"ref": "number.softness"}},
                    {"offset": 1, "color": "#ffffff"}
                ],
                "repetition": {"kind": "tiled", "width_px": 24, "height_px": 24}
            }},
            "stroke.body": {"type": "stroke", "value": {
                "paint": {"kind": "solid", "color": {"ref": "color.ink"}},
                "width": {"px": 3}, "linecap": {"ref": "choice.cap"}
            }},
            "text.body": {"type": "text-style", "value": {
                "font_stack": {"ref": "font.body"}, "font_size_px": {"px": 16},
                "line_height": {"ref": "leading.alias"}, "wrap": "normal"
            }},
            "effect.raised": {"type": "effect", "value": {
                "kind": "shadow-stack", "composition": "independent", "color_space": "srgb",
                "shadows": [{
                    "offset_x": {"ref": "size.offset"}, "offset_y": {"ref": "size.offset"},
                    "std_deviation": {"px": 0}, "color": {"ref": "color.shadow"}, "opacity": 0.8
                }]
            }},
            "effect.alias": {"type": "effect", "value": {"ref": "effect.raised"}}
        },
        "styles": {"node": {
            "fill": {"ref": "paint.surface"}, "stroke": {"ref": "stroke.body"},
            "typography": {"ref": "text.body"}, "effect": {"ref": "effect.alias"},
            "padding": {"ref": "space.body.alias"}
        }},
        "rules": [{"target": "actor", "family": "sequence", "variant": "main", "ordinal": {"exact": 2},
            "style": {"effect": {"ref": "effect.raised"}}}],
        "palettes": {"chart-series": {"ref": "palette.series"}},
        "canvas": {"base": "#f6f3e9", "layers": [{
            "paint": {"ref": "paint.surface"}, "opacity": 0.2, "blend_mode": "multiply",
            "offset_x": {"px": 2}, "offset_y": -3
        }], "bleed": {"top": 4, "right": 4, "bottom": 6, "left": 4}},
        "typography": {"default": {"ref": "text.body"}, "families": {"sequence": {"font_weight": 600}}},
        "mermaid": {"theme": "base", "dark_mode": false},
        "assets": {"available_sources": ["system"]},
        "requirements": {"capabilities": ["effects"]}
    });
    let design: ThemeDesignV1 = serde_json::from_value(input.clone()).expect("typed composites");
    let canonical = design.canonical_json_bytes().expect("canonicalize");
    assert_eq!(
        serde_json::from_slice::<ThemeDesignV1>(&canonical).expect("canonical round trip"),
        design
    );
    assert!(matches!(
        &design.tokens["color.shadow"].value,
        ThemeDesignTokenValueV1::Color(ThemeValueV1::Ref(ThemeReferenceV1 { r#ref })) if r#ref == "color.ink"
    ));
}

#[test]
fn clearable_fields_remain_distinct_from_omission_and_container_nulls() {
    let input = json!({"styles": {"node": {
        "fill": null, "effect": null, "stroke": {"paint": null, "width": null},
        "typography": {"font_stack": null, "font_size_px": null}
    }}});
    let design: ThemeDesignV1 = serde_json::from_value(input.clone()).expect("clear style facets");
    assert_eq!(design.styles["node"].fill, SpecifiedWireV1::Clear);
    assert_eq!(design.styles["node"].opacity, SpecifiedWireV1::Unspecified);
    assert_eq!(
        design.canonical_json_bytes().expect("canonicalize"),
        serde_json_canonicalizer::to_vec(&input).expect("canonical input")
    );

    for input in [
        json!({"tokens": null}),
        json!({"styles": null}),
        json!({"rules": null}),
        json!({"palettes": null}),
        json!({"canvas": null}),
        json!({"typography": null}),
        json!({"mermaid": null}),
        json!({"assets": null}),
        json!({"requirements": null}),
        json!({"canvas": {"layers": null}}),
        json!({"canvas": {"base": null}}),
        json!({"typography": {"families": null}}),
        json!({"styles": {"node": {"stroke": null}}}),
        json!({"styles": {"node": {"typography": null}}}),
    ] {
        assert!(
            serde_json::from_value::<ThemeDesignV1>(input.clone()).is_err(),
            "explicit null container must fail: {input}"
        );
    }
}

#[test]
fn references_and_token_payloads_fail_closed() {
    for input in [
        json!({"type": "color", "value": "#fff", "extra": true}),
        json!({"type": "future", "value": "#fff"}),
        json!({"type": "color"}),
        json!({"type": "color", "value": 12}),
        json!({"type": "number", "value": "12"}),
        json!({"type": "length", "value": {"px": 2, "percent": 50}}),
        json!({"type": "insets", "value": {"top": 2}}),
        json!({"type": "line-height", "value": {"percent": 120}}),
        json!({"type": "color", "value": null}),
        json!({"type": "color", "value": "#fff", "label": null}),
        json!({"type": "color", "value": {"ref": "color.ink", "fallback": "#fff"}}),
        json!({"type": "color", "value": {"ref": 3}}),
        json!({"type": "stroke", "value": {"width": {"ref": "size.edge", "px": 2}}}),
        json!({"type": "effect", "value": {"kind": "shadow-stack", "composition": "unknown", "shadows": []}}),
    ] {
        assert!(
            serde_json::from_value::<ThemeDesignTokenV1>(input.clone()).is_err(),
            "invalid token shape must fail: {input}"
        );
    }
    assert!(
        serde_json::from_str::<ThemeDesignTokenV1>(
            r##"{"type":"color","type":"color","value":"#fff"}"##
        )
        .is_err()
    );
}

#[test]
fn structured_styles_and_effects_reject_unrecognized_operations() {
    for input in [
        json!({"unknown": {}}),
        json!({"styles": {"node": {"shadow": "6px 6px black"}}}),
        json!({"styles": {"node": {"fill": {"kind": "solid", "color": "#fff", "extra": true}}}}),
        json!({"rules": [{"target": "node", "selector": ".node", "style": {}}]}),
        json!({"canvas": {"layers": [{"paint": "#fff", "extra": true}]}}),
        json!({"styles": {"node": {"effect": {
            "kind": "shadow-stack", "composition": "independent", "shadows": [{
                "offset_x": 6, "offset_y": 6, "std_deviation": 0, "color": "#000", "spread": 2
            }]
        }}}}),
        json!({"styles": {"node": {"effect": {
            "kind": "shadow-stack", "composition": "independent", "shadows": [{
                "offset_x": 6, "offset_y": 6, "std_deviation": 0, "color": "#000", "inset": true
            }]
        }}}}),
    ] {
        assert!(
            serde_json::from_value::<ThemeDesignV1>(input.clone()).is_err(),
            "invalid design shape must fail: {input}"
        );
    }
}

#[test]
fn advanced_literal_graphs_and_patterns_remain_typed() {
    let input = json!({
        "canvas": {"base": {"kind": "pattern", "pattern": "grid", "cell_width": {"px": 24},
            "cell_height": 24, "foreground": {"ref": "color.grid"}, "background": "transparent", "angle_degrees": 0}},
        "styles": {"node": {"effect": {"kind": "graph", "color_space": "linear-rgb", "primitives": [
            {"kind": "gaussian-blur", "input": "source-graphic", "std_deviation": 2}
        ]}}}
    });
    let design: ThemeDesignV1 =
        serde_json::from_value(input.clone()).expect("literal escape hatch");
    assert_eq!(
        serde_json::from_slice::<ThemeDesignV1>(
            &design.canonical_json_bytes().expect("canonicalize")
        )
        .expect("round trip"),
        design
    );
}

#[test]
fn serialization_rejects_non_finite_values_through_nested_typed_paths() {
    let mut designs = Vec::new();
    let mut direct = ThemeDesignV1::default();
    direct.styles.insert(
        "node".into(),
        ThemeDesignStyleV1 {
            opacity: SpecifiedWireV1::Value(ThemeValueV1::Literal(f32::NAN)),
            ..Default::default()
        },
    );
    designs.push(direct);

    for value in [
        ThemeDesignTokenValueV1::Insets(ThemeValueV1::Literal(ThemeInsetsWireV1::Sides {
            top: 1.0,
            right: 2.0,
            bottom: f32::NAN,
            left: 2.0,
        })),
        ThemeDesignTokenValueV1::LineHeight(ThemeValueV1::Literal(ThemeLineHeightWireV1::Px {
            px: f32::INFINITY,
        })),
    ] {
        let mut design = ThemeDesignV1::default();
        design.tokens.insert(
            "invalid".into(),
            ThemeDesignTokenV1 {
                label: None,
                description: None,
                group: None,
                value,
            },
        );
        designs.push(design);
    }

    let mut shadow = ThemeDesignV1::default();
    shadow.tokens.insert(
        "effect.raise".into(),
        ThemeDesignTokenV1 {
            label: None,
            description: None,
            group: None,
            value: ThemeDesignTokenValueV1::Effect(ThemeValueV1::Literal(
                ThemeDesignEffectV1::ShadowStack {
                    composition: ThemeShadowCompositionV1::Independent,
                    color_space: None,
                    shadows: vec![ThemeDesignShadowV1 {
                        offset_x: ThemeValueV1::Literal(ThemeLengthWireV1::PxValue(6.0)),
                        offset_y: ThemeValueV1::Literal(ThemeLengthWireV1::PxValue(6.0)),
                        std_deviation: ThemeValueV1::Literal(ThemeLengthWireV1::Px {
                            px: f32::INFINITY,
                        }),
                        color: ThemeValueV1::Literal("#000".into()),
                        opacity: None,
                    }],
                },
            )),
        },
    );
    designs.push(shadow);

    let mut gradient = ThemeDesignV1::default();
    gradient.canvas = Some(ThemeDesignCanvasV1 {
        layers: vec![ThemeDesignCanvasLayerV1 {
            paint: ThemeValueV1::Literal(ThemeDesignPaintV1::Structured(
                ThemeDesignPaintObjectV1::LinearGradient {
                    angle_degrees: ThemeValueV1::Literal(90.0),
                    stops: vec![ThemeDesignGradientStopV1 {
                        offset: ThemeValueV1::Literal(0.0),
                        color: ThemeValueV1::Literal("#fff".into()),
                        opacity: Some(ThemeValueV1::Literal(f32::NEG_INFINITY)),
                    }],
                    repetition: None,
                },
            )),
            opacity: None,
            blend_mode: None,
            offset_x: None,
            offset_y: None,
        }],
        ..Default::default()
    });
    designs.push(gradient);

    for design in designs {
        assert!(serde_json::to_value(&design).is_err());
        assert_eq!(
            design
                .canonical_json_bytes()
                .expect_err("non-finite input")
                .kind(),
            CanonicalJsonErrorKind::InvalidInput
        );
    }
}

#[test]
fn named_authoring_objects_reject_positional_sequence_representations() {
    for input in [
        json!([]),
        json!({"styles":{"node":[]}}),
        json!({"rules":[[]]}),
        json!({"canvas":[]}),
        json!({"canvas":{"layers":[["red"]]}}),
        json!({"typography":[]}),
        json!({"styles":{"node":{"stroke":[]}}}),
    ] {
        assert!(
            serde_json::from_value::<ThemeDesignV1>(input.clone()).is_err(),
            "{input}"
        );
    }
}
