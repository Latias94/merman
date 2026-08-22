mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{DiagramFamilyId, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette,
    OrdinalSelector, ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeVariant,
};
use merman_render::environment::{RenderEnvironment, TextMeasurementPolicy};
use merman_render::family;
use merman_render::model::PieDiagramLayout;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

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
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
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
        svg.contains(r#"viewBox="0 0 225 450""#),
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
fn pie_explicit_default_fill_remains_legacy_compatibility() {
    let theme = pie_slice_default_fill_theme(
        CanvasPaint::solid("#111827").expect("valid explicit-Default Pie fill"),
    );
    let rendered = render_pie_with_theme("pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n", &theme);
    let (slice_fills, _) = pie_terminal_fills(rendered.svg());
    assert_eq!(slice_fills, ["#111827", "#111827"]);

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 1);
}

#[test]
fn pie_ordinal_stroke_rules_fail_closed_for_matching_slice_occurrences() {
    let source = "pie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n";
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
        assert!(
            !rendered
                .svg()
                .contains("#merman .pieCircle{stroke:#7c3aed;"),
            "unsupported {case} ordinal stroke must not claim a terminal Pie rule"
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
fn pie_static_stroke_respects_per_site_mermaid_ownership() {
    let theme =
        pie_slice_stroke_theme(CanvasPaint::solid("#7c3aed").expect("valid typed Pie stroke"));
    for (owned_key, owned_color, owned_rule, typed_rule, forbidden_typed_rule) in [
        (
            "pieStrokeColor",
            "#0f172a",
            "#merman .pieCircle{stroke:#0f172a;stroke-width:",
            "#merman .pieOuterCircle{stroke:#7c3aed;stroke-width:",
            "#merman .pieCircle{stroke:#7c3aed;",
        ),
        (
            "pieOuterStrokeColor",
            "#1e293b",
            "#merman .pieOuterCircle{stroke:#1e293b;stroke-width:",
            "#merman .pieCircle{stroke:#7c3aed;stroke-width:",
            "#merman .pieOuterCircle{stroke:#7c3aed;",
        ),
    ] {
        let source = format!(
            "%%{{init: {{\"themeVariables\": {{\"{owned_key}\": \"{owned_color}\"}}}}}}%%\npie\n  \"Alpha\" : 1\n  \"Beta\" : 1\n"
        );
        let rendered = render_pie_with_theme(&source, &theme);
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|error| panic!("valid Pie SVG for {owned_key}: {error}"));
        let stylesheet = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .and_then(|node| node.text())
            .expect("Pie stylesheet");

        assert!(
            stylesheet.contains(owned_rule),
            "the explicit Mermaid {owned_key} value must retain terminal ownership: {stylesheet}"
        );
        assert!(
            !stylesheet.contains(forbidden_typed_rule),
            "typed Pie stroke must not overwrite the explicitly owned {owned_key} site: {stylesheet}"
        );
        assert!(
            stylesheet.contains(typed_rule),
            "typed Pie stroke must still own the independent site for {owned_key}: {stylesheet}"
        );
        assert_eq!(stylesheet.matches("#merman .pieCircle{").count(), 1);
        assert_eq!(stylesheet.matches("#merman .pieOuterCircle{").count(), 1);

        drop(document);
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
