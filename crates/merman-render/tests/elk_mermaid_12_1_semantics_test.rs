#![cfg(feature = "layout-elk")]

//! Call-site regressions for Mermaid 12.1's shared ELK edge projection.
//! Sources: render.ts and subgraphFeedbackEdges.ts at
//! 21f72f07ea22c0af48a3149c550654e80d8e40cb.

use base64::Engine as _;
use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::{Value, json};

#[derive(Debug, PartialEq)]
struct Label {
    id: String,
    center: (f64, f64),
    width: f64,
    height: f64,
}

#[derive(Debug, PartialEq)]
struct Edge {
    id: String,
    points: Vec<(f64, f64)>,
    start_marker: Option<String>,
    end_marker: Option<String>,
}

#[derive(Debug, PartialEq)]
struct Rendered {
    layout: Value,
    labels: Vec<Label>,
    edges: Vec<Edge>,
}

fn has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_whitespace().any(|value| value == class))
}

fn translation(value: &str) -> (f64, f64) {
    let numbers: Vec<f64> = value
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(')'))
        .expect("label uses a translate transform")
        .split([',', ' '])
        .filter(|value| !value.is_empty())
        .map(|value| value.parse().expect("finite coordinate"))
        .collect();
    assert_eq!(numbers.len(), 2);
    assert!(numbers.iter().all(|number| number.is_finite()));
    (numbers[0], numbers[1])
}

fn render(source: &str, algorithm: &str, elk_options: Value) -> Rendered {
    let parsed = Engine::new()
        .with_site_config(MermaidConfig::from_value(json!({
            "layout": algorithm, "htmlLabels": false, "look": "classic",
            "elk": elk_options,
        })))
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse real family fixture")
        .expect("detect diagram");
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::headless_svg_defaults(),
        RenderEnvironment::deterministic().begin_session().unwrap(),
    )
    .expect("prepare real family fixture");
    let projection = artifact.layout_json().expect("layout JSON");
    let layouts = projection["layout"].as_object().expect("tagged layout");
    assert_eq!(layouts.len(), 1);
    let layout = layouts.values().next().unwrap().clone();
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("elk-12-1".to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render real family fixture");
    let document = roxmltree::Document::parse(svg.svg()).expect("valid SVG");
    let labels = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && has_class(*node, "edgeLabel")
                && node.attribute("transform").is_some()
                && node
                    .ancestors()
                    .any(|parent| has_class(parent, "edgeLabels"))
        })
        .map(|node| {
            let inner = node
                .children()
                .find(|child| has_class(*child, "label") && child.attribute("data-id").is_some())
                .expect("main label identity");
            let offset = translation(inner.attribute("transform").expect("label centering"));
            // Requirement's SVG text is horizontally centered inside the label group;
            // its x translation is zero. Read the emitted measured box when available.
            let dimensions = inner
                .descendants()
                .find(|child| {
                    child.has_tag_name("foreignObject")
                        || (child.has_tag_name("rect") && has_class(*child, "background"))
                })
                .map(|bbox| {
                    (
                        bbox.attribute("width").unwrap().parse::<f64>().unwrap(),
                        bbox.attribute("height").unwrap().parse::<f64>().unwrap(),
                    )
                })
                .unwrap_or((-2.0 * offset.0, -2.0 * offset.1));
            Label {
                id: inner.attribute("data-id").unwrap().to_owned(),
                center: translation(node.attribute("transform").unwrap()),
                width: dimensions.0,
                height: dimensions.1,
            }
        })
        .collect();
    let edges = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .map(|node| {
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(node.attribute("data-points").expect("paint path points"))
                .expect("base64 points");
            let points: Vec<Value> = serde_json::from_slice(&decoded).expect("point JSON");
            Edge {
                id: node.attribute("data-id").expect("edge identity").to_owned(),
                points: points.iter().map(point).collect(),
                start_marker: node.attribute("marker-start").map(str::to_owned),
                end_marker: node.attribute("marker-end").map(str::to_owned),
            }
        })
        .collect();
    Rendered {
        layout,
        labels,
        edges,
    }
}

fn point(value: &Value) -> (f64, f64) {
    (value["x"].as_f64().unwrap(), value["y"].as_f64().unwrap())
}

fn semantic_edges(layout: &Value) -> Vec<Value> {
    layout["edges"]
        .as_array()
        .expect("layout edges")
        .iter()
        .map(|edge| json!([edge["id"], edge["from"], edge["to"]]))
        .collect()
}

fn provider_points(layout: &Value) -> Vec<Value> {
    layout["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| edge["points"].clone())
        .collect()
}

fn assert_pair_labels(source: &str) {
    // Box deliberately has no provider sections. Both labels therefore start at the same
    // fallback midpoint, making the source's pair correction observable without layout goldens.
    let disabled = render(
        source,
        "elk.box",
        json!({"straightenEdges": false, "lineHops": false}),
    );
    let enabled = render(
        source,
        "elk.box",
        json!({"straightenEdges": true, "lineHops": false}),
    );
    assert_eq!(
        disabled, enabled,
        "pair correction is independent of terminal straightening"
    );
    assert_eq!(
        disabled.edges.len(),
        2,
        "both declarations survive preparation"
    );
    assert_eq!(disabled.labels.len(), 2, "both semantic labels are painted");
    assert!(
        provider_points(&disabled.layout)
            .iter()
            .all(|points| points == &json!([])),
        "fallback paint routes must not replace raw provider sections"
    );
    let [a, b] = disabled.labels.as_slice() else {
        unreachable!()
    };
    assert!(a.width > 0.0 && b.width > 0.0);
    assert!((a.center.1 - b.center.1).abs() < 0.001);
    // OPPOSITE_LABEL_MIN_GAP in pinned render.ts is 4; it applies to exactly two labels
    // sharing an unordered endpoint pair, including two same-direction declarations.
    let gap = (b.center.0 - a.center.0).abs() - a.width / 2.0 - b.width / 2.0;
    assert!(
        (gap - 4.0).abs() < 0.003,
        "source's four-pixel label gap: {disabled:#?}"
    );
    let route = &disabled.edges[0].points;
    let first = route.first().expect("paint fallback source");
    let last = route.last().expect("paint fallback target");
    assert!(
        ((a.center.0 + b.center.0) / 2.0 - (first.0 + last.0) / 2.0).abs() < 0.003,
        "symmetric separation preserves the fallback midpoint"
    );
}

#[cfg(feature = "diagram-state")]
#[test]
fn state_two_label_pairs_are_separated_even_when_straightening_is_disabled() {
    for second in ["A --> B: beta", "B --> A: beta"] {
        assert_pair_labels(&format!("stateDiagram-v2\nA --> B: alpha\n{second}\n"));
    }
}

#[cfg(feature = "diagram-requirement")]
#[test]
fn requirement_two_label_pairs_are_separated_even_when_straightening_is_disabled() {
    for second in ["A - refines -> B", "B - refines -> A"] {
        assert_pair_labels(&format!(
            "requirementDiagram\nelement A {{\ntype: part\n}}\nelement B {{\ntype: part\n}}\nA - satisfies -> B\n{second}\n"
        ));
    }
}

#[cfg(feature = "diagram-usecase")]
#[test]
fn usecase_two_label_pairs_are_separated_even_when_straightening_is_disabled() {
    for second in ["A -- beta --> B", "B -- beta --> A"] {
        assert_pair_labels(&format!("usecase-beta\nA -- alpha --> B\n{second}\n"));
    }
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_two_label_pairs_are_separated_even_when_straightening_is_disabled() {
    for second in ["A ||--|| B : beta", "B ||--|| A : beta"] {
        assert_pair_labels(&format!("erDiagram\nA ||--|| B : alpha\n{second}\n"));
    }
}

fn assert_semantic_route_endpoints(layout: &Value) {
    let nodes = layout["nodes"].as_array().unwrap();
    for edge in layout["edges"].as_array().unwrap() {
        let points = edge["points"].as_array().unwrap();
        assert!(!points.is_empty(), "layered provider emitted a route");
        for (id, endpoint) in [
            (&edge["from"], &points[0]),
            (&edge["to"], points.last().unwrap()),
        ] {
            let node = nodes
                .iter()
                .find(|node| &node["id"] == id)
                .expect("semantic endpoint node");
            let (x, y) = point(endpoint);
            let (cx, cy) = point(node);
            let (half_width, half_height) = (
                node["width"].as_f64().unwrap() / 2.0,
                node["height"].as_f64().unwrap() / 2.0,
            );
            let (dx, dy) = ((x - cx).abs(), (y - cy).abs());
            assert!(
                dx <= half_width + 1e-6
                    && dy <= half_height + 1e-6
                    && ((dx - half_width).abs() < 1e-6 || (dy - half_height).abs() < 1e-6),
                "restored path endpoint belongs to its original semantic node: {edge:#?} / {node:#?}"
            );
        }
    }
}

fn assert_compound_feedback(source: &str) {
    let default = render(
        source,
        "elk",
        json!({"straightenEdges": false, "lineHops": false}),
    );
    let enabled = render(
        source,
        "elk",
        json!({"orientFeedbackEdges": true, "straightenEdges": false, "lineHops": false}),
    );
    let disabled = render(
        source,
        "elk",
        json!({"orientFeedbackEdges": false, "straightenEdges": false, "lineHops": false}),
    );
    assert_eq!(
        default, enabled,
        "compound feedback orientation defaults on"
    );
    assert_eq!(
        semantic_edges(&enabled.layout),
        semantic_edges(&disabled.layout),
        "temporary provider reversal must not leak into semantic source/target"
    );
    assert_ne!(
        provider_points(&enabled.layout),
        provider_points(&disabled.layout),
        "the collapsed-subgraph feedback fixture must exercise the option"
    );
    for rendered in [&enabled, &disabled] {
        assert_semantic_route_endpoints(&rendered.layout);
        assert_eq!(rendered.edges.len(), 3);
        assert!(rendered.edges.iter().all(|edge| edge.end_marker.is_some()));
    }
    let markers = |rendered: &Rendered| {
        rendered
            .edges
            .iter()
            .map(|edge| {
                (
                    edge.id.clone(),
                    edge.start_marker.clone(),
                    edge.end_marker.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        markers(&enabled),
        markers(&disabled),
        "semantic markers do not reverse"
    );
}

#[cfg(feature = "diagram-state")]
#[test]
fn state_feedback_restores_semantic_endpoints_across_nested_containers() {
    // State dataFetcher assigns the current non-root parent on every reference. Declare
    // the inner group last so its members retain Group, rather than Outer, as their parent.
    assert_compound_feedback(
        "stateDiagram-v2\nstate Outer {\nB --> X: outgoing\nX --> A: feedback\nstate Group {\nA --> B\n}\n}\n",
    );
}

#[cfg(feature = "diagram-usecase")]
#[test]
fn usecase_feedback_restores_semantic_endpoints_across_a_system_boundary() {
    // Pinned usecase.parser.spec.ts rejects nested boundaries. Exercise the supported
    // compound boundary here; State and ER cover feedback at a nested hierarchy level.
    assert_compound_feedback(
        "usecase-beta\nsystemBoundary Group(Group)\nA(Alpha)\nB(Beta)\nend\nX(External)\nA --> B\nB -- outgoing --> X\nX -- feedback --> A\n",
    );
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_feedback_restores_semantic_endpoints_across_nested_containers() {
    assert_compound_feedback(
        "erDiagram\nsubgraph Outer [Outer]\nsubgraph Group [Group]\nA\nB\nend\nX\nend\nA ||--|| B : internal\nB ||--|| X : outgoing\nX ||--|| A : feedback\n",
    );
}

#[cfg(feature = "diagram-requirement")]
#[test]
fn requirement_plain_cycle_is_unchanged_by_compound_feedback_orientation() {
    // Requirement has no subgraph syntax. Its plain cycle is explicitly outside the new
    // orientation policy and remains the provider's cycle-breaking responsibility.
    let source = "requirementDiagram\nelement A {\ntype: part\n}\nelement B {\ntype: part\n}\nelement C {\ntype: part\n}\nA - satisfies -> B\nB - refines -> C\nC - traces -> A\n";
    let enabled = render(
        source,
        "elk",
        json!({"orientFeedbackEdges": true, "lineHops": false}),
    );
    let disabled = render(
        source,
        "elk",
        json!({"orientFeedbackEdges": false, "lineHops": false}),
    );
    assert_eq!(enabled, disabled);
    assert_eq!(enabled.edges.len(), 3);
}

fn distance_to_route(point: (f64, f64), route: &[(f64, f64)]) -> f64 {
    route
        .windows(2)
        .map(|ends| {
            let (dx, dy) = (ends[1].0 - ends[0].0, ends[1].1 - ends[0].1);
            let length = dx * dx + dy * dy;
            let t = if length == 0.0 {
                0.0
            } else {
                (((point.0 - ends[0].0) * dx + (point.1 - ends[0].1) * dy) / length).clamp(0.0, 1.0)
            };
            (point.0 - ends[0].0 - t * dx).hypot(point.1 - ends[0].1 - t * dy)
        })
        .fold(f64::INFINITY, f64::min)
}

fn assert_labels_follow_straightened_routes(sources: &[String]) {
    let mut moved_labels = 0;
    let mut changed_routes = 0;
    for source in sources {
        let original = render(
            source,
            "elk",
            json!({"straightenEdges": false, "lineHops": false}),
        );
        let straight = render(
            source,
            "elk",
            json!({"straightenEdges": true, "lineHops": false}),
        );
        assert_eq!(original.layout["nodes"], straight.layout["nodes"]);
        assert_eq!(
            provider_points(&original.layout),
            provider_points(&straight.layout),
            "straightening projects paint routes without rewriting provider sections"
        );
        for after in &straight.edges {
            let before = original
                .edges
                .iter()
                .find(|edge| edge.id == after.id)
                .unwrap();
            assert_eq!(
                after.points.first(),
                before.points.first(),
                "source port stays fixed"
            );
            assert_eq!(
                after.points.last(),
                before.points.last(),
                "target port stays fixed"
            );
            if after.points == before.points {
                continue;
            }
            changed_routes += 1;
            let Some(label) = straight.labels.iter().find(|label| label.id == after.id) else {
                continue;
            };
            let old_label = original
                .labels
                .iter()
                .find(|label| label.id == after.id)
                .unwrap();
            if (label.center.0 - old_label.center.0).hypot(label.center.1 - old_label.center.1)
                < 0.001
            {
                continue;
            }
            moved_labels += 1;
            // The pinned integration regression (#8292) checks distance to the actual route.
            // This checks the family call site, not a duplicate of the moved-run algorithm.
            assert!(
                distance_to_route(label.center, &after.points) < 0.003,
                "a moved main label follows its rendered channel: {label:?} / {after:?}"
            );
        }
    }
    assert!(
        changed_routes > 0,
        "fixture must exercise terminal straightening"
    );
    assert!(
        moved_labels > 0,
        "fixture must exercise main-label reprojection"
    );
}

const RELATIONS: [(&str, &str, &str); 8] = [
    ("A", "B", "calls"),
    ("A", "C", "reads"),
    ("A", "D", "writes"),
    ("B", "D", "caches"),
    ("C", "D", "depends"),
    ("D", "E", "emits"),
    ("E", "A", "feedback"),
    ("B", "C", "shares"),
];

#[cfg(feature = "diagram-state")]
#[test]
fn state_main_labels_follow_their_straightened_paint_routes() {
    let sources: Vec<_> = ["LR", "TB"]
        .into_iter()
        .map(|direction| {
            let mut source = format!("stateDiagram-v2\ndirection {direction}\n");
            for (from, to, label) in RELATIONS {
                source.push_str(&format!("{from} --> {to}: {label}\n"));
            }
            source
        })
        .collect();
    assert_labels_follow_straightened_routes(&sources);
}

#[cfg(feature = "diagram-usecase")]
#[test]
fn usecase_main_labels_follow_their_straightened_paint_routes() {
    let sources: Vec<_> = ["LR", "TB"]
        .into_iter()
        .map(|direction| {
            let mut source = format!("usecase-beta\ndirection {direction}\n");
            for (from, to, label) in RELATIONS {
                source.push_str(&format!("{from} -- {label} --> {to}\n"));
            }
            source
        })
        .collect();
    assert_labels_follow_straightened_routes(&sources);
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_main_labels_follow_their_straightened_paint_routes() {
    let sources: Vec<_> = ["LR", "TB"]
        .into_iter()
        .map(|direction| {
            let mut source = format!("erDiagram\ndirection {direction}\n");
            for (from, to, label) in RELATIONS {
                source.push_str(&format!("{from} ||--|| {to} : {label}\n"));
            }
            source
        })
        .collect();
    assert_labels_follow_straightened_routes(&sources);
}

#[cfg(feature = "diagram-requirement")]
#[test]
fn requirement_main_labels_follow_their_straightened_paint_routes() {
    let sources: Vec<_> = ["LR", "TB"]
        .into_iter()
        .map(|direction| {
            let mut source = format!("requirementDiagram\ndirection {direction}\n");
            for id in ["A", "B", "C", "D", "E"] {
                source.push_str(&format!("element {id} {{\ntype: part\n}}\n"));
            }
            for (index, (from, to, _)) in RELATIONS.into_iter().enumerate() {
                let label = ["satisfies", "refines", "traces"][index % 3];
                source.push_str(&format!("{from} - {label} -> {to}\n"));
            }
            source
        })
        .collect();
    assert_labels_follow_straightened_routes(&sources);
}
