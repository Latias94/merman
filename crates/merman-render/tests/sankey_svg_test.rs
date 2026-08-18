use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue,
    ThemePortabilityRequirement, ThemeRuleSet, ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn sankey_node_palette_theme(colors: &[&str]) -> DiagramTheme {
    let palette = OrdinalPalette::new(
        colors
            .iter()
            .map(|color| ThemeColorValue::parse(*color).expect("valid Sankey node palette color")),
    )
    .expect("non-empty Sankey node palette");
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette),
            ),
        )
        .expect("compile Sankey node palette theme")
}

fn render_sankey_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::RenderedFamilySvg {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Sankey")
        .expect("detect themed Sankey");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Sankey session");
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed Sankey")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed Sankey")
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

fn render_sankey(source: &str, options: &SvgRenderOptions) -> String {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("layout ok")
        .render_svg(options, &SvgDebugOptions::default())
        .expect("render SVG")
        .svg()
        .to_owned()
}

fn try_render_sankey_with_resource_policy(
    source: &str,
    options: &SvgRenderOptions,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Sankey resource-bound fixture")
        .expect("detect Sankey resource-bound fixture");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Sankey resource-bound session");
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)?
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
        render_sankey_with_theme_and_engine("sankey-beta\nA,B,10\n", &theme, Engine::new());

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
fn sankey_explicit_node_colors_outrank_typed_palette_per_node() {
    let theme = sankey_node_palette_theme(&["#123456", "#abcdef"]);
    let cases = [
        (
            "site",
            "sankey-beta\nA,B,10\n",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "sankey": { "nodeColors": { "A": "#fedcba" } }
            }))),
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
            Engine::new(),
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
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "sankey": {
                "nodeColors": {
                    "A": "#fedcba",
                    "B": "#654321"
                }
            }
        }))),
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
        let rendered = render_sankey_with_theme_and_engine(&source, &theme, Engine::new());
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
