use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemePortabilityRequirement,
    ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant,
};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{LayoutOptions, family};
use serde_json::{Value, json};

fn render_cluster(config: Value, source_style: &str, paint: CanvasPaint) -> (String, usize, usize) {
    render_cluster_with_spec(
        config,
        source_style,
        DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(paint),
                )
                .with_variant(ThemeVariant::Default),
            ),
        ),
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render portable ER cluster")
}

fn render_cluster_with_spec(
    config: Value,
    source_style: &str,
    spec: DiagramThemeSpec,
    requirement: ThemePortabilityRequirement,
) -> Result<(String, usize, usize), merman_render::Error> {
    let theme = DiagramThemeCompiler::new()
        .compile(spec)
        .expect("compile ER cluster text theme");
    let source = format!(
        "---\nconfig: {config}\n---\nerDiagram\nsubgraph Orders [\"**Order** Domain\"]\n CUSTOMER\nend\nstyle CUSTOMER color:#556677\n{source_style}\n"
    );
    let parsed = merman_render::__private::install_parse_compatibility(
        &theme,
        Engine::new().with_site_config(MermaidConfig::from_value(json!({"secure": []}))),
    )
    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
    .expect("parse ER cluster")
    .expect("detect ER cluster");
    assert_eq!(
        parsed
            .metadata()
            .effective_config
            .as_value()
            .pointer("/themeVariables/titleColor"),
        config.pointer("/themeVariables/titleColor"),
        "the fixture must retain its explicit titleColor owner or fallback"
    );
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(requirement)
        .begin_session_with_theme(&theme)
        .expect("begin ER cluster session");
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare ER cluster")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    let svg = rendered.svg().to_owned();
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    if matches!(requirement, ThemePortabilityRequirement::RequirePortable) {
        assert_eq!(evidence.theme_residual_count(), 0);
    }
    assert_eq!(evidence.compatibility_residual_count(), 0);
    Ok((
        svg,
        evidence.applied_count(),
        evidence.not_applicable_count(),
    ))
}

#[test]
fn svg_cluster_uses_typed_text_fill_when_title_color_selects_the_text_fallback() {
    for look in ["classic", "neo", "handDrawn"] {
        for (paint, color) in [
            (
                CanvasPaint::solid("#123abc").expect("solid text paint"),
                "#123abc",
            ),
            (CanvasPaint::Transparent, "transparent"),
        ] {
            let (svg, applied, not_applicable) = render_cluster(
                json!({"htmlLabels": false, "look": look, "themeVariables": {"titleColor": false}}),
                "",
                paint,
            );
            let document = roxmltree::Document::parse(&svg).expect("valid SVG");
            let text = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("text")
                        && node
                            .ancestors()
                            .any(|ancestor| ancestor.attribute("class") == Some("cluster-label"))
                })
                .expect("visible SVG cluster text");
            assert_eq!(
                text.attribute("style"),
                Some(format!("color:{color};fill:{color}").as_str()),
                "{look}"
            );
            assert!(
                text.descendants()
                    .any(|node| node.text().is_some_and(|text| text.contains("Order")))
            );
            assert!(text.descendants().any(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("font-weight") == Some("bold")
                    && node.text() == Some("Order")
            }));
            assert_eq!((applied, not_applicable), (1, 0));
        }
    }
}

#[test]
fn cluster_title_config_source_color_and_html_labels_keep_their_owners() {
    for (config, source_style, expected_style) in [
        (
            json!({"htmlLabels": false, "themeVariables": {"titleColor": "#c0ffee"}}),
            "",
            Some(""),
        ),
        (
            json!({"htmlLabels": false, "themeVariables": {"titleColor": false, "textColor": "#c0ffee"}}),
            "",
            Some(""),
        ),
        (
            json!({"htmlLabels": false, "themeVariables": {"titleColor": false}}),
            "style Orders color:#c0ffee",
            Some("fill:#c0ffee !important"),
        ),
        (
            json!({"htmlLabels": true, "themeVariables": {"titleColor": false}}),
            "",
            None,
        ),
    ] {
        let (svg, applied, not_applicable) =
            render_cluster(config, source_style, CanvasPaint::solid("#123abc").unwrap());
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let cluster = document
            .descendants()
            .find(|node| node.attribute("class") == Some("cluster-label"))
            .expect("cluster label");
        let text = cluster.descendants().find(|node| node.has_tag_name("text"));
        assert_eq!(
            text.and_then(|text| text.attribute("style")),
            expected_style
        );
        assert_eq!((applied, not_applicable), (0, 1));
    }
}

#[test]
fn subgraph_source_typography_is_fail_closed_until_measured() {
    for source_style in [
        "style Orders font-family:SourceFace",
        "style Orders font-size:30px",
    ] {
        let spec = DiagramThemeSpec::new().with_typography(
            merman_render::diagram_theme::TypographySpec::default().with_family_style(
                merman_core::DiagramFamilyId::ER,
                ThemeTextStyle::default()
                    .with_font_stack(FontStack::single("ErTypedFace").expect("font stack"))
                    .with_font_size_px(24.0)
                    .expect("font size"),
            ),
        );
        let (svg, applied, not_applicable) = render_cluster_with_spec(
            json!({"htmlLabels": false, "themeVariables": {"titleColor": false}}),
            source_style,
            spec,
            ThemePortabilityRequirement::BestEffort,
        )
        .expect("render ER cluster in best effort");
        assert!(
            svg.contains("!important"),
            "source style must remain visible: {svg}"
        );
        // One typography property remains typed; the source-owned property must stay residual.
        assert_eq!(applied, 1);
        assert_eq!(not_applicable, 0);
    }
}
