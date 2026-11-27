//! Class Dagre projection and compound namespace layout.

use merman_render_graph::class::measured::MeasuredClassDiagram;

use super::measured::{MeasuredGraph, measure_class_diagram};
use super::{
    ClassDiagramModel, ClassPreparedLabels, ClassRenderConfig, ClassTextThemePlan,
    EdgeTerminalMetrics, Rect, apply_class_terminal_labels, class_namespace_ids_in_decl_order,
    class_namespace_label, compute_bounds, normalize_dir, terminal_path_for_edge,
};
use crate::graph_layout::ir::Direction;
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
use crate::model::{
    Bounds, ClassDiagramLayout, ClassRenderItem, ClassRenderRoot, ClassRenderRootId,
    ClassRenderTree, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::text::TextMeasurer;
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use indexmap::IndexMap;
use merman_model::presentation::PresentationConfig;
use rustc_hash::FxHashMap;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

type ClassLayoutGraph = Graph<NodeLabel, EdgeLabel, GraphLabel>;
type ExtractedClusterGraph = (Box<ClassLayoutGraph>, HashSet<String>);

#[cfg(test)]
mod tests;

pub(crate) fn layout(
    model: &ClassDiagramModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    render_config: &ClassRenderConfig,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<(ClassDiagramLayout, ClassPreparedLabels)> {
    let settings = render_config.layout_settings();
    let MeasuredClassDiagram {
        graph,
        class_label_plans_by_id,
        prepared_labels,
        node_label_metrics_by_id,
        namespace_label_metrics_by_id,
    } = measure_class_diagram(
        model,
        effective_config,
        measurer,
        math_renderer,
        render_config,
        false,
        Some(work_control),
    )?;
    let namespace_ids = class_namespace_ids_in_decl_order(model);
    let effective_dir = normalize_dir(&model.model().direction);
    let namespace_padding = settings.namespace_padding;
    let title_margin_top = settings.title_margin_top;
    let title_margin_bottom = settings.title_margin_bottom;
    let mut prepared = prepare_graph(Box::new(project_graph(graph)))?;
    let mut render_roots = Vec::new();
    let (mut fragments, _) = layout_prepared(
        &mut prepared,
        &node_label_metrics_by_id,
        &mut render_roots,
        work_control,
    )?;

    let node_rect_by_id: HashMap<_, _> = fragments
        .nodes
        .values()
        .map(|node| {
            (
                node.id.as_str(),
                Rect::from_center(node.x, node.y, node.width, node.height),
            )
        })
        .collect();

    for (edge, terminal_meta) in &mut fragments.edges {
        let Some(meta) = terminal_meta.as_ref() else {
            continue;
        };
        let points = if let (Some(from), Some(to)) = (
            node_rect_by_id.get(edge.from.as_str()).copied(),
            node_rect_by_id.get(edge.to.as_str()).copied(),
        ) {
            terminal_path_for_edge(&edge.points, from, to)
        } else {
            edge.points.clone()
        };
        apply_class_terminal_labels(edge, meta, &points)?;
    }

    let mut clusters: Vec<LayoutCluster> = Vec::new();
    // Mermaid renders namespaces as Dagre clusters. The cluster geometry comes from the Dagre
    // compound layout (not a post-hoc union of class-node bboxes). Use the computed namespace
    // node x/y/width/height and mirror `clusters.js` sizing tweaks for title width.
    for id in namespace_ids {
        let Some(ns_node) = fragments.nodes.get(id) else {
            continue;
        };
        let cx = ns_node.x;
        let cy = ns_node.y;
        let base_w = ns_node.width.max(1.0);
        let base_h = ns_node.height.max(1.0);

        let title = class_namespace_label(model, id).to_string();
        let (tw, th) = namespace_label_metrics_by_id[id];
        let min_title_w = (tw + namespace_padding).max(1.0);
        let width = if base_w <= min_title_w {
            min_title_w
        } else {
            base_w
        };
        let diff = if base_w <= min_title_w {
            (width - base_w) / 2.0 - namespace_padding
        } else {
            -namespace_padding
        };
        let offset_y = th - namespace_padding / 2.0;
        let title_label = LayoutLabel {
            x: cx,
            y: (cy - base_h / 2.0) + title_margin_top + th / 2.0,
            width: tw,
            height: th,
        };

        clusters.push(LayoutCluster {
            id: id.to_string(),
            x: cx,
            y: cy,
            width,
            height: base_h,
            diff,
            offset_y,
            title,
            title_label,
            requested_dir: None,
            effective_dir: effective_dir.clone(),
            padding: namespace_padding,
            title_margin_top,
            title_margin_bottom,
        });
    }

    let render_tree = ClassRenderTree {
        roots: render_roots,
        top: fragments.render_root_id,
    };
    let nodes: Vec<LayoutNode> = fragments.nodes.into_values().collect();
    let edges: Vec<LayoutEdge> = fragments.edges.into_iter().map(|(e, _)| e).collect();

    let bounds = compute_bounds(&nodes, &edges, &clusters);

    Ok((
        ClassDiagramLayout {
            nodes,
            edges,
            clusters,
            bounds,
            uses_elk_adapter_dom: false,
            class_label_plans_by_id,
            render_tree,
        },
        prepared_labels,
    ))
}

/// Debug-only helper: builds the production Class Dagre graph and returns it before layout runs.
///
/// Shares measurement and the Dagre projection with production, including source order,
/// named multiedges, terminal-label metrics, and namespace parents.
pub fn debug_build_class_diagram_dagre_graph(
    model: &ClassDiagramModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
) -> Result<ClassLayoutGraph> {
    let typography_theme = ClassTextThemePlan::resolve(None, effective_config);
    let render_config = ClassRenderConfig::resolve(effective_config, &typography_theme);
    let measured = measure_class_diagram(
        model,
        effective_config,
        measurer,
        None,
        &render_config,
        false,
        None,
    )?;
    Ok(project_graph(measured.graph))
}

fn project_graph(measured: MeasuredGraph) -> ClassLayoutGraph {
    let rankdir = match measured.direction {
        Direction::Down => RankDir::TB,
        Direction::Up => RankDir::BT,
        Direction::Right => RankDir::LR,
        Direction::Left => RankDir::RL,
    };

    let mut graph = Graph::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    });
    graph.set_graph(GraphLabel {
        rankdir,
        nodesep: measured.nodesep,
        ranksep: measured.ranksep,
        // The SVG viewport adds Mermaid's graph margins after layout.
        marginx: 0.0,
        marginy: 0.0,
        ..Default::default()
    });
    // Replay node insertion, parent updates, and named edges in their original orders.
    for (id, node) in measured.nodes {
        graph.set_node(
            id,
            NodeLabel {
                width: node.width,
                height: node.height,
                ..Default::default()
            },
        );
    }
    for (child, parent) in measured.parents {
        graph.set_parent(child, parent);
    }
    for ((source, target, id), edge) in measured.edges {
        let mut label = EdgeLabel {
            width: edge.width,
            height: edge.height,
            labelpos: LabelPos::C,
            labeloffset: 10.0,
            minlen: 1,
            weight: 1.0,
            ..Default::default()
        };
        if let Some(meta) = &edge.terminals {
            let extras = &mut *label.extras;
            for (key, metrics) in [
                ("startLeft", meta.start_left),
                ("startRight", meta.start_right),
                ("endLeft", meta.end_left),
                ("endRight", meta.end_right),
            ] {
                if let Some((width, height)) = metrics {
                    let dimensions = [
                        ("width".to_owned(), Value::from(width)),
                        ("height".to_owned(), Value::from(height)),
                    ];
                    extras.insert(
                        key.to_owned(),
                        Value::Object(dimensions.into_iter().collect()),
                    );
                }
            }
            extras.insert("startMarker".to_string(), meta.start_marker.into());
            extras.insert("endMarker".to_string(), meta.end_marker.into());
        }
        graph.set_edge_named(source, target, Some(id), Some(label));
    }
    graph
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PreparedGraphId(usize);

struct PreparedGraph {
    graph: Box<ClassLayoutGraph>,
    extracted: BTreeMap<String, PreparedGraphId>,
    injected_cluster_root_id: Option<String>,
}

struct PreparedGraphArena {
    graphs: Vec<PreparedGraph>,
    top: PreparedGraphId,
}

fn extract_cluster_copy_order(
    graph: &ClassLayoutGraph,
    cluster_id: &str,
    root_id: &str,
    out: &mut Vec<String>,
) {
    // Mirrors Mermaid's `copy(...)`: children are copied before the non-root cluster node itself.
    // That order decides which nested cluster is extracted first in later recursive passes.
    let mut stack: Vec<(String, bool)> = vec![(cluster_id.to_string(), false)];
    while let Some((node, expanded)) = stack.pop() {
        if expanded {
            if node != root_id {
                out.push(node);
            }
            continue;
        }

        let children = graph.children(&node);
        if children.is_empty() {
            if node != root_id {
                out.push(node);
            }
            continue;
        }

        stack.push((node, true));
        for child in children.iter().rev() {
            stack.push((child.to_string(), false));
        }
    }
}

struct ClassClusterHierarchy<'a> {
    ids: Vec<&'a str>,
    index_by_id: FxHashMap<&'a str, usize>,
    parent: Vec<Option<usize>>,
    depth: Vec<usize>,
    root: Vec<usize>,
    ancestor_jumps: Vec<Vec<usize>>,
    postorder: Vec<usize>,
}

impl<'a> ClassClusterHierarchy<'a> {
    fn new(graph: &'a ClassLayoutGraph) -> Self {
        let ids = graph.nodes().collect::<Vec<_>>();
        let index_by_id = ids
            .iter()
            .copied()
            .enumerate()
            .map(|(index, id)| (id, index))
            .collect::<FxHashMap<_, _>>();
        let parent = ids
            .iter()
            .map(|id| {
                graph
                    .parent(id)
                    .and_then(|parent| index_by_id.get(parent).copied())
            })
            .collect::<Vec<_>>();
        let mut depth = vec![0; ids.len()];
        let mut root = vec![0; ids.len()];
        let mut stack = parent
            .iter()
            .enumerate()
            .filter_map(|(index, parent)| parent.is_none().then_some(index))
            .collect::<Vec<_>>();
        for index in stack.iter().copied() {
            root[index] = index;
        }
        let mut traversal_order = Vec::with_capacity(ids.len());
        while let Some(index) = stack.pop() {
            traversal_order.push(index);
            for child in graph
                .children_iter(ids[index])
                .filter_map(|id| index_by_id.get(id).copied())
            {
                depth[child] = depth[index] + 1;
                root[child] = root[index];
                stack.push(child);
            }
        }

        let ancestor_levels = usize::BITS as usize - ids.len().max(1).leading_zeros() as usize;
        let mut ancestor_jumps = Vec::with_capacity(ancestor_levels);
        ancestor_jumps.push(
            parent
                .iter()
                .enumerate()
                .map(|(index, parent)| parent.unwrap_or(index))
                .collect::<Vec<_>>(),
        );
        for level in 1..ancestor_levels {
            let previous = &ancestor_jumps[level - 1];
            ancestor_jumps.push(
                previous
                    .iter()
                    .map(|ancestor| previous[*ancestor])
                    .collect(),
            );
        }

        traversal_order.reverse();
        Self {
            ids,
            index_by_id,
            parent,
            depth,
            root,
            ancestor_jumps,
            postorder: traversal_order,
        }
    }

    fn lowest_common_ancestor(&self, mut lhs: usize, mut rhs: usize) -> Option<usize> {
        if self.root[lhs] != self.root[rhs] {
            return None;
        }
        if self.depth[lhs] < self.depth[rhs] {
            std::mem::swap(&mut lhs, &mut rhs);
        }

        let depth_delta = self.depth[lhs] - self.depth[rhs];
        for (level, ancestors) in self.ancestor_jumps.iter().enumerate() {
            if depth_delta & (1 << level) != 0 {
                lhs = ancestors[lhs];
            }
        }
        if lhs == rhs {
            return Some(lhs);
        }
        for ancestors in self.ancestor_jumps.iter().rev() {
            let lhs_ancestor = ancestors[lhs];
            let rhs_ancestor = ancestors[rhs];
            if lhs_ancestor != rhs_ancestor {
                lhs = lhs_ancestor;
                rhs = rhs_ancestor;
            }
        }
        self.parent[lhs]
    }

    fn boundary_crossings(&self, graph: &ClassLayoutGraph) -> Vec<i64> {
        // For one edge, the clusters whose strict-descendant boundary it crosses are exactly the
        // two ancestor paths from each endpoint's parent up to, but excluding, their LCA. Tree
        // differences mark both paths in O(log N), including Mermaid's rule that an edge incident
        // on the cluster node itself does not make that cluster ineligible.
        let mut crossings = vec![0_i64; self.ids.len()];
        for edge in graph.edges() {
            let Some(&from) = self.index_by_id.get(edge.v.as_str()) else {
                continue;
            };
            let Some(&to) = self.index_by_id.get(edge.w.as_str()) else {
                continue;
            };
            let common = self.lowest_common_ancestor(from, to);
            for endpoint in [from, to] {
                if Some(endpoint) == common {
                    continue;
                }
                if let Some(parent) = self.parent[endpoint] {
                    crossings[parent] += 1;
                    if let Some(common) = common {
                        crossings[common] -= 1;
                    }
                }
            }
        }
        for index in self.postorder.iter().copied() {
            if let Some(parent) = self.parent[index] {
                crossings[parent] += crossings[index];
            }
        }
        crossings
    }
}

fn class_cluster_candidates(graph: &ClassLayoutGraph) -> Vec<String> {
    let hierarchy = ClassClusterHierarchy::new(graph);
    let boundary_crossings = hierarchy.boundary_crossings(graph);
    hierarchy
        .ids
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, id)| {
            graph.children_iter(id).next().is_some() && boundary_crossings[*index] == 0
        })
        .map(|(_, id)| id.to_string())
        .collect()
}

fn class_nodes_in_hierarchy_order(graph: &ClassLayoutGraph) -> Vec<&str> {
    // Mermaid's Dagre renderer inserts cluster elements using a hierarchy preorder. Parent
    // clusters must therefore precede their descendants so an opaque parent fill cannot cover
    // the child cluster's frame and label.
    let mut ordered = Vec::with_capacity(graph.node_count());
    let mut stack = graph.children_root().into_iter().rev().collect::<Vec<_>>();
    while let Some(id) = stack.pop() {
        ordered.push(id);
        stack.extend(graph.children(id).into_iter().rev());
    }
    ordered
}

fn prepare_graph(graph: Box<ClassLayoutGraph>) -> Result<PreparedGraphArena> {
    // Mermaid's default Class renderer uses the shared Dagre rendering-util path. Its
    // graphlib pre-pass extracts clusters *without* external connections into their own subgraphs,
    // toggles their rankdir (TB <-> LR), and renders them recursively to obtain concrete cluster
    // geometry before laying out the parent graph.
    //
    // Reference: pinned Mermaid `rendering-util/layout-algorithms/dagre`:
    // - eligible cluster: has children, and no edge crosses its descendant boundary
    // - extracted subgraph gets `rankdir = parent.rankdir === 'TB' ? 'LR' : 'TB'`
    // - `copy(...)` walks child clusters first and copies a non-root cluster node after its
    //   children, so child extractions may later be moved under an extracted parent
    // - recursive render copies `nodesep` and sets child `ranksep = parent.ranksep + 25`
    // - margins are fixed at 8

    struct PendingCluster {
        id: String,
        moved_ids: HashSet<String>,
    }

    struct PrepareFrame {
        graph: Box<ClassLayoutGraph>,
        candidates: Vec<String>,
        next_candidate: usize,
        extracted: BTreeMap<String, PreparedGraphId>,
        injected_cluster_root_id: Option<String>,
        pending: Option<PendingCluster>,
    }

    fn frame_for(
        graph: Box<ClassLayoutGraph>,
        injected_cluster_root_id: Option<String>,
    ) -> PrepareFrame {
        let candidates = class_cluster_candidates(&graph);
        PrepareFrame {
            graph,
            candidates,
            next_candidate: 0,
            extracted: BTreeMap::new(),
            injected_cluster_root_id,
            pending: None,
        }
    }

    let mut graphs: Vec<PreparedGraph> = Vec::new();
    let mut stack = vec![frame_for(graph, None)];
    let mut completed_child = None;
    loop {
        if let Some(child_id) = completed_child.take() {
            let Some(parent) = stack.last_mut() else {
                return Ok(PreparedGraphArena {
                    graphs,
                    top: child_id,
                });
            };
            let Some(pending) = parent.pending.take() else {
                return Err(Error::InvalidModel {
                    message: format!(
                        "prepared Class child graph {} has no parent extraction",
                        child_id.0
                    ),
                });
            };
            for moved_id in pending.moved_ids {
                if let Some(moved_child_id) = parent.extracted.remove(&moved_id) {
                    graphs[child_id.0]
                        .extracted
                        .insert(moved_id, moved_child_id);
                }
            }
            parent.extracted.insert(pending.id, child_id);
        }

        let Some(frame) = stack.last_mut() else {
            return Err(Error::InvalidModel {
                message: "missing Class prepare frame".to_string(),
            });
        };
        let mut child_frame = None;
        while let Some(cluster_id) = frame.candidates.get(frame.next_candidate).cloned() {
            frame.next_candidate += 1;
            if frame.graph.children(&cluster_id).is_empty() {
                continue;
            }

            let parent_dir = frame.graph.graph().rankdir;
            let nodesep = frame.graph.graph().nodesep;
            let ranksep = frame.graph.graph().ranksep;
            let (mut subgraph, moved_ids) = extract_cluster_graph(&cluster_id, &mut frame.graph)?;
            let child_label = subgraph.graph_mut();
            child_label.rankdir = if parent_dir == RankDir::TB {
                RankDir::LR
            } else {
                RankDir::TB
            };
            child_label.nodesep = nodesep;
            child_label.ranksep = ranksep;
            child_label.marginx = 8.0;
            child_label.marginy = 8.0;
            frame.pending = Some(PendingCluster {
                id: cluster_id.clone(),
                moved_ids,
            });
            child_frame = Some(frame_for(subgraph, Some(cluster_id)));
            break;
        }

        if let Some(child_frame) = child_frame {
            stack.push(child_frame);
            continue;
        }

        let frame = stack.pop().expect("checked Class prepare frame");
        let graph_id = PreparedGraphId(graphs.len());
        graphs.push(PreparedGraph {
            graph: frame.graph,
            extracted: frame.extracted,
            injected_cluster_root_id: frame.injected_cluster_root_id,
        });
        completed_child = Some(graph_id);
    }
}

fn extract_cluster_graph(
    cluster_id: &str,
    graph: &mut ClassLayoutGraph,
) -> Result<ExtractedClusterGraph> {
    if graph.children(cluster_id).is_empty() {
        return Err(Error::InvalidModel {
            message: format!("cluster has no children: {cluster_id}"),
        });
    }

    let mut descendants: Vec<String> = Vec::new();
    extract_cluster_copy_order(graph, cluster_id, cluster_id, &mut descendants);

    let moved_set: HashSet<String> = descendants.iter().cloned().collect();

    let mut sub = Box::new(ClassLayoutGraph::new(GraphOptions {
        directed: true,
        multigraph: true,
        compound: true,
    }));

    // Preserve parent graph settings as a base.
    sub.set_graph(graph.graph().clone());

    for id in &descendants {
        let Some(label) = graph.node(id).cloned() else {
            continue;
        };
        sub.set_node(id.clone(), label);
    }

    for key in graph.edge_keys() {
        if moved_set.contains(&key.v)
            && moved_set.contains(&key.w)
            && let Some(label) = graph.edge_by_key(&key).cloned()
        {
            sub.set_edge_named(key.v.clone(), key.w.clone(), key.name.clone(), Some(label));
        }
    }

    for id in &descendants {
        let Some(parent) = graph.parent(id) else {
            continue;
        };
        if moved_set.contains(parent) {
            sub.set_parent(id.clone(), parent.to_string());
        }
    }

    for id in &descendants {
        let _ = graph.remove_node(id);
    }

    Ok((sub, moved_set))
}

fn edge_terminal_metrics_from_extras(e: &EdgeLabel) -> EdgeTerminalMetrics {
    let extras = &*e.extras;
    let get_pair = |key: &str| -> Option<(f64, f64)> {
        let obj = extras.get(key)?;
        let w = obj.get("width").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let h = obj.get("height").and_then(|v| v.as_f64()).unwrap_or(0.0);
        if w > 0.0 && h > 0.0 {
            Some((w, h))
        } else {
            None
        }
    };
    let start_marker = extras
        .get("startMarker")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let end_marker = extras
        .get("endMarker")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    EdgeTerminalMetrics {
        start_left: get_pair("startLeft"),
        start_right: get_pair("startRight"),
        end_left: get_pair("endLeft"),
        end_right: get_pair("endRight"),
        start_marker,
        end_marker,
        #[cfg(feature = "layout-elk")]
        start_arrow_type: None,
        #[cfg(feature = "layout-elk")]
        end_arrow_type: None,
    }
}

struct LayoutFragments {
    nodes: IndexMap<String, LayoutNode>,
    edges: Vec<(LayoutEdge, Option<EdgeTerminalMetrics>)>,
    render_root_id: ClassRenderRootId,
}

fn layout_prepared(
    arena: &mut PreparedGraphArena,
    node_label_metrics_by_id: &HashMap<String, (f64, f64)>,
    render_roots: &mut Vec<ClassRenderRoot>,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<(LayoutFragments, Rect)> {
    if arena.top.0 >= arena.graphs.len() {
        return Err(Error::InvalidModel {
            message: format!(
                "invalid prepared Class graph top {} for {} graphs",
                arena.top.0,
                arena.graphs.len()
            ),
        });
    }

    // Mermaid adds 25px rank separation at each recursive render boundary. Propagate those graph
    // settings top-down before laying nodes out bottom-up.
    let mut settings_stack = vec![arena.top];
    while let Some(parent_id) = settings_stack.pop() {
        let parent_ranksep = arena.graphs[parent_id.0].graph.graph().ranksep;
        let parent_nodesep = arena.graphs[parent_id.0].graph.graph().nodesep;
        let child_ids = arena.graphs[parent_id.0]
            .extracted
            .values()
            .copied()
            .collect::<Vec<_>>();
        for child_id in child_ids.into_iter().rev() {
            let Some(child) = arena.graphs.get_mut(child_id.0) else {
                return Err(Error::InvalidModel {
                    message: format!("missing prepared Class child graph {}", child_id.0),
                });
            };
            child.graph.graph_mut().ranksep = parent_ranksep + 25.0;
            child.graph.graph_mut().nodesep = parent_nodesep;
            settings_stack.push(child_id);
        }
    }

    enum PreparedFrame {
        Enter(PreparedGraphId),
        Exit(PreparedGraphId),
    }
    let mut graph_state = vec![0_u8; arena.graphs.len()];
    let mut postorder = Vec::with_capacity(arena.graphs.len());
    let mut stack = vec![PreparedFrame::Enter(arena.top)];
    while let Some(frame) = stack.pop() {
        match frame {
            PreparedFrame::Enter(graph_id) => {
                let Some(graph) = arena.graphs.get(graph_id.0) else {
                    return Err(Error::InvalidModel {
                        message: format!("missing prepared Class graph {}", graph_id.0),
                    });
                };
                match graph_state[graph_id.0] {
                    1 => {
                        return Err(Error::InvalidModel {
                            message: format!(
                                "cycle in prepared Class graph arena at {}",
                                graph_id.0
                            ),
                        });
                    }
                    2 => {
                        return Err(Error::InvalidModel {
                            message: format!(
                                "prepared Class graph {} has multiple owners",
                                graph_id.0
                            ),
                        });
                    }
                    _ => {}
                }
                graph_state[graph_id.0] = 1;
                stack.push(PreparedFrame::Exit(graph_id));
                for child_id in graph.extracted.values().rev() {
                    stack.push(PreparedFrame::Enter(*child_id));
                }
            }
            PreparedFrame::Exit(graph_id) => {
                graph_state[graph_id.0] = 2;
                postorder.push(graph_id);
            }
        }
    }
    if let Some(unattached) = graph_state.iter().position(|state| *state == 0) {
        return Err(Error::InvalidModel {
            message: format!("unattached prepared Class graph {unattached}"),
        });
    }

    let mut results = (0..arena.graphs.len())
        .map(|_| None)
        .collect::<Vec<Option<(LayoutFragments, Rect)>>>();
    for graph_id in postorder {
        let child_links = arena.graphs[graph_id.0].extracted.clone();
        let mut extracted_fragments = BTreeMap::new();
        for (cluster_id, child_id) in child_links {
            let Some(result) = results.get_mut(child_id.0).and_then(Option::take) else {
                return Err(Error::InvalidModel {
                    message: format!(
                        "missing laid out Class child graph {} for {cluster_id}",
                        child_id.0
                    ),
                });
            };
            extracted_fragments.insert(cluster_id, result);
        }
        let result = layout_prepared_node(
            &mut arena.graphs[graph_id.0],
            node_label_metrics_by_id,
            render_roots,
            extracted_fragments,
            work_control,
        )?;
        results[graph_id.0] = Some(result);
    }

    results
        .get_mut(arena.top.0)
        .and_then(Option::take)
        .ok_or_else(|| Error::InvalidModel {
            message: format!("missing laid out Class top graph {}", arena.top.0),
        })
}

fn layout_prepared_node(
    prepared: &mut PreparedGraph,
    node_label_metrics_by_id: &HashMap<String, (f64, f64)>,
    render_roots: &mut Vec<ClassRenderRoot>,
    extracted_fragments: BTreeMap<String, (LayoutFragments, Rect)>,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<(LayoutFragments, Rect)> {
    let root_namespace_id = prepared.injected_cluster_root_id.clone();
    let render_root_id = ClassRenderRootId(render_roots.len());
    render_roots.push(ClassRenderRoot {
        namespace_id: root_namespace_id,
        ..Default::default()
    });
    let mut fragments = LayoutFragments {
        nodes: IndexMap::new(),
        edges: Vec::new(),
        render_root_id,
    };

    if let Some(root_id) = prepared.injected_cluster_root_id.clone() {
        if prepared.graph.node(&root_id).is_none() {
            prepared
                .graph
                .set_node(root_id.clone(), NodeLabel::default());
        }
        let top_level_ids: Vec<String> = prepared
            .graph
            .node_ids()
            .into_iter()
            .filter(|id| id != &root_id && prepared.graph.parent(id).is_none())
            .collect();
        for id in top_level_ids {
            prepared.graph.set_parent(id, root_id.clone());
        }
    }

    for (id, (_sub_frag, bounds)) in &extracted_fragments {
        let Some(n) = prepared.graph.node_mut(id) else {
            return Err(Error::InvalidModel {
                message: format!("missing cluster placeholder node: {id}"),
            });
        };
        n.width = bounds.width().max(1.0);
        n.height = bounds.height().max(1.0);
    }

    // Mermaid's dagre wrapper always sets `compound: true`, and Dagre's ranker expects a connected
    // graph. `dugong::layout` mirrors Dagre's full pipeline (including `nestingGraph`)
    // and should be used for class diagrams even when there are no explicit clusters.
    crate::layout_work::measure_layout_solve!(dugong::layout_controlled(
        &mut prepared.graph,
        work_control
    ))
    .map_err(|error| work_control.map_dugong_error(error))?;

    // Mermaid does not render Dagre's internal dummy nodes/edges (border nodes, edge label nodes,
    // nesting artifacts). Filter them out before computing bounds and before merging extracted
    // layouts back into the parent.
    let mut dummy_nodes: HashSet<String> = HashSet::new();
    for id in prepared.graph.node_ids() {
        let Some(n) = prepared.graph.node(&id) else {
            continue;
        };
        if n.dummy.is_some() {
            dummy_nodes.insert(id);
            continue;
        }
        let is_cluster =
            !prepared.graph.children(&id).is_empty() || prepared.extracted.contains_key(&id);
        let (label_width, label_height) = node_label_metrics_by_id
            .get(id.as_str())
            .copied()
            .map(|(w, h)| (Some(w), Some(h)))
            .unwrap_or((None, None));
        fragments.nodes.insert(
            id.clone(),
            LayoutNode {
                id: id.clone(),
                x: n.x.unwrap_or(0.0),
                y: n.y.unwrap_or(0.0),
                width: n.width,
                height: n.height,
                is_cluster,
                label_width,
                label_height,
            },
        );
    }

    for key in prepared.graph.edge_keys() {
        let Some(e) = prepared.graph.edge_by_key(&key) else {
            continue;
        };
        if e.nesting_edge {
            continue;
        }
        if dummy_nodes.contains(&key.v) || dummy_nodes.contains(&key.w) {
            continue;
        }
        if !fragments.nodes.contains_key(&key.v) || !fragments.nodes.contains_key(&key.w) {
            continue;
        }
        let id = key
            .name
            .clone()
            .unwrap_or_else(|| format!("edge:{}:{}", key.v, key.w));

        let label = if e.width > 0.0 && e.height > 0.0 {
            Some(LayoutLabel {
                x: e.x.unwrap_or(0.0),
                y: e.y.unwrap_or(0.0),
                width: e.width,
                height: e.height,
            })
        } else {
            None
        };

        let points = e
            .points
            .iter()
            .map(|p| LayoutPoint { x: p.x, y: p.y })
            .collect::<Vec<_>>();

        let edge = LayoutEdge {
            id,
            from: key.v.clone(),
            to: key.w.clone(),
            from_cluster: None,
            to_cluster: None,
            points,
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        };

        let terminals = edge_terminal_metrics_from_extras(e);
        let has_terminals = terminals.start_left.is_some()
            || terminals.start_right.is_some()
            || terminals.end_left.is_some()
            || terminals.end_right.is_some();
        let terminal_meta = if has_terminals { Some(terminals) } else { None };

        fragments.edges.push((edge, terminal_meta));
    }

    let mut child_roots = extracted_fragments
        .iter()
        .map(|(id, (fragments, _))| (id.clone(), fragments.render_root_id))
        .collect::<BTreeMap<_, _>>();
    let current_node_ids = prepared.graph.node_ids();
    let edge_ids = fragments
        .edges
        .iter()
        .map(|(edge, _)| edge.id.clone())
        .collect();
    let cluster_ids = class_nodes_in_hierarchy_order(&prepared.graph)
        .into_iter()
        .filter(|id| {
            !dummy_nodes.contains(*id)
                && prepared.graph.child_count(id) > 0
                && !prepared.extracted.contains_key(*id)
        })
        .map(str::to_owned)
        .collect();
    let mut items = Vec::new();
    for id in current_node_ids {
        if dummy_nodes.contains(id.as_str()) {
            continue;
        }
        if let Some(root_id) = child_roots.remove(id.as_str()) {
            items.push(ClassRenderItem::Subgraph(root_id));
        } else if prepared.graph.children(&id).is_empty() {
            items.push(ClassRenderItem::Node(id));
        }
    }
    if !child_roots.is_empty() {
        return Err(Error::InvalidModel {
            message: format!(
                "class layout did not attach extracted render roots: {}",
                child_roots.keys().cloned().collect::<Vec<_>>().join(", ")
            ),
        });
    }
    let Some(render_root) = render_roots.get_mut(render_root_id.0) else {
        return Err(Error::InvalidModel {
            message: format!("missing class render root arena entry {}", render_root_id.0),
        });
    };
    render_root.edge_ids = edge_ids;
    render_root.cluster_ids = cluster_ids;
    render_root.items = items;

    for (cluster_id, (mut sub_frag, sub_bounds)) in extracted_fragments {
        let Some(cluster_node) = fragments.nodes.get(&cluster_id).cloned() else {
            return Err(Error::InvalidModel {
                message: format!("missing cluster placeholder layout: {cluster_id}"),
            });
        };
        let (sub_cx, sub_cy) = sub_bounds.center();
        let dx = cluster_node.x - sub_cx;
        let dy = cluster_node.y - sub_cy;

        for n in sub_frag.nodes.values_mut() {
            n.x += dx;
            n.y += dy;
        }
        for (e, _t) in &mut sub_frag.edges {
            for p in &mut e.points {
                p.x += dx;
                p.y += dy;
            }
            if let Some(l) = e.label.as_mut() {
                l.x += dx;
                l.y += dy;
            }
        }

        // The extracted subgraph includes its own copy of the cluster root node so bounds match
        // Mermaid's `updateNodeBounds(...)`. Do not merge that node back into the parent layout,
        // otherwise we'd overwrite the placeholder position computed by the parent graph layout.
        let _ = sub_frag.nodes.swap_remove(&cluster_id);

        fragments.nodes.extend(sub_frag.nodes);
        fragments.edges.extend(sub_frag.edges);
    }

    let mut points: Vec<(f64, f64)> = Vec::new();
    for n in fragments.nodes.values() {
        let r = Rect::from_center(n.x, n.y, n.width, n.height);
        points.push((r.min_x(), r.min_y()));
        points.push((r.max_x(), r.max_y()));
    }
    for (e, _t) in &fragments.edges {
        for p in &e.points {
            points.push((p.x, p.y));
        }
        if let Some(l) = &e.label {
            let r = Rect::from_center(l.x, l.y, l.width, l.height);
            points.push((r.min_x(), r.min_y()));
            points.push((r.max_x(), r.max_y()));
        }
    }
    let bounds = Bounds::from_points(points)
        .map(|b| Rect::from_min_max(b.min_x, b.min_y, b.max_x, b.max_y))
        .unwrap_or_else(|| Rect::from_min_max(0.0, 0.0, 0.0, 0.0));

    Ok((fragments, bounds))
}
