//! Node effects must reach shape terminals without changing label or source ownership.

use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    DiagramEffectSet, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding,
    EffectColorSpace, EffectGraph, EffectInput, EffectPrimitive, OrdinalSelector, Specified,
    ThemeColorValue, ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    ThemeTarget,
};
use merman_render::environment::RenderEnvironment;
use merman_render::family::{self, RenderedFamilySvg};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

fn theme(rules: ThemeRuleSet) -> DiagramTheme {
    let graph = EffectGraph::new(
        "node-glow",
        [
            EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: -12.0,
                offset_y: 0.0,
                blur_radius: 2.0,
                spread: 0.0,
                color: ThemeColorValue::parse("#f02020").unwrap(),
            },
            EffectPrimitive::DropShadow {
                input: EffectInput::Previous,
                offset_x: 12.0,
                offset_y: 0.0,
                blur_radius: 2.0,
                spread: 0.0,
                color: ThemeColorValue::parse("#2040f0").unwrap(),
            },
        ],
    )
    .unwrap()
    .with_color_space(EffectColorSpace::Srgb);
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(rules).with_effects(
                DiagramEffectSet::default()
                    .with_graph(graph)
                    .unwrap()
                    .with_binding(EffectBinding::new(ThemeTarget::Node, "node-glow").unwrap())
                    .unwrap(),
            ),
        )
        .unwrap()
}

fn render(
    source: &str,
    theme: &DiagramTheme,
    look: &str,
    strict: bool,
) -> merman_render::Result<RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(
        theme,
        Engine::new().with_site_config(MermaidConfig::from_value(
            json!({"htmlLabels":false, "look":look}),
        )),
    )
    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
    .unwrap()
    .unwrap();
    let mut environment = RenderEnvironment::deterministic();
    if strict {
        environment = environment
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable);
    }
    let session = environment.begin_session_with_theme(theme).unwrap();
    family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
}

fn applications(svg: &str) -> usize {
    roxmltree::Document::parse(svg)
        .unwrap()
        .descendants()
        .filter(|node| {
            node.attribute("filter")
                .is_some_and(|value| value.contains("-theme-effect-"))
        })
        .count()
}

#[test]
fn classic_shapes_receive_ordered_shadows_without_filtering_their_labels() {
    let theme = theme(ThemeRuleSet::default());
    let rendered = render(
        "flowchart LR\nA[Rect] --> B(Round) --> C{Diamond} --> D((Circle)) --> E(((Double)))",
        &theme,
        "classic",
        true,
    )
    .unwrap();
    let svg = rendered.svg();
    assert_eq!(applications(svg), 5);
    let xml = roxmltree::Document::parse(svg).unwrap();
    for group in xml.descendants().filter(|node| {
        node.attribute("filter")
            .is_some_and(|value| value.contains("-theme-effect-"))
    }) {
        assert!(["g", "rect", "polygon", "circle"].contains(&group.tag_name().name()));
        assert!(
            group
                .parent_element()
                .unwrap()
                .attribute("class")
                .unwrap_or_default()
                .split_whitespace()
                .any(|class| class == "node")
        );
        assert!(
            !group
                .descendants()
                .any(|child| child.has_tag_name("text") || child.has_tag_name("foreignObject")),
            "node shape effects must not silently style labels"
        );
    }
    assert_eq!(
        xml.descendants()
            .filter(|node| node.has_tag_name("feGaussianBlur"))
            .count(),
        10
    );
}

#[test]
fn mixed_shapes_and_nonclassic_looks_keep_unconsumed_effects_residual() {
    let theme = theme(ThemeRuleSet::default());
    for (source, look, expected) in [
        ("flowchart LR\nA[Rect] --> B[/Lean/]", "classic", 1),
        ("flowchart LR\nA[Rect] --> B[Rect]", "handDrawn", 0),
        ("flowchart LR\nA[Rect] --> B[Rect]", "neo", 0),
    ] {
        let rendered = render(source, &theme, look, false).unwrap();
        assert_eq!(applications(rendered.svg()), expected);
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.theme_residual_count(), 1);
        assert!(render(source, &theme, look, true).is_err());
    }
}

#[test]
fn ordinal_clear_removes_only_its_effect_and_suppresses_unsupported_geometry() {
    let mut patch = ThemeStylePatch::default();
    patch.effects.effect = Specified::Clear;
    let theme = theme(ThemeRuleSet::default().with_rule(
        ThemeRule::new(ThemeTarget::Node, patch).with_ordinal(OrdinalSelector::Exact(2)),
    ));
    let rendered = render(
        "flowchart LR\nA[Rect] --> B[/Lean/]",
        &theme,
        "classic",
        true,
    )
    .unwrap();
    assert_eq!(applications(rendered.svg()), 1);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn source_fill_keeps_glow_and_source_filter_suppresses_only_its_node() {
    let theme = theme(ThemeRuleSet::default());
    let fill = render(
        "flowchart LR\nA --> B\nstyle A fill:#00ff00",
        &theme,
        "classic",
        true,
    )
    .unwrap();
    assert_eq!(applications(fill.svg()), 2);
    assert!(fill.svg().contains("fill:#00ff00"));
    for declaration in [
        "style A filter:none",
        "classDef clean filter:none\nclass A clean",
    ] {
        let rendered = render(
            &format!("flowchart LR\nA --> B\n{declaration}"),
            &theme,
            "classic",
            false,
        )
        .unwrap();
        assert_eq!(applications(rendered.svg()), 1, "{declaration}");
    }
}

#[test]
fn glow_expands_viewport_without_moving_layout_nodes_and_survives_nested_roots() {
    let source = "flowchart LR\nsubgraph Group\nA --> B\nend\nB --> C";
    let plain = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let themed = theme(ThemeRuleSet::default());
    let base = render(source, &plain, "classic", true).unwrap();
    let glow = render(source, &themed, "classic", true).unwrap();
    assert_eq!(applications(glow.svg()), 3);
    let base_xml = roxmltree::Document::parse(base.svg()).unwrap();
    let glow_xml = roxmltree::Document::parse(glow.svg()).unwrap();
    let viewport = |xml: &roxmltree::Document<'_>| {
        xml.root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|part| part.parse::<f64>().unwrap())
            .collect::<Vec<_>>()
    };
    let before = viewport(&base_xml);
    let after = viewport(&glow_xml);
    assert!(after[2] > before[2] + 20.0, "{before:?} -> {after:?}");
    assert!(after[3] >= before[3]);
    let positions = |xml: &roxmltree::Document<'_>| {
        xml.descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("data-id").is_some())
            .map(|node| {
                (
                    node.attribute("data-id").unwrap().to_owned(),
                    node.attribute("transform").unwrap_or_default().to_owned(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(positions(&base_xml), positions(&glow_xml));
}

#[test]
fn public_cyberpunk_recipe_exchange_retains_actual_node_glow() {
    use merman_render::diagram_theme::ThemePreset;
    let compiler = DiagramThemeCompiler::new();
    let selected = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let encoded =
        serde_json::to_vec(&compiler.export_preset(ThemePreset::Cyberpunk).unwrap()).unwrap();
    let imported = DiagramThemeCompiler::new()
        .compile_recipe(serde_json::from_slice(&encoded).unwrap())
        .unwrap();
    let source = "flowchart LR\nA[Alpha] -->|Advance| B[Beta]";
    let direct = render(source, &selected, "classic", true).unwrap();
    let exchanged = render(source, &imported, "classic", true).unwrap();
    assert_eq!(direct.svg(), exchanged.svg());
    assert_eq!(applications(direct.svg()), 2);
    let xml = roxmltree::Document::parse(direct.svg()).unwrap();
    let deviations = xml
        .descendants()
        .filter(|node| node.has_tag_name("feGaussianBlur"))
        .map(|node| node.attribute("stdDeviation").unwrap())
        .collect::<Vec<_>>();
    assert_eq!(deviations, ["8", "16", "8", "16"]);
}

#[test]
fn html_class_opacity_keeps_the_original_shape_selector_boundary() {
    let source = "---\nconfig:\n  htmlLabels: true\n---\nflowchart LR\nA[Alpha]:::dim --> B[Beta]\nclassDef dim opacity:0.5";
    let rendered = render(source, &theme(ThemeRuleSet::default()), "classic", false).unwrap();
    let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(
        xml.descendants()
            .any(|node| node.has_tag_name("foreignObject"))
    );
    let node = xml
        .descendants()
        .find(|node| node.attribute("data-id") == Some("A"))
        .unwrap();
    let shape = node
        .children()
        .find(|node| node.has_tag_name("rect"))
        .expect("classDef > * must still select the original rectangle directly");
    assert!(
        shape
            .attribute("filter")
            .unwrap()
            .contains("-theme-effect-")
    );
    assert!(shape.attribute("style").unwrap().contains("opacity:0.5"));
    assert!(
        !node
            .children()
            .any(|node| node.has_tag_name("g") && node.attribute("filter").is_some())
    );
}

#[test]
fn diamond_miter_stroke_is_inside_the_filter_region_even_without_blur() {
    let graph = EffectGraph::new(
        "hard",
        [EffectPrimitive::DropShadow {
            input: EffectInput::SourceGraphic,
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 0.0,
            spread: 0.0,
            color: ThemeColorValue::parse("#ffffff").unwrap(),
        }],
    )
    .unwrap();
    let themed = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_effects(
                DiagramEffectSet::default()
                    .with_graph(graph)
                    .unwrap()
                    .with_binding(EffectBinding::new(ThemeTarget::Node, "hard").unwrap())
                    .unwrap(),
            ),
        )
        .unwrap();
    // The polygon writer certifies the numeric source width, and the same width must bound
    // its miter tips when a typed filter is attached.
    let source = "flowchart LR\nA{Diamond}\nstyle A stroke-width:20px";
    let rendered = render(source, &themed, "classic", true).unwrap();
    let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
    let shape = xml
        .descendants()
        .find(|node| {
            node.has_tag_name("polygon") && node.attribute("class") == Some("label-container")
        })
        .unwrap();
    let coordinates = shape
        .attribute("points")
        .unwrap()
        .split_whitespace()
        .map(|point| {
            let (x, y) = point.split_once(',').unwrap();
            (x.parse::<f64>().unwrap(), y.parse::<f64>().unwrap())
        })
        .collect::<Vec<_>>();
    let width = coordinates
        .iter()
        .map(|point| point.0)
        .fold(f64::NEG_INFINITY, f64::max)
        - coordinates
            .iter()
            .map(|point| point.0)
            .fold(f64::INFINITY, f64::min);
    let height = coordinates
        .iter()
        .map(|point| point.1)
        .fold(f64::NEG_INFINITY, f64::max)
        - coordinates
            .iter()
            .map(|point| point.1)
            .fold(f64::INFINITY, f64::min);
    let filter_id = shape
        .attribute("filter")
        .unwrap()
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let filter = xml
        .descendants()
        .find(|node| node.attribute("id") == Some(filter_id))
        .unwrap();
    let region_x = filter.attribute("x").unwrap().parse::<f64>().unwrap();
    let region_y = filter.attribute("y").unwrap().parse::<f64>().unwrap();
    let horizontal_miter = 10.0 * width.hypot(height) / height;
    let vertical_miter = 10.0 * width.hypot(height) / width;
    assert!(-region_x * width + 1e-3 >= horizontal_miter);
    assert!(-region_y * height + 1e-3 >= vertical_miter);
}

#[test]
fn host_region_ceiling_is_enforced_before_node_filter_emission() {
    use merman_render::diagram_theme::{ThemeResourceLimitId, ThemeResourcePolicy};
    let theme = theme(ThemeRuleSet::default());
    let parsed = merman_render::__private::install_parse_compatibility(&theme, Engine::new())
        .parse_diagram_for_render_model_sync("flowchart LR\nA --> B", ParseOptions::strict())
        .unwrap()
        .unwrap();
    let ceiling = ThemeResourcePolicy::default()
        .with_limit(ThemeResourceLimitId::MaxEffectFilterRegionMagnitude, 1)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .with_theme_resource_ceiling(ceiling)
        .begin_session_with_theme(&theme)
        .unwrap();
    let error = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .err()
        .expect("materialized shadow exceeds the caller's ceiling");
    assert!(
        error
            .to_string()
            .contains("max_effect_filter_region_magnitude"),
        "{error}"
    );
}

#[test]
fn relative_source_stroke_width_keeps_geometry_and_effect_residual() {
    let theme = theme(ThemeRuleSet::default());
    for declaration in [
        "style A stroke-width:20%",
        "style A stroke-width:2em",
        "classDef relative stroke-width:20%\nclass A relative",
    ] {
        let source = format!("flowchart LR\nA --> B\n{declaration}");
        let rendered = render(&source, &theme, "classic", false).unwrap();
        assert_eq!(applications(rendered.svg()), 1, "{declaration}");
        assert!(rendered.svg().contains(if declaration.contains("20%") {
            "stroke-width:20%"
        } else {
            "stroke-width:2em"
        }));
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert!(evidence.theme_residual_count() > 0, "{declaration}");
        assert!(render(&source, &theme, "classic", true).is_err());
    }
}

#[test]
fn swimlane_glow_reaches_real_nodes_without_filtering_lane_or_edge_labels() {
    let theme = theme(ThemeRuleSet::default());
    let source = "swimlane-beta LR\nsubgraph Customer\nA[Request]\nend\nsubgraph Support\nB[Respond]\nend\nA -->|Question| B";
    let rendered = render(source, &theme, "classic", true).unwrap();
    assert_eq!(applications(rendered.svg()), 2);
    let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert!(
        xml.descendants()
            .any(|node| node.is_text() && node.text() == Some("Question"))
    );
    for shape in xml
        .descendants()
        .filter(|node| node.attribute("filter").is_some())
    {
        assert!(shape.has_tag_name("rect"));
        assert!(
            shape
                .parent_element()
                .unwrap()
                .attribute("class")
                .unwrap_or_default()
                .split_whitespace()
                .any(|class| class == "node")
        );
    }
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn diamond_paint_and_stroke_receipt_does_not_certify_unsupported_radius() {
    use merman_render::diagram_theme::CanvasPaint;
    let mut patch = ThemeStylePatch::default()
        .with_fill(CanvasPaint::solid("#123abc").unwrap())
        .with_stroke(CanvasPaint::solid("#def012").unwrap());
    patch.stroke.width = Specified::Value(3.0);
    patch.stroke.dasharray = Specified::Value(vec![4.0, 2.0]);
    let themed =
        theme(ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch.clone())));
    let source = "flowchart LR\nA{Decision}";
    let rendered = render(source, &themed, "classic", true).unwrap();
    let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
    let polygon = xml
        .descendants()
        .find(|node| {
            node.has_tag_name("polygon") && node.attribute("class") == Some("label-container")
        })
        .unwrap();
    let style = polygon.attribute("style").unwrap();
    for value in [
        "fill:#123abc",
        "stroke:#def012",
        "stroke-width:3px",
        "stroke-dasharray:4 2",
    ] {
        assert!(style.contains(value), "{style}");
    }
    patch.geometry.radius = Specified::Value(10.0);
    let radius = theme(ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch)));
    assert!(render(source, &radius, "classic", true).is_err());
    let rendered = render(source, &radius, "classic", false).unwrap();
    let completion = rendered.into_completion();
    assert!(
        merman_render::__private::family_evidence(completion.report()).theme_residual_count() > 0
    );
}
