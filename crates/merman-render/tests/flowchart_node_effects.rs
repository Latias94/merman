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
    let document = roxmltree::Document::parse(direct.svg()).unwrap();
    for label in ["Alpha", "Advance", "Beta"] {
        let text = document
            .descendants()
            .find(|node| node.is_text() && node.text() == Some(label))
            .unwrap();
        assert_eq!(
            text.parent_element().unwrap().attribute("font-weight"),
            Some("600"),
            "{label}"
        );
    }
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
fn diamond_paint_and_stroke_preserve_shape_without_claiming_rounded_geometry() {
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
    let rounded_request = render(source, &radius, "classic", true).unwrap();
    assert_eq!(
        rounded_request.svg(),
        rendered.svg(),
        "radius must not redraw the Diamond"
    );
    let completion = rounded_request.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.theme_residual_count(), 0);
    assert!(evidence.applied_count() > 0);
}

#[test]
fn rounded_rect_consumes_typed_radius_and_stroke_without_changing_its_default() {
    let mut patch = ThemeStylePatch::default();
    patch.geometry.radius = Specified::Value(10.0);
    patch.stroke.width = Specified::Value(3.0);
    let rules = ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch));
    let compiler = DiagramThemeCompiler::new();
    let themed = compiler
        .compile(DiagramThemeSpec::new().with_styles(rules.clone()))
        .unwrap();
    let baseline = compiler.compile(DiagramThemeSpec::new()).unwrap();
    for look in ["classic", "neo"] {
        for (selected, radius) in [(&baseline, "5"), (&themed, "10")] {
            let rendered = render("flowchart LR\nA(Round)", selected, look, true).unwrap();
            let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
            let rect = xml
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("class") == Some("basic label-container")
                })
                .unwrap();
            assert_eq!(rect.attribute("rx"), Some(radius), "{look}");
            assert_eq!(rect.attribute("ry"), Some(radius), "{look}");
            if radius == "10" {
                let style = rect.attribute("style").unwrap();
                assert!(style.contains("stroke-width:3px"));
                assert!(style.contains("rx:10px !important"));
                assert!(style.contains("ry:10px !important"));
            }
        }
    }
    let glowing = render("flowchart LR\nA(Round)", &theme(rules), "classic", true).unwrap();
    assert_eq!(applications(glowing.svg()), 1);
}

#[test]
fn public_cyberpunk_recipe_exchange_keeps_flowchart_widths_and_applicable_corners() {
    use merman_render::diagram_theme::ThemePreset;
    let compiler = DiagramThemeCompiler::new();
    let selected = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let wire =
        serde_json::to_vec(&compiler.export_preset(ThemePreset::Cyberpunk).unwrap()).unwrap();
    let imported = DiagramThemeCompiler::new()
        .compile_recipe(serde_json::from_slice(&wire).unwrap())
        .unwrap();
    let source =
        include_str!("../../merman-theme-fixtures/fixtures/public-cyberpunk/flowchart.mmd");
    for theme in [&selected, &imported] {
        let rendered = render(source, theme, "classic", true).unwrap();
        let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
        let nodes: Vec<_> = xml
            .descendants()
            .filter(|node| {
                node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|class| class == "label-container")
                })
            })
            .collect();
        assert_eq!(nodes.len(), 4);
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node.has_tag_name("polygon"))
                .count(),
            1
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node.has_tag_name("rect"))
                .count(),
            3
        );
        for node in nodes {
            if node.has_tag_name("rect") {
                assert_eq!(node.attribute("rx"), Some("10"));
                assert_eq!(node.attribute("ry"), Some("10"));
            } else {
                assert!(node.attribute("rx").is_none());
                assert!(node.attribute("ry").is_none());
            }
            assert!(
                node.attribute("style")
                    .unwrap()
                    .contains("stroke-width:3px")
            );
        }
        let edges: Vec<_> = xml
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .collect();
        assert_eq!(edges.len(), 3);
        for edge in edges {
            assert!(
                edge.attribute("style")
                    .unwrap()
                    .contains("stroke-width:2px")
            );
        }
    }
}

#[test]
fn rounded_rect_radius_preserves_source_ownership_zero_and_unsupported_clear() {
    let compile_radius = |radius| {
        let mut patch = ThemeStylePatch::default();
        patch.geometry.radius = radius;
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch)),
            ))
            .unwrap()
    };
    let radius = compile_radius(Specified::Value(10.0));
    for prefix in ["", "---\nconfig:\n  layout: swimlane\n---\n"] {
        let source = format!("{prefix}flowchart LR\nA(Round)\nstyle A rx:4px,ry:6px");
        let rendered = render(&source, &radius, "classic", true).unwrap();
        let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
        let rect = xml
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class") == Some("basic label-container")
            })
            .unwrap();
        assert_eq!(rect.attribute("rx"), Some("4"));
        assert_eq!(rect.attribute("ry"), Some("6"));
        let style = rect.attribute("style").unwrap();
        assert!(style.contains("rx:4px !important"));
        assert!(style.contains("ry:6px !important"));
        let zero = compile_radius(Specified::Value(0.0));
        let source = format!("{prefix}flowchart LR\nA(Round)");
        let rendered = render(&source, &zero, "classic", true).unwrap();
        let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
        let rect = xml
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class") == Some("basic label-container")
            })
            .unwrap();
        assert_eq!(rect.attribute("rx"), Some("0"));
        assert_eq!(rect.attribute("ry"), Some("0"));
        let clear = compile_radius(Specified::Clear);
        assert!(render(&source, &clear, "classic", true).is_err());
        assert!(render(&source, &radius, "handDrawn", true).is_err());
    }
}

#[test]
fn neo_configuration_keeps_actual_css_radius_ownership_for_both_rectangles() {
    let mut patch = ThemeStylePatch::default();
    patch.geometry.radius = Specified::Value(10.0);
    let selected = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch)),
        ))
        .unwrap();
    for (shape, attribute) in [("[Rectangle]", "4"), ("(Rounded rectangle)", "4")] {
        let source =
            format!("---\nconfig:\n  themeVariables:\n    radius: 4\n---\nflowchart LR\nA{shape}");
        let rendered = render(&source, &selected, "neo", true).unwrap();
        let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
        let rect = xml
            .descendants()
            .find(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class") == Some("basic label-container")
            })
            .unwrap();
        assert_eq!(rect.attribute("rx"), Some(attribute));
        assert_eq!(rect.attribute("ry"), Some(attribute));
        let style = rect.attribute("style").unwrap_or_default();
        assert!(!style.contains("rx:10"));
        assert!(!style.contains("ry:10"));
        assert!(rendered.svg().contains("{rx:4px;ry:4px;}"));
    }
}

#[test]
fn rectangle_source_radius_keeps_each_axis_and_declaration_precedence() {
    let mut patch = ThemeStylePatch::default();
    patch.geometry.radius = Specified::Value(10.0);
    let selected = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch)),
        ))
        .unwrap();
    for look in ["classic", "neo"] {
        for shape in ["[Rectangle]", "(Rounded rectangle)"] {
            for (declarations, rx, ry) in [
                ("style A rx:4px", Some("4"), None),
                ("style A rx:4px!important,RX:6px", Some("4"), None),
                ("style A rx:4px,RX:6px,rx:8px", Some("8"), None),
                ("style A ry:6px", None, Some("6")),
                (
                    "classDef corner rx:4px,ry:6px\nclass A corner\nstyle A ry:2px",
                    Some("4"),
                    Some("2"),
                ),
                ("style A rx:4px!important,rx:8px,ry:0", Some("4"), Some("0")),
            ] {
                let source = format!("flowchart LR\nA{shape}\n{declarations}");
                let rendered = render(&source, &selected, look, true).unwrap();
                let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
                let rect = xml
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("rect")
                            && node.attribute("class") == Some("basic label-container")
                    })
                    .unwrap();
                let fallback = (look == "neo" || shape.starts_with('(')).then_some("5");
                assert_eq!(rect.attribute("rx"), rx.or(fallback), "{look} {source}");
                assert_eq!(rect.attribute("ry"), ry.or(fallback), "{look} {source}");
            }
        }
    }
}

#[test]
fn node_radius_applies_only_to_verified_corner_channels() {
    use merman_render::diagram_theme::CanvasPaint;
    let compiler = DiagramThemeCompiler::new();
    let mut radius = ThemeStylePatch::default();
    radius.geometry.radius = Specified::Value(10.0);
    let radius_rule = ThemeRule::new(ThemeTarget::Node, radius.clone());
    let radius_theme = compiler
        .compile(
            DiagramThemeSpec::new()
                .with_styles(ThemeRuleSet::default().with_rule(radius_rule.clone())),
        )
        .unwrap();
    for prefix in ["", "---\nconfig:\n  layout: swimlane\n---\n"] {
        let diamonds = format!("{prefix}flowchart LR\nA{{Decision}}");
        assert!(render(&diamonds, &radius_theme, "handDrawn", true).is_err());
        let rough = render(&diamonds, &radius_theme, "handDrawn", false)
            .unwrap()
            .into_completion();
        assert_eq!(
            merman_render::__private::family_evidence(rough.report()).theme_residual_count(),
            1
        );
        let result = render(&diamonds, &radius_theme, "classic", true).unwrap();
        let completion = result.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
        let mixed = format!("{prefix}flowchart LR\nA[Rectangle] --> B{{Decision}} --> C(Rounded)");
        let result = render(&mixed, &radius_theme, "classic", true).unwrap();
        let xml = roxmltree::Document::parse(result.svg()).unwrap();
        let rectangles = xml
            .descendants()
            .filter(|node| {
                node.has_tag_name("rect")
                    && node.attribute("class") == Some("basic label-container")
            })
            .collect::<Vec<_>>();
        assert_eq!(rectangles.len(), 2);
        assert!(
            rectangles.iter().all(
                |rect| rect.attribute("rx") == Some("10") && rect.attribute("ry") == Some("10")
            )
        );
        let completion = result.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
        assert!(render(&mixed, &radius_theme, "handDrawn", true).is_err());
        let unknown_channel = format!("{prefix}flowchart LR\nA[Rectangle] --> B((Circle))");
        assert!(render(&unknown_channel, &radius_theme, "classic", true).is_err());
        let source_radius = format!("{diamonds}\nstyle A rx:4px");
        assert!(render(&source_radius, &radius_theme, "classic", true).is_err());
        let mut clear = ThemeStylePatch::default();
        clear.geometry.radius = Specified::Clear;
        let clear = compiler
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, clear)),
            ))
            .unwrap();
        assert!(render(&diamonds, &clear, "classic", true).is_err());
        let paint = ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123abc").unwrap());
        let mut combined = paint.clone();
        combined.geometry.radius = Specified::Value(10.0);
        let combined = compiler
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, combined)),
            ))
            .unwrap();
        let split = compiler
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(ThemeTarget::Node, paint))
                        .with_rule(radius_rule.clone()),
                ),
            )
            .unwrap();
        assert_eq!(
            render(&diamonds, &combined, "classic", true).unwrap().svg(),
            render(&diamonds, &split, "classic", true).unwrap().svg()
        );
        let mut unsupported_sibling = radius.clone();
        unsupported_sibling.paint.opacity = Specified::Value(0.5);
        let unsupported_sibling = compiler
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(ThemeTarget::Node, unsupported_sibling)),
                ),
            )
            .unwrap();
        assert!(render(&diamonds, &unsupported_sibling, "classic", true).is_err());
    }
}
