use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::DiagramFamilyId;
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette,
    ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget, ThemeTextStyle, TypographySpec,
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
            .map(|color| ThemeColorValue::parse(*color).expect("valid Sankey node palette color")),
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
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Sankey")
        .expect("detect themed Sankey");
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

fn render_sankey_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
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
    let rendered = render_sankey_with_theme_and_engine(source, &theme, Engine::new());
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
        Engine::new(),
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
        Engine::new(),
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
        Engine::new(),
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
            Some(config) => Engine::new().with_site_config(MermaidConfig::from_value(config)),
            None => Engine::new(),
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
        assert_eq!(evidence.compatibility_residual_count(), 0, "owner={owner}");
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
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render oversized direct Sankey font stack");

    assert!(rendered.svg().contains(&expected_font));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
        Engine::new(),
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
    assert_eq!(evidence.compatibility_residual_count(), 0);

    let error = render_sankey_with_theme_requirement(
        source,
        &theme,
        Engine::new(),
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
        Engine::new(),
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
                    let mut config = serde_json::json!({"sankey": {"showValues": outlined}});
                    if config_owner {
                        config["themeVariables"] = serde_json::json!({"textColor":"#445566"});
                    }
                    let engine = Engine::new().with_site_config(MermaidConfig::from_value(config));
                    let rendered = render_sankey_with_theme_requirement(
                        "sankey-beta\nA,B,10\n",
                        &theme,
                        engine.clone(),
                        ThemePortabilityRequirement::BestEffort,
                    )
                    .unwrap();
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
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}
