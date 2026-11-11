use merman_core::{MermaidEngine, ParseOptions};
use merman_model::OperationControl;
use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};
use merman_model::resources::InputResourcePolicy;
use merman_render::DiagramFamilyId;
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, CanvasSpec, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack,
    OrdinalPalette, ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn sankey_node_palette(colors: &[&str]) -> OrdinalPalette {
    OrdinalPalette::new(
        colors
            .iter()
            .map(|color| ThemeColorValue::parse(color).expect("valid Sankey node palette color")),
    )
    .expect("non-empty Sankey node palette")
}

fn sankey_node_palette_theme(colors: &[&str]) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_ordinal_palette(ThemeTarget::Node, sankey_node_palette(colors)),
            ),
        )
        .expect("compile Sankey node palette theme")
}

fn sankey_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::SANKEY, typography),
        ))
        .expect("compile Sankey typography theme")
}

fn sankey_text_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    let rule = ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(fill),
    )
    .for_family(DiagramFamilyId::SANKEY);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
        .expect("compile Sankey text fill theme")
}

fn render_sankey_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = common::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Sankey")
        .expect("detect themed Sankey")
        .into_presentation();
    render_sankey_presentation_with_theme_requirement(&parsed, theme, portability)
}

fn render_sankey_presentation_with_theme_requirement(
    parsed: &PresentationDiagram,
    theme: &DiagramTheme,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Sankey session");
    family::prepare(parsed, &LayoutOptions::default(), session)?.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("sankey-theme".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )
}

fn amend_sankey_node_ids(parsed: PresentationDiagram, ids: [&str; 2]) -> PresentationDiagram {
    let (metadata, mut model, context) = parsed.into_parts();
    let RenderSemanticModel::Sankey(sankey) = &mut model else {
        panic!("expected Sankey model");
    };
    let [source, target] = sankey.graph.nodes.as_mut_slice() else {
        panic!("Sankey label fixture must contain exactly two nodes");
    };
    let [link] = sankey.graph.links.as_mut_slice() else {
        panic!("Sankey label fixture must contain exactly one link");
    };
    assert_eq!(link.source, source.id);
    assert_eq!(link.target, target.id);
    assert_ne!(ids[0], ids[1]);
    source.id = ids[0].to_string();
    target.id = ids[1].to_string();
    link.source = source.id.clone();
    link.target = target.id.clone();
    PresentationDiagram::try_from_frozen(
        metadata,
        model,
        context,
        &InputResourcePolicy::default(),
        &OperationControl::new(),
    )
    .expect("validate caller-authored Sankey presentation")
    .expect("admit caller-authored Sankey presentation")
}

fn render_sankey_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
) -> family::RenderedFamilySvg {
    render_sankey_with_theme_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render strict portable Sankey")
}

fn sankey_node_fills(svg: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).expect("valid Sankey SVG");
    document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("node"))
        .map(|node| {
            node.children()
                .find(|child| child.has_tag_name("rect"))
                .and_then(|rect| rect.attribute("fill"))
                .unwrap_or_else(|| panic!("Sankey node group must contain a filled rect: {svg}"))
                .to_string()
        })
        .collect()
}

fn sankey_style_text(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Sankey SVG");
    document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Sankey SVG stylesheet")
        .to_owned()
}

fn render_sankey(source: &str, options: &SvgRenderOptions) -> String {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("layout ok")
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render SVG")
        .svg()
        .to_owned()
}

#[test]
fn sankey_visible_values_use_ecmascript_number_text() {
    for (value, expected) in [
        ("1e30", "1e+30"),
        ("1e21", "999999999999999900000"),
        ("10", "10"),
    ] {
        let svg = render_sankey(
            &format!("sankey\nA,B,{value}\n"),
            &SvgRenderOptions::default(),
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let labels = document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .map(|node| node.text().unwrap_or_default().to_string())
            .collect::<Vec<_>>();
        assert_eq!(labels, [format!("A\n{expected}"), format!("B\n{expected}")]);
    }
}

fn try_render_sankey_with_resource_policy(
    source: &str,
    options: &SvgRenderOptions,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Sankey resource-bound fixture")
        .expect("detect Sankey resource-bound fixture")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Sankey resource-bound session");
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)?
        .render_svg(options, &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn sankey_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "sankey-beta\nInput,Transform,10\nTransform,Output,8\n";
    let options = SvgRenderOptions::default();
    let baseline = try_render_sankey_with_resource_policy(
        source,
        &options,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Sankey baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Sankey fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Sankey SVG byte ceiling");
    let exact = try_render_sankey_with_resource_policy(source, &options, exact_policy)
        .expect("the exact Sankey family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Sankey SVG byte ceiling");
    let error = try_render_sankey_with_resource_policy(source, &options, below_policy)
        .expect_err("one byte below the Sankey family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Sankey MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.max, below_exact);
    assert!(limit.actual > limit.max);
    assert!(limit.explicit_overrides.iter().any(|resource_override| {
        resource_override.id == ResourceLimitId::MaxSvgBytes
            && resource_override.value == below_exact
    }));
}

#[test]
fn sankey_prepared_labels_enforce_the_reported_retained_byte_boundary_without_a_theme() {
    let source = "---\nconfig:\n  sankey:\n    showValues: false\n---\nsankey\nAB,CD,1\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let baseline = family::prepare(
        &parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic()
            .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .begin_session()
            .unwrap(),
    )
    .unwrap()
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .unwrap();
    let expected_svg = baseline.svg().to_owned();
    let completion = baseline.into_completion();
    let exact = completion
        .report()
        .session_report()
        .prepared_text_retained_bytes_peak();
    assert!(
        exact >= 4,
        "both literal labels must own their payload bytes"
    );
    drop(completion);

    for maximum in [1, exact - 1, exact] {
        let result = try_render_sankey_with_resource_policy(
            source,
            &SvgRenderOptions::default(),
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, maximum)
                .unwrap(),
        );
        if maximum == exact {
            assert_eq!(result.unwrap(), expected_svg);
            continue;
        }
        let merman_render::Error::ResourceLimitExceeded(limit) = result.unwrap_err() else {
            panic!("the prepared-label byte ceiling must reject before SVG output");
        };
        assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(
            limit.limit,
            ResourceLimitId::MaxPreparedTextRetainedBytes.as_str()
        );
        assert_eq!(limit.max, maximum);
        assert!(limit.actual > maximum);
        assert!(limit.explicit_overrides.iter().any(|resource_override| {
            resource_override.id == ResourceLimitId::MaxPreparedTextRetainedBytes
                && resource_override.value == maximum
        }));
    }
}

#[test]
fn sankey_prepared_labels_bound_prefix_and_suffix_amplification_before_svg_output() {
    let prefix = "p".repeat(64 * 1024);
    let suffix = "s".repeat(8 * 1024);
    let links = (0..31)
        .map(|index| format!("N{index},N{},1\n", index + 1))
        .collect::<String>();
    let source = format!(
        "---\nconfig: {}\n---\nsankey\n{links}",
        serde_json::json!({"sankey": {"showValues": true, "prefix": prefix, "suffix": suffix}})
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    for maximum in [4 * 1024, 4 * 1024 * 1024] {
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, maximum)
                    .unwrap()
                    .with_limit(ResourceLimitId::MaxSvgBytes, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();
        match family::prepare(&parsed, &LayoutOptions::default(), session) {
            Ok(artifact) => {
                assert_eq!(maximum, 4 * 1024 * 1024);
                drop(artifact);
            }
            Err(merman_render::Error::ResourceLimitExceeded(limit)) => {
                assert_eq!(maximum, 4 * 1024);
                assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
                assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
                assert_eq!(
                    limit.limit,
                    ResourceLimitId::MaxPreparedTextRetainedBytes.as_str()
                );
                assert_eq!(limit.max, maximum);
                assert!(limit.actual >= 32 * (64 + 8) * 1024);
                assert!(limit.actual < 4 * 1024 * 1024);
            }
            Err(error) => panic!("unexpected Sankey preparation error: {error}"),
        }
    }
}

#[test]
fn sankey_svg_uses_configured_node_colors_and_outlined_labels() {
    let svg = render_sankey(
        r##"---
config:
  sankey:
    nodeColors:
      A: "#112233"
      B: rebeccapurple
    labelStyle: outlined
---
sankey-beta
A,B,10
"##,
        &SvgRenderOptions::default(),
    );

    assert!(
        svg.contains(r##"fill="#112233""##),
        "expected node A to use configured fill: {svg}"
    );
    assert!(
        svg.contains(r#"fill="rebeccapurple""#),
        "expected node B to use configured fill: {svg}"
    );
    assert!(
        svg.contains(r##"stop-color="#112233""##),
        "expected source gradient stop to use configured color: {svg}"
    );
    assert!(
        svg.contains(r#"stop-color="rebeccapurple""#),
        "expected target gradient stop to use configured color: {svg}"
    );
    assert!(
        svg.contains(r#"class="sankey-label-bg""#),
        "expected outlined label background text: {svg}"
    );
    assert!(
        svg.contains(r#"class="sankey-label-fg""#),
        "expected outlined label foreground text: {svg}"
    );
    assert!(
        svg.contains(".sankey-label-bg"),
        "expected outlined label CSS: {svg}"
    );
}

#[test]
fn sankey_viewport_contains_all_raw_labels_with_their_emitted_anchors() {
    use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextStyle};

    let long_records = format!("{},Output,1\n", "WideLabel".repeat(20));
    let measurer = DeterministicTextMeasurer::default();
    let text_style = TextStyle {
        font_family: Some("monospace".to_string()),
        font_size: 14.0,
        ..TextStyle::default()
    };
    for (label_style, records, show_values) in [
        ("outlined", "A,B,1\n", false),
        ("legacy", "A,B,1\n", false),
        ("outlined", "Input,Hub,1\nOther,Hub,2\nHub,Output,3\n", true),
        ("legacy", long_records.as_str(), true),
    ] {
        for use_max_width in [false, true] {
            let source = format!(
                "---\nconfig:\n  fontFamily: monospace\n  sankey:\n    labelStyle: {label_style}\n    showValues: {show_values}\n    useMaxWidth: {use_max_width}\n    prefix: '<br>value: '\n    suffix: ' units'\n---\nsankey\n{records}"
            );
            let svg = render_sankey(&source, &SvgRenderOptions::default());
            let document = roxmltree::Document::parse(&svg).unwrap();
            let root = document.root_element();
            let view_box = root
                .attribute("viewBox")
                .unwrap()
                .split_whitespace()
                .map(|number| number.parse::<f64>().unwrap())
                .collect::<Vec<_>>();
            let left = view_box[0];
            let right = left + view_box[2];
            let labels = document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .collect::<Vec<_>>();
            assert!(!labels.is_empty());
            for label in labels {
                let text = label.text().unwrap_or_default();
                let width = measurer.measure_svg_raw_text_bbox_width_px(text, &text_style);
                let x = label.attribute("x").unwrap().parse::<f64>().unwrap();
                let label_left = match label.attribute("text-anchor") {
                    Some("start") => x,
                    Some("end") => x - width,
                    anchor => panic!("unexpected Sankey label anchor {anchor:?}"),
                };
                assert!(
                    left <= label_left + 1e-6 && right + 1e-6 >= label_left + width,
                    "viewport [{left}, {right}] must contain {text:?} at [{label_left}, {}]: {source}",
                    label_left + width
                );
            }
            if use_max_width {
                assert_eq!(root.attribute("width"), Some("100%"));
                assert!(
                    root.attribute("style")
                        .unwrap()
                        .contains(&format!("max-width: {}px", view_box[2]))
                );
            } else {
                assert_eq!(
                    root.attribute("width").unwrap().parse::<f64>().unwrap(),
                    view_box[2]
                );
            }
            if records == "A,B,1\n" {
                if label_style == "legacy" {
                    assert_eq!((left, right), (0.0, 600.0));
                } else {
                    assert!(right > 606.0);
                }
            } else if label_style == "outlined" {
                assert!(left < 0.0 && right > 600.0);
            } else {
                assert!(right > 600.0);
            }
        }
    }
}

#[test]
fn sankey_label_bounds_use_the_raw_text_profile_and_emitted_font() {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfile,
        TextMeasurementProfileIdentity,
    };
    use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct RawLabelMeasurer {
        labels: Mutex<Vec<String>>,
    }

    impl TextMeasurer for RawLabelMeasurer {
        fn measure(&self, _text: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 0.0,
                height: style.font_size,
                line_count: 1,
            }
        }

        fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
            assert_eq!(style.font_family.as_deref(), Some("monospace"));
            assert_eq!(style.font_size, 14.0);
            assert!(style.font_weight.is_none());
            assert!(style.font_style.is_none());
            self.labels.lock().unwrap().push(text.to_owned());
            80.0
        }
    }

    let backend = Arc::new(RawLabelMeasurer::default());
    let profile = TextMeasurementProfile::new(
        TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.sankey-raw-labels").unwrap(),
            "1",
        )
        .unwrap(),
        backend.clone(),
    );
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::deterministic()
            .with_profile_for_phase(TextMeasurementPhase::SvgBBox, profile),
    );
    let source = concat!(
        "---\nconfig:\n  fontFamily: monospace\n  sankey:\n",
        "    labelStyle: outlined\n    prefix: 'total '\n    suffix: ' units'\n---\n",
        "sankey\nSource,Target,1\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    // Exercise the renderer's preprocessor-placeholder decoding with checked typed IDs;
    // the CSV grammar itself admits only ASCII field content.
    let parsed = amend_sankey_node_ids(parsed, ["ﬂ°°35¶ß32;", "ﬂ°°32¶ß"]);
    let session = environment.begin_session().unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let view_box = document
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .map(|number| number.parse::<f64>().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(view_box[0] + view_box[2], 686.0);
    assert!(sankey_style_text(rendered.svg()).contains(".node-labels{font-family:monospace;}"));
    let measured = backend.labels.lock().unwrap();
    assert_eq!(*measured, ["#32;\ntotal 1 units", " \ntotal 1 units"]);
    let emitted = document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| node.text().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(
        emitted,
        ["#32;\ntotal 1 units", " \ntotal 1 units"].repeat(2),
        "outlined passes must serialize the measured text without decoding it again"
    );
}

#[test]
fn sankey_typed_text_fill_reaches_root_and_outlined_label_foreground() {
    let theme = sankey_text_fill_theme(CanvasPaint::solid("#123456").expect("valid fill"));
    let source = concat!(
        "---\n",
        "config:\n",
        "  sankey:\n",
        "    labelStyle: outlined\n",
        "---\n",
        "sankey-beta\n",
        "A,B,10\n",
    );
    let rendered = render_sankey_with_theme_and_engine(source, &theme, MermaidEngine::new());
    let style = sankey_style_text(rendered.svg());
    let root_rule = style
        .split_once('}')
        .map(|(rule, _)| rule)
        .expect("Sankey root CSS rule");
    assert!(
        root_rule.contains("#sankey-theme{") && root_rule.contains("fill:#123456;"),
        "{style}"
    );
    assert!(
        style.contains("#sankey-theme .sankey-label-fg{fill:#123456;}"),
        "{style}"
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn sankey_generated_ids_are_prefixed_when_diagram_id_is_provided() {
    let svg = render_sankey(
        "sankey-beta\nA,B,10\n",
        &SvgRenderOptions {
            diagram_id: Some("sankey-inline".to_string()),
            ..SvgRenderOptions::default()
        },
    );

    assert!(
        svg.contains(r#"id="sankey-inline-node-1""#),
        "expected scoped Sankey node id: {svg}"
    );
    assert!(
        svg.contains(r#"id="sankey-inline-linearGradient-3""#),
        "expected scoped Sankey gradient id: {svg}"
    );
    assert!(
        svg.contains(r#"stroke="url(#sankey-inline-linearGradient-3)""#),
        "expected scoped Sankey gradient reference: {svg}"
    );
    assert!(
        !svg.contains(r#"id="node-1""#),
        "expected no bare Sankey node id: {svg}"
    );
    assert!(
        !svg.contains(r#"id="linearGradient-3""#),
        "expected no bare Sankey gradient id: {svg}"
    );
    assert!(
        !svg.contains(r#"stroke="url(#linearGradient-3)""#),
        "expected no bare Sankey gradient reference: {svg}"
    );
}

#[test]
fn sankey_generated_ids_keep_mermaid_style_without_diagram_id() {
    let svg = render_sankey("sankey-beta\nA,B,10\n", &SvgRenderOptions::default());

    assert!(
        svg.contains(r#"id="node-1""#),
        "expected Mermaid-style Sankey node id without explicit diagram_id: {svg}"
    );
    assert!(
        svg.contains(r#"id="linearGradient-3""#),
        "expected Mermaid-style Sankey gradient id without explicit diagram_id: {svg}"
    );
    assert!(
        svg.contains(r#"stroke="url(#linearGradient-3)""#),
        "expected Mermaid-style Sankey gradient reference without explicit diagram_id: {svg}"
    );
    assert!(
        !svg.contains(r#"id="sankey-node-1""#),
        "expected default rendering to avoid implicit node id scoping: {svg}"
    );
    assert!(
        !svg.contains(r#"id="sankey-linearGradient-3""#),
        "expected default rendering to avoid implicit resource id scoping: {svg}"
    );
}

#[test]
fn sankey_node_palette_reaches_terminal_rects_and_gradient_stops() {
    let theme = sankey_node_palette_theme(&["#123456", "#abcdef"]);
    let rendered =
        render_sankey_with_theme_and_engine("sankey-beta\nA,B,10\n", &theme, MermaidEngine::new());

    assert_eq!(
        sankey_node_fills(rendered.svg()),
        vec!["#123456", "#abcdef"]
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Sankey SVG");
    let gradient_stops = document
        .descendants()
        .filter(|node| node.has_tag_name("stop"))
        .filter_map(|node| node.attribute("stop-color"))
        .collect::<Vec<_>>();
    assert_eq!(gradient_stops, vec!["#123456", "#abcdef"]);

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn sankey_palette_and_typography_evidence_compose_without_shadowing() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(
                        DiagramFamilyId::SANKEY,
                        ThemeTextStyle::default().with_font_stack(
                            FontStack::single("SankeyCombined")
                                .expect("valid combined Sankey font stack"),
                        ),
                    ),
                )
                .with_styles(ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Node,
                    sankey_node_palette(&["#123456", "#abcdef"]),
                )),
        )
        .expect("compile combined Sankey theme");
    let rendered = render_sankey_with_theme_requirement(
        "sankey-beta\nA,B,10\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render combined strict portable Sankey theme");

    assert!(rendered.svg().contains("font-family:SankeyCombined;"));
    assert_eq!(
        sankey_node_fills(rendered.svg()),
        vec!["#123456", "#abcdef"]
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn sankey_typed_font_stack_reaches_every_inherited_css_surface_and_terminal_labels() {
    let font_stack =
        FontStack::new(["SankeyTyped", "sans-serif"]).expect("valid Sankey typed font stack");
    let expected_font = font_stack.as_css().to_string();
    let theme = sankey_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let rendered = render_sankey_with_theme_requirement(
        "sankey-beta\nInput,Transform,10\nTransform,Output,8\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render strict portable Sankey font stack");

    for expected in [
        format!("#sankey-theme{{font-family:{expected_font};"),
        format!("#sankey-theme svg{{font-family:{expected_font};"),
        format!("#sankey-theme .label{{font-family:{expected_font};}}"),
        format!("#sankey-theme .node-labels{{font-family:{expected_font};}}"),
        format!("--mermaid-font-family:{expected_font};"),
    ] {
        assert!(
            rendered.svg().contains(&expected),
            "missing Sankey inherited font writer `{expected}`: {}",
            rendered.svg()
        );
    }
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Sankey SVG");
    let label_group = document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("node-labels"))
        .expect("Sankey node-labels group");
    assert_eq!(label_group.attribute("font-size"), Some("14"));
    assert_eq!(
        label_group
            .children()
            .filter(|node| node.has_tag_name("text"))
            .count(),
        3
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn sankey_typed_font_stack_covers_both_outlined_label_passes() {
    let theme = sankey_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("SankeyOutlined").expect("valid outlined Sankey font stack"),
    ));
    let source = concat!(
        "---\n",
        "config:\n",
        "  sankey:\n",
        "    labelStyle: outlined\n",
        "---\n",
        "sankey-beta\n",
        "A,B,10\n",
    );
    let rendered = render_sankey_with_theme_requirement(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render strict portable outlined Sankey font stack");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid outlined Sankey SVG");
    for class_name in ["sankey-label-bg", "sankey-label-fg"] {
        assert_eq!(
            document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("text") && node.attribute("class") == Some(class_name)
                })
                .count(),
            2,
            "class={class_name}"
        );
    }
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn sankey_explicit_font_family_owners_outrank_the_typed_stack() {
    let theme =
        sankey_typography_theme(ThemeTextStyle::default().with_font_stack(
            FontStack::single("SankeyTyped").expect("valid Sankey typed font stack"),
        ));
    for (owner, source, site_config, expected, excluded) in [
        (
            "site-root",
            "sankey-beta\nA,B,10\n",
            Some(serde_json::json!({ "fontFamily": "SankeySiteRoot" })),
            "SankeySiteRoot",
            None,
        ),
        (
            "site-theme-variables",
            "sankey-beta\nA,B,10\n",
            Some(serde_json::json!({
                "fontFamily": "SankeySiteRoot",
                "themeVariables": { "fontFamily": "SankeySiteTheme" }
            })),
            "SankeySiteTheme",
            Some("SankeySiteRoot"),
        ),
        (
            "source-root",
            "%%{init: {\"fontFamily\": \"SankeySourceRoot\"}}%%\nsankey-beta\nA,B,10\n",
            Some(serde_json::json!({ "secure": [] })),
            "SankeySourceRoot",
            None,
        ),
        (
            "source-theme-variables",
            "%%{init: {\"fontFamily\": \"SankeySourceRoot\", \"themeVariables\": {\"fontFamily\": \"SankeySourceTheme\"}}}%%\nsankey-beta\nA,B,10\n",
            Some(serde_json::json!({ "secure": [] })),
            "SankeySourceTheme",
            Some("SankeySourceRoot"),
        ),
    ] {
        let engine = match site_config {
            Some(config) => MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            ),
            None => MermaidEngine::new(),
        };
        let rendered = render_sankey_with_theme_requirement(
            source,
            &theme,
            engine,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("render config-owned Sankey font stack");

        assert!(
            rendered.svg().contains(&format!("font-family:{expected};")),
            "owner={owner}, svg={}",
            rendered.svg()
        );
        if let Some(excluded) = excluded {
            assert!(!rendered.svg().contains(excluded), "owner={owner}");
        }
        assert!(!rendered.svg().contains("SankeyTyped"), "owner={owner}");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "owner={owner}");
        assert_eq!(evidence.applied_count(), 0, "owner={owner}");
        assert_eq!(evidence.not_applicable_count(), 1, "owner={owner}");
        assert_eq!(evidence.theme_residual_count(), 0, "owner={owner}");
    }
}

#[test]
fn sankey_direct_font_stack_is_not_bounded_by_the_retired_bridge_limit() {
    let font_stack =
        FontStack::new((0..32).map(|index| format!("sankey-font-{index}-{}", "x".repeat(180))))
            .expect("valid oversized direct Sankey font stack");
    let expected_font = font_stack.as_css();
    assert!(expected_font.len() > 4 * 1024);
    let theme = sankey_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let rendered = render_sankey_with_theme_requirement(
        "sankey-beta\nA,B,10\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render oversized direct Sankey font stack");

    assert!(rendered.svg().contains(&expected_font));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn sankey_font_size_is_unsupported_without_recreating_the_legacy_projection() {
    let theme = sankey_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid unsupported Sankey font size"),
    );
    let source = "sankey-beta\nA,B,10\n";
    let rendered = render_sankey_with_theme_requirement(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort renders the structured Sankey font-size residual");

    assert!(!rendered.svg().contains("font-size:24px"));
    assert!(
        rendered
            .svg()
            .contains(r#"class="node-labels" font-size="14""#)
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);

    let error = render_sankey_with_theme_requirement(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .err()
    .expect("RequirePortable rejects unsupported Sankey font size");
    assert!(matches!(
        error,
        merman_render::Error::UnverifiedFamilyTheme {
            family_id: DiagramFamilyId::SANKEY,
            residual_count: 1,
        }
    ));
}

#[test]
fn sankey_mixed_font_stack_and_size_fails_closed() {
    let theme = sankey_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(
                FontStack::single("SankeyMixed").expect("valid Sankey mixed font stack"),
            )
            .with_font_size_px(24.0)
            .expect("valid unsupported Sankey mixed font size"),
    );
    let rendered = render_sankey_with_theme_requirement(
        "sankey-beta\nA,B,10\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort renders mixed Sankey typography as a residual");

    assert!(rendered.svg().contains("SankeyMixed"));
    assert!(!rendered.svg().contains("font-size:24px"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn sankey_explicit_node_colors_outrank_typed_palette_per_node() {
    let theme = sankey_node_palette_theme(&["#123456", "#abcdef"]);
    let cases = [
        (
            "site",
            "sankey-beta\nA,B,10\n",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "sankey": { "nodeColors": { "A": "#fedcba" } }
                    }),
                ),
            ),
        ),
        (
            "source",
            concat!(
                "---\n",
                "config:\n",
                "  sankey:\n",
                "    nodeColors:\n",
                "      A: '#fedcba'\n",
                "---\n",
                "sankey-beta\n",
                "A,B,10\n",
            ),
            MermaidEngine::new(),
        ),
    ];

    for (owner, source, engine) in cases {
        let rendered = render_sankey_with_theme_and_engine(source, &theme, engine);
        assert_eq!(
            sankey_node_fills(rendered.svg()),
            vec!["#fedcba", "#abcdef"],
            "{owner} nodeColors ownership"
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{owner}");
        assert_eq!(evidence.applied_count(), 1, "{owner}");
        assert_eq!(evidence.not_applicable_count(), 0, "{owner}");
        assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
    }
}

#[test]
fn sankey_node_palette_is_not_applicable_when_every_visible_node_is_owned() {
    let theme = sankey_node_palette_theme(&["#123456", "#abcdef"]);
    let rendered = render_sankey_with_theme_and_engine(
        "sankey-beta\nA,B,10\n",
        &theme,
        MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "sankey": {
                    "nodeColors": {
                        "A": "#fedcba",
                        "B": "#654321"
                    }
                }
            })),
        ),
    );

    assert_eq!(
        sankey_node_fills(rendered.svg()),
        vec!["#fedcba", "#654321"]
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn sankey_link_source_and_target_colors_are_derived_without_edge_evidence() {
    let theme = sankey_node_palette_theme(&["#123456", "#abcdef"]);
    for (link_color, expected_stroke) in [("source", "#123456"), ("target", "#abcdef")] {
        let source = format!(
            "---\nconfig:\n  sankey:\n    linkColor: {link_color}\n---\nsankey-beta\nA,B,10\n"
        );
        let rendered = render_sankey_with_theme_and_engine(&source, &theme, MermaidEngine::new());
        let document =
            roxmltree::Document::parse(rendered.svg()).expect("valid derived-link Sankey SVG");
        let link_path = document
            .descendants()
            .find(|node| {
                node.has_tag_name("path")
                    && node
                        .parent()
                        .is_some_and(|parent| parent.attribute("class") == Some("link"))
            })
            .expect("Sankey link path");
        assert_eq!(link_path.attribute("stroke"), Some(expected_stroke));

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{link_color}");
        assert_eq!(evidence.applied_count(), 1, "{link_color}");
        assert_eq!(evidence.not_applicable_count(), 0, "{link_color}");
        assert_eq!(evidence.theme_residual_count(), 0, "{link_color}");
    }
}

#[test]
fn sankey_text_evidence_accounts_for_each_winning_facet() {
    use merman_render::diagram_theme::{OrdinalSelector, Specified};

    for split in [false, true] {
        for font_sibling in [false, true] {
            for config_owner in [false, true] {
                for outlined in [false, true] {
                    let fill = ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#123456").unwrap());
                    let mut sibling = ThemeStylePatch::default();
                    if font_sibling {
                        sibling.typography.font_size_px = Specified::Value(40.0);
                    } else {
                        sibling = sibling.with_stroke(CanvasPaint::solid("#654321").unwrap());
                    }
                    let rules = if split {
                        ThemeRuleSet::default()
                            .with_rule(ThemeRule::new(ThemeTarget::Text, fill))
                            .with_rule(ThemeRule::new(ThemeTarget::Text, sibling))
                    } else {
                        sibling.paint.fill = fill.paint.fill;
                        ThemeRuleSet::default()
                            .with_rule(ThemeRule::new(ThemeTarget::Text, sibling))
                    };
                    let theme = DiagramThemeCompiler::new()
                        .compile(DiagramThemeSpec::new().with_styles(rules))
                        .unwrap();
                    let mut config = serde_json::json!({"sankey": {"showValues": false, "labelStyle": if outlined { "outlined" } else { "legacy" }}});
                    if config_owner {
                        config["themeVariables"] = serde_json::json!({"textColor":"#445566"});
                    }
                    let engine = MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    );
                    let rendered = render_sankey_with_theme_requirement(
                        "sankey-beta\nA,B,10\n",
                        &theme,
                        engine.clone(),
                        ThemePortabilityRequirement::BestEffort,
                    )
                    .unwrap();
                    let document = roxmltree::Document::parse(rendered.svg())
                        .expect("valid mixed-facet Sankey SVG");
                    for class in ["sankey-label-bg", "sankey-label-fg"] {
                        assert_eq!(
                            document
                                .descendants()
                                .filter(|node| node.has_tag_name("text")
                                    && node.attribute("class") == Some(class))
                                .count(),
                            if outlined { 2 } else { 0 },
                            "outlined={outlined}, terminal pass {class}"
                        );
                    }
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(
                        evidence.theme_residual_count(),
                        1,
                        "split={split}, font={font_sibling}, config={config_owner}"
                    );
                    assert_eq!(
                        evidence.applied_count(),
                        usize::from(split && !config_owner)
                    );
                    assert!(
                        render_sankey_with_theme_requirement(
                            "sankey-beta\nA,B,10\n",
                            &theme,
                            engine,
                            ThemePortabilityRequirement::RequirePortable
                        )
                        .is_err()
                    );
                }
            }
        }
    }

    // Only the second label wins this unsupported sibling; it must not be missed by a static probe.
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    ))
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#654321").unwrap()),
                        )
                        .with_ordinal(OrdinalSelector::Exact(2)),
                    ),
            ),
        )
        .unwrap();
    let rendered = render_sankey_with_theme_requirement(
        "sankey-beta\nA,B,10\n",
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn sankey_explicit_node_color_does_not_consume_the_default_palette_domain() {
    for link_color in ["gradient", "target"] {
        let source = format!(
            "---\nconfig:\n  sankey:\n    nodeColors:\n      A: '#112233'\n    linkColor: {link_color}\n---\nsankey-beta\nA,B,10\n"
        );
        let svg = render_sankey(&source, &SvgRenderOptions::default());
        assert_eq!(sankey_node_fills(&svg), ["#112233", "#4e79a7"]);
        let document = roxmltree::Document::parse(&svg).unwrap();
        if link_color == "gradient" {
            let colors = document
                .descendants()
                .filter(|node| node.has_tag_name("stop"))
                .map(|node| node.attribute("stop-color").unwrap())
                .collect::<Vec<_>>();
            assert_eq!(colors, ["#112233", "#4e79a7"]);
        } else {
            assert!(document.descendants().any(
                |node| node.has_tag_name("path") && node.attribute("stroke") == Some("#4e79a7")
            ));
        }
    }
}

#[test]
fn sankey_title_requests_are_not_applicable_even_with_frontmatter_title() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Title,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            ))),
        )
        .unwrap();
    let baseline = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    for source in [
        "sankey-beta\nA,B,10\n",
        "---\ntitle: Metadata title\n---\nsankey-beta\nA,B,10\n",
    ] {
        let rendered = render_sankey_with_theme_and_engine(source, &theme, MermaidEngine::new());
        let unthemed = render_sankey_with_theme_and_engine(source, &baseline, MermaidEngine::new());
        assert_eq!(rendered.svg(), unthemed.svg());
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn sankey_scalar_node_rules_respect_selectors_and_fill_only_source_ownership() {
    use merman_render::diagram_theme::{OrdinalSelector, ThemeVariant};

    let stroke = ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#654321").unwrap());
    let styles = ThemeRuleSet::default()
        .with_rule(ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        ))
        .with_rule(ThemeRule::new(ThemeTarget::Node, stroke.clone()))
        .with_rule(ThemeRule::new(ThemeTarget::Node, stroke.clone()))
        .with_rule(
            ThemeRule::new(ThemeTarget::Node, stroke.clone())
                .with_ordinal(OrdinalSelector::Exact(3)),
        )
        .with_rule(ThemeRule::new(ThemeTarget::Node, stroke).with_variant(ThemeVariant::Warning));
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .unwrap();
    let baseline = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    for (source, owns_fill) in [
        ("sankey-beta\nA,B,10\n", false),
        (
            "---\nconfig:\n  sankey:\n    nodeColors:\n      A: '#112233'\n---\nsankey-beta\nA,B,10\n",
            false,
        ),
        (
            "---\nconfig:\n  sankey:\n    nodeColors:\n      A: '#112233'\n      B: '#445566'\n---\nsankey-beta\nA,B,10\n",
            true,
        ),
    ] {
        let rendered = render_sankey_with_theme_requirement(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let unthemed = render_sankey_with_theme_and_engine(source, &baseline, MermaidEngine::new());
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for rect in document
            .descendants()
            .filter(|node| node.has_tag_name("rect"))
        {
            assert_eq!(rect.attribute("stroke"), Some("#654321"));
        }
        assert_eq!(
            sankey_node_fills(rendered.svg()),
            sankey_node_fills(unthemed.svg())
                .into_iter()
                .map(|fill| if matches!(fill.as_str(), "#112233" | "#445566") {
                    fill
                } else {
                    "#123456".to_owned()
                })
                .collect::<Vec<_>>()
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 5);
        assert_eq!(evidence.applied_count(), if owns_fill { 1 } else { 2 });
        assert_eq!(
            evidence.not_applicable_count(),
            if owns_fill { 4 } else { 3 }
        );
        assert_eq!(evidence.theme_residual_count(), 0);
        assert!(
            render_sankey_with_theme_requirement(
                source,
                &theme,
                MermaidEngine::new(),
                ThemePortabilityRequirement::RequirePortable,
            )
            .is_ok()
        );
    }
}

#[test]
fn sankey_presets_account_for_absent_title_and_real_node_stroke() {
    for descriptor in merman_render::diagram_theme::theme_preset_descriptors() {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(descriptor.preset())
            .unwrap();
        let rendered = render_sankey_with_theme_requirement(
            "sankey-beta\nA,B,10\n",
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(
            evidence.accounted_count(),
            evidence.required_count(),
            "{}",
            descriptor.id()
        );
        assert!(evidence.applied_count() > 0, "{}", descriptor.id());
    }
}

#[test]
fn sankey_static_node_stroke_supports_default_solid_transparent_and_clear() {
    use merman_render::diagram_theme::{Specified, ThemeVariant};
    for (paint, expected) in [
        (
            Specified::Value(CanvasPaint::solid("#123456").unwrap()),
            "#123456",
        ),
        (Specified::Value(CanvasPaint::Transparent), "transparent"),
        (Specified::Clear, "none"),
    ] {
        for variant in [None, Some(ThemeVariant::Default)] {
            let mut patch = ThemeStylePatch::default();
            patch.stroke.paint = paint.clone();
            let mut rule = ThemeRule::new(ThemeTarget::Node, patch);
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)),
                )
                .unwrap();
            let source = "---\nconfig:\n  sankey:\n    nodeColors:\n      A: '#445566'\n---\nsankey-beta\nA,B,10\n";
            let rendered =
                render_sankey_with_theme_and_engine(source, &theme, MermaidEngine::new());
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let strokes = document
                .descendants()
                .filter(|node| node.has_tag_name("rect"))
                .map(|node| node.attribute("stroke").unwrap())
                .collect::<Vec<_>>();
            assert_eq!(strokes, [expected, expected]);
            if expected == "#123456" {
                let view_box = document
                    .root_element()
                    .attribute("viewBox")
                    .unwrap()
                    .split_whitespace()
                    .map(|value| value.parse::<f64>().unwrap())
                    .collect::<Vec<_>>();
                assert!(
                    view_box[1] <= -0.5,
                    "visible node strokes need their full top outset"
                );
                assert!(
                    view_box[3] >= 401.0,
                    "visible node strokes need their full bottom outset"
                );
            }
            assert_eq!(sankey_node_fills(rendered.svg()), ["#445566", "#4e79a7"]);
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.not_applicable_count(), 0);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn sankey_node_stroke_does_not_certify_unsupported_geometry_in_the_same_rule() {
    use merman_render::diagram_theme::OrdinalSelector;

    for ordinal in [
        None,
        Some(OrdinalSelector::Exact(1)),
        Some(OrdinalSelector::cycle(6, 0).unwrap()),
    ] {
        let mut rule = ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#123456").unwrap())
                .with_stroke_width(9.0)
                .unwrap(),
        );
        if let Some(ordinal) = ordinal {
            rule = rule.with_ordinal(ordinal);
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap();
        let source = "sankey-beta\nA,B,10\n";
        let rendered = render_sankey_with_theme_requirement(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert!(rendered.svg().contains(r##"stroke="#123456""##));
        assert!(!rendered.svg().contains(r#"stroke-width="9""#));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
        assert!(
            render_sankey_with_theme_requirement(
                source,
                &theme,
                MermaidEngine::new(),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_err()
        );
    }
}

#[test]
fn sankey_mixed_node_rule_retains_stroke_under_source_owned_fill() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#123456").unwrap())
                        .with_stroke(CanvasPaint::solid("#abcdef").unwrap()),
                )),
            ),
        )
        .unwrap();
    for colors in [
        serde_json::json!({}),
        serde_json::json!({"A":"#445566"}),
        serde_json::json!({"A":"#445566", "B":"#778899"}),
    ] {
        let expected_fill =
            ["A", "B"].map(|id| colors[id].as_str().unwrap_or("#123456").to_owned());
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                serde_json::json!({"sankey":{"nodeColors":colors}}),
            ),
        );
        let rendered = render_sankey_with_theme_requirement(
            "sankey-beta\nA,B,10\n",
            &theme,
            engine,
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(
                    |node| node.has_tag_name("rect") && node.attribute("stroke") == Some("#abcdef")
                )
                .count(),
            2
        );
        assert_eq!(sankey_node_fills(rendered.svg()), expected_fill);
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 0);
    }
}

#[test]
fn sankey_static_node_fill_preserves_source_and_drives_final_link_colors() {
    use merman_render::diagram_theme::{Specified, ThemeVariant};
    for variant in [None, Some(ThemeVariant::Default)] {
        for (fill, expected) in [
            (
                Specified::Value(CanvasPaint::solid("#123456").unwrap()),
                "#123456",
            ),
            (Specified::Value(CanvasPaint::Transparent), "transparent"),
            (Specified::Clear, "#4e79a7"),
        ] {
            let mut patch = ThemeStylePatch::default();
            patch.paint.fill = fill;
            let mut rule = ThemeRule::new(ThemeTarget::Node, patch);
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default()
                            .with_rule(rule)
                            .with_ordinal_palette(
                                ThemeTarget::Node,
                                sankey_node_palette(&["#abcdef"]),
                            ),
                    ),
                )
                .unwrap();
            for mode in ["gradient", "source", "target"] {
                let source = format!(
                    "---\nconfig:\n  sankey:\n    nodeColors:\n      A: '#112233'\n    linkColor: {mode}\n---\nsankey-beta\nA,B,10\n"
                );
                let rendered =
                    render_sankey_with_theme_and_engine(&source, &theme, MermaidEngine::new());
                assert_eq!(sankey_node_fills(rendered.svg()), ["#112233", expected]);
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                if mode == "gradient" {
                    let stops = document
                        .descendants()
                        .filter(|node| node.has_tag_name("stop"))
                        .map(|node| node.attribute("stop-color").unwrap())
                        .collect::<Vec<_>>();
                    assert_eq!(stops, ["#112233", expected]);
                } else {
                    let stroke = if mode == "source" {
                        "#112233"
                    } else {
                        expected
                    };
                    assert!(document.descendants().any(|node| node.has_tag_name("path")
                        && node.attribute("stroke") == Some(stroke)));
                }
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 2);
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.not_applicable_count(), 1);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn sankey_text_palette_accounts_for_real_labels_and_fill_owners() {
    for typed_fill in [false, true] {
        let mut styles = ThemeRuleSet::default().with_ordinal_palette(
            ThemeTarget::Text,
            OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap(),
        );
        if typed_fill {
            styles = styles.with_rule(ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap()),
            ));
        }
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .unwrap();
        for source_fill in [false, true] {
            let engine = if source_fill {
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({"themeVariables":{"textColor":"#445566"}}),
                    ),
                )
            } else {
                MermaidEngine::new()
            };
            let rendered = render_sankey_with_theme_requirement(
                "sankey-beta\nA,B,10\n",
                &theme,
                engine,
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1 + usize::from(typed_fill));
            assert_eq!(evidence.accounted_count(), evidence.required_count());
            assert_eq!(
                evidence.applied_count(),
                usize::from(typed_fill && !source_fill)
            );
            assert_eq!(
                evidence.theme_residual_count(),
                usize::from(!typed_fill && !source_fill)
            );
            assert_eq!(
                evidence.not_applicable_count(),
                if source_fill {
                    evidence.required_count()
                } else {
                    usize::from(typed_fill)
                }
            );
        }
    }
}

#[test]
fn sankey_effect_bindings_keep_their_domains_independent_of_source_fill() {
    use merman_render::diagram_theme::{
        DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive,
    };
    for target in [ThemeTarget::Node, ThemeTarget::Text, ThemeTarget::Title] {
        let effects = DiagramEffectSet::default()
            .with_graph(
                EffectGraph::new(
                    "blur",
                    [EffectPrimitive::GaussianBlur {
                        input: EffectInput::SourceGraphic,
                        std_deviation: 1.0,
                    }],
                )
                .unwrap(),
            )
            .unwrap()
            .with_binding(EffectBinding::new(target, "blur").unwrap())
            .unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_effects(effects))
            .unwrap();
        let rendered = render_sankey_with_theme_requirement(
            "---\nconfig:\n  themeVariables:\n    textColor: '#445566'\n  sankey:\n    nodeColors:\n      A: '#112233'\n      B: '#778899'\n---\nsankey-beta\nA,B,10\n",
            &theme, MermaidEngine::new(), ThemePortabilityRequirement::BestEffort,
        ).unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(
            evidence.theme_residual_count(),
            usize::from(target != ThemeTarget::Title)
        );
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(target == ThemeTarget::Title)
        );
    }
}

#[allow(dead_code)]
mod common;

#[test]
fn sankey_decoded_empty_labels_do_not_consume_text_themes_or_merge_node_identities() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(
                        DiagramFamilyId::SANKEY,
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Sankey Visibility").unwrap())
                            .with_font_size_px(24.0)
                            .unwrap(),
                    ),
                )
                .with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap()),
                        ))
                        .with_ordinal_palette(
                            ThemeTarget::Node,
                            sankey_node_palette(&["#abcdef", "#fedcba"]),
                        ),
                ),
        )
        .unwrap();
    for label_style in ["legacy", "outlined"] {
        // Explicit renderer-only IDs preserve placeholder decoding and XML filtering.
        for (first, second, decoded) in [
            ("ﬂ°°32¶ß", "ﬂ°°9¶ß", [" ", "\t"]),
            ("ﬂ°°1¶ß", "ﬂ°°2¶ß", ["", ""]),
        ] {
            for show_values in [false, true] {
                let source = format!(
                    "---\nconfig:\n  sankey:\n    labelStyle: {label_style}\n    showValues: {show_values}\n---\nsankey\nSource,Target,1\n"
                );
                let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                let parsed = amend_sankey_node_ids(parsed, [first, second]);
                let merman_model::presentation::RenderSemanticModel::Sankey(model) = parsed.model()
                else {
                    panic!("expected Sankey model");
                };
                let ids = model
                    .graph
                    .nodes
                    .iter()
                    .map(|node| node.id.clone())
                    .collect::<Vec<_>>();
                assert_eq!(ids.len(), 2);
                assert_ne!(ids[0], ids[1]);
                let session = RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(if show_values {
                        ThemePortabilityRequirement::BestEffort
                    } else {
                        ThemePortabilityRequirement::RequirePortable
                    })
                    .begin_session_with_theme(&theme)
                    .unwrap();
                let artifact =
                    family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
                let projection = artifact.layout_json().unwrap();
                let layout = &projection["layout"]["SankeyDiagram"];
                for (index, id) in ids.iter().enumerate() {
                    assert_eq!(layout["nodes"][index]["id"].as_str(), Some(id.as_str()));
                }
                assert_eq!(layout["links"][0]["source"].as_str(), Some(ids[0].as_str()));
                assert_eq!(layout["links"][0]["target"].as_str(), Some(ids[1].as_str()));
                let rendered = artifact
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap();
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let labels = document
                    .descendants()
                    .filter(|node| node.has_tag_name("text"))
                    .map(|node| node.text().unwrap_or_default().to_owned())
                    .collect::<Vec<_>>();
                let expected = decoded.map(|text| {
                    if show_values {
                        format!("{text}\n1")
                    } else {
                        text.to_owned()
                    }
                });
                assert_eq!(labels.len(), if label_style == "outlined" { 4 } else { 2 });
                for (index, label) in labels.iter().enumerate() {
                    assert_eq!(label, &expected[index % 2]);
                }
                assert_eq!(sankey_node_fills(rendered.svg()), ["#abcdef", "#fedcba"]);
                assert_eq!(
                    document
                        .descendants()
                        .filter(|node| node.has_tag_name("path"))
                        .count(),
                    1,
                    "distinct raw IDs retain their connecting edge"
                );
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 4);
                assert_eq!(evidence.applied_count(), if show_values { 3 } else { 1 });
                assert_eq!(
                    evidence.not_applicable_count(),
                    if show_values { 0 } else { 3 }
                );
                assert_eq!(evidence.theme_residual_count(), usize::from(show_values));
                assert_eq!(evidence.accounted_count(), 4);
            }
        }
    }
}

#[test]
fn sankey_text_ordinals_use_visible_labels_without_renumbering_nodes() {
    use merman_render::diagram_theme::OrdinalSelector;

    for label_style in ["legacy", "outlined"] {
        for (first_id, render_ids, show_values, ordinal, applicable, first_visible) in [
            (" ", None, false, 1, false, false),
            (" ", None, false, 2, true, false),
            (" ", None, true, 1, true, true),
            ("A", None, false, 1, true, true),
            (" ", None, false, 3, false, false),
            ("Source", Some(["ﬂ°°32¶ß", "B"]), false, 1, false, false),
            ("Source", Some(["ﬂ°°32¶ß", "B"]), false, 2, true, false),
            ("Source", Some(["ﬂ°°1¶ß", "B"]), false, 1, false, false),
            ("Source", Some(["ﬂ°°32¶ß", "B"]), true, 1, true, true),
        ] {
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default().with_rule(
                            ThemeRule::new(
                                ThemeTarget::Text,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#123456").unwrap()),
                            )
                            .with_ordinal(OrdinalSelector::Exact(ordinal)),
                        ),
                    ),
                )
                .unwrap();
            let source = format!(
                "---\nconfig:\n  sankey:\n    showValues: {show_values}\n    labelStyle: {label_style}\n---\nsankey-beta\n\"{first_id}\",B,10\n"
            );
            let portability = if applicable {
                ThemePortabilityRequirement::BestEffort
            } else {
                ThemePortabilityRequirement::RequirePortable
            };
            let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .expect("parse valid Sankey label seed")
                .expect("detect Sankey label seed")
                .into_presentation();
            let parsed = if let Some(ids) = render_ids {
                amend_sankey_node_ids(parsed, ids)
            } else {
                parsed
            };
            let rendered =
                render_sankey_presentation_with_theme_requirement(&parsed, &theme, portability)
                    .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let visible = document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .map(|node| node.text().is_some_and(|text| !text.trim().is_empty()))
                .collect::<Vec<_>>();
            assert_eq!(
                visible,
                [first_visible, true].repeat(if label_style == "outlined" { 2 } else { 1 }),
                "{source}"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(!applicable),
                "{source}, ordinal={ordinal}"
            );
            assert_eq!(
                evidence.theme_residual_count(),
                usize::from(applicable),
                "{source}, ordinal={ordinal}"
            );
        }
    }
}

#[test]
fn sankey_visible_text_ordinals_keep_fill_and_stroke_ownership_independent() {
    use merman_render::diagram_theme::OrdinalSelector;

    for source_owns_fill in [false, true] {
        for ordinal in [1, 2] {
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default()
                            .with_rule(ThemeRule::new(
                                ThemeTarget::Text,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#123456").unwrap()),
                            ))
                            .with_rule(
                                ThemeRule::new(
                                    ThemeTarget::Text,
                                    ThemeStylePatch::default()
                                        .with_stroke(CanvasPaint::solid("#654321").unwrap()),
                                )
                                .with_ordinal(OrdinalSelector::Exact(ordinal)),
                            ),
                    ),
                )
                .unwrap();
            let engine = if source_owns_fill {
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(
                        serde_json::json!({"themeVariables":{"textColor":"#445566"}}),
                    ),
                )
            } else {
                MermaidEngine::new()
            };
            let rendered = render_sankey_with_theme_requirement(
                "---\nconfig:\n  sankey:\n    showValues: false\n---\nsankey-beta\n\" \",B,10\n",
                &theme,
                engine,
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let label = document
                .descendants()
                .find(|node| node.has_tag_name("text") && node.text() == Some("B"))
                .unwrap();
            assert_eq!(
                label.attribute("fill"),
                (!source_owns_fill).then_some("#123456")
            );
            assert!(label.attribute("stroke").is_none());
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.applied_count(), usize::from(!source_owns_fill));
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(source_owns_fill) + usize::from(ordinal == 1)
            );
            assert_eq!(evidence.theme_residual_count(), usize::from(ordinal == 2));
        }
    }
}

fn assert_sankey_compositing(svg: &str, blend_mode: &str, layers: [&str; 3]) {
    let document = roxmltree::Document::parse(svg).expect("valid Sankey SVG");
    let actual_layers = document
        .descendants()
        .filter_map(|node| node.attribute("class"))
        .filter(|class| matches!(*class, "links" | "nodes" | "node-labels"))
        .collect::<Vec<_>>();
    assert_eq!(actual_layers, layers);
    let link_groups = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("link"))
        .collect::<Vec<_>>();
    assert!(!link_groups.is_empty());
    let expected_inline = format!("mix-blend-mode: {blend_mode};");
    for link in link_groups {
        assert_eq!(link.attribute("style"), Some(expected_inline.as_str()));
    }
    assert!(sankey_style_text(svg).contains(&format!(
        ".link{{fill:none;stroke-opacity:0.5;mix-blend-mode:{blend_mode};}}"
    )));
}

#[test]
fn sankey_without_compiled_theme_keeps_mermaid_compositing_and_order() {
    for mermaid_theme in ["default", "dark"] {
        for label_style in ["legacy", "outlined"] {
            let source = format!(
                "---\nconfig:\n  theme: {mermaid_theme}\n  sankey:\n    labelStyle: {label_style}\n---\nsankey-beta\nA,B,10\nB,C,7\n"
            );
            let svg = render_sankey(&source, &SvgRenderOptions::default());
            assert_sankey_compositing(&svg, "multiply", ["nodes", "node-labels", "links"]);
            for id in [
                "node-1",
                "node-2",
                "node-3",
                "linearGradient-4",
                "linearGradient-5",
            ] {
                assert!(svg.contains(&format!("id=\"{id}\"")));
            }
        }
    }
}

#[test]
fn sankey_compiled_palettes_keep_normal_flows_beneath_nodes_and_labels() {
    for (canvas, text, colors) in [
        ("#F5F7FC", "#1C2636", ["#2864DC", "#B95D12", "#137F75"]),
        ("#101722", "#E8EFF7", ["#77ABFF", "#F7AF6A", "#58CFBA"]),
    ] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_canvas(CanvasSpec::solid(canvas).unwrap())
                    .with_styles(
                        ThemeRuleSet::default()
                            .with_rule(ThemeRule::new(
                                ThemeTarget::Text,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid(text).unwrap()),
                            ))
                            .with_ordinal_palette(ThemeTarget::Node, sankey_node_palette(&colors)),
                    ),
            )
            .unwrap();
        for label_style in ["legacy", "outlined"] {
            for link_color in ["source", "target", "gradient", "#8B5CF6"] {
                let source = format!(
                    "---\nconfig:\n  sankey:\n    labelStyle: {label_style}\n    linkColor: '{link_color}'\n    nodeColors:\n      A: '#112233'\n---\nsankey-beta\nA,B,10\nB,C,7\n"
                );
                let rendered =
                    render_sankey_with_theme_and_engine(&source, &theme, MermaidEngine::new());
                assert_sankey_compositing(
                    rendered.svg(),
                    "normal",
                    ["links", "nodes", "node-labels"],
                );
                assert_eq!(
                    sankey_node_fills(rendered.svg()),
                    ["#112233", colors[1], colors[2]]
                );

                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let strokes = document
                    .descendants()
                    .filter(|node| node.has_tag_name("path"))
                    .map(|node| node.attribute("stroke").unwrap())
                    .collect::<Vec<_>>();
                let expected_strokes = match link_color {
                    "source" => ["#112233", colors[1]],
                    "target" => [colors[1], colors[2]],
                    "gradient" => [
                        "url(#sankey-theme-linearGradient-4)",
                        "url(#sankey-theme-linearGradient-5)",
                    ],
                    _ => [link_color; 2],
                };
                assert_eq!(strokes, expected_strokes);
                let stops = document
                    .descendants()
                    .filter(|node| node.has_tag_name("stop"))
                    .map(|node| node.attribute("stop-color").unwrap())
                    .collect::<Vec<_>>();
                if link_color == "gradient" {
                    assert_eq!(stops, ["#112233", colors[1], colors[1], colors[2]]);
                    for id in [
                        "sankey-theme-linearGradient-4",
                        "sankey-theme-linearGradient-5",
                    ] {
                        assert!(document.descendants().any(|node| {
                            node.has_tag_name("linearGradient") && node.attribute("id") == Some(id)
                        }));
                    }
                } else {
                    assert!(stops.is_empty());
                }
                let ids = document
                    .descendants()
                    .filter_map(|node| node.attribute("id"))
                    .collect::<Vec<_>>();
                let unique_ids = ids.iter().collect::<std::collections::BTreeSet<_>>();
                assert_eq!(ids.len(), unique_ids.len());
                for id in [
                    "sankey-theme-node-1",
                    "sankey-theme-node-2",
                    "sankey-theme-node-3",
                ] {
                    assert!(ids.contains(&id));
                }
                let labels = document
                    .descendants()
                    .filter(|node| node.has_tag_name("text"))
                    .collect::<Vec<_>>();
                assert_eq!(labels.len(), if label_style == "outlined" { 6 } else { 3 });
                assert!(
                    labels
                        .iter()
                        .all(|label| label.attribute("fill") == Some(text))
                );
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 2);
                assert_eq!(evidence.accounted_count(), 2);
                assert_eq!(evidence.applied_count(), 2);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn sankey_canvas_only_compiled_theme_uses_normal_compositing_without_family_claims() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_canvas(CanvasSpec::solid("#101722").unwrap()))
        .unwrap();
    let rendered =
        render_sankey_with_theme_and_engine("sankey-beta\nA,B,10\n", &theme, MermaidEngine::new());
    assert_sankey_compositing(rendered.svg(), "normal", ["links", "nodes", "node-labels"]);
    assert_eq!(sankey_node_fills(rendered.svg()), ["#4e79a7", "#f28e2c"]);
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 0);
    assert_eq!(evidence.accounted_count(), 0);
}

#[test]
fn sankey_compiled_compositing_preserves_exact_svg_byte_limits_and_terminal_evidence() {
    let theme = sankey_node_palette_theme(&["#77ABFF", "#F7AF6A", "#58CFBA"]);
    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync("sankey-beta\nA,B,10\nB,C,7\n", ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let render = |policy| {
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap();
        family::prepare(&parsed, &LayoutOptions::default(), session)?
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    };
    let baseline = render(RenderResourcePolicy::unbounded_for_trusted_input()).unwrap();
    let exact_bytes = baseline.svg().len();
    let exact = render(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(exact.svg(), baseline.svg());
    assert_sankey_compositing(exact.svg(), "normal", ["links", "nodes", "node-labels"]);
    let evidence = merman_render::__private::family_evidence(exact.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);

    let error = render(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes - 1)
            .unwrap(),
    )
    .err()
    .expect("one byte below the themed Sankey SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected themed Sankey MaxSvgBytes rejection, got {error}");
    };
    assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
    assert_eq!(limit.max, exact_bytes - 1);
    assert!(limit.actual > limit.max);
}

#[test]
fn sankey_empty_compiled_theme_changes_only_compositing_and_layer_order() {
    let source = "sankey-beta\nA,B,10\n";
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let plain = family::prepare(&parsed, &LayoutOptions::default(), session)
        .unwrap()
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("sankey-theme".to_owned()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .unwrap();
    let themed = render_sankey_with_theme_and_engine(source, &theme, MermaidEngine::new());
    let document = roxmltree::Document::parse(plain.svg()).unwrap();
    let links = document
        .descendants()
        .find(|node| node.attribute("class") == Some("links"))
        .unwrap()
        .range();
    let nodes = document
        .descendants()
        .find(|node| node.attribute("class") == Some("nodes"))
        .unwrap()
        .range();
    let mut expected = plain.svg().to_owned();
    let link_markup = expected[links.clone()].to_owned();
    expected.replace_range(links, "");
    expected.insert_str(nodes.start, &link_markup);
    expected = expected
        .replace("mix-blend-mode:multiply;", "mix-blend-mode:normal;")
        .replace("mix-blend-mode: multiply;", "mix-blend-mode: normal;");
    assert_eq!(themed.svg(), expected);
}

#[test]
fn sankey_family_node_cycles_override_surface_palettes_and_drive_final_link_paints() {
    use merman_render::diagram_theme::{OrdinalSelector, ThemeVariant};

    let graph = (0..7)
        .map(|index| format!("Node{index},Node{},10\n", index + 1))
        .collect::<String>();
    for colors in [
        [
            "#2864DC", "#B95D12", "#137F75", "#8455C7", "#BA426F", "#8A7515",
        ],
        [
            "#77ABFF", "#F7AF6A", "#58CFBA", "#B99AF0", "#F18CB3", "#DBCA70",
        ],
    ] {
        let mut styles = ThemeRuleSet::default().with_ordinal_palette(
            ThemeTarget::Node,
            sankey_node_palette(&["#182334", "#243040"]),
        );
        for (offset, color) in colors.iter().enumerate() {
            styles = styles.with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap()),
                )
                .for_family(DiagramFamilyId::SANKEY)
                .with_ordinal(OrdinalSelector::cycle(6, offset).unwrap()),
            );
        }
        styles = styles.with_rule(
            ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#7F95AD").unwrap()),
            )
            .for_family(DiagramFamilyId::SANKEY)
            .with_variant(ThemeVariant::Default)
            .with_ordinal(OrdinalSelector::Exact(1)),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .unwrap();
        let expected_fills = (0..8)
            .map(|index| {
                if index == 0 {
                    "#13579B"
                } else {
                    colors[index % 6]
                }
            })
            .collect::<Vec<_>>();
        for link_color in ["source", "target", "gradient"] {
            let source = format!(
                "---\nconfig:\n  sankey:\n    nodeColors:\n      Node0: '#13579B'\n    linkColor: {link_color}\n---\nsankey-beta\n{graph}"
            );
            let rendered =
                render_sankey_with_theme_and_engine(&source, &theme, MermaidEngine::new());
            assert_sankey_compositing(rendered.svg(), "normal", ["links", "nodes", "node-labels"]);
            assert_eq!(sankey_node_fills(rendered.svg()), expected_fills);
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let strokes = document
                .descendants()
                .filter(|node| node.has_tag_name("rect"))
                .map(|node| node.attribute("stroke"))
                .collect::<Vec<_>>();
            assert_eq!(
                strokes,
                [Some("#7F95AD"), None, None, None, None, None, None, None]
            );
            if link_color == "gradient" {
                let stops = document
                    .descendants()
                    .filter(|node| node.has_tag_name("stop"))
                    .map(|node| node.attribute("stop-color").unwrap())
                    .collect::<Vec<_>>();
                let expected_stops = expected_fills
                    .windows(2)
                    .flat_map(|pair| pair.iter().copied())
                    .collect::<Vec<_>>();
                assert_eq!(stops, expected_stops);
            } else {
                let links = document
                    .descendants()
                    .filter(|node| node.has_tag_name("path"))
                    .map(|node| node.attribute("stroke").unwrap())
                    .collect::<Vec<_>>();
                assert_eq!(
                    links,
                    if link_color == "source" {
                        &expected_fills[..7]
                    } else {
                        &expected_fills[1..]
                    }
                );
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 8);
            assert_eq!(evidence.accounted_count(), 8);
            assert_eq!(evidence.applied_count(), 7);
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn sankey_node_cycle_paints_preserve_clear_transparency_and_source_fill_ownership() {
    use merman_render::diagram_theme::{OrdinalSelector, Specified, ThemeVariant};

    for (paint, fill, stroke) in [
        (
            Specified::Value(CanvasPaint::solid("#77ABFF").unwrap()),
            "#77ABFF",
            "#77ABFF",
        ),
        (
            Specified::Value(CanvasPaint::Transparent),
            "transparent",
            "transparent",
        ),
        (Specified::Clear, "#4e79a7", "none"),
    ] {
        for with_stroke in [false, true] {
            let mut patch = ThemeStylePatch::default();
            patch.paint.fill = paint.clone();
            if with_stroke {
                patch.stroke.paint = paint.clone();
            }
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default()
                            .with_rule(
                                ThemeRule::new(ThemeTarget::Node, patch)
                                    .for_family(DiagramFamilyId::SANKEY)
                                    .with_variant(ThemeVariant::Default)
                                    .with_ordinal(OrdinalSelector::cycle(6, 0).unwrap()),
                            )
                            .with_ordinal_palette(
                                ThemeTarget::Node,
                                sankey_node_palette(&["#182334", "#243040"]),
                            ),
                    ),
                )
                .unwrap();
            for source_owns_fill in [false, true] {
                let engine = if source_owns_fill {
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            serde_json::json!({"sankey": {"nodeColors": {"A": "#13579B"}}}),
                        ),
                    )
                } else {
                    MermaidEngine::new()
                };
                let rendered =
                    render_sankey_with_theme_and_engine("sankey-beta\nA,B,10\n", &theme, engine);
                assert_eq!(
                    sankey_node_fills(rendered.svg()),
                    [if source_owns_fill { "#13579B" } else { fill }, "#243040"]
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let strokes = document
                    .descendants()
                    .filter(|node| node.has_tag_name("rect"))
                    .map(|node| node.attribute("stroke"))
                    .collect::<Vec<_>>();
                assert_eq!(strokes, [with_stroke.then_some(stroke), None]);
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                let rule_applied = with_stroke || !source_owns_fill;
                assert_eq!(evidence.required_count(), 2);
                assert_eq!(evidence.accounted_count(), 2);
                assert_eq!(evidence.applied_count(), 1 + usize::from(rule_applied));
                assert_eq!(evidence.not_applicable_count(), usize::from(!rule_applied));
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn sankey_css_binding_ignores_nonstring_text_color_owners() {
    let theme = sankey_text_fill_theme(CanvasPaint::solid("#c2188b").unwrap());
    for (value, expected, applied) in [
        (None, "#c2188b", 1),
        (Some(serde_json::json!(null)), "#c2188b", 1),
        (Some(serde_json::json!(false)), "#c2188b", 1),
        (Some(serde_json::json!("#125634")), "#125634", 0),
    ] {
        let variables = value.as_ref().map_or_else(
            || serde_json::json!({}),
            |value| serde_json::json!({"textColor": value}),
        );
        let config = serde_json::json!({"themeVariables": variables});
        let source = format!("---\nconfig: {config}\n---\nsankey-beta\nA,B,1\n");
        let rendered = render_sankey_with_theme_requirement(
            &source,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"sankey": {"showValues": false}}),
                ),
            ),
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let stylesheet = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .unwrap();
        assert!(
            stylesheet
                .split_once('}')
                .unwrap()
                .0
                .ends_with(&format!("fill:{expected};")),
            "textColor={value:?}: {stylesheet}"
        );
        let labels = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text")
                    && node.parent().and_then(|parent| parent.attribute("class"))
                        == Some("node-labels")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            labels.iter().map(|node| node.text()).collect::<Vec<_>>(),
            [Some("A"), Some("B")]
        );
        for label in labels {
            assert_eq!(
                label.attribute("fill"),
                (applied == 1).then_some(expected),
                "textColor={value:?}: {label:?}"
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), applied, "textColor={value:?}");
        assert_eq!(evidence.not_applicable_count(), 1 - applied);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}
