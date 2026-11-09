mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{MermaidEngine, ParseOptions};
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
use merman_render::{DiagramFamilyId, LayoutOptions};

fn radar_series_palette_theme(colors: &[&str]) -> DiagramTheme {
    let palette = OrdinalPalette::new(
        colors
            .iter()
            .map(|color| ThemeColorValue::parse(color).expect("valid Radar series palette color")),
    )
    .expect("non-empty Radar series palette");
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::ChartSeries, palette),
        ))
        .expect("compile Radar series palette theme")
}

fn radar_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::RADAR, typography),
        ))
        .expect("compile Radar typography theme")
}

fn radar_title_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    let rule = ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default().with_fill(fill),
    )
    .for_family(DiagramFamilyId::RADAR);
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
        .expect("compile Radar title fill theme")
}

fn render_radar_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = common::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Radar")
        .expect("detect themed Radar")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Radar session");
    family::prepare(&parsed, &LayoutOptions::default(), session)?
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn render_radar_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: MermaidEngine,
) -> family::RenderedFamilySvg {
    render_radar_with_theme_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render strict portable Radar")
}

fn radar_stylesheet(svg: &str) -> &str {
    svg.split_once("<style>")
        .and_then(|(_, tail)| tail.split_once("</style>"))
        .map(|(stylesheet, _)| stylesheet)
        .unwrap_or_else(|| panic!("Radar SVG must contain one stylesheet: {svg}"))
}

fn render_radar_svg_from_text(text: &str) -> String {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse radar")
        .expect("detect radar")
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout radar");

    artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render radar")
        .svg()
        .to_owned()
}

fn try_render_radar_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = legacy_init_theme_compat_engine()
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Radar resource-bound fixture")
        .expect("detect Radar resource-bound fixture")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Radar resource-bound session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn radar_svg_skips_mismatched_curves_and_preserves_authored_series_indices() {
    for graticule in ["circle", "polygon"] {
        for show_legend in [false, true] {
            for (curves, expected_indices, series_count) in [
                ("curve Short{1,2}", Vec::new(), 1),
                ("curve Long{1,2,3,4}", Vec::new(), 1),
                (
                    "curve Short{100,1}, Complete{1,2,3}, Long{1,2,3,200}",
                    vec![1],
                    3,
                ),
            ] {
                let source = format!(
                    "radar-beta\naxis A,B,C\n{curves}\ngraticule {graticule}\nticks 0\nshowLegend {show_legend}\n"
                );
                let svg = render_radar_svg_from_text(&source);
                let document = roxmltree::Document::parse(&svg).unwrap();
                let curve_nodes = document
                    .descendants()
                    .filter(|node| {
                        node.attribute("class")
                            .is_some_and(|class| class.starts_with("radarCurve-"))
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    curve_nodes
                        .iter()
                        .map(|node| node.attribute("class").unwrap())
                        .collect::<Vec<_>>(),
                    expected_indices
                        .iter()
                        .map(|index| format!("radarCurve-{index}"))
                        .collect::<Vec<_>>(),
                    "{source}"
                );
                for node in curve_nodes {
                    assert_eq!(
                        node.tag_name().name(),
                        if graticule == "polygon" {
                            "polygon"
                        } else {
                            "path"
                        }
                    );
                }
                let legend_classes = document
                    .descendants()
                    .filter_map(|node| node.attribute("class"))
                    .filter(|class| class.starts_with("radarLegendBox-"))
                    .collect::<Vec<_>>();
                let legend_count = if show_legend { series_count } else { 0 };
                assert_eq!(
                    legend_classes,
                    (0..legend_count)
                        .map(|index| format!("radarLegendBox-{index}"))
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}

#[test]
fn radar_svg_preserves_fractional_tick_counts_and_radii_in_both_graticules() {
    for graticule in ["circle", "polygon"] {
        for (ticks, expected_radii) in [
            ("2", vec![150.0, 300.0]),
            ("2.0", vec![150.0, 300.0]),
            ("2.5", vec![120.0, 240.0, 360.0]),
            ("0", Vec::new()),
        ] {
            let source = format!(
                "radar-beta\naxis A,B,C\ncurve Complete{{1,2,3}}\ngraticule {graticule}\nticks {ticks}\n"
            );
            let svg = render_radar_svg_from_text(&source);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let graticules = document
                .descendants()
                .filter(|node| node.attribute("class") == Some("radarGraticule"))
                .collect::<Vec<_>>();
            assert_eq!(graticules.len(), expected_radii.len(), "{source}");
            for (node, expected_radius) in graticules.iter().zip(expected_radii) {
                assert_eq!(node.tag_name().name(), graticule);
                let radius = if graticule == "circle" {
                    node.attribute("r").unwrap().parse::<f64>().unwrap()
                } else {
                    -node
                        .attribute("points")
                        .unwrap()
                        .split_whitespace()
                        .next()
                        .unwrap()
                        .split(',')
                        .nth(1)
                        .unwrap()
                        .parse::<f64>()
                        .unwrap()
                };
                assert!((radius - expected_radius).abs() < 1e-9, "{source}");
            }
        }
    }
}

#[test]
fn radar_strict_series_evidence_tracks_sparse_curves_and_real_legend_consumers() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(
                        ThemeTarget::ChartSeries,
                        ThemeStylePatch::default()
                            .with_stroke(CanvasPaint::solid("#445566").unwrap()),
                    ))
                    .with_ordinal_palette(
                        ThemeTarget::ChartSeries,
                        OrdinalPalette::new(
                            ["#123456", "#abcdef", "#fedcba"]
                                .map(|color| ThemeColorValue::parse(color).unwrap()),
                        )
                        .unwrap(),
                    ),
            ),
        )
        .unwrap();
    for graticule in ["circle", "polygon"] {
        for show_legend in [false, true] {
            for (curves, has_drawable_curve) in [
                ("curve Short{1,2}, Long{1,2,3,4}", false),
                ("curve Short{1,2}, Complete{1,2,3}, Long{1,2,3,4}", true),
            ] {
                let source = format!(
                    "radar-beta\naxis A,B,C\n{curves}\ngraticule {graticule}\nshowLegend {show_legend}\n"
                );
                let rendered =
                    render_radar_with_theme_and_engine(&source, &theme, MermaidEngine::new());
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                assert_eq!(
                    document
                        .descendants()
                        .filter(|node| node.attribute("class") == Some("radarCurve-1"))
                        .count(),
                    usize::from(has_drawable_curve)
                );
                if has_drawable_curve {
                    assert!(radar_stylesheet(rendered.svg()).contains(
                        ".radarCurve-1{color:#abcdef;fill:#abcdef;fill-opacity:0.5;stroke:#445566;"
                    ));
                }
                let visible = has_drawable_curve || show_legend;
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 2);
                assert_eq!(evidence.accounted_count(), 2);
                assert_eq!(evidence.applied_count(), if visible { 2 } else { 0 });
                assert_eq!(evidence.not_applicable_count(), if visible { 0 } else { 2 });
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn radar_hidden_skipped_curves_do_not_capture_ordinal_theme_rules() {
    use merman_render::diagram_theme::OrdinalSelector;

    let source = "radar-beta\naxis A,B,C\ncurve Short{1,2}, Complete{1,2,3}\nshowLegend false\n";
    for ordinal in [1, 2] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::ChartSeries,
                            ThemeStylePatch::default().with_stroke_width(7.0).unwrap(),
                        )
                        .with_ordinal(OrdinalSelector::Exact(ordinal)),
                    ),
                ),
            )
            .unwrap();
        let result = render_radar_with_theme_requirement(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable,
        );
        if ordinal == 1 {
            let rendered = result.expect("an ordinal on a hidden skipped series is not applicable");
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(evidence.theme_residual_count(), 0);
        } else {
            let error = match result {
                Ok(_) => panic!("unsupported width on the drawable series must remain a residual"),
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((DiagramFamilyId::RADAR, 1))
            );
        }
    }
}

#[test]
fn radar_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"radar-beta
title Bounded capability
axis Reliability,Performance,Clarity
curve Current{3,4,2}
curve Target{5,5,5}
"#;
    let baseline = try_render_radar_svg_with_resource_policy(
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Radar baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Radar fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Radar SVG byte ceiling");
    let exact = try_render_radar_svg_with_resource_policy(source, exact_policy)
        .expect("the exact Radar family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Radar SVG byte ceiling");
    let error = try_render_radar_svg_with_resource_policy(source, below_policy)
        .expect_err("one byte below the Radar family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Radar MaxSvgBytes rejection, got {error}");
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
fn radar_frontmatter_title_renders_unless_the_body_overrides_it() {
    let frontmatter_svg = render_radar_svg_from_text(
        r#"---
title: Frontmatter radar
---
radar-beta
axis A,B,C
curve score{1,2,3}
"#,
    );
    assert!(
        frontmatter_svg.contains(r#"class="radarTitle""#)
            && frontmatter_svg.contains(">Frontmatter radar</text>"),
        "frontmatter title should render when the Radar body has none: {frontmatter_svg}"
    );

    let body_svg = render_radar_svg_from_text(
        r#"---
title: Frontmatter radar
---
radar-beta
title Body radar
axis A,B,C
curve score{1,2,3}
"#,
    );
    assert!(body_svg.contains(">Body radar</text>"));
    assert!(!body_svg.contains(">Frontmatter radar</text>"));
}

#[test]
fn radar_typed_title_fill_reaches_the_title_terminal() {
    let theme = radar_title_fill_theme(CanvasPaint::solid("#123456").expect("valid fill"));
    let rendered = render_radar_with_theme_and_engine(
        "radar-beta\ntitle Themed radar\naxis A,B,C\ncurve Current{3,4,2}\n",
        &theme,
        MermaidEngine::new(),
    );

    let stylesheet = radar_stylesheet(rendered.svg());
    assert!(
        stylesheet.contains("#radar .radarTitle{font-size:16px;color:#123456;"),
        "Radar title stylesheet did not contain typed fill: {stylesheet}"
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_default_title_fill_preserves_winners_and_configuration_owners() {
    use merman_render::diagram_theme::ThemeVariant;

    for (fill, expected) in [
        (CanvasPaint::solid("#123456").unwrap(), "#123456"),
        (CanvasPaint::Transparent, "transparent"),
    ] {
        for later_override in [false, true] {
            let mut rules = ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(fill.clone()),
                )
                .with_variant(ThemeVariant::Default)
                .for_family(DiagramFamilyId::RADAR),
            );
            if later_override {
                rules = rules.with_rule(
                    ThemeRule::new(
                        ThemeTarget::Title,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#abcdef").unwrap()),
                    )
                    .for_family(DiagramFamilyId::RADAR),
                );
            }
            let theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_styles(rules))
                .unwrap();
            for title_present in [false, true] {
                for config_owner in [false, true] {
                    let source = if title_present {
                        "radar-beta\ntitle Typed radar\naxis A,B,C\ncurve Current{3,4,2}\n"
                    } else {
                        "radar-beta\naxis A,B,C\ncurve Current{3,4,2}\n"
                    };
                    let engine = if config_owner {
                        MermaidEngine::new().with_site_config(
                            merman_core::presentation::SourcePresentationConfig::from_value(
                                serde_json::json!({"themeVariables":{"titleColor":"#445566"}}),
                            ),
                        )
                    } else {
                        MermaidEngine::new()
                    };
                    let rendered = render_radar_with_theme_and_engine(source, &theme, engine);
                    if title_present {
                        let color = if config_owner {
                            "#445566"
                        } else if later_override {
                            "#abcdef"
                        } else {
                            expected
                        };
                        assert!(
                            radar_stylesheet(rendered.svg())
                                .contains(&format!(".radarTitle{{font-size:16px;color:{color};")),
                            "{}",
                            rendered.svg()
                        );
                    }
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(
                        evidence.applied_count(),
                        usize::from(title_present && !config_owner)
                    );
                    assert_eq!(
                        evidence.not_applicable_count(),
                        1 + usize::from(later_override)
                            - usize::from(title_present && !config_owner)
                    );
                    assert_eq!(evidence.theme_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn radar_mixed_typography_is_direct_and_portable() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("RadarMixed").expect("valid mixed Radar font stack"))
        .with_font_size_px(24.0)
        .expect("valid mixed Radar font size");
    let theme = radar_typography_theme(typography);
    let rendered = render_radar_with_theme_and_engine(
        "radar-beta\ntitle Mixed radar\naxis A,B,C\ncurve Current{3,4,2}\n",
        &theme,
        MermaidEngine::new(),
    );

    let stylesheet = radar_stylesheet(rendered.svg());
    assert!(stylesheet.contains("#radar{font-family:RadarMixed;font-size:24px;"));
    assert!(stylesheet.contains("#radar svg{font-family:RadarMixed;font-size:24px;}"));
    assert!(stylesheet.contains("#radar .radarTitle{font-size:24px;"));
    assert!(stylesheet.contains("#radar .radarAxisLabel{font-size:12px;"));
    assert!(stylesheet.contains("#radar .radarLegendText{text-anchor:start;font-size:12px;"));
    assert!(stylesheet.contains("#radar :root{--mermaid-font-family:RadarMixed;}"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_digit_leading_font_family_is_quoted_and_portable() {
    let theme = radar_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("123Radar").expect("valid digit-leading Radar font stack"),
    ));
    let rendered = render_radar_with_theme_and_engine(
        "radar-beta\ntitle Quoted radar\naxis A,B,C\ncurve Current{3,4,2}\n",
        &theme,
        MermaidEngine::new(),
    );

    let stylesheet = radar_stylesheet(rendered.svg());
    assert!(stylesheet.contains("font-family:\"123Radar\";"));
    assert!(!stylesheet.contains("font-family:123Radar;"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_zero_size_text_does_not_certify_fonts_but_positive_and_host_sizes_do() {
    let theme = radar_typography_theme(
        ThemeTextStyle::default().with_font_stack(FontStack::single("Radar Typed").unwrap()),
    );
    for (title_size, axis_size, legend_size, applied) in [
        ("0px", 0.0, 0.0, false),
        ("-0.0pt", 0.0, 0.0, false),
        ("0em", 0.0, 0.0, false),
        ("0%", 0.0, 0.0, false),
        ("0px", -1.0, -1.0, false),
        ("18px", 0.0, 0.0, true),
        ("0px", 12.0, 0.0, true),
        ("0px", 0.0, 12.0, true),
        ("0px", 0.5, 0.0, true),
        ("0px", 1e-10, 1e-10, false),
        ("var(--radar-size)", 0.0, 0.0, true),
        ("calc(0px + 1vw)", 0.0, 0.0, true),
    ] {
        let config = serde_json::json!({
            "themeVariables": {
                "fontSize": title_size,
                "radar": { "axisLabelFontSize": axis_size, "legendFontSize": legend_size }
            }
        });
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(config.clone()),
        );
        let rendered = render_radar_with_theme_and_engine(RADAR_TEXT_SOURCE, &theme, engine);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .count(),
            5
        );
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.attribute("class") == Some("radarAxisLine"))
                .count(),
            3,
            "zero-size labels must retain all axis lines"
        );
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("class") == Some("radarLegendBox-0"))
        );
        assert!(
            radar_stylesheet(rendered.svg())
                .contains(&format!(".radarTitle{{font-size:{title_size};"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "{config}");
        assert_eq!(evidence.accounted_count(), 1, "{config}");
        assert_eq!(evidence.applied_count(), usize::from(applied), "{config}");
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(!applied),
            "{config}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{config}");
    }
}

#[test]
fn radar_negative_local_sizes_inherit_root_visibility_for_typography() {
    let theme = radar_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("RadarInherited").unwrap())
            .with_font_weight(700)
            .unwrap(),
    );
    for (root_size, local_size, visible) in [
        ("0px", -1.0, false),
        ("0px", 0.0, false),
        ("0px", 0.5, true),
        ("16px", -1.0, true),
        ("var(--radar-size)", -1.0, true),
    ] {
        let config = radar_label_size_config(root_size, local_size, local_size);
        let rendered = render_radar_with_theme_requirement(
            "radar-beta\naxis A,B,C\ncurve Alpha{1,2,3}\n",
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config.clone()),
            ),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .count(),
            5,
            "all text terminals, including the empty title, remain checkpointed"
        );
        if local_size == -1.0 {
            let stylesheet = radar_stylesheet(rendered.svg());
            assert!(stylesheet.contains(".radarAxisLabel{font-size:-1px;"));
            assert!(stylesheet.contains(".radarLegendText{text-anchor:start;font-size:-1px;"));
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2, "{config}");
        assert_eq!(evidence.accounted_count(), 2, "{config}");
        assert_eq!(evidence.applied_count(), usize::from(visible), "{config}");
        assert_eq!(
            evidence.theme_residual_count(),
            usize::from(visible),
            "{config}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            if visible { 0 } else { 2 },
            "{config}"
        );
    }
}

#[test]
fn radar_zero_size_title_does_not_certify_title_fill() {
    let theme = radar_title_fill_theme(CanvasPaint::solid("#13579b").unwrap());
    for (title_size, applied) in [
        ("0px", false),
        ("0em", false),
        ("18px", true),
        ("var(--radar-size)", true),
    ] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
                "themeVariables": { "fontSize": title_size }
            })),
        );
        let rendered = render_radar_with_theme_and_engine(RADAR_TEXT_SOURCE, &theme, engine);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(document.descendants().any(|node| {
            node.attribute("class") == Some("radarTitle") && node.text() == Some("Radar heading")
        }));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "{title_size}");
        assert_eq!(evidence.accounted_count(), 1, "{title_size}");
        assert_eq!(
            evidence.applied_count(),
            usize::from(applied),
            "{title_size}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(!applied),
            "{title_size}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{title_size}");
    }
}

#[test]
fn radar_typography_config_ownership_is_property_local() {
    let stack_theme =
        radar_typography_theme(ThemeTextStyle::default().with_font_stack(
            FontStack::single("RadarTyped").expect("valid Radar typed font stack"),
        ));
    for (source, engine, expected_font) in [
        (
            "radar-beta\ntitle Owned radar\naxis A,B,C\ncurve Current{3,4,2}\n",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables": { "fontFamily": "RadarThemeOwner" }
                    }),
                ),
            ),
            "RadarThemeOwner",
        ),
        (
            "radar-beta\ntitle Owned radar\naxis A,B,C\ncurve Current{3,4,2}\n",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "fontFamily": "RadarRootOwner"
                    }),
                ),
            ),
            "RadarRootOwner",
        ),
        (
            concat!(
                "%%{init: {\"themeVariables\": {\"fontFamily\": \"RadarSourceOwner\"}}}%%\n",
                "radar-beta\ntitle Owned radar\naxis A,B,C\ncurve Current{3,4,2}\n",
            ),
            legacy_init_theme_compat_engine(),
            "RadarSourceOwner",
        ),
    ] {
        let rendered = render_radar_with_theme_and_engine(source, &stack_theme, engine);
        let stylesheet = radar_stylesheet(rendered.svg());
        assert!(stylesheet.contains(&format!("font-family:{expected_font};")));
        assert!(!stylesheet.contains("RadarTyped"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }

    let size_theme = radar_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Radar typed font size"),
    );
    for (source, engine, expected_size, typed_applies) in [
        (
            "radar-beta\ntitle Owned radar\naxis A,B,C\ncurve Current{3,4,2}\n",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables": { "fontSize": "22px" }
                    }),
                ),
            ),
            "22px",
            false,
        ),
        (
            concat!(
                "%%{init: {\"themeVariables\": {\"fontSize\": \"23px\"}}}%%\n",
                "radar-beta\ntitle Owned radar\naxis A,B,C\ncurve Current{3,4,2}\n",
            ),
            legacy_init_theme_compat_engine(),
            "23px",
            false,
        ),
        (
            "radar-beta\ntitle Owned radar\naxis A,B,C\ncurve Current{3,4,2}\n",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "fontSize": "31px"
                    }),
                ),
            ),
            "24px",
            true,
        ),
    ] {
        let rendered = render_radar_with_theme_and_engine(source, &size_theme, engine);
        let stylesheet = radar_stylesheet(rendered.svg());
        assert!(stylesheet.contains(&format!("font-size:{expected_size};")));
        assert_eq!(stylesheet.contains("font-size:24px;"), typed_applies);
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), usize::from(typed_applies));
        assert_eq!(evidence.not_applicable_count(), usize::from(!typed_applies));
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn radar_ignored_font_size_config_keeps_typed_size_and_sibling_evidence_independent() {
    let baseline = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let body = "radar-beta\ntitle Radar heading\naxis Alpha,Beta,Gamma\ncurve Current{3,4,2}\n";
    for value in [serde_json::json!(true), serde_json::json!("")] {
        let config = serde_json::json!({"themeVariables": {"fontSize": value}});
        let authored = format!("---\nconfig: {config}\n---\n{body}");
        for (source, site) in [
            (body, config.clone()),
            (authored.as_str(), serde_json::json!({})),
            (
                authored.as_str(),
                serde_json::json!({"themeVariables": {"fontSize": "23px"}}),
            ),
        ] {
            let engine = || {
                MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site.clone()),
                )
            };
            let plain = render_radar_with_theme_and_engine(source, &baseline, engine());
            assert!(radar_stylesheet(plain.svg()).contains(".radarTitle{font-size:16px;"));
            for unsupported_weight in [false, true] {
                let mut typography = ThemeTextStyle::default().with_font_size_px(24.0).unwrap();
                if unsupported_weight {
                    typography = typography.with_font_weight(700).unwrap();
                }
                let theme = radar_typography_theme(typography);
                let rendered = render_radar_with_theme_requirement(
                    source,
                    &theme,
                    engine(),
                    ThemePortabilityRequirement::BestEffort,
                )
                .unwrap();
                let css = radar_stylesheet(rendered.svg());
                assert!(css.contains(".radarTitle{font-size:24px;"));
                assert!(css.contains(".radarAxisLabel{font-size:12px;"));
                assert!(css.contains(".radarLegendText{text-anchor:start;font-size:12px;"));
                assert!(!css.contains("font-weight:700"));
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(
                    evidence.required_count(),
                    1 + usize::from(unsupported_weight)
                );
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.not_applicable_count(), 0);
                assert_eq!(
                    evidence.theme_residual_count(),
                    usize::from(unsupported_weight)
                );
            }
        }
    }
}

#[test]
fn radar_font_size_ownership_ignores_invalid_css_and_preserves_valid_host_values() {
    let theme = radar_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("RadarIndependent").unwrap())
            .with_font_size_px(31.0)
            .unwrap(),
    );
    let body = "radar-beta\ntitle Radar heading\naxis Alpha,Beta,Gamma\ncurve Current{3,4,2}\n";
    for (value, expected_size, typed_size_applies) in [
        (serde_json::json!("auto"), "31px", true),
        (serde_json::json!("red"), "31px", true),
        (serde_json::json!(24), "31px", true),
        (serde_json::json!("24"), "31px", true),
        (serde_json::json!(-2), "31px", true),
        (serde_json::json!("-2px"), "31px", true),
        (serde_json::json!("-2pt"), "31px", true),
        (serde_json::json!("-1vw"), "31px", true),
        (serde_json::json!("-1rem"), "31px", true),
        (serde_json::json!("-1rlh"), "31px", true),
        (serde_json::json!("24furlong"), "31px", true),
        (serde_json::json!("0futureunit"), "31px", true),
        (serde_json::json!("24deg"), "31px", true),
        (serde_json::json!("\"24px\""), "31px", true),
        (serde_json::json!("-1em"), "31px", true),
        (serde_json::json!("-30%"), "31px", true),
        (serde_json::json!("24px 32px"), "31px", true),
        (serde_json::json!(0), "0", false),
        (serde_json::json!("0px"), "0px", false),
        (serde_json::json!("22px"), "22px", false),
        (serde_json::json!("24pt"), "24pt", false),
        (serde_json::json!("2vw"), "2vw", false),
        (serde_json::json!("1cap"), "1cap", false),
        (serde_json::json!("1rem"), "1rem", false),
        (serde_json::json!("1rlh"), "1rlh", false),
        (serde_json::json!("-0.0pt"), "-0.0pt", false),
        (serde_json::json!("1.5em"), "1.5em", false),
        (serde_json::json!("120%"), "120%", false),
        (serde_json::json!("medium"), "medium", false),
        (serde_json::json!("inherit"), "inherit", false),
        (serde_json::json!("initial"), "initial", false),
        (
            serde_json::json!("calc(20px + 1vw)"),
            "calc(20px + 1vw)",
            false,
        ),
        (
            serde_json::json!("clamp(16px, 2vw, 32px)"),
            "clamp(16px, 2vw, 32px)",
            false,
        ),
    ] {
        let config = serde_json::json!({"themeVariables": {"fontSize": value}});
        let authored = format!("---\nconfig: {config}\n---\n{body}");
        for (source, site) in [
            (body, config.clone()),
            (authored.as_str(), serde_json::json!({})),
            (
                authored.as_str(),
                serde_json::json!({"themeVariables": {"fontSize": "23px"}}),
            ),
        ] {
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(site),
            );
            let rendered = render_radar_with_theme_and_engine(source, &theme, engine);
            let css = radar_stylesheet(rendered.svg());
            assert!(
                css.contains(&format!(".radarTitle{{font-size:{expected_size};")),
                "{config}: {css}"
            );
            assert!(css.contains("#radar{font-family:RadarIndependent;"));
            assert!(css.contains(".radarAxisLabel{font-size:12px;"));
            assert!(css.contains(".radarLegendText{text-anchor:start;font-size:12px;"));

            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2, "{config}");
            assert_eq!(evidence.accounted_count(), 2, "{config}");
            assert_eq!(
                evidence.applied_count(),
                1 + usize::from(typed_size_applies),
                "{config}"
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(!typed_size_applies),
                "{config}"
            );
            assert_eq!(evidence.theme_residual_count(), 0, "{config}");
        }
    }
}

#[test]
fn radar_source_font_size_admission_preserves_host_values_and_typed_fallback() {
    let theme = radar_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("RadarIndependent").unwrap())
            .with_font_size_px(31.0)
            .unwrap(),
    );
    let body = "radar-beta\ntitle Radar heading\naxis Alpha,Beta,Gamma\ncurve Current{3,4,2}\n";
    let authored = format!(
        "---\nconfig: {{\"themeVariables\":{{\"fontSize\":\"var(--radar-size)\"}}}}\n---\n{body}"
    );
    for (case, source, site, effective_size, expected_size, typed_size_applies) in [
        (
            "host variable",
            body,
            serde_json::json!({"themeVariables": {"fontSize": "var(--radar-size)"}}),
            "var(--radar-size)",
            "var(--radar-size)",
            false,
        ),
        (
            "rejected source variable preserves typed fallback",
            authored.as_str(),
            serde_json::json!({}),
            "16px",
            "31px",
            true,
        ),
        (
            "rejected source variable preserves static host value",
            authored.as_str(),
            serde_json::json!({"themeVariables": {"fontSize": "23px"}}),
            "23px",
            "23px",
            false,
        ),
        (
            "rejected source variable preserves dynamic host value",
            authored.as_str(),
            serde_json::json!({"themeVariables": {"fontSize": "var(--radar-size)"}}),
            "var(--radar-size)",
            "var(--radar-size)",
            false,
        ),
    ] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(site),
        );
        let parsed = common::install_parse_compatibility(&theme, engine)
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        assert_eq!(
            parsed
                .metadata()
                .effective_config
                .get_str("themeVariables.fontSize"),
            Some(effective_size),
            "{case}"
        );
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| panic!("{case}: {error}"));
        let css = radar_stylesheet(rendered.svg());
        assert!(
            css.contains(&format!(".radarTitle{{font-size:{expected_size};")),
            "{case}: {css}"
        );
        assert!(
            css.contains(&format!(
                "#radar{{font-family:RadarIndependent;font-size:{expected_size};"
            )),
            "{case}: {css}"
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2, "{case}");
        assert_eq!(evidence.accounted_count(), 2, "{case}");
        assert_eq!(
            evidence.applied_count(),
            1 + usize::from(typed_size_applies),
            "{case}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(!typed_size_applies),
            "{case}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
    }
}

#[test]
fn radar_unsupported_typography_sibling_keeps_independent_direct_properties() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(
            FontStack::single("RadarSuppressed").expect("valid suppressed Radar font stack"),
        )
        .with_font_size_px(24.0)
        .expect("valid suppressed Radar font size")
        .with_font_weight(700)
        .expect("valid unsupported Radar font weight");
    let theme = radar_typography_theme(typography);
    let source = "radar-beta\ntitle Suppressed radar\naxis A,B,C\ncurve Current{3,4,2}\n";

    let rendered = render_radar_with_theme_requirement(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("BestEffort renders unsupported Radar typography");
    let stylesheet = radar_stylesheet(rendered.svg());
    assert!(stylesheet.contains("RadarSuppressed"));
    assert!(stylesheet.contains("font-size:24px;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 3);
    assert_eq!(evidence.accounted_count(), 3);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);

    let error = match render_radar_with_theme_requirement(
        source,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::RequirePortable,
    ) {
        Ok(_) => panic!("RequirePortable must reject unsupported Radar typography"),
        Err(error) => error,
    };
    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::RADAR, 1))
    );
}

#[test]
fn radar_base_font_size_is_not_applicable_without_a_visual_title() {
    let size_theme = radar_typography_theme(
        ThemeTextStyle::default()
            .with_font_size_px(24.0)
            .expect("valid Radar applicability font size"),
    );
    let rendered = render_radar_with_theme_and_engine(
        "radar-beta\naccTitle: Accessible only\naxis A,B,C\ncurve VisibleLegend{3,4,2}\n",
        &size_theme,
        MermaidEngine::new(),
    );
    let stylesheet = radar_stylesheet(rendered.svg());
    assert!(rendered.svg().contains(">Accessible only</title>"));
    assert!(stylesheet.contains("#radar .radarAxisLabel{font-size:12px;"));
    assert!(stylesheet.contains("#radar .radarLegendText{text-anchor:start;font-size:12px;"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_direct_font_stack_is_not_limited_by_the_retired_bridge_patch_ceiling() {
    let families = (0..32)
        .map(|index| format!("radar-font-{index}-{}", "x".repeat(180)))
        .collect::<Vec<_>>();
    let font_stack = FontStack::new(families).expect("valid oversized Radar font stack");
    let expected_css = font_stack.as_css();
    assert!(expected_css.len() > 4 * 1024);
    let theme = radar_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let rendered = render_radar_with_theme_and_engine(
        "radar-beta\naxis A,B,C\ncurve Current{3,4,2}\n",
        &theme,
        MermaidEngine::new(),
    );
    let stylesheet = radar_stylesheet(rendered.svg());

    assert!(stylesheet.contains(&format!("font-family:{expected_css};")));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_series_palette_reaches_curve_and_legend_terminal_css() {
    let theme = radar_series_palette_theme(&["#123456", "#abcdef"]);
    let rendered = render_radar_with_theme_and_engine(
        "radar-beta\naxis A,B,C\ncurve One{1,2,3}\ncurve Two{3,2,1}\n",
        &theme,
        MermaidEngine::new(),
    );
    let svg = rendered.svg();

    assert!(svg.contains(
        ".radarCurve-0{color:#123456;fill:#123456;fill-opacity:0.5;stroke:#123456;stroke-width:2;}"
    ));
    assert!(svg.contains(".radarLegendBox-0{fill:#123456;fill-opacity:0.5;stroke:#123456;}"));
    assert!(svg.contains(
        ".radarCurve-1{color:#abcdef;fill:#abcdef;fill-opacity:0.5;stroke:#abcdef;stroke-width:2;}"
    ));
    assert!(svg.contains(".radarLegendBox-1{fill:#abcdef;fill-opacity:0.5;stroke:#abcdef;}"));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_series_palette_supports_transparent_paint_and_cycles() {
    let theme = radar_series_palette_theme(&["transparent", "#00aa00"]);
    let rendered = render_radar_with_theme_and_engine(
        "radar-beta\naxis A,B,C\ncurve One{1,2,3}\ncurve Two{3,2,1}\ncurve Three{2,3,1}\n",
        &theme,
        MermaidEngine::new(),
    );
    let svg = rendered.svg();

    assert!(svg.contains(".radarCurve-0{color:#00000000;fill:#00000000"));
    assert!(svg.contains(".radarLegendBox-0{fill:#00000000"));
    assert!(svg.contains(".radarCurve-2{color:#00000000;fill:#00000000"));
    assert!(svg.contains(".radarLegendBox-2{fill:#00000000"));
    assert!(svg.contains(".radarCurve-1{color:#00aa00;fill:#00aa00"));
    assert!(svg.contains(".radarLegendBox-1{fill:#00aa00"));
}

#[test]
fn radar_explicit_color_scale_slot_outranks_the_typed_series_palette() {
    let theme = radar_series_palette_theme(&["#123456", "#abcdef"]);
    let cases = [
        (
            "radar-beta\naxis A,B,C\ncurve One{1,2,3}\ncurve Two{3,2,1}\n",
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "themeVariables": { "cScale0": "#fedcba" }
                    }),
                ),
            ),
        ),
        (
            concat!(
                "%%{init: {\"themeVariables\": {\"cScale0\": \"#fedcba\"}}}%%\n",
                "radar-beta\naxis A,B,C\ncurve One{1,2,3}\ncurve Two{3,2,1}\n",
            ),
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({
                        "secure": []
                    }),
                ),
            ),
        ),
    ];

    for (source, engine) in cases {
        let rendered = render_radar_with_theme_and_engine(source, &theme, engine);
        let svg = rendered.svg();
        assert!(svg.contains(".radarCurve-0{color:#fedcba;fill:#fedcba"));
        assert!(svg.contains(".radarLegendBox-0{fill:#fedcba"));
        assert!(svg.contains(".radarCurve-1{color:#abcdef;fill:#abcdef"));

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn radar_series_palette_is_applied_without_a_visible_legend() {
    let theme = radar_series_palette_theme(&["#123456"]);
    let rendered = render_radar_with_theme_and_engine(
        "radar-beta\nshowLegend false\naxis A,B,C\ncurve One{1,2,3}\n",
        &theme,
        MermaidEngine::new(),
    );

    assert!(
        rendered
            .svg()
            .contains(".radarCurve-0{color:#123456;fill:#123456")
    );
    assert!(!rendered.svg().contains(r#"class="radarLegendBox-0""#));
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_series_palette_is_not_applicable_without_curves() {
    let theme = radar_series_palette_theme(&["#123456"]);
    let rendered = render_radar_with_theme_and_engine(
        "radar-beta\naxis A,B,C\n",
        &theme,
        MermaidEngine::new(),
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_series_palette_fails_closed_beyond_the_twelve_slot_surface() {
    let theme = radar_series_palette_theme(&["#123456"]);
    let curves = (0..13)
        .map(|index| format!("curve C{index}{{1,2,3}}"))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("radar-beta\naxis A,B,C\n{curves}\n");

    let parsed = common::install_parse_compatibility(&theme, MermaidEngine::new())
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse oversized themed Radar")
        .expect("detect oversized themed Radar")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict oversized Radar session");
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare oversized themed Radar")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
    let error = match rendered {
        Ok(_) => panic!("thirteen Radar curves exceed the direct twelve-slot terminal surface"),
        Err(error) => error,
    };

    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::RADAR, 1))
    );
}

#[test]
fn radar_series_palette_fails_closed_for_a_non_twelve_theme_color_limit() {
    let theme = radar_series_palette_theme(&["#123456"]);
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(serde_json::json!({
            "themeVariables": { "THEME_COLOR_LIMIT": 2 }
        })),
    );
    let parsed = common::install_parse_compatibility(&theme, engine)
        .parse_diagram_for_render_model_sync(
            "radar-beta\naxis A,B,C\ncurve One{1,2,3}\n",
            ParseOptions::strict(),
        )
        .expect("parse nonstandard-limit themed Radar")
        .expect("detect nonstandard-limit themed Radar")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict nonstandard-limit Radar session");
    let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
        .expect("prepare nonstandard-limit themed Radar")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
    let error = match rendered {
        Ok(_) => panic!("a non-twelve Radar color limit must remain an explicit residual"),
        Err(error) => error,
    };

    assert_eq!(
        error.unverified_family_theme(),
        Some((DiagramFamilyId::RADAR, 1))
    );
}

#[test]
fn radar_axis_static_paint_reaches_terminal_without_bridge() {
    use merman_render::diagram_theme::ThemeVariant;
    for stroke in [false, true] {
        for variant in [None, Some(ThemeVariant::Default)] {
            for (paint, css) in [
                (CanvasPaint::solid("#123456").unwrap(), "#123456"),
                (CanvasPaint::Transparent, "transparent"),
            ] {
                let patch = if stroke {
                    ThemeStylePatch::default().with_stroke(paint)
                } else {
                    ThemeStylePatch::default().with_fill(paint)
                };
                let mut rule =
                    ThemeRule::new(ThemeTarget::Axis, patch).for_family(DiagramFamilyId::RADAR);
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let theme = DiagramThemeCompiler::new()
                    .compile(
                        DiagramThemeSpec::new()
                            .with_styles(ThemeRuleSet::default().with_rule(rule)),
                    )
                    .unwrap();
                let rendered = render_radar_with_theme_and_engine(
                    "radar-beta\naxis A,B,C\ncurve Current{3,4,2}\n",
                    &theme,
                    MermaidEngine::new(),
                );
                let stylesheet = radar_stylesheet(rendered.svg());
                assert!(
                    stylesheet.contains(&format!(".radarAxisLine{{stroke:{css};")),
                    "{stylesheet}"
                );
                assert!(
                    stylesheet.contains(&format!(".radarAxisLabel{{font-size:12px;color:{css};")),
                    "{stylesheet}"
                );
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

fn radar_axis_rules(rules: impl IntoIterator<Item = ThemeStylePatch>) -> DiagramTheme {
    let rules = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), |rules, patch| {
            rules.with_rule(
                ThemeRule::new(ThemeTarget::Axis, patch).for_family(DiagramFamilyId::RADAR),
            )
        });
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(rules))
        .unwrap()
}

#[test]
fn radar_axis_stroke_priority_is_independent_of_rule_grouping() {
    let fill = CanvasPaint::solid("#123456").unwrap();
    let stroke = CanvasPaint::solid("#abcdef").unwrap();
    for patches in [
        vec![
            ThemeStylePatch::default()
                .with_fill(fill.clone())
                .with_stroke(stroke.clone()),
        ],
        vec![
            ThemeStylePatch::default().with_stroke(stroke.clone()),
            ThemeStylePatch::default().with_fill(fill.clone()),
        ],
    ] {
        let count = patches.len();
        let theme = radar_axis_rules(patches);
        let rendered = render_radar_with_theme_and_engine(
            "radar-beta\naxis A,B,C\ncurve Current{3,4,2}\n",
            &theme,
            MermaidEngine::new(),
        );
        assert!(radar_stylesheet(rendered.svg()).contains(".radarAxisLine{stroke:#abcdef;"));
        assert!(!radar_stylesheet(rendered.svg()).contains("#123456"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), count - 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn radar_axis_unsupported_stroke_width_remains_residual_in_mixed_and_split_rules() {
    let paint = ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123456").unwrap());
    let width = ThemeStylePatch::default().with_stroke_width(7.0).unwrap();
    for patches in [
        vec![paint.clone().with_stroke_width(7.0).unwrap()],
        vec![paint.clone(), width],
    ] {
        let theme = radar_axis_rules(patches);
        let rendered = render_radar_with_theme_requirement(
            "radar-beta\naxis A,B,C\ncurve Current{3,4,2}\n",
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert!(
            radar_stylesheet(rendered.svg())
                .contains(".radarAxisLine{stroke:#123456;stroke-width:2;}")
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert!(evidence.theme_residual_count() > 0);
        let error = match render_radar_with_theme_requirement(
            "radar-beta\naxis A,B,C\ncurve Current{3,4,2}\n",
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable,
        ) {
            Ok(_) => panic!("unsupported axis width must reject strict rendering"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::RADAR, 1))
        );
    }
}

#[test]
fn radar_axis_configuration_owns_only_its_color_channel() {
    let theme = radar_axis_rules([
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123456").unwrap())
    ]);
    for (config, axis, line, applied) in [
        (
            serde_json::json!({"radar":{"axisColor":"#abcdef"}}),
            "#abcdef",
            "#123456",
            false,
        ),
        (
            serde_json::json!({"themeVariables":{"lineColor":"#abcdef"}}),
            "#123456",
            "#abcdef",
            true,
        ),
        (
            serde_json::json!({"radar":{"axisColor":"#abcdef"},"themeVariables":{"lineColor":"#fedcba"}}),
            "#abcdef",
            "#fedcba",
            false,
        ),
    ] {
        let rendered = render_radar_with_theme_and_engine(
            "radar-beta\naxis A,B,C\ncurve Current{3,4,2}\n",
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            ),
        );
        let css = radar_stylesheet(rendered.svg());
        assert!(
            css.contains(&format!(".radarAxisLine{{stroke:{axis};")),
            "{css}"
        );
        assert!(
            css.contains(&format!(".marker{{fill:{line};stroke:{line};")),
            "{css}"
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(applied));
        assert_eq!(evidence.not_applicable_count(), usize::from(!applied));
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn radar_ignored_config_colors_do_not_own_typed_paint() {
    let styles = [ThemeTarget::Text, ThemeTarget::Title, ThemeTarget::Axis]
        .into_iter()
        .fold(ThemeRuleSet::default(), |styles, target| {
            let paint = CanvasPaint::solid("#123456").unwrap();
            let patch = if target == ThemeTarget::Axis {
                ThemeStylePatch::default().with_stroke(paint)
            } else {
                ThemeStylePatch::default().with_fill(paint)
            };
            styles.with_rule(ThemeRule::new(target, patch).for_family(DiagramFamilyId::RADAR))
        });
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .unwrap();
    let config = serde_json::json!({
        "themeVariables": {
            "textColor": 42,
            "titleColor": 42,
            "radar": { "axisColor": 42 }
        }
    });
    for owner in ["site", "frontmatter", "directive"] {
        let source = match owner {
            "frontmatter" => format!("---\nconfig: {config}\n---\n{RADAR_TEXT_SOURCE}"),
            "directive" => format!("%%{{init: {config}}}%%\n{RADAR_TEXT_SOURCE}"),
            _ => RADAR_TEXT_SOURCE.to_owned(),
        };
        let engine = if owner == "site" {
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config.clone()),
            )
        } else {
            MermaidEngine::new()
        };
        let rendered = render_radar_with_theme_and_engine(&source, &theme, engine);
        let css = radar_stylesheet(rendered.svg());
        assert!(
            css.split_once('}').unwrap().0.ends_with("fill:#123456;"),
            "{owner}: {css}"
        );
        assert!(
            css.contains(".radarTitle{font-size:16px;color:#123456;fill:#123456;"),
            "{owner}: {css}"
        );
        assert!(
            css.contains(".radarAxisLine{stroke:#123456;"),
            "{owner}: {css}"
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 3, "{owner}");
        assert_eq!(evidence.not_applicable_count(), 0, "{owner}");
        assert_eq!(evidence.theme_residual_count(), 0, "{owner}");
    }
}

#[test]
fn radar_ignored_source_scale_slot_allows_scalar_and_palette_winners() {
    let scalar = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::ChartSeries,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#123456").unwrap())
                        .with_stroke(CanvasPaint::solid("#abcdef").unwrap()),
                )),
            ),
        )
        .unwrap();
    let palette = radar_series_palette_theme(&["#123456"]);
    let config = serde_json::json!({"themeVariables": {"cScale0": 42}});
    for source in [
        format!("---\nconfig: {config}\n---\n{RADAR_TEXT_SOURCE}"),
        format!("%%{{init: {config}}}%%\n{RADAR_TEXT_SOURCE}"),
    ] {
        for (theme, color, stroke) in [
            (&scalar, "hsl(240, 100%, 76.2745098039%)", "#abcdef"),
            (&palette, "#123456", "#123456"),
        ] {
            let rendered = render_radar_with_theme_and_engine(&source, theme, MermaidEngine::new());
            let css = radar_stylesheet(rendered.svg());
            assert!(
                css.contains(&format!(
                    ".radarCurve-0{{color:{color};fill:#123456;fill-opacity:0.5;stroke:{stroke};"
                )),
                "{css}"
            );
            assert!(
                css.contains(&format!(
                    ".radarLegendBox-0{{fill:#123456;fill-opacity:0.5;stroke:{stroke};"
                )),
                "{css}"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.not_applicable_count(), 0);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn radar_static_axis_stroke_overrides_ordinal_palette_in_strict_mode() {
    let palette = OrdinalPalette::new([
        ThemeColorValue::parse("#abcdef").unwrap(),
        ThemeColorValue::parse("#fedcba").unwrap(),
    ])
    .unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Axis,
                            ThemeStylePatch::default()
                                .with_stroke(CanvasPaint::solid("#123456").unwrap()),
                        )
                        .for_family(DiagramFamilyId::RADAR),
                    )
                    .with_ordinal_palette(ThemeTarget::Axis, palette),
            ),
        )
        .unwrap();
    let rendered = render_radar_with_theme_and_engine(
        "radar-beta\naxis A,B,C\ncurve Current{3,4,2}\n",
        &theme,
        MermaidEngine::new(),
    );
    assert!(radar_stylesheet(rendered.svg()).contains(".radarAxisLine{stroke:#123456;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

const RADAR_TEXT_SOURCE: &str =
    "radar-beta\ntitle Radar heading\naxis Alpha,Beta,Gamma\ncurve Current{3,4,2}\n";

fn radar_label_size_config(title: &str, axis: f64, legend: f64) -> serde_json::Value {
    serde_json::json!({
        "themeVariables": {
            "fontSize": title,
            "radar": { "axisLabelFontSize": axis, "legendFontSize": legend }
        }
    })
}

fn radar_text_fill_spec(
    paint: CanvasPaint,
    variant: Option<merman_render::diagram_theme::ThemeVariant>,
) -> DiagramThemeSpec {
    let mut rule = ThemeRule::new(
        ThemeTarget::Text,
        ThemeStylePatch::default().with_fill(paint),
    )
    .for_family(DiagramFamilyId::RADAR);
    if let Some(variant) = variant {
        rule = rule.with_variant(variant);
    }
    DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule))
}

#[test]
fn radar_text_static_fill_reaches_labels_and_title_without_bridge() {
    use merman_render::diagram_theme::ThemeVariant;
    for variant in [None, Some(ThemeVariant::Default)] {
        for (paint, expected) in [
            (CanvasPaint::solid("#123456").unwrap(), "#123456"),
            (CanvasPaint::Transparent, "transparent"),
        ] {
            let theme = DiagramThemeCompiler::new()
                .compile(radar_text_fill_spec(paint, variant))
                .unwrap();
            let rendered =
                render_radar_with_theme_and_engine(RADAR_TEXT_SOURCE, &theme, MermaidEngine::new());
            let css = radar_stylesheet(rendered.svg());
            assert!(
                css.split_once('}')
                    .unwrap()
                    .0
                    .ends_with(&format!("fill:{expected};")),
                "{css}"
            );
            assert!(
                css.contains(&format!(
                    ".radarTitle{{font-size:16px;color:{expected};fill:{expected};"
                )),
                "{css}"
            );
            for class in ["radarAxisLabel", "radarLegendText", "radarTitle"] {
                assert!(rendered.svg().contains(&format!("class=\"{class}\"")));
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn radar_zero_size_labels_do_not_certify_text_fill() {
    let theme = DiagramThemeCompiler::new()
        .compile(radar_text_fill_spec(
            CanvasPaint::solid("#123456").unwrap(),
            None,
        ))
        .unwrap();
    for (title_size, axis_size, legend_size, applied) in [
        ("0px", 0.0, 0.0, false),
        ("0%", 0.0, 0.0, false),
        ("0px", -1.0, -1.0, false),
        ("0px", 1e-10, 1e-10, false),
        ("18px", 0.0, 0.0, true),
        ("0px", 12.0, 0.0, true),
        ("0px", 0.0, 12.0, true),
        ("18px", -1.0, -1.0, true),
        ("var(--radar-size)", 0.0, 0.0, true),
        ("calc(0px + 1vw)", 0.0, 0.0, true),
    ] {
        let config = radar_label_size_config(title_size, axis_size, legend_size);
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(config.clone()),
        );
        let rendered = render_radar_with_theme_and_engine(RADAR_TEXT_SOURCE, &theme, engine);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let labels = document
            .descendants()
            .filter(|node| node.has_tag_name("text"))
            .filter_map(|node| node.text())
            .collect::<Vec<_>>();
        assert_eq!(
            labels,
            ["Alpha", "Beta", "Gamma", "Current", "Radar heading"]
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "{config}");
        assert_eq!(evidence.accounted_count(), 1, "{config}");
        assert_eq!(evidence.applied_count(), usize::from(applied), "{config}");
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(!applied),
            "{config}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{config}");
    }
}

#[test]
fn radar_text_fill_uses_the_final_typed_title_size_and_source_overrides() {
    let typography = ThemeTextStyle::default().with_font_size_px(31.0).unwrap();
    let theme = DiagramThemeCompiler::new()
        .compile(
            radar_text_fill_spec(CanvasPaint::solid("#123456").unwrap(), None).with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::RADAR, typography),
            ),
        )
        .unwrap();
    for (configured_size, final_size, applied) in [
        ("bogus", "31px", 2),
        ("0px", "0px", 0),
        ("18px", "18px", 1),
        ("var(--radar-size)", "var(--radar-size)", 1),
    ] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                radar_label_size_config(configured_size, 0.0, 0.0),
            ),
        );
        let rendered = render_radar_with_theme_and_engine(RADAR_TEXT_SOURCE, &theme, engine);
        assert!(radar_stylesheet(rendered.svg()).contains(&format!(
            ".radarTitle{{font-size:{final_size};color:#123456;fill:#123456;"
        )));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2, "{configured_size}");
        assert_eq!(evidence.accounted_count(), 2, "{configured_size}");
        assert_eq!(evidence.applied_count(), applied, "{configured_size}");
        assert_eq!(
            evidence.not_applicable_count(),
            2 - applied,
            "{configured_size}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{configured_size}");
    }
}

#[test]
fn radar_zero_size_legend_fill_cannot_borrow_other_visible_text_or_swatches() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Legend,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#13579b").unwrap()),
            ))),
        )
        .unwrap();
    for (legend_size, applied) in [(0.0, false), (1e-10, false), (12.0, true)] {
        let config = radar_label_size_config("18px", 12.0, legend_size);
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(config),
        );
        let rendered = render_radar_with_theme_and_engine(RADAR_TEXT_SOURCE, &theme, engine);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let label = document
            .descendants()
            .find(|node| node.attribute("class") == Some("radarLegendText"))
            .unwrap();
        assert_eq!(label.text(), Some("Current"));
        assert_eq!(label.attribute("fill"), applied.then_some("#13579b"));
        assert!(document.descendants().any(|node| {
            node.has_tag_name("rect") && node.attribute("class") == Some("radarLegendBox-0")
        }));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "{legend_size}");
        assert_eq!(evidence.accounted_count(), 1, "{legend_size}");
        assert_eq!(
            evidence.applied_count(),
            usize::from(applied),
            "{legend_size}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(!applied),
            "{legend_size}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{legend_size}");
    }
}

#[test]
fn radar_zero_size_text_keeps_axis_and_series_paint_consumers() {
    let styles = [
        (ThemeTarget::Text, "#123456"),
        (ThemeTarget::Legend, "#13579b"),
        (ThemeTarget::Axis, "#2468ac"),
        (ThemeTarget::ChartSeries, "#abcdef"),
    ]
    .into_iter()
    .fold(ThemeRuleSet::default(), |styles, (target, color)| {
        styles.with_rule(ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap()),
        ))
    });
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .unwrap();
    let engine = MermaidEngine::new().with_site_config(
        merman_core::presentation::SourcePresentationConfig::from_value(radar_label_size_config(
            "0px", 0.0, 0.0,
        )),
    );
    let rendered = render_radar_with_theme_and_engine(RADAR_TEXT_SOURCE, &theme, engine);
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert_eq!(
        document
            .descendants()
            .filter(|node| node.attribute("class") == Some("radarAxisLine"))
            .count(),
        3
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarCurve-0"))
    );
    assert!(
        document
            .descendants()
            .any(|node| node.attribute("class") == Some("radarLegendBox-0"))
    );
    let css = radar_stylesheet(rendered.svg());
    assert!(css.contains(".radarAxisLine{stroke:#2468ac;"));
    assert!(
        css.split_once(".radarCurve-0{")
            .unwrap()
            .1
            .split_once('}')
            .unwrap()
            .0
            .contains("fill:#abcdef;")
    );
    assert!(css.contains(".radarLegendBox-0{fill:#abcdef;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 4);
    assert_eq!(evidence.accounted_count(), 4);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_hidden_text_ordinals_are_not_residual_consumers() {
    use merman_render::diagram_theme::OrdinalSelector;

    for target in [ThemeTarget::Text, ThemeTarget::Legend] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            target,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap()),
                        )
                        .with_ordinal(OrdinalSelector::Exact(1)),
                    ),
                ),
            )
            .unwrap();
        for visible in [false, true] {
            let title_size = if target == ThemeTarget::Text && visible {
                "18px"
            } else {
                "0px"
            };
            let legend_size = if target == ThemeTarget::Legend && visible {
                12.0
            } else {
                0.0
            };
            let engine = MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    radar_label_size_config(title_size, 0.0, legend_size),
                ),
            );
            let rendered = render_radar_with_theme_requirement(
                RADAR_TEXT_SOURCE,
                &theme,
                engine,
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1, "{target:?}: {visible}");
            assert_eq!(evidence.accounted_count(), 1, "{target:?}: {visible}");
            assert_eq!(evidence.applied_count(), 0, "{target:?}: {visible}");
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(!visible),
                "{target:?}: {visible}"
            );
            assert_eq!(
                evidence.theme_residual_count(),
                usize::from(visible),
                "{target:?}: {visible}"
            );
        }
    }
}

fn radar_text_theme_with_rules(extra: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let spec = radar_text_fill_spec(CanvasPaint::solid("#123456").unwrap(), None);
    let styles = extra
        .into_iter()
        .fold(spec.styles().clone(), ThemeRuleSet::with_rule);
    DiagramThemeCompiler::new()
        .compile(spec.with_styles(styles))
        .unwrap()
}

#[test]
fn radar_text_and_title_configuration_own_their_actual_channels() {
    let theme = radar_text_theme_with_rules([]);
    for (config, base, title, applied) in [
        (
            serde_json::json!({"themeVariables":{"textColor":"#abcdef"}}),
            "#abcdef",
            "#abcdef",
            false,
        ),
        (
            serde_json::json!({"theme":"base","themeVariables":{"textColor":"#abcdef"}}),
            "#abcdef",
            "#123456",
            true,
        ),
        (
            serde_json::json!({"themeVariables":{"titleColor":"#abcdef"}}),
            "#123456",
            "#abcdef",
            true,
        ),
        (
            serde_json::json!({"themeVariables":{"textColor":"#abcdef","titleColor":"#fedcba"}}),
            "#abcdef",
            "#fedcba",
            false,
        ),
    ] {
        let rendered = render_radar_with_theme_and_engine(
            RADAR_TEXT_SOURCE,
            &theme,
            MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(config),
            ),
        );
        let css = radar_stylesheet(rendered.svg());
        assert!(
            css.split_once('}')
                .unwrap()
                .0
                .ends_with(&format!("fill:{base};")),
            "{css}"
        );
        assert!(
            css.contains(&format!(
                ".radarTitle{{font-size:16px;color:{title};fill:{title};"
            )),
            "{css}"
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(applied));
        assert_eq!(evidence.not_applicable_count(), usize::from(!applied));
        assert_eq!(evidence.theme_residual_count(), 0);
    }

    let source = format!(
        "%%{{init: {{\"theme\": \"base\", \"themeVariables\": {{\"textColor\": \"#abcdef\"}}}}}}%%\n{RADAR_TEXT_SOURCE}"
    );
    let rendered = render_radar_with_theme_and_engine(&source, &theme, MermaidEngine::new());
    let css = radar_stylesheet(rendered.svg());
    assert!(css.split_once('}').unwrap().0.ends_with("fill:#abcdef;"));
    assert!(css.contains(".radarTitle{font-size:16px;color:rgb(8.5000000002, 5.7500000001, 0);fill:rgb(8.5000000002, 5.7500000001, 0);"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_explicit_title_fill_overrides_only_the_text_title_fallback() {
    let theme = radar_text_theme_with_rules([ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap()),
    )
    .for_family(DiagramFamilyId::RADAR)]);
    let rendered =
        render_radar_with_theme_and_engine(RADAR_TEXT_SOURCE, &theme, MermaidEngine::new());
    let css = radar_stylesheet(rendered.svg());
    assert!(css.split_once('}').unwrap().0.ends_with("fill:#123456;"));
    assert!(css.contains(".radarTitle{font-size:16px;color:#abcdef;fill:#abcdef;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_clear_title_fill_blocks_text_fallback_but_not_label_paint() {
    use merman_render::diagram_theme::Specified;
    let mut patch = ThemeStylePatch::default();
    patch.paint.fill = Specified::Clear;
    let theme = radar_text_theme_with_rules([
        ThemeRule::new(ThemeTarget::Title, patch).for_family(DiagramFamilyId::RADAR)
    ]);
    let rendered = render_radar_with_theme_requirement(
        RADAR_TEXT_SOURCE,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    let css = radar_stylesheet(rendered.svg());
    assert!(css.split_once('}').unwrap().0.ends_with("fill:#123456;"));
    assert!(!css.contains(".radarTitle{font-size:16px;color:#123456;fill:#123456;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert!(
        render_radar_with_theme_requirement(
            RADAR_TEXT_SOURCE,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::RequirePortable
        )
        .is_err()
    );
}

#[test]
fn radar_text_ordinal_uses_all_visible_occurrences_and_preserves_residuals() {
    use merman_render::diagram_theme::OrdinalSelector;
    // Three axis labels, one legend label, then the non-empty title.
    for ordinal in [2, 4, 5, 6] {
        let rule = ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap()),
        )
        .for_family(DiagramFamilyId::RADAR)
        .with_ordinal(OrdinalSelector::Exact(ordinal));
        let theme = radar_text_theme_with_rules([rule]);
        let rendered = render_radar_with_theme_requirement(
            RADAR_TEXT_SOURCE,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1, "ordinal={ordinal}");
        assert_eq!(
            evidence.theme_residual_count(),
            usize::from(ordinal <= 5),
            "ordinal={ordinal}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(ordinal > 5),
            "ordinal={ordinal}"
        );
        assert_eq!(
            render_radar_with_theme_requirement(
                RADAR_TEXT_SOURCE,
                &theme,
                MermaidEngine::new(),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_ok(),
            ordinal > 5,
            "ordinal={ordinal}"
        );
    }
}

#[test]
fn radar_text_unsupported_sibling_is_not_certified_by_an_applied_fill() {
    let fill = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
    let width = ThemeStylePatch::default().with_stroke_width(7.0).unwrap();
    for patches in [
        vec![fill.clone().with_stroke_width(7.0).unwrap()],
        vec![fill, width],
    ] {
        let styles = patches
            .into_iter()
            .fold(ThemeRuleSet::default(), |styles, patch| {
                styles.with_rule(
                    ThemeRule::new(ThemeTarget::Text, patch).for_family(DiagramFamilyId::RADAR),
                )
            });
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .unwrap();
        let rendered = render_radar_with_theme_requirement(
            RADAR_TEXT_SOURCE,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        assert!(
            radar_stylesheet(rendered.svg())
                .split_once('}')
                .unwrap()
                .0
                .ends_with("fill:#123456;")
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.theme_residual_count(), 1);
        assert!(
            render_radar_with_theme_requirement(
                RADAR_TEXT_SOURCE,
                &theme,
                MermaidEngine::new(),
                ThemePortabilityRequirement::RequirePortable
            )
            .is_err()
        );
    }
}

#[test]
fn radar_text_title_fallback_overrides_an_unused_title_palette() {
    let spec = radar_text_fill_spec(CanvasPaint::solid("#123456").unwrap(), None);
    let styles = spec.styles().clone().with_ordinal_palette(
        ThemeTarget::Title,
        OrdinalPalette::new([ThemeColorValue::parse("#abcdef").unwrap()]).unwrap(),
    );
    let theme = DiagramThemeCompiler::new()
        .compile(spec.with_styles(styles))
        .unwrap();
    let rendered =
        render_radar_with_theme_and_engine(RADAR_TEXT_SOURCE, &theme, MermaidEngine::new());
    assert!(
        radar_stylesheet(rendered.svg())
            .contains(".radarTitle{font-size:16px;color:#123456;fill:#123456;")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_non_fill_title_rule_does_not_capture_the_text_fallback() {
    let theme = radar_text_theme_with_rules([ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default().with_stroke_width(7.0).unwrap(),
    )
    .for_family(DiagramFamilyId::RADAR)]);
    let rendered = render_radar_with_theme_requirement(
        RADAR_TEXT_SOURCE,
        &theme,
        MermaidEngine::new(),
        ThemePortabilityRequirement::BestEffort,
    )
    .unwrap();
    assert!(
        radar_stylesheet(rendered.svg())
            .contains(".radarTitle{font-size:16px;color:#123456;fill:#123456;")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn radar_legend_fill_paints_real_labels_and_leaves_swatches_unchanged() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Legend,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            ))),
        )
        .unwrap();
    let baseline = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    for (source, swatch, visible_label) in [
        ("radar-beta\naxis A,B,C\ncurve Current{1,2,3}\n", true, true),
        // A swatch is not a consumer of Legend label paint.
        (
            "radar-beta\naxis A,B,C\ncurve Current[\"\"]{1,2,3}\n",
            true,
            false,
        ),
        (
            "radar-beta\nshowLegend false\naxis A,B,C\ncurve Current{1,2,3}\n",
            false,
            false,
        ),
        ("radar-beta\naxis A,B,C\n", false, false),
    ] {
        let rendered = render_radar_with_theme_requirement(
            source,
            &theme,
            MermaidEngine::new(),
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap();
        let unthemed = render_radar_with_theme_and_engine(source, &baseline, MermaidEngine::new());
        assert_eq!(
            radar_stylesheet(rendered.svg()),
            radar_stylesheet(unthemed.svg())
        );
        if visible_label {
            assert!(
                rendered
                    .svg()
                    .contains(r##"class="radarLegendText" fill="#123456">Current</text>"##)
            );
        } else {
            assert_eq!(rendered.svg(), unthemed.svg());
        }
        assert_eq!(
            rendered.svg().contains(r#"class="radarLegendBox-0""#),
            swatch
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), usize::from(visible_label));
        assert_eq!(evidence.not_applicable_count(), usize::from(!visible_label));
        assert_eq!(evidence.theme_residual_count(), 0);
        assert!(
            render_radar_with_theme_requirement(
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
fn radar_legend_selectors_and_other_paint_owners_keep_dedicated_residuals() {
    use merman_render::diagram_theme::{OrdinalSelector, ThemeVariant};

    let fill = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap());
    let styles = ThemeRuleSet::default()
        .with_rule(ThemeRule::new(ThemeTarget::Legend, fill.clone()))
        .with_rule(
            ThemeRule::new(ThemeTarget::Legend, fill.clone())
                .with_ordinal(OrdinalSelector::Exact(1)),
        )
        .with_rule(
            ThemeRule::new(ThemeTarget::Legend, fill.clone())
                .with_ordinal(OrdinalSelector::Exact(2)),
        )
        .with_rule(ThemeRule::new(ThemeTarget::Legend, fill).with_variant(ThemeVariant::Warning))
        .with_rule(ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#13579b").unwrap()),
        ))
        .with_ordinal_palette(
            ThemeTarget::ChartSeries,
            OrdinalPalette::new([ThemeColorValue::parse("#2468ac").unwrap()]).unwrap(),
        );
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .unwrap();
    let rendered = render_radar_with_theme_requirement(
        "---\nconfig:\n  themeVariables:\n    cScale0: '#445566'\n---\nradar-beta\naxis A,B,C\ncurve Current{1,2,3}\n",
        &theme, MermaidEngine::new(), ThemePortabilityRequirement::BestEffort,
    ).unwrap();
    assert!(radar_stylesheet(rendered.svg()).contains(".radarLegendBox-0{fill:#445566;"));
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 6);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 4);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn radar_presets_account_for_legend_consumers_without_incomplete_evidence() {
    for descriptor in merman_render::diagram_theme::theme_preset_descriptors() {
        let theme = DiagramThemeCompiler::new()
            .compile_preset(descriptor.preset())
            .unwrap();
        let rendered = render_radar_with_theme_requirement(
            RADAR_TEXT_SOURCE,
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
fn radar_legend_static_paint_respects_text_source_and_clear_baseline() {
    use merman_render::diagram_theme::{Specified, ThemeVariant};

    for (fill, expected) in [
        (
            Specified::Value(CanvasPaint::solid("#123456").unwrap()),
            "#123456",
        ),
        (Specified::Value(CanvasPaint::Transparent), "transparent"),
        (Specified::Clear, "#333"),
    ] {
        for variant in [None, Some(ThemeVariant::Default)] {
            let mut patch = ThemeStylePatch::default();
            patch.paint.fill = fill.clone();
            let mut legend = ThemeRule::new(ThemeTarget::Legend, patch);
            if let Some(variant) = variant {
                legend = legend.with_variant(variant);
            }
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default()
                            .with_rule(ThemeRule::new(
                                ThemeTarget::Text,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#abcdef").unwrap()),
                            ))
                            .with_rule(legend),
                    ),
                )
                .unwrap();
            for source_text in [false, true] {
                let config = if source_text {
                    serde_json::json!({"themeVariables":{"textColor":"#445566", "cScale0":"#778899"}})
                } else {
                    serde_json::json!({"themeVariables":{"cScale0":"#778899"}})
                };
                let rendered = render_radar_with_theme_and_engine(
                    "radar-beta\naxis A,B,C\ncurve Current{1,2,3}\n",
                    &theme,
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(config),
                    ),
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let label = document
                    .descendants()
                    .find(|node| node.attribute("class") == Some("radarLegendText"))
                    .unwrap();
                assert_eq!(
                    label.attribute("fill"),
                    if source_text { None } else { Some(expected) }
                );
                assert!(
                    radar_stylesheet(rendered.svg()).contains(".radarLegendBox-0{fill:#778899;")
                );
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 2);
                assert_eq!(evidence.accounted_count(), 2);
                assert_eq!(evidence.applied_count(), if source_text { 0 } else { 2 });
                assert_eq!(
                    evidence.not_applicable_count(),
                    if source_text { 2 } else { 0 }
                );
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn radar_series_scalar_paints_and_palette_have_independent_property_winners() {
    use merman_render::diagram_theme::{Specified, ThemeVariant};

    let source = "radar-beta\naxis A,B,C\ncurve Current{1,2,3}\n";
    let baseline = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let baseline_svg = render_radar_with_theme_and_engine(source, &baseline, MermaidEngine::new());
    let baseline_color = radar_stylesheet(baseline_svg.svg())
        .split_once(".radarCurve-0{color:")
        .unwrap()
        .1
        .split(';')
        .next()
        .unwrap();
    for stroke_property in [false, true] {
        for (specified, expected) in [
            (
                Specified::Value(CanvasPaint::solid("#123456").unwrap()),
                "#123456",
            ),
            (Specified::Value(CanvasPaint::Transparent), "transparent"),
            (Specified::Clear, baseline_color),
        ] {
            let mut patch = ThemeStylePatch::default();
            if stroke_property {
                patch.stroke.paint = specified;
            } else {
                patch.paint.fill = specified;
            }
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default()
                            .with_rule(
                                ThemeRule::new(ThemeTarget::ChartSeries, patch)
                                    .with_variant(ThemeVariant::Default),
                            )
                            .with_ordinal_palette(
                                ThemeTarget::ChartSeries,
                                OrdinalPalette::new([ThemeColorValue::parse("#2468ac").unwrap()])
                                    .unwrap(),
                            ),
                    ),
                )
                .unwrap();
            for source_owner in [false, true] {
                let engine = if source_owner {
                    MermaidEngine::new().with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(
                            serde_json::json!({"themeVariables":{"cScale0":"#445566"}}),
                        ),
                    )
                } else {
                    MermaidEngine::new()
                };
                let rendered = render_radar_with_theme_and_engine(source, &theme, engine);
                let css = radar_stylesheet(rendered.svg());
                let curve = css
                    .split_once(".radarCurve-0{")
                    .unwrap()
                    .1
                    .split('}')
                    .next()
                    .unwrap();
                let (fill, stroke) = if source_owner {
                    ("#445566", "#445566")
                } else if stroke_property {
                    ("#2468ac", expected)
                } else {
                    (expected, "#2468ac")
                };
                assert!(
                    curve
                        .split(';')
                        .any(|property| property == format!("fill:{fill}")),
                    "{curve}"
                );
                assert!(
                    curve
                        .split(';')
                        .any(|property| property == format!("stroke:{stroke}")),
                    "{curve}"
                );
                assert!(css.contains(&format!(".radarLegendBox-0{{fill:{fill};")));
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.required_count(), 2);
                assert_eq!(evidence.accounted_count(), 2);
                assert_eq!(evidence.applied_count(), if source_owner { 0 } else { 2 });
                assert_eq!(
                    evidence.not_applicable_count(),
                    if source_owner { 2 } else { 0 }
                );
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn radar_series_scalar_paint_is_not_limited_to_twelve_palette_slots() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(
                    ThemeTarget::ChartSeries,
                    ThemeStylePatch::default()
                        .with_fill(CanvasPaint::solid("#123456").unwrap())
                        .with_stroke(CanvasPaint::solid("#abcdef").unwrap()),
                )),
            ),
        )
        .unwrap();
    let mut source = "radar-beta\naxis A,B,C\n".to_owned();
    for index in 0..13 {
        source.push_str(&format!("curve Curve{index}{{1,2,3}}\n"));
    }
    let rendered = render_radar_with_theme_and_engine(&source, &theme, MermaidEngine::new());
    assert!(
        radar_stylesheet(rendered.svg()).contains(".radarCurve-12{fill:#123456;stroke:#abcdef;}")
    );
    assert!(
        radar_stylesheet(rendered.svg())
            .contains(".radarLegendBox-12{fill:#123456;stroke:#abcdef;}")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn radar_series_and_legend_unsupported_siblings_retain_residuals() {
    for target in [ThemeTarget::ChartSeries, ThemeTarget::Legend] {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        target,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap())
                            .with_stroke_width(7.0)
                            .unwrap(),
                    )),
                ),
            )
            .unwrap();
        for (source, visible) in [
            ("radar-beta\naxis A,B,C\ncurve Current{1,2,3}\n", true),
            ("radar-beta\naxis A,B,C\n", false),
        ] {
            let rendered = render_radar_with_theme_requirement(
                source,
                &theme,
                MermaidEngine::new(),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.accounted_count(), 1);
            assert_eq!(evidence.theme_residual_count(), usize::from(visible));
            assert_eq!(evidence.not_applicable_count(), usize::from(!visible));
            assert_eq!(
                render_radar_with_theme_requirement(
                    source,
                    &theme,
                    MermaidEngine::new(),
                    ThemePortabilityRequirement::RequirePortable
                )
                .is_ok(),
                !visible
            );
        }
    }
}

#[test]
fn radar_unsupported_palettes_and_effects_are_accounted_on_real_domains() {
    use merman_render::diagram_theme::{
        DiagramEffectSet, EffectBinding, EffectGraph, EffectInput, EffectPrimitive,
    };
    for target in [ThemeTarget::Legend, ThemeTarget::ChartSeries] {
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
        let mut spec = DiagramThemeSpec::new().with_effects(effects);
        let required = if target == ThemeTarget::Legend {
            spec = spec.with_styles(ThemeRuleSet::default().with_ordinal_palette(
                target,
                OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap(),
            ));
            2
        } else {
            1
        };
        let theme = DiagramThemeCompiler::new().compile(spec).unwrap();
        for (source, visible) in [
            ("radar-beta\naxis A,B,C\ncurve Current{1,2,3}\n", true),
            ("radar-beta\naxis A,B,C\n", false),
        ] {
            let rendered = render_radar_with_theme_requirement(
                source,
                &theme,
                MermaidEngine::new(),
                ThemePortabilityRequirement::BestEffort,
            )
            .unwrap();
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), required);
            assert_eq!(evidence.accounted_count(), required);
            assert_eq!(evidence.applied_count(), 0);
            assert_eq!(
                evidence.theme_residual_count(),
                if visible { required } else { 0 }
            );
            assert_eq!(
                evidence.not_applicable_count(),
                if visible { 0 } else { required }
            );
        }
    }
}

#[test]
fn radar_empty_theme_preserves_unthemed_svg_bytes() {
    let source = "radar-beta\naxis A,B,C\ncurve Current{1,2,3}\n";
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
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let themed = render_radar_with_theme_and_engine(source, &theme, MermaidEngine::new());
    assert_eq!(plain.svg(), themed.svg());
}

#[test]
fn radar_svg_preserves_db_accessibility_text() {
    for (title, description, expected_title, expected_description) in [
        (
            "Accessible <!--a11y-->",
            "First\n<!--a11y-->",
            Some("Accessible "),
            Some("First\n"),
        ),
        (
            "<!--a11y-->\u{0085}<!--a11y-->",
            "<!--a11y-->\u{0085}<!--a11y-->",
            Some("\u{0085}"),
            Some("\u{0085}"),
        ),
        ("<!--a11y-->", "<!--a11y-->", None, None),
    ] {
        let source = format!(
            "radar-beta\naccTitle: {title}\naccDescr {{\n{description}\n}}\naxis A,B,C\ncurve Metric{{1,2,3}}\n"
        );
        let svg = render_radar_svg_from_text(&source);
        common::assert_svg_accessibility(&svg, expected_title, expected_description);
    }
}
