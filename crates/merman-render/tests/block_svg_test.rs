mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, FontStack, OrdinalPalette,
    OrdinalSelector, ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet,
    ThemeStylePatch, ThemeTarget, ThemeTextStyle, ThemeVariant, TypographySpec,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{DiagramFamilyId, LayoutOptions};
use regex::Regex;

fn render_block_svg_from_text(text: &str) -> String {
    let engine = Engine::new();
    render_block_svg_from_text_with_engine(&engine, text)
}

fn render_block_svg_from_text_with_engine(engine: &Engine, text: &str) -> String {
    try_render_block_svg_from_text_with_engine_and_policy(
        engine,
        text,
        RenderResourcePolicy::interactive(),
    )
    .expect("svg render ok")
}

fn try_render_block_svg_from_text_with_engine(
    engine: &Engine,
    text: &str,
) -> merman_render::Result<String> {
    try_render_block_svg_from_text_with_engine_and_policy(
        engine,
        text,
        RenderResourcePolicy::interactive(),
    )
}

fn try_render_block_svg_from_text_with_engine_and_policy(
    engine: &Engine,
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let parsed = engine
        .parse_diagram_for_render_model_sync(text, ParseOptions::default())
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)?;

    Ok(artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())?
        .svg()
        .to_owned())
}

fn translated_center(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let transform = node.attribute("transform").expect("node transform");
    let captures = Regex::new(
        r"^translate\(\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*,\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*\)$",
    )
    .expect("valid transform regex")
    .captures(transform)
    .expect("translate(x, y) transform");
    (
        captures[1].parse().expect("numeric translate x"),
        captures[2].parse().expect("numeric translate y"),
    )
}

fn path_start(path: roxmltree::Node<'_, '_>) -> (f64, f64) {
    let d = path.attribute("d").expect("path data");
    let captures =
        Regex::new(r"^M\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*,\s*(-?(?:\d+(?:\.\d*)?|\.\d+))")
            .expect("valid path regex")
            .captures(d)
            .expect("path starts with an absolute move");
    (
        captures[1].parse().expect("numeric path x"),
        captures[2].parse().expect("numeric path y"),
    )
}

fn root_view_box(document: &roxmltree::Document<'_>) -> (f64, f64, f64, f64) {
    let root = document.root_element();
    let values = root
        .attribute("viewBox")
        .expect("root viewBox")
        .split_ascii_whitespace()
        .map(|value| value.parse::<f64>().expect("numeric viewBox component"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 4, "viewBox must contain four components");
    (values[0], values[1], values[2], values[3])
}

fn parse_translate(raw: &str) -> (f64, f64) {
    let captures = Regex::new(
        r"^translate\(\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*,\s*(-?(?:\d+(?:\.\d*)?|\.\d+))\s*\)$",
    )
    .expect("valid transform regex")
    .captures(raw)
    .expect("translate(x, y) transform");
    (
        captures[1].parse().expect("numeric translate x"),
        captures[2].parse().expect("numeric translate y"),
    )
}

fn global_foreign_object_bounds(foreign_object: roxmltree::Node<'_, '_>) -> (f64, f64, f64, f64) {
    let width = foreign_object
        .attribute("width")
        .expect("foreignObject width")
        .parse::<f64>()
        .expect("numeric foreignObject width");
    let height = foreign_object
        .attribute("height")
        .expect("foreignObject height")
        .parse::<f64>()
        .expect("numeric foreignObject height");
    let x = foreign_object
        .attribute("x")
        .unwrap_or("0")
        .parse::<f64>()
        .expect("numeric foreignObject x");
    let y = foreign_object
        .attribute("y")
        .unwrap_or("0")
        .parse::<f64>()
        .expect("numeric foreignObject y");

    let (mut tx, mut ty) = (x, y);
    for ancestor in foreign_object.ancestors().filter(|node| node.is_element()) {
        if let Some(transform) = ancestor.attribute("transform") {
            let (ancestor_x, ancestor_y) = parse_translate(transform);
            tx += ancestor_x;
            ty += ancestor_y;
        }
    }
    (tx, ty, tx + width, ty + height)
}

fn global_path_geometry_bounds(path: roxmltree::Node<'_, '_>) -> (f64, f64, f64, f64) {
    let d = path.attribute("d").expect("path data");
    let fragment = format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><path d="{d}"/></svg>"#);
    let bounds = merman_render::svg::debug_svg_emitted_bounds(&fragment)
        .expect("path geometry bounds")
        .bounds;
    let (mut tx, mut ty) = (0.0, 0.0);
    for ancestor in path.ancestors().filter(|node| node.is_element()) {
        if let Some(transform) = ancestor.attribute("transform") {
            let (ancestor_x, ancestor_y) = parse_translate(transform);
            tx += ancestor_x;
            ty += ancestor_y;
        }
    }
    (
        bounds.min_x + tx,
        bounds.min_y + ty,
        bounds.max_x + tx,
        bounds.max_y + ty,
    )
}

fn deep_block_chain(depth: usize) -> String {
    let mut input = String::from("block\n");
    for level in 0..depth {
        input.push_str(&format!("block:n{level}[\"n{level}\"]\n"));
    }
    input.push_str("leaf[\"leaf\"]\n");
    for _ in 0..depth {
        input.push_str("end\n");
    }
    input
}

fn block_node_stroke_theme(stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_stroke(stroke),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .expect("compile Block node stroke theme")
}

fn block_node_fill_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(fill),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .expect("compile Block node fill theme")
}

fn block_node_fill_and_stroke_theme(fill: CanvasPaint, stroke: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(fill)
                            .with_stroke(stroke),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .expect("compile Block node fill and stroke theme")
}

fn block_node_fill_with_ordinal_palette_theme(fill: CanvasPaint) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default().with_fill(fill),
                        )
                        .for_family(DiagramFamilyId::BLOCK),
                    )
                    .with_ordinal_palette(
                        ThemeTarget::Node,
                        OrdinalPalette::new([
                            ThemeColorValue::parse("#123456").expect("valid palette color")
                        ])
                        .expect("non-empty Block node palette"),
                    ),
            ),
        )
        .expect("compile Block node fill and ordinal palette theme")
}

fn block_node_ordinal_palette_theme() -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Node,
                    OrdinalPalette::new([
                        ThemeColorValue::parse("#123456").expect("valid palette color")
                    ])
                    .expect("non-empty Block node palette"),
                ),
            ),
        )
        .expect("compile Block node ordinal palette theme")
}

fn block_typography_theme(font_family: &str, font_size_px: f32) -> DiagramTheme {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single(font_family).expect("valid Block font stack"))
        .with_font_size_px(font_size_px)
        .expect("valid Block font size");
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::BLOCK, typography),
        ))
        .expect("compile Block typography theme")
}

fn render_block_with_theme_and_engine(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
) -> family::RenderedFamilySvg {
    render_block_with_theme_requirement(
        source,
        theme,
        engine,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn render_block_with_theme_requirement(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    requirement: ThemePortabilityRequirement,
) -> family::RenderedFamilySvg {
    let parsed = merman_render::__private::install_parse_compatibility(theme, engine)
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse themed Block")
        .expect("detect themed Block");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(requirement)
        .begin_session_with_theme(theme)
        .expect("begin themed Block session");

    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare themed Block")
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("block-theme".to_string()),
                ..SvgRenderOptions::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render themed Block")
}

fn block_node_label_theme(patch: ThemeStylePatch) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                ThemeRule::new(ThemeTarget::NodeLabel, patch).for_family(DiagramFamilyId::BLOCK),
            )),
        )
        .unwrap()
}

#[test]
fn block_node_label_paint_is_bound_to_node_terminals() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for html in [false, true] {
        let rendered = render_block_with_theme_and_engine(
            "block\n A[\"Alpha\"] -- \"Edge\" --> B[\"Beta\"]\n",
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"htmlLabels": html}),
            )),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let stylesheet = document
            .descendants()
            .find(|n| n.has_tag_name("style"))
            .unwrap()
            .text()
            .unwrap();
        assert!(
            !stylesheet.contains("#b316cd"),
            "node paint must not replace shared label CSS"
        );
        for id in ["block-theme-A", "block-theme-B"] {
            let node = document
                .descendants()
                .find(|n| n.attribute("id") == Some(id))
                .unwrap();
            let terminal = node
                .descendants()
                .find(|n| n.has_tag_name(if html { "p" } else { "text" }))
                .unwrap();
            assert!(
                terminal
                    .attribute("style")
                    .unwrap_or("")
                    .contains("#b316cd")
            );
        }
        let edge = document
            .descendants()
            .find(|node| node.attribute("class") == Some("edgeLabel"))
            .expect("edge label terminal");
        assert!(
            !edge
                .descendants()
                .any(|node| node.attribute("style").unwrap_or("").contains("#b316cd"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
    }
}

#[test]
fn block_empty_node_label_paint_is_not_applicable() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for html in [false, true] {
        let rendered = render_block_with_theme_and_engine(
            "block\n A[\" \"]\n",
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"htmlLabels": html}),
            )),
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[test]
fn block_node_label_source_color_suppresses_theme_evidence() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for html in [false, true] {
        let rendered = render_block_with_theme_and_engine(
            "block\n A[\"Alpha\"]\n style A color:#123456\n",
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"htmlLabels": html}),
            )),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let node = document
            .descendants()
            .find(|n| n.attribute("id") == Some("block-theme-A"))
            .unwrap();
        assert!(
            node.descendants()
                .any(|n| n.attribute("style").unwrap_or("").contains("#123456"))
        );
        assert!(
            !node
                .descendants()
                .any(|n| n.attribute("style").unwrap_or("").contains("#b316cd"))
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[test]
fn block_node_label_overwritten_rule_has_no_residual() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::NodeLabel,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#abcdef").unwrap()),
                        )
                        .with_variant(ThemeVariant::Default)
                        .for_family(DiagramFamilyId::BLOCK),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::NodeLabel,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap()),
                        )
                        .for_family(DiagramFamilyId::BLOCK),
                    ),
            ),
        )
        .unwrap();
    let rendered =
        render_block_with_theme_and_engine("block\n A[\"Alpha\"]\n", &theme, Engine::new());
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn block_node_label_mixed_rule_cannot_certify_unconsumed_stroke() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke(CanvasPaint::solid("#abcdef").unwrap()),
    );
    let rendered = render_block_with_theme_requirement(
        "block\n A[\"Alpha\"]\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert!(
        evidence.theme_residual_count() > 0
            || evidence.accounted_count() < evidence.required_count()
    );
}

#[test]
fn block_node_label_mixed_rule_is_rejected_in_strict_mode() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#123456").unwrap())
            .with_stroke(CanvasPaint::solid("#abcdef").unwrap()),
    );
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync("block\n A[\"Alpha\"]\n", ParseOptions::strict())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
        .begin_session_with_theme(&theme)
        .unwrap();
    let artifact =
        family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
    assert!(
        artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .is_err()
    );
}

#[test]
fn block_node_label_mixed_source_ownership_proves_only_theme_nodes() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for html in [false, true] {
        let rendered = render_block_with_theme_and_engine(
            "block\n A[\"Alpha\"] B[\"Beta\"]\n style A color:#123456\n",
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"htmlLabels": html}),
            )),
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for (id, color) in [("block-theme-A", "#123456"), ("block-theme-B", "#b316cd")] {
            let node = document
                .descendants()
                .find(|n| n.attribute("id") == Some(id))
                .unwrap();
            assert!(
                node.descendants()
                    .any(|n| n.attribute("style").unwrap_or("").contains(color))
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
    }
}

#[test]
fn block_node_label_config_color_is_not_a_typed_application() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for path in ["nodeTextColor", "primaryTextColor", "textColor"] {
        let rendered = render_block_with_theme_and_engine(
            "block\n A[\"Alpha\"]\n",
            &theme,
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"themeVariables": {path: "#123456"}}),
            )),
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0, "{path}");
        assert_eq!(evidence.not_applicable_count(), 1, "{path}");
    }
}

#[test]
fn block_node_label_preserves_generic_text_author_order() {
    for node_last in [false, true] {
        let targets = if node_last {
            [ThemeTarget::Text, ThemeTarget::NodeLabel]
        } else {
            [ThemeTarget::NodeLabel, ThemeTarget::Text]
        };
        let rules = targets
            .into_iter()
            .fold(ThemeRuleSet::default(), |rules, target| {
                let color = if target == ThemeTarget::NodeLabel {
                    "#b316cd"
                } else {
                    "#123456"
                };
                rules.with_rule(
                    ThemeRule::new(
                        target,
                        ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap()),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                )
            });
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap();
        let rendered = render_block_with_theme_requirement(
            "block\n A[\"Alpha\"]\n",
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let node = document
            .descendants()
            .find(|n| n.attribute("id") == Some("block-theme-A"))
            .unwrap();
        assert_eq!(
            node.descendants()
                .any(|n| n.attribute("style").unwrap_or("").contains("#b316cd")),
            node_last
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(node_last));
        if !node_last {
            assert_eq!(evidence.not_applicable_count(), 1);
        }
    }
}

#[test]
fn block_node_label_class_color_owns_only_assigned_classes() {
    let theme = block_node_label_theme(
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
    );
    for html in [false, true] {
        for (source, owned) in [
            (
                "block\n A[\"Alpha\"]\n class A paint\n classDef paint color:#123456\n",
                true,
            ),
            (
                "block\n A[\"Alpha\"]\n classDef default color:#123456\n",
                true,
            ),
            (
                "block\n A[\"Alpha\"]\n class A plain\n classDef plain stroke:#123456\n classDef default color:#654321\n",
                false,
            ),
        ] {
            let rendered = render_block_with_theme_and_engine(
                source,
                &theme,
                Engine::new().with_site_config(MermaidConfig::from_value(
                    serde_json::json!({"htmlLabels": html}),
                )),
            );
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let node = document
                .descendants()
                .find(|n| n.attribute("id") == Some("block-theme-A"))
                .unwrap();
            assert_eq!(
                node.descendants()
                    .any(|n| n.attribute("style").unwrap_or("").contains("#b316cd")),
                !owned
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), usize::from(!owned));
            assert_eq!(evidence.not_applicable_count(), usize::from(owned));
        }
    }
}

#[test]
fn block_node_label_ordinal_winner_keeps_an_unsupported_residual() {
    let rules = ThemeRuleSet::default()
        .with_rule(
            ThemeRule::new(
                ThemeTarget::NodeLabel,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
            )
            .for_family(DiagramFamilyId::BLOCK),
        )
        .with_rule(
            ThemeRule::new(
                ThemeTarget::NodeLabel,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )
            .with_ordinal(OrdinalSelector::exact(2).unwrap())
            .for_family(DiagramFamilyId::BLOCK),
        );
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(rules))
        .unwrap();
    let rendered = render_block_with_theme_requirement(
        "block\n A[\"Alpha\"] B[\"Beta\"]\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn block_explicit_default_font_size_has_a_typed_terminal_receipt() {
    for explicit in [false, true] {
        let style = if explicit {
            ThemeTextStyle::default().with_font_size_px(16.0).unwrap()
        } else {
            ThemeTextStyle::default()
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::BLOCK, style),
            ))
            .unwrap();
        let rendered =
            render_block_with_theme_and_engine("block\n A[\"Alpha\"]\n", &theme, Engine::new());
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), usize::from(explicit));
        assert_eq!(evidence.applied_count(), usize::from(explicit));
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn block_typed_typography_reaches_layout_css_and_strict_receipt() {
    let theme = block_typography_theme("Block Sans", 22.0);
    let rendered = render_block_with_theme_and_engine(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
        &theme,
        Engine::new(),
    );
    let svg = rendered.svg();
    assert!(
        svg.contains("#block-theme{font-family:\"Block Sans\";font-size:22px;"),
        "typed Block typography must reach the writer-owned root CSS: {svg}"
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_typography_receipt_matches_writer_number_formatting() {
    let theme = block_typography_theme("Block Sans", 1.0000001);
    let rendered = render_block_with_theme_and_engine(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
        &theme,
        Engine::new(),
    );
    assert!(
        rendered
            .svg()
            .contains("#block-theme{font-family:\"Block Sans\";font-size:1px;")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_mixed_base_typography_fails_closed() {
    let typography = ThemeTextStyle::default()
        .with_font_stack(FontStack::single("Mixed Block Font").expect("valid Block font stack"))
        .with_font_size_px(22.0)
        .expect("valid Block font size")
        .with_font_weight(700)
        .expect("valid unsupported Block font weight");
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::BLOCK, typography),
        ))
        .expect("compile mixed Block typography theme");
    let rendered = render_block_with_theme_requirement(
        "block\n  A[\"Alpha\"]\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 3);
    assert_eq!(evidence.accounted_count(), 3);
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn block_explicit_typography_ownership_outranks_typed_values() {
    let theme = block_typography_theme("Typed Block Sans", 22.0);
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "fontFamily": "Site Block Sans",
        "themeVariables": { "fontSize": "30px" }
    })));
    let rendered = render_block_with_theme_and_engine(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
        &theme,
        engine,
    );
    let svg = rendered.svg();
    assert!(svg.contains("#block-theme{font-family:Site Block Sans;font-size:30px;"));
    assert!(!svg.contains("Typed Block Sans"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 2);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

fn terminal_shell_styles(svg: &str, node_id: &str) -> Vec<(String, String)> {
    let document = roxmltree::Document::parse(svg).expect("valid Block SVG");
    let node = document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .unwrap_or_else(|| panic!("missing Block node {node_id}: {svg}"));

    node.descendants()
        .filter_map(|shell| {
            if !shell.is_element() {
                return None;
            }
            let tag = shell.tag_name().name();
            matches!(tag, "rect" | "circle" | "path" | "polygon")
                .then(|| shell.attribute("style").map(|style| (tag, style)))
                .flatten()
        })
        .map(|(tag, style)| (tag.to_string(), style.to_string()))
        .collect()
}

fn terminal_node_has_class(svg: &str, node_id: &str, expected_class: &str) -> bool {
    let document = roxmltree::Document::parse(svg).expect("valid Block SVG");
    document
        .descendants()
        .find(|node| node.attribute("id") == Some(node_id))
        .and_then(|node| node.attribute("class"))
        .is_some_and(|classes| {
            classes
                .split_ascii_whitespace()
                .any(|class| class == expected_class)
        })
}

fn terminal_stroke(style: &str) -> Option<&str> {
    style
        .split(';')
        .filter_map(|declaration| declaration.trim().split_once(':'))
        .filter(|(property, _)| property.trim().eq_ignore_ascii_case("stroke"))
        .map(|(_, value)| value.trim())
        .next_back()
}

fn terminal_fill(style: &str) -> Option<&str> {
    style
        .split(';')
        .filter_map(|declaration| declaration.trim().split_once(':'))
        .filter(|(property, _)| property.trim().eq_ignore_ascii_case("fill"))
        .map(|(_, value)| value.trim())
        .next_back()
}

#[test]
fn block_typed_node_fill_reaches_every_terminal_shape() {
    let source = r#"block-beta
  columns 5
  rect["Rect"] circle(("Circle")) double((("Double"))) cylinder[("Cylinder")] polygon{{"Polygon"}}
"#;
    let cases = [
        (
            CanvasPaint::solid("#654321").expect("valid Block node fill"),
            "#654321",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    for (fill, expected_fill) in cases {
        let theme = block_node_fill_theme(fill);
        let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());
        let expected_shells = [
            ("block-theme-rect", ["rect"].as_slice()),
            ("block-theme-circle", ["circle"].as_slice()),
            ("block-theme-double", ["circle", "circle"].as_slice()),
            ("block-theme-cylinder", ["path"].as_slice()),
            ("block-theme-polygon", ["polygon"].as_slice()),
        ];

        for (node_id, expected_tags) in expected_shells {
            let shells = terminal_shell_styles(rendered.svg(), node_id);
            assert_eq!(
                shells
                    .iter()
                    .map(|(tag, _)| tag.as_str())
                    .collect::<Vec<_>>(),
                expected_tags,
                "unexpected terminal shells for {node_id}: {}",
                rendered.svg()
            );
            assert!(
                shells
                    .iter()
                    .all(|(_, style)| terminal_fill(style) == Some(expected_fill)),
                "typed fill must reach every terminal shell for {node_id}: {shells:?}"
            );
        }

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn block_ordinal_palette_is_not_applicable_when_typed_fill_wins() {
    let theme = block_node_fill_with_ordinal_palette_theme(
        CanvasPaint::solid("#654321").expect("valid typed Block node fill"),
    );
    let rendered =
        render_block_with_theme_and_engine("block\n  A[\"Alpha\"]\n", &theme, Engine::new());
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn block_ordinal_palette_is_not_applicable_when_source_fill_wins() {
    let source = r#"block
  A["Alpha"]
  classDef default fill:#bb0000
"#;
    let rendered = render_block_with_theme_and_engine(
        source,
        &block_node_ordinal_palette_theme(),
        Engine::new(),
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());

    assert_eq!(evidence.theme_residual_count(), 0);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn block_typed_node_fill_and_stroke_share_the_same_shell_truth() {
    let theme = block_node_fill_and_stroke_theme(
        CanvasPaint::solid("#654321").expect("valid Block node fill"),
        CanvasPaint::solid("#123456").expect("valid Block node stroke"),
    );
    let rendered = render_block_with_theme_and_engine(
        "block-beta\n  double(((\"Double\")))\n",
        &theme,
        Engine::new(),
    );
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-double");

    assert_eq!(shells.len(), 2, "expected both circle shells: {shells:?}");
    assert!(
        shells.iter().all(|(_, style)| {
            terminal_fill(style) == Some("#654321") && terminal_stroke(style) == Some("#123456")
        }),
        "fill and stroke must be proved on every shell: {shells:?}"
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_mixed_node_paint_rule_reconciles_source_ownership_per_property_and_node() {
    let source = r#"block-beta
  columns 2
  fillOwned["Fill owned"] strokeOwned(("Stroke owned"))
  style fillOwned fill:#0000bb
  style strokeOwned stroke:#00bb00
"#;
    let theme = block_node_fill_and_stroke_theme(
        CanvasPaint::solid("#654321").expect("valid Block node fill"),
        CanvasPaint::solid("#123456").expect("valid Block node stroke"),
    );
    let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());

    let fill_owned = terminal_shell_styles(rendered.svg(), "block-theme-fillOwned");
    assert_eq!(terminal_fill(&fill_owned[0].1), Some("#0000bb"));
    assert_eq!(terminal_stroke(&fill_owned[0].1), Some("#123456"));

    let stroke_owned = terminal_shell_styles(rendered.svg(), "block-theme-strokeOwned");
    assert_eq!(terminal_fill(&stroke_owned[0].1), Some("#654321"));
    assert_eq!(terminal_stroke(&stroke_owned[0].1), Some("#00bb00"));

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_source_owned_fill_does_not_hide_direct_stroke_evidence() {
    let source = r#"block-beta
  columns 2
  first["First"] second(("Second"))
  classDef default fill:#bb0000
"#;
    let theme = block_node_fill_and_stroke_theme(
        CanvasPaint::Transparent,
        CanvasPaint::solid("#123456").expect("valid Block node stroke"),
    );
    let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());

    for node_id in ["block-theme-first", "block-theme-second"] {
        assert!(
            terminal_node_has_class(rendered.svg(), node_id, "default"),
            "source-owned default class must be attached to {node_id}: {}",
            rendered.svg()
        );
        let shells = terminal_shell_styles(rendered.svg(), node_id);
        assert!(
            shells.iter().all(|(_, style)| {
                terminal_fill(style) != Some("transparent")
                    && terminal_stroke(style) == Some("#123456")
            }),
            "source fill and typed stroke must remain property-local for {node_id}: {shells:?}"
        );
    }

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_inline_and_class_fills_outrank_typed_node_fill_per_node() {
    let source = r#"block-beta
  columns 3
  bare["Bare"] assigned(("Assigned")) inline{{"Inline"}}
  classDef accent fill:#00bb00
  class assigned accent
  style inline fill:#0000bb
"#;
    let theme =
        block_node_fill_theme(CanvasPaint::solid("#654321").expect("valid Block node fill"));
    let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());

    let bare = terminal_shell_styles(rendered.svg(), "block-theme-bare");
    assert_eq!(terminal_fill(&bare[0].1), Some("#654321"), "{bare:?}");
    let inline = terminal_shell_styles(rendered.svg(), "block-theme-inline");
    assert_eq!(terminal_fill(&inline[0].1), Some("#0000bb"), "{inline:?}");
    let assigned = terminal_shell_styles(rendered.svg(), "block-theme-assigned");
    assert!(
        assigned
            .iter()
            .all(|(_, style)| terminal_fill(style) != Some("#654321")),
        "assigned class must suppress typed inline fill: {assigned:?}"
    );
    assert!(
        terminal_node_has_class(rendered.svg(), "block-theme-assigned", "accent"),
        "the source-owned class must be attached to the assigned node: {}",
        rendered.svg()
    );
    assert!(
        rendered
            .svg()
            .contains(r#"#block-theme .accent&gt;*{fill:#00bb00!important;}"#),
        "assigned class fill must retain source ownership: {}",
        rendered.svg()
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_default_class_fill_makes_typed_node_fill_not_applicable() {
    let source = r#"block
  A["Alpha"]
  classDef default fill:#bb0000
"#;
    let theme = block_node_fill_theme(
        CanvasPaint::solid("#654321").expect("valid suppressed Block node fill"),
    );
    let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-A");

    assert!(
        shells
            .iter()
            .all(|(_, style)| terminal_fill(style) != Some("#654321")),
        "default class must suppress typed inline fill: {shells:?}"
    );
    assert!(
        terminal_node_has_class(rendered.svg(), "block-theme-A", "default"),
        "the implicit default class must be attached to the node: {}",
        rendered.svg()
    );
    assert!(
        rendered
            .svg()
            .contains(r#"#block-theme .default&gt;*{fill:#bb0000!important;}"#),
        "default class fill must retain source ownership: {}",
        rendered.svg()
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_explicit_main_background_outranks_typed_node_fill() {
    let theme = block_node_fill_theme(
        CanvasPaint::solid("#654321").expect("valid config-suppressed Block node fill"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "mainBkg": "#445566" }
    })));
    let rendered = render_block_with_theme_and_engine("block\n  A[\"Alpha\"]\n", &theme, engine);
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-A");

    assert!(
        shells
            .iter()
            .all(|(_, style)| terminal_fill(style) != Some("#654321")),
        "explicit mainBkg must suppress typed inline fill: {shells:?}"
    );
    assert!(
        rendered.svg().contains("fill:#445566;stroke:"),
        "explicit mainBkg must remain the Block base fill: {}",
        rendered.svg()
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_explicit_default_fill_uses_the_typed_writer() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid("#654321")
                                .expect("valid legacy-compatible Block node fill"),
                        ),
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .expect("compile explicit Default Block node fill");
    let rendered = render_block_with_theme_requirement(
        "block\n  A[\"Alpha\"]\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-A");

    assert!(
        shells
            .iter()
            .any(|(_, style)| terminal_fill(style) == Some("#654321")),
        "explicit Default must use the typed writer: {shells:?}"
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);
}

#[test]
fn block_typed_node_stroke_reaches_every_terminal_shape() {
    let source = r#"block-beta
  columns 5
  rect["Rect"] circle(("Circle")) double((("Double"))) cylinder[("Cylinder")] polygon{{"Polygon"}}
"#;
    let cases = [
        (
            CanvasPaint::solid("#123456").expect("valid Block node stroke"),
            "#123456",
        ),
        (CanvasPaint::Transparent, "transparent"),
    ];

    for (stroke, expected_stroke) in cases {
        let theme = block_node_stroke_theme(stroke);
        let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());
        let expected_shells = [
            ("block-theme-rect", ["rect"].as_slice()),
            ("block-theme-circle", ["circle"].as_slice()),
            ("block-theme-double", ["circle", "circle"].as_slice()),
            ("block-theme-cylinder", ["path"].as_slice()),
            ("block-theme-polygon", ["polygon"].as_slice()),
        ];

        for (node_id, expected_tags) in expected_shells {
            let shells = terminal_shell_styles(rendered.svg(), node_id);
            assert_eq!(
                shells
                    .iter()
                    .map(|(tag, _)| tag.as_str())
                    .collect::<Vec<_>>(),
                expected_tags,
                "unexpected terminal shells for {node_id}: {}",
                rendered.svg()
            );
            assert!(
                shells
                    .iter()
                    .all(|(_, style)| terminal_stroke(style) == Some(expected_stroke)),
                "typed stroke must reach every terminal shell for {node_id}: {shells:?}"
            );
        }

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn block_inline_and_class_strokes_outrank_typed_node_stroke_per_node() {
    let source = r#"block-beta
  columns 3
  bare["Bare"] assigned(("Assigned")) inline{{"Inline"}}
  classDef accent stroke:#00bb00
  class assigned accent
  style inline stroke:#0000bb
"#;
    let theme =
        block_node_stroke_theme(CanvasPaint::solid("#123456").expect("valid Block node stroke"));
    let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());

    let bare = terminal_shell_styles(rendered.svg(), "block-theme-bare");
    assert_eq!(terminal_stroke(&bare[0].1), Some("#123456"), "{bare:?}");
    let inline = terminal_shell_styles(rendered.svg(), "block-theme-inline");
    assert_eq!(terminal_stroke(&inline[0].1), Some("#0000bb"), "{inline:?}");
    let assigned = terminal_shell_styles(rendered.svg(), "block-theme-assigned");
    assert!(
        assigned
            .iter()
            .all(|(_, style)| terminal_stroke(style) != Some("#123456")),
        "assigned class must suppress typed inline stroke: {assigned:?}"
    );
    assert!(
        terminal_node_has_class(rendered.svg(), "block-theme-assigned", "accent"),
        "the source-owned class must be attached to the assigned node: {}",
        rendered.svg()
    );
    assert!(
        rendered
            .svg()
            .contains(r#"#block-theme .accent&gt;*{stroke:#00bb00!important;}"#),
        "assigned class stroke must retain source ownership: {}",
        rendered.svg()
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_default_class_stroke_makes_typed_node_stroke_not_applicable() {
    let source = r#"block
  A["Alpha"]
  classDef default stroke:#bb0000
"#;
    let theme = block_node_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid suppressed Block node stroke"),
    );
    let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-A");

    assert!(
        shells
            .iter()
            .all(|(_, style)| terminal_stroke(style) != Some("#123456")),
        "default class must suppress typed inline stroke: {shells:?}"
    );
    assert!(
        terminal_node_has_class(rendered.svg(), "block-theme-A", "default"),
        "the implicit default class must be attached to the node: {}",
        rendered.svg()
    );
    assert!(
        rendered
            .svg()
            .contains(r#"#block-theme .default&gt;*{stroke:#bb0000!important;}"#),
        "default class stroke must retain source ownership: {}",
        rendered.svg()
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_explicit_node_border_outranks_typed_node_stroke() {
    let theme = block_node_stroke_theme(
        CanvasPaint::solid("#123456").expect("valid config-suppressed Block node stroke"),
    );
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": { "nodeBorder": "#445566" }
    })));
    let rendered = render_block_with_theme_and_engine("block\n  A[\"Alpha\"]\n", &theme, engine);
    let shells = terminal_shell_styles(rendered.svg(), "block-theme-A");

    assert!(
        shells
            .iter()
            .all(|(_, style)| terminal_stroke(style) != Some("#123456")),
        "explicit nodeBorder must suppress typed inline stroke: {shells:?}"
    );
    assert!(
        rendered.svg().contains("stroke:#445566;stroke-width:1px"),
        "explicit nodeBorder must remain the Block base stroke: {}",
        rendered.svg()
    );

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_svg_scopes_text_and_edge_colors_for_html_labels() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
    );

    assert!(
        !svg.contains("<style></style>"),
        "expected block SVG to emit scoped CSS instead of an empty style element"
    );
    assert!(
        svg.contains(r#"#merman .label text,#merman span,#merman p{fill:#333;color:#333;}"#),
        "expected block HTML/SVG labels to avoid inheriting host page text color"
    );
    assert!(
        svg.contains(r#"#merman .flowchart-link{stroke:#333333;fill:none;}"#),
        "expected block edges to carry their scoped stroke color"
    );
}

#[test]
fn block_public_svg_render_handles_deep_chain() {
    const DEPTH: usize = 1200;
    let svg = try_render_block_svg_from_text_with_engine_and_policy(
        &Engine::new(),
        &deep_block_chain(DEPTH),
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("deep Block SVG render");

    assert!(
        svg.contains(r#"id="merman-leaf""#),
        "expected deep Block leaf to render without stack-dependent traversal"
    );
}

#[test]
fn block_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
    let source = r#"block
  A["Alpha"] --> B["Beta"]
  classDef branded fill:#696,stroke:#333
  class A branded
"#;
    let engine = Engine::new();
    let baseline = try_render_block_svg_from_text_with_engine_and_policy(
        &engine,
        source,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .expect("render the unbounded Block baseline");
    let exact_bytes = baseline.len();
    assert!(exact_bytes > 1, "Block fixture must emit a non-empty SVG");

    let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
        .expect("valid exact Block SVG byte ceiling");
    let exact =
        try_render_block_svg_from_text_with_engine_and_policy(&engine, source, exact_policy)
            .expect("the exact Block family SVG byte ceiling must succeed");
    assert_eq!(exact.as_bytes(), baseline.as_bytes());

    let below_exact = exact_bytes - 1;
    let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
        .expect("valid below-exact Block SVG byte ceiling");
    let error =
        try_render_block_svg_from_text_with_engine_and_policy(&engine, source, below_policy)
            .expect_err("one byte below the Block family SVG size must fail");
    let merman_render::Error::ResourceLimitExceeded(limit) = error else {
        panic!("expected Block MaxSvgBytes rejection, got {error}");
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
fn block_svg_honors_visible_edge_stroke_width_theme() {
    let engine = legacy_init_theme_compat_engine();
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        r##"%%{init: {"themeVariables": {"strokeWidth": 4, "lineColor": "#112233"}}}%%
block
  A --> B
"##,
    );

    assert!(
        svg.contains(r#"#merman .edge-thickness-normal{stroke-width:4px;}"#),
        "expected shared Mermaid edge thickness CSS to reach visible Block edges: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .edgePaths .path{stroke:#112233;stroke-width:2.0px;}"#),
        "expected Block edge-path CSS to use Mermaid's edgePaths contract: {svg}"
    );
    assert!(
        svg.contains(r#"class="edge-thickness-normal edge-pattern-solid edge-thickness-normal edge-pattern-solid flowchart-link LS-a1 LE-b1""#),
        "expected Block edge path to carry the themed edge-thickness-normal class: {svg}"
    );
}

#[test]
fn block_svg_uses_mermaid_11_15_dom_ids_and_html_label_shape() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"] --> B["Beta"]
"#,
    );

    assert!(
        svg.contains(r#"id="merman-A""#),
        "expected Block node DOM id to be diagram-prefixed: {svg}"
    );
    assert!(
        svg.contains(r#"id="merman-1-A-B""#),
        "expected Block edge DOM id to be diagram-prefixed: {svg}"
    );
    assert!(
        svg.contains(r#"style="display: table-cell; white-space: nowrap; line-height: 1.5;"><span class="nodeLabel"><p>Alpha</p></span>"#),
        "expected Block node label to use Mermaid 11.15 XHTML paragraph shape: {svg}"
    );
}

#[test]
fn block_svg_keeps_blank_placeholder_label_paragraph() {
    let svg = render_block_svg_from_text(
        r#"block
  blockArrowId6<["   "]>(down)
"#,
    );

    assert!(
        svg.contains(r#"<span class="nodeLabel"><p>   </p></span>"#),
        "expected blank Block placeholder labels to keep Mermaid's paragraph child: {svg}"
    );
}

#[test]
fn block_svg_honors_configured_node_text_color() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "nodeTextColor": "#123456"
        }
    })));
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        r#"block
  A["Alpha"]
"#,
    );

    assert!(
        svg.contains(r#"#merman .label text,#merman span,#merman p{fill:#123456;color:#123456;}"#),
        "expected nodeTextColor theme variable to drive block label color"
    );
}

#[test]
fn block_svg_fades_cluster_theme_colors() {
    let engine = legacy_init_theme_compat_engine();
    let svg = render_block_svg_from_text_with_engine(
        &engine,
        r##"%%{init: {"themeVariables": {"clusterBkg": "rebeccapurple", "clusterBorder": "hsl(80, 100%, 96.2745098039%)"}}}%%
block
  block
    A["Alpha"]
  end
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .node .cluster{fill:rgba(102, 51, 153, 0.5);stroke:rgba(248.6666666666, 255, 235.9999999999, 0.2);stroke-width:1px;}"#
        ),
        "expected block composite cluster CSS to follow Mermaid 11.15 fade() colors"
    );
}

#[test]
fn block_svg_rejects_unsupported_cluster_theme_color() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "clusterBkg": "not-a-css-color"
        }
    })));
    let error = try_render_block_svg_from_text_with_engine(
        &engine,
        r#"block
  block
    A["Alpha"]
  end
"#,
    )
    .expect_err("unsupported khroma color must fail the render operation");

    assert!(error.to_string().contains("not-a-css-color"));
}

#[test]
fn block_svg_normalizes_khroma_colors_and_preserves_css_tokens() {
    let svg = render_block_svg_from_text(
        r#"block
  A["HSL"]
  B["Alpha"]
  C["Variable"]
  style A color:hsl(80 100% 96%)
  style B color:#8090a080
  style C color:var(--MyColor)
"#,
    );

    assert!(svg.contains("color: rgb(248, 255, 235); display: table-cell;"));
    assert!(svg.contains("color: rgba(128, 144, 160, 0.502); display: table-cell;"));
    assert!(svg.contains("color: var(--MyColor); display: table-cell;"));
}

#[test]
fn block_svg_applies_class_definitions_to_assigned_nodes() {
    let svg = render_block_svg_from_text(
        r#"block
  Frontend Backend Database[("Database")]

  classDef front fill:#696,stroke:#333;
  classDef back fill:#969,stroke:#333;
  class Frontend front
  class Backend,Database back
"#,
    );

    assert!(
        svg.contains(r#"class="node front flowchart-label""#),
        "expected the Frontend node to retain its assigned class: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .front&gt;*{fill:#696!important;stroke:#333!important;}"#),
        "expected the front class definition to style Block shapes: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .back&gt;*{fill:#969!important;stroke:#333!important;}"#),
        "expected the back class definition to style Block shapes: {svg}"
    );
}

#[test]
fn block_svg_xml_escapes_class_definition_values() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"]
  classDef branded font-family:"A&B",fill:#696
  class A branded
"#,
    );

    roxmltree::Document::parse(&svg).expect("classDef CSS must remain well-formed XML");
    assert!(
        svg.contains(r#"font-family:&quot;A&amp;B&quot;!important;"#),
        "expected classDef values to be XML escaped inside the style element: {svg}"
    );
}

#[test]
fn block_svg_drops_unsafe_inline_css_without_losing_safe_siblings() {
    let svg = render_block_svg_from_text(
        r#"block
  A["Alpha"]
  style A fill:url(https://example.test/paint.svg),stroke:#123456
"#,
    );

    assert!(
        !svg.contains("example.test") && !svg.contains("url(https://"),
        "unsafe external CSS resources must not reach the SVG: {svg}"
    );
    let shells = terminal_shell_styles(&svg, "merman-A");
    assert_eq!(shells.len(), 1, "expected one canonical Block shell");
    assert_eq!(terminal_stroke(&shells[0].1), Some("#123456"));
    assert_eq!(terminal_fill(&shells[0].1), None);
}

#[test]
fn block_circle_edge_starts_on_the_rendered_circle_boundary() {
    let svg = render_block_svg_from_text(
        r##"block-beta
  columns 3
  user(("User")):3
  space:3
  ui["Web UI"] api["API Server"] db[("Database")]

  user --> ui
  ui --> api
  api --> db

  style user fill:#ffe0b2,stroke:#fb8c00
  style db fill:#bbdefb,stroke:#1e88e5
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let user = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-user"))
        .expect("rendered user node");
    let circle = user
        .descendants()
        .find(|node| node.has_tag_name("circle"))
        .expect("rendered user circle");
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "flowchart-link")
                })
                && node
                    .attribute("id")
                    .is_some_and(|id| id.contains("user-ui"))
        })
        .expect("user to ui edge");

    let (center_x, center_y) = translated_center(user);
    let (edge_x, edge_y) = path_start(edge);
    let radius: f64 = circle
        .attribute("r")
        .expect("circle radius")
        .parse()
        .expect("numeric circle radius");
    let endpoint_radius = ((edge_x - center_x).powi(2) + (edge_y - center_y).powi(2)).sqrt();

    assert!(
        (endpoint_radius - radius).abs() <= 1e-3,
        "edge must start on the rendered circle: center=({center_x},{center_y}), endpoint=({edge_x},{edge_y}), endpoint_radius={endpoint_radius}, circle_radius={radius}, svg={svg}"
    );
}

#[test]
fn block_root_viewbox_uses_rendered_shape_bounds() {
    let svg = render_block_svg_from_text(
        r#"block
  id1((("This is the text in the circle")))
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let node = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-id1"))
        .expect("rendered double-circle node");
    let outer_circle = node
        .descendants()
        .find(|node| node.attribute("class") == Some("outer-circle"))
        .expect("rendered outer circle");
    let (center_x, center_y) = translated_center(node);
    let radius = outer_circle
        .attribute("r")
        .expect("outer circle radius")
        .parse::<f64>()
        .expect("numeric outer circle radius");
    let (view_x, view_y, view_width, view_height) = root_view_box(&document);
    let padding = 5.0;

    assert!(view_x <= center_x - radius - padding + 1e-9);
    assert!(view_y <= center_y - radius - padding + 1e-9);
    assert!(view_x + view_width >= center_x + radius + padding - 1e-9);
    assert!(view_y + view_height >= center_y + radius + padding - 1e-9);
}

#[test]
fn block_root_viewbox_ignores_non_schema_diagram_padding() {
    let default_svg = render_block_svg_from_text(
        r#"block
  A["A"] --> B["B"]
"#,
    );
    let opaque_config_svg = render_block_svg_from_text(
        r#"%%{init: {"block": {"diagramPadding": 250}}}%%
block
  A["A"] --> B["B"]
"#,
    );
    let default_document = roxmltree::Document::parse(&default_svg).expect("default Block SVG");
    let opaque_config_document =
        roxmltree::Document::parse(&opaque_config_svg).expect("configured Block SVG");

    assert_eq!(
        root_view_box(&default_document),
        root_view_box(&opaque_config_document),
        "Block viewBox padding is the fixed Mermaid 5px expansion, not an opaque config surface"
    );
}

#[test]
fn block_root_viewbox_contains_edge_label_foreign_object() {
    let svg = render_block_svg_from_text(
        r#"block
  A["A"] -- "A very long edge label" --> B["B"]
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let edge_label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class") == Some("edgeLabel")
                && node
                    .descendants()
                    .any(|descendant| descendant.has_tag_name("foreignObject"))
        })
        .expect("rendered Block edge label group");
    let foreign_object = edge_label
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .expect("rendered Block edge label foreignObject");
    let (label_min_x, label_min_y, label_max_x, label_max_y) =
        global_foreign_object_bounds(foreign_object);
    let (view_x, view_y, view_width, view_height) = root_view_box(&document);
    let view_max_x = view_x + view_width;
    let view_max_y = view_y + view_height;

    assert!(
        label_min_x >= view_x - 1e-6
            && label_min_y >= view_y - 1e-6
            && label_max_x <= view_max_x + 1e-6
            && label_max_y <= view_max_y + 1e-6,
        "edge label foreignObject must be contained by the root viewBox: label=({label_min_x},{label_min_y})..({label_max_x},{label_max_y}), viewBox=({view_x},{view_y}) {view_width}x{view_height}, svg={svg}"
    );
}

#[test]
fn block_html_labels_false_emits_svg_text_for_nodes_and_edges() {
    let svg = render_block_svg_from_text(
        r#"%%{init: {"htmlLabels": false}}%%
block
  A["Alpha"] -- "Edge label" --> B["Beta"]
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");

    assert!(
        document
            .descendants()
            .all(|node| !node.has_tag_name("foreignObject")),
        "htmlLabels:false must not emit foreignObject labels: {svg}"
    );

    let text_content = document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .flat_map(|node| {
            node.descendants()
                .filter_map(|descendant| descendant.text())
        })
        .collect::<String>();
    assert!(
        text_content.contains("Alpha")
            && text_content.contains("Beta")
            && text_content.contains("Edge label"),
        "htmlLabels:false must preserve node and edge label text in SVG text terminals: {svg}"
    );

    for label in document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("label"))
    {
        let (x, _) = translated_center(label);
        assert_eq!(x, 0.0, "SVG text is already horizontally centered: {svg}");
    }
    let background = document
        .descendants()
        .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
        .expect("edge label background");
    for attribute in ["width", "height"] {
        assert!(
            background
                .attribute(attribute)
                .unwrap()
                .parse::<f64>()
                .unwrap()
                > 0.0
        );
    }
}

#[test]
fn block_svg_labels_preserve_inline_text_color() {
    let svg = render_block_svg_from_text(
        r#"%%{init: {"htmlLabels": false}}%%
block
  A["Alpha"] B["Beta"] C["Variable"]
  style A fill:#eee,color:#123456,color:#abcdef
  style B color:hsl(80 100% 96%)
  style C color:var(--MyColor)
"#,
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (id, expected) in [
        ("merman-A", "fill:#abcdef;"),
        ("merman-B", "fill:hsl(80 100% 96%);"),
        ("merman-C", "fill:var(--MyColor);"),
    ] {
        let node = document
            .descendants()
            .find(|node| node.attribute("id") == Some(id))
            .unwrap();
        let text = node
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(text.attribute("style"), Some(expected), "{id}: {svg}");
    }
}

#[test]
fn block_svg_edge_background_uses_operation_text_bounds() {
    use merman_render::environment::{
        HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
        MeasurementProfileId, TextMeasurementOperation, TextMeasurementPhase,
        TextMeasurementPolicy, TextMeasurementProfileIdentity,
    };
    use merman_render::text::TextMetrics;
    use std::sync::Arc;

    struct LabelHost(f64);
    impl HostTextMeasurer for LabelHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            Ok(match request.operation {
                TextMeasurementOperation::Wrapped => {
                    Some(HostTextMeasurement::Metrics(TextMetrics {
                        width: 80.0,
                        height: 40.0,
                        line_count: 2,
                    }))
                }
                TextMeasurementOperation::CreateTextBBoxYOffset => {
                    assert_eq!(request.text, "First\nSecond");
                    assert_eq!(request.style.font_size, 16.0);
                    Some(HostTextMeasurement::Length(self.0))
                }
                _ => None,
            })
        }
    }
    for bbox_y in [-7.5, 9.0] {
        let parsed = Engine::new().parse_diagram_for_render_model_sync(
            "%%{init: {\"htmlLabels\": false}}%%\nblock\n A[\"Alpha<br/>line\"] -- \"First<br/>Second\" --> B[\"Beta\"]\n",
            ParseOptions::default(),
        ).unwrap().unwrap();
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.block-svg-label").unwrap(),
            "1",
        )
        .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::host_display(
                identity,
                Arc::new(LabelHost(bbox_y)),
                TextMeasurementPhase::ALL,
            ))
            .begin_session()
            .unwrap();
        let rendered = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let rect = document
            .descendants()
            .find(|node| node.attribute("class") == Some("background"))
            .unwrap();
        let number = |name| rect.attribute(name).unwrap().parse::<f64>().unwrap();
        assert_eq!((number("width"), number("height")), (84.0, 44.0));
        assert_eq!((number("x"), number("y")), (-42.0, bbox_y - 2.0));
        let label = rect.parent().unwrap();
        let (tx, ty) = translated_center(label);
        assert_eq!((tx, ty), (0.0, -bbox_y - 20.0));
        assert_eq!(tx + number("x") + number("width") / 2.0, 0.0);
        assert_eq!(ty + number("y") + number("height") / 2.0, 0.0);
        let (edge_x, edge_y) = translated_center(label.parent().unwrap());
        let (view_x, view_y, view_w, view_h) = root_view_box(&document);
        assert!(edge_x - 42.0 >= view_x && edge_x + 42.0 <= view_x + view_w);
        assert!(edge_y - 22.0 >= view_y && edge_y + 22.0 <= view_y + view_h);
        let rows = label
            .descendants()
            .filter(|node| node.attribute("class") == Some("row text-outer-tspan"))
            .count();
        assert_eq!(rows, 2);
    }
}

#[test]
fn block_root_viewbox_contains_rough_stroke_bounds() {
    let svg = render_block_svg_from_text(
        r#"block
  A(["A"])
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let stadium = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-A"))
        .expect("rendered stadium node");
    let outer_path = stadium
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|class| class == "basic label-container outer-path")
        })
        .expect("rendered stadium outer-path group");
    let stroke_path = outer_path
        .children()
        .find(|node| node.has_tag_name("path") && node.attribute("fill") == Some("none"))
        .expect("RoughJS stadium stroke path");
    let (min_x, min_y, max_x, max_y) = global_path_geometry_bounds(stroke_path);
    let stroke_width = stroke_path
        .attribute("stroke-width")
        .expect("RoughJS stroke width")
        .parse::<f64>()
        .expect("numeric RoughJS stroke width");
    let (view_x, view_y, view_width, view_height) = root_view_box(&document);
    let view_max_x = view_x + view_width;
    let view_max_y = view_y + view_height;
    let outset = stroke_width / 2.0;

    assert!(
        min_x - outset >= view_x - 1e-6
            && min_y - outset >= view_y - 1e-6
            && max_x + outset <= view_max_x + 1e-6
            && max_y + outset <= view_max_y + 1e-6,
        "root viewBox must contain the emitted RoughJS stroke envelope: path=({min_x},{min_y})..({max_x},{max_y}), stroke={stroke_width}, viewBox=({view_x},{view_y}) {view_width}x{view_height}, svg={svg}"
    );
}

#[test]
fn block_stadium_renders_rough_outer_path_and_clips_edge() {
    let svg = render_block_svg_from_text(
        r#"block
  A(["A"]) --> B["B"]
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Block SVG");
    let stadium = document
        .descendants()
        .find(|node| node.attribute("id") == Some("merman-A"))
        .expect("rendered stadium node");
    let outer_path = stadium
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("class")
                    .is_some_and(|class| class == "basic label-container outer-path")
        })
        .expect("rendered stadium outer-path group");
    let paths = outer_path
        .children()
        .filter(|node| node.has_tag_name("path"))
        .collect::<Vec<_>>();
    assert_eq!(
        paths.len(),
        2,
        "RoughJS stadiums emit fill and stroke paths"
    );
    assert_eq!(paths[0].attribute("stroke"), Some("none"));
    assert_eq!(paths[1].attribute("fill"), Some("none"));
    let stroke_width: f64 = paths[1]
        .attribute("stroke-width")
        .expect("RoughJS stroke width")
        .parse()
        .expect("numeric RoughJS stroke width");
    assert!((stroke_width - 1.3).abs() <= 1e-6);
    assert!(!outer_path.children().any(|node| node.has_tag_name("rect")));
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path") && node.attribute("id").is_some_and(|id| id.contains("A-B"))
        })
        .expect("stadium edge");

    let (center_x, center_y) = translated_center(stadium);
    let (edge_x, edge_y) = path_start(edge);
    assert!(
        edge_x > center_x,
        "edge must leave the stadium on its right side"
    );
    assert!((edge_y - center_y).abs() <= 1e-3);
}

#[test]
fn block_absent_text_rules_have_no_terminal_even_with_frontmatter_and_composite_labels() {
    let source = "---\ntitle: Metadata title\n---\nblock\n  columns 1\n  block:group[\"Composite title\"]\n    A[\"Alpha\"]\n  end\n";
    for target in [ThemeTarget::Title, ThemeTarget::ClusterLabel] {
        for html_labels in [false, true] {
            for variant in [
                None,
                Some(ThemeVariant::Default),
                Some(ThemeVariant::Active),
            ] {
                for transparent in [false, true] {
                    let paint = if transparent {
                        CanvasPaint::Transparent
                    } else {
                        CanvasPaint::solid("#b316cd").unwrap()
                    };
                    let mut rule =
                        ThemeRule::new(target, ThemeStylePatch::default().with_fill(paint))
                            .for_family(DiagramFamilyId::BLOCK);
                    if let Some(variant) = variant {
                        rule = rule.with_variant(variant);
                    }
                    let theme = DiagramThemeCompiler::new()
                        .compile(
                            DiagramThemeSpec::new()
                                .with_styles(ThemeRuleSet::default().with_rule(rule)),
                        )
                        .unwrap();
                    let rendered = render_block_with_theme_and_engine(
                        source,
                        &theme,
                        Engine::new().with_site_config(MermaidConfig::from_value(
                            serde_json::json!({
                                "htmlLabels": html_labels
                            }),
                        )),
                    );
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    assert!(document.descendants().all(|node| {
                        !node
                            .attribute("class")
                            .unwrap_or("")
                            .split_ascii_whitespace()
                            .any(|class| matches!(class, "flowchartTitleText" | "cluster-label"))
                    }));
                    for cluster in document.descendants().filter(|node| {
                        node.attribute("class")
                            .unwrap_or("")
                            .split_ascii_whitespace()
                            .any(|class| class == "cluster")
                    }) {
                        assert!(cluster.descendants().all(|node| {
                            !matches!(node.tag_name().name(), "text" | "span" | "p")
                        }));
                    }
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(evidence.required_count(), 1);
                    assert_eq!(evidence.applied_count(), 0);
                    assert_eq!(evidence.not_applicable_count(), 1);
                    assert_eq!(evidence.theme_residual_count(), 0);
                    assert_eq!(evidence.compatibility_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn absent_block_text_siblings_do_not_suppress_typed_node_paint() {
    use merman_render::diagram_theme::{OrdinalSelector, Specified};
    for target in [ThemeTarget::Title, ThemeTarget::ClusterLabel] {
        for clear in [false, true] {
            let mut patch = ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#b316cd").unwrap())
                .with_stroke(CanvasPaint::solid("#2468ac").unwrap());
            if clear {
                patch.paint.fill = Specified::Clear;
            }
            let absent_text = ThemeRule::new(target, patch)
                .for_family(DiagramFamilyId::BLOCK)
                .with_ordinal(OrdinalSelector::exact(1).unwrap());
            let node = ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#654321").unwrap()),
            )
            .for_family(DiagramFamilyId::BLOCK);
            let theme = DiagramThemeCompiler::new()
                .compile(
                    DiagramThemeSpec::new().with_styles(
                        ThemeRuleSet::default()
                            .with_rule(absent_text)
                            .with_rule(node),
                    ),
                )
                .unwrap();
            let rendered =
                render_block_with_theme_and_engine("block\n A[\"Alpha\"]\n", &theme, Engine::new());
            assert!(
                terminal_shell_styles(rendered.svg(), "block-theme-A")
                    .iter()
                    .any(|(_, style)| terminal_fill(style) == Some("#654321"))
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 2);
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
            assert_eq!(evidence.compatibility_residual_count(), 0);
        }
    }
}

#[test]
fn block_edge_label_background_typed_route_has_a_real_css_consumer() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::EdgeLabelBackground,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#b316cd").expect("valid paint")),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .expect("compile Block edge-label theme");
    let rendered = render_block_with_theme_requirement(
        "block\n  A[\"Alpha\"] -- \"relates\" --> B[\"Beta\"]\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::RequirePortable,
    );
    assert!(
        rendered
            .svg()
            .contains("#block-theme .edgeLabel{background-color:#b316cd;")
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.required_count(), 1);
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.compatibility_residual_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn block_background_reconciles_missing_labels_and_explicit_config() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::EdgeLabelBackground,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#b316cd").unwrap()),
                    )
                    .for_family(DiagramFamilyId::BLOCK),
                ),
            ),
        )
        .unwrap();
    for html_labels in [false, true] {
        for config_owned in [false, true] {
            for has_label in [false, true] {
                let mut config = serde_json::json!({"htmlLabels": html_labels});
                if config_owned {
                    config["themeVariables"] =
                        serde_json::json!({"edgeLabelBackground": "#123456"});
                }
                let source = if has_label {
                    "block\n A -- \"relates\" --> B\n"
                } else {
                    "block\n A --> B\n"
                };
                let rendered = render_block_with_theme_and_engine(
                    source,
                    &theme,
                    Engine::new().with_site_config(MermaidConfig::from_value(config)),
                );
                let expected = if config_owned { "#123456" } else { "#b316cd" };
                assert!(rendered.svg().contains(&format!(
                    "#block-theme .edgeLabel{{background-color:{expected};"
                )));
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                let applied = usize::from(has_label && !config_owned);
                assert_eq!(evidence.required_count(), 1);
                assert_eq!(evidence.applied_count(), applied);
                assert_eq!(evidence.not_applicable_count(), 1 - applied);
                assert_eq!(evidence.compatibility_residual_count(), 0);
                assert_eq!(evidence.theme_residual_count(), 0);
            }
        }
    }
}

#[test]
fn block_background_unsupported_winning_facet_stays_residual() {
    for ordinal in [false, true] {
        let patch = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap());
        let patch = if ordinal {
            patch
        } else {
            patch.with_stroke(CanvasPaint::solid("#2468ac").unwrap())
        };
        let rule = ThemeRule::new(ThemeTarget::EdgeLabelBackground, patch)
            .for_family(DiagramFamilyId::BLOCK);
        let rule = if ordinal {
            rule.with_ordinal(OrdinalSelector::exact(2).unwrap())
        } else {
            rule
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
            .unwrap();
        let source = "block\n A -- \"first\" --> B\n B -- \"second\" --> C\n";
        let rendered = render_block_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
        let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable)
            .begin_session_with_theme(&theme)
            .unwrap();
        let result = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
        assert!(
            matches!(
                result,
                Err(merman_render::Error::UnverifiedFamilyTheme {
                    family_id: DiagramFamilyId::BLOCK,
                    residual_count: 1,
                })
            ),
            "unsupported winning background facet must produce a Block theme residual"
        );
    }
}

fn block_edge_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let rules = rules
        .into_iter()
        .fold(ThemeRuleSet::default(), |set, rule| {
            set.with_rule(rule.for_family(DiagramFamilyId::BLOCK))
        });
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(rules))
        .unwrap()
}

#[test]
fn block_edge_static_paint_is_written_only_to_visible_edge_paths() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for paint in [
            CanvasPaint::solid("#b316cd").unwrap(),
            CanvasPaint::Transparent,
        ] {
            for fallback in [false, true] {
                let css = if matches!(paint, CanvasPaint::Transparent) {
                    "transparent"
                } else {
                    "#b316cd"
                };
                let patch = if fallback {
                    ThemeStylePatch::default().with_fill(paint.clone())
                } else {
                    ThemeStylePatch::default().with_stroke(paint.clone())
                };
                let mut rule = ThemeRule::new(ThemeTarget::Edge, patch);
                if let Some(variant) = variant {
                    rule = rule.with_variant(variant);
                }
                let theme = block_edge_theme([rule]);
                let rendered = render_block_with_theme_and_engine(
                    "block\n A --> B\n B --> C\n",
                    &theme,
                    Engine::new(),
                );
                let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                let edges = document
                    .descendants()
                    .filter(|node| node.attribute("data-edge") == Some("true"))
                    .collect::<Vec<_>>();
                assert_eq!(edges.len(), 2);
                for edge in edges {
                    assert_eq!(edge.tag_name().name(), "path");
                    assert!(
                        edge.attribute("style")
                            .unwrap()
                            .contains(&format!("stroke:{css};fill:none;"))
                    );
                    assert!(!edge.attribute("d").unwrap().is_empty());
                }
                for marker in document
                    .descendants()
                    .filter(|node| node.has_tag_name("marker"))
                {
                    assert!(marker.descendants().all(|node| {
                        node.attribute("style")
                            .is_none_or(|style| !style.contains(css))
                    }));
                }
                assert!(
                    !document
                        .descendants()
                        .filter(|node| node.has_tag_name("style"))
                        .any(|node| node.text().unwrap_or("").contains(css))
                );
                let evidence =
                    merman_render::__private::family_evidence(rendered.into_completion().report());
                assert_eq!(evidence.applied_count(), 1);
                assert_eq!(evidence.compatibility_residual_count(), 0);
            }
        }
    }
}

#[test]
fn block_edge_source_config_and_absent_edges_are_not_applicable() {
    let theme = block_edge_theme([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
    )]);
    for (source, engine) in [
        (
            "block\n A --> B\n",
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"themeVariables":{"lineColor":"#2468ac"}}),
            )),
        ),
        ("block\n A\n", Engine::new()),
    ] {
        let rendered = render_block_with_theme_and_engine(source, &theme, engine);
        assert!(!rendered.svg().contains("#b316cd"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.applied_count(), 0);
    }
}

#[test]
fn block_edge_ordinal_and_clear_stroke_block_fill_fallback() {
    use merman_render::diagram_theme::Specified;
    for ordinal in [false, true] {
        let mut patch =
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap());
        let mut rule = if ordinal {
            ThemeRule::new(ThemeTarget::Edge, patch)
                .with_ordinal(OrdinalSelector::exact(1).unwrap())
        } else {
            patch.stroke.paint = Specified::Clear;
            ThemeRule::new(ThemeTarget::Edge, patch)
        };
        rule = rule.for_family(DiagramFamilyId::BLOCK);
        let theme = block_edge_theme([rule]);
        let rendered = render_block_with_theme_requirement(
            "block\n A --> B\n",
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        assert!(!rendered.svg().contains("stroke:#b316cd"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.theme_residual_count(), 1);
    }
}

#[test]
fn block_edge_shadowed_fill_sibling_does_not_hide_winning_stroke() {
    let theme = block_edge_theme([
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#abcdef").unwrap())
                .with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap()),
        ),
    ]);
    let rendered = render_block_with_theme_and_engine("block\n A --> B\n", &theme, Engine::new());
    assert!(rendered.svg().contains("stroke:#b316cd;fill:none;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn block_edge_class_ownership_follows_actual_parent_selector() {
    let theme = block_edge_theme([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
    )]);
    for (class, owned) in [("block", true), ("unrelated", false)] {
        let source = format!("block\n A --> B\n classDef {class} stroke:#2468ac\n");
        let rendered = render_block_with_theme_and_engine(&source, &theme, Engine::new());
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let path = document
            .descendants()
            .find(|node| node.attribute("data-edge") == Some("true"))
            .unwrap();
        assert_eq!(
            path.attribute("style")
                .unwrap()
                .contains("stroke:#b316cd;fill:none;"),
            !owned
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(!owned));
        assert_eq!(evidence.not_applicable_count(), usize::from(owned));
    }
}

#[test]
fn block_edge_mixed_rule_only_certifies_consumed_winning_facets() {
    let theme = block_edge_theme([
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default()
                .with_stroke(CanvasPaint::solid("#b316cd").unwrap())
                .with_stroke_width(4.0)
                .unwrap(),
        ),
        ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#2468ac").unwrap()),
        ),
    ]);
    let rendered = render_block_with_theme_requirement(
        "block\n A --> B\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    assert!(rendered.svg().contains("stroke:#2468ac;fill:none;"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn block_edge_invisible_self_loop_keeps_rendering_without_applied_paint() {
    for mixed in [false, true] {
        let theme = block_edge_theme([ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
        )]);
        let source = if mixed {
            "block\n A --> A\n A --> B\n"
        } else {
            "block\n A --> A\n"
        };
        let rendered = render_block_with_theme_and_engine(source, &theme, Engine::new());
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.attribute("data-edge") == Some("true"))
                .count(),
            if mixed { 2 } else { 1 }
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(mixed));
        assert_eq!(evidence.not_applicable_count(), usize::from(!mixed));
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn block_edge_unsupported_gradient_stroke_blocks_solid_fill_fallback() {
    use merman_render::diagram_theme::{GradientStop, LinearGradient};

    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
        ],
    )
    .unwrap();
    let theme = block_edge_theme([ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#b316cd").unwrap())
            .with_stroke(CanvasPaint::LinearGradient(gradient)),
    )]);
    let rendered = render_block_with_theme_requirement(
        "block\n A --> B\n",
        &theme,
        Engine::new(),
        ThemePortabilityRequirement::BestEffort,
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let edge = document
        .descendants()
        .find(|node| node.attribute("data-edge") == Some("true"))
        .unwrap();
    assert_eq!(edge.attribute("style"), Some("undefined;;;undefined"));
    assert!(!rendered.svg().contains("#b316cd"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn block_edge_invisible_self_loop_preserves_semantic_ordinals() {
    for ordinal in [1, 2] {
        let theme = block_edge_theme([ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
        )
        .with_ordinal(OrdinalSelector::exact(ordinal).unwrap())]);
        let rendered = render_block_with_theme_requirement(
            "block\n A --> A\n A --> B\n",
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let edges = document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .collect::<Vec<_>>();
        assert_eq!(edges.len(), 2);
        assert!(
            edges
                .iter()
                .all(|edge| { edge.attribute("style") == Some("undefined;;;undefined") })
        );
        assert!(!rendered.svg().contains("#b316cd"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), usize::from(ordinal == 1));
        assert_eq!(evidence.theme_residual_count(), usize::from(ordinal == 2));
    }
}

const BLOCK_CLUSTER_SOURCE: &str =
    "block\n columns 1\n block:group[\"Group\"]\n A[\"Alpha\"]\n end\n B[\"Beta\"]\n";

fn block_cluster_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    block_edge_theme(rules)
}

#[test]
fn block_cluster_static_paint_reaches_only_composite_shells() {
    for variant in [None, Some(ThemeVariant::Default)] {
        for transparent in [false, true] {
            let paint = if transparent {
                CanvasPaint::Transparent
            } else {
                CanvasPaint::solid("#b316cd").unwrap()
            };
            let mut rule = ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default()
                    .with_fill(paint.clone())
                    .with_stroke(paint),
            );
            if let Some(variant) = variant {
                rule = rule.with_variant(variant);
            }
            let theme = block_cluster_theme([rule]);
            let rendered =
                render_block_with_theme_and_engine(BLOCK_CLUSTER_SOURCE, &theme, Engine::new());
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            let css = if transparent {
                "transparent"
            } else {
                "#b316cd"
            };
            let cluster = document
                .descendants()
                .find(|node| {
                    node.attribute("class")
                        .unwrap_or("")
                        .split_whitespace()
                        .any(|class| class == "composite")
                })
                .unwrap();
            assert_eq!(cluster.tag_name().name(), "rect");
            assert!(
                cluster
                    .attribute("style")
                    .unwrap()
                    .contains(&format!("fill:{css};stroke:{css};"))
            );
            let ordinary = document.descendants().filter(|node| {
                node.has_tag_name("rect")
                    && node
                        .attribute("class")
                        .unwrap_or("")
                        .contains("label-container")
                    && *node != cluster
            });
            assert!(
                ordinary
                    .into_iter()
                    .all(|node| !node.attribute("style").unwrap_or("").contains(css))
            );
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 1);
            assert_eq!(evidence.compatibility_residual_count(), 0);
        }
    }
}

#[test]
fn block_cluster_node_priority_is_resolved_per_paint_property() {
    for split in [false, true] {
        let node_patch = if split {
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap())
        } else {
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#2468ac").unwrap())
                .with_stroke(CanvasPaint::solid("#2468ac").unwrap())
        };
        let theme = block_cluster_theme([
            ThemeRule::new(ThemeTarget::Node, node_patch),
            ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#b316cd").unwrap())
                    .with_stroke(CanvasPaint::solid("#b316cd").unwrap()),
            ),
        ]);
        let rendered =
            render_block_with_theme_and_engine(BLOCK_CLUSTER_SOURCE, &theme, Engine::new());
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let cluster = document
            .descendants()
            .find(|node| node.attribute("class").unwrap_or("").contains("composite"))
            .unwrap();
        let style = cluster.attribute("style").unwrap();
        assert!(style.contains("fill:#2468ac;"));
        assert!(style.contains(if split {
            "stroke:#b316cd;"
        } else {
            "stroke:#2468ac;"
        }));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), if split { 2 } else { 1 });
        assert_eq!(evidence.not_applicable_count(), usize::from(!split));
    }
}

#[test]
fn block_cluster_source_and_explicit_config_own_their_properties() {
    for source_style in [false, true] {
        let theme = block_cluster_theme([ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::solid("#b316cd").unwrap())
                .with_stroke(CanvasPaint::solid("#2468ac").unwrap()),
        )]);
        let source = if source_style {
            format!("{BLOCK_CLUSTER_SOURCE}style group fill:#abcdef,stroke:#123456\n")
        } else {
            BLOCK_CLUSTER_SOURCE.to_string()
        };
        let engine = if source_style {
            Engine::new()
        } else {
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({"themeVariables":{"clusterBkg":"#abcdef","clusterBorder":"#123456"}})))
        };
        let rendered = render_block_with_theme_and_engine(&source, &theme, engine);
        assert!(!rendered.svg().contains("fill:#b316cd;"));
        assert!(!rendered.svg().contains("stroke:#2468ac;"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[test]
fn block_cluster_empty_domain_is_not_applicable_and_unsupported_winners_stay_residual() {
    for case in ["empty", "mixed", "ordinal"] {
        let mut patch =
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap());
        if case == "mixed" {
            patch = patch.with_stroke_width(4.0).unwrap();
        }
        let mut rule = ThemeRule::new(ThemeTarget::Cluster, patch);
        if case == "ordinal" {
            rule = rule.with_ordinal(OrdinalSelector::exact(1).unwrap());
        }
        let theme = block_cluster_theme([rule]);
        let rendered = render_block_with_theme_requirement(
            if case == "empty" {
                "block\n A\n"
            } else {
                BLOCK_CLUSTER_SOURCE
            },
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(
            evidence.not_applicable_count(),
            usize::from(case == "empty")
        );
        assert_eq!(
            evidence.theme_residual_count(),
            usize::from(case != "empty")
        );
    }
}

#[test]
fn block_cluster_class_source_and_config_masks_are_property_local() {
    for case in ["class", "cluster-config", "node-config"] {
        let theme = block_cluster_theme([
            ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#2468ac").unwrap()),
            ),
            ThemeRule::new(
                ThemeTarget::Cluster,
                ThemeStylePatch::default()
                    .with_fill(CanvasPaint::solid("#b316cd").unwrap())
                    .with_stroke(CanvasPaint::solid("#123456").unwrap()),
            ),
        ]);
        let source = if case == "class" {
            format!("{BLOCK_CLUSTER_SOURCE}classDef brand fill:#abcdef\nclass group brand\n")
        } else {
            BLOCK_CLUSTER_SOURCE.to_string()
        };
        let engine = if case == "cluster-config" {
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"themeVariables":{"clusterBkg":"#abcdef"}}),
            ))
        } else if case == "node-config" {
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"themeVariables":{"mainBkg":"#abcdef", "nodeBorder":"#abcdef"}}),
            ))
        } else {
            Engine::new()
        };
        let rendered = render_block_with_theme_and_engine(&source, &theme, engine);
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let cluster = document
            .descendants()
            .find(|node| node.attribute("class").unwrap_or("").contains("composite"))
            .unwrap();
        let style = cluster.attribute("style").unwrap();
        assert_eq!(style.contains("fill:#b316cd;"), case == "node-config");
        assert!(style.contains(if case == "node-config" {
            "stroke:#123456;"
        } else {
            "stroke:#2468ac;"
        }));
        let ordinary = document.descendants().filter(|node| {
            node.has_tag_name("rect")
                && node
                    .attribute("class")
                    .unwrap_or("")
                    .contains("label-container")
                && *node != cluster
        });
        assert!(ordinary.into_iter().all(|node| {
            node.attribute("style")
                .unwrap_or("")
                .contains("stroke:#2468ac;")
                == (case != "node-config")
        }));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[test]
fn block_cluster_fallback_preserves_unsupported_node_gradient_residual() {
    use merman_render::diagram_theme::{GradientStop, LinearGradient};
    let gradient = LinearGradient::new(
        90.0,
        [
            GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
            GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
        ],
    )
    .unwrap();
    let theme = block_cluster_theme([
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default().with_fill(CanvasPaint::LinearGradient(gradient)),
        ),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
        ),
    ]);
    for source in [
        "block\n block:group[\"Group\"]\n end\n",
        BLOCK_CLUSTER_SOURCE,
    ] {
        let rendered = render_block_with_theme_requirement(
            source,
            &theme,
            Engine::new(),
            ThemePortabilityRequirement::BestEffort,
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        let cluster = document
            .descendants()
            .find(|node| node.attribute("class").unwrap_or("").contains("composite"))
            .unwrap();
        assert!(
            cluster
                .attribute("style")
                .unwrap()
                .contains("fill:#b316cd;")
        );
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 1);
    }
}

#[test]
fn block_cluster_structural_node_class_owns_actual_shell_paint() {
    let theme = block_cluster_theme([ThemeRule::new(
        ThemeTarget::Cluster,
        ThemeStylePatch::default()
            .with_fill(CanvasPaint::solid("#b316cd").unwrap())
            .with_stroke(CanvasPaint::solid("#2468ac").unwrap()),
    )]);
    let source = format!("{BLOCK_CLUSTER_SOURCE}classDef node fill:#abcdef,stroke:#123456\n");
    let rendered = render_block_with_theme_and_engine(&source, &theme, Engine::new());
    assert!(
        rendered
            .svg()
            .contains(".node&gt;*{fill:#abcdef!important;stroke:#123456!important;}")
    );
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let cluster = document
        .descendants()
        .find(|node| node.attribute("class").unwrap_or("").contains("composite"))
        .unwrap();
    assert!(!cluster.attribute("style").unwrap().contains("#b316cd"));
    assert!(!cluster.attribute("style").unwrap().contains("#2468ac"));
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 1);
}

#[test]
fn block_cluster_and_node_ignore_default_class_when_plain_is_assigned() {
    let theme = block_cluster_theme([
        ThemeRule::new(
            ThemeTarget::Node,
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#2468ac").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::Cluster,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#b316cd").unwrap()),
        ),
    ]);
    let source = format!(
        "{BLOCK_CLUSTER_SOURCE}classDef default fill:#abcdef,stroke:#123456\nclassDef plain color:#000000\nclass group,B plain\n"
    );
    let rendered = render_block_with_theme_and_engine(&source, &theme, Engine::new());
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let cluster = document
        .descendants()
        .find(|node| node.attribute("class").unwrap_or("").contains("composite"))
        .unwrap();
    assert!(
        cluster
            .attribute("style")
            .unwrap()
            .contains("fill:#b316cd;stroke:#2468ac;")
    );
    assert!(terminal_node_has_class(
        rendered.svg(),
        "block-theme-group",
        "plain"
    ));
    assert!(!terminal_node_has_class(
        rendered.svg(),
        "block-theme-group",
        "default"
    ));
    let ordinary = terminal_shell_styles(rendered.svg(), "block-theme-B");
    assert_eq!(terminal_stroke(&ordinary[0].1), Some("#2468ac"));
    let default_owned = terminal_shell_styles(rendered.svg(), "block-theme-A");
    assert!(
        default_owned
            .iter()
            .all(|(_, style)| terminal_stroke(style) != Some("#2468ac"))
    );
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.not_applicable_count(), 0);
}
