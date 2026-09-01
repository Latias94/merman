mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{DiagramFamilyId, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramEffectSet, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec,
    EffectBinding, EffectGraph, EffectInput, EffectPrimitive, FontStack, OrdinalPalette,
    OrdinalSelector, ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
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
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");
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
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");

    artifact
        .render_svg(options, &SvgDebugOptions::default())
        .expect("svg render ok")
        .svg()
        .to_owned()
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
    let parsed = merman_render::__private::install_parse_compatibility(
        theme,
        legacy_init_theme_compat_engine(),
    )
    .parse_diagram_for_render_model_sync(text, ParseOptions::strict())
    .expect("parse themed Pie")
    .expect("detect themed Pie");
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Pie session");

    family::prepare(parsed, &LayoutOptions::default(), session)?
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
        assert_eq!(evidence.compatibility_residual_count(), 0);
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
    assert_eq!(source_evidence.compatibility_residual_count(), 0);

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
    assert_eq!(absent_evidence.compatibility_residual_count(), 0);
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
    assert_eq!(
        evidence.compatibility_residual_count(),
        0,
        "typed Pie title and section text routes have no compatibility residual"
    );
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
            [expected_font.clone()],
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn pie_font_size_only_is_explicitly_unsupported_without_a_legacy_bridge() {
    let typography = ThemeTextStyle::default()
        .with_font_size_px(24.0)
        .expect("valid unsupported Pie font size");
    let theme = pie_typography_theme(typography);
    let rendered = try_render_pie_with_theme_requirement(
        "pie title Fixed role sizes\n  \"Alpha\" : 1\n",
        &theme,
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("best-effort unsupported Pie font size should still render");

    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman .pieTitleText", "font-size"),
        ["25px"]
    );
    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman .slice", "font-size"),
        ["17px"]
    );
    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman .legend text", "font-size"),
        ["17px"]
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn pie_mixed_font_stack_and_size_keeps_the_stack_but_fails_closed() {
    let font_stack = FontStack::single("Pie Mixed").expect("valid mixed Pie font stack");
    let expected_font = font_stack.as_css();
    let typography = ThemeTextStyle::default()
        .with_font_stack(font_stack)
        .with_font_size_px(24.0)
        .expect("valid mixed Pie font size");
    let theme = pie_typography_theme(typography);
    let rendered = try_render_pie_with_theme_requirement(
        "pie title Mixed typography\n  \"Alpha\" : 1\n",
        &theme,
        ThemePortabilityRequirement::BestEffort,
    )
    .expect("best-effort mixed Pie typography should render");

    assert_eq!(
        pie_stylesheet_property_values(rendered.svg(), "#merman .pieTitleText", "font-family"),
        [expected_font]
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.accounted_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
        .expect("detect Pie resource-bound fixture");
    let session = RenderEnvironment::deterministic()
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Pie resource-bound session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)?;
    let rendered =
        artifact.render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

#[test]
fn pie_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"pie showData title Bounded distribution
  "Alpha" : 3
  "Beta" : 2
  "Gamma" : 1
"#;
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

fn render_pie_with_svg_limit(
    text: &str,
    diagram_id: &str,
    maximum: usize,
) -> merman_render::Result<String> {
    let engine = legacy_init_theme_compat_engine();
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, maximum)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
        .begin_session()
        .expect("begin bounded Pie session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).expect("layout ok");

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
    let expected_left_offset = root_viewbox_width(&left_svg) - 490.0;
    assert!(
        (left_offset.0 - expected_left_offset).abs() <= 1.0e-9
            && left_offset.1.abs() <= f64::EPSILON,
        "left legend should move the pie group right by legend width: {left_svg}"
    );
    assert!(left_svg.contains(r#"class="legend" transform="translate(-207,-22)""#));
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
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
        assert_eq!(evidence.compatibility_residual_count(), 0, "{case}");

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
        assert_eq!(evidence.compatibility_residual_count(), 0);
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
        assert_eq!(evidence.compatibility_residual_count(), 0);
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
        assert_eq!(evidence.compatibility_residual_count(), 0, "{owned_key}");
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
    assert_eq!(evidence.compatibility_residual_count(), 0);
}
