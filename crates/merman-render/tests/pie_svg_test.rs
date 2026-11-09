mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::ParseOptions;
use merman_model::family::DiagramFamilyId;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec,
    EffectBinding, EffectGraph, EffectInput, EffectPrimitive, FontStack, OrdinalPalette,
    OrdinalSelector, ThemeColorValue, ThemePortabilityRequirement, ThemePreset, ThemeRule,
    ThemeRuleSet, ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::{RenderEnvironment, TextMeasurementPolicy};
use merman_render::family;
use merman_render::model::PieDiagramLayout;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{
    Error, LayoutOptions, RenderResourcePolicy, ResourceLimitCause, ResourceLimitId,
    ResourceLimitPhase,
};

fn layout_pie_from_text(text: &str) -> PieDiagramLayout {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");
    let projection = artifact.layout_json().expect("serialize Pie layout");
    serde_json::from_value(projection["layout"]["PieDiagram"].clone())
        .expect("Pie layout projection")
}

fn render_pie_from_text(text: &str) -> String {
    render_pie_from_text_with_options(text, &SvgRenderOptions::default())
}

fn render_pie_from_text_with_options(text: &str, options: &SvgRenderOptions) -> String {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("svg render ok")
        .svg()
        .to_owned()
}

#[test]
fn pie_multiline_string_reaches_visible_legend_and_preserves_slice_values() {
    let svg = render_pie_from_text("pie showData\n\"First\nSecond\": 10\n\"Other\": 5\n");
    let document = roxmltree::Document::parse(&svg).expect("valid multiline Pie SVG");
    let legends: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("legend"))
        .map(|legend| {
            legend
                .children()
                .find(|node| node.has_tag_name("text"))
                .unwrap()
                .text()
                .unwrap()
        })
        .collect();
    assert_eq!(legends, ["First\nSecond [10]", "Other [5]"]);
    let percentages: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("text") && node.attribute("class") == Some("slice"))
        .map(|node| node.text().unwrap())
        .collect();
    assert_eq!(percentages, ["67%", "33%"]);
    assert_eq!(
        document
            .descendants()
            .filter(|node| {
                node.has_tag_name("path") && node.attribute("class") == Some("pieCircle")
            })
            .count(),
        2
    );
}

#[test]
fn pie_configured_text_sizes_preserve_css_and_expand_the_horizontal_viewport() {
    let diagram = "pie title TitleTitleTitleTitle\n  \"LegendLegendLegend\" : 1\n";
    let baseline = render_pie_from_text(diagram);
    let enlarged = render_pie_from_text(&format!(
        "%%{{init: {{\"themeVariables\": {{\"pieTitleTextSize\": \"60px\", \"pieLegendTextSize\": \"40px\"}}}}}}%%\n{diagram}"
    ));
    let width = |svg: &str| {
        let document = roxmltree::Document::parse(svg).unwrap();
        document
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .nth(2)
            .unwrap()
            .parse::<f64>()
            .unwrap()
    };
    assert!(enlarged.contains("font-size:60px;"));
    assert!(enlarged.contains("font-size:40px;"));
    assert!(width(&enlarged) > width(&baseline));
}

#[test]
fn pie_absolute_role_sizes_preserve_css_and_share_the_px_viewport() {
    let diagram = "pie title Typography sizing comparison comparison\n  \"WideLegendWideLegendWideLegendWideLegend\" : 1\n";
    for (key, selector) in [
        ("pieTitleTextSize", "#merman .pieTitleText"),
        ("pieLegendTextSize", "#merman .legend text"),
    ] {
        for (css, pixels, from_site) in [
            ("24pt", "32px", false),
            ("2PC", "32px", false),
            ("1in", "96px", false),
            ("2.54cm", "96px", false),
            ("25.4mm", "96px", false),
            ("101.6q", "96px", false),
            (r"24p\74", "32px", false),
            (r"24p\74 !important", "32px", true),
        ] {
            let render = |size| {
                let config = serde_json::json!({"themeVariables": {key: size}});
                let (source, engine) = if from_site {
                    (
                        diagram.to_owned(),
                        merman_core::MermaidEngine::new().with_site_config(
                            merman_core::presentation::SourcePresentationConfig::from_value(config),
                        ),
                    )
                } else {
                    (
                        format!("---\nconfig: {config}\n---\n{diagram}"),
                        merman_core::MermaidEngine::new(),
                    )
                };
                let parsed = engine
                    .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation();
                assert_eq!(
                    parsed
                        .metadata()
                        .effective_config
                        .get_str(&format!("themeVariables.{key}")),
                    Some(size),
                    "the admitted size must reach measurement, site={from_site}"
                );
                let session = RenderEnvironment::deterministic().begin_session().unwrap();
                family::prepare(&parsed, &LayoutOptions::default(), session)
                    .unwrap()
                    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                    .unwrap()
                    .svg()
                    .to_owned()
            };
            let actual = render(css);
            let expected = render(pixels);
            assert_eq!(
                pie_stylesheet_property_values(&actual, selector, "font-size"),
                [css]
            );
            let viewport = |svg: &str| {
                let document = roxmltree::Document::parse(svg).unwrap();
                document
                    .root_element()
                    .attribute("viewBox")
                    .unwrap()
                    .split_whitespace()
                    .map(|value| value.parse::<f64>().unwrap())
                    .collect::<Vec<_>>()
            };
            for (actual, expected) in viewport(&actual).into_iter().zip(viewport(&expected)) {
                assert!(
                    (actual - expected).abs() < 1e-9,
                    "{key}/{css}: {actual} != {expected}"
                );
            }
        }
    }
}

#[test]
fn pie_outer_stroke_config_reaches_layout_circle_and_stylesheet() {
    assert_eq!(
        pie_outer_circle_radius(&render_pie_from_text("pie\n\"A\": 1\n")),
        186.0
    );
    for (width, css, expected_radius) in [
        ("0", "0", 185.0),
        ("20.5", "20.5", 195.25),
        ("'20.5px'", "20.5px", 195.0),
        ("'5em'", "5em", 187.5),
        ("'100%'", "100%", 235.0),
        ("'calc(1px + 1em)'", "calc(1px + 1em)", 186.0),
        ("'2px'", "2px", 186.0),
    ] {
        let source = format!(
            "---\nconfig:\n  themeVariables:\n    pieOuterStrokeWidth: {width}\n---\npie\n\"A\": 1\n"
        );
        let layout = layout_pie_from_text(&source);
        let svg = render_pie_from_text(&source);
        assert_eq!(layout.outer_radius, expected_radius, "{width}");
        assert_eq!(pie_outer_circle_radius(&svg), expected_radius, "{width}");
        assert_eq!(
            pie_stylesheet_property_values(&svg, "#merman .pieOuterCircle", "stroke-width"),
            [css],
            "the authored CSS spelling must survive its separate geometry consumer"
        );
    }
}

#[test]
fn pie_title_remains_valid_xml_when_control_character_removal_joins_cdata_terminators() {
    let svg = render_pie_from_text("pie title Ω]]\u{1f}>β >\n  \"Alpha\" : 1\n");
    let document = roxmltree::Document::parse(&svg).expect("the Pie producer must emit valid XML");
    let title = document
        .descendants()
        .find(|node| node.attribute("class") == Some("pieTitleText"))
        .expect("visible Pie title");
    assert_eq!(title.text(), Some("Ω]]>β >"));
    assert!(svg.contains("Ω]]&gt;β >"));
}

fn render_pie_with_theme(text: &str, theme: &DiagramTheme) -> family::RenderedFamilySvg {
    try_render_pie_with_theme_requirement(text, theme, ThemePortabilityRequirement::BestEffort)
        .expect("render themed Pie")
}

fn try_render_pie_with_theme_requirement(
    text: &str,
    theme: &DiagramTheme,
    portability: ThemePortabilityRequirement,
) -> merman_render::Result<family::RenderedFamilySvg> {
    let parsed = common::install_parse_compatibility(theme, legacy_init_theme_compat_engine())
        .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
        .expect("parse themed Pie")
        .expect("detect themed Pie")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Pie session");

    family::prepare(&parsed, &LayoutOptions::default(), session)?
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn pie_slice_palette(colors: &[&str]) -> OrdinalPalette {
    OrdinalPalette::new(
        colors
            .iter()
            .map(|color| ThemeColorValue::parse(color).expect("valid Pie palette color")),
    )
    .expect("non-empty Pie palette")
}

fn pie_slice_palette_theme(colors: &[&str]) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_ordinal_palette(ThemeTarget::PieSlice, pie_slice_palette(colors)),
            ),
        )
        .expect("compile Pie palette theme")
}

fn pie_slice_fill_and_palette_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::PieSlice,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#111827").expect("valid static Pie fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::PIE),
                    )
                    .with_ordinal_palette(
                        ThemeTarget::PieSlice,
                        pie_slice_palette(&["#ef4444", "#22c55e"]),
                    ),
            ),
        )
        .expect("compile Pie fill and palette theme")
}

fn pie_slice_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::PieSlice,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::PIE),
                ),
            ),
        )
        .expect("compile Pie fill theme")
}

fn pie_slice_default_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::PieSlice,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::PIE)
                    .with_variant(ThemeVariant::Default),
                ),
            ),
        )
        .expect("compile explicit-Default Pie fill theme")
}

fn pie_slice_effect_theme() -> DiagramTheme {
    let effect_id = "pie-slice-blur";
    let effects = DiagramEffectSet::default()
        .with_graph(
            EffectGraph::new(
                effect_id,
                [EffectPrimitive::GaussianBlur {
                    input: EffectInput::SourceGraphic,
                    std_deviation: 1.0,
                }],
            )
            .expect("valid Pie effect graph"),
        )
        .expect("unique Pie effect graph")
        .with_binding(
            EffectBinding::new(ThemeTarget::PieSlice, effect_id).expect("valid Pie effect binding"),
        )
        .expect("unique Pie effect binding");
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_effects(effects))
        .expect("compile Pie effect theme")
}

fn pie_slice_fill_with_later_ordinal_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::PieSlice,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#111827").expect("valid static Pie fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::PIE),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::PieSlice,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#7c3aed").expect("valid ordinal Pie fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::PIE)
                        .with_ordinal(OrdinalSelector::exact(2).expect("valid Pie exact ordinal")),
                    ),
            ),
        )
        .expect("compile Pie static and ordinal fill theme")
}

fn pie_slice_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::PieSlice,
                        ThemeStylePatch::default().with_stroke(stroke),
                    )
                    .for_family(DiagramFamilyId::PIE),
                ),
            ),
        )
        .expect("compile Pie stroke theme")
}

fn pie_slice_default_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::PieSlice,
                        ThemeStylePatch::default().with_stroke(stroke),
                    )
                    .for_family(DiagramFamilyId::PIE)
                    .with_variant(ThemeVariant::Default),
                ),
            ),
        )
        .expect("compile explicit-Default Pie stroke theme")
}

fn pie_title_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Title,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::PIE),
                ),
            ),
        )
        .expect("compile Pie title fill theme")
}

fn pie_title_default_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Title,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::PIE)
                    .with_variant(ThemeVariant::Default),
                ),
            ),
        )
        .expect("compile explicit-Default Pie title fill theme")
}

fn pie_text_default_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::PIE)
                    .with_variant(ThemeVariant::Default),
                ),
            ),
        )
        .expect("compile explicit-Default Pie text fill theme")
}

fn pie_title_and_text_fill_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#111827").expect("valid Pie title fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::PIE),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#b45309").expect("valid Pie text fill"),
                            ),
                        )
                        .for_family(DiagramFamilyId::PIE),
                    ),
            ),
        )
        .expect("compile mixed Pie title and text theme")
}

fn pie_typography_theme(typography: ThemeTextStyle) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::PIE, typography),
        ))
        .expect("compile Pie typography theme")
}

fn pie_slice_ordinal_stroke_theme(selector: OrdinalSelector) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::PieSlice,
                        ThemeStylePatch::default().with_stroke(
                            CanvasPaint::solid("#7c3aed").expect("valid ordinal Pie stroke"),
                        ),
                    )
                    .for_family(DiagramFamilyId::PIE)
                    .with_ordinal(selector),
                ),
            ),
        )
        .expect("compile Pie ordinal stroke theme")
}

fn pie_terminal_fills(svg: &str) -> (Vec<String>, Vec<String>) {
    let document = roxmltree::Document::parse(svg).expect("valid themed Pie SVG");
    let slice_fills = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "pieCircle")
                })
        })
        .map(|node| node.attribute("fill").expect("Pie slice fill").to_string())
        .collect();
    let legend_styles = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("rect")
                && node.parent().is_some_and(|parent| {
                    parent.has_tag_name("g") && parent.attribute("class") == Some("legend")
                })
        })
        .map(|node| {
            node.attribute("style")
                .expect("Pie legend style")
                .to_string()
        })
        .collect();
    (slice_fills, legend_styles)
}

fn pie_stylesheet_property_values(svg: &str, selector: &str, property: &str) -> Vec<String> {
    let document = roxmltree::Document::parse(svg).expect("valid themed Pie SVG");
    document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .flat_map(|css| css.split('}'))
        .filter_map(|rule| rule.split_once('{'))
        .filter(|(candidate, _)| candidate.trim() == selector)
        .flat_map(|(_, declarations)| declarations.split(';'))
        .filter_map(|declaration| declaration.split_once(':'))
        .filter(|(name, _)| name.trim() == property)
        .map(|(_, value)| value.trim().to_string())
        .collect()
}

#[test]
fn pie_static_title_fill_is_verified_from_scoped_stylesheet_and_visible_title() {
    for (fill, expected_css) in [
        (
            CanvasPaint::solid("#2563eb").expect("valid solid Pie title fill"),
            "#2563eb",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = pie_title_fill_theme(fill);
        let rendered = try_render_pie_with_theme_requirement(
            "pie title Release distribution\n  \"Alpha\" : 1\n",
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("unqualified Pie title fill must be portable");
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .pieTitleText", "fill"),
            [expected_css]
        );
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Pie SVG");
        assert_eq!(
            document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("text")
                        && node.attribute("class") == Some("pieTitleText")
                        && node.text().is_some_and(|text| !text.trim().is_empty())
                })
                .count(),
            1,
            "the typed title route must bind exactly one visible title terminal"
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(
            evidence.status(),
            merman_render::__private::FamilyEvidenceStatus::Verified
        );
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn pie_static_title_fill_respects_source_ownership_and_absent_title() {
    let theme = pie_title_fill_theme(CanvasPaint::solid("#2563eb").expect("valid Pie title fill"));
    let source_owned = try_render_pie_with_theme_requirement(
        r##"%%{init: {"themeVariables": {"pieTitleTextColor": "#b45309"}}}%%
pie title Source title
  "Alpha" : 1
"##,
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("source-owned Pie title must be NotApplicable");
    assert_eq!(
        pie_stylesheet_property_values(source_owned.svg(), "#merman .pieTitleText", "fill"),
        ["#b45309"]
    );
    let source_evidence =
        merman_render::__private::family_evidence(source_owned.into_completion().report());
    assert_eq!(source_evidence.required_count(), 1);
    assert_eq!(source_evidence.applied_count(), 0);
    assert_eq!(source_evidence.not_applicable_count(), 1);
    assert_eq!(source_evidence.theme_residual_count(), 0);

    let absent = try_render_pie_with_theme_requirement(
        "pie\n  \"Alpha\" : 1\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("an absent Pie title must be NotApplicable");
    let absent_evidence =
        merman_render::__private::family_evidence(absent.into_completion().report());
    assert_eq!(absent_evidence.required_count(), 1);
    assert_eq!(absent_evidence.applied_count(), 0);
    assert_eq!(absent_evidence.not_applicable_count(), 1);
    assert_eq!(absent_evidence.theme_residual_count(), 0);
}

#[test]
fn pie_static_title_fill_tracks_frontmatter_titles() {
    let theme = pie_title_fill_theme(CanvasPaint::solid("#2563eb").expect("valid Pie title fill"));
    let rendered = try_render_pie_with_theme_requirement(
        "---\ntitle: Frontmatter title\n---\npie\n  \"Alpha\" : 1\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("frontmatter Pie title must be a typed terminal");
    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman .pieTitleText", "fill"),
        ["#2563eb"]
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
}

#[test]
fn pie_title_fill_is_independent_from_legacy_text_fill() {
    let theme = pie_title_and_text_fill_theme();
    let rendered = try_render_pie_with_theme_requirement(
        "pie title Mixed ownership\n  \"Alpha\" : 1\n",
        &theme,
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("best-effort mixed Pie title and text render");
    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman .pieTitleText", "fill"),
        ["#111827"]
    );
    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman .slice", "fill"),
        ["#b45309"]
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn pie_typed_font_stack_reaches_layout_stylesheet_and_terminal_evidence() {
    let font_stack =
        FontStack::new(["Pie Typed", "sans-serif"]).expect("valid Pie typed font stack");
    let expected_font = font_stack.as_css();
    let theme = pie_typography_theme(ThemeTextStyle::default().with_font_stack(font_stack));
    let rendered = try_render_pie_with_theme_requirement(
        "pie title Release distribution\n  \"Alpha\" : 1\n  \"Beta\" : 2\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("typed Pie font stack must be portable");

    for selector in [
        "#merman .pieTitleText",
        "#merman .slice",
        "#merman .legend text",
    ] {
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), selector, "font-family"),
            std::slice::from_ref(&expected_font),
            "selector={selector}"
        );
    }

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(
        evidence.status(),
        merman_render::__private::FamilyEvidenceStatus::Verified
    );
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn pie_typed_font_stack_respects_source_font_family_ownership() {
    let theme = pie_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Pie Typed").expect("valid Pie typed font stack")),
    );
    let rendered = try_render_pie_with_theme_requirement(
        r##"%%{init: {"themeVariables": {"fontFamily": "Source Pie"}}}%%
pie title Source font
  "Alpha" : 1
"##,
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("source-owned Pie font family must be portable");

    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman .pieTitleText", "font-family"),
        ["Source Pie"]
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn pie_base_font_size_is_portable_but_does_not_replace_fixed_role_sizes() {
    for explicit in [false, true] {
        let style = if explicit {
            ThemeTextStyle::default().with_font_size_px(16.0).unwrap()
        } else {
            ThemeTextStyle::default()
        };
        let theme = pie_typography_theme(style);
        let rendered = try_render_pie_with_theme_requirement(
            "pie\n \"Alpha\" : 1\n",
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("fixed Pie roles do not consume the base font size");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), usize::from(explicit));
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), usize::from(explicit));
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn pie_font_size_only_keeps_fixed_roles_and_records_no_false_application() {
    let typography = ThemeTextStyle::default()
        .with_font_size_px(24.0)
        .expect("valid Pie base font size");
    let theme = pie_typography_theme(typography);
    let rendered = try_render_pie_with_theme_requirement(
        "pie title Fixed role sizes\n  \"Alpha\" : 1\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("fixed roles make the base-size request not applicable");
    for (selector, size) in [
        ("#merman .pieTitleText", "25px"),
        ("#merman .slice", "17px"),
        ("#merman .legend text", "17px"),
    ] {
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), selector, "font-size"),
            [size]
        );
    }
    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman", "font-size"),
        ["24px"]
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn pie_font_stack_and_base_size_have_independent_terminal_ownership() {
    let font_stack = FontStack::single("Pie Mixed").expect("valid mixed Pie font stack");
    let expected_font = font_stack.as_css();
    let theme = pie_typography_theme(
        ThemeTextStyle::default()
            .with_font_stack(font_stack)
            .with_font_size_px(24.0)
            .unwrap(),
    );
    let rendered = try_render_pie_with_theme_requirement(
        "pie title Mixed typography\n  \"Alpha\" : 1\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("font stack applies independently of fixed role sizes");
    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman .pieTitleText", "font-family"),
        [expected_font]
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn pie_explicit_default_title_fill_is_typed_and_verified() {
    let theme = pie_title_default_fill_theme(
        CanvasPaint::solid("#2563eb").expect("valid explicit-Default Pie title fill"),
    );
    let rendered = try_render_pie_with_theme_requirement(
        "pie title Default title\n  \"Alpha\" : 1\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("explicit-Default Pie title fill must be portable");
    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman .pieTitleText", "fill"),
        ["#2563eb"]
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn pie_explicit_default_text_fill_is_typed_and_verified() {
    let theme = pie_text_default_fill_theme(
        CanvasPaint::solid("#b45309").expect("valid explicit-Default Pie text fill"),
    );
    let rendered = try_render_pie_with_theme_requirement(
        "pie\n  \"Alpha\" : 1\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("explicit-Default Pie text fill must be portable");
    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman .slice", "fill"),
        ["#b45309"]
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Pie SVG");
    assert_eq!(
        document
            .descendants()
            .filter(|node| {
                node.has_tag_name("text")
                    && node.attribute("class") == Some("slice")
                    && node.text().is_some_and(|text| !text.trim().is_empty())
            })
            .count(),
        1,
        "the typed section-text route must bind exactly one visible terminal"
    );
    drop(document);
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

fn pie_rules_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let styles = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), |styles, rule| {
            styles.with_rule(rule.for_family(DiagramFamilyId::PIE))
        });
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .expect("compile Pie rule evidence fixture")
}

fn pie_fill_rule(target: ThemeTarget, color: &str) -> ThemeRule {
    ThemeRule::new(
        target,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid(color).expect("valid Pie rule evidence color")),
    )
}

#[test]
fn pie_shadowed_default_rules_follow_property_winners() {
    let source = "pie title Winners\n  \"Alpha\" : 1\n  \"Beta\" : 1\n";
    for target in [ThemeTarget::Text, ThemeTarget::Title, ThemeTarget::PieSlice] {
        for default_first in [true, false] {
            let earlier = pie_fill_rule(target, "#123456");
            let later = pie_fill_rule(target, "#abcdef");
            let rules = if default_first {
                [earlier.with_variant(ThemeVariant::Default), later]
            } else {
                [earlier, later.with_variant(ThemeVariant::Default)]
            };
            let theme = pie_rules_theme(rules);
            let rendered = try_render_pie_with_theme_requirement(
                source,
                &theme,
                ThemePortabilityRequirement::RequirePortable,
            )
            .expect("a fully shadowed Default rule must be NotApplicable");
            match target {
                ThemeTarget::Text | ThemeTarget::Title => {
                    let selector = if target == ThemeTarget::Text {
                        "#merman .slice"
                    } else {
                        "#merman .pieTitleText"
                    };
                    assert_eq!(
                        pie_stylesheet_property_values(rendered.svg(), selector, "fill"),
                        ["#abcdef"]
                    );
                }
                ThemeTarget::PieSlice => {
                    assert_eq!(pie_terminal_fills(rendered.svg()).0, ["#abcdef", "#abcdef"]);
                }
                _ => unreachable!("the fixture only contains Pie fill targets"),
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2, "{target:?}/{default_first}");
            assert_eq!(evidence.accounted_count(), 2, "{target:?}/{default_first}");
            assert_eq!(evidence.applied_count(), 1, "{target:?}/{default_first}");
            assert_eq!(
                evidence.not_applicable_count(),
                1,
                "{target:?}/{default_first}"
            );
            assert_eq!(
                evidence.theme_residual_count(),
                0,
                "{target:?}/{default_first}"
            );
        }
    }
}

#[test]
fn pie_text_ordinals_are_residual_when_they_win_any_occurrence() {
    let source = "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n";
    for selector in [
        OrdinalSelector::exact(1).unwrap(),
        OrdinalSelector::exact(2).unwrap(),
        OrdinalSelector::cycle(2, 0).unwrap(),
    ] {
        for variant in [None, Some(ThemeVariant::Default)] {
            let mut rule = pie_fill_rule(ThemeTarget::Text, "#123456").with_ordinal(selector);
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = pie_rules_theme([rule]);
            let rendered = render_pie_with_theme(source, &theme);
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1, "{selector:?}/{variant:?}");
            assert_eq!(evidence.accounted_count(), 1, "{selector:?}/{variant:?}");
            assert_eq!(evidence.applied_count(), 0, "{selector:?}/{variant:?}");
            assert_eq!(
                evidence.not_applicable_count(),
                0,
                "{selector:?}/{variant:?}"
            );
            assert_eq!(
                evidence.theme_residual_count(),
                1,
                "{selector:?}/{variant:?}"
            );
            let error = match try_render_pie_with_theme_requirement(
                source,
                &theme,
                ThemePortabilityRequirement::RequirePortable,
            ) {
                Ok(_) => panic!("a winning Pie text ordinal must fail closed"),
                Err(error) => error,
            };
            assert_eq!(
                error.unverified_family_theme(),
                Some((DiagramFamilyId::PIE, 1))
            );
        }
    }
}

#[test]
fn pie_text_ordinals_respect_shadowing_and_absent_occurrences() {
    let source = "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n";
    for (source, rule, later_static) in [
        (
            source,
            pie_fill_rule(ThemeTarget::Text, "#123456")
                .with_ordinal(OrdinalSelector::exact(3).unwrap()),
            false,
        ),
        (
            "pie\n",
            pie_fill_rule(ThemeTarget::Text, "#123456")
                .with_ordinal(OrdinalSelector::exact(1).unwrap()),
            false,
        ),
        (
            source,
            pie_fill_rule(ThemeTarget::Text, "#123456")
                .with_ordinal(OrdinalSelector::exact(2).unwrap())
                .with_variant(ThemeVariant::Warning),
            false,
        ),
        (
            source,
            pie_fill_rule(ThemeTarget::Text, "#123456")
                .with_ordinal(OrdinalSelector::exact(2).unwrap())
                .with_variant(ThemeVariant::Default),
            true,
        ),
    ] {
        let mut rules = vec![rule];
        if later_static {
            rules.push(pie_fill_rule(ThemeTarget::Text, "#abcdef"));
        }
        let theme = pie_rules_theme(rules);
        let rendered = try_render_pie_with_theme_requirement(
            source,
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("a shadowed or absent Pie text ordinal must be NotApplicable");
        if later_static {
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), "#merman .slice", "fill"),
                ["#abcdef"]
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1 + usize::from(later_static));
        assert_eq!(evidence.accounted_count(), evidence.required_count());
        assert_eq!(evidence.applied_count(), usize::from(later_static));
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn pie_text_ordinal_does_not_select_shared_static_css() {
    let source = "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n";
    for (selector, static_wins) in [
        (OrdinalSelector::exact(1).unwrap(), true),
        (OrdinalSelector::exact(2).unwrap(), true),
        (OrdinalSelector::cycle(1, 0).unwrap(), false),
    ] {
        let theme = pie_rules_theme([
            pie_fill_rule(ThemeTarget::Text, "#abcdef").with_variant(ThemeVariant::Default),
            pie_fill_rule(ThemeTarget::Text, "#123456").with_ordinal(selector),
        ]);
        let rendered = render_pie_with_theme(source, &theme);
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .slice", "fill"),
            ["#abcdef"],
            "an unsupported ordinal cannot replace the shared CSS fallback"
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2, "{selector:?}");
        assert_eq!(evidence.accounted_count(), 2, "{selector:?}");
        assert_eq!(
            evidence.applied_count(),
            usize::from(static_wins),
            "{selector:?}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(!static_wins),
            "{selector:?}"
        );
        assert_eq!(evidence.theme_residual_count(), 1, "{selector:?}");
    }
}

#[test]
fn pie_text_source_fill_suppresses_only_the_owned_rule_property() {
    let source = r##"%%{init: {"themeVariables": {"pieSectionTextColor": "#abcdef"}}}%%
pie
  "Alpha" : 1
  "Beta" : 1
"##;
    for ordinal in [None, Some(OrdinalSelector::exact(2).unwrap())] {
        for typography_sibling in [false, true] {
            let mut patch =
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
            if typography_sibling {
                patch.typography.font_size_px =
                    merman_render::diagram_theme::Specified::Value(24.0);
            }
            let mut rule =
                ThemeRule::new(ThemeTarget::Text, patch).with_variant(ThemeVariant::Default);
            if let Some(ordinal) = ordinal {
                rule = rule.with_ordinal(ordinal);
            }
            let theme = pie_rules_theme([rule]);
            let rendered = render_pie_with_theme(source, &theme);
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), "#merman .slice", "fill"),
                ["#abcdef"]
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.accounted_count(), 1);
            let size_applies = typography_sibling && ordinal.is_none();
            let size_residual = typography_sibling && ordinal.is_some();
            assert_eq!(evidence.applied_count(), usize::from(size_applies));
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(!typography_sibling)
            );
            assert_eq!(evidence.theme_residual_count(), usize::from(size_residual));
        }
    }
}

#[test]
fn pie_typed_palette_is_verified_from_every_terminal_slice_and_legend_swatch() {
    let theme = pie_slice_palette_theme(&["transparent", "#22c55e"]);
    let rendered = render_pie_with_theme("pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n", &theme);
    let (slice_fills, legend_styles) = pie_terminal_fills(rendered.svg());
    assert_eq!(slice_fills, ["#00000000", "#22c55e"]);
    assert_eq!(
        legend_styles,
        [
            "fill: rgba(0, 0, 0, 0); stroke: rgba(0, 0, 0, 0);",
            "fill: rgb(34, 197, 94); stroke: rgb(34, 197, 94);",
        ]
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(
        evidence.status(),
        merman_render::__private::FamilyEvidenceStatus::Verified
    );
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

fn try_render_pie_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse Pie resource-bound fixture")
        .expect("detect Pie resource-bound fixture")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Pie resource-bound session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn pie_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    for source in [
        r#"pie showData title Bounded distribution
  "Alpha" : 3
  "Beta" : 2
  "Gamma" : 1
"#,
        "---\nconfig:\n  themeVariables:\n    pieOuterStrokeWidth: 20px\n---\npie\n\"A\": 1\n",
        "---\nconfig:\n  pie:\n    legendPosition: left\n  themeVariables:\n    pieTitleTextSize: 80px\n---\npie showData title WWWWWWWWWWWWWWWW\n\"A\": 1\n",
    ] {
        let baseline = try_render_pie_svg_with_resource_policy(
            source,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .expect("render the unbounded Pie baseline");
        let exact_bytes = baseline.len();
        assert!(exact_bytes > 1, "Pie fixture must emit a non-empty SVG");

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
            .expect("valid exact Pie SVG byte ceiling");
        let exact = try_render_pie_svg_with_resource_policy(source, exact_policy)
            .expect("the exact Pie family SVG byte ceiling must succeed");
        assert_eq!(exact.as_bytes(), baseline.as_bytes());

        let below_exact = exact_bytes - 1;
        let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
            .expect("valid below-exact Pie SVG byte ceiling");
        let error = try_render_pie_svg_with_resource_policy(source, below_policy)
            .expect_err("one byte below the Pie family SVG size must fail");
        let merman_render::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected Pie MaxSvgBytes rejection, got {error}");
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
}

fn render_pie_with_svg_limit(
    text: &str,
    diagram_id: &str,
    maximum: usize,
) -> merman_render::Result<String> {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected")
        .into_presentation();
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .expect("begin bounded Pie session");
    let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some(diagram_id.to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .map(|rendered| rendered.svg().to_owned())
}

fn render_pie_error_with_svg_limit(text: &str, diagram_id: &str, maximum: usize) -> Error {
    match render_pie_with_svg_limit(text, diagram_id, maximum) {
        Ok(_) => panic!("bounded Pie SVG must exceed the test budget"),
        Err(error) => error,
    }
}

fn root_viewbox_width(svg: &str) -> f64 {
    let start = svg.find(r#"viewBox=""#).expect("viewBox start") + r#"viewBox=""#.len();
    let end = svg[start..].find('"').expect("viewBox end") + start;
    svg[start..end]
        .split_whitespace()
        .nth(2)
        .expect("viewBox width")
        .parse::<f64>()
        .expect("viewBox width parses")
}

fn pie_content_translate(svg: &str) -> (f64, f64) {
    let document = roxmltree::Document::parse(svg).expect("valid Pie SVG");
    let centered = document
        .root_element()
        .children()
        .find(|node| node.is_element() && node.attribute("transform").is_some())
        .expect("centered pie group");
    let transform = centered
        .children()
        .find(|node| node.is_element() && node.attribute("transform").is_some())
        .and_then(|node| node.attribute("transform"))
        .expect("translated pie content group");
    let values = transform
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(')'))
        .expect("translate transform")
        .split(',')
        .map(|value| value.parse::<f64>().expect("numeric translate component"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 2, "two-dimensional translate transform");
    (values[0], values[1])
}

fn pie_outer_circle_radius(svg: &str) -> f64 {
    let document = roxmltree::Document::parse(svg).expect("valid Pie SVG");
    document
        .descendants()
        .find(|node| node.attribute("class") == Some("pieOuterCircle"))
        .and_then(|node| node.attribute("r"))
        .expect("outer circle radius")
        .parse()
        .expect("numeric outer circle radius")
}

#[test]
fn pie_slices_follow_input_order_like_mermaid_11_16() {
    let layout = layout_pie_from_text(
        r#"pie
  "A" : 10
  "B" : 100
  "C" : 50
"#,
    );

    let labels: Vec<&str> = layout
        .slices
        .iter()
        .map(|slice| slice.label.as_str())
        .collect();

    assert_eq!(labels, vec!["A", "B", "C"]);
}

#[test]
fn pie_large_diagram_id_stylesheet_is_preflighted_at_exact_n() {
    let source = "pie\n  \"A\" : 1\n";
    // Host-special characters exercise the public normalization boundary. Counting and
    // materialization share the same writer over the resulting CSS-safe ID rather than assuming
    // a byte slope.
    let diagram_id = "diagram<&:.id".repeat(128);
    let full_svg = render_pie_from_text_with_options(
        source,
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.clone()),
            ..SvgRenderOptions::default()
        },
    );
    let projected_svg_bytes = full_svg.len();
    let n_minus_one_maximum = projected_svg_bytes
        .checked_sub(1)
        .expect("Pie SVG projection must not be empty");

    let Error::ResourceLimitExceeded(n_minus_one) =
        render_pie_error_with_svg_limit(source, &diagram_id, n_minus_one_maximum)
    else {
        panic!("expected N-1 Pie CSS byte projection error");
    };
    assert_eq!(n_minus_one.cause, ResourceLimitCause::Ceiling);
    assert_eq!(n_minus_one.phase, ResourceLimitPhase::SvgOutput);
    assert_eq!(n_minus_one.limit, ResourceLimitId::MaxSvgBytes.as_str());
    assert_eq!(n_minus_one.actual, projected_svg_bytes);
    assert_eq!(n_minus_one.max, n_minus_one_maximum);

    let exact = render_pie_with_svg_limit(source, &diagram_id, projected_svg_bytes)
        .expect("the exact Pie CSS byte projection must succeed");
    assert_eq!(exact, full_svg);
}

#[test]
fn pie_chart_content_is_grouped_before_title_and_legend_like_mermaid_11_16() {
    let svg = render_pie_from_text(
        r#"pie
  "A" : 3
  "B" : 2
"#,
    );

    assert!(
        svg.contains(
            r#"<g transform="translate(225,225)"><g><circle cx="0" cy="0" r="186" class="pieOuterCircle"/>"#
        ),
        "pie geometry should start in its own attribute-free group: {svg}"
    );
    assert!(
        svg.contains(
            r#">40%</text></g><text x="0" y="-200" class="pieTitleText"/><g class="legend""#
        ),
        "the pie group should close before the sibling title and legend nodes: {svg}"
    );
}

#[test]
fn pie_frontmatter_title_renders_unless_the_body_overrides_it() {
    let frontmatter_svg = render_pie_from_text(
        r#"---
title: Frontmatter pie
---
pie
  "A" : 1
"#,
    );
    assert!(
        frontmatter_svg.contains(r#"class="pieTitleText">Frontmatter pie</text>"#),
        "frontmatter title should render when the Pie body has none: {frontmatter_svg}"
    );

    let body_svg = render_pie_from_text(
        r#"---
title: Frontmatter pie
---
pie title Body pie
  "A" : 1
"#,
    );
    assert!(body_svg.contains(r#"class="pieTitleText">Body pie</text>"#));
    assert!(!body_svg.contains(">Frontmatter pie</text>"));
}

#[test]
fn pie_frontmatter_title_preserves_common_db_boundary_whitespace() {
    for title in ["  Frontmatter pie  ", "\u{a0}Frontmatter pie\u{a0}"] {
        let source = format!("---\ntitle: \"{title}\"\n---\npie\n  \"A\" : 1\n");
        let svg = render_pie_from_text(&source);

        assert!(
            svg.contains(&format!(r#"class="pieTitleText">{title}</text>"#)),
            "frontmatter title should be emitted exactly: {svg}"
        );
    }
}

#[test]
fn pie_hidden_slices_still_reserve_color_domain_slots() {
    let layout = layout_pie_from_text(
        r#"pie
  "A" : 10
  "B" : 100
  "C" : 0.1
  "D" : 50
"#,
    );

    let slices: Vec<(&str, &str)> = layout
        .slices
        .iter()
        .map(|slice| (slice.label.as_str(), slice.fill.as_str()))
        .collect();

    assert_eq!(
        slices,
        vec![
            ("A", "#ECECFF"),
            ("B", "#ffffde"),
            ("D", "hsl(240, 100%, 86.2745098039%)")
        ]
    );
}

#[test]
fn pie_redux_dark_primary_override_derives_first_slice_color() {
    let layout = layout_pie_from_text(
        r##"%%{init: {"theme": "redux-dark", "themeVariables": {"primaryColor": "#123456"}}}%%
pie
  "A" : 10
  "B" : 20
"##,
    );

    let first = layout.slices.first().expect("first slice");
    assert_eq!(first.fill, "#123456");
}

#[test]
fn pie_text_position_config_moves_slice_labels() {
    let layout = layout_pie_from_text(
        r#"%%{init: {"pie": {"textPosition": 0.5}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    let first = layout
        .slices
        .iter()
        .find(|slice| slice.label == "A")
        .expect("slice A exists");

    assert!((first.text_x - 92.5).abs() < 1e-9);
    assert!(first.text_y.abs() < 1e-9);
}

#[test]
fn pie_full_circles_keep_closed_outer_and_reversed_inner_rings() {
    for (hole, expected) in [
        (0.0, "M0,-185A185,185,0,1,1,0,185A185,185,0,1,1,0,-185Z"),
        (
            0.4,
            "M0,-185A185,185,0,1,1,0,185A185,185,0,1,1,0,-185M0,-74A74,74,0,1,0,0,74A74,74,0,1,0,0,-74Z",
        ),
    ] {
        for sections in ["  \"A\" : 1\n", "  \"A\" : 1000\n  \"Filtered\" : 1\n"] {
            let source =
                format!("%%{{init: {{\"pie\": {{\"donutHole\": {hole}}}}}}}%%\npie\n{sections}");
            let svg = render_pie_from_text(&source);
            let document = roxmltree::Document::parse(&svg).expect("valid Pie SVG");
            let paths: Vec<_> = document
                .descendants()
                .filter(|node| node.attribute("class") == Some("pieCircle"))
                .map(|node| node.attribute("d").expect("slice path data"))
                .collect();
            assert_eq!(paths, [expected], "hole={hole}, sections={sections}");
        }
    }
}

#[test]
fn pie_donut_sector_paths_match_d3_rounding_and_winding() {
    // d3-shape 3.2.1 / d3-path 3.1.1: non-full 2:1 sectors retain opposite ring winding.
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"donutHole": 0.4}}}%%
pie
  "A" : 2
  "B" : 1
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Pie SVG");
    let paths: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("class") == Some("pieCircle"))
        .map(|node| node.attribute("d").expect("slice path data"))
        .collect();
    assert_eq!(
        paths,
        [
            "M0,-185A185,185,0,1,1,-160.215,92.5L-64.086,37A74,74,0,1,0,0,-74Z",
            "M-160.215,92.5A185,185,0,0,1,0,-185L0,-74A74,74,0,0,0,-64.086,37Z",
        ]
    );
}

#[test]
fn pie_donut_hole_config_renders_annular_slice_paths() {
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"donutHole": 0.4}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    assert!(
        svg.contains("A74,74"),
        "expected inner-radius arc in donut slice path: {svg}"
    );
    assert!(
        !svg.contains("L0,0Z"),
        "donut slices should not close through the center: {svg}"
    );
}

#[test]
fn pie_invalid_donut_hole_config_falls_back_to_solid_slices() {
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"donutHole": 1.2}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    assert!(
        !svg.contains("A222,222"),
        "invalid donutHole should not be used as an inner radius: {svg}"
    );
    assert!(
        svg.contains("L0,0Z"),
        "invalid donutHole should fall back to solid slices: {svg}"
    );
}

#[test]
fn pie_legend_position_config_controls_layout_regions() {
    let diagram = |position: &str| {
        layout_pie_from_text(&format!(
            r#"%%{{init: {{"pie": {{"legendPosition": "{position}"}}}}}}%%
pie
  "A" : 1
  "B" : 1
"#
        ))
    };

    let right = diagram("right");
    let right_bounds = right.bounds.as_ref().expect("right bounds");
    assert!(right_bounds.max_x > 490.0);
    assert_eq!(right_bounds.max_y, 450.0);
    assert_eq!(right.legend_x, 216.0);
    assert_eq!(right.legend_items[0].y, -22.0);

    let top = diagram("top");
    let top_bounds = top.bounds.as_ref().expect("top bounds");
    assert_eq!(top_bounds.max_x, 490.0);
    assert_eq!(top_bounds.max_y, 494.0);
    assert!(top.legend_x < 0.0);
    assert_eq!(top.legend_items[0].y, -185.0);

    let bottom = diagram("bottom");
    let bottom_bounds = bottom.bounds.as_ref().expect("bottom bounds");
    assert_eq!(bottom_bounds.max_x, 490.0);
    assert_eq!(bottom_bounds.max_y, 494.0);
    assert!(bottom.legend_x < 0.0);
    assert_eq!(bottom.legend_items[0].y, 207.0);

    let left = diagram("left");
    let left_bounds = left.bounds.as_ref().expect("left bounds");
    assert!(left_bounds.max_x > 490.0);
    assert_eq!(left_bounds.max_y, 450.0);
    assert_eq!(left.legend_x, -207.0);
    assert_eq!(left.legend_items[0].y, -22.0);

    let center = diagram("center");
    let center_bounds = center.bounds.as_ref().expect("center bounds");
    assert_eq!(center_bounds.max_x, 490.0);
    assert_eq!(center_bounds.max_y, 450.0);
    assert!(center.legend_x < 0.0);
    assert_eq!(center.legend_items[0].y, -22.0);
}

#[test]
fn pie_legend_position_top_and_left_move_the_pie_group() {
    let top_svg = render_pie_from_text(
        r#"%%{init: {"pie": {"legendPosition": "top"}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );
    assert!(top_svg.contains(r#"viewBox="0 0 490 494""#));
    let top_offset = pie_content_translate(&top_svg);
    assert!(
        top_offset.0.abs() <= f64::EPSILON && (top_offset.1 - 66.0).abs() <= f64::EPSILON,
        "top legend should move the pie group below the legend: {top_svg}"
    );

    let left_svg = render_pie_from_text(
        r#"%%{init: {"pie": {"legendPosition": "left"}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );
    let left_offset = pie_content_translate(&left_svg);
    assert!(
        left_offset.0 > 22.0 && left_offset.1.abs() <= f64::EPSILON,
        "left legend should move the pie group right by legend width: {left_svg}"
    );
    assert!(left_svg.contains(r#"class="legend" transform="translate(-207,-22)""#));
}

#[test]
fn pie_left_legend_offset_is_independent_of_title_viewport_expansion() {
    use merman_render::environment::{
        MeasurementProfileId, TextMeasurementProfile, TextMeasurementProfileIdentity,
    };
    use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
    use std::sync::Arc;

    struct FixedPieMeasurer;

    impl TextMeasurer for FixedPieMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: self.measure_svg_text_bounding_client_rect_width_px(text, style),
                height: style.font_size,
                line_count: 1,
            }
        }

        fn measure_svg_text_bounding_client_rect_width_px(
            &self,
            text: &str,
            _style: &TextStyle,
        ) -> f64 {
            match text {
                "A" => 12.0,
                "A [1]" => 40.0,
                "T" => 80.0,
                "WWWWWWWWWWWWWWWW" => 1_280.0,
                other => panic!("unexpected Pie measurement: {other:?}"),
            }
        }
    }

    for (header, expected_viewbox, expected_offset) in [
        ("", "0 0 524 450", 34.0),
        (" title T", "0 0 524 450", 34.0),
        (" title WWWWWWWWWWWWWWWW", "-415 0 1280 450", 34.0),
        (" showData", "0 0 552 450", 62.0),
        (" showData title T", "0 0 552 450", 62.0),
        (" showData title WWWWWWWWWWWWWWWW", "-415 0 1280 450", 62.0),
    ] {
        let source =
            format!("---\nconfig:\n  pie:\n    legendPosition: left\n---\npie{header}\n\"A\": 1\n");
        let parsed = legacy_init_theme_compat_engine()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("pie-title-legend-geometry").unwrap(),
                "test",
            )
            .unwrap(),
            Arc::new(FixedPieMeasurer),
        );
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile))
            .begin_session()
            .unwrap();
        let artifact = family::prepare(&parsed, &LayoutOptions::default(), session).unwrap();
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert_eq!(
            pie_content_translate(rendered.svg()),
            (expected_offset, 0.0),
            "{header}"
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            document.root_element().attribute("viewBox"),
            Some(expected_viewbox),
            "the root must still contain the measured title"
        );
    }
}

#[test]
fn empty_pie_root_viewport_is_finite_for_headless_rendering() {
    let svg = render_pie_from_text("pie");

    assert!(
        svg.contains(r#"viewBox="0 0 450 450""#),
        "empty pie should keep the finite Mermaid empty-root viewport: {svg}"
    );
    assert!(
        !svg.contains("Infinity") && !svg.contains("NaN"),
        "empty pie should not leak non-finite SVG values: {svg}"
    );
}

#[test]
fn empty_pie_with_title_keeps_title_widened_root_viewport() {
    let svg = render_pie_from_text("pie title sample title");
    let viewbox_width = root_viewbox_width(&svg);

    assert!(
        viewbox_width > 250.0,
        "empty pie title should widen the root viewport instead of falling back to 225px: {svg}"
    );
    assert!(
        !svg.contains("Infinity") && !svg.contains("NaN"),
        "titled empty pie should not leak non-finite SVG values: {svg}"
    );
}

#[test]
fn pie_highlight_slice_config_marks_matching_slice_and_emits_css() {
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"highlightSlice": "A"}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    assert!(
        svg.contains(r#".pieCircle.highlighted{scale:1.05;opacity:1;}"#),
        "Mermaid 11.16 pie CSS should include highlighted slice styling: {svg}"
    );
    assert!(
        svg.contains(r#"class="pieCircle highlighted""#),
        "Mermaid 11.16 should mark the configured highlighted slice: {svg}"
    );
    assert!(
        svg.contains(r#"class="pieCircle"/>"#),
        "non-matching slices should keep the ordinary pieCircle class: {svg}"
    );
}

#[test]
fn pie_hover_highlight_slice_config_marks_all_slices_and_emits_css() {
    let svg = render_pie_from_text(
        r#"%%{init: {"pie": {"highlightSlice": "hover"}}}%%
pie
  "A" : 1
  "B" : 1
"#,
    );

    assert!(
        svg.contains(
            r#".pieCircle.highlightedOnHover:hover{transition-duration:250ms;scale:1.05;opacity:1;}"#
        ),
        "Mermaid 11.16 pie CSS should include hover-highlight styling: {svg}"
    );
    assert!(
        svg.matches(r#"class="pieCircle highlightedOnHover""#)
            .count()
            >= 2,
        "Mermaid 11.16 should mark every slice as hover-highlightable: {svg}"
    );
}

#[test]
fn pie_static_fill_rule_outranks_the_typed_palette_for_slices_and_legend() {
    let theme = pie_slice_fill_and_palette_theme();
    let rendered = try_render_pie_with_theme_requirement(
        "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("static Pie fill and shadowed palette must be portable");
    let (slice_fills, legend_styles) = pie_terminal_fills(rendered.svg());
    assert_eq!(slice_fills, ["#111827", "#111827"]);
    assert_eq!(
        legend_styles,
        [
            "fill: rgb(17, 24, 39); stroke: rgb(17, 24, 39);",
            "fill: rgb(17, 24, 39); stroke: rgb(17, 24, 39);",
        ]
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(
        evidence.status(),
        merman_render::__private::FamilyEvidenceStatus::Verified
    );
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn pie_static_fill_preserves_per_slot_source_ownership() {
    let theme = pie_slice_fill_theme(CanvasPaint::solid("#111827").expect("valid static Pie fill"));
    let rendered = try_render_pie_with_theme_requirement(
        r##"%%{init: {"themeVariables": {"pie1": "#f59e0b"}}}%%
pie
  "Alpha" : 1
  "Beta" : 1
"##,
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("one source-owned Pie slot must not suppress the independent typed slot");
    let (slice_fills, legend_styles) = pie_terminal_fills(rendered.svg());
    assert_eq!(slice_fills, ["#f59e0b", "#111827"]);
    assert_eq!(
        legend_styles,
        [
            "fill: rgb(245, 158, 11); stroke: rgb(245, 158, 11);",
            "fill: rgb(17, 24, 39); stroke: rgb(17, 24, 39);",
        ]
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
}

#[test]
fn pie_static_fill_is_not_applicable_when_every_slot_is_source_owned() {
    let theme = pie_slice_fill_theme(CanvasPaint::solid("#111827").expect("valid static Pie fill"));
    let rendered = try_render_pie_with_theme_requirement(
        r##"%%{init: {"themeVariables": {"pie1": "#f59e0b", "pie2": "#0ea5e9"}}}%%
pie
  "Alpha" : 1
  "Beta" : 1
"##,
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("source ownership of every Pie slot must be portable NotApplicable");
    let (slice_fills, _) = pie_terminal_fills(rendered.svg());
    assert_eq!(slice_fills, ["#f59e0b", "#0ea5e9"]);

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn pie_static_fill_is_not_applicable_without_visible_slices() {
    let theme = pie_slice_fill_theme(CanvasPaint::solid("#111827").expect("valid static Pie fill"));
    let rendered = try_render_pie_with_theme_requirement(
        "pie\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("an empty Pie has no scalar-fill terminal occurrence");

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn pie_effect_binding_is_reconciled_against_visible_slice_occurrences() {
    let theme = pie_slice_effect_theme();
    let visible = try_render_pie_with_theme_requirement(
        "pie\n  \"Alpha\" : 1\n",
        &theme,
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("best-effort Pie effect render");
    let evidence = merman_render::__private::family_evidence(visible.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);

    let empty = try_render_pie_with_theme_requirement(
        "pie\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("an empty Pie makes the effect binding not applicable");
    let evidence = merman_render::__private::family_evidence(empty.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn pie_later_ordinal_fill_prevents_static_fill_from_claiming_that_occurrence() {
    let theme = pie_slice_fill_with_later_ordinal_theme();
    let rendered = render_pie_with_theme("pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n", &theme);
    let (slice_fills, legend_styles) = pie_terminal_fills(rendered.svg());
    assert_eq!(slice_fills, ["#111827", "#ffffde"]);
    assert_eq!(
        legend_styles,
        [
            "fill: rgb(17, 24, 39); stroke: rgb(17, 24, 39);",
            "fill: rgb(255, 255, 222); stroke: rgb(255, 255, 222);",
        ]
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn pie_explicit_default_fill_is_typed_and_verified() {
    let theme = pie_slice_default_fill_theme(
        CanvasPaint::solid("#111827").expect("valid explicit-Default Pie fill"),
    );
    let rendered = try_render_pie_with_theme_requirement(
        "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("explicit-Default Pie fill is portable");
    let (slice_fills, _) = pie_terminal_fills(rendered.svg());
    assert_eq!(slice_fills, ["#111827", "#111827"]);

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(
        evidence.status(),
        merman_render::__private::FamilyEvidenceStatus::Verified
    );
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn pie_ordinal_stroke_rules_fail_closed_for_matching_slice_occurrences() {
    let source = "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n";
    let baseline = render_pie_from_text(source);
    let baseline_slice_strokes =
        pie_stylesheet_property_values(&baseline, "#merman .pieCircle", "stroke");
    let baseline_outer_strokes =
        pie_stylesheet_property_values(&baseline, "#merman .pieOuterCircle", "stroke");
    assert_eq!(baseline_slice_strokes.len(), 1);
    assert_eq!(baseline_outer_strokes.len(), 1);
    for (case, selector) in [
        (
            "exact",
            OrdinalSelector::exact(2).expect("valid matching Pie exact ordinal"),
        ),
        (
            "cycle",
            OrdinalSelector::cycle(2, 0).expect("valid matching Pie cycle ordinal"),
        ),
    ] {
        let theme = pie_slice_ordinal_stroke_theme(selector);
        let rendered = try_render_pie_with_theme_requirement(
            source,
            &theme,
            ThemePortabilityRequirement::BestEffort,
        )
        .unwrap_or_else(|error| panic!("render best-effort {case} Pie stroke: {error}"));
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .pieCircle", "stroke"),
            baseline_slice_strokes,
            "unsupported {case} ordinal stroke must not alter the slice stroke writer"
        );
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .pieOuterCircle", "stroke",),
            baseline_outer_strokes,
            "unsupported {case} ordinal stroke must not alter the outer stroke writer"
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(
            evidence.status(),
            merman_render::__private::FamilyEvidenceStatus::Unverified,
            "{case}"
        );
        assert_eq!(evidence.required_count(), 1, "{case}");
        assert_eq!(evidence.accounted_count(), 1, "{case}");
        assert_eq!(evidence.applied_count(), 0, "{case}");
        assert_eq!(evidence.not_applicable_count(), 0, "{case}");
        assert_eq!(evidence.theme_residual_count(), 1, "{case}");

        let error = match try_render_pie_with_theme_requirement(
            source,
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        ) {
            Ok(_) => panic!("matching {case} ordinal Pie stroke must fail closed"),
            Err(error) => error,
        };
        assert_eq!(
            error.unverified_family_theme(),
            Some((DiagramFamilyId::PIE, 1)),
            "{case}"
        );
    }
}

#[test]
fn pie_ordinal_stroke_preserves_static_winners_only_on_uncovered_occurrences() {
    let source = "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n";
    for with_fill in [false, true] {
        for (ordinal, static_applied) in [
            (OrdinalSelector::exact(2).unwrap(), 1),
            (OrdinalSelector::cycle(1, 0).unwrap(), 0),
        ] {
            let stroke_rule = |color| {
                ThemeRule::new(
                    ThemeTarget::PieSlice,
                    ThemeStylePatch::default()
                        .with_stroke(CanvasPaint::solid(color).expect("valid Pie stroke")),
                )
            };
            let mut rules = vec![
                stroke_rule("#123456"),
                stroke_rule("#abcdef").with_ordinal(ordinal),
            ];
            if with_fill {
                rules.push(pie_fill_rule(ThemeTarget::PieSlice, "#456789"));
            }
            let theme = pie_rules_theme(rules);
            let rendered = render_pie_with_theme(source, &theme);
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), "#merman .pieCircle", "stroke"),
                ["#123456"],
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(
                evidence.applied_count(),
                static_applied + usize::from(with_fill)
            );
            assert_eq!(evidence.not_applicable_count(), 1 - static_applied);
            assert_eq!(evidence.theme_residual_count(), 1);
            assert_eq!(evidence.accounted_count(), evidence.required_count());
        }
    }
}

#[test]
fn pie_static_stroke_is_verified_from_slice_and_outer_circle_css() {
    for (stroke, expected_css) in [
        (
            CanvasPaint::solid("#7c3aed").expect("valid solid Pie stroke"),
            "#7c3aed",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = pie_slice_stroke_theme(stroke);
        let rendered = render_pie_with_theme("pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n", &theme);
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Pie SVG");
        let stylesheet = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Pie stylesheet");

        assert!(
            stylesheet.contains(&format!(
                "#merman .pieCircle{{stroke:{expected_css};stroke-width:"
            )),
            "typed Pie slice stroke must be emitted by the terminal writer: {stylesheet}"
        );
        assert!(
            stylesheet.contains(&format!(
                "#merman .pieOuterCircle{{stroke:{expected_css};stroke-width:"
            )),
            "typed Pie outer stroke must be emitted by the terminal writer: {stylesheet}"
        );
        assert_eq!(stylesheet.matches("#merman .pieCircle{").count(), 1);
        assert_eq!(stylesheet.matches("#merman .pieOuterCircle{").count(), 1);
        assert_eq!(
            document
                .descendants()
                .filter(|node| {
                    node.has_tag_name("path")
                        && node.attribute("class").is_some_and(|class| {
                            class
                                .split_ascii_whitespace()
                                .any(|part| part == "pieCircle")
                        })
                })
                .count(),
            2,
            "both semantic slices must reach the CSS-owned terminal surface"
        );
        assert!(document.descendants().any(|node| {
            node.has_tag_name("circle") && node.attribute("class") == Some("pieOuterCircle")
        }));

        drop(document);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(
            evidence.status(),
            merman_render::__private::FamilyEvidenceStatus::Verified
        );
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn pie_source_outer_width_preserves_typed_stroke_portable_evidence() {
    let theme =
        pie_slice_stroke_theme(CanvasPaint::solid("#7c3aed").expect("valid typed Pie stroke"));
    for outer_color in [None, Some("#0f172a")] {
        let source_color = outer_color
            .map(|color| format!("    pieOuterStrokeColor: '{color}'\n"))
            .unwrap_or_default();
        let source = format!(
            "---\nconfig:\n  themeVariables:\n    pieOuterStrokeWidth: 20px\n{source_color}---\npie\n\"A\": 1\n"
        );
        let rendered = try_render_pie_with_theme_requirement(
            &source,
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("source width and independently owned stroke paint remain portable");
        assert_eq!(pie_outer_circle_radius(rendered.svg()), 195.0);
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .pieCircle", "stroke"),
            ["#7c3aed"]
        );
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .pieOuterCircle", "stroke"),
            [outer_color.unwrap_or("#7c3aed")]
        );
        assert_eq!(
            pie_stylesheet_property_values(
                rendered.svg(),
                "#merman .pieOuterCircle",
                "stroke-width"
            ),
            ["20px"]
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.accounted_count(), evidence.required_count());
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn pie_explicit_default_stroke_is_verified_from_slice_and_outer_circle_css() {
    for (stroke, expected_css) in [
        (
            CanvasPaint::solid("#7c3aed").expect("valid explicit-Default Pie stroke"),
            "#7c3aed",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ] {
        let theme = pie_slice_default_stroke_theme(stroke);
        let rendered = try_render_pie_with_theme_requirement(
            "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n",
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("explicit-Default Pie stroke is portable");
        let stylesheet =
            pie_stylesheet_property_values(rendered.svg(), "#merman .pieCircle", "stroke");
        assert_eq!(stylesheet, [expected_css]);
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .pieOuterCircle", "stroke"),
            [expected_css]
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(
            evidence.status(),
            merman_render::__private::FamilyEvidenceStatus::Verified
        );
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn pie_static_stroke_respects_per_site_mermaid_ownership() {
    let theme =
        pie_slice_stroke_theme(CanvasPaint::solid("#7c3aed").expect("valid typed Pie stroke"));
    for (owned_key, owned_color, owned_selector, typed_selector) in [
        (
            "pieStrokeColor",
            "#0f172a",
            "#merman .pieCircle",
            "#merman .pieOuterCircle",
        ),
        (
            "pieOuterStrokeColor",
            "#1e293b",
            "#merman .pieOuterCircle",
            "#merman .pieCircle",
        ),
    ] {
        let source = format!(
            "%%{{init: {{\"themeVariables\": {{\"{owned_key}\": \"{owned_color}\"}}}}}}%%\npie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n"
        );
        let rendered = render_pie_with_theme(&source, &theme);
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), owned_selector, "stroke"),
            [owned_color],
            "the explicit Mermaid {owned_key} value must retain terminal ownership"
        );
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), typed_selector, "stroke"),
            ["#7c3aed"],
            "typed Pie stroke must still own the independent site for {owned_key}"
        );
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1, "{owned_key}");
        assert_eq!(evidence.accounted_count(), 1, "{owned_key}");
        assert_eq!(evidence.applied_count(), 1, "{owned_key}");
        assert_eq!(evidence.not_applicable_count(), 0, "{owned_key}");
        assert_eq!(evidence.theme_residual_count(), 0, "{owned_key}");
    }
}

#[test]
fn pie_static_stroke_is_not_applicable_when_mermaid_owns_both_sites() {
    let theme =
        pie_slice_stroke_theme(CanvasPaint::solid("#7c3aed").expect("valid typed Pie stroke"));
    let rendered = render_pie_with_theme(
        r##"%%{init: {"themeVariables": {"pieStrokeColor": "#0f172a", "pieOuterStrokeColor": "#1e293b"}}}%%
pie
  "Alpha" : 1
  "Beta" : 1
"##,
        &theme,
    );
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed Pie SVG");
    let stylesheet = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .expect("Pie stylesheet");

    assert!(stylesheet.contains("#merman .pieCircle{stroke:#0f172a;stroke-width:"));
    assert!(stylesheet.contains("#merman .pieOuterCircle{stroke:#1e293b;stroke-width:"));
    assert!(!stylesheet.contains("#merman .pieCircle{stroke:#7c3aed;"));
    assert!(!stylesheet.contains("#merman .pieOuterCircle{stroke:#7c3aed;"));

    drop(document);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.accounted_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

fn pie_text_role_rule(target: ThemeTarget, size: f32, color: &str) -> ThemeRule {
    let mut patch = ThemeStylePatch::default()
        .with_fill(CanvasPaint::solid(color).expect("valid Pie text fill"));
    patch.typography.font_size_px = merman_render::diagram_theme::Specified::Value(size);
    ThemeRule::new(target, patch)
}

#[test]
fn pie_typed_text_roles_emit_the_ghibli_sizes_and_independent_legend_ink() {
    for variant in [None, Some(ThemeVariant::Default)] {
        let theme = pie_rules_theme(
            [
                (ThemeTarget::Title, 25.0, "#3A2E2C"),
                (ThemeTarget::Text, 17.0, "#3A2E2C"),
                (ThemeTarget::Legend, 12.0, "#6B5B4F"),
            ]
            .map(|(target, size, color)| {
                let rule = pie_text_role_rule(target, size, color);
                variant.map_or(rule.clone(), |variant| rule.with_variant(variant))
            }),
        );
        let rendered = try_render_pie_with_theme_requirement(
            "pie title Forest harvest\n  \"Acorns\" : 2\n  \"Berries\" : 1\n",
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("the three typed Pie text roles are portable");
        for (selector, size, color) in [
            ("#merman .pieTitleText", "25px", "#3A2E2C"),
            ("#merman .slice", "17px", "#3A2E2C"),
            ("#merman .legend text", "12px", "#6B5B4F"),
        ] {
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), selector, "font-size"),
                [size]
            );
            let expected_color = ThemeColorValue::parse(color).unwrap().as_css();
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), selector, "fill"),
                [expected_color]
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 3);
        assert_eq!(evidence.applied_count(), 3);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn pie_builtin_ghibli_preserves_role_sizes_shared_fills_and_inherited_font() {
    let theme = DiagramThemeCompiler::new()
        .compile_preset(ThemePreset::Ghibli)
        .expect("compile the built-in native Ghibli preset");
    let rendered = try_render_pie_with_theme_requirement(
        "pie title Forest harvest\n  \"Acorns\" : 2\n  \"Berries\" : 1\n",
        &theme,
        ThemePortabilityRequirement::RequirePortable,
    )
    .expect("render Ghibli through the real Pie consumers");
    let expected_font = FontStack::new(["Open Sans", "Noto Sans SC", "sans-serif"])
        .unwrap()
        .as_css();
    for (selector, size, color) in [
        ("#merman .slice", "17px", "#3A2E2C"),
        ("#merman .pieTitleText", "25px", "#3A2E2C"),
        ("#merman .legend text", "12px", "#6B5B4F"),
    ] {
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), selector, "font-size"),
            [size]
        );
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), selector, "fill"),
            [color]
        );
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), selector, "font-family"),
            std::slice::from_ref(&expected_font)
        );
    }
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(
        evidence.status(),
        merman_render::__private::FamilyEvidenceStatus::Verified
    );
    assert!(evidence.applied_count() > 0);
    assert_eq!(evidence.accounted_count(), evidence.required_count());
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn pie_legend_visibility_uses_final_entity_decoded_xml_text() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_styles(ThemeRuleSet::default().with_rule(pie_text_role_rule(
                    ThemeTarget::Legend,
                    12.0,
                    "#6B5B4F",
                )))
                .with_typography(
                    TypographySpec::default().with_default(
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Pie Visibility").unwrap()),
                    ),
                ),
        )
        .unwrap();
    for (header, label, visible) in [
        ("pie", "#32;", false),
        ("pie", "#nbsp;", false),
        ("pie", "#31;", false),
        ("pie", "\u{1}", false),
        ("pie", "\u{fffe}", false),
        ("pie", "#32;\u{1}#nbsp;", false),
        ("pie", "#32;Oak\u{1}", true),
        ("pie showData", "#32;", true),
    ] {
        let source = format!("{header}\n  \"{label}\" : 0\n");
        let rendered = try_render_pie_with_theme_requirement(
            &source,
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("legend rule and inherited font follow the final emitted text visibility");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 2, "{source:?}");
        assert_eq!(
            evidence.applied_count(),
            if visible { 2 } else { 0 },
            "{source:?}"
        );
        assert_eq!(
            evidence.not_applicable_count(),
            if visible { 0 } else { 2 },
            "{source:?}"
        );
        assert_eq!(evidence.theme_residual_count(), 0, "{source:?}");
    }
    for (ordinal, residual) in [(1, 0), (2, 1)] {
        let theme = pie_rules_theme([pie_text_role_rule(ThemeTarget::Legend, 12.0, "#6B5B4F")
            .with_ordinal(OrdinalSelector::exact(ordinal).unwrap())]);
        let rendered = render_pie_with_theme("pie\n  \"#32;\" : 0\n  \"Oak\" : 0\n", &theme);
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1 - residual);
        assert_eq!(evidence.theme_residual_count(), residual);
    }
}

#[test]
fn pie_role_text_properties_keep_source_ownership_independent() {
    for (target, selector, fill_key, size_key) in [
        (
            ThemeTarget::Title,
            "#merman .pieTitleText",
            "pieTitleTextColor",
            "pieTitleTextSize",
        ),
        (
            ThemeTarget::Text,
            "#merman .slice",
            "pieSectionTextColor",
            "pieSectionTextSize",
        ),
        (
            ThemeTarget::Legend,
            "#merman .legend text",
            "pieLegendTextColor",
            "pieLegendTextSize",
        ),
    ] {
        let theme = pie_rules_theme([pie_text_role_rule(target, 31.0, "#123456")]);
        for (source_fill, source_size) in [(true, false), (false, true), (true, true)] {
            let mut variables = serde_json::Map::new();
            if source_fill {
                variables.insert(fill_key.to_owned(), serde_json::json!("#abcdef"));
            }
            if source_size {
                variables.insert(size_key.to_owned(), serde_json::json!("19px"));
            }
            let config = serde_json::json!({"themeVariables": variables});
            let source =
                format!("%%{{init: {config}}}%%\npie title Source owner\n  \"Alpha\" : 1\n");
            let rendered = try_render_pie_with_theme_requirement(
                &source,
                &theme,
                ThemePortabilityRequirement::RequirePortable,
            )
            .expect("source ownership suppresses only the corresponding text property");
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), selector, "fill"),
                [if source_fill { "#abcdef" } else { "#123456" }]
            );
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), selector, "font-size"),
                [if source_size { "19px" } else { "31px" }]
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(
                evidence.applied_count(),
                usize::from(!(source_fill && source_size))
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(source_fill && source_size)
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn pie_text_presence_follows_drawn_percentages_and_all_legend_entries() {
    let theme = pie_rules_theme([
        pie_text_role_rule(ThemeTarget::Title, 25.0, "#123456"),
        pie_text_role_rule(ThemeTarget::Text, 17.0, "#234567"),
        pie_text_role_rule(ThemeTarget::Legend, 12.0, "#345678"),
    ]);
    for (source, applied, absent) in [
        ("pie\n", 0, 3),
        ("pie\n  \"Alpha\" : 0\n  \"Beta\" : 0\n", 1, 2),
        ("pie title Title only\n", 1, 2),
        (
            "---\ntitle: Metadata title\n---\npie\n  \"Alpha\" : 1\n",
            3,
            0,
        ),
    ] {
        let rendered = try_render_pie_with_theme_requirement(
            source,
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("absent text surfaces are not applicable");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 3, "{source}");
        assert_eq!(evidence.applied_count(), applied, "{source}");
        assert_eq!(evidence.not_applicable_count(), absent, "{source}");
        assert_eq!(evidence.theme_residual_count(), 0, "{source}");
    }
    let filtered = "pie\n  \"Visible\" : 1000\n  \"Legend only\" : 1\n";
    for (target, residual) in [(ThemeTarget::Text, 0), (ThemeTarget::Legend, 1)] {
        let theme = pie_rules_theme([pie_text_role_rule(target, 31.0, "#123456")
            .with_ordinal(OrdinalSelector::exact(2).unwrap())]);
        let rendered = render_pie_with_theme(filtered, &theme);
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1 - residual);
        assert_eq!(evidence.theme_residual_count(), residual);
    }
}

#[test]
fn pie_role_ordinals_and_unsupported_mixed_facets_fail_closed() {
    for (target, selector) in [
        (ThemeTarget::Title, "#merman .pieTitleText"),
        (ThemeTarget::Text, "#merman .slice"),
        (ThemeTarget::Legend, "#merman .legend text"),
    ] {
        for ordinal in [
            OrdinalSelector::exact(1).unwrap(),
            OrdinalSelector::cycle(1, 0).unwrap(),
        ] {
            let theme = pie_rules_theme([
                pie_text_role_rule(target, 29.0, "#abcdef"),
                pie_text_role_rule(target, 41.0, "#123456").with_ordinal(ordinal),
            ]);
            let source = "pie title Ordinal\n  \"Alpha\" : 1\n  \"Beta\" : 1\n";
            let rendered = render_pie_with_theme(source, &theme);
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), selector, "font-size"),
                ["29px"]
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.theme_residual_count(), 1);
            assert_eq!(evidence.accounted_count(), evidence.required_count());
            assert!(
                try_render_pie_with_theme_requirement(
                    source,
                    &theme,
                    ThemePortabilityRequirement::RequirePortable
                )
                .is_err()
            );
        }
        let mut patch =
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
        patch.typography.font_size_px = merman_render::diagram_theme::Specified::Value(29.0);
        patch.typography.letter_spacing_px = merman_render::diagram_theme::Specified::Value(2.0);
        let theme = pie_rules_theme([ThemeRule::new(target, patch)]);
        let rendered = render_pie_with_theme("pie title Mixed\n  \"Alpha\" : 1\n", &theme);
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
    }
}

#[test]
fn pie_role_weight_and_style_have_a_bounded_native_consumer() {
    for target in [ThemeTarget::Title, ThemeTarget::Text, ThemeTarget::Legend] {
        let mut patch = ThemeStylePatch::default();
        patch.typography.font_weight = merman_render::diagram_theme::Specified::Value(650);
        patch.typography.font_style = merman_render::diagram_theme::Specified::Value(
            merman_render::diagram_theme::FontStyle::Italic,
        );
        let theme = pie_rules_theme([ThemeRule::new(target, patch)]);
        let rendered = try_render_pie_with_theme_requirement(
            "pie title Styled\n  \"Alpha\" : 1\n",
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .expect("bounded role weight and style are portable");
        let selector = match target {
            ThemeTarget::Title => "#merman .pieTitleText",
            ThemeTarget::Text => "#merman .slice",
            ThemeTarget::Legend => "#merman .legend text",
            _ => unreachable!(),
        };
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), selector, "font-weight"),
            ["650"]
        );
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), selector, "font-style"),
            ["italic"]
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

fn pie_role_typography_patch(property: &str, clear: bool) -> ThemeStylePatch {
    use merman_render::diagram_theme::{FontStyle, Specified};

    let mut patch = ThemeStylePatch::default();
    match property {
        "font-size" => {
            patch.typography.font_size_px = if clear {
                Specified::Clear
            } else {
                Specified::Value(31.0)
            };
        }
        "font-weight" => {
            patch.typography.font_weight = if clear {
                Specified::Clear
            } else {
                Specified::Value(650)
            };
        }
        "font-style" => {
            patch.typography.font_style = if clear {
                Specified::Clear
            } else {
                Specified::Value(FontStyle::Italic)
            };
        }
        _ => unreachable!("bounded Pie role typography fixture"),
    }
    patch
}

#[test]
fn pie_role_typography_clear_keeps_baseline_css_and_independent_text_roles() {
    let source = "pie title Baseline typography\n  \"Alpha\" : 2\n  \"Beta\" : 1\n";
    for target in [ThemeTarget::Text, ThemeTarget::Title, ThemeTarget::Legend] {
        for property in ["font-size", "font-weight", "font-style"] {
            // A Text request remains on percentage labels when another role is cleared.
            let fallback = (target != ThemeTarget::Text).then(|| {
                ThemeRule::new(
                    ThemeTarget::Text,
                    pie_role_typography_patch(property, false),
                )
            });
            let baseline = render_pie_with_theme(source, &pie_rules_theme(fallback.clone()));
            for previous_value in [false, true] {
                for variant in [None, Some(ThemeVariant::Default)] {
                    let mut rules = fallback.clone().into_iter().collect::<Vec<_>>();
                    if previous_value {
                        rules.push(ThemeRule::new(
                            target,
                            pie_role_typography_patch(property, false),
                        ));
                    }
                    let clear = ThemeRule::new(target, pie_role_typography_patch(property, true));
                    rules
                        .push(variant.map_or(clear.clone(), |variant| clear.with_variant(variant)));
                    let rendered = try_render_pie_with_theme_requirement(
                        source,
                        &pie_rules_theme(rules),
                        ThemePortabilityRequirement::RequirePortable,
                    )
                    .expect("a winning typography Clear must be portable");
                    assert_eq!(
                        rendered.svg(),
                        baseline.svg(),
                        "{target:?}/{property}/previous={previous_value}/variant={variant:?}"
                    );
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(
                        evidence.required_count(),
                        1 + usize::from(previous_value) + usize::from(fallback.is_some())
                    );
                    assert_eq!(
                        evidence.applied_count(),
                        1 + usize::from(fallback.is_some())
                    );
                    assert_eq!(evidence.not_applicable_count(), usize::from(previous_value));
                    assert_eq!(evidence.accounted_count(), evidence.required_count());
                    assert_eq!(evidence.theme_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn pie_role_typography_clear_preserves_source_sizes_and_requires_real_text() {
    use merman_render::diagram_theme::Specified;

    let config = serde_json::json!({"themeVariables": {
        "fontSize": "20px",
        "pieTitleTextSize": "150%",
        "pieSectionTextSize": "1.25em",
        "pieLegendTextSize": "inherit"
    }});
    for (body, present) in [
        ("pie\n", [false, false, false]),
        ("pie title Title only\n", [true, false, false]),
        ("pie\n  \"Alpha\" : 0\n", [false, false, true]),
        ("pie title All roles\n  \"Alpha\" : 1\n", [true, true, true]),
    ] {
        let source = format!("%%{{init: {config}}}%%\n{body}");
        let baseline = render_pie_with_theme(&source, &pie_rules_theme([]));
        for (index, target) in [ThemeTarget::Title, ThemeTarget::Text, ThemeTarget::Legend]
            .into_iter()
            .enumerate()
        {
            for size_only in [true, false] {
                let mut value = pie_role_typography_patch("font-size", false);
                let mut clear = pie_role_typography_patch("font-size", true);
                if !size_only {
                    value.typography.font_weight = Specified::Value(650);
                    value.typography.font_style =
                        Specified::Value(merman_render::diagram_theme::FontStyle::Italic);
                    clear.typography.font_weight = Specified::Clear;
                    clear.typography.font_style = Specified::Clear;
                }
                let theme =
                    pie_rules_theme([ThemeRule::new(target, value), ThemeRule::new(target, clear)]);
                let rendered = try_render_pie_with_theme_requirement(
                    &source,
                    &theme,
                    ThemePortabilityRequirement::RequirePortable,
                )
                .expect("source size ownership must not suppress weight/style Clear");
                assert_eq!(rendered.svg(), baseline.svg(), "{target:?}/{body:?}");
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                let applied = usize::from(present[index] && !size_only);
                assert_eq!(evidence.required_count(), 2);
                assert_eq!(evidence.applied_count(), applied);
                assert_eq!(evidence.not_applicable_count(), 2 - applied);
                assert_eq!(evidence.accounted_count(), 2);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn pie_base_size_needs_a_visible_inherited_role_and_respects_source_base_size() {
    let theme = pie_typography_theme(ThemeTextStyle::default().with_font_size_px(24.0).unwrap());
    for (base_size, expected_base, applied) in [(None, "24px", 1), (Some("20px"), "20px", 0)] {
        let mut variables =
            serde_json::json!({"pieTitleTextSize": "150%", "pieLegendTextSize": "inherit"});
        if let Some(size) = base_size {
            variables["fontSize"] = serde_json::json!(size);
        }
        let config = serde_json::json!({"themeVariables": variables});
        let source = format!("%%{{init: {config}}}%%\npie title Inherited\n  \"Alpha\" : 1\n");
        let rendered = try_render_pie_with_theme_requirement(
            &source,
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman", "font-size"),
            [expected_base]
        );
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .pieTitleText", "font-size"),
            ["150%"]
        );
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .legend text", "font-size"),
            ["inherit"]
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), applied);
        assert_eq!(evidence.not_applicable_count(), 1 - applied);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
    let unresolved = render_pie_with_theme(
        "%%{init: {\"themeVariables\": {\"pieLegendTextSize\": \"calc(1em + 2px)\"}}}%%\npie\n  \"Alpha\" : 1\n",
        &theme,
    );
    let evidence = merman_render::__private::family_evidence(unresolved.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn pie_empty_native_theme_preserves_unstyled_svg_bytes() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    for source in [
        "pie title Baseline\n  \"Alpha\" : 1\n  \"Zero\" : 0\n",
        "pie showData\n  \"Decimal\" : 1.234567\n",
        "%%{init: {\"themeVariables\": {\"pieTitleTextSize\": \"1.5em\", \"pieLegendTextSize\": \"20px\"}}}%%\npie title Raw sizes\n  \"Alpha\" : 1\n",
    ] {
        assert_eq!(
            render_pie_from_text(source),
            render_pie_with_theme(source, &theme).svg()
        );
    }
}

#[test]
fn pie_invalid_role_sizes_yield_to_typed_values_without_changing_sibling_roles() {
    for (target, key, selector, siblings) in [
        (
            ThemeTarget::Title,
            "pieTitleTextSize",
            "#merman .pieTitleText",
            [("#merman .slice", "17px"), ("#merman .legend text", "17px")],
        ),
        (
            ThemeTarget::Text,
            "pieSectionTextSize",
            "#merman .slice",
            [
                ("#merman .pieTitleText", "25px"),
                ("#merman .legend text", "17px"),
            ],
        ),
        (
            ThemeTarget::Legend,
            "pieLegendTextSize",
            "#merman .legend text",
            [
                ("#merman .pieTitleText", "25px"),
                ("#merman .slice", "17px"),
            ],
        ),
    ] {
        let mut patch = ThemeStylePatch::default();
        patch.typography.font_size_px = merman_render::diagram_theme::Specified::Value(31.0);
        let theme = pie_rules_theme([ThemeRule::new(target, patch)]);
        for (size, source_owns_size) in [
            ("", false),
            ("  ", false),
            ("bogus", false),
            ("-4px", false),
            ("25", false),
            ("32px", true),
            ("24pt", true),
            ("150%", true),
            ("0px", true),
            ("calc(1em + 2px)", true),
        ] {
            let config = serde_json::json!({"themeVariables": {key: size}});
            let source = format!(
                "---\nconfig: {config}\n---\npie title Release audit\n  \"Alpha\" : 5\n  \"Beta\" : 3\n"
            );
            let rendered = try_render_pie_with_theme_requirement(
                &source,
                &theme,
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            let expected = if source_owns_size { size } else { "31px" };
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), selector, "font-size"),
                [expected]
            );
            for (sibling, baseline) in siblings {
                assert_eq!(
                    pie_stylesheet_property_values(rendered.svg(), sibling, "font-size"),
                    [baseline]
                );
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), usize::from(!source_owns_size));
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(source_owns_size)
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn pie_role_size_admission_preserves_trusted_opaque_owners() {
    let body = "pie title Typography\n  \"Alpha\" : 1\n";
    for (target, key, selector, baseline) in [
        (
            ThemeTarget::Title,
            "pieTitleTextSize",
            "#merman .pieTitleText",
            "25px",
        ),
        (
            ThemeTarget::Text,
            "pieSectionTextSize",
            "#merman .slice",
            "17px",
        ),
        (
            ThemeTarget::Legend,
            "pieLegendTextSize",
            "#merman .legend text",
            "17px",
        ),
    ] {
        let mut patch = ThemeStylePatch::default();
        patch.typography.font_size_px = merman_render::diagram_theme::Specified::Value(31.0);
        let theme = pie_rules_theme([ThemeRule::new(target, patch)]);
        let variable = "var(--pie-size)";
        let important = r"24p\74 !important";
        let source_variable = format!(
            "---\nconfig: {}\n---\n{body}",
            serde_json::json!({"themeVariables": {key: variable}})
        );
        let source_important = format!(
            "---\nconfig: {}\n---\n{body}",
            serde_json::json!({"themeVariables": {key: important}})
        );
        // Source admits individual values, not host substitution or declaration priority.
        // Trusted site values must keep their ownership even when the size is not measurable.
        for (case, source, host_size, effective_size, emitted_size, typed_applies) in [
            (
                "trusted commented important",
                body,
                Some("/* host comment */24pt !important"),
                "/* host comment */24pt !important",
                "/* host comment */24pt !important",
                false,
            ),
            (
                "trusted commented expression",
                body,
                Some("calc(1em /* comment */ + 2px)"),
                "calc(1em /* comment */ + 2px)",
                "calc(1em /* comment */ + 2px)",
                false,
            ),
            (
                "trusted missing important value",
                body,
                Some("!important"),
                "!important",
                "31px",
                true,
            ),
            (
                "trusted duplicate priority",
                body,
                Some("bogus !important !important"),
                "bogus !important !important",
                "31px",
                true,
            ),
            (
                "trusted variable",
                body,
                Some(variable),
                variable,
                variable,
                false,
            ),
            (
                "trusted escaped important",
                body,
                Some(important),
                important,
                important,
                false,
            ),
            (
                "filtered source variable",
                source_variable.as_str(),
                None,
                baseline,
                "31px",
                true,
            ),
            (
                "filtered source important",
                source_important.as_str(),
                None,
                baseline,
                "31px",
                true,
            ),
            (
                "filtered variable preserves host variable",
                source_variable.as_str(),
                Some(variable),
                variable,
                variable,
                false,
            ),
            (
                "filtered important preserves host variable",
                source_important.as_str(),
                Some(variable),
                variable,
                variable,
                false,
            ),
            (
                "filtered variable preserves host static",
                source_variable.as_str(),
                Some("24pt"),
                "24pt",
                "24pt",
                false,
            ),
        ] {
            let site = host_size.map_or_else(
                || serde_json::json!({}),
                |size| serde_json::json!({"themeVariables": {key: size}}),
            );
            let engine = common::install_parse_compatibility(
                &theme,
                merman_core::MermaidEngine::new().with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(site),
                ),
            );
            let parsed = engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
            let path = format!("themeVariables.{key}");
            assert_eq!(
                parsed.metadata().effective_config.get_str(&path),
                Some(effective_size),
                "{key}/{case}"
            );
            assert_eq!(
                parsed
                    .metadata()
                    .effective_config
                    .config_path_overrides_typed_default(&path),
                host_size.is_some(),
                "{key}/{case}: provenance retains admitted host configuration"
            );
            let session = RenderEnvironment::deterministic()
                .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
                .begin_session_with_theme(&theme)
                .unwrap();
            let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
                .unwrap()
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap_or_else(|error| panic!("{key}/{case}: {error}"));
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), selector, "font-size"),
                [emitted_size],
                "{key}/{case}"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1, "{key}/{case}");
            assert_eq!(
                evidence.applied_count(),
                usize::from(typed_applies),
                "{key}/{case}"
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(!typed_applies),
                "{key}/{case}"
            );
            assert_eq!(evidence.accounted_count(), 1, "{key}/{case}");
            assert_eq!(evidence.theme_residual_count(), 0, "{key}/{case}");
        }
    }
}

#[test]
fn pie_site_role_size_ownership_preserves_independent_weight_and_fill() {
    use merman_render::diagram_theme::Specified;

    let mut patch = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
    patch.typography.font_size_px = Specified::Value(31.0);
    patch.typography.font_weight = Specified::Value(650);
    let theme = pie_rules_theme([ThemeRule::new(ThemeTarget::Legend, patch)]);
    for (value, expected) in [
        (serde_json::json!(25), "31px"),
        (serde_json::json!("bogus"), "31px"),
        (serde_json::json!("-4px"), "31px"),
        (serde_json::json!("24pt"), "24pt"),
        (serde_json::json!("0px"), "0px"),
    ] {
        let engine = common::install_parse_compatibility(
            &theme,
            merman_core::MermaidEngine::new().with_site_config(
                merman_core::presentation::SourcePresentationConfig::from_value(
                    serde_json::json!({"themeVariables": {"pieLegendTextSize": value}}),
                ),
            ),
        );
        let parsed = engine
            .parse_diagram_for_render_model_sync("pie\n  \"Alpha\" : 1\n", ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap();
        let rendered = family::prepare(&parsed, &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        for (property, expected) in [
            ("font-size", expected),
            ("font-weight", "650"),
            ("fill", "#123456"),
        ] {
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), "#merman .legend text", property),
                [expected]
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), usize::from(expected != "0px"));
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(expected == "0px")
        );
        assert_eq!(evidence.accounted_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn pie_zero_sized_roles_do_not_certify_fonts_or_paint() {
    let roles = [
        (
            ThemeTarget::Title,
            "pieTitleTextSize",
            "#merman .pieTitleText",
        ),
        (ThemeTarget::Text, "pieSectionTextSize", "#merman .slice"),
        (
            ThemeTarget::Legend,
            "pieLegendTextSize",
            "#merman .legend text",
        ),
    ];
    for with_paint in [false, true] {
        let rules = if with_paint {
            roles
                .iter()
                .fold(ThemeRuleSet::default(), |rules, (target, _, _)| {
                    rules.with_rule(ThemeRule::new(
                        *target,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    ))
                })
        } else {
            ThemeRuleSet::default()
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_typography(
                        TypographySpec::default().with_family_style(
                            DiagramFamilyId::PIE,
                            ThemeTextStyle::default()
                                .with_font_stack(FontStack::single("Pie Visibility").unwrap()),
                        ),
                    )
                    .with_styles(rules),
            )
            .unwrap();
        for (zero, positive_role) in [
            ("0px", None),
            ("0", None),
            ("-0pt", None),
            ("0em", None),
            ("0%", None),
            ("0px", Some(0)),
            ("0px", Some(1)),
            ("0px", Some(2)),
        ] {
            let mut variables = serde_json::Map::new();
            for (index, (_, key, _)) in roles.iter().enumerate() {
                variables.insert(
                    (*key).to_owned(),
                    serde_json::json!(if positive_role == Some(index) {
                        "17px"
                    } else {
                        zero
                    }),
                );
            }
            let config = serde_json::json!({"themeVariables": variables});
            let source = format!("---\nconfig: {config}\n---\npie title Hidden\n  \"Alpha\" : 1\n");
            let rendered = try_render_pie_with_theme_requirement(
                &source,
                &theme,
                ThemePortabilityRequirement::RequirePortable,
            )
            .unwrap();
            for (index, (_, _, selector)) in roles.iter().enumerate() {
                assert_eq!(
                    pie_stylesheet_property_values(rendered.svg(), selector, "font-size"),
                    [if positive_role == Some(index) {
                        "17px"
                    } else {
                        zero
                    }]
                );
            }
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert_eq!(
                document
                    .descendants()
                    .filter(|node| node.has_tag_name("text")
                        && node.text().is_some_and(|text| !text.trim().is_empty()))
                    .count(),
                3,
                "hidden text must still reach the complete ordered receipt"
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            let required = if with_paint { 4 } else { 1 };
            let applied = if positive_role.is_some() {
                1 + usize::from(with_paint)
            } else {
                0
            };
            assert_eq!(evidence.required_count(), required);
            assert_eq!(
                evidence.applied_count(),
                applied,
                "{zero}/{positive_role:?}/paint={with_paint}"
            );
            assert_eq!(evidence.not_applicable_count(), required - applied);
            assert_eq!(evidence.accounted_count(), required);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn pie_negative_base_sizes_leave_valid_fallbacks_and_themes_available() {
    let theme = pie_typography_theme(ThemeTextStyle::default().with_font_size_px(27.0).unwrap());
    for invalid in [
        serde_json::json!(-1),
        serde_json::json!("-1"),
        serde_json::json!("-1px"),
    ] {
        for legacy_root in [None, Some(32)] {
            let mut config = serde_json::json!({
                "themeVariables": {
                    "fontSize": invalid,
                    "pieTitleTextSize": "inherit",
                    "pieSectionTextSize": "1em",
                    "pieLegendTextSize": "100%"
                }
            });
            if let Some(size) = legacy_root {
                config["fontSize"] = serde_json::json!(size);
            }
            let source =
                format!("---\nconfig: {config}\n---\npie title Negative base\n  \"Alpha\" : 1\n");
            let plain = render_pie_from_text(&source);
            let expected_plain = format!("{}px", legacy_root.unwrap_or(16));
            assert_eq!(
                pie_stylesheet_property_values(&plain, "#merman", "font-size"),
                [expected_plain]
            );
            let rendered = render_pie_with_theme(&source, &theme);
            let expected = format!("{}px", legacy_root.unwrap_or(27));
            for selector in ["#merman", "#merman svg"] {
                assert_eq!(
                    pie_stylesheet_property_values(rendered.svg(), selector, "font-size"),
                    [expected.as_str()],
                    "{invalid}/{legacy_root:?}"
                );
            }
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1);
            assert_eq!(evidence.applied_count(), usize::from(legacy_root.is_none()));
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(legacy_root.is_some())
            );
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn pie_inherited_zero_and_subpixel_sizes_share_css_measurement_and_visibility() {
    let roles = [
        (
            ThemeTarget::Title,
            "pieTitleTextSize",
            "#merman .pieTitleText",
            "inherit",
        ),
        (
            ThemeTarget::Text,
            "pieSectionTextSize",
            "#merman .slice",
            "1em",
        ),
        (
            ThemeTarget::Legend,
            "pieLegendTextSize",
            "#merman .legend text",
            "100%",
        ),
    ];
    let mut rules = ThemeRuleSet::default().with_rule(ThemeRule::new(
        ThemeTarget::PieSlice,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#abcdef").unwrap()),
    ));
    for (target, _, _, _) in roles {
        rules = rules.with_rule(ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
        ));
    }
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_typography(
                    TypographySpec::default().with_family_style(
                        DiagramFamilyId::PIE,
                        ThemeTextStyle::default()
                            .with_font_stack(FontStack::single("Pie Inheritance").unwrap()),
                    ),
                )
                .with_styles(rules),
        )
        .unwrap();
    for (base, positive_title) in [
        ("0px", false),
        ("0.5px", false),
        ("1px", false),
        ("0px", true),
    ] {
        let mut variables = serde_json::json!({"fontSize": base});
        for (_, key, _, relative) in roles {
            variables[key] = serde_json::json!(if positive_title && key == "pieTitleTextSize" {
                "17px"
            } else {
                relative
            });
        }
        let source = format!(
            "---\nconfig: {}\n---\npie title Hidden\n  \"Alpha\" : 1\n",
            serde_json::json!({"themeVariables": variables})
        );
        let rendered = try_render_pie_with_theme_requirement(
            &source,
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        for selector in ["#merman", "#merman svg"] {
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), selector, "font-size"),
                [base]
            );
        }
        for (_, key, selector, relative) in roles {
            assert_eq!(
                pie_stylesheet_property_values(rendered.svg(), selector, "font-size"),
                [if positive_title && key == "pieTitleTextSize" {
                    "17px"
                } else {
                    relative
                }]
            );
        }
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .count(),
            3,
            "zero-sized terminals remain in the ordered receipt"
        );
        assert!(document.descendants().any(|node| {
            node.attribute("class") == Some("pieCircle")
                && node.attribute("fill") == Some("#abcdef")
        }));
        assert!(document.descendants().any(|node| {
            node.attribute("class") == Some("pieOuterCircle") && node.attribute("r") == Some("186")
        }));
        if base == "0px" && !positive_title {
            assert_eq!(
                document.root_element().attribute("viewBox"),
                Some("0 0 512 450")
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        let applied = if base != "0px" {
            5
        } else if positive_title {
            3
        } else {
            1
        };
        assert_eq!(evidence.required_count(), 5);
        assert_eq!(
            evidence.applied_count(),
            applied,
            "{base}, title={positive_title}"
        );
        assert_eq!(evidence.not_applicable_count(), 5 - applied);
        assert_eq!(evidence.accounted_count(), 5);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn pie_base_size_ownership_follows_the_selected_path_and_root_number_fallback() {
    let theme = pie_typography_theme(ThemeTextStyle::default().with_font_size_px(27.0).unwrap());
    for (variables, root, expected, applied) in [
        (serde_json::json!({}), serde_json::json!(32), "27px", 1),
        (serde_json::json!({}), serde_json::json!("32px"), "27px", 1),
        (
            serde_json::json!({"fontSize": "  "}),
            serde_json::Value::Null,
            "27px",
            1,
        ),
        (
            serde_json::json!({"fontSize": "bogus"}),
            serde_json::Value::Null,
            "27px",
            1,
        ),
        (
            serde_json::json!({"fontSize": "bogus"}),
            serde_json::json!("32px"),
            "27px",
            1,
        ),
        (
            serde_json::json!({"fontSize": "32px"}),
            serde_json::json!(48),
            "32px",
            0,
        ),
        (
            serde_json::json!({"fontSize": "bogus"}),
            serde_json::json!(32),
            "32px",
            0,
        ),
    ] {
        let mut variables = variables;
        variables["pieTitleTextSize"] = serde_json::json!("150%");
        variables["pieLegendTextSize"] = serde_json::json!("inherit");
        let mut config = serde_json::json!({"themeVariables": variables});
        if !root.is_null() {
            config["fontSize"] = root;
        }
        let source =
            format!("---\nconfig: {config}\n---\npie title Release audit\n  \"Alpha\" : 5\n");
        let rendered = try_render_pie_with_theme_requirement(
            &source,
            &theme,
            ThemePortabilityRequirement::RequirePortable,
        )
        .unwrap();
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman", "font-size"),
            [expected]
        );
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .slice", "font-size"),
            ["17px"]
        );
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .pieTitleText", "font-size"),
            ["150%"]
        );
        assert_eq!(
            pie_stylesheet_property_values(rendered.svg(), "#merman .legend text", "font-size"),
            ["inherit"]
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), applied);
        assert_eq!(evidence.not_applicable_count(), 1 - applied);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}
