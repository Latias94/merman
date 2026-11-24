//! Native API timings on one frozen Flowchart measurement per topology. Preparation, projection,
//! output inspection and destruction run outside the clock. Each JSONL sample covers one call.

use super::{Endpoint, Marker, MeasuredGraph, Size, project_input, shoal};
use crate::environment::RenderEnvironment;
use crate::graph_layout::ir::Direction;
use crate::layout_work::OperationLayoutWorkControl;
use crate::resources::RenderResourcePolicy;
use crate::{LayoutOptions, family};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, NodeLabel, RankDir};
use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_layout_elk as elk;
use merman_model::presentation::RenderSemanticModel;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::collections::HashSet;
use std::fs;
use std::hint::black_box;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::Instant;

const CASES: [&str; 7] = [
    "scc-75",
    "scc-75-declarations-2",
    "scc-100",
    "scc-100-edges-reverse",
    "chain-500",
    "fan-in-500",
    "fan-out-500",
];
const MEASURED_ROUNDS: usize = 12;
const SCRATCH_CAP: usize = 2 * 1024 * 1024;

thread_local! {
    static REQUESTED: Cell<bool> = const { Cell::new(false) };
    static CAPTURED: Cell<Option<Box<(MeasuredGraph, shoal::Options)>>> = const { Cell::new(None) };
}

pub(super) fn capture(graph: &MeasuredGraph, options: shoal::Options) {
    if REQUESTED.get() {
        let previous = CAPTURED.replace(Some(Box::new((graph.clone(), options))));
        assert!(
            previous.is_none(),
            "expected one measured graph per topology"
        );
    }
}

#[derive(Deserialize)]
struct Plan {
    schema_version: u32,
    suite: String,
    topologies: Vec<Topology>,
}

#[derive(Deserialize)]
struct Topology {
    id: String,
    nodes: usize,
    edges: usize,
    source: String,
    source_sha256: String,
}

fn sha256(bytes: &[u8]) -> String {
    data_encoding::HEXLOWER.encode(&Sha256::digest(bytes))
}

fn node_index(endpoint: Endpoint) -> usize {
    let Endpoint::Node(index) = endpoint else {
        panic!("the native kernel corpus requires flat node endpoints")
    };
    index as usize
}

struct Frozen {
    measured: MeasuredGraph,
    shoal: shoal::Graph,
    options: shoal::Options,
    elk: elk::Graph,
}

impl Frozen {
    fn new(case: &Topology) -> Self {
        assert_eq!(
            sha256(case.source.as_bytes()),
            case.source_sha256,
            "{}",
            case.id
        );
        let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
            json!({"layout": "shoal", "htmlLabels": true}),
        ));
        let diagram = engine
            .parse_diagram_for_render_model_sync(&case.source, ParseOptions::strict())
            .unwrap()
            .expect("nonempty flowchart")
            .into_presentation();
        let RenderSemanticModel::Flowchart(model) = diagram.model() else {
            panic!("{}: expected a Flowchart model", case.id);
        };
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
            .begin_session()
            .unwrap();
        let mut work = OperationLayoutWorkControl::new(session.work_meter().clone());
        REQUESTED.set(true);
        let prepared =
            family::prepare_layout(&diagram, &LayoutOptions::headless_svg_defaults(), session);
        REQUESTED.set(false);
        drop(prepared.unwrap_or_else(|error| panic!("{}: {error}", case.id)));
        let measurement = CAPTURED
            .take()
            .expect("preparation must reach the Shoal bridge");
        let (measured, options) = *measurement;
        let counts = (case.nodes, case.edges);
        assert_eq!(
            (model.nodes.len(), model.edges.len()),
            counts,
            "{}",
            case.id
        );
        assert_eq!(
            (measured.nodes.len(), measured.edges.len()),
            counts,
            "{}",
            case.id
        );
        assert!(model.subgraphs.is_empty() && measured.groups.is_empty());
        let nodes = measured
            .nodes
            .iter()
            .zip(&model.nodes)
            .enumerate()
            .map(|(index, (node, semantic))| {
                assert_eq!(
                    node.seq as usize, index,
                    "{}: node declaration order",
                    case.id
                );
                assert!(node.parent.is_none());
                elk::Node {
                    id: semantic.id.clone(),
                    kind: elk::NodeKind::Leaf,
                    width: node.size.width,
                    height: node.size.height,
                    label_text: None,
                    container: Default::default(),
                    parent: None,
                    direction: None,
                    hierarchy_handling: None,
                    layer_constraint: None,
                    port_alignment: None,
                    label: None,
                }
            })
            .collect();
        let mut edge_ids = HashSet::new();
        let edges = measured
            .edges
            .iter()
            .zip(&model.edges)
            .map(|(edge, semantic)| {
                let source = &model.nodes[node_index(edge.source)].id;
                let target = &model.nodes[node_index(edge.target)].id;
                assert_eq!(
                    (source, target),
                    (&semantic.from, &semantic.to),
                    "{}: directed edge occurrence",
                    case.id
                );
                assert!(
                    edge_ids.insert(&semantic.id),
                    "{}: duplicate edge ID {}",
                    case.id,
                    semantic.id
                );
                assert!(edge.start_label.is_none() && edge.end_label.is_none());
                assert_eq!(edge.weight, 1.0, "ELK has no edge weight input");
                elk::Edge {
                    id: semantic.id.clone(),
                    source: source.clone(),
                    target: target.clone(),
                    label: edge.label.map(|size| elk::Label {
                        width: size.width,
                        height: size.height,
                    }),
                    terminal_labels: Vec::new(),
                    minlen: edge.min_rank_span as usize,
                    inside_self_loops_yo: false,
                }
            })
            .collect();
        let elk = elk::Graph {
            id: "root".into(),
            direction: match measured.direction {
                Direction::Down => elk::Direction::Down,
                Direction::Up => elk::Direction::Up,
                Direction::Right => elk::Direction::Right,
                Direction::Left => elk::Direction::Left,
            },
            spacing: elk::Spacing {
                node_node: measured.node_spacing,
                layer_layer: measured.rank_spacing,
                ..Default::default()
            },
            options: crate::elk_options::layout_options(&diagram.metadata().effective_config),
            nodes,
            edges,
        };
        let shoal = project_input(&measured, &Default::default(), &mut work).unwrap();
        Self {
            measured,
            shoal,
            options,
            elk,
        }
    }

    fn dagre(&self) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            directed: true,
            multigraph: true,
            compound: true,
        });
        graph.set_graph(GraphLabel {
            rankdir: match self.measured.direction {
                Direction::Down => RankDir::TB,
                Direction::Up => RankDir::BT,
                Direction::Right => RankDir::LR,
                Direction::Left => RankDir::RL,
            },
            nodesep: self.measured.node_spacing,
            ranksep: self.measured.rank_spacing,
            marginx: 8.0,
            marginy: 8.0,
            ..Default::default()
        });
        for node in &self.elk.nodes {
            graph.set_node(
                &node.id,
                NodeLabel {
                    width: node.width,
                    height: node.height,
                    ..Default::default()
                },
            );
        }
        for (edge, measured) in self.elk.edges.iter().zip(&self.measured.edges) {
            let label = measured.label.unwrap_or_default();
            graph
                .try_set_edge_named(
                    &edge.source,
                    &edge.target,
                    Some(&edge.id),
                    Some(EdgeLabel {
                        minlen: measured.min_rank_span as usize,
                        weight: f64::from(measured.weight),
                        width: label.width,
                        height: label.height,
                        labeloffset: 10.0,
                        ..Default::default()
                    }),
                )
                .unwrap();
        }
        graph
    }

    fn snapshot(&self) -> Value {
        let marker =
            |value: Marker| json!({"trim": value.trim, "depth": value.depth, "width": value.width});
        let size = |value: Size| [value.width, value.height];
        let nodes: Vec<_> = self.measured.nodes.iter().zip(&self.elk.nodes).map(|(node, native)| json!({
            "id": native.id, "seq": node.seq, "size": size(node.size), "parent": node.parent,
            "port_span": node.port_span, "perpendicular_port_span": node.perpendicular_port_span,
            "anchor_y": node.anchor_y,
        })).collect();
        let edges: Vec<_> = self.measured.edges.iter().zip(&self.elk.edges).map(|(edge, native)| json!({
            "id": native.id, "source": node_index(edge.source), "target": node_index(edge.target),
            "min_rank_span": edge.min_rank_span, "weight": edge.weight, "label": edge.label.map(size),
            "start_label": edge.start_label.map(size), "end_label": edge.end_label.map(size),
            "start_marker": marker(edge.start_marker), "end_marker": marker(edge.end_marker),
        })).collect();
        json!({
            "direction": format!("{:?}", self.measured.direction), "routing": format!("{:?}", self.measured.routing),
            "node_spacing": self.measured.node_spacing, "rank_spacing": self.measured.rank_spacing,
            "component_spacing": self.measured.component_spacing,
            "inherit_crossed_group_direction": self.measured.inherit_crossed_group_direction,
            "nodes": nodes, "edges": edges, "groups": [],
        })
    }
}

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum Backend {
    Dagre,
    Elk,
    Shoal,
}

const ORDERS: [[Backend; 3]; 6] = {
    use Backend::*;
    [
        [Dagre, Elk, Shoal],
        [Shoal, Elk, Dagre],
        [Elk, Shoal, Dagre],
        [Dagre, Shoal, Elk],
        [Shoal, Dagre, Elk],
        [Elk, Dagre, Shoal],
    ]
};

#[derive(Default, Serialize)]
struct Inventory {
    nodes: usize,
    edges: usize,
    route_points: usize,
    invalid_boxes: usize,
    invalid_routes: usize,
}

fn inspect(
    nodes: impl Iterator<Item = [f64; 4]>,
    routes: impl Iterator<Item = impl ExactSizeIterator<Item = [f64; 2]>>,
) -> Inventory {
    let mut result = Inventory::default();
    for rect in nodes {
        result.nodes += 1;
        result.invalid_boxes +=
            usize::from(!rect.into_iter().all(f64::is_finite) || rect[2] <= 0. || rect[3] <= 0.);
    }
    for mut points in routes {
        result.edges += 1;
        result.route_points += points.len();
        result.invalid_routes += usize::from(
            points.len() < 2 || !points.all(|point| point.into_iter().all(f64::is_finite)),
        );
    }
    result
}

fn timed<T>(solve: impl FnOnce() -> T) -> (u64, T) {
    let start = Instant::now();
    let result = solve();
    black_box(&result);
    (start.elapsed().as_nanos().try_into().unwrap(), result)
}

fn sample(
    backend: Backend,
    input: &Frozen,
    scratch: &mut shoal::LayoutScratch,
) -> (u64, Result<Inventory, String>) {
    match backend {
        Backend::Dagre => {
            let mut graph = input.dagre();
            let (elapsed, result) = timed(|| dugong::layout(&mut graph));
            let inventory = result.map(|()| {
                inspect(
                    graph.nodes().map(|id| {
                        let node = graph.node(id).unwrap();
                        [
                            node.x.unwrap_or(f64::NAN),
                            node.y.unwrap_or(f64::NAN),
                            node.width,
                            node.height,
                        ]
                    }),
                    graph.edges().map(|key| {
                        graph
                            .edge_by_key(key)
                            .unwrap()
                            .points
                            .iter()
                            .map(|point| [point.x, point.y])
                    }),
                )
            });
            (elapsed, inventory.map_err(|error| error.to_string()))
        }
        Backend::Elk => {
            let (elapsed, result) = timed(|| elk::layout(&input.elk));
            let inventory = result.map(|out| {
                inspect(
                    out.nodes
                        .iter()
                        .map(|node| [node.x, node.y, node.width, node.height]),
                    out.edges
                        .iter()
                        .map(|edge| edge.points.iter().map(|point| [point.x, point.y])),
                )
            });
            (elapsed, inventory.map_err(|error| error.to_string()))
        }
        Backend::Shoal => {
            let (elapsed, result) = timed(|| {
                shoal::layout_with_control(
                    &input.shoal,
                    &input.options,
                    scratch,
                    &mut shoal::NoopWorkControl,
                )
            });
            let inventory = result.map(|out| {
                inspect(
                    out.nodes
                        .iter()
                        .map(|node| [node.x, node.y, node.width, node.height]),
                    out.edges
                        .iter()
                        .map(|edge| edge.points.iter().map(|point| [point.x, point.y])),
                )
            });
            (elapsed, inventory.map_err(|error| error.to_string()))
        }
    }
}

fn write_row(out: &mut impl Write, row: Value) {
    serde_json::to_writer(&mut *out, &row).unwrap();
    out.write_all(b"\n").unwrap();
    out.flush().unwrap();
}

#[test]
#[ignore = "run serially on an idle host with MERMAN_SHOAL_STRESS_PLAN and MERMAN_SHOAL_KERNEL_OUT"]
fn native_kernel_samples() {
    let path = |key| PathBuf::from(std::env::var_os(key).unwrap_or_else(|| panic!("set {key}")));
    let plan_path = path("MERMAN_SHOAL_STRESS_PLAN");
    let bytes = fs::read(&plan_path).unwrap();
    let plan: Plan = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(plan.schema_version, 1);
    let output = path("MERMAN_SHOAL_KERNEL_OUT");
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut out = BufWriter::new(
        fs::File::create_new(&output)
            .unwrap_or_else(|error| panic!("{}: {error}", output.display())),
    );
    write_row(
        &mut out,
        json!({
            "record": "run", "schema_version": 1, "suite": plan.suite, "plan_sha256": sha256(&bytes),
            "cases": CASES, "warmup_rounds": 1, "measured_rounds": MEASURED_ROUNDS,
            "orders": ORDERS, "scratch_cap_bytes": SCRATCH_CAP, "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "debug_assertions": cfg!(debug_assertions),
            "timed": ["dugong::layout", "merman_layout_elk::layout", "merman_layout_shoal::layout_with_control"],
            "preparation": "one untimed Shoal preparation per source with a separate scratch; Dagre input rebuilt before each call, ELK and Shoal inputs reused",
            "common_input": "original IDs and declaration order, outer node sizes, directed edge occurrences, middle label sizes, minimum spans, direction and spacing",
            "projection": "Shoal also consumes the measured port spans, anchors and marker metrics; Dagre and ELK use their native endpoint models",
            "excluded": ["parse", "theme", "measurement", "MeasuredGraph to native input", "result inspection/drop", "SVG"],
            "work_control": "native unrestricted; native algorithm policies and internal conversions are included",
            "scratch": "one default LayoutScratch per topology, reused across calls; null = no caller-owned scratch API",
        }),
    );
    let mut failed = 0;
    for id in CASES {
        let mut matches = plan.topologies.iter().filter(|topology| topology.id == id);
        let case = matches
            .next()
            .unwrap_or_else(|| panic!("missing topology {id}"));
        assert!(matches.next().is_none(), "duplicate topology {id}");
        let input = Frozen::new(case);
        let snapshot = input.snapshot();
        let measured_sha256 = sha256(&serde_json::to_vec(&snapshot).unwrap());
        write_row(
            &mut out,
            json!({
                "record": "input", "case": id, "source_sha256": case.source_sha256,
                "measured_sha256": measured_sha256, "measured": snapshot,
                "shoal_options": format!("{:?}", input.options), "elk_options": format!("{:?}", input.elk.options),
            }),
        );
        let mut scratch = shoal::LayoutScratch::default();
        for round in 0..=MEASURED_ROUNDS {
            let warmup = round == 0;
            let order = ORDERS[round.saturating_sub(1) % ORDERS.len()];
            for (position, backend) in order.into_iter().enumerate() {
                let before = (backend == Backend::Shoal).then(|| scratch.retained_bytes());
                let (elapsed, result) = sample(backend, &input, &mut scratch);
                let after = (backend == Backend::Shoal).then(|| scratch.retained_bytes());
                let valid = result.as_ref().is_ok_and(|geometry| {
                    (geometry.nodes, geometry.edges) == (case.nodes, case.edges)
                        && geometry.invalid_boxes == 0
                        && geometry.invalid_routes == 0
                }) && after.is_none_or(|bytes| bytes <= SCRATCH_CAP);
                failed += usize::from(!valid);
                write_row(
                    &mut out,
                    json!({
                        "record": "sample", "case": id, "backend": backend, "measured_sha256": measured_sha256,
                        "phase": if warmup { "warmup" } else { "measured" }, "round": round, "position": position,
                        "elapsed_ns": elapsed, "scratch_before_bytes": before, "scratch_after_bytes": after,
                        "valid": valid, "geometry": result.as_ref().ok(), "error": result.as_ref().err(),
                    }),
                );
            }
        }
    }
    write_row(
        &mut out,
        json!({"record": "complete", "failed_samples": failed}),
    );
    assert_eq!(
        failed,
        0,
        "inspect native kernel samples in {}",
        output.display()
    );
}
