mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
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
    let palette =
        OrdinalPalette::new(colors.iter().map(|color| {
            ThemeColorValue::parse(*color).expect("valid Radar series palette color")
        }))
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
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Radar")
        .expect("detect themed Radar");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Radar session");
    family::prepare(parsed, &LayoutOptions::default(), session)?
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn render_radar_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
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
        .expect("detect radar");
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact =
        family::prepare(parsed, &LayoutOptions::default(), session).expect("layout radar");

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
        .expect("detect Radar resource-bound fixture");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Radar resource-bound session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
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
        Engine::new(),
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
                        Engine::new().with_site_config(MermaidConfig::from_value(
                            serde_json::json!({"themeVariables":{"titleColor":"#445566"}}),
                        ))
                    } else {
                        Engine::new()
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
        Engine::new(),
    );

    for path in [
        "fontFamily",
        "themeVariables.fontFamily",
        "themeVariables.fontSize",
    ] {
        assert!(
            !merman_core::__private::fallback_overlay_owns_path(
                &rendered.metadata().effective_config,
                path,
            ),
            "Radar typed typography must not retain fallback ownership of {path}"
        );
    }
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn radar_digit_leading_font_family_is_quoted_and_portable() {
    let theme = radar_typography_theme(ThemeTextStyle::default().with_font_stack(
        FontStack::single("123Radar").expect("valid digit-leading Radar font stack"),
    ));
    let rendered = render_radar_with_theme_and_engine(
        "radar-beta\ntitle Quoted radar\naxis A,B,C\ncurve Current{3,4,2}\n",
        &theme,
        Engine::new(),
    );

    let stylesheet = radar_stylesheet(rendered.svg());
    assert!(stylesheet.contains("font-family:\"123Radar\";"));
    assert!(!stylesheet.contains("font-family:123Radar;"));

    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "fontFamily": "RadarThemeOwner" }
            }))),
            "RadarThemeOwner",
        ),
        (
            "radar-beta\ntitle Owned radar\naxis A,B,C\ncurve Current{3,4,2}\n",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "fontFamily": "RadarRootOwner"
            }))),
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
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "fontSize": "22px" }
            }))),
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
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "fontSize": "31px"
            }))),
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
        Engine::new(),
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
        Engine::new(),
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
        Engine::new(),
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
        Engine::new(),
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
        Engine::new(),
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
        Engine::new(),
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
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "themeVariables": { "cScale0": "#fedcba" }
            }))),
        ),
        (
            concat!(
                "%%{init: {\"themeVariables\": {\"cScale0\": \"#fedcba\"}}}%%\n",
                "radar-beta\naxis A,B,C\ncurve One{1,2,3}\ncurve Two{3,2,1}\n",
            ),
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "secure": []
            }))),
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
        Engine::new(),
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
    let rendered =
        render_radar_with_theme_and_engine("radar-beta\naxis A,B,C\n", &theme, Engine::new());

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

    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .expect("parse oversized themed Radar")
        .expect("detect oversized themed Radar");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict oversized Radar session");
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
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
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "THEME_COLOR_LIMIT": 2 }
    })));
    let parsed = merman_render::__private::install_parse_compatibility(&theme, engine)
        .parse_diagram_for_render_model_sync(
            "radar-beta\naxis A,B,C\ncurve One{1,2,3}\n",
            ParseOptions::strict(),
        )
        .expect("parse nonstandard-limit themed Radar")
        .expect("detect nonstandard-limit themed Radar");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .expect("begin strict nonstandard-limit Radar session");
    let rendered = family::prepare(parsed, &LayoutOptions::default(), session)
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
                    Engine::new(),
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
                assert_eq!(evidence.compatibility_residual_count(), 0);
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
            Engine::new(),
        );
        assert!(radar_stylesheet(rendered.svg()).contains(".radarAxisLine{stroke:#abcdef;"));
        assert!(!radar_stylesheet(rendered.svg()).contains("#123456"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), count - 1);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert_eq!(evidence.compatibility_residual_count(), 0);
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
            Engine::new(),
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
        assert_eq!(evidence.compatibility_residual_count(), 0);
        let error = match render_radar_with_theme_requirement(
            "radar-beta\naxis A,B,C\ncurve Current{3,4,2}\n",
            &theme,
            Engine::new(),
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
            Engine::new().with_site_config(MermaidConfig::from_value(config)),
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
        assert_eq!(evidence.compatibility_residual_count(), 0);
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
        Engine::new(),
    );
    assert!(radar_stylesheet(rendered.svg()).contains(".radarAxisLine{stroke:#123456;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}
