#![cfg(feature = "diagram-flowchart")]

//! Mermaid 12.2 circle.ts, doubleCircle.ts and labelHelper's ignoreMinWidth contract.
//! Source: 1fb7869866dd619168d840291c9c54b4f129758b.

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationConfig;
use merman_render::environment::RenderEnvironment;
use merman_render::model::{FlowchartLayout, LayoutNode};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{DeterministicTextMeasurer, TextMeasurer, TextStyle};
use merman_render::{LayoutOptions, family};
use serde_json::json;

const LAYOUTS: &[&str] = if cfg!(feature = "layout-elk") {
    &["dagre", "elk"]
} else {
    &["dagre"]
};

fn render(
    layout: &str,
    look: &str,
    html: bool,
    minimum: f64,
) -> (FlowchartLayout, String, TextStyle) {
    render_source(
        "flowchart LR\nA((a)) --> B(((a))) --> R[a]\n",
        common::presentation_config(json!({
            "layout": layout,
            "look": look,
            "handDrawnSeed": 1,
            "htmlLabels": html,
            "flowchart": { "minNodeWidth": minimum, "padding": 16, "wrappingWidth": 120 },
        })),
    )
}

fn render_source(input: &str, config: PresentationConfig) -> (FlowchartLayout, String, TextStyle) {
    let parsed = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_config(
                config.as_config().clone(),
            ),
        )
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse circle diagram")
        .expect("detect flowchart")
        .into_presentation();
    let artifact = family::prepare(
        &parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("prepare circle diagram");
    let config = artifact.metadata().effective_config.as_value();
    let text_style = TextStyle {
        font_family: config
            .pointer("/themeVariables/fontFamily")
            .or_else(|| config.get("fontFamily"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        font_size: config
            .pointer("/themeVariables/fontSize")
            .and_then(|value| {
                value.as_f64().or_else(|| {
                    value
                        .as_str()
                        .and_then(|value| value.trim().trim_end_matches("px").parse().ok())
                })
            })
            .unwrap_or(16.0),
        font_weight: config
            .get("fontWeight")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        font_style: None,
    };
    let layout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone())
            .expect("flowchart layout");
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("circle-12-2".to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render circle diagram");
    (layout, svg.svg().to_owned(), text_style)
}

fn node<'a>(layout: &'a FlowchartLayout, id: &str) -> &'a LayoutNode {
    layout.nodes.iter().find(|node| node.id == id).unwrap()
}

fn source_node<'a>(document: &'a roxmltree::Document<'_>, id: &str) -> roxmltree::Node<'a, 'a> {
    let mut matches = document.descendants().filter(|group| {
        group.has_tag_name("g")
            && group.attribute("data-id") == Some(id)
            && group.attribute("data-et") == Some("node")
    });
    let node = matches.next().expect("rendered source node");
    assert!(matches.next().is_none(), "duplicate source node occurrence");
    let prefix = format!(
        "{}-merman-flowchart-document-node-",
        document.root_element().attribute("id").unwrap(),
    );
    let ordinal = node
        .attribute("id")
        .unwrap()
        .strip_prefix(&prefix)
        .expect("node DOM identity belongs to its canonical document owner");
    assert!(!ordinal.is_empty() && ordinal.bytes().all(|byte| byte.is_ascii_digit()));
    let ids = document
        .descendants()
        .filter_map(|node| node.attribute("id"))
        .collect::<Vec<_>>();
    assert_eq!(
        ids.len(),
        ids.iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        "duplicate SVG DOM IDs"
    );
    node
}

#[cfg(feature = "layout-elk")]
fn upstream_source_node<'a>(
    document: &'a roxmltree::Document<'_>,
    id: &str,
) -> roxmltree::Node<'a, 'a> {
    let mut matches = document.descendants().filter(|group| {
        group.has_tag_name("g")
            && group.attribute("id").is_some_and(|value| {
                value
                    .split_once(&format!("-flowchart-{id}-"))
                    .is_some_and(|(scope, ordinal)| {
                        scope == document.root_element().attribute("id").unwrap()
                            && !ordinal.is_empty()
                            && ordinal.bytes().all(|byte| byte.is_ascii_digit())
                    })
            })
    });
    let node = matches.next().expect("pinned upstream source node");
    assert!(matches.next().is_none(), "duplicate pinned source node");
    node
}

fn label_dimensions(document: &roxmltree::Document<'_>, id: &str, style: &TextStyle) -> (f64, f64) {
    let group = source_node(document, id);
    if let Some(label) = group
        .descendants()
        .find(|element| element.has_tag_name("foreignObject"))
    {
        let dimension = |name| label.attribute(name).unwrap().parse::<f64>().unwrap();
        return (dimension("width"), dimension("height"));
    }

    let text = group
        .descendants()
        .find(|element| element.has_tag_name("text"))
        .expect("SVG node label");
    let visible: String = text
        .descendants()
        .filter(|element| element.is_text())
        .map(|element| element.text().unwrap())
        .collect();
    assert_eq!(visible, "a");
    let rows: Vec<_> = text
        .children()
        .filter(|element| element.has_tag_name("tspan"))
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "the circle fixture must remain a single SVG row"
    );
    assert_eq!(rows[0].attribute("dy"), Some("1.1em"));
    // The circle encloses literal glyph bounds, not createText's 1.1em row advance.
    let measurer = DeterministicTextMeasurer::default();
    (
        measurer.measure_svg_tspan_text_bbox_width_px(&visible, style),
        measurer.measure_svg_tspan_text_bbox_height_px(&visible, style),
    )
}

fn radius(document: &roxmltree::Document<'_>, id: &str, class: Option<&str>) -> f64 {
    source_node(document, id)
        .descendants()
        .find(|element| {
            element.has_tag_name("circle")
                && class.is_none_or(|class| {
                    element.attribute("class").is_some_and(|classes| {
                        classes.split_whitespace().any(|value| value == class)
                    })
                })
        })
        .expect("rendered circle")
        .attribute("r")
        .unwrap()
        .parse()
        .unwrap()
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
}

#[test]
fn circles_ignore_diagram_minimum_in_layout_and_svg_for_dagre_and_elk() {
    for algorithm in LAYOUTS {
        for look in ["classic", "neo"] {
            for html in [false, true] {
                let (natural, natural_svg, natural_style) = render(algorithm, look, html, 0.0);
                let (minimum, svg, style) = render(algorithm, look, html, 120.0);
                let natural_document = roxmltree::Document::parse(&natural_svg).unwrap();
                let document = roxmltree::Document::parse(&svg).unwrap();
                for id in ["A", "B"] {
                    let natural = node(&natural, id);
                    let current = node(&minimum, id);
                    assert_close(current.width, natural.width);
                    assert_close(current.height, natural.height);
                    let natural_label = label_dimensions(&natural_document, id, &natural_style);
                    let current_label = label_dimensions(&document, id, &style);
                    assert_close(current_label.0, natural_label.0);
                    assert_close(current_label.1, natural_label.1);
                    assert_close(current.width, current.height);
                    assert!(current_label.0 < 120.0);
                }
                assert!(node(&minimum, "R").width > node(&natural, "R").width);

                let circle = node(&minimum, "A");
                let (label_width, label_height) = label_dimensions(&document, "A", &style);
                let label_radius = label_width.hypot(label_height) / 2.0;
                let circle_radius = label_radius + if look == "neo" { 16.0 } else { 8.0 };
                assert_close(circle.width, 2.0 * circle_radius);
                assert_close(radius(&document, "A", None), circle_radius);

                let double = node(&minimum, "B");
                let (label_width, label_height) = label_dimensions(&document, "B", &style);
                let inner_radius = label_width.hypot(label_height) / 2.0 + 16.0;
                let outer_radius = inner_radius + if look == "neo" { 12.0 } else { 5.0 };
                assert_close(double.width, 2.0 * outer_radius);
                assert_close(radius(&document, "B", Some("inner-circle")), inner_radius);
                assert_close(radius(&document, "B", Some("outer-circle")), outer_radius);
            }
        }
    }
}

fn shape<'a>(document: &'a roxmltree::Document<'_>, id: &str) -> roxmltree::Node<'a, 'a> {
    source_node(document, id)
        .children()
        .find(roxmltree::Node::is_element)
        .expect("shape precedes label")
}

fn assert_rough_ring(ring: roxmltree::Node<'_, '_>, stroke_width: f64) {
    assert!(ring.has_tag_name("g"));
    let paths: Vec<_> = ring
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(paths.len(), 2, "Rough fill followed by outline");
    for path in &paths {
        assert!(path.has_tag_name("path"));
        assert_eq!(path.attribute("fill"), Some("none"));
        assert_eq!(path.attribute("stroke-dasharray"), Some("0 0"));
        let data = path.attribute("d").expect("generated Rough path");
        assert!(data.contains('M') && data.contains('C'));
    }
    assert_eq!(paths[0].attribute("stroke-width"), Some("1.5"));
    assert_eq!(
        paths[1]
            .attribute("stroke-width")
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        stroke_width,
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn hand_drawn_circle_fixture_preserves_pinned_rough_groups_and_ring_order() {
    // This browser artifact is deliberately family-local: text wrapping, Rough control points,
    // getBBox() and propagated ELK routes remain bounded browser residuals.
    let input = include_str!(
        "../../../fixtures/_family_local/mermaid-12-2/flowchart/local_mermaid_12_2_handdrawn_circle_min_width.mmd"
    );
    let upstream = include_str!(
        "../../../fixtures/_family_local/mermaid-12-2/flowchart/local_mermaid_12_2_handdrawn_circle_min_width.upstream.svg"
    );
    let (minimum, svg, _) = render_source(input, PresentationConfig::empty_object());
    let (_, repeated, _) = render_source(input, PresentationConfig::empty_object());
    assert_eq!(
        svg, repeated,
        "fixture seed is inherited by the render session"
    );
    let natural_input = input.replace("minNodeWidth: 180", "minNodeWidth: 0");
    let (natural, _, _) = render_source(&natural_input, PresentationConfig::empty_object());
    for id in ["A", "B"] {
        assert_close(node(&minimum, id).width, node(&natural, id).width);
        assert_close(node(&minimum, id).height, node(&natural, id).height);
        assert!(node(&minimum, id).width < 180.0);
    }
    assert!(node(&minimum, "R").width > node(&natural, "R").width);
    // The existing roughr boundary stores stroke width as f32; the browser uses a JS Number.
    // Assert each exact representation instead of accepting arbitrary numeric drift.
    for (svg, circle_stroke_width, canonical_ids) in [
        (svg.as_str(), f64::from(1.3_f32), true),
        (upstream, 1.3, false),
    ] {
        let document = roxmltree::Document::parse(svg).unwrap();
        let source_node = |id| {
            if canonical_ids {
                source_node(&document, id)
            } else {
                upstream_source_node(&document, id)
            }
        };
        let shape = |id| {
            source_node(id)
                .children()
                .find(roxmltree::Node::is_element)
                .expect("shape precedes label")
        };
        let circle = shape("A");
        assert_eq!(circle.attribute("class"), Some("basic label-container"));
        assert_rough_ring(circle, circle_stroke_width);
        let double = shape("B");
        assert_eq!(double.attribute("class"), Some("default"));
        let rings: Vec<_> = double
            .children()
            .filter(roxmltree::Node::is_element)
            .collect();
        assert_eq!(rings.len(), 2);
        assert_rough_ring(rings[0], 2.5);
        assert_rough_ring(rings[1], 1.5);
        for id in ["A", "B"] {
            assert_eq!(source_node(id).attribute("data-look"), Some("handDrawn"));
            assert!(
                !source_node(id)
                    .descendants()
                    .any(|node| node.has_tag_name("circle"))
            );
        }
    }
}

#[test]
fn hand_drawn_circles_keep_the_source_construction_dimensions() {
    for algorithm in LAYOUTS {
        for html in [false, true] {
            let (natural, _, _) = render(algorithm, "handDrawn", html, 0.0);
            let (minimum, svg, style) = render(algorithm, "handDrawn", html, 180.0);
            let document = roxmltree::Document::parse(&svg).unwrap();
            for (id, padding) in [("A", 8.0), ("B", 21.0)] {
                let current = node(&minimum, id);
                let (width, height) = label_dimensions(&document, id, &style);
                let diameter = width.hypot(height) + padding * 2.0;
                assert_close(current.width, diameter);
                assert_close(current.height, diameter);
                assert_close(current.width, node(&natural, id).width);
            }
            assert_rough_ring(shape(&document, "A"), f64::from(1.3_f32));
        }
    }
}

#[test]
fn hand_drawn_circle_hachure_enforces_actual_work_boundaries() {
    use merman_model::{OperationControl, OperationLedgerError, OperationPhase};
    use merman_render::resources::{
        RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
    };

    for (source, double_circle) in [
        ("flowchart LR\nA((a))\n", false),
        ("flowchart LR\nA(((a)))\n", true),
    ] {
        for (font_size, padding) in [(1e4, 16.0), (16.0, 1e4)] {
            let parse = |look| {
                MermaidEngine::new()
                    .with_site_config(
                        merman_core::presentation::SourcePresentationConfig::from_value(json!({
                            "look": look,
                            "handDrawnSeed": 1,
                            "htmlLabels": false,
                            "themeVariables": { "fontSize": font_size },
                            "flowchart": { "padding": padding },
                        })),
                    )
                    .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .into_presentation()
            };
            let parsed = parse("handDrawn");
            let prepare = |policy, control| {
                family::prepare(
                    &parsed,
                    &LayoutOptions::headless_svg_defaults(),
                    RenderEnvironment::deterministic()
                        .with_resource_policy(policy)
                        .begin_session_with_control(control)
                        .unwrap(),
                )
                .expect("the small model fits before render-time Rough generation")
            };
            let policy = RenderResourcePolicy::default();
            let svg_options = SvgRenderOptions::default();
            let debug = SvgDebugOptions::default();
            let (expected_svg, report) = prepare(policy, OperationControl::new())
                .render_svg(&svg_options, &debug)
                .expect("these finite dimensions fit the actual default work budget")
                .into_completion()
                .into_output_and_report();
            let required_work = report.session_report().layout_work_units();
            let document = roxmltree::Document::parse(&expected_svg).unwrap();
            let circle = shape(&document, "A");
            let rings = if double_circle {
                circle
                    .children()
                    .filter(roxmltree::Node::is_element)
                    .collect::<Vec<_>>()
            } else {
                vec![circle]
            };
            assert_eq!(rings.len(), if double_circle { 2 } else { 1 });
            let mut fill_operations = 0;
            for (index, ring) in rings.into_iter().enumerate() {
                let stroke_width = if double_circle {
                    if index == 0 { 2.5 } else { 1.5 }
                } else {
                    f64::from(1.3_f32)
                };
                assert_rough_ring(ring, stroke_width);
                let fill = ring.children().find(roxmltree::Node::is_element).unwrap();
                for segment in roughr::PathParser::from(fill.attribute("d").unwrap()) {
                    segment.expect("valid generated hachure operation");
                    fill_operations += 1;
                }
            }
            assert!(fill_operations > 0);

            let exact_policy = policy
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, required_work)
                .unwrap();
            let exact = prepare(exact_policy, OperationControl::new())
                .render_svg(&svg_options, &debug)
                .expect("the measured exact work ceiling admits unchanged Rough output")
                .into_completion();
            assert_eq!(exact.output(), &expected_svg);
            assert_eq!(
                exact.report().session_report().layout_work_units(),
                required_work
            );

            let below_policy = policy
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, required_work - 1)
                .unwrap();
            let classic = parse("classic");
            let classic = family::prepare(
                &classic,
                &LayoutOptions::headless_svg_defaults(),
                RenderEnvironment::deterministic()
                    .with_resource_policy(below_policy)
                    .begin_session()
                    .unwrap(),
            )
            .expect("classic dimensions fit the same bounded model preparation")
            .render_svg(&svg_options, &debug)
            .expect("classic circles fit without Rough generation")
            .into_completion();
            let classic_work = classic.report().session_report().layout_work_units();
            assert!(required_work >= classic_work + fill_operations);

            let generation_ceiling = classic_work + fill_operations / 2;
            for maximum in [generation_ceiling, required_work - 1] {
                let control = OperationControl::new();
                let bounded = policy
                    .with_limit(ResourceLimitId::MaxLayoutWorkUnits, maximum)
                    .unwrap();
                let error = prepare(bounded, control.clone())
                    .render_svg(&svg_options, &debug)
                    .err()
                    .expect("Rough generation must retain the actual work ceiling");
                let merman_render::Error::ResourceLimitExceeded(error) = error else {
                    panic!("expected geometry work rejection, got {error}");
                };
                assert_eq!(error.limit, ResourceLimitId::MaxLayoutWorkUnits.as_str());
                assert_eq!(error.max, maximum);
                assert!(error.actual > maximum);
                assert_eq!(error.cause, ResourceLimitCause::Ceiling);
                assert_eq!(error.phase, ResourceLimitPhase::LayoutModel);
                let terminal = control
                    .terminal_checkpoint_at(OperationPhase::Emit)
                    .expect_err("the work rejection must remain terminal");
                let OperationLedgerError::ResourceLimitExceeded(terminal) = terminal else {
                    panic!("expected the original work ceiling terminal");
                };
                assert_eq!(terminal.id, ResourceLimitId::MaxLayoutWorkUnits.as_str());
                assert_eq!(terminal.limit, maximum as u64);
                assert!(terminal.consumed <= maximum as u64);
                assert!(terminal.consumed + terminal.requested > maximum as u64);
                if maximum == generation_ceiling {
                    assert_eq!(terminal.phase, OperationPhase::Emit);
                }
            }
        }
    }
}

#[test]
fn hand_drawn_circles_with_no_stroke_omit_outline_without_changing_fill_seed_order() {
    let config = common::presentation_config(json!({
        "look": "handDrawn", "handDrawnSeed": 1, "htmlLabels": false,
    }));
    let source = "flowchart LR\nA((a))\nB(((a)))\n";
    let (_, visible, _) = render_source(source, config.clone());
    let (_, hidden, _) = render_source(
        &format!("{source}style A stroke:none\nstyle B stroke:none\n"),
        config,
    );
    let visible = roxmltree::Document::parse(&visible).unwrap();
    let hidden = roxmltree::Document::parse(&hidden).unwrap();
    for id in ["A", "B"] {
        let visible_shape = shape(&visible, id);
        let hidden_shape = shape(&hidden, id);
        let visible_paths: Vec<_> = visible_shape
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .collect();
        let hidden_paths: Vec<_> = hidden_shape
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .collect();
        assert_eq!(visible_paths.len(), hidden_paths.len() * 2);
        for (ring, hidden_fill) in visible_paths.chunks_exact(2).zip(hidden_paths) {
            assert_eq!(hidden_fill.attribute("d"), ring[0].attribute("d"));
            assert_eq!(hidden_fill.attribute("stroke-width"), Some("1.5"));
            assert_ne!(hidden_fill.attribute("stroke"), Some("none"));
        }
    }
}

#[allow(dead_code)]
mod common;
