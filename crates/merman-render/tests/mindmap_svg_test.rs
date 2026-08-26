#![cfg(feature = "layout-cytoscape")]

use merman_core::{Engine, MermaidConfig, ParseOptions, ParsedDiagramRender};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack,
    MermaidThemeCompatibility, OrdinalPalette, ThemeAdmissionPolicy, ThemeColorValue,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    ThemeTextStyle, TrustedThemeLane, TrustedThemeLanes, TypographySpec,
};
use merman_render::environment::{RenderEnvironment, RenderSession};
use merman_render::family;
use merman_render::model::MindmapDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

fn render_mindmap_svg_from_text(text: &str, diagram_id: &str) -> String {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::headless_svg_defaults();
    let artifact = family::prepare(parsed, &layout_options, session).expect("layout ok");

    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render svg")
        .svg()
        .to_owned()
}

fn mindmap_node_palette_theme(colors: &[&str]) -> DiagramTheme {
    let palette = OrdinalPalette::new(
        colors
            .iter()
            .map(|color| ThemeColorValue::parse(*color).expect("valid Mindmap palette color")),
    )
    .expect("non-empty Mindmap Node palette");
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette),
            ),
        )
        .expect("compile Mindmap Node palette")
}

fn mindmap_edge_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_stroke(stroke),
                    )
                    .for_family(merman_render::DiagramFamilyId::MINDMAP),
                ),
            ),
        )
        .expect("compile Mindmap Edge stroke")
}

fn mindmap_font_stack_theme(font_stack: FontStack) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(
                merman_render::DiagramFamilyId::MINDMAP,
                ThemeTextStyle::default().with_font_stack(font_stack),
            ),
        ))
        .expect("compile Mindmap font stack theme")
}

fn mindmap_font_size_theme(font_size_px: f32) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    merman_render::DiagramFamilyId::MINDMAP,
                    ThemeTextStyle::default()
                        .with_font_size_px(font_size_px)
                        .expect("valid Mindmap font size"),
                ),
            ),
        )
        .expect("compile Mindmap font size theme")
}

fn prepare_mindmap_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::FamilyRenderArtifact {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Mindmap")
        .expect("detect themed Mindmap");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Mindmap session");
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Mindmap")
}

fn render_mindmap_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    diagram_id: &str,
    portability: ThemePortabilityRequirement,
) -> family::RenderedFamilySvg {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Mindmap")
        .expect("detect themed Mindmap");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Mindmap session");
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Mindmap")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Mindmap")
}

fn mindmap_node_attribute(svg: &str, node_dom_id: &str, attribute: &str) -> Option<String> {
    let document = roxmltree::Document::parse(svg).expect("valid Mindmap SVG");
    document
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("id") == Some(node_dom_id))
        .and_then(|node| node.attribute(attribute))
        .map(str::to_string)
}

fn mindmap_edge_attribute(svg: &str, edge_id: &str, attribute: &str) -> Option<String> {
    let document = roxmltree::Document::parse(svg).expect("valid Mindmap SVG");
    document
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("data-edge") == Some("true")
                && node.attribute("data-id") == Some(edge_id)
        })
        .and_then(|node| node.attribute(attribute))
        .map(str::to_string)
}

fn mindmap_stylesheet(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid Mindmap SVG");
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect::<String>()
}

fn mindmap_rule_fill<'a>(
    stylesheet: &'a str,
    prefix: &str,
    terminator: &str,
    expectation: &str,
) -> &'a str {
    stylesheet
        .split_once(prefix)
        .and_then(|(_, remaining)| remaining.split_once(terminator))
        .map(|(fill, _)| fill)
        .unwrap_or_else(|| panic!("{expectation}"))
}

fn mindmap_mermaid_section_shape_fill<'a>(
    stylesheet: &'a str,
    diagram_id: &str,
    section: usize,
) -> &'a str {
    let prefix = format!(
        "#{diagram_id} .section-{section} rect,#{diagram_id} .section-{section} path,#{diagram_id} .section-{section} circle,#{diagram_id} .section-{section} polygon,#{diagram_id} .section-{section} path{{fill:"
    );
    mindmap_rule_fill(stylesheet, &prefix, ";}", "Mindmap section shape fill rule")
}

fn mindmap_typed_section_shape_fill<'a>(
    stylesheet: &'a str,
    diagram_id: &str,
    section: usize,
) -> &'a str {
    let prefix = format!(
        "#{diagram_id} [data-mindmap-section=\"{section}\"] rect,#{diagram_id} [data-mindmap-section=\"{section}\"] path,#{diagram_id} [data-mindmap-section=\"{section}\"] circle,#{diagram_id} [data-mindmap-section=\"{section}\"] polygon{{fill:"
    );
    mindmap_rule_fill(
        stylesheet,
        &prefix,
        ";}",
        "typed Mindmap section shape fill rule",
    )
}

fn mindmap_typed_neo_section_shape_fill<'a>(
    stylesheet: &'a str,
    diagram_id: &str,
    section: usize,
) -> &'a str {
    let prefix = format!(
        "#{diagram_id} [data-look=\"neo\"].mindmap-node[data-mindmap-section=\"{section}\"] rect,#{diagram_id} [data-look=\"neo\"].mindmap-node[data-mindmap-section=\"{section}\"] path,#{diagram_id} [data-look=\"neo\"].mindmap-node[data-mindmap-section=\"{section}\"] circle,#{diagram_id} [data-look=\"neo\"].mindmap-node[data-mindmap-section=\"{section}\"] polygon{{fill:"
    );
    mindmap_rule_fill(
        stylesheet,
        &prefix,
        ";}",
        "typed neo Mindmap section shape fill rule",
    )
}

fn mindmap_neo_section_shape_fill<'a>(
    stylesheet: &'a str,
    diagram_id: &str,
    section: usize,
) -> &'a str {
    let selector = format!(
        "#{diagram_id} [data-look=\"neo\"].mindmap-node.section-{section} rect,#{diagram_id} [data-look=\"neo\"].mindmap-node.section-{section} path,#{diagram_id} [data-look=\"neo\"].mindmap-node.section-{section} circle,#{diagram_id} [data-look=\"neo\"].mindmap-node.section-{section} polygon"
    );
    stylesheet
        .rsplit_once(&selector)
        .and_then(|(_, remaining)| remaining.split_once('}'))
        .and_then(|(declarations, _)| declarations.split_once("fill:"))
        .and_then(|(_, fill)| fill.split_once(';'))
        .map(|(fill, _)| fill)
        .expect("final neo Mindmap section shape fill rule")
}

fn try_render_mindmap_svg_with_resource_policy(
    text: &str,
    diagram_id: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Mindmap resource-bound session");
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Mindmap resource-bound fixture")
        .expect("detect Mindmap resource-bound fixture");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    )?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn mindmap_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = "mindmap\n  Root\n    Child\n";
    let diagram_id = "mindmap-bounded";
    let baseline = try_render_mindmap_svg_with_resource_policy(
        source,
        diagram_id,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Mindmap baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Mindmap fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Mindmap SVG byte ceiling");
    let exact = try_render_mindmap_svg_with_resource_policy(source, diagram_id, exact_policy)
        .expect("the exact Mindmap family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Mindmap SVG byte ceiling");
    let error = try_render_mindmap_svg_with_resource_policy(source, diagram_id, below_policy)
        .expect_err("one byte below the Mindmap family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Mindmap MaxSvgBytes rejection, got {error}");
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

fn layout_mindmap_typed(
    parsed: &ParsedDiagramRender,
    session: RenderSession,
) -> MindmapDiagramLayout {
    let artifact =
        family::prepare(parsed.clone(), &LayoutOptions::default(), session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize Mindmap layout");
    serde_json::from_value(projection["layout"]["MindmapDiagram"].clone())
        .expect("Mindmap layout projection")
}

fn deep_mindmap_chain(depth: usize) -> String {
    let mut input = String::from("---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n");
    for level in 0..depth {
        input.push_str(&" ".repeat(level));
        input.push_str(&format!("n{level}\n"));
    }
    input
}

#[test]
fn mindmap_svg_emits_mermaid_11_15_classic_dom_surface() {
    let svg = render_mindmap_svg_from_text(
        r#"mindmap
  Root
    Child
"#,
        "m15-mindmap",
    );

    assert!(
        svg.contains(r#"id="m15-mindmap-node_0" data-look="classic""#),
        "expected classic Mindmap node DOM id to be diagram-prefixed and expose data-look: {svg}"
    );
    assert!(
        svg.contains(r#"id="m15-mindmap-edge_0_1""#)
            && svg.contains(r#"data-id="edge_0_1""#)
            && svg.contains(r#"data-look="classic""#),
        "expected Mindmap edge DOM id to be diagram-prefixed while data-id keeps the raw edge id: {svg}"
    );
    assert!(
        svg.contains(r#"<span class="nodeLabel markdown-node-label"><p>Root</p></span>"#),
        "expected Mindmap XHTML labels to keep Mermaid 11.15 class ordering: {svg}"
    );
    assert!(
        svg.contains(r#"id="m15-mindmap_mindmap-pointEnd-margin""#)
            && svg.contains(r#"id="m15-mindmap_mindmap-pointStart-margin""#),
        "expected Mermaid 11.15 Mindmap margin markers: {svg}"
    );
    assert!(
        svg.contains(r#"id="m15-mindmap-drop-shadow""#)
            && svg.contains(r#"id="m15-mindmap-drop-shadow-small""#),
        "expected Mermaid 11.15 Mindmap scoped drop-shadow defs: {svg}"
    );
}

#[test]
fn mindmap_node_palette_reaches_branch_shapes_without_recoloring_the_root() {
    let theme = mindmap_node_palette_theme(&[
        "#123456",
        "transparent",
        "#dc2626",
        "#ea580c",
        "#ca8a04",
        "#16a34a",
        "#0891b2",
        "#2563eb",
        "#7c3aed",
        "#c026d3",
        "#db2777",
    ]);
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    First:::section-root\n    Second\n    Third\n    Fourth\n    Fifth\n    Sixth\n    Seventh\n    Eighth\n    Ninth\n    Tenth\n    Eleventh\n    Disabled((Disabled))\n    :::disabled\n    DisabledRect[Disabled rect]\n    :::disabled\n",
        &theme,
        Engine::new(),
        "mindmap-direct-palette",
        ThemePortabilityRequirement::RequirePortable,
    );

    let stylesheet = mindmap_stylesheet(rendered.svg());
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-direct-palette-node_1",
            "data-mindmap-section",
        )
        .as_deref(),
        Some("0")
    );
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-direct-palette-node_2",
            "data-mindmap-section",
        )
        .as_deref(),
        Some("1")
    );
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-direct-palette-node_11",
            "data-mindmap-section",
        )
        .as_deref(),
        Some("10")
    );
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-direct-palette-node_12",
            "data-mindmap-section",
        ),
        None,
        "the Mermaid disabled class must retain terminal fill ownership"
    );
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-direct-palette-node_13",
            "data-mindmap-section",
        )
        .as_deref(),
        Some("1"),
        "a disabled rect still consumes the direct palette because Mermaid only owns circle fill"
    );
    assert_eq!(
        mindmap_typed_section_shape_fill(&stylesheet, "mindmap-direct-palette", 0),
        "#123456"
    );
    assert_eq!(
        mindmap_typed_section_shape_fill(&stylesheet, "mindmap-direct-palette", 1),
        "#00000000"
    );
    assert_eq!(
        mindmap_typed_section_shape_fill(&stylesheet, "mindmap-direct-palette", 10),
        "#db2777"
    );
    assert!(
        stylesheet
            .find("#mindmap-direct-palette [data-mindmap-section=\"0\"] rect")
            .unwrap()
            > stylesheet
                .find("#mindmap-direct-palette .section-root rect")
                .unwrap(),
        "the private section owner must outrank a colliding user class"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn mindmap_typed_font_stack_reaches_layout_css_and_strict_receipt() {
    let font_stack =
        FontStack::new(["Mindmap Typed", "monospace"]).expect("valid Mindmap typed font stack");
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  A root label that exercises the shared font measurement\n    Child\n",
        &mindmap_font_stack_theme(font_stack),
        Engine::new(),
        "mindmap-typed-font",
        ThemePortabilityRequirement::RequirePortable,
    );

    let stylesheet = mindmap_stylesheet(rendered.svg());
    assert!(
        stylesheet.contains("Mindmap Typed") && stylesheet.contains("monospace"),
        "typed Mindmap font must reach the shared stylesheet: {stylesheet}"
    );

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn mindmap_site_font_family_outranks_typed_font_stack() {
    let typed_stack = FontStack::single("Mindmap Typed").expect("valid Mindmap typed font");
    let theme = mindmap_font_stack_theme(typed_stack);
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    Child\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "themeVariables": { "fontFamily": "Site Mindmap Font" }
        }))),
        "mindmap-site-font",
        ThemePortabilityRequirement::RequirePortable,
    );

    let stylesheet = mindmap_stylesheet(rendered.svg());
    assert!(stylesheet.contains("Site Mindmap Font"));
    assert!(!stylesheet.contains("Mindmap Typed"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn mindmap_font_size_remains_unsupported_and_fails_closed() {
    let theme = mindmap_font_size_theme(24.0);
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    Child\n",
        &theme,
        Engine::new(),
        "mindmap-unsupported-font-size",
        ThemePortabilityRequirement::BestEffort,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);

    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync("mindmap\n  Root\n    Child\n", ParseOptions::strict())
        .expect("parse strict Mindmap with unsupported FontSize")
        .expect("detect strict Mindmap with unsupported FontSize");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict Mindmap FontSize session");
    let error = match family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare strict Mindmap FontSize")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("strict Mindmap FontSize must remain unsupported"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::MINDMAP, 1)),
    );
}

#[test]
fn mindmap_direct_edge_stroke_reaches_classic_hand_drawn_neo_and_redux_paths() {
    let source = "mindmap\n  Root\n    Child\n";
    for (case, stroke, expected_stroke) in [
        (
            "solid",
            CanvasPaint::solid("#123456").expect("valid Mindmap Edge stroke"),
            "#123456",
        ),
        ("transparent", CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = mindmap_edge_stroke_theme(stroke);
        for (look_case, config) in [
            ("classic", json!({ "look": "classic" })),
            ("hand-drawn", json!({ "look": "handDrawn" })),
            ("neo", json!({ "look": "neo" })),
            ("redux", json!({ "theme": "redux", "look": "neo" })),
        ] {
            let rendered = render_mindmap_with_theme_and_engine(
                source,
                &theme,
                Engine::new().with_site_config(MermaidConfig::from_value(config)),
                &format!("mindmap-edge-{case}-{look_case}"),
                ThemePortabilityRequirement::RequirePortable,
            );
            let style = mindmap_edge_attribute(rendered.svg(), "edge_0_1", "style")
                .expect("directly themed Mindmap edge style");
            assert_eq!(
                style,
                format!("stroke:{expected_stroke} !important"),
                "case={case} look={look_case}: {}",
                rendered.svg(),
            );

            let completion = rendered.into_completion();
            let evidence = merman_render::__private::family_evidence(completion.report());
            assert_eq!(evidence.required_count(), 1, "case={case} look={look_case}");
            assert_eq!(evidence.applied_count(), 1, "case={case} look={look_case}");
            assert_eq!(
                evidence.theme_residual_count(),
                0,
                "case={case} look={look_case}",
            );
        }
    }
}

#[test]
fn mindmap_color_scale_ownership_suppresses_only_edges_using_that_slot() {
    let theme = mindmap_edge_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid Mindmap Edge stroke"),
    );
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    First\n      First child\n    Second\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "themeVariables": { "cScale1": "#fedcba" }
        }))),
        "mindmap-edge-color-scale-owner",
        ThemePortabilityRequirement::RequirePortable,
    );

    for edge_id in ["edge_0_1", "edge_1_2"] {
        assert_eq!(
            mindmap_edge_attribute(rendered.svg(), edge_id, "style"),
            None,
            "cScale1 must retain the edges in its section: {}",
            rendered.svg(),
        );
    }
    assert_eq!(
        mindmap_edge_attribute(rendered.svg(), "edge_0_3", "style").as_deref(),
        Some("stroke:#123456 !important"),
        "an unrelated section must retain the typed terminal owner: {}",
        rendered.svg(),
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn mindmap_neo_redux_node_border_ownership_suppresses_the_typed_edge_stroke() {
    let theme = mindmap_edge_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid Mindmap Edge stroke"),
    );
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    First\n    Second\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "theme": "redux",
            "look": "neo",
            "themeVariables": { "nodeBorder": "#fedcba" }
        }))),
        "mindmap-edge-node-border-owner",
        ThemePortabilityRequirement::RequirePortable,
    );

    for edge_id in ["edge_0_1", "edge_0_2"] {
        assert_eq!(
            mindmap_edge_attribute(rendered.svg(), edge_id, "style"),
            None,
            "the source-owned Redux nodeBorder must retain each Neo edge: {}",
            rendered.svg(),
        );
    }
    let stylesheet = mindmap_stylesheet(rendered.svg());
    assert!(
        stylesheet.contains(r#"[data-look="neo"].section-edge-0{stroke:#fedcba;}"#,),
        "the source-owned final edge value must remain in the terminal stylesheet: {stylesheet}",
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn mindmap_line_color_does_not_claim_the_edge_terminal() {
    let theme = mindmap_edge_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid Mindmap Edge stroke"),
    );
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    Child\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "themeVariables": { "lineColor": "#fedcba" }
        }))),
        "mindmap-edge-line-color",
        ThemePortabilityRequirement::RequirePortable,
    );

    assert_eq!(
        mindmap_edge_attribute(rendered.svg(), "edge_0_1", "style").as_deref(),
        Some("stroke:#123456 !important"),
        "lineColor is not a Mindmap edge source owner: {}",
        rendered.svg(),
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn mindmap_edge_fill_is_explicitly_unsupported_without_a_dead_line_color_bridge() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#123456")
                                .expect("valid unsupported Mindmap Edge fill"),
                        ),
                    )
                    .for_family(merman_render::DiagramFamilyId::MINDMAP),
                ),
            ),
        )
        .expect("compile unsupported Mindmap Edge fill");
    let source = "mindmap\n  Root\n    Child\n";

    let rendered = render_mindmap_with_theme_and_engine(
        source,
        &theme,
        Engine::new(),
        "mindmap-unsupported-edge-fill",
        ThemePortabilityRequirement::BestEffort,
    );
    assert_eq!(
        mindmap_edge_attribute(rendered.svg(), "edge_0_1", "style"),
        None,
        "an unsupported Edge.fill must not manufacture a Mindmap terminal style",
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);

    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse strict Mindmap with unsupported Edge.fill")
        .expect("detect strict Mindmap with unsupported Edge.fill");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict Mindmap session");
    let error = match family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare strict Mindmap with unsupported Edge.fill")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    {
        Ok(_) => panic!("strict Mindmap must reject an applicable unsupported Edge.fill"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::MINDMAP, 1)),
    );
}

#[test]
fn mindmap_edge_stroke_is_not_applicable_without_edge_paths() {
    let theme = mindmap_edge_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid Mindmap Edge stroke"),
    );
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n",
        &theme,
        Engine::new(),
        "mindmap-edge-empty",
        ThemePortabilityRequirement::RequirePortable,
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid empty Mindmap SVG");
    assert!(
        document
            .descendants()
            .all(|node| node.attribute("data-edge") != Some("true")),
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn mindmap_site_color_scale_slot_outranks_only_its_typed_palette_entry() {
    let theme = mindmap_node_palette_theme(&["#123456", "#abcdef"]);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "themeVariables": { "cScale1": "#fedcba" }
    })));
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    First\n    Second\n",
        &theme,
        engine,
        "mindmap-source-palette",
        ThemePortabilityRequirement::RequirePortable,
    );

    let stylesheet = mindmap_stylesheet(rendered.svg());
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-source-palette-node_1",
            "data-mindmap-section",
        ),
        None
    );
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-source-palette-node_2",
            "data-mindmap-section",
        )
        .as_deref(),
        Some("1")
    );
    assert_eq!(
        mindmap_mermaid_section_shape_fill(&stylesheet, "mindmap-source-palette", 0),
        "#fedcba"
    );
    assert_eq!(
        mindmap_typed_section_shape_fill(&stylesheet, "mindmap-source-palette", 1),
        "#abcdef"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn mindmap_explicit_mermaid_color_scale_slot_outranks_the_typed_palette() {
    let palette = OrdinalPalette::new([
        ThemeColorValue::parse("#123456").unwrap(),
        ThemeColorValue::parse("#abcdef").unwrap(),
    ])
    .unwrap();
    let compatibility = MermaidThemeCompatibility::default()
        .with_variable("cScale1", "#fedcba")
        .expect("valid explicit Mindmap Mermaid compatibility");
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_styles(
                    ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette),
                )
                .with_mermaid_compatibility(compatibility),
        )
        .expect("compile explicit Mermaid precedence fixture");
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    First\n    Second\n",
        &theme,
        Engine::new(),
        "mindmap-explicit-mermaid",
        ThemePortabilityRequirement::BestEffort,
    );
    let stylesheet = mindmap_stylesheet(rendered.svg());

    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-explicit-mermaid-node_1",
            "data-mindmap-section",
        ),
        None
    );
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-explicit-mermaid-node_2",
            "data-mindmap-section",
        )
        .as_deref(),
        Some("1")
    );
    assert_eq!(
        mindmap_mermaid_section_shape_fill(&stylesheet, "mindmap-explicit-mermaid", 0),
        "#fedcba"
    );
    assert_eq!(
        mindmap_typed_section_shape_fill(&stylesheet, "mindmap-explicit-mermaid", 1),
        "#abcdef"
    );
}

#[test]
fn mindmap_derived_color_scale_slot_retains_source_ownership() {
    let theme = mindmap_node_palette_theme(&["#123456", "#abcdef"]);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "theme": "base",
        "themeVariables": { "secondaryColor": "#00ff00" }
    })));
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    First\n    Second\n",
        &theme,
        engine,
        "mindmap-derived-palette",
        ThemePortabilityRequirement::RequirePortable,
    );

    let stylesheet = mindmap_stylesheet(rendered.svg());
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-derived-palette-node_1",
            "data-mindmap-section",
        ),
        None
    );
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-derived-palette-node_2",
            "data-mindmap-section",
        )
        .as_deref(),
        Some("1")
    );
    assert_eq!(
        mindmap_mermaid_section_shape_fill(&stylesheet, "mindmap-derived-palette", 0),
        "hsl(120, 100%, 25%)",
        "the source-derived color scale must retain the exact terminal value"
    );
    assert_eq!(
        mindmap_typed_section_shape_fill(&stylesheet, "mindmap-derived-palette", 1),
        "#abcdef"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn mindmap_gradient_changes_neo_stroke_without_stealing_node_fill() {
    let theme = mindmap_node_palette_theme(&["#123456"]);
    let classic = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    Child\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "look": "classic",
            "themeVariables": { "useGradient": true }
        }))),
        "mindmap-classic-gradient",
        ThemePortabilityRequirement::RequirePortable,
    );
    let classic_stylesheet = mindmap_stylesheet(classic.svg());
    assert_eq!(
        mindmap_node_attribute(
            classic.svg(),
            "mindmap-classic-gradient-node_1",
            "data-mindmap-section",
        )
        .as_deref(),
        Some("0")
    );
    assert_eq!(
        mindmap_typed_section_shape_fill(&classic_stylesheet, "mindmap-classic-gradient", 0),
        "#123456"
    );

    let neo = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    Child\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "look": "neo",
            "themeVariables": { "useGradient": false }
        }))),
        "mindmap-neo-palette",
        ThemePortabilityRequirement::RequirePortable,
    );
    let neo_stylesheet = mindmap_stylesheet(neo.svg());
    assert_eq!(
        mindmap_node_attribute(
            neo.svg(),
            "mindmap-neo-palette-node_1",
            "data-mindmap-section",
        )
        .as_deref(),
        Some("0")
    );
    assert_eq!(
        mindmap_typed_neo_section_shape_fill(&neo_stylesheet, "mindmap-neo-palette", 0),
        "#123456"
    );
    assert!(
        neo_stylesheet
            .find("#mindmap-neo-palette [data-look=\"neo\"].mindmap-node[data-mindmap-section=\"0\"] rect")
            .unwrap()
            > neo_stylesheet
                .find("#mindmap-neo-palette [data-look=\"neo\"].mindmap-node.section-0 rect")
                .unwrap(),
        "the equal-specificity typed selector must follow the Mermaid neo baseline"
    );

    let gradient = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    Child\n",
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({
            "look": "neo",
            "themeVariables": { "useGradient": true }
        }))),
        "mindmap-neo-gradient",
        ThemePortabilityRequirement::RequirePortable,
    );
    let gradient_stylesheet = mindmap_stylesheet(gradient.svg());

    assert_eq!(
        mindmap_node_attribute(
            gradient.svg(),
            "mindmap-neo-gradient-node_1",
            "data-mindmap-section",
        )
        .as_deref(),
        Some("0"),
        "the typed palette still owns fill while Mermaid's gradient owns stroke"
    );
    assert_eq!(
        mindmap_typed_neo_section_shape_fill(&gradient_stylesheet, "mindmap-neo-gradient", 0,),
        "#123456"
    );
    assert!(
        gradient_stylesheet.contains("stroke:url(#mindmap-neo-gradient-gradient)"),
        "the Mermaid neo gradient must remain in the terminal stylesheet"
    );

    let completion = gradient.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn mindmap_neo_main_background_keeps_terminal_ownership_over_the_node_palette() {
    let theme = mindmap_node_palette_theme(&["#123456"]);
    for (case, config) in [
        (
            "explicit",
            json!({
                "theme": "base",
                "look": "neo",
                "themeVariables": {
                    "mainBkg": "#fedcba",
                    "useGradient": true
                }
            }),
        ),
        (
            "derived",
            json!({
                "theme": "base",
                "look": "neo",
                "themeVariables": {
                    "primaryColor": "#fedcba",
                    "useGradient": true
                }
            }),
        ),
    ] {
        let diagram_id = format!("mindmap-neo-main-background-{case}");
        let rendered = render_mindmap_with_theme_and_engine(
            "mindmap\n  Root\n    Child\n",
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(config)),
            &diagram_id,
            ThemePortabilityRequirement::RequirePortable,
        );

        assert_eq!(
            mindmap_node_attribute(
                rendered.svg(),
                &format!("{diagram_id}-node_1"),
                "data-mindmap-section",
            ),
            None,
            "a source-owned neo fill must not receive the typed terminal owner attribute"
        );
        assert_eq!(
            mindmap_neo_section_shape_fill(&mindmap_stylesheet(rendered.svg()), &diagram_id, 0,),
            "#fedcba"
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn mindmap_palette_fails_closed_when_the_section_css_is_outside_the_color_limit() {
    let theme = mindmap_node_palette_theme(&["#123456"]);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "themeVariables": { "THEME_COLOR_LIMIT": 1 }
    })));
    let error =
        match prepare_mindmap_with_theme_and_engine("mindmap\n  Root\n    Child\n", &theme, engine)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        {
            Ok(_) => panic!("Mindmap palette must not be verified without a branch CSS rule"),
            Err(error) => error,
        };

    assert_eq!(
        error.unverified_family_theme(),
        Some((merman_render::DiagramFamilyId::MINDMAP, 1))
    );
}

#[test]
fn mindmap_legacy_cluster_fill_does_not_shadow_the_direct_node_palette() {
    let palette = OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap();
    let styles = ThemeRuleSet::default()
        .with_ordinal_palette(ThemeTarget::Node, palette)
        .with_rule(
            ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#fedcba").unwrap()),
            )
            .for_family(merman_render::DiagramFamilyId::MINDMAP),
        );
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile mixed direct and compatibility Mindmap theme");
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "theme": "base"
    })));
    let rendered = render_mindmap_with_theme_and_engine(
        "mindmap\n  Root\n    Child\n",
        &theme,
        engine,
        "mindmap-mixed-route",
        ThemePortabilityRequirement::BestEffort,
    );

    let stylesheet = mindmap_stylesheet(rendered.svg());
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-mixed-route-node_1",
            "data-mindmap-section",
        )
        .as_deref(),
        Some("0")
    );
    assert_eq!(
        mindmap_typed_section_shape_fill(&stylesheet, "mindmap-mixed-route", 0),
        "#123456"
    );
}

#[test]
fn mindmap_classic_raw_theme_css_remains_the_terminal_node_fill_owner() {
    let theme = mindmap_node_palette_theme(&["#123456"]);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(json!({
        "themeCSS": ".custom rect { fill: #22c55e; }"
    })));
    let parsed = merman_render::__private::install_parse_compatibility(&theme, engine)
        .parse_diagram_for_render_model_sync(
            "mindmap\n  Root\n    Child[Child]\n    :::custom\n",
            ParseOptions::strict(),
        )
        .expect("parse Mindmap with raw theme CSS")
        .expect("detect Mindmap with raw theme CSS");
    let session = RenderEnvironment::deterministic()
        .with_theme_admission_policy(ThemeAdmissionPolicy::permissive().with_trusted_lanes(
            TrustedThemeLanes::from_allowed([TrustedThemeLane::RawThemeCss]),
        ))
        .begin_session_with_theme(&theme)
        .expect("begin trusted raw-theme-CSS session");
    let rendered = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Mindmap with raw theme CSS")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("mindmap-raw-theme-css".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render Mindmap with raw theme CSS");

    let stylesheet = mindmap_stylesheet(rendered.svg());
    assert_eq!(
        mindmap_node_attribute(
            rendered.svg(),
            "mindmap-raw-theme-css-node_1",
            "data-mindmap-section",
        ),
        Some("0".to_string()),
        "the typed palette remains composable with unrelated raw theme CSS"
    );
    assert_eq!(
        mindmap_typed_section_shape_fill(&stylesheet, "mindmap-raw-theme-css", 0),
        "#123456"
    );
    assert!(stylesheet.contains("#22c55e"));
    assert!(
        stylesheet.find("#22c55e").unwrap()
            > stylesheet
                .find("#mindmap-raw-theme-css [data-mindmap-section=\"0\"] rect")
                .unwrap(),
        "raw theme CSS must remain later in the cascade than the classic typed palette"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert!(evidence.output_mutated());
}

#[test]
fn mindmap_break_spaces_keeps_the_trailing_indentation_line_box() {
    let svg = render_mindmap_svg_from_text(
        "mindmap\n  root[\n    Multi-line root\n    with three lines\n  ]\n",
        "mindmap-break-spaces",
    );

    assert!(
        svg.contains(
            r#"<rect class="basic label-container" style="" x="-120" y="-46" width="240" height="92"/>"#
        ),
        "expected the fixed-width Mindmap node to include three 24px line boxes: {svg}"
    );
    assert!(
        svg.contains(r#"<foreignObject width="200" height="72">"#),
        "expected the label foreignObject to preserve the trailing indentation row: {svg}"
    );
    assert!(
        svg.contains(
            "<span class=\"nodeLabel markdown-node-label\">    Multi-line root\n    with three lines\n  </span>"
        ),
        "expected SVG emission to preserve the source whitespace used by break-spaces: {svg}"
    );
}

#[test]
fn mindmap_hex_entity_placeholders_remain_literal_well_formed_xml() {
    for (source, expected) in [
        ("mindmap\n  root[&#x41;]\n", "<p>&amp;&amp;x41;</p>"),
        ("mindmap\n  root[&#X41;]\n", "<p>&amp;&amp;X41;</p>"),
    ] {
        let svg = render_mindmap_svg_from_text(source, "mindmap-hex-entity");

        assert!(svg.contains(expected), "expected {expected:?}: {svg}");
        roxmltree::Document::parse(&svg).expect("Mindmap SVG must be well-formed XML");
    }
}

#[test]
fn mindmap_named_entity_placeholders_are_decoded_before_svg_emission() {
    let svg =
        render_mindmap_svg_from_text("mindmap\n  root[Root #quot;]\n", "mindmap-named-entity");

    assert!(
        svg.contains("<p>Root \"</p>"),
        "expected the Mermaid quote placeholder to become visible text: {svg}"
    );
    assert!(
        !svg.contains('ﬂ') && !svg.contains('¶'),
        "Mermaid entity placeholders must not leak into SVG: {svg}"
    );
    roxmltree::Document::parse(&svg).expect("Mindmap SVG must be well-formed XML");
}

#[test]
fn mindmap_typed_layout_handles_deep_chain() {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .unwrap();
    const DEPTH: usize = 1200;
    let source = deep_mindmap_chain(DEPTH);

    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");

    // Exercise the public JSON projection as well as its ordinary destruction. The semantic
    // projection contains the same deeply nested rootNode tree even though the typed model and
    // layout are flat.
    let layout = layout_mindmap_typed(&parsed, session);

    assert_eq!(layout.nodes.len(), DEPTH);
    assert_eq!(layout.edges.len(), DEPTH - 1);
    let expected_last = format!("{}", DEPTH - 1);
    assert_eq!(
        layout.nodes.last().map(|node| node.id.as_str()),
        Some(expected_last.as_str())
    );
}

#[test]
fn mindmap_svg_wraps_section_classes_after_mermaid_palette_cycle() {
    let svg = render_mindmap_svg_from_text(
        r#"mindmap
  root((Many siblings))
    s01[Node 01]
    s02[Node 02]
    s03[Node 03]
    s04[Node 04]
    s05[Node 05]
    s06[Node 06]
    s07[Node 07]
    s08[Node 08]
    s09[Node 09]
    s10[Node 10]
    s11[Node 11]
    s12[Node 12]
"#,
        "m15-mindmap-cycle",
    );

    assert!(
        svg.contains(r#"class="node mindmap-node section-10" id="m15-mindmap-cycle-node_11""#),
        "expected eleventh sibling to use section-10 before the cycle wraps: {svg}"
    );
    assert!(
        svg.contains(r#"class="node mindmap-node section-0" id="m15-mindmap-cycle-node_12""#),
        "expected twelfth sibling to wrap back to section-0 like Mermaid 11.15: {svg}"
    );
    assert!(
        !svg.contains("section-11") && !svg.contains("section-edge-11"),
        "Mindmap section classes should wrap instead of emitting stale section-11 tokens: {svg}"
    );
}

#[test]
fn mindmap_svg_uses_direct_classic_shapes_for_rounded_and_hexagon_nodes() {
    let svg = render_mindmap_svg_from_text(
        r#"mindmap
  root((Root))
    rounded(Rounded)
    hex{{Hexagon}}
"#,
        "m15-mindmap-shapes",
    );

    assert!(
        svg.contains(r#"<rect class="basic label-container" style="" rx="5" ry="5""#),
        "expected classic rounded Mindmap nodes to render as direct rect DOM: {svg}"
    );
    assert!(
        svg.contains(r#"<polygon points=""#) && svg.contains(r#"class="label-container""#),
        "expected classic hexagon Mindmap nodes to render as direct polygon DOM: {svg}"
    );
    assert!(
        !svg.contains(r#"class="basic label-container outer-path""#),
        "classic Mindmap rounded/hexagon nodes should not use the old rough outer-path wrapper: {svg}"
    );
}

#[test]
fn mindmap_tidy_tree_config_dispatches_bidirectional_layout() {
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            r#"---
config:
  layout: tidy-tree
---
mindmap
  root((Root))
    Left
      Left child
    Right
      Right child
    Also left
"#,
            ParseOptions::strict(),
        )
        .expect("parse ok")
        .expect("diagram detected");

    let layout = layout_mindmap_typed(&parsed, session);
    let node = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("missing node {id}"))
    };

    let root = node("0");
    let left = node("1");
    let left_child = node("2");
    let right = node("3");
    let right_child = node("4");
    let also_left = node("5");
    assert!(root.x.is_finite() && root.y.is_finite());
    assert!(left.x < root.x && left_child.x < left.x);
    assert!(right.x > root.x && right_child.x > right.x);
    assert!(also_left.x < root.x);

    assert!(layout.edges.iter().all(|edge| edge.points.len() == 4));
    let edge_to_left = layout
        .edges
        .iter()
        .find(|edge| edge.from == "0" && edge.to == "1")
        .expect("root-to-left edge");
    let edge_to_right = layout
        .edges
        .iter()
        .find(|edge| edge.from == "0" && edge.to == "3")
        .expect("root-to-right edge");
    assert!(edge_to_left.points[1].x < root.x);
    assert!(edge_to_right.points[1].x > root.x);
}
