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
    render_with_html_labels(source, theme, look, strict, false)
}

fn render_with_html_labels(
    source: &str,
    theme: &DiagramTheme,
    look: &str,
    strict: bool,
    html_labels: bool,
) -> merman_render::Result<RenderedFamilySvg> {
    let parsed = merman_render::__private::install_parse_compatibility(
        theme,
        Engine::new().with_site_config(MermaidConfig::from_value(
            json!({"htmlLabels":html_labels, "look":look}),
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

fn filter_reference<'a>(node: roxmltree::Node<'a, '_>) -> Option<&'a str> {
    node.attribute("filter").or_else(|| {
        node.attribute("style")?
            .split("filter:")
            .nth(1)?
            .split(';')
            .next()
    })
}

fn applications(svg: &str) -> usize {
    roxmltree::Document::parse(svg)
        .unwrap()
        .descendants()
        .filter(|node| {
            filter_reference(*node).is_some_and(|value| value.contains("-theme-effect-"))
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
fn public_cyberpunk_recipe_exchange_retains_shape_edge_and_text_glow() {
    use merman_render::diagram_theme::ThemePreset;
    let compiler = DiagramThemeCompiler::new();
    let selected = compiler.compile_preset(ThemePreset::Cyberpunk).unwrap();
    let encoded =
        serde_json::to_vec(&compiler.export_preset(ThemePreset::Cyberpunk).unwrap()).unwrap();
    let imported = DiagramThemeCompiler::new()
        .compile_recipe(serde_json::from_slice(&encoded).unwrap())
        .unwrap();
    let source = "flowchart LR\nA[Alpha] -->|Advance| B[Beta]";
    for html_labels in [false, true] {
        let direct =
            render_with_html_labels(source, &selected, "classic", true, html_labels).unwrap();
        let exchanged =
            render_with_html_labels(source, &imported, "classic", true, html_labels).unwrap();
        assert_eq!(direct.svg(), exchanged.svg());
        let document = roxmltree::Document::parse(direct.svg()).unwrap();
        for label in ["Alpha", "Advance", "Beta"] {
            let text = document
                .descendants()
                .find(|node| node.is_text() && node.text() == Some(label))
                .unwrap();
            assert!(
                text.ancestors().any(|node| {
                    node.attribute("font-weight") == Some("600")
                        || node
                            .attribute("style")
                            .is_some_and(|style| style.contains("font-weight:600"))
                }),
                "{label}: weight 600 must reach the actual text"
            );
            let reference = text
                .ancestors()
                .find_map(filter_reference)
                .expect("public text must have its own glow application");
            let id = reference
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let filter = document
                .descendants()
                .find(|node| node.attribute("id") == Some(id))
                .unwrap();
            let deviations: Vec<_> = filter
                .descendants()
                .filter(|node| node.has_tag_name("feGaussianBlur"))
                .map(|node| node.attribute("stdDeviation").unwrap())
                .collect();
            assert_eq!(
                deviations,
                ["5"],
                "{label}: text uses the reference's 10px shadow"
            );
            assert!(
                !text
                    .ancestors()
                    .find(|node| filter_reference(*node).is_some())
                    .unwrap()
                    .descendants()
                    .any(|node| node.has_tag_name("rect"))
            );
        }
        assert_eq!(applications(direct.svg()), 6);
        let mut deviations = document
            .descendants()
            .filter(|node| node.has_tag_name("feGaussianBlur"))
            .map(|node| node.attribute("stdDeviation").unwrap())
            .collect::<Vec<_>>();
        deviations.sort_unstable();
        assert_eq!(deviations, ["16", "16", "5", "5", "5", "6", "8", "8"]);
    }
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
        .parse_diagram_for_render_model_sync("---\nconfig:\n  look: classic\n  theme: default\n  layout: dagre\n---\nflowchart LR\nA --> B", ParseOptions::strict())
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
            let rendered = render(
                "---\nconfig:\n  theme: default\n---\nflowchart LR\nA(Round)",
                selected,
                look,
                true,
            )
            .unwrap();
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
    let glowing = render(
        "---\nconfig:\n  theme: default\n---\nflowchart LR\nA(Round)",
        &theme(rules),
        "classic",
        true,
    )
    .unwrap();
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
fn neo_configuration_keeps_native_radius_ownership_for_both_rectangles() {
    let mut patch = ThemeStylePatch::default();
    patch.geometry.radius = Specified::Value(10.0);
    let selected = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Node, patch)),
        ))
        .unwrap();
    for shape in ["[Rectangle]", "(Rounded rectangle)"] {
        for (declarations, rx, ry) in [
            ("", "4", "4"),
            ("style A rx:6px", "6", "4"),
            ("style A ry:0", "4", "0"),
        ] {
            let source = format!(
                "---\nconfig:\n  themeVariables:\n    radius: 4\n---\nflowchart LR\nA{shape}\n{declarations}"
            );
            let rendered = render(&source, &selected, "neo", true).unwrap();
            let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
            let rect = xml
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("class") == Some("basic label-container")
                })
                .unwrap();
            assert_eq!(rect.attribute("rx"), Some(rx), "{source}");
            assert_eq!(rect.attribute("ry"), Some(ry), "{source}");
            let style = rect.attribute("style").unwrap_or_default();
            assert!(!style.contains("rx:10"));
            assert!(!style.contains("ry:10"));
            assert!(!rendered.svg().contains("{rx:4px;ry:4px;}"));
        }
    }
}

#[test]
fn neo_rectangle_defaults_keep_shape_specific_radius() {
    let theme = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    for (palette, default_radius) in [("default", "5"), ("neo", "3")] {
        for (shape, radius) in [
            ("[Rectangle]", None),
            ("(Rounded rectangle)", Some(default_radius)),
        ] {
            let source = format!("---\nconfig:\n  theme: {palette}\n---\nflowchart LR\nA{shape}\n");
            let rendered = render(&source, &theme, "neo", true).unwrap();
            let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
            let rect = xml
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("class") == Some("basic label-container")
                })
                .unwrap();
            assert_eq!(rect.attribute("rx"), radius, "{source}");
            assert_eq!(rect.attribute("ry"), radius, "{source}");
        }
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
                let source = format!(
                    "---\nconfig:\n  theme: default\n---\nflowchart LR\nA{shape}\n{declarations}"
                );
                let rendered = render(&source, &selected, look, true).unwrap();
                let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
                let rect = xml
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("rect")
                            && node.attribute("class") == Some("basic label-container")
                    })
                    .unwrap();
                let fallback = shape.starts_with('(').then_some("5");
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

fn edge_glow_theme_with_rules(rules: ThemeRuleSet, include_node_binding: bool) -> DiagramTheme {
    let graph = EffectGraph::new(
        "edge-glow",
        [EffectPrimitive::DropShadow {
            input: EffectInput::SourceGraphic,
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 6.0,
            spread: 0.0,
            color: ThemeColorValue::parse("rgba(0, 242, 255, 0.6)").unwrap(),
        }],
    )
    .unwrap()
    .with_color_space(EffectColorSpace::Srgb);
    let mut effects = DiagramEffectSet::default().with_graph(graph).unwrap();
    // Deliberately insert Node first to catch accidentally selecting another target's binding.
    if include_node_binding {
        effects = effects
            .with_binding(EffectBinding::new(ThemeTarget::Node, "edge-glow").unwrap())
            .unwrap();
    }
    effects = effects
        .with_binding(EffectBinding::new(ThemeTarget::Edge, "edge-glow").unwrap())
        .unwrap();
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new()
                .with_styles(rules)
                .with_effects(effects),
        )
        .unwrap()
}

fn edge_glow_theme() -> DiagramTheme {
    edge_glow_theme_with_rules(ThemeRuleSet::default(), false)
}

#[test]
fn edge_glow_reaches_horizontal_and_vertical_paths_without_filtering_labels() {
    let theme = edge_glow_theme();
    for direction in ["LR", "TB"] {
        let rendered = render(
            &format!("flowchart {direction}\nA[Alpha] -->|Advance| B[Beta]"),
            &theme,
            "classic",
            true,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
        let edge = xml
            .descendants()
            .find(|node| node.attribute("data-edge") == Some("true"))
            .unwrap();
        let reference = edge
            .attribute("filter")
            .expect("edge must reference its glow");
        let filter_id = reference
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        let filter = xml
            .descendants()
            .find(|node| node.attribute("id") == Some(filter_id))
            .unwrap();
        assert_eq!(filter.attribute("filterUnits"), Some("userSpaceOnUse"));
        assert!(filter.attribute("width").unwrap().parse::<f64>().unwrap() > 0.0);
        assert!(filter.attribute("height").unwrap().parse::<f64>().unwrap() > 0.0);
        let label = xml
            .descendants()
            .find(|node| node.is_text() && node.text() == Some("Advance"))
            .unwrap();
        assert!(
            label
                .ancestors()
                .all(|node| node.attribute("filter").is_none())
        );
        assert_eq!(applications(rendered.svg()), 1);
    }
}

#[test]
fn edge_glow_clear_suppresses_binding_and_ordinal_requests_remain_residual() {
    let source = "flowchart LR\nA --> B --> C";
    let mut clear = ThemeStylePatch::default();
    clear.effects.effect = Specified::Clear;
    let clear_theme = edge_glow_theme_with_rules(
        ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Edge, clear.clone())),
        false,
    );
    let cleared = render(source, &clear_theme, "classic", true).unwrap();
    assert_eq!(applications(cleared.svg()), 0);
    let completion = cleared.into_completion();
    assert_eq!(
        merman_render::__private::family_evidence(completion.report()).theme_residual_count(),
        0
    );

    for effect in [Specified::Clear, Specified::Value("edge-glow".to_owned())] {
        let mut patch = ThemeStylePatch::default();
        patch.effects.effect = effect;
        let ordinal = edge_glow_theme_with_rules(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(ThemeTarget::Edge, patch).with_ordinal(OrdinalSelector::Exact(2)),
            ),
            false,
        );
        let rendered = render(source, &ordinal, "classic", false).unwrap();
        let completion = rendered.into_completion();
        assert!(
            merman_render::__private::family_evidence(completion.report()).theme_residual_count()
                > 0
        );
        assert!(render(source, &ordinal, "classic", true).is_err());
    }
}

#[test]
fn edge_glow_static_rule_owns_its_filter_and_preserves_path_geometry() {
    let source = "flowchart LR\nA --> B --> C";
    let plain = DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new())
        .unwrap();
    let baseline = render(source, &plain, "classic", true).unwrap();
    let mut patch = ThemeStylePatch::default();
    patch.effects.effect = Specified::Value("edge-glow".to_owned());
    let theme = edge_glow_theme_with_rules(
        ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Edge, patch)),
        false,
    );
    let glow = render(source, &theme, "classic", true).unwrap();
    let base_xml = roxmltree::Document::parse(baseline.svg()).unwrap();
    let glow_xml = roxmltree::Document::parse(glow.svg()).unwrap();
    let paths = |xml: &roxmltree::Document<'_>| {
        xml.descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| node.attribute("d").unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(paths(&base_xml), paths(&glow_xml));
    assert_eq!(applications(glow.svg()), 2);
    let completion = glow.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn edge_glow_failure_is_not_hidden_by_stroke_from_the_same_rule() {
    use merman_render::diagram_theme::CanvasPaint;

    let source = "flowchart LR\nA --> B";
    let mut patch = ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123abc").unwrap());
    patch.effects.effect = Specified::Value("edge-glow".to_owned());
    let theme = edge_glow_theme_with_rules(
        ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Edge, patch)),
        false,
    );
    assert!(render(source, &theme, "classic", true).is_ok());

    // Neo consumes the stroke but cannot certify this rule's sibling effect facet.
    let rendered = render(source, &theme, "neo", false).unwrap();
    let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
    let edge = xml
        .descendants()
        .find(|node| node.attribute("data-edge") == Some("true"))
        .unwrap();
    assert!(edge.attribute("style").unwrap().contains("stroke:#123abc"));
    assert_eq!(edge.attribute("filter"), None);
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.theme_residual_count(), 1);
    assert_eq!(evidence.applied_count(), 0);
    assert!(render(source, &theme, "neo", true).is_err());
}

#[test]
fn edge_glow_preserves_multiple_binding_ownership_in_nested_roots() {
    let theme = edge_glow_theme_with_rules(ThemeRuleSet::default(), true);
    let source = "flowchart LR\nsubgraph Outer\nA --> B\nend\nB --> C";
    let rendered = render(source, &theme, "classic", true).unwrap();
    assert_eq!(applications(rendered.svg()), 5);
    let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
    assert_eq!(
        xml.descendants()
            .filter(|node| node.has_tag_name("filter")
                && node.attribute("filterUnits") == Some("userSpaceOnUse"))
            .count(),
        2
    );
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 2);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn edge_glow_rejects_unbounded_source_geometry_and_nonclassic_looks() {
    let theme = edge_glow_theme();
    for (source, look) in [
        ("flowchart LR\nA --> B", "handDrawn"),
        ("flowchart LR\nA --> B", "neo"),
        (
            "flowchart LR\nA --> B\nlinkStyle 0 stroke-width:2em",
            "classic",
        ),
        (
            "flowchart LR\nA --> B\nlinkStyle 0 stroke-miterlimit:20",
            "classic",
        ),
        (
            "flowchart LR\nA --> B\nlinkStyle 0 vector-effect:non-scaling-stroke",
            "classic",
        ),
        (
            "flowchart LR\nA --> B\nlinkStyle 0 filter:blur(2px)",
            "classic",
        ),
    ] {
        let rendered = render(source, &theme, look, false).unwrap();
        assert_eq!(applications(rendered.svg()), 0, "{source} ({look})");
        assert!(
            render(source, &theme, look, true).is_err(),
            "{source} ({look})"
        );
    }
}

#[test]
fn edge_glow_rejects_generated_ancestor_filter_and_geometry_overrides() {
    let theme = edge_glow_theme();
    for html in [false, true] {
        for declaration in [
            "classDef edgePaths filter:none",
            "classDef root filter:none",
            "classDef edgePaths stroke-width:80px",
            "classDef root stroke-miterlimit:100",
            "classDef edgePaths transform:scale(2)",
        ] {
            let source = format!("flowchart LR\nA --> B\n{declaration}");
            let rendered =
                render_with_html_labels(&source, &theme, "classic", false, html).unwrap();
            assert_eq!(
                applications(rendered.svg()),
                0,
                "{declaration}, html={html}"
            );
            let completion = rendered.into_completion();
            assert!(
                merman_render::__private::family_evidence(completion.report())
                    .theme_residual_count()
                    > 0
            );
            assert!(render_with_html_labels(&source, &theme, "classic", true, html).is_err());
        }
        for declaration in [
            "classDef unused filter:none",
            "classDef edgePaths fill:#123456",
        ] {
            let source = format!("flowchart LR\nA --> B\n{declaration}");
            let rendered = render_with_html_labels(&source, &theme, "classic", true, html).unwrap();
            assert_eq!(
                applications(rendered.svg()),
                1,
                "{declaration}, html={html}"
            );
        }
    }
}

fn text_glow_theme_for(targets: &[ThemeTarget], rules: ThemeRuleSet) -> DiagramTheme {
    let graph = EffectGraph::new(
        "label-glow",
        [EffectPrimitive::DropShadow {
            input: EffectInput::SourceGraphic,
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 8.0,
            spread: 0.0,
            color: ThemeColorValue::parse("#ff2080").unwrap(),
        }],
    )
    .unwrap()
    .with_color_space(EffectColorSpace::Srgb);
    let mut effects = DiagramEffectSet::default().with_graph(graph).unwrap();
    for target in targets {
        effects = effects
            .with_binding(EffectBinding::new(*target, "label-glow").unwrap())
            .unwrap();
    }
    let spec = DiagramThemeSpec::new()
        .with_styles(rules)
        .with_effects(effects);
    DiagramThemeCompiler::new().compile(spec).unwrap()
}

#[test]
fn html_node_glow_filters_only_plain_label_content() {
    let theme = text_glow_theme_for(&[ThemeTarget::NodeLabel], ThemeRuleSet::default());
    let source = "flowchart LR\nsubgraph Group\nA[Alpha] --> B(Round) --> C{Diamond} --> D((Circle)) --> E(((Double)))\nF[\"<span>Two</span><br/>Rows\"]\nend";
    let rendered = render_with_html_labels(source, &theme, "classic", true, true).unwrap();
    let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
    let filtered: Vec<_> = xml
        .descendants()
        .filter(|n| n.attribute("filter").is_some())
        .collect();
    assert_eq!(filtered.len(), 6);
    for terminal in filtered {
        assert!(
            terminal
                .descendants()
                .any(|n| n.has_tag_name("foreignObject"))
        );
        assert!(
            !terminal
                .descendants()
                .any(|n| matches!(n.tag_name().name(), "rect" | "path" | "polygon" | "circle"))
        );
        assert!(terminal.descendants().any(|n| n.is_text()));
    }
}

#[test]
fn html_node_glow_does_not_admit_rich_or_unbounded_source_content() {
    let mut clear = ThemeStylePatch::default();
    clear.effects.effect = Specified::Clear;
    for rules in [
        ThemeRuleSet::default(),
        ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::NodeLabel, clear)),
    ] {
        let theme = text_glow_theme_for(&[ThemeTarget::NodeLabel], rules);
        for source in [
            "flowchart LR\nA[\"<span style='background:red'>Alpha</span>\"]",
            "flowchart LR\nA[\"<b>Alpha</b>\"]",
            "flowchart LR\nA[Alpha]\nclassDef label font-size:80px",
            "flowchart LR\nA[Alpha]\nclassDef nodeLabel background:red",
            "flowchart LR\nA[(Cylinder)]",
            "flowchart LR\nA[Alpha]:::edgeLabel",
            "flowchart LR\nA[Alpha]:::pink\nclassDef pink background:red",
            "flowchart LR\nA[Alpha]\nclassDef default background:red",
            "flowchart LR\nA[Alpha]:::pad\nclassDef pad padding:40px",
            "flowchart LR\nA[Alpha]:::filtered\nclassDef filtered filter:blur(20px)",
            "flowchart LR\nA[Alpha]:::icon-shape",
        ] {
            assert!(
                render_with_html_labels(source, &theme, "classic", true, true).is_err(),
                "{source}"
            );
            let rendered = render_with_html_labels(source, &theme, "classic", false, true).unwrap();
            assert_eq!(applications(rendered.svg()), 0, "{source}");
            let completion = rendered.into_completion();
            assert!(
                merman_render::__private::family_evidence(completion.report())
                    .theme_residual_count()
                    > 0,
                "{source}"
            );
        }
    }
}

#[test]
fn html_node_glow_does_not_inherit_sibling_cluster_styles() {
    let theme = text_glow_theme_for(&[ThemeTarget::NodeLabel], ThemeRuleSet::default());
    let plain = text_glow_theme_for(&[], ThemeRuleSet::default());
    for source in [
        "flowchart LR\nsubgraph Group\nA[Alpha]\nend\nclass Group image-shape",
        "flowchart LR\nsubgraph Group\nA[Alpha]\nend\nclass Group huge\nclassDef huge font-size:80px",
        "flowchart LR\nA[Alpha]:::styled\nclassDef styled fill:#eee,stroke:#111,stroke-width:3px,font-size:20px",
    ] {
        let baseline = render_with_html_labels(source, &plain, "classic", false, true)
            .unwrap()
            .into_completion();
        let rendered = render_with_html_labels(source, &theme, "classic", false, true).unwrap();
        assert_eq!(applications(rendered.svg()), 1, "{source}");
        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.theme_residual_count(), 0, "{source}");
        // Source typography has its own admission. Applying a shadow neither certifies
        // nor obscures a pre-existing source-style residual on a different terminal.
        let source_residuals =
            merman_render::__private::family_evidence(baseline.report()).source_residual_count();
        assert_eq!(
            evidence.source_residual_count(),
            source_residuals,
            "{source}"
        );
        if source_residuals == 0 {
            render_with_html_labels(source, &theme, "classic", true, true).unwrap();
        } else {
            assert!(matches!(
                render_with_html_labels(source, &theme, "classic", true, true),
                Err(merman_render::Error::UnverifiedFamilyStyle { .. })
            ));
        }
    }
}

#[test]
fn html_edge_label_glow_keeps_the_composited_background_outside_the_filter() {
    use merman_core::theme_color::{ColorChannel, ThemeColor};
    for padding in [
        merman_render::diagram_theme::InsetsPx::all(0.0),
        merman_render::diagram_theme::InsetsPx {
            top: 3.0,
            right: 7.0,
            bottom: 5.0,
            left: 11.0,
        },
    ] {
        let rules = ThemeRuleSet::default()
            .with_rule(ThemeRule::new(
                ThemeTarget::EdgeLabel,
                ThemeStylePatch::default().with_padding(padding),
            ))
            .with_rule(ThemeRule::new(
                ThemeTarget::EdgeLabelBackground,
                ThemeStylePatch::default().with_fill(
                    merman_render::diagram_theme::CanvasPaint::solid("#00ff00").unwrap(),
                ),
            ));
        let theme = text_glow_theme_for(&[ThemeTarget::EdgeLabel], rules);
        for (background, alpha) in [
            ("#ff0000", 1.0),
            ("rgba(255,0,0,0.4)", 0.7),
            ("transparent", 0.5),
        ] {
            for label in ["Advance", "<span>First</span><br/>Second"] {
                let source = format!(
                    "---\nconfig:\n  themeVariables:\n    edgeLabelBackground: '{background}'\n---\nflowchart LR\nA[Alpha] -->|{label}| B[Beta]"
                );
                let rendered =
                    render_with_html_labels(&source, &theme, "classic", true, true).unwrap();
                let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
                let filtered: Vec<_> = xml
                    .descendants()
                    .filter(|n| filter_reference(*n).is_some())
                    .collect();
                assert_eq!(filtered.len(), 1, "{source}");
                let text = filtered[0];
                assert!(text.has_tag_name("p"));
                assert!(
                    text.attribute("style")
                        .unwrap()
                        .contains("background:transparent;")
                );
                assert!(!text.descendants().any(|n| n.has_tag_name("rect")));
                let div = text.ancestors().find(|n| n.has_tag_name("div")).unwrap();
                let painted = div
                    .attribute("style")
                    .unwrap()
                    .split("background:")
                    .last()
                    .unwrap()
                    .trim_end_matches(';');
                let color = ThemeColor::parse(painted).unwrap();
                assert!((color.channel(ColorChannel::Alpha) - alpha).abs() < 0.001);
                assert_eq!(
                    color.channel(ColorChannel::Red),
                    if background == "transparent" {
                        0.0
                    } else {
                        255.0
                    }
                );
                let fo = div.parent().unwrap();
                assert_eq!(
                    fo.attribute("data-merman-fallback-text-filter"),
                    filter_reference(text)
                );
            }
        }
    }
}

#[test]
fn html_edge_label_effect_and_clear_reject_unbounded_background_or_text() {
    let mut clear = ThemeStylePatch::default();
    clear.effects.effect = Specified::Clear;
    for rules in [
        ThemeRuleSet::default(),
        ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::EdgeLabel, clear)),
    ] {
        let theme = text_glow_theme_for(&[ThemeTarget::EdgeLabel], rules);
        for source in [
            "flowchart LR\nA -->|<b>Advance</b>| B",
            "flowchart LR\nA -->|<span style='background:red'>Advance</span>| B",
            "flowchart LR\nA -->|Advance| B\nclassDef labelBkg padding:40px",
            "flowchart LR\nA -->|Advance| B\nclassDef edgeLabel fill:red",
            "flowchart LR\nA -->|Advance| B\nclassDef edgeLabel opacity:0.2",
            "flowchart LR\nA -->|Advance| B\nclassDef label filter:blur(20px)",
        ] {
            assert!(
                render_with_html_labels(source, &theme, "classic", true, true).is_err(),
                "{source}"
            );
            let rendered = render_with_html_labels(source, &theme, "classic", false, true).unwrap();
            assert_eq!(applications(rendered.svg()), 0, "{source}");
            assert!(
                merman_render::__private::family_evidence(rendered.into_completion().report())
                    .theme_residual_count()
                    > 0,
                "{source}"
            );
        }
    }
    let mut clear = ThemeStylePatch::default();
    clear.effects.effect = Specified::Clear;
    let theme = text_glow_theme_for(
        &[ThemeTarget::EdgeLabel],
        ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::EdgeLabel, clear)),
    );
    let rendered = render_with_html_labels(
        "flowchart LR\nA -->|Advance| B",
        &theme,
        "classic",
        true,
        true,
    )
    .unwrap();
    assert_eq!(applications(rendered.svg()), 0);
}

#[test]
fn label_glow_clear_preserves_other_terminals_and_nested_viewport() {
    for html_labels in [false, true] {
        let targets = &[ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel];
        let mut clear = ThemeStylePatch::default();
        clear.effects.effect = Specified::Clear;
        let theme = text_glow_theme_for(
            targets,
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(ThemeTarget::NodeLabel, clear)
                    .with_ordinal(OrdinalSelector::Exact(1)),
            ),
        );
        let rendered = render_with_html_labels(
            "flowchart LR\nsubgraph Group\nA[Alpha] -->|Advance| B[Beta]\nend",
            &theme,
            "classic",
            true,
            html_labels,
        )
        .unwrap();
        let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
        let filtered: Vec<_> = xml
            .descendants()
            .filter(|node| filter_reference(*node).is_some())
            .collect();
        assert_eq!(filtered.len(), 2);
        let visible: Vec<_> = filtered
            .iter()
            .flat_map(|node| node.descendants())
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect();
        assert!(!visible.contains(&"Alpha"));
        assert!(visible.contains(&"Beta"));
        assert!(visible.contains(&"Advance"));
        let viewport: Vec<f64> = xml
            .root_element()
            .attribute("viewBox")
            .unwrap()
            .split_whitespace()
            .map(|value| value.parse().unwrap())
            .collect();
        for terminal in &filtered {
            let id = filter_reference(*terminal)
                .unwrap()
                .strip_prefix("url(#")
                .unwrap()
                .strip_suffix(')')
                .unwrap();
            let filter = xml
                .descendants()
                .find(|node| node.attribute("id") == Some(id))
                .unwrap();
            let mut translation = (0.0, 0.0);
            for transform in terminal
                .ancestors()
                .filter_map(|node| node.attribute("transform"))
            {
                for token in svgtypes::TransformListParser::from(transform) {
                    let svgtypes::TransformListToken::Translate { tx, ty } = token.unwrap() else {
                        panic!("this fixture expects translation-only groups: {transform}");
                    };
                    translation.0 += tx;
                    translation.1 += ty;
                }
            }
            let number = |name| filter.attribute(name).unwrap().parse::<f64>().unwrap();
            let x = number("x") + translation.0;
            let y = number("y") + translation.1;
            assert!(
                x >= viewport[0] - 0.001 && y >= viewport[1] - 0.001,
                "nested text filter origin must fit the viewport: ({x}, {y}) vs {viewport:?}"
            );
            assert!(
                x + number("width") <= viewport[0] + viewport[2] + 0.001
                    && y + number("height") <= viewport[1] + viewport[3] + 0.001,
                "nested text filter extent must fit the viewport: ({x}, {y}) vs {viewport:?}"
            );
        }

        let plain = text_glow_theme_for(&[], ThemeRuleSet::default());
        let source = "flowchart LR\nA[Alpha] -->|Advance| B[Beta]";
        let plain = render_with_html_labels(source, &plain, "classic", true, html_labels).unwrap();
        let glow = render_with_html_labels(
            source,
            &text_glow_theme_for(targets, ThemeRuleSet::default()),
            "classic",
            true,
            html_labels,
        )
        .unwrap();
        let plain_xml = roxmltree::Document::parse(plain.svg()).unwrap();
        let glow_xml = roxmltree::Document::parse(glow.svg()).unwrap();
        let viewbox = |xml: &roxmltree::Document<'_>| {
            xml.root_element()
                .attribute("viewBox")
                .unwrap()
                .split_whitespace()
                .map(|value| value.parse::<f64>().unwrap())
                .collect::<Vec<_>>()
        };
        let before = viewbox(&plain_xml);
        let after = viewbox(&glow_xml);
        assert!(
            after[3] > before[3],
            "text glow must expand the viewport: {before:?} -> {after:?}"
        );
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
        assert_eq!(positions(&plain_xml), positions(&glow_xml));
    }
}

#[test]
fn native_label_glow_rejects_unmeasured_structural_styles() {
    {
        let theme = text_glow_theme_for(
            &[ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel],
            ThemeRuleSet::default(),
        );
        for declaration in ["font-weight:800", "stroke-width:20px", "transform:scale(2)"] {
            let source = format!(
                "flowchart LR\nA[Alpha] -->|Advance| B[Beta]\nclassDef label {declaration}"
            );
            assert!(
                render(&source, &theme, "classic", true).is_err(),
                "{declaration}"
            );
            let rendered = render(&source, &theme, "classic", false).unwrap();
            let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
            assert!(
                !xml.descendants()
                    .any(|node| node.attribute("filter").is_some()),
                "{declaration}"
            );
        }
    }
}

#[test]
fn native_label_glow_keeps_shape_only_source_strokes_separate() {
    {
        let theme = text_glow_theme_for(
            &[ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel],
            ThemeRuleSet::default(),
        );
        let rendered = render("flowchart LR\nA[Alpha] -->|Advance| B[Beta]\nstyle A stroke-width:4px\nlinkStyle 0 stroke-width:3px", &theme, "classic", true).unwrap();
        let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert_eq!(
            xml.descendants()
                .filter(|node| node.attribute("filter").is_some())
                .count(),
            3
        );
    }
}

#[test]
fn ordinary_native_label_glow_uses_prepared_host_metrics_without_font_assets() {
    let theme = text_glow_theme_for(
        &[ThemeTarget::NodeLabel, ThemeTarget::EdgeLabel],
        ThemeRuleSet::default(),
    );
    for source in [
        "flowchart LR\nA[Alpha] -->|Advance| B[Beta]",
        "flowchart TB\nsubgraph Outer\nA[Alpha<br/><br/>中文] -->|Advance<br/>继续| B[Beta]\nend",
    ] {
        let rendered = render(source, &theme, "classic", true).unwrap();
        let xml = roxmltree::Document::parse(rendered.svg()).unwrap();
        let filtered: Vec<_> = xml
            .descendants()
            .filter(|node| node.attribute("filter").is_some())
            .collect();
        assert_eq!(filtered.len(), 3);
        for terminal in filtered {
            assert!(terminal.descendants().any(|node| node.has_tag_name("text")));
            assert!(!terminal.descendants().any(|node| matches!(
                node.tag_name().name(),
                "rect" | "polygon" | "circle" | "path" | "foreignObject"
            )));
        }
    }
}

#[test]
fn typed_html_background_padding_keeps_natural_paint_and_retains_a_residual() {
    use merman_render::diagram_theme::{CanvasPaint, InsetsPx};
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(ThemeRule::new(
                        ThemeTarget::EdgeLabelBackground,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("rgba(255,0,0,0.4)").unwrap()),
                    ))
                    .with_rule(ThemeRule::new(
                        ThemeTarget::EdgeLabel,
                        ThemeStylePatch::default().with_padding(InsetsPx {
                            top: 3.0,
                            right: 7.0,
                            bottom: 5.0,
                            left: 11.0,
                        }),
                    )),
            ),
        )
        .unwrap();
    let body = "A -->|A long label with natural HTML wrapping that can exceed host estimates| B";
    let special = "C@{ icon: 'missing:icon', label: 'Special label', form: 'square' }";
    for body in [
        body.to_owned(),
        format!("{body}\n{special}"),
        format!("{special}\n{body}"),
    ] {
        let source = format!("flowchart LR\n{body}");
        let output = render_with_html_labels(&source, &theme, "classic", false, true).unwrap();
        let xml = roxmltree::Document::parse(output.svg()).unwrap();
        assert!(xml.descendants().any(|node| node.has_tag_name("div") && node.attribute("class") == Some("labelBkg")));
        assert!(
            !xml.descendants()
                .any(|node| node.has_tag_name("rect")
                    && node.attribute("class") == Some("background"))
        );
        let evidence = merman_render::__private::family_evidence(output.into_completion().report());
        assert_eq!(evidence.theme_residual_count(), 1, "{source}");
        assert!(
            render_with_html_labels(&source, &theme, "classic", true, true).is_err(),
            "{source}"
        );
        assert!(
            render_with_html_labels(&source, &theme, "classic", true, false).is_ok(),
            "{source}"
        );
    }
}
