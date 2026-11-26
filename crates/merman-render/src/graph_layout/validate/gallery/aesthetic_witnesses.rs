use super::super::aesthetic::emit;
use super::{
    Affine, Body, CrossAxis, Document, Edge, Label, LabelOwner, PointMarker, Rect, Scene, Vec2,
    render, view_box,
};
use std::fmt::Write as _;

#[test]
fn painted_decision_branch_metrics() {
    let source = "flowchart TD\nA[Start] --> B{Condition?}\nB -->|Yes| C[Execute]\nB -->|No| D[End]\nC --> D";
    let drawing = render(source, "decision-aesthetic").unwrap();
    let doc = Document::parse(&drawing.svg).unwrap();
    let scene = drawing.scene(&doc, view_box(doc.root_element()).size());
    let body_names: Vec<_> = drawing
        .layout
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect();
    let edge_names: Vec<_> = drawing
        .layout
        .edges
        .iter()
        .map(|edge| edge.id.as_str())
        .collect();
    let metrics = emit(
        "decision-bypass",
        "TB",
        &scene,
        &body_names,
        &edge_names,
        &[],
    );
    let measured: Vec<_> = metrics
        .label_optical_balance
        .iter()
        .map(|label| label.edge)
        .collect();
    let decision = drawing.node("B");
    let branches: Vec<_> = scene
        .edges
        .iter()
        .zip(edge_names.iter().copied())
        .filter_map(|(edge, name)| (edge.start.body == decision).then_some(name))
        .collect();
    assert_eq!(branches.len(), 2);
    assert_eq!(measured, branches);
}

#[test]
fn offset_s_controls_are_measured_in_path_order() {
    let scene = Scene {
        edges: vec![
            // The short branch from the original Condition? SVG before the monotone fix.
            Edge::new(
                0,
                1,
                "M119.805,254.2 C111.6122,277.608 76,272.592 76,296 L76,338",
                CrossAxis::X,
            ),
            Edge::new(0, 1, "M0,0 C0,8 6,10 14,10", CrossAxis::X),
            Edge::new(0, 1, "M60,0 C30,0 30,20 0,20", CrossAxis::Y),
        ],
        ..Scene::default()
    };
    let metrics = emit(
        "offset-controls",
        "mixed",
        &scene,
        &[] as &[&str],
        &["short-branch", "rounded-corner", "reverse-sweep"],
        &[],
    );
    assert_eq!(metrics.cubic_count, 3);
    let [short, reverse] = metrics.s_curves.as_slice() else {
        panic!("expected two offset sweeps, got {:?}", metrics.s_curves);
    };
    assert_eq!(short.edge, "short-branch");
    assert!(!short.ordered_controls);
    close(short.handle_ratio, 0.56);
    assert_eq!(reverse.edge, "reverse-sweep");
    assert!(reverse.ordered_controls);
    close(reverse.handle_ratio, 0.50);
    close(metrics.max_s_curve_handle_ratio.unwrap(), 0.56);
}

#[test]
fn label_balance_uses_both_label_boundaries_and_the_marker_root() {
    let mut edge = Edge::new(0, 1, "M0,0 C0,5 0,17 0,70 L20,70", CrossAxis::X);
    edge.transform = Affine::translate((10., 10.));
    edge.start.setback = 4.;
    edge.end.marker = Some(PointMarker {
        depth: 9.,
        half_width: 5.,
    });
    let mut scene = Scene {
        edges: vec![edge],
        labels: vec![Label {
            owner: LabelOwner::Edge(0),
            bounds: Rect::new(7., 31., 13., 51.),
        }],
        ..Scene::default()
    };
    let measure = |scene: &Scene<'_>| {
        emit(
            "label-balance",
            "TB",
            scene,
            &[] as &[&str],
            &["labelled"],
            &[],
        )
    };
    let metrics = measure(&scene);
    let [before, after] = metrics.label_optical_balance[0].clear_shaft_px.unwrap();
    close(before, 25.);
    close(after, 40.);
    close(metrics.label_optical_balance_ratio.unwrap(), 1.6);

    scene.labels[0].bounds = Rect::new(11., 31., 17., 51.);
    let metrics = measure(&scene);
    assert_eq!(metrics.label_optical_balance[0].clear_shaft_px, None);
    assert_eq!(metrics.label_optical_balance_ratio, None);
}

#[test]
fn sibling_metrics_compare_turns_at_both_ends() {
    let mut edges = vec![
        Edge::new(
            0,
            1,
            "M-8,0 L-8,8 C-8,17 -26,19 -26,28 L-26,60",
            CrossAxis::X,
        ),
        Edge::new(0, 2, "M8,4 L8,12 C12,20 26,32 26,42 L26,60", CrossAxis::X),
        Edge::new(
            1,
            3,
            "M-26,70 L-26,99 C-26,104 -8,110 -8,115 L-8,120",
            CrossAxis::X,
        ),
        Edge::new(
            2,
            3,
            "M26,70 L26,99 C26,104 8,110 8,115 L8,120",
            CrossAxis::X,
        ),
    ];
    edges[1].start.setback = 4.;
    for edge in &mut edges[..2] {
        edge.start.outward_normal = Some(Vec2::new(0., 1.));
    }
    for edge in &mut edges[2..] {
        edge.end.outward_normal = Some(Vec2::new(0., -1.));
    }
    let scene = Scene {
        bodies: [
            Rect::new(-20., -10., 20., 0.),
            Rect::new(-30., 60., -22., 70.),
            Rect::new(22., 60., 30., 70.),
            Rect::new(-20., 120., 20., 130.),
        ]
        .into_iter()
        .map(Body::node)
        .collect(),
        edges,
        ..Scene::default()
    };
    let metrics = emit(
        "fork-and-join",
        "TB",
        &scene,
        &["fork", "left", "right", "join"],
        &["left-out", "right-out", "left-in", "right-in"],
        &[],
    );
    let [fork, join] = metrics.face_turn_coherence.as_slice() else {
        panic!(
            "expected two turning faces, got {:?}",
            metrics.face_turn_coherence
        );
    };
    assert_eq!(fork.body, "fork");
    close(fork.max_adjacent_delta_px, 4.);
    assert_eq!(join.body, "join");
    close(join.max_adjacent_delta_px, 0.);

    let [fork, join] = metrics.sibling_departure_symmetry.as_slice() else {
        panic!(
            "expected one sibling split and one join, got {:?}",
            metrics.sibling_departure_symmetry
        );
    };
    assert_eq!((fork.body, fork.end), ("fork", "start"));
    close(fork.turn_start_delta_px, 4.);
    close(fork.landing_horizon_delta_px, 14.);
    close(fork.turn_span_ratio.unwrap(), 1.5);
    close(fork.mirror_tangent_error_deg, 0.5_f64.atan().to_degrees());
    assert_eq!((join.body, join.end), ("join", "end"));
    close(join.turn_start_delta_px, 0.);
    close(join.landing_horizon_delta_px, 0.);
    close(join.turn_span_ratio.unwrap(), 1.);
    close(join.mirror_tangent_error_deg, 0.);
}

#[test]
fn large_curved_fans_keep_one_cubic_per_edge_on_average() {
    for nodes in [250, 500] {
        for fan_in in [true, false] {
            let name = if fan_in {
                format!("fan-in-{nodes}")
            } else {
                format!("fan-out-{nodes}")
            };
            let mut source = String::from("flowchart TD\n");
            for node in 1..nodes {
                let (from, to) = if fan_in { (node, 0) } else { (0, node) };
                writeln!(source, "N{from} --> N{to}").unwrap();
            }
            let drawing = render(&source, &name).unwrap_or_else(|error| panic!("{name}: {error}"));
            let doc = Document::parse(&drawing.svg).unwrap();
            let scene = drawing.scene(&doc, view_box(doc.root_element()).size());
            assert_eq!(scene.edges.len(), nodes - 1, "{name}");
            let body_names: Vec<_> = drawing
                .layout
                .nodes
                .iter()
                .map(|node| node.id.as_str())
                .collect();
            let edge_names: Vec<_> = drawing
                .layout
                .edges
                .iter()
                .map(|edge| edge.id.as_str())
                .collect();
            let metrics = emit(&name, "TB", &scene, &body_names, &edge_names, &[]);
            let ratio = metrics.large_fan_cubic_ratio.expect("painted fan");
            assert!(
                ratio <= 1.05,
                "{name}: {} cubics / {} edges = {ratio}",
                metrics.cubic_count,
                scene.edges.len()
            );
        }
    }
}

fn close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-6, "{actual} != {expected}");
}
