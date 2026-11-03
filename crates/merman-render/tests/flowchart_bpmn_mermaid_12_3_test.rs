#![cfg(feature = "diagram-flowchart")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::model::FlowchartLayout;
use merman_render::resources::{RenderResourcePolicy, ResourceLimitId};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::{LayoutOptions, family};
use serde_json::json;

mod common;

const SHAPES: [&str; 9] = [
    "bpmn-start",
    "bpmn-intermediate",
    "bpmn-boundary",
    "bpmn-end",
    "bpmn-gateway",
    "bpmn-activity",
    "bpmn-data",
    "bpmn-data-store",
    "bpmn-annotation",
];

fn source(label: &str) -> String {
    let mut source = String::from("flowchart LR\n");
    for (index, shape) in SHAPES.iter().enumerate() {
        source.push_str(&format!(
            "N{index}@{{ shape: {shape}, label: \"{label}\" }}\n"
        ));
    }
    source.push_str("N0 --> N1 --> N2 --> N3 --> N4 --> N5 --> N6 --> N7 --> N8\n");
    source
}

fn render(
    input: &str,
    layout: &str,
    look: &str,
    html: bool,
    policy: RenderResourcePolicy,
) -> merman_render::Result<(FlowchartLayout, String)> {
    let config = common::presentation_config(json!({
        "layout": layout, "look": look, "htmlLabels": html, "handDrawnSeed": 1,
        "flowchart": {"wrappingWidth": 240},
    }));
    let parsed = MermaidEngine::new()
        .with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_config(
                config.as_config().clone(),
            ),
        )
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("parse shared BPMN shape syntax")
        .expect("detect flowchart")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .unwrap();
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)?;
    let layout =
        serde_json::from_value(artifact.layout_json()?["layout"]["FlowchartV2"].clone()).unwrap();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("bpmn-12-3".to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )?
        .svg()
        .to_owned();
    Ok((layout, svg))
}

fn class_contains(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|part| part == class))
}

#[test]
fn bpmn_shared_shapes_reach_layout_svg_and_editor_in_every_existing_look() {
    let names: Vec<_> =
        merman_model::families::flowchart::shapes::public_pinned_shape_names().collect();
    assert!(SHAPES.iter().all(|shape| names.contains(shape)));
    let layouts: &[&str] = if cfg!(feature = "layout-elk") {
        &["dagre", "elk"]
    } else {
        &["dagre"]
    };
    for layout in layouts {
        for look in ["classic", "neo", "handDrawn"] {
            for html in [false, true] {
                let (layout, svg) = render(
                    &source("Step"),
                    layout,
                    look,
                    html,
                    RenderResourcePolicy::unbounded_for_trusted_input(),
                )
                .unwrap();
                assert_eq!(layout.nodes.len(), 9);
                let document = roxmltree::Document::parse(&svg).unwrap();
                for (index, shape) in SHAPES.iter().enumerate() {
                    let id = format!("N{index}");
                    let group = document
                        .descendants()
                        .find(|node| {
                            node.attribute("data-et") == Some("node")
                                && node.attribute("data-id") == Some(id.as_str())
                        })
                        .unwrap();
                    assert!(class_contains(
                        group,
                        if look == "handDrawn" {
                            "rough-node"
                        } else {
                            "node"
                        }
                    ));
                    let reserved = group
                        .children()
                        .find(|node| class_contains(*node, "bpmn-bounds"));
                    assert_eq!(reserved.is_some(), *shape != "bpmn-activity");
                    if let Some(bounds) = reserved {
                        assert_eq!(bounds.attribute("opacity"), Some("0"));
                        assert_eq!(bounds.attribute("aria-hidden"), Some("true"));
                        assert_eq!(
                            group.children().find(|node| node.is_element()),
                            Some(bounds)
                        );
                    }
                    if index < 4 {
                        let rings: Vec<_> = group
                            .descendants()
                            .filter(|node| class_contains(*node, "bpmn-event-ring"))
                            .collect();
                        assert_eq!(rings.len(), if index == 1 || index == 2 { 2 } else { 1 });
                        assert_eq!(rings[0].attribute("r"), Some("18"));
                        assert_eq!(rings[0].attribute("stroke-width"), None);
                        assert_eq!(
                            rings[0].attribute("style"),
                            Some(match index {
                                0 => "stroke-width: 1.6px;",
                                3 => "stroke-width: 3.4px;",
                                _ => "stroke-width: 1.4px;",
                            })
                        );
                    }
                    assert!(
                        !group
                            .descendants()
                            .any(|node| class_contains(node, "bpmn-glyph"))
                    );
                    assert!(
                        group
                            .descendants()
                            .any(|node| class_contains(node, "label"))
                    );
                }
                let activity = layout.nodes.iter().find(|node| node.id == "N5").unwrap();
                assert!(activity.width >= 100.0 && activity.height >= 80.0);
                assert!(document.descendants().any(|node| class_contains(
                    node,
                    "bpmn-gateway-diamond"
                ) && node.attribute("points")
                    == Some("0,-25 25,0 0,25 -25,0")));
                assert_eq!(
                    document
                        .descendants()
                        .filter(|node| class_contains(*node, "bpmn-store-rings"))
                        .count(),
                    3
                );
                assert!(!svg.contains("openArrowEnd"));
            }
        }
    }
}

#[test]
fn bpmn_captions_reserve_space_without_resizing_the_notation_and_annotation_wraps_at_160() {
    let (short, _) = render(
        &source("Go"),
        "dagre",
        "classic",
        true,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .unwrap();
    let (long, svg) = render(
        &source("A deliberately long caption which wraps across more than one line"),
        "dagre",
        "classic",
        true,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .unwrap();
    let short_start = short.nodes.iter().find(|node| node.id == "N0").unwrap();
    let long_start = long.nodes.iter().find(|node| node.id == "N0").unwrap();
    assert!(long_start.width > short_start.width);
    assert!(long_start.height >= short_start.height);
    let document = roxmltree::Document::parse(&svg).unwrap();
    let annotation = document
        .descendants()
        .find(|node| {
            node.attribute("data-et") == Some("node") && node.attribute("data-id") == Some("N8")
        })
        .unwrap();
    let div = annotation
        .descendants()
        .find(|node| node.has_tag_name("div"))
        .unwrap();
    assert!(div.attribute("style").unwrap().contains("max-width: 160px"));
    assert!(
        document
            .descendants()
            .filter(|node| class_contains(*node, "bpmn-event-ring"))
            .all(|ring| matches!(ring.attribute("r"), Some("18" | "15")))
    );
}

#[test]
fn bpmn_edge_endpoints_intersect_the_notation_instead_of_the_caption_bounds() {
    use base64::Engine as _;
    use merman_render::model::{LayoutNode, LayoutPoint};

    fn assert_endpoint(algorithm: &str, shape: &str, node: &LayoutNode, point: &LayoutPoint) {
        let delta_x = point.x - node.x;
        let delta_y = point.y - node.y;
        let body_half_width = match shape {
            "bpmn-start" | "bpmn-intermediate" | "bpmn-boundary" | "bpmn-end" | "bpmn-data" => 18.0,
            "bpmn-gateway" | "bpmn-data-store" => 25.0,
            "bpmn-activity" | "bpmn-annotation" => node.width / 2.0,
            _ => unreachable!(),
        };
        let expected_distance = if algorithm == "elk" {
            let outline_ray_steps = 20;
            let quantum = (node.width / 2.0) / f64::from(1_u32 << outline_ray_steps);
            ((body_half_width + 1e-9) / quantum).floor() * quantum
        } else {
            body_half_width
        };
        assert!(
            (delta_x.abs() - expected_distance).abs() < 1e-6 && delta_y.abs() < 1e-6,
            "{algorithm} {shape}: {point:?}, center: ({}, {}), expected distance: {expected_distance}",
            node.x,
            node.y,
        );
        if shape == "bpmn-annotation" {
            assert!(delta_x < 0.0, "{algorithm} {shape}: {point:?}");
        }
    }

    let layouts: &[&str] = if cfg!(feature = "layout-elk") {
        &["dagre", "elk"]
    } else {
        &["dagre"]
    };
    for algorithm in layouts {
        let (layout, svg) = render(
            &source("A long caption reserves more width than the notation body"),
            algorithm,
            "classic",
            false,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let document = roxmltree::Document::parse(&svg).unwrap();
        for edge in &layout.edges {
            let path = document
                .descendants()
                .find(|node| {
                    node.attribute("data-edge") == Some("true")
                        && node.attribute("data-id") == Some(edge.id.as_str())
                })
                .unwrap();
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(path.attribute("data-points").unwrap())
                .unwrap();
            let points: Vec<LayoutPoint> = serde_json::from_slice(&bytes).unwrap();
            for (id, point) in [
                (&edge.from, points.first().unwrap()),
                (&edge.to, points.last().unwrap()),
            ] {
                let index = id.strip_prefix('N').unwrap().parse::<usize>().unwrap();
                let node = layout.nodes.iter().find(|node| node.id == *id).unwrap();
                assert_endpoint(algorithm, SHAPES[index], node, point);
            }
        }
    }
}

#[test]
fn bpmn_internal_options_do_not_create_new_flowchart_author_metadata() {
    let input = "flowchart LR\nA@{ shape: bpmn-data, dataDirection: input, isCollection: true, markers: [loop] }\nB@{ shape: bpmn-start, icon: 'bpmn:message' }\n";
    let (_, svg) = render(
        input,
        "dagre",
        "classic",
        true,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .unwrap();
    assert!(!svg.contains("bpmn-data-arrow"));
    assert!(!svg.contains("bpmn-data-collection"));
    assert!(!svg.contains("bpmn-event-body"));
    assert!(!svg.contains("bpmn-activity-markers"));
}

#[test]
fn bpmn_body_styles_and_event_ring_weights_keep_the_source_precedence_in_every_look() {
    let input = format!(
        "{}style N0 stroke-width:9px\nstyle N5 fill:#abcdef,stroke:#123456,stroke-width:4px\n",
        source("Step"),
    );
    for look in ["classic", "neo", "handDrawn"] {
        let (_, svg) = render(
            &input,
            "dagre",
            look,
            false,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let document = roxmltree::Document::parse(&svg).unwrap();
        let activity = document
            .descendants()
            .find(|node| class_contains(*node, "bpmn-activity-rect"))
            .unwrap();
        let style = activity.attribute("style").unwrap();
        assert!(style.contains("fill:#abcdef"), "{style}");
        assert!(style.contains("stroke:#123456"), "{style}");
        assert!(style.contains("stroke-width:4px"), "{style}");
        assert_eq!(activity.attribute("rx"), Some("10"));
        let start = document
            .descendants()
            .find(|node| class_contains(*node, "bpmn-event-start"))
            .unwrap();
        let body = start
            .descendants()
            .find(|node| class_contains(*node, "bpmn-event-body"))
            .unwrap();
        assert!(
            body.attribute("style")
                .unwrap()
                .contains("stroke-width:9px")
        );
        let ring = body
            .children()
            .find(|node| node.has_tag_name("circle"))
            .unwrap();
        assert_eq!(ring.attribute("style"), Some("stroke-width: 1.6px;"));
    }
}

#[test]
fn bpmn_shape_output_obeys_the_existing_exact_svg_budget() {
    let input = source("Step");
    let (_, baseline) = render(
        &input,
        "dagre",
        "classic",
        false,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    )
    .unwrap();
    let exact = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, baseline.len())
        .unwrap();
    assert_eq!(
        render(&input, "dagre", "classic", false, exact).unwrap().1,
        baseline
    );
    let below = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, baseline.len() - 1)
        .unwrap();
    assert!(matches!(
        render(&input, "dagre", "classic", false, below),
        Err(merman_render::Error::ResourceLimitExceeded(_))
    ));
}
