mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
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

fn render_radar_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::RenderedFamilySvg {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Radar")
        .expect("detect themed Radar");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(theme)
        .expect("begin strict portable Radar session");
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed Radar")
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render themed Radar")
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
