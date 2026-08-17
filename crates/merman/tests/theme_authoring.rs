use merman::diagram_theme::{
    DiagramFamilyId, DiagramThemeCompiler, ThemeColorTokenV1, ThemeDefinitionAdmissionError,
    ThemeDefinitionBuilderV1, ThemeDefinitionCompileError, ThemeDefinitionV1,
    ThemeMaterializationError, ThemeMaterializer, ThemeRuleBuilderV1, ThemeRuleFacetV1,
    ThemeRuleSetWireV1, ThemeStylePatchWireV1, ThemeSupportOutputV1, ThemeSupportQueryV1,
    ThemeSupportStateV1, ThemeTarget, ThemeTokensV1, ThemeVariant, compile_theme_definition,
    compile_theme_definition_json, describe_theme_support,
};
use merman::svg::{ThemeResourceLimitId, ThemeResourcePolicy};
use merman::{OperationControl, RenderOutput, RenderRequest, Renderer};

struct AuthoringRenderWitness {
    name: &'static str,
    diagram_id: &'static str,
    source: &'static str,
}

const AUTHORING_RENDER_WITNESSES: [AuthoringRenderWitness; 3] = [
    AuthoringRenderWitness {
        name: "Flowchart",
        diagram_id: "authoring-flowchart",
        source: "flowchart LR\nA[Alpha] --> B[Beta]\n",
    },
    AuthoringRenderWitness {
        name: "State",
        diagram_id: "authoring-state",
        source: "stateDiagram-v2\n[*] --> Active\nActive --> [*]\n",
    },
    AuthoringRenderWitness {
        name: "Sequence",
        diagram_id: "authoring-sequence",
        source: "sequenceDiagram\nAlice->>Bob: Hello\nBob-->>Alice: World\n",
    },
];

fn light_authoring_witness() -> ThemeDefinitionV1 {
    ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_canvas("#fffdf5")
            .with_surface("#fef3c7")
            .with_surface_alt("#fde68a")
            .with_surface_muted("#fef9c3")
            .with_text("#3f2d20")
            .with_border("#b45309")
            .with_line("#92400e")
            .with_accent("#c2410c")
            .with_series(vec![
                "#f59e0b".to_owned(),
                "#84cc16".to_owned(),
                "#06b6d4".to_owned(),
            ]),
    )
}

fn dark_authoring_witness() -> ThemeDefinitionV1 {
    ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_canvas("#07111f")
            .with_surface("#172554")
            .with_surface_alt("#1e3a8a")
            .with_surface_muted("#0f172a")
            .with_text("#e0f2fe")
            .with_border("#38bdf8")
            .with_line("#7dd3fc")
            .with_accent("#f472b6")
            .with_series(vec![
                "#22d3ee".to_owned(),
                "#a78bfa".to_owned(),
                "#fb7185".to_owned(),
            ]),
    )
}

fn materialize_pretty_json_round_trip(
    definition: &ThemeDefinitionV1,
    witness_name: &str,
) -> merman::diagram_theme::MaterializedThemeWireV1 {
    let pretty_json = serde_json::to_string_pretty(definition)
        .unwrap_or_else(|error| panic!("{witness_name} definition should serialize: {error}"));
    let decoded: ThemeDefinitionV1 = serde_json::from_str(&pretty_json)
        .unwrap_or_else(|error| panic!("{witness_name} definition should decode: {error}"));
    assert_eq!(
        definition
            .canonical_json_bytes()
            .expect("the typed definition should canonicalize"),
        decoded
            .canonical_json_bytes()
            .expect("the JSON definition should canonicalize"),
        "{witness_name} canonical definition bytes must survive pretty JSON",
    );

    let materializer = ThemeMaterializer::new();
    let typed = materializer
        .materialize_theme(definition)
        .unwrap_or_else(|error| {
            panic!("{witness_name} typed definition should materialize: {error}")
        });
    let imported = materializer
        .materialize_theme(&decoded)
        .unwrap_or_else(|error| {
            panic!("{witness_name} JSON definition should materialize: {error}")
        });
    assert_eq!(
        typed.spec(),
        imported.spec(),
        "{witness_name} materialized spec must survive pretty JSON",
    );
    imported
}

fn witness_svg_request(diagram_id: &str) -> merman::SvgRequest {
    merman::SvgRequest {
        options: merman::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id.to_owned()),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn render_authoring_svg(
    renderer: &Renderer,
    witness: &AuthoringRenderWitness,
    theme: &merman::diagram_theme::DiagramTheme,
) -> String {
    let output = renderer
        .render(
            RenderRequest::svg(
                witness.source,
                OperationControl::new(),
                witness_svg_request(witness.diagram_id),
            )
            .with_theme(theme.clone()),
        )
        .unwrap_or_else(|error| panic!("{} SVG should render: {error}", witness.name));
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("{} should produce an SVG", witness.name);
    };
    output.svg().to_owned()
}

#[cfg(feature = "png")]
fn render_authoring_png(
    renderer: &Renderer,
    witness: &AuthoringRenderWitness,
    theme: &merman::diagram_theme::DiagramTheme,
) -> Vec<u8> {
    let output = renderer
        .render(
            RenderRequest::png(
                witness.source,
                OperationControl::new(),
                merman::PngRequest {
                    svg: witness_svg_request(witness.diagram_id),
                    options: merman::svg::export::RasterOptions::default(),
                },
            )
            .with_theme(theme.clone()),
        )
        .unwrap_or_else(|error| panic!("{} PNG should render: {error}", witness.name));
    let RenderOutput::Png(Some(output)) = output else {
        panic!("{} should produce a PNG", witness.name);
    };
    assert!(output.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
    output.into_bytes()
}

#[test]
fn prefreeze_tokens_only_themes_round_trip_and_render_without_state_leakage() {
    let light_definition = light_authoring_witness();
    let dark_definition = dark_authoring_witness();
    assert!(light_definition.styles().is_empty());
    assert!(dark_definition.styles().is_empty());
    let light_materialized = materialize_pretty_json_round_trip(&light_definition, "light");
    let dark_materialized = materialize_pretty_json_round_trip(&dark_definition, "dark");
    let compiler = DiagramThemeCompiler::new();
    let light_theme = compiler
        .compile_spec_wire(light_materialized.into_spec())
        .expect("the light materialized spec should compile once");
    let dark_theme = compiler
        .compile_spec_wire(dark_materialized.into_spec())
        .expect("the dark materialized spec should compile once");
    let first_renderer = Renderer::new();
    let second_renderer = Renderer::new();

    for witness in &AUTHORING_RENDER_WITNESSES {
        let first_light = render_authoring_svg(&first_renderer, witness, &light_theme);
        let first_dark = render_authoring_svg(&first_renderer, witness, &dark_theme);
        let repeated_light = render_authoring_svg(&first_renderer, witness, &light_theme);
        let independent_light = render_authoring_svg(&second_renderer, witness, &light_theme);
        let independent_dark = render_authoring_svg(&second_renderer, witness, &dark_theme);

        assert_eq!(
            first_light, repeated_light,
            "{} light -> dark -> light SVG rendering must not leak state",
            witness.name,
        );
        assert_eq!(
            first_light, independent_light,
            "{} light SVG must be stable across Renderer instances",
            witness.name,
        );
        assert_eq!(
            first_dark, independent_dark,
            "{} dark SVG must be stable across Renderer instances",
            witness.name,
        );
        assert_ne!(
            first_light, first_dark,
            "{} light and dark SVGs must differ",
            witness.name,
        );
    }

    #[cfg(feature = "png")]
    for witness in &AUTHORING_RENDER_WITNESSES {
        let first_light = render_authoring_png(&first_renderer, witness, &light_theme);
        let first_dark = render_authoring_png(&first_renderer, witness, &dark_theme);
        let repeated_light = render_authoring_png(&first_renderer, witness, &light_theme);
        let independent_light = render_authoring_png(&second_renderer, witness, &light_theme);
        let independent_dark = render_authoring_png(&second_renderer, witness, &dark_theme);

        assert_eq!(
            first_light, repeated_light,
            "{} light -> dark -> light PNG rendering must not leak state",
            witness.name,
        );
        assert_eq!(
            first_light, independent_light,
            "{} light PNG must be stable across Renderer instances",
            witness.name,
        );
        assert_eq!(
            first_dark, independent_dark,
            "{} dark PNG must be stable across Renderer instances",
            witness.name,
        );
        assert_ne!(
            first_light, first_dark,
            "{} light and dark PNGs must differ",
            witness.name,
        );
    }
}

#[test]
fn complete_spec_can_cold_start_without_authoring_materialization() {
    const COLD_START_COLOR: &str = "#dc2626";
    let style = merman::svg::ThemeStylePatch::default().with_fill(
        merman::svg::CanvasPaint::solid(COLD_START_COLOR)
            .expect("the cold-start fixture color should be valid"),
    );
    let spec = merman::svg::DiagramThemeSpec::new().with_styles(
        merman::svg::ThemeRuleSet::default().with_rule(
            merman::svg::ThemeRule::new(merman::svg::ThemeTarget::Node, style)
                .for_family(DiagramFamilyId::FLOWCHART),
        ),
    );
    let theme = DiagramThemeCompiler::new()
        .compile(spec)
        .expect("the complete spec should compile without authoring materialization");
    let svg = render_authoring_svg(&Renderer::new(), &AUTHORING_RENDER_WITNESSES[0], &theme);
    let document = roxmltree::Document::parse(&svg).expect("valid cold-start Flowchart SVG");
    let node_style = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "label-container")
                })
        })
        .and_then(|node| node.attribute("style"))
        .expect("the cold-start Flowchart node style");
    assert!(
        node_style.contains(&format!("fill:{COLD_START_COLOR} !important")),
        "the complete-spec rule must own the terminal node fill: {node_style}",
    );
}

#[test]
fn typed_family_rule_matches_the_equivalent_shareable_json() {
    let typed = ThemeDefinitionBuilderV1::new(ThemeTokensV1::default())
        .with_rule(
            ThemeRuleBuilderV1::new(ThemeTarget::Node)
                .for_family(DiagramFamilyId::FLOWCHART)
                .with_variant(ThemeVariant::Primary)
                .with_ordinal_cycle(4, 1)
                .with_fill_color("#60a5fa")
                .with_stroke_color("#1e293b")
                .with_stroke_width(2.0)
                .with_radius(8.0)
                .clear(ThemeRuleFacetV1::FillOpacity),
        )
        .with_ordinal_palette(ThemeTarget::Node, ["#60a5fa", "#34d399"])
        .build();
    let decoded: ThemeDefinitionV1 = serde_json::from_str(
        r##"{
            "authoring_schema_version": 1,
            "expansion_version": 1,
            "tokens": {},
            "styles": [
                {
                    "kind": "rule",
                    "target": "node",
                    "family": "flowchart",
                    "variant": "primary",
                    "ordinal": {"cycle": {"period": 4, "offset": 1}},
                    "style": {
                        "fill": "#60a5fa",
                        "fill_opacity": null,
                        "stroke": {"paint": "#1e293b", "width": 2},
                        "radius": 8
                    }
                },
                {
                    "kind": "ordinal-palette",
                    "target": "node",
                    "colors": ["#60a5fa", "#34d399"]
                }
            ]
        }"##,
    )
    .expect("the equivalent shared JSON should decode");

    assert_eq!(
        typed
            .canonical_json_bytes()
            .expect("the typed definition should canonicalize"),
        decoded
            .canonical_json_bytes()
            .expect("the decoded definition should canonicalize"),
    );
}

#[test]
fn versioned_authoring_materializes_and_compiles_through_the_rust_facade() {
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_color(ThemeColorTokenV1::Canvas, "#0f172a")
            .with_color(ThemeColorTokenV1::Surface, "#111827")
            .with_color(ThemeColorTokenV1::Text, "#e5e7eb"),
    );
    let compiler = DiagramThemeCompiler::new();
    let materialized = ThemeMaterializer::new()
        .materialize_theme(&definition)
        .expect("the explicit materialization step should succeed");
    let expected = compiler
        .compile_spec_wire(materialized.into_spec())
        .expect("the explicit compilation step should succeed");
    let theme = compile_theme_definition(&compiler, &definition)
        .expect("the materialized complete spec should compile");

    assert_eq!(theme.recipe_fingerprint(), expected.recipe_fingerprint());
}

#[test]
fn series_token_reaches_flowchart_node_and_pie_slice_terminal_fills() {
    const FIRST_SERIES_COLOR: &str = "#12ab34";
    let definition = ThemeDefinitionV1::new(
        ThemeTokensV1::default()
            .with_series(vec![FIRST_SERIES_COLOR.to_owned(), "#3456de".to_owned()]),
    );
    let theme = compile_theme_definition(&DiagramThemeCompiler::new(), &definition)
        .expect("the shared authoring definition should compile");

    let flowchart = Renderer::new()
        .render(
            RenderRequest::svg(
                "flowchart LR\nA[Alpha] --> B[Beta]\n",
                OperationControl::new(),
                merman::SvgRequest::default(),
            )
            .with_theme(theme.clone()),
        )
        .expect("the authored Flowchart should render");
    let RenderOutput::Svg(Some(flowchart)) = flowchart else {
        panic!("expected an authored Flowchart SVG");
    };
    let flowchart_document =
        roxmltree::Document::parse(flowchart.svg()).expect("valid authored Flowchart SVG");
    let first_node = flowchart_document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("A")
                && node.attribute("data-et") == Some("node")
        })
        .expect("the first Flowchart node wrapper");
    let first_node_style = first_node
        .descendants()
        .find(|node| {
            node.is_element()
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "label-container")
                })
        })
        .and_then(|node| node.attribute("style"))
        .expect("the first Flowchart node shape style");
    assert!(
        first_node_style.contains(&format!("fill:{FIRST_SERIES_COLOR} !important")),
        "tokens.series[0] must own the first Flowchart node fill: {first_node_style}"
    );

    let pie = Renderer::new()
        .render(
            RenderRequest::svg(
                "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n",
                OperationControl::new(),
                merman::SvgRequest::default(),
            )
            .with_theme(theme),
        )
        .expect("the authored Pie should render");
    let RenderOutput::Svg(Some(pie)) = pie else {
        panic!("expected an authored Pie SVG");
    };
    let pie_document = roxmltree::Document::parse(pie.svg()).expect("valid authored Pie SVG");
    let first_slice_fill = pie_document
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "pieCircle")
                })
        })
        .and_then(|node| node.attribute("fill"))
        .expect("the first Pie slice fill");
    assert_eq!(first_slice_fill, FIRST_SERIES_COLOR);

    let first_legend_style = pie_document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.parent().is_some_and(|parent| {
                    parent.has_tag_name("g") && parent.attribute("class") == Some("legend")
                })
        })
        .and_then(|node| node.attribute("style"))
        .expect("the first Pie legend swatch style");
    assert!(
        first_legend_style.contains("fill: rgb(18, 171, 52)"),
        "tokens.series[0] must own the first Pie legend swatch: {first_legend_style}"
    );
}

#[test]
fn rust_facade_exposes_the_versioned_support_discovery_operation() {
    let support = describe_theme_support(&ThemeSupportQueryV1::known(
        DiagramFamilyId::FLOWCHART.as_str(),
        ThemeSupportOutputV1::StandaloneSvg,
        ThemeTarget::Node.id(),
        ThemeRuleFacetV1::Radius,
    ));

    assert_eq!(support.state(), ThemeSupportStateV1::Conditional);
    assert_eq!(support.query().family_id(), "flowchart");
}

#[test]
fn rust_facade_preserves_materialization_and_compilation_error_sources() {
    let materialization_error = compile_theme_definition(
        &DiagramThemeCompiler::new(),
        &ThemeDefinitionV1::new(ThemeTokensV1::default().with_series(Vec::new())),
    )
    .expect_err("an empty authored series should fail during materialization");
    assert!(matches!(
        materialization_error,
        ThemeDefinitionCompileError::Materialization(ThemeMaterializationError::EmptySeries)
    ));

    let compilation_error = compile_theme_definition(
        &DiagramThemeCompiler::new(),
        &ThemeDefinitionV1::new(ThemeTokensV1::default()).with_styles(vec![
            ThemeRuleSetWireV1::Rule {
                target: "future-node".to_owned(),
                family: None,
                variant: None,
                ordinal: None,
                style: ThemeStylePatchWireV1::default(),
            },
        ]),
    )
    .expect_err("an unknown semantic target should fail during compilation");
    assert!(matches!(
        compilation_error,
        ThemeDefinitionCompileError::Compilation(_)
    ));
}

#[test]
fn json_authoring_rejects_the_first_rule_beyond_the_v1_budget_before_typed_decode() {
    let rule = r##"{"kind":"rule","target":"node","style":{"fill":"#123456"}}"##;
    let admitted_styles = std::iter::repeat_n(rule, 489).collect::<Vec<_>>().join(",");

    let exact = format!(
        r##"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{admitted_styles}]}}"##
    );
    compile_theme_definition_json(&DiagramThemeCompiler::new(), exact.as_bytes())
        .expect("the exact V1 authored-rule budget should compile");

    let oversized = format!(
        r##"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{admitted_styles},{{"kind":"rule","target":{{"typed":"decode must not reach this value"}},"style":{{"fill":"#123456"}}}}]}}"##
    );
    let error = compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized.as_bytes())
        .expect_err("the first rule beyond the V1 budget must fail during streaming admission");
    assert!(matches!(
        error,
        ThemeDefinitionCompileError::Materialization(
            ThemeMaterializationError::RuleBudgetExceeded {
                actual: 490,
                max: 489
            }
        )
    ));
}

#[test]
fn json_authoring_collection_limits_accept_the_exact_boundary_and_reject_the_next_item() {
    let colors = std::iter::repeat_n("\"#123456\"", 256)
        .collect::<Vec<_>>()
        .join(",");
    let exact_series = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"series":[{colors}]}}}}"#
    );
    compile_theme_definition_json(&DiagramThemeCompiler::new(), exact_series.as_bytes())
        .expect("the exact series-color budget should compile");
    let oversized_series = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"series":[{colors},{{"typed":"decode must not reach this value"}}]}}}}"#
    );
    assert!(matches!(
        compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized_series.as_bytes()),
        Err(ThemeDefinitionCompileError::Materialization(
            ThemeMaterializationError::InvalidTokenValue {
                path: "/tokens/series"
            }
        ))
    ));

    let families = (0..32)
        .map(|index| format!("\"Authoring Font {index}\""))
        .collect::<Vec<_>>()
        .join(",");
    let exact_fonts = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"typography":{{"font_stack":[{families}]}}}}}}"#
    );
    compile_theme_definition_json(&DiagramThemeCompiler::new(), exact_fonts.as_bytes())
        .expect("the exact font-stack budget should compile");
    let oversized_fonts = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{"typography":{{"font_stack":[{families},{{"typed":"decode must not reach this value"}}]}}}}}}"#
    );
    assert!(matches!(
        compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized_fonts.as_bytes()),
        Err(ThemeDefinitionCompileError::Materialization(
            ThemeMaterializationError::InvalidTokenValue {
                path: "/tokens/typography/font_stack"
            }
        ))
    ));

    let exact_palette = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{{"kind":"ordinal-palette","target":"node","colors":[{colors}]}}]}}"#
    );
    compile_theme_definition_json(&DiagramThemeCompiler::new(), exact_palette.as_bytes())
        .expect("the exact ordinal-palette color budget should compile");
    let oversized_palette = format!(
        r#"{{"authoring_schema_version":1,"expansion_version":1,"tokens":{{}},"styles":[{{"kind":"ordinal-palette","target":"node","colors":[{colors},{{"typed":"decode must not reach this value"}}]}}]}}"#
    );
    assert!(matches!(
        compile_theme_definition_json(&DiagramThemeCompiler::new(), oversized_palette.as_bytes()),
        Err(ThemeDefinitionCompileError::Admission(
            ThemeDefinitionAdmissionError::InvalidJson { .. }
        ))
    ));
}

#[test]
fn typed_compact_paint_obeys_the_decoded_string_ceiling() {
    let definition = ThemeDefinitionBuilderV1::new(ThemeTokensV1::default())
        .with_rule(
            ThemeRuleBuilderV1::new(ThemeTarget::Node).with_fill_color("x".repeat(64 * 1024 + 1)),
        )
        .build();
    let error = compile_theme_definition(&DiagramThemeCompiler::new(), &definition)
        .expect_err("compact paint strings must use the shared decoded-string ceiling");

    assert!(matches!(
        error,
        ThemeDefinitionCompileError::Admission(ThemeDefinitionAdmissionError::CollectionLimit {
            path: "/styles/rule/style/paint/color",
            actual: 65_537,
            max: 65_536,
        })
    ));
}

#[test]
fn typed_authoring_uses_the_caller_owned_encoded_input_policy_before_expansion() {
    let policy = ThemeResourcePolicy::default()
        .with_limit(ThemeResourceLimitId::MaxThemeEncodedBytes, 1)
        .expect("one byte is a valid caller-owned encoded-theme ceiling");
    let compiler = DiagramThemeCompiler::new().with_resource_policy(policy);
    let error =
        compile_theme_definition(&compiler, &ThemeDefinitionV1::new(ThemeTokensV1::default()))
            .expect_err(
                "the typed facade must apply the compiler policy before materialization expands",
            );

    assert!(matches!(
        error,
        ThemeDefinitionCompileError::Admission(ThemeDefinitionAdmissionError::ResourceLimit(
            ref error
        )) if error.limit == "max_theme_encoded_bytes" && error.actual == 2 && error.max == 1
    ));
}
