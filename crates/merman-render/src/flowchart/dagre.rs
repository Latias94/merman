use crate::geom::Size;
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
use crate::model::{
    FlowchartLayout, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint,
};
use crate::resources::OperationWorkMeter;
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions, is_javascript_array_index};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel};
use indexmap::IndexMap;
use merman_model::presentation::PresentationConfig;
use merman_render_core::dagre_geometry::compact_self_loop_geometry;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::config::{FlowchartConfigView, FlowchartLayoutSettings};
use super::label::compute_bounds_controlled;
use super::node::NodeLayoutDimensionsRequest;
use super::{
    FlowEdge, FlowSubgraph, FlowchartModel, FlowchartRenderContext, FlowchartRenderModelRef,
};
use super::{
    FlowchartLabelMetricsRequest, FlowchartSvgLabelOwner, FlowchartSvgLabelSidecarBuilder,
    FlowchartSvgWidthMode, flowchart_apply_html_node_class_box_metrics,
    flowchart_effective_text_style_for_classes,
    flowchart_effective_text_style_for_classes_with_provenance, flowchart_node_svg_width_mode,
    measure_flowchart_svg_label_for_layout_with_metrics_and_source_style,
    measure_flowchart_svg_label_for_layout_with_source_style,
};

use merman_render_flow::dagre::{
    adjust_flowchart_clusters_and_edges, charge_node_insertion_ordering, checked_n_log_n,
    child_id_snapshot_work_upper_bound, extract_clusters_recursively, lowest_common_parent,
    node_id_snapshot_work_upper_bound, normalize_dir, ordered_key_update_work_upper_bound,
    rank_dir_from_flow, toggled_dir,
};

fn unparented_parent_batch_work_upper_bound(
    node_slots: usize,
    existing_parent_count: usize,
    assignment_count: usize,
    numeric_assignment_count: usize,
    work_control: &OperationLayoutWorkControl,
) -> Result<usize> {
    if assignment_count == 0 {
        return Ok(0);
    }
    let height = if node_slots <= 1 {
        0
    } else {
        usize::BITS as usize - (node_slots - 1).leading_zeros() as usize
    };
    // Union by rank bounds every pre-compression find to `height`. Account for two traversals per
    // find, two finds per forest link, vector initialization/reservation, and one replay pass.
    let find_work = work_control.checked_mul(work_control.checked_add(height, 1)?, 4)?;
    let forest_items = work_control.checked_add(existing_parent_count, assignment_count)?;
    let union_work = work_control.checked_mul(forest_items, find_work)?;
    let linear_work = work_control.checked_add(
        work_control.checked_mul(node_slots, 6)?,
        work_control.checked_mul(assignment_count, 2)?,
    )?;
    // Each array-index child requires one root-bucket removal and one target-bucket insertion.
    // Callers derive this count while already traversing assignments in pinned Graphlib order, so
    // ordinary IDs do not pay a logarithmic ordered-map term that cannot occur.
    let ordered_updates = work_control.checked_mul(numeric_assignment_count, 2)?;
    let ordered_work =
        ordered_key_update_work_upper_bound(node_slots, ordered_updates, work_control)?;
    work_control.checked_add(
        work_control.checked_add(union_work, linear_work)?,
        ordered_work,
    )
}

#[derive(Debug, Clone, Copy, Default)]
struct UnparentedParentBatchShape {
    existing_parent_count: usize,
    numeric_assignment_count: usize,
}

fn apply_unparented_parent_assignments(
    graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    assignments: &[(usize, usize)],
    shape: UnparentedParentBatchShape,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<()> {
    if assignments.is_empty() {
        return Ok(());
    }
    let work = unparented_parent_batch_work_upper_bound(
        graph.node_slot_count(),
        shape.existing_parent_count,
        assignments.len(),
        shape.numeric_assignment_count,
        work_control,
    )?;
    work_control.charge_adapter(work)?;
    match graph.try_set_unparented_parents_ix(assignments) {
        Ok(_) => Ok(()),
        Err(dugong::graphlib::GraphError::ParentCycle {
            child_ix,
            parent_ix,
        }) => {
            let child = graph.node_id_by_ix(child_ix).unwrap_or("<removed>");
            let parent = graph.node_id_by_ix(parent_ix).unwrap_or("<removed>");
            Err(Error::InvalidModel {
                message: format!("Setting {parent} as parent of {child} would create a cycle"),
            })
        }
        Err(error) => Err(Error::InvalidModel {
            message: error.to_string(),
        }),
    }
}

type FlowSubgraphIndex<'a> = HashMap<&'a str, &'a FlowSubgraph>;

fn effective_cluster_dir(sg: &FlowSubgraph, parent_dir: &str, inherit_dir: bool) -> String {
    if let Some(dir) = sg.dir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return normalize_dir(dir);
    }
    if inherit_dir {
        return normalize_dir(parent_dir);
    }
    toggled_dir(parent_dir)
}

fn compute_effective_dir_by_id(
    subgraphs_in_order: &[FlowSubgraph],
    subgraphs_by_id: &FlowSubgraphIndex<'_>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    diagram_dir: &str,
    inherit_dir: bool,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<HashMap<String, String>> {
    fn compute_one_iterative(
        id: &str,
        subgraphs_by_id: &FlowSubgraphIndex<'_>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        diagram_dir: &str,
        inherit_dir: bool,
        memo: &mut HashMap<String, String>,
        work_control: &mut OperationLayoutWorkControl,
    ) -> Result<()> {
        if memo.contains_key(id) {
            return Ok(());
        }

        let mut path: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut cur = id.to_string();
        let mut parent_dir = loop {
            work_control.charge_adapter(1)?;
            if let Some(dir) = memo.get(&cur) {
                break dir.clone();
            }
            if !seen.insert(cur.clone()) {
                let dir = toggled_dir(diagram_dir);
                memo.insert(cur.clone(), dir.clone());
                break dir;
            }

            path.push(cur.clone());
            let Some(parent) = g.parent(&cur).filter(|p| subgraphs_by_id.contains_key(*p)) else {
                break normalize_dir(diagram_dir);
            };
            cur = parent.to_string();
        };

        for node in path.into_iter().rev() {
            work_control.charge_adapter(1)?;
            if let Some(dir) = memo.get(&node) {
                parent_dir = dir.clone();
                continue;
            }

            let dir = subgraphs_by_id
                .get(node.as_str())
                .map(|sg| effective_cluster_dir(sg, &parent_dir, inherit_dir))
                .unwrap_or_else(|| toggled_dir(&parent_dir));
            memo.insert(node, dir.clone());
            parent_dir = dir;
        }
        Ok(())
    }

    let mut memo: HashMap<String, String> = HashMap::new();
    // `std::collections::HashMap` seeds each map independently, so walking its keys makes the
    // amount of memoized parent-path work vary between otherwise identical renders. Mermaid's
    // source order is already available here; use it for deterministic work accounting and error
    // timing while retaining the map for lookups.
    for subgraph in subgraphs_in_order {
        work_control.charge_adapter(1)?;
        compute_one_iterative(
            &subgraph.id,
            subgraphs_by_id,
            g,
            diagram_dir,
            inherit_dir,
            &mut memo,
            work_control,
        )?;
    }
    Ok(memo)
}

const SELF_LOOP_ID_EXTRA: &str = "selfLoopId";
const SELF_LOOP_NODE_EXTRA: &str = "selfLoopNode";
const SELF_LOOP_ORDER_EXTRA: &str = "selfLoopOrder";

#[derive(Debug, Clone, PartialEq, Eq)]
struct FlowchartSelfLoopSegmentMeta {
    logical_edge_id: String,
    owner_index: usize,
    node_id: String,
    order: u8,
}

struct FlowchartLayoutEdgeCandidate {
    edge: LayoutEdge,
    owner_index: usize,
    self_loop: Option<FlowchartSelfLoopSegmentMeta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct FlowchartLayoutSegmentKey {
    owner_index: usize,
    self_loop_order: Option<u8>,
}

impl FlowchartLayoutSegmentKey {
    fn new(owner_index: usize, self_loop: Option<&FlowchartSelfLoopSegmentMeta>) -> Self {
        Self {
            owner_index,
            self_loop_order: self_loop.map(|meta| meta.order),
        }
    }
}

fn flowchart_layout_edge_key(
    edge: &FlowEdge,
    self_loop: Option<&FlowchartSelfLoopSegmentMeta>,
) -> String {
    match self_loop {
        Some(meta) => format!("{}-cyclic-special-{}", meta.node_id, meta.order),
        None => edge.id.clone(),
    }
}

fn annotate_flowchart_self_loop_segment(
    label: &mut EdgeLabel,
    meta: &FlowchartSelfLoopSegmentMeta,
) {
    let extras = &mut *label.extras;
    extras.insert(
        SELF_LOOP_ID_EXTRA.to_string(),
        Value::String(meta.logical_edge_id.clone()),
    );
    extras.insert(
        SELF_LOOP_NODE_EXTRA.to_string(),
        Value::String(meta.node_id.clone()),
    );
    extras.insert(SELF_LOOP_ORDER_EXTRA.to_string(), Value::from(meta.order));
}

fn checked_flowchart_edge_owner_index(
    owner_index: usize,
    work_control: &OperationLayoutWorkControl,
) -> Result<u32> {
    let Ok(index) = u32::try_from(owner_index) else {
        work_control.checkpoint()?;
        return Err(work_control.record_arithmetic_overflow().into());
    };
    Ok(index)
}

fn flowchart_edge_owner_index(label: &EdgeLabel) -> Option<usize> {
    usize::try_from(label.owner_index?).ok()
}

fn flowchart_layout_segment_key(label: &EdgeLabel) -> Option<FlowchartLayoutSegmentKey> {
    let owner_index = flowchart_edge_owner_index(label)?;
    let self_loop_order = label
        .extras
        .get(SELF_LOOP_ORDER_EXTRA)
        .and_then(Value::as_u64)
        .and_then(|order| u8::try_from(order).ok());
    Some(FlowchartLayoutSegmentKey {
        owner_index,
        self_loop_order,
    })
}

fn flowchart_self_loop_segment_meta(label: &EdgeLabel) -> Option<FlowchartSelfLoopSegmentMeta> {
    let logical_edge_id = label.extras.get(SELF_LOOP_ID_EXTRA)?.as_str()?.to_string();
    let node_id = label
        .extras
        .get(SELF_LOOP_NODE_EXTRA)?
        .as_str()?
        .to_string();
    let order = u8::try_from(label.extras.get(SELF_LOOP_ORDER_EXTRA)?.as_u64()?).ok()?;
    Some(FlowchartSelfLoopSegmentMeta {
        logical_edge_id,
        owner_index: flowchart_edge_owner_index(label)?,
        node_id,
        order,
    })
}

fn merge_flowchart_self_loop_segments(
    model_edges: &[FlowEdge],
    layout_nodes: &[LayoutNode],
    rankdir: &str,
    layout_edges: Vec<FlowchartLayoutEdgeCandidate>,
) -> Vec<FlowchartLayoutEdgeCandidate> {
    let mut output = Vec::with_capacity(layout_edges.len());
    let mut segments_by_owner: IndexMap<usize, Vec<FlowchartLayoutEdgeCandidate>> = IndexMap::new();
    for candidate in layout_edges {
        if candidate.self_loop.is_some() {
            segments_by_owner
                .entry(candidate.owner_index)
                .or_default()
                .push(candidate);
        } else {
            output.push(candidate);
        }
    }

    let nodes_by_id: HashMap<&str, &LayoutNode> = layout_nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    for (owner_index, mut segments) in segments_by_owner {
        if segments.len() != 3 {
            output.extend(segments);
            continue;
        }
        segments.sort_by_key(|candidate| {
            candidate
                .self_loop
                .as_ref()
                .map_or(u8::MAX, |meta| meta.order)
        });
        if segments
            .iter()
            .filter_map(|candidate| candidate.self_loop.as_ref().map(|meta| meta.order))
            .ne([0_u8, 1, 2])
        {
            output.extend(segments);
            continue;
        }

        let first = &segments[0].edge;
        let first_meta = segments[0]
            .self_loop
            .as_ref()
            .expect("sorted self-loop segment metadata");
        let middle = &segments[1].edge;
        let last = &segments[2].edge;
        let Some(original) = model_edges.get(owner_index) else {
            output.extend(segments);
            continue;
        };
        let node_id = first_meta.node_id.as_str();
        let Some(node) = nodes_by_id.get(node_id).copied() else {
            output.extend(segments);
            continue;
        };

        let helper_ids = [
            format!("{node_id}---{node_id}---1"),
            format!("{node_id}---{node_id}---2"),
        ];
        let mut hints = helper_ids
            .iter()
            .filter_map(|id| nodes_by_id.get(id.as_str()).copied())
            .map(|helper| LayoutPoint {
                x: helper.x,
                y: helper.y,
            })
            .collect::<Vec<_>>();
        if hints.is_empty() {
            hints.extend(
                [first, middle, last]
                    .into_iter()
                    .flat_map(|edge| edge.points.iter().cloned()),
            );
        }

        let label_width = middle.label.as_ref().map_or(0.0, |label| label.width);
        let label_height = middle.label.as_ref().map_or(0.0, |label| label.height);
        let geometry = compact_self_loop_geometry(
            &LayoutPoint {
                x: node.x,
                y: node.y,
            },
            Size::new(node.width, node.height),
            rank_dir_from_flow(rankdir),
            &hints,
            0.0,
            Size::new(label_width, label_height),
        );
        let label = middle.label.as_ref().map(|_| LayoutLabel {
            x: geometry.label_center.x,
            y: geometry.label_center.y,
            width: label_width,
            height: label_height,
        });

        output.push(FlowchartLayoutEdgeCandidate {
            owner_index,
            self_loop: None,
            edge: LayoutEdge {
                id: original.id.clone(),
                from: original.from.clone(),
                to: original.to.clone(),
                from_cluster: first
                    .from_cluster
                    .clone()
                    .or_else(|| middle.from_cluster.clone())
                    .or_else(|| last.from_cluster.clone()),
                to_cluster: first
                    .to_cluster
                    .clone()
                    .or_else(|| middle.to_cluster.clone())
                    .or_else(|| last.to_cluster.clone()),
                points: geometry.points,
                label,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: None,
                end_marker: None,
                stroke_dasharray: None,
            },
        });
    }

    output
}

fn render_edge_label_for_layout<'a>(
    model: &'a FlowchartRenderModelRef<'_>,
    semantic_index: usize,
    edge: &'a FlowEdge,
    self_loop: Option<&FlowchartSelfLoopSegmentMeta>,
) -> Option<&'a str> {
    if self_loop.is_some() {
        edge.label.as_deref()
    } else {
        model.edge_label_for_render(semantic_index, edge)
    }
}

fn edge_label_is_non_empty(
    model: &FlowchartRenderModelRef<'_>,
    semantic_index: usize,
    edge: &FlowEdge,
    self_loop: Option<&FlowchartSelfLoopSegmentMeta>,
) -> bool {
    render_edge_label_for_layout(model, semantic_index, edge, self_loop)
        .is_some_and(|text| !crate::flowchart::flowchart_label_is_empty_for_render(text))
}

#[cfg(test)]
pub(crate) fn layout_flowchart_typed(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<FlowchartLayout> {
    layout_flowchart_typed_with_work_meter(
        model,
        effective_config,
        measurer,
        math_renderer,
        Arc::new(OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )),
    )
}

#[cfg(test)]
pub(crate) fn layout_flowchart_typed_with_work_meter(
    model: &FlowchartModel,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<FlowchartLayout> {
    let edge_style_plan = crate::test_support::prepare_flowchart_edge_style_plan(
        model,
        effective_config,
        false,
        work_meter.as_ref(),
    )?;
    layout(
        model,
        &FlowchartRenderContext::default(),
        &super::FlowchartClassDefinitions::new(&model.class_defs),
        effective_config,
        measurer,
        math_renderer,
        None,
        &edge_style_plan,
        work_meter,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Layout receives independent label artifacts and cumulative work controls"
)]
pub(crate) fn layout(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    class_defs: &super::FlowchartClassDefinitions<'_>,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    svg_label_sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
    work_meter: Arc<OperationWorkMeter>,
) -> Result<FlowchartLayout> {
    let mut work_control = OperationLayoutWorkControl::new(work_meter);
    layout_flowchart_with_model(
        model,
        render_label_sources,
        class_defs,
        effective_config,
        measurer,
        math_renderer,
        svg_label_sidecar,
        edge_style_plan,
        &mut work_control,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Layout receives independent label artifacts and cumulative work controls"
)]
fn layout_flowchart_with_model(
    model: &FlowchartModel,
    render_label_sources: &FlowchartRenderContext,
    class_defs: &super::FlowchartClassDefinitions<'_>,
    effective_config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    svg_label_sidecar: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_style_plan: &crate::svg::FlowchartEdgeStylePlan,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<FlowchartLayout> {
    let render_model = FlowchartRenderModelRef::new(model, render_label_sources);
    let model = &render_model;
    // Shape validation is an adapter-owned full node scan. Charge it before touching the model so
    // a low operation budget still bounds invalid inputs and preserves the old preflight's
    // resource-error precedence when the scan itself cannot be admitted.
    work_control.charge_adapter(model.nodes.len())?;
    super::validate_flowchart_model_shapes(model)?;

    // Project the lossless semantic edges through the collapsed-subgraph view before expanding
    // self-loops. Boundary edges use the visible collapsed node as their endpoint, while edges
    // wholly hidden by a collapsed subgraph are omitted.
    work_control.charge_adapter(model.edges.len())?;
    let mut projected_edges = Vec::with_capacity(model.edges.len());
    for (edge_index, edge) in model.edges.iter().enumerate() {
        if let Some(projected) = super::project_flowchart_edge(edge, render_label_sources) {
            projected_edges.push((edge_index, projected));
        }
    }
    work_control.charge_adapter(projected_edges.len())?;

    // Mermaid's dagre adapter expands self-loop edges into a chain of two special label nodes plus
    // three edges. This avoids `v == w` edges in Dagre and is required for SVG parity (Mermaid
    // uses `*-cyclic-special-*` ids when rendering self-loops).
    let self_loop_count = projected_edges
        .iter()
        .filter(|(_, edge)| edge.from == edge.to)
        .count();
    let derived_render_edges = work_control.checked_add(
        projected_edges.len(),
        work_control.checked_mul(self_loop_count, 2)?,
    )?;
    work_control.charge_adapter(derived_render_edges)?;
    let mut render_edges: Vec<std::borrow::Cow<'_, FlowEdge>> =
        Vec::with_capacity(derived_render_edges);
    let mut render_edge_self_loop_meta: Vec<Option<FlowchartSelfLoopSegmentMeta>> =
        Vec::with_capacity(derived_render_edges);
    let mut render_edge_owner_indices = Vec::with_capacity(derived_render_edges);
    let mut self_loop_label_node_ids: Vec<String> = Vec::new();
    let mut self_loop_label_node_id_set: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    let merge_self_loops = super::flowchart_dagre_merges_self_loop_segments(&model.keyword);
    let mut unmerged_first_slots = HashMap::new();
    for (edge_index, projected) in projected_edges {
        if projected.from != projected.to {
            render_edges.push(projected.into_render_edge());
            render_edge_self_loop_meta.push(None);
            render_edge_owner_indices.push(edge_index);
            continue;
        }

        let edge = projected.semantic;
        let mut helper_edges = super::flowchart_self_loop_helper_edges(edge);
        helper_edges.edge_mid.label = model
            .edge_label_for_render(edge_index, edge)
            .map(str::to_owned);
        if self_loop_label_node_id_set.insert(helper_edges.special_id_1.clone()) {
            self_loop_label_node_ids.push(helper_edges.special_id_1.clone());
        }
        if self_loop_label_node_id_set.insert(helper_edges.special_id_2.clone()) {
            self_loop_label_node_ids.push(helper_edges.special_id_2.clone());
        }

        // Mermaid clears the label text on the end segments, but keeps the label (if any) on the
        // mid edge (`edgeMid` is a structuredClone of the original edge without label changes).
        let first_slot = if merge_self_loops {
            None
        } else {
            work_control.charge_adapter(1)?;
            // Graphlib setEdge overwrites a parallel helper triple without moving its first
            // insertion position. Keep both its geometry and label owner at those same slots.
            Some(
                *unmerged_first_slots
                    .entry(edge.from.as_str())
                    .or_insert(render_edges.len()),
            )
        };
        for (order, segment) in [
            helper_edges.edge1,
            helper_edges.edge_mid,
            helper_edges.edge2,
        ]
        .into_iter()
        .enumerate()
        {
            let meta = Some(FlowchartSelfLoopSegmentMeta {
                logical_edge_id: edge.id.clone(),
                owner_index: edge_index,
                node_id: edge.from.clone(),
                order: order as u8,
            });
            let slot = first_slot
                .map(|first| work_control.checked_add(first, order))
                .transpose()?
                .unwrap_or(render_edges.len());
            if slot == render_edges.len() {
                render_edges.push(std::borrow::Cow::Owned(segment));
                render_edge_owner_indices.push(edge_index);
                render_edge_self_loop_meta.push(meta);
            } else {
                render_edges[slot] = std::borrow::Cow::Owned(segment);
                render_edge_owner_indices[slot] = edge_index;
                render_edge_self_loop_meta[slot] = meta;
            }
        }
    }
    let FlowchartLayoutSettings {
        nodesep,
        ranksep,
        node_padding,
        state_padding,
        wrapping_width,
        edge_label_wrapping_width,
        cluster_title_wrapping_width,
        edge_html_labels,
        node_wrap_mode,
        edge_wrap_mode,
        cluster_wrap_mode,
        cluster_padding,
        title_margin_top,
        title_margin_bottom,
        title_total_margin,
        y_shift,
        inherit_dir,
        text_style,
        html_label_text_style,
    } = svg_label_sidecar.map_or_else(
        || FlowchartConfigView::new(effective_config).layout_settings(),
        |sidecar| sidecar.layout_settings(effective_config),
    );
    let look = crate::config::config_diagram_look(effective_config);
    let look_is_neo = look.is_neo();
    let node_label_base_style = if node_wrap_mode == WrapMode::HtmlLike {
        &html_label_text_style
    } else {
        &text_style
    };
    let cluster_label_base_style = if cluster_wrap_mode == WrapMode::HtmlLike {
        &html_label_text_style
    } else {
        &text_style
    };
    let edge_label_base_style = if edge_wrap_mode == WrapMode::HtmlLike {
        &html_label_text_style
    } else {
        &text_style
    };

    let diagram_direction = normalize_dir(model.direction.as_deref().unwrap_or("TB"));
    let has_subgraphs = !model.subgraphs.is_empty();
    work_control.charge_adapter(model.subgraphs.len())?;
    // Parsed Mermaid 12.1 models already contain one canonical group per id. Public typed
    // callers can still construct duplicate groups; retain first-owner presentation and merged
    // membership for that supported input without rebuilding parser semantics here.
    let mut subgraphs_by_id: FlowSubgraphIndex<'_> = HashMap::with_capacity(model.subgraphs.len());
    let mut subgraph_index_by_id: HashMap<&str, usize> =
        HashMap::with_capacity(model.subgraphs.len());
    let mut canonical_subgraphs_in_order: Vec<(usize, &FlowSubgraph)> = Vec::new();
    let mut subgraph_members_by_id: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut nonempty_subgraph_ids: HashSet<&str> = HashSet::new();
    for (index, subgraph) in model.subgraphs.iter().enumerate() {
        let id = subgraph.id.as_str();
        if !subgraphs_by_id.contains_key(id) {
            subgraphs_by_id.insert(id, subgraph);
            subgraph_index_by_id.insert(id, index);
            canonical_subgraphs_in_order.push((index, subgraph));
        }
        subgraph_members_by_id
            .entry(id)
            .or_default()
            .extend(subgraph.nodes.iter().map(String::as_str));
        if !subgraph.nodes.is_empty() {
            nonempty_subgraph_ids.insert(id);
        }
    }
    work_control.charge_adapter(model.subgraphs.len())?;
    let subgraph_ids: std::collections::HashSet<&str> =
        model.subgraphs.iter().map(|sg| sg.id.as_str()).collect();
    let collapsed_subgraph_ids: HashSet<&str> = model
        .subgraphs
        .iter()
        .filter(|sg| model.is_subgraph_collapsed(sg.id.as_str()))
        .filter(|sg| model.collapsed_replacement(sg.id.as_str()).is_none())
        .map(|sg| sg.id.as_str())
        .collect();
    // Only expanded, non-empty subgraphs participate in Dagre's compound-cluster pipeline.
    // Collapsed subgraphs are ordinary leaf nodes; nested collapsed/expanded descendants replaced
    // by an outer collapsed ancestor are omitted from the graph entirely.
    let cluster_subgraph_ids: HashSet<&str> = model
        .subgraphs
        .iter()
        .filter(|sg| !collapsed_subgraph_ids.contains(sg.id.as_str()))
        .filter(|sg| model.collapsed_replacement(sg.id.as_str()).is_none())
        .filter(|sg| !sg.nodes.is_empty())
        .map(|sg| sg.id.as_str())
        .collect();
    let mut g: Graph<NodeLabel, EdgeLabel, GraphLabel> = Graph::new(GraphOptions {
        multigraph: true,
        // Mermaid's Dagre adapter always enables `compound: true`, even if there are no explicit
        // subgraphs. This also allows `nestingGraph.run` to connect components during ranking.
        compound: true,
        directed: true,
    });
    g.set_graph(GraphLabel {
        rankdir: rank_dir_from_flow(&diagram_direction),
        nodesep,
        ranksep,
        marginx: 8.0,
        marginy: 8.0,
        acyclicer: None,
        ..Default::default()
    });

    let mut empty_subgraph_ids: Vec<String> = Vec::new();
    let mut cluster_node_labels: std::collections::HashMap<String, NodeLabel> =
        std::collections::HashMap::new();
    work_control.charge_adapter(canonical_subgraphs_in_order.len())?;
    for (_, sg) in &canonical_subgraphs_in_order {
        if model.collapsed_replacement(sg.id.as_str()).is_some() {
            continue;
        }
        if collapsed_subgraph_ids.contains(sg.id.as_str()) {
            continue;
        }
        if !nonempty_subgraph_ids.contains(sg.id.as_str()) {
            // Mermaid renders empty subgraphs as regular nodes. Keep the semantic `subgraph`
            // definition around for styling/title, but size + lay it out as a leaf node.
            empty_subgraph_ids.push(sg.id.clone());
            continue;
        }
        // Mermaid does not pre-size compound (subgraph) nodes based on title metrics for Dagre
        // layout. Their dimensions are computed from children (border nodes) and then adjusted at
        // render time for title width and configured margins.
        cluster_node_labels.insert(sg.id.clone(), NodeLabel::default());
    }

    let mut leaf_node_labels: std::collections::HashMap<String, NodeLabel> =
        std::collections::HashMap::new();
    let mut leaf_label_metrics_by_id: HashMap<String, (f64, f64)> = HashMap::new();
    let collapsed_leaf_count = collapsed_subgraph_ids.len();
    let leaf_label_capacity = work_control.checked_add(
        model.nodes.len(),
        work_control.checked_add(empty_subgraph_ids.len(), collapsed_leaf_count)?,
    )?;
    work_control.charge_adapter(leaf_label_capacity)?;
    leaf_label_metrics_by_id.reserve(leaf_label_capacity);
    work_control.charge_adapter(model.nodes.len())?;
    for (node_index, n) in model.nodes.iter().enumerate() {
        // Mermaid treats the subgraph id as the "group node" id (a cluster can be referenced in
        // edges). Avoid introducing a separate leaf node that would collide with the cluster node
        // of the same id.
        if subgraph_ids.contains(n.id.as_str())
            || model.collapsed_replacement(n.id.as_str()).is_some()
        {
            continue;
        }
        super::validate_flowchart_node_asset_dimensions(n)?;
        let raw_label = model.node_label_for_render(n).unwrap_or(&n.id);
        let wrapping_width = super::bpmn::wrapping_width(n.layout_shape.as_deref(), wrapping_width);
        let label_type = n.label_type.as_deref().unwrap_or("text");
        let owner = FlowchartSvgLabelOwner::Node(node_index);
        let label_dom = (
            edge_html_labels,
            super::FlowchartLabelDom::node(
                n.layout_shape.as_deref().unwrap_or("squareRect"),
                raw_label,
                label_type,
                node_wrap_mode == WrapMode::HtmlLike,
                look.is_hand_drawn(),
            ),
        );
        let structural = super::FlowchartStructuralLabelTypography::prepare(
            class_defs,
            label_dom.0,
            label_dom.1,
            |units| work_control.charge_adapter(units),
        )?;
        let node_parent =
            structural.apply_to_parent(std::borrow::Cow::Borrowed(node_label_base_style));
        let (mut node_text_style, rectangle_size) = super::style::flowchart_node_layout_style(
            node_parent.as_ref(),
            &model.class_defs,
            &n.classes,
            &n.styles,
        );
        structural.apply_to_content(&mut node_text_style.style);
        let svg_width_mode = flowchart_node_svg_width_mode(
            raw_label,
            label_type,
            node_wrap_mode,
            n.layout_shape.as_deref().unwrap_or("squareRect"),
        );
        let mut metrics = measure_flowchart_svg_label_for_layout_with_source_style(
            svg_label_sidecar,
            Some(owner),
            Some(n.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer,
                raw_label,
                label_type,
                style: node_text_style.as_ref(),
                max_width_px: Some(wrapping_width),
                wrap_mode: node_wrap_mode,
                config: effective_config,
                math_renderer,
            },
            Some(&node_text_style),
            svg_width_mode,
        );
        if node_wrap_mode == WrapMode::HtmlLike && edge_html_labels {
            flowchart_apply_html_node_class_box_metrics(
                &mut metrics,
                raw_label,
                label_type,
                node_text_style.as_ref(),
                &model.class_defs,
                &n.classes,
            );
        }
        metrics = metrics.with_label_min_width(
            raw_label,
            super::flowchart_node_label_min_width(
                raw_label,
                n.layout_shape.as_deref(),
                effective_config,
            ),
            None,
        );
        leaf_label_metrics_by_id.insert(n.id.clone(), (metrics.width, metrics.height));
        let ((width, height), _) = NodeLayoutDimensionsRequest {
            layout_shape: n.layout_shape.as_deref(),
            layout_direction: &diagram_direction,
            metrics,
            has_label: !raw_label.is_empty(),
            wrapping_width,
            node_constraint: n.constraint.as_deref(),
            padding: node_padding,
            look_is_neo,
            state_padding,
            node_icon: n.icon.as_deref(),
            node_img: n.img.as_deref(),
            node_pos: n.pos.as_deref(),
            node_asset_width: n.asset_width,
            node_asset_height: n.asset_height,
        }
        .measure(
            rectangle_size,
            look.is_hand_drawn(),
            svg_label_sidecar.map(|builder| (builder, owner)),
            |units| work_control.charge_adapter(units),
        )?;
        leaf_node_labels.insert(
            n.id.clone(),
            NodeLabel {
                width,
                height,
                ..Default::default()
            },
        );
    }
    work_control.charge_adapter(canonical_subgraphs_in_order.len())?;
    for &(subgraph_index, sg) in &canonical_subgraphs_in_order {
        if nonempty_subgraph_ids.contains(sg.id.as_str())
            || collapsed_subgraph_ids.contains(sg.id.as_str())
            || model.collapsed_replacement(sg.id.as_str()).is_some()
        {
            continue;
        }
        let label_type = sg.label_type.as_deref().unwrap_or("text");
        let (classes, styles) = model.effective_subgraph_css(subgraph_index, sg);
        let title = model.subgraph_title_for_render(subgraph_index, sg);
        let label_dom = (
            edge_html_labels,
            super::FlowchartLabelDom::node(
                "squareRect",
                title,
                label_type,
                node_wrap_mode == WrapMode::HtmlLike,
                crate::config::config_diagram_look(effective_config).is_hand_drawn(),
            ),
        );
        let structural = super::FlowchartStructuralLabelTypography::prepare(
            class_defs,
            label_dom.0,
            label_dom.1,
            |units| work_control.charge_adapter(units),
        )?;
        let parent_style =
            structural.apply_to_parent(std::borrow::Cow::Borrowed(cluster_label_base_style));
        let mut sg_text_style = flowchart_effective_text_style_for_classes_with_provenance(
            parent_style.as_ref(),
            &model.class_defs,
            classes,
            styles,
        );
        structural.apply_to_content(&mut sg_text_style.style);
        // Mermaid renders an empty subgraph through the ordinary node `labelHelper`: wrapping
        // probes use `flowchart.wrappingWidth` and `getComputedTextLength()`, while the final label
        // dimensions come from `getBBox()`. Selecting `ComputedLength` here would add a post-wrap
        // per-line measurement pass that is absent upstream.
        let mut metrics = measure_flowchart_svg_label_for_layout_with_source_style(
            svg_label_sidecar,
            Some(FlowchartSvgLabelOwner::EmptySubgraphNode(subgraph_index)),
            Some(sg.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer,
                raw_label: title,
                label_type,
                style: sg_text_style.as_ref(),
                max_width_px: Some(wrapping_width),
                wrap_mode: node_wrap_mode,
                config: effective_config,
                math_renderer,
            },
            Some(&sg_text_style),
            FlowchartSvgWidthMode::Bbox,
        );
        if node_wrap_mode == WrapMode::HtmlLike && edge_html_labels {
            flowchart_apply_html_node_class_box_metrics(
                &mut metrics,
                title,
                label_type,
                sg_text_style.as_ref(),
                &model.class_defs,
                classes,
            );
        }
        leaf_label_metrics_by_id.insert(sg.id.clone(), (metrics.width, metrics.height));
        let ((width, height), _) = NodeLayoutDimensionsRequest {
            layout_shape: Some("squareRect"),
            layout_direction: &diagram_direction,
            metrics,
            has_label: !title.is_empty(),
            wrapping_width,
            node_constraint: None,
            padding: cluster_padding,
            look_is_neo: false,
            state_padding,
            node_icon: None,
            node_img: None,
            node_pos: None,
            node_asset_width: None,
            node_asset_height: None,
        }
        .measure([None; 2], false, None, |_| Ok(()))?;
        leaf_node_labels.insert(
            sg.id.clone(),
            NodeLabel {
                width,
                height,
                ..Default::default()
            },
        );
    }

    // A collapsed subgraph is laid out as a regular leaf node. Its title uses the same label
    // preparation path as a subgraph title, but its outer dimensions reserve the separator and
    // indicator rows rendered by `collapsedGroup`.
    work_control.charge_adapter(collapsed_leaf_count)?;
    for &(subgraph_index, sg) in &canonical_subgraphs_in_order {
        if !collapsed_subgraph_ids.contains(sg.id.as_str()) {
            continue;
        }
        let label_type = sg.label_type.as_deref().unwrap_or("text");
        let (classes, styles) = model.effective_subgraph_css(subgraph_index, sg);
        let title = model.subgraph_title_for_render(subgraph_index, sg);
        let label_dom = (
            edge_html_labels,
            super::FlowchartLabelDom::node(
                "collapsedGroup",
                title,
                label_type,
                node_wrap_mode == WrapMode::HtmlLike,
                crate::config::config_diagram_look(effective_config).is_hand_drawn(),
            ),
        );
        let structural = super::FlowchartStructuralLabelTypography::prepare(
            class_defs,
            label_dom.0,
            label_dom.1,
            |units| work_control.charge_adapter(units),
        )?;
        let base_style = if node_wrap_mode == WrapMode::HtmlLike {
            &html_label_text_style
        } else {
            &text_style
        };
        let base_style = structural.apply_to_parent(std::borrow::Cow::Borrowed(base_style));
        let mut text_style_for_label = flowchart_effective_text_style_for_classes(
            base_style.as_ref(),
            &model.class_defs,
            classes,
            styles,
        );
        structural.apply_to_content(&mut text_style_for_label);
        let mut metrics = measure_flowchart_svg_label_for_layout_with_source_style(
            svg_label_sidecar,
            Some(FlowchartSvgLabelOwner::SubgraphTitle(subgraph_index)),
            Some(sg.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer,
                raw_label: title,
                label_type,
                style: text_style_for_label.as_ref(),
                max_width_px: Some(wrapping_width),
                wrap_mode: node_wrap_mode,
                config: effective_config,
                math_renderer,
            },
            None,
            FlowchartSvgWidthMode::Bbox,
        );
        if node_wrap_mode == WrapMode::HtmlLike && edge_html_labels {
            flowchart_apply_html_node_class_box_metrics(
                &mut metrics,
                title,
                label_type,
                text_style_for_label.as_ref(),
                &model.class_defs,
                classes,
            );
        }
        leaf_label_metrics_by_id.insert(sg.id.clone(), (metrics.width, metrics.height));
        leaf_node_labels.insert(
            sg.id.clone(),
            NodeLabel {
                width: (metrics.width + 16.0).max(80.0),
                height: metrics.height + 44.0,
                ..Default::default()
            },
        );
    }

    // Mermaid constructs the Dagre graph by:
    // 1) inserting subgraph (cluster) nodes first (in reverse `subgraphs[]` order), then
    // 2) inserting vertex nodes (in FlowDB `Map` insertion order),
    // and setting `parentId` as each node is inserted.
    //
    // Matching property creation matters because Graphlib exposes JavaScript `Object.keys`
    // order: array-index IDs enumerate numerically before ordinary IDs in creation order. That
    // order affects compound children, anchor selection, and deterministic layout tie-breaking.
    let mut inserted: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut parent_assigned: std::collections::HashSet<String> = std::collections::HashSet::new();
    let insert_one = |id: &str,
                      g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
                      inserted: &mut std::collections::HashSet<String>,
                      work_control: &mut OperationLayoutWorkControl|
     -> Result<()> {
        if inserted.contains(id) {
            return Ok(());
        }
        if let Some(lbl) = cluster_node_labels.get(id).cloned() {
            charge_node_insertion_ordering(g, id, work_control)?;
            g.set_node(id.to_string(), lbl);
            inserted.insert(id.to_string());
            return Ok(());
        }
        if let Some(lbl) = leaf_node_labels.get(id).cloned() {
            charge_node_insertion_ordering(g, id, work_control)?;
            g.set_node(id.to_string(), lbl);
            inserted.insert(id.to_string());
        }
        Ok(())
    };

    let mut existing_parent_count = 0usize;
    if has_subgraphs {
        // Match Mermaid's `FlowDB.getData()` parent assignment: build `parentId` by iterating
        // subgraphs in reverse order and recording each membership.
        let mut parent_by_id: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let membership_count = model.subgraphs.iter().try_fold(0usize, |total, subgraph| {
            work_control.checked_add(total, subgraph.nodes.len())
        })?;
        let hierarchy_validation_work =
            work_control.checked_add(membership_count, model.subgraphs.len())?;
        work_control.charge_adapter(hierarchy_validation_work)?;
        for sg in model.subgraphs.iter().rev() {
            for child in &sg.nodes {
                parent_by_id.insert(child.clone(), sg.id.clone());
            }
        }

        let insert_with_parent = |id: &str,
                                  g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
                                  inserted: &mut std::collections::HashSet<String>,
                                  parent_assigned: &mut std::collections::HashSet<String>,
                                  parent_assignments: &mut Vec<(usize, usize)>,
                                  numeric_parent_assignments: &mut usize,
                                  work_control: &mut OperationLayoutWorkControl|
         -> Result<()> {
            insert_one(id, g, inserted, work_control)?;
            if !parent_assigned.insert(id.to_string()) {
                return Ok(());
            }
            if let Some(parent) = parent_by_id.get(id) {
                charge_node_insertion_ordering(g, parent, work_control)?;
                g.ensure_node_ref(parent);
                if let (Some(child_ix), Some(parent_ix)) = (g.node_ix(id), g.node_ix(parent)) {
                    parent_assignments.push((child_ix, parent_ix));
                    *numeric_parent_assignments += usize::from(is_javascript_array_index(id));
                }
            }
            Ok(())
        };

        let insertion_visits = work_control.checked_add(
            work_control.checked_add(model.subgraphs.len(), model.nodes.len())?,
            model.vertex_calls.len(),
        )?;
        let insertion_work = work_control.checked_add(insertion_visits, parent_by_id.len())?;
        work_control.charge_adapter(insertion_work)?;
        let mut parent_assignments = Vec::with_capacity(parent_by_id.len());
        let mut numeric_parent_assignments = 0usize;
        for sg in model.subgraphs.iter().rev() {
            if model.collapsed_replacement(sg.id.as_str()).is_some() {
                continue;
            }
            insert_with_parent(
                sg.id.as_str(),
                &mut g,
                &mut inserted,
                &mut parent_assigned,
                &mut parent_assignments,
                &mut numeric_parent_assignments,
                work_control,
            )?;
        }
        for n in &model.nodes {
            if model.collapsed_replacement(n.id.as_str()).is_some() {
                continue;
            }
            insert_with_parent(
                n.id.as_str(),
                &mut g,
                &mut inserted,
                &mut parent_assigned,
                &mut parent_assignments,
                &mut numeric_parent_assignments,
                work_control,
            )?;
        }
        for id in &model.vertex_calls {
            if model.collapsed_replacement(id.as_str()).is_some() {
                continue;
            }
            insert_with_parent(
                id.as_str(),
                &mut g,
                &mut inserted,
                &mut parent_assigned,
                &mut parent_assignments,
                &mut numeric_parent_assignments,
                work_control,
            )?;
        }
        apply_unparented_parent_assignments(
            &mut g,
            &parent_assignments,
            UnparentedParentBatchShape {
                existing_parent_count: 0,
                numeric_assignment_count: numeric_parent_assignments,
            },
            work_control,
        )?;
        existing_parent_count = parent_assignments.len();
    } else {
        // No subgraphs: insertion order still matters for deterministic Dagre tie-breaking.
        let insertion_work =
            work_control.checked_add(model.nodes.len(), model.vertex_calls.len())?;
        work_control.charge_adapter(insertion_work)?;
        for n in &model.nodes {
            insert_one(n.id.as_str(), &mut g, &mut inserted, work_control)?;
        }
        for id in &model.vertex_calls {
            insert_one(id.as_str(), &mut g, &mut inserted, work_control)?;
        }
    }

    // Materialize self-loop helper label nodes and place them in the same parent cluster as the
    // base node (if any), matching Mermaid `@11.12.2` dagre layout adapter behavior.
    work_control.charge_adapter(self_loop_label_node_ids.len())?;
    let mut self_loop_parent_assignments = Vec::new();
    let mut numeric_self_loop_parent_assignments = 0usize;
    for id in &self_loop_label_node_ids {
        if !g.has_node(id) {
            charge_node_insertion_ordering(&g, id, work_control)?;
            g.set_node(
                id.clone(),
                NodeLabel {
                    // Mermaid initializes these labelRect nodes at 10x10, but then immediately
                    // runs `insertNode(...)` + `updateNodeBounds(...)` before Dagre layout. For an
                    // empty `labelRect`, the measured bbox collapses to ~0.1x0.1 and that is what
                    // Dagre actually sees for spacing. Match that here for layout parity.
                    width: 0.1_f32 as f64,
                    height: 0.1_f32 as f64,
                    ..Default::default()
                },
            );
        }
        let Some((base, _)) = id.split_once("---") else {
            continue;
        };
        if let Some(p) = g.parent(base) {
            let child_ix = g
                .node_ix(id)
                .expect("a self-loop helper node is present after insertion");
            let parent_ix = g
                .node_ix(p)
                .expect("the base node parent is present in the layout graph");
            self_loop_parent_assignments.push((child_ix, parent_ix));
            numeric_self_loop_parent_assignments += usize::from(is_javascript_array_index(id));
        }
    }
    apply_unparented_parent_assignments(
        &mut g,
        &self_loop_parent_assignments,
        UnparentedParentBatchShape {
            existing_parent_count,
            numeric_assignment_count: numeric_self_loop_parent_assignments,
        },
        work_control,
    )?;

    let effective_dir_by_id = if has_subgraphs {
        compute_effective_dir_by_id(
            &model.subgraphs,
            &subgraphs_by_id,
            &g,
            &diagram_direction,
            inherit_dir,
            work_control,
        )?
    } else {
        HashMap::new()
    };

    work_control.charge_adapter(render_edges.len())?;
    for ((e, self_loop_meta), edge_owner_index) in render_edges
        .iter()
        .zip(&render_edge_self_loop_meta)
        .zip(&render_edge_owner_indices)
    {
        // Mermaid 11.16 stores helper identity as edge metadata. The graph key is intentionally
        // node-scoped, so a later parallel self-loop overwrites the earlier triple in Graphlib.
        let edge_key = flowchart_layout_edge_key(e, self_loop_meta.as_ref());
        let owner_index = checked_flowchart_edge_owner_index(*edge_owner_index, work_control)?;

        let from = e.from.clone();
        let to = e.to.clone();

        if edge_label_is_non_empty(model, *edge_owner_index, e, self_loop_meta.as_ref()) {
            let label_text =
                render_edge_label_for_layout(model, *edge_owner_index, e, self_loop_meta.as_ref())
                    .unwrap_or_default();
            let label_type = e.label_type.as_deref().unwrap_or("text");
            let edge_key_identity = crate::flowchart::FlowchartEdgeKey::new(*edge_owner_index);
            let edge_text_style = edge_style_plan
                .edge_label_text_style_for(edge_key_identity, edge_label_base_style)?;
            let metrics = if label_type == "markdown" && edge_wrap_mode != WrapMode::HtmlLike {
                crate::text::measure_wrapped_markdown_with_inline_styles(
                    measurer,
                    label_text,
                    edge_text_style.as_ref(),
                    Some(edge_label_wrapping_width),
                    edge_wrap_mode,
                )
            } else if edge_wrap_mode == WrapMode::SvgLike {
                let render_id = self_loop_meta
                    .as_ref()
                    .map_or(e.id.as_str(), |meta| meta.logical_edge_id.as_str());
                measure_flowchart_svg_label_for_layout_with_metrics_and_source_style(
                    svg_label_sidecar,
                    Some(FlowchartSvgLabelOwner::Edge(*edge_owner_index)),
                    Some(render_id),
                    FlowchartLabelMetricsRequest {
                        measurer,
                        raw_label: label_text,
                        label_type,
                        // Mermaid wraps the temporary SVG text before applying `labelStyle`.
                        style: edge_label_base_style,
                        max_width_px: Some(edge_label_wrapping_width),
                        wrap_mode: edge_wrap_mode,
                        config: effective_config,
                        math_renderer,
                    },
                    edge_text_style.as_ref(),
                    Some(&edge_text_style),
                    FlowchartSvgWidthMode::Bbox,
                )
            } else {
                measure_flowchart_svg_label_for_layout_with_source_style(
                    svg_label_sidecar,
                    Some(FlowchartSvgLabelOwner::Edge(*edge_owner_index)),
                    Some(e.id.as_str()),
                    FlowchartLabelMetricsRequest {
                        measurer,
                        raw_label: label_text,
                        label_type,
                        style: edge_text_style.as_ref(),
                        max_width_px: Some(edge_label_wrapping_width),
                        wrap_mode: edge_wrap_mode,
                        config: effective_config,
                        math_renderer,
                    },
                    Some(&edge_text_style),
                    FlowchartSvgWidthMode::Bbox,
                )
            };
            let (label_width, label_height) = if edge_html_labels {
                (metrics.width.max(1.0), metrics.height.max(1.0))
            } else {
                // Mermaid's SVG edge-labels include a padded background rect (+2px left/right and
                // +2px top/bottom).
                (
                    (metrics.width + 4.0).max(1.0),
                    (metrics.height + 4.0).max(1.0),
                )
            };

            let minlen = e.length.max(1);
            let mut el = EdgeLabel {
                width: label_width,
                height: label_height,
                labelpos: LabelPos::C,
                // Dagre layout defaults `labeloffset` to 10 when unspecified.
                labeloffset: 10.0,
                minlen,
                weight: 1.0,
                owner_index: Some(owner_index),
                ..Default::default()
            };
            if let Some(meta) = self_loop_meta {
                annotate_flowchart_self_loop_segment(&mut el, meta);
            }
            g.set_edge_named(from, to, Some(edge_key), Some(el));
        } else {
            let mut el = EdgeLabel {
                width: 0.0,
                height: 0.0,
                labelpos: LabelPos::C,
                // Dagre layout defaults `labeloffset` to 10 when unspecified.
                labeloffset: 10.0,
                minlen: e.length.max(1),
                weight: 1.0,
                owner_index: Some(owner_index),
                ..Default::default()
            };
            if let Some(meta) = self_loop_meta {
                annotate_flowchart_self_loop_segment(&mut el, meta);
            }
            g.set_edge_named(from, to, Some(edge_key), Some(el));
        }
    }

    let external_connections_by_id = if has_subgraphs {
        adjust_flowchart_clusters_and_edges(&mut g, work_control)?
    } else {
        std::collections::HashMap::new()
    };

    let mut edge_endpoints_by_segment: HashMap<FlowchartLayoutSegmentKey, (String, String)> =
        HashMap::new();
    let mut edge_key_by_segment: HashMap<FlowchartLayoutSegmentKey, String> = HashMap::new();
    let edge_snapshot_work = work_control.checked_add(g.edge_slot_count(), g.edge_count())?;
    work_control.charge_adapter(edge_snapshot_work)?;
    for ek in g.edge_keys() {
        let Some(edge_key) = ek.name.as_deref() else {
            continue;
        };
        let Some(label) = g.edge_by_key(&ek) else {
            continue;
        };
        let Some(segment_key) = flowchart_layout_segment_key(label) else {
            continue;
        };
        edge_key_by_segment.insert(segment_key, edge_key.to_string());
        edge_endpoints_by_segment.insert(segment_key, (ek.v.clone(), ek.w.clone()));
    }

    let mut extracted_graphs: std::collections::HashMap<
        String,
        Graph<NodeLabel, EdgeLabel, GraphLabel>,
    > = std::collections::HashMap::new();
    let mut extracted_order = Vec::new();
    if has_subgraphs {
        extract_clusters_recursively(
            &mut g,
            |id| {
                subgraphs_by_id
                    .get(id)
                    .and_then(|subgraph| subgraph.dir.as_deref())
            },
            &external_connections_by_id,
            &mut extracted_graphs,
            &mut extracted_order,
            work_control,
        )?;
        // Cluster extraction can rebind a cross-boundary edge to the cluster node. Refresh root
        // endpoints after extraction so output lookup uses the surviving nodes.
        let edge_snapshot_work = work_control.checked_add(g.edge_slot_count(), g.edge_count())?;
        work_control.charge_adapter(edge_snapshot_work)?;
        edge_key_by_segment.clear();
        edge_endpoints_by_segment.clear();
        for ek in g.edge_keys() {
            let Some(edge_key) = ek.name.as_deref() else {
                continue;
            };
            let Some(label) = g.edge_by_key(&ek) else {
                continue;
            };
            let Some(segment_key) = flowchart_layout_segment_key(label) else {
                continue;
            };
            edge_key_by_segment.insert(segment_key, edge_key.to_string());
            edge_endpoints_by_segment.insert(segment_key, (ek.v, ek.w));
        }
    }

    // Mermaid's flowchart-v2 renderer inserts node DOM elements in pinned Graphlib
    // `graph.nodes()` order before Dagre layout, including recursively extracted cluster graphs.
    // This per-root map is the render plan consumed by the SVG renderer, hierarchy ownership, and
    // viewBox calculation; capture it from the same extracted graphs used for layout.
    let mut dom_node_order_by_root: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    let mut dom_order_work = node_id_snapshot_work_upper_bound(&g, work_control)?;
    for id in &extracted_order {
        let graph = extracted_graphs
            .get(id)
            .expect("the extraction order references an extracted graph");
        let graph_work = node_id_snapshot_work_upper_bound(graph, work_control)?;
        dom_order_work = work_control.checked_add(dom_order_work, graph_work)?;
    }
    work_control.charge_adapter(dom_order_work)?;
    dom_node_order_by_root.insert(String::new(), g.node_ids());
    for id in &extracted_order {
        let cg = extracted_graphs
            .get(id)
            .expect("the extraction order references an extracted graph");
        dom_node_order_by_root.insert(id.clone(), cg.node_ids());
    }
    type Rect = crate::geom::Box2;

    // A recursive cluster becomes a leaf in the parent graph. Keep its inner frame and the
    // bounds of the rendered group separate so a CSS resize does not scale or recenter children.
    #[derive(Clone, Copy)]
    struct StyledClusterGeometry {
        frame: Rect,
        group: Rect,
    }

    impl StyledClusterGeometry {
        fn child_offset(self, parent_x: f64, parent_y: f64) -> (f64, f64) {
            (
                parent_x - self.group.width().max(1.0) / 2.0 - self.frame.min_x(),
                parent_y - self.group.height().max(1.0) / 2.0 - self.frame.min_y(),
            )
        }
    }

    let mut subgraph_css_sizes = HashMap::new();
    if !extracted_graphs.is_empty() && !look.is_hand_drawn() && model.keyword != "agentflow-beta" {
        work_control.charge_adapter(canonical_subgraphs_in_order.len())?;
        for &(index, subgraph) in &canonical_subgraphs_in_order {
            if !cluster_subgraph_ids.contains(subgraph.id.as_str()) {
                continue;
            }
            let (classes, styles) = model.effective_subgraph_css(index, subgraph);
            let size =
                super::style::flowchart_subgraph_css_size(&model.class_defs, classes, styles);
            if size != [None; 2] {
                subgraph_css_sizes.insert(subgraph.id.as_str(), size);
            }
        }
    }

    struct ClusterTitleMetricsContext<'a> {
        model: &'a FlowchartRenderModelRef<'a>,
        subgraphs_by_id: &'a FlowSubgraphIndex<'a>,
        subgraph_index_by_id: &'a HashMap<&'a str, usize>,
        class_defs: &'a indexmap::IndexMap<String, Vec<String>>,
        measurer: &'a dyn TextMeasurer,
        text_style: &'a TextStyle,
        html_label_text_style: &'a TextStyle,
        title_wrapping_width: f64,
        wrap_mode: WrapMode,
        config: &'a PresentationConfig,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
    }

    fn cluster_title_metrics_for_layout(
        id: &str,
        ctx: &ClusterTitleMetricsContext<'_>,
    ) -> Option<(f64, f64)> {
        let sg = ctx.subgraphs_by_id.get(id)?;
        let subgraph_index = ctx.subgraph_index_by_id.get(id).copied()?;
        let title = ctx.model.subgraph_title_for_render(subgraph_index, sg);
        let label_type = sg.label_type.as_deref().unwrap_or("text");
        let title_width_limit = (label_type == "markdown").then_some(ctx.title_wrapping_width);
        let base_style = if ctx.wrap_mode == WrapMode::HtmlLike {
            ctx.html_label_text_style
        } else {
            ctx.text_style
        };
        let (classes, styles) = ctx.model.effective_subgraph_css(subgraph_index, sg);
        let text_style = flowchart_effective_text_style_for_classes_with_provenance(
            base_style,
            ctx.class_defs,
            classes,
            styles,
        );
        let owner = Some(FlowchartSvgLabelOwner::SubgraphTitle(subgraph_index));
        let metrics = measure_flowchart_svg_label_for_layout_with_source_style(
            ctx.svg_label_sidecar,
            owner,
            Some(id),
            FlowchartLabelMetricsRequest {
                measurer: ctx.measurer,
                raw_label: title,
                label_type,
                style: text_style.as_ref(),
                max_width_px: title_width_limit,
                wrap_mode: ctx.wrap_mode,
                config: ctx.config,
                math_renderer: ctx.math_renderer,
            },
            Some(&text_style),
            FlowchartSvgWidthMode::Bbox,
        );
        Some((metrics.width.max(1.0), metrics.height.max(1.0)))
    }

    fn measure_extracted_graph(
        cluster_id: &str,
        ctx: &mut RecursiveLayoutContext<'_>,
    ) -> Result<(Option<Rect>, Option<StyledClusterGeometry>)> {
        let Some(g) = ctx.extracted.get(cluster_id) else {
            return Ok((None, None));
        };
        let node_snapshot_work = node_id_snapshot_work_upper_bound(g, ctx.work_control)?;
        ctx.work_control.charge_adapter(node_snapshot_work)?;
        let mut group = None::<Rect>;
        let mut include = |rect: Rect| {
            if let Some(group) = &mut group {
                group.union(rect);
            } else {
                group = Some(rect);
            }
        };
        let mut frame = None;
        let mut used_css_size = false;
        for id in g.node_ids() {
            let Some(n) = g.node(&id) else { continue };
            let (Some(x), Some(y)) = (n.x, n.y) else {
                continue;
            };
            if g.child_count(&id) == 0 {
                if let Some(geometry) = ctx.styled_clusters.get(id.as_str()) {
                    let (dx, dy) = geometry.child_offset(x, y);
                    let mut child = geometry.group;
                    child.translate(dx, dy);
                    include(child);
                    used_css_size = true;
                } else {
                    include(Rect::from_center(x, y, n.width, n.height));
                }
                continue;
            }

            let mut width = n.width;
            let mut height = n.height;
            let mut title = None;
            if ctx.subgraph_id_set.contains(id.as_str()) {
                // The cluster frame grows for the title and margins after the inner Dagre layout.
                if ctx.title_total_margin > 0.0 {
                    height = (height + ctx.title_total_margin).max(1.0);
                }
                title = cluster_title_metrics_for_layout(&id, ctx.title_metrics_ctx);
                if let Some((title_w, title_h)) = title {
                    width = width.max(title_w + ctx.cluster_padding);
                    height = height.max(title_h + ctx.title_total_margin);
                }
            }
            let rect = Rect::from_center(x, y, width, height);
            if id == cluster_id {
                frame = Some(rect);
            }
            let Some(&[css_width, css_height]) = ctx.subgraph_css_sizes.get(id.as_str()) else {
                include(rect);
                continue;
            };
            used_css_size = true;
            let left = rect.min_x();
            let top = rect.min_y();
            let width = css_width.unwrap_or(width);
            let height = css_height.unwrap_or(height);
            if width > 0.0 && height > 0.0 {
                include(Rect::from_min_max(left, top, left + width, top + height));
            }
            if let Some((title_w, title_h)) = title {
                include(Rect::from_center(
                    x,
                    top + ctx.title_margin_top + title_h / 2.0,
                    title_w,
                    title_h,
                ));
            }
        }
        let edge_work = ctx
            .work_control
            .checked_add(g.edge_slot_count(), g.edge_count())?;
        ctx.work_control.charge_adapter(edge_work)?;
        for key in g.edge_keys() {
            let Some(edge) = g.edge_by_key(&key) else {
                continue;
            };
            if let (Some(x), Some(y)) = (edge.x, edge.y)
                && (edge.width > 0.0 || edge.height > 0.0)
            {
                include(Rect::from_center(x, y, edge.width, edge.height));
            }
        }
        let styled = if used_css_size {
            frame
                .zip(group)
                .map(|(frame, group)| StyledClusterGeometry { frame, group })
        } else {
            None
        };
        Ok((group, styled))
    }

    fn apply_mermaid_subgraph_title_shifts(
        graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        extracted: &std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        subgraph_id_set: &std::collections::HashSet<&str>,
        y_shift: f64,
    ) {
        if y_shift.abs() < 1e-9 {
            return;
        }

        // Mermaid v11.12.2 adjusts Y positions after Dagre layout:
        // - regular nodes: +subGraphTitleTotalMargin/2
        // - clusterNode nodes (recursively rendered clusters): +subGraphTitleTotalMargin
        // - pure cluster nodes (non-recursive clusters): no y-shift (but height grows elsewhere)
        for id in graph.node_ids() {
            // A cluster is only a Mermaid "clusterNode" placeholder if it is a leaf in the
            // current graph. Extracted graphs contain an injected parent cluster node with the
            // same id (and children), which must follow the pure-cluster path.
            let is_cluster_node = extracted.contains_key(&id) && graph.child_count(&id) == 0;
            let delta_y = if is_cluster_node {
                y_shift * 2.0
            } else if subgraph_id_set.contains(id.as_str()) && graph.child_count(&id) != 0 {
                0.0
            } else {
                y_shift
            };
            if delta_y.abs() > 1e-9 {
                let Some(y) = graph.node(&id).and_then(|n| n.y) else {
                    continue;
                };
                if let Some(n) = graph.node_mut(&id) {
                    n.y = Some(y + delta_y);
                }
            }
        }

        // Mermaid shifts all edge points and the edge label position by +subGraphTitleTotalMargin/2.
        for ek in graph.edge_keys() {
            let Some(e) = graph.edge_mut_by_key(&ek) else {
                continue;
            };
            if let Some(y) = e.y {
                e.y = Some(y + y_shift);
            }
            for p in &mut e.points {
                p.y += y_shift;
            }
        }
    }

    struct RecursiveLayoutContext<'a> {
        extracted:
            &'a mut std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        subgraph_id_set: &'a std::collections::HashSet<&'a str>,
        y_shift: f64,
        cluster_node_labels: &'a std::collections::HashMap<String, NodeLabel>,
        title_total_margin: f64,
        title_margin_top: f64,
        title_metrics_ctx: &'a ClusterTitleMetricsContext<'a>,
        cluster_padding: f64,
        subgraph_css_sizes: &'a HashMap<&'a str, [Option<f64>; 2]>,
        styled_clusters: &'a mut HashMap<String, StyledClusterGeometry>,
        work_control: &'a mut OperationLayoutWorkControl,
    }

    fn layout_graph_with_recursive_clusters(
        graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        ctx: &mut RecursiveLayoutContext<'_>,
    ) -> Result<()> {
        struct LayoutFrame {
            cluster_id: Option<String>,
            child_ids: Option<Vec<String>>,
        }

        fn recursive_child_ids(
            graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
            extracted: &std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        ) -> Vec<String> {
            graph
                .node_ids()
                .into_iter()
                // Only recurse into extracted graphs for leaf cluster nodes ("clusterNode" in
                // Mermaid). Child graphs also get their parent cluster node injected, with
                // children, and must not recurse back into themselves.
                .filter(|id| graph.child_count(id) == 0 && extracted.contains_key(id))
                .collect()
        }

        fn inject_parent_cluster(
            graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
            cluster_id: &str,
            ctx: &mut RecursiveLayoutContext<'_>,
        ) -> Result<()> {
            let inserts_cluster = usize::from(!graph.has_node(cluster_id));
            let final_node_order_slots = ctx
                .work_control
                .checked_add(graph.node_order_slot_count(), inserts_cluster)?;
            let final_node_count = ctx
                .work_control
                .checked_add(graph.node_count(), inserts_cluster)?;
            let snapshot_work = ctx
                .work_control
                .checked_add(final_node_order_slots, final_node_count)?;
            ctx.work_control.charge_adapter(snapshot_work)?;

            if inserts_cluster != 0 {
                let lbl = ctx
                    .cluster_node_labels
                    .get(cluster_id)
                    .cloned()
                    .unwrap_or_default();
                charge_node_insertion_ordering(graph, cluster_id, ctx.work_control)?;
                graph.set_node(cluster_id.to_string(), lbl);
            }
            let node_ids = graph.node_ids();
            let cluster_ix = graph
                .node_ix(cluster_id)
                .expect("the injected cluster node is present");
            let mut parent_assignments = Vec::new();
            let mut existing_parent_count = usize::from(graph.parent(cluster_id).is_some());
            let mut numeric_parent_assignments = 0usize;
            for node_id in node_ids {
                if node_id == cluster_id {
                    continue;
                }
                if graph.parent(&node_id).is_some() {
                    existing_parent_count += 1;
                } else {
                    let child_ix = graph
                        .node_ix(&node_id)
                        .expect("a node snapshot contains only live nodes");
                    parent_assignments.push((child_ix, cluster_ix));
                    numeric_parent_assignments += usize::from(is_javascript_array_index(&node_id));
                }
            }
            apply_unparented_parent_assignments(
                graph,
                &parent_assignments,
                UnparentedParentBatchShape {
                    existing_parent_count,
                    numeric_assignment_count: numeric_parent_assignments,
                },
                ctx.work_control,
            )?;
            Ok(())
        }

        fn update_child_cluster_bounds(
            graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
            ids: &[String],
            ctx: &mut RecursiveLayoutContext<'_>,
        ) -> Result<()> {
            for id in ids {
                // In Mermaid, `updateNodeBounds(...)` measures the recursively rendered `<g
                // class="root">` group. In that render path, the child graph contains a node
                // matching the cluster id (inserted via `graph.setNode(parentCluster.id, ...)`),
                // whose computed compound bounds correspond to the cluster box measured in the DOM.
                let (bounds, styled) = measure_extracted_graph(id, ctx)?;
                if let Some(geometry) = styled {
                    ctx.styled_clusters.insert(id.clone(), geometry);
                }
                if let Some(r) = bounds {
                    if let Some(n) = graph.node_mut(id) {
                        n.width = r.width().max(1.0);
                        n.height = r.height().max(1.0);
                    }
                } else if let Some(n_child) = ctx.extracted.get(id).and_then(|child| child.node(id))
                    && let Some(n) = graph.node_mut(id)
                {
                    n.width = n_child.width.max(1.0);
                    n.height = n_child.height.max(1.0);
                }
            }
            Ok(())
        }

        fn layout_one_graph(
            graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
            ctx: &mut RecursiveLayoutContext<'_>,
        ) -> Result<()> {
            if let Err(error) = crate::layout_work::measure_layout_solve!(
                dugong::layout_controlled(graph, &mut *ctx.work_control)
            ) {
                return Err(ctx.work_control.map_dugong_error(error));
            }
            // The pinned Mermaid title-shift pass is a strict no-op at the default zero margin.
            // Avoid materializing point counts and graph snapshots that the mutation would never
            // consume.
            if ctx.y_shift.abs() < 1e-9 {
                return Ok(());
            }
            ctx.work_control.charge_adapter(graph.edge_slot_count())?;
            let point_work = graph.edges().try_fold(0usize, |total, edge_key| {
                let points = graph
                    .edge_by_key(edge_key)
                    .map_or(0, |edge| edge.points.len());
                ctx.work_control.checked_add(total, points)
            })?;
            let node_snapshot_work = node_id_snapshot_work_upper_bound(graph, ctx.work_control)?;
            let edge_snapshot_work = ctx
                .work_control
                .checked_add(graph.edge_slot_count(), graph.edge_count())?;
            let graph_work = ctx
                .work_control
                .checked_add(node_snapshot_work, edge_snapshot_work)?;
            let shift_work = ctx.work_control.checked_add(graph_work, point_work)?;
            ctx.work_control.charge_adapter(shift_work)?;
            apply_mermaid_subgraph_title_shifts(
                graph,
                ctx.extracted,
                ctx.subgraph_id_set,
                ctx.y_shift,
            );
            Ok(())
        }

        if ctx.extracted.is_empty() {
            return layout_one_graph(graph, ctx);
        }

        let mut stack = vec![LayoutFrame {
            cluster_id: None,
            child_ids: None,
        }];

        while let Some(frame) = stack.pop() {
            let Some(child_ids) = frame.child_ids else {
                let cluster_id = frame.cluster_id;
                let graph_snapshot_work = match &cluster_id {
                    Some(id) => ctx.extracted.get(id).map_or(Ok(0), |graph| {
                        node_id_snapshot_work_upper_bound(graph, ctx.work_control)
                    })?,
                    None => node_id_snapshot_work_upper_bound(graph, ctx.work_control)?,
                };
                ctx.work_control.charge_adapter(graph_snapshot_work)?;
                let Some((child_ids, parent_nodesep, parent_ranksep)) = (match &cluster_id {
                    Some(id) => ctx.extracted.get(id).map(|g| {
                        (
                            recursive_child_ids(g, ctx.extracted),
                            g.graph().nodesep,
                            g.graph().ranksep,
                        )
                    }),
                    None => Some((
                        recursive_child_ids(graph, ctx.extracted),
                        graph.graph().nodesep,
                        graph.graph().ranksep,
                    )),
                }) else {
                    continue;
                };

                let mut child_frames = Vec::with_capacity(child_ids.len());
                for child_id in child_ids.iter().rev() {
                    // Match Mermaid `recursiveRender` behavior: before laying out a recursively
                    // rendered cluster graph, override `nodesep` to the parent graph spacing and
                    // `ranksep` to `parent.ranksep + 25`. This compounds for nested recursive
                    // clusters.
                    if let Some(child) = ctx.extracted.get_mut(child_id) {
                        child.graph_mut().nodesep = parent_nodesep;
                        child.graph_mut().ranksep = parent_ranksep + 25.0;
                    }
                    child_frames.push(LayoutFrame {
                        cluster_id: Some(child_id.clone()),
                        child_ids: None,
                    });
                }
                stack.push(LayoutFrame {
                    cluster_id,
                    child_ids: Some(child_ids),
                });
                stack.extend(child_frames);
                continue;
            };

            if let Some(cluster_id) = frame.cluster_id {
                let Some(mut current) = ctx.extracted.remove(&cluster_id) else {
                    continue;
                };
                update_child_cluster_bounds(&mut current, &child_ids, ctx)?;
                inject_parent_cluster(&mut current, &cluster_id, ctx)?;
                layout_one_graph(&mut current, ctx)?;
                ctx.extracted.insert(cluster_id, current);
            } else {
                update_child_cluster_bounds(graph, &child_ids, ctx)?;
                layout_one_graph(graph, ctx)?;
            }
        }
        Ok(())
    }

    let mut styled_clusters = HashMap::new();
    {
        let title_metrics_ctx = ClusterTitleMetricsContext {
            model,
            subgraphs_by_id: &subgraphs_by_id,
            subgraph_index_by_id: &subgraph_index_by_id,
            class_defs: &model.class_defs,
            measurer,
            text_style: &text_style,
            html_label_text_style: &html_label_text_style,
            title_wrapping_width: cluster_title_wrapping_width,
            wrap_mode: cluster_wrap_mode,
            config: effective_config,
            math_renderer,
            svg_label_sidecar,
        };
        let mut recursive_layout_ctx = RecursiveLayoutContext {
            extracted: &mut extracted_graphs,
            subgraph_id_set: &cluster_subgraph_ids,
            y_shift,
            cluster_node_labels: &cluster_node_labels,
            title_total_margin,
            title_margin_top,
            title_metrics_ctx: &title_metrics_ctx,
            cluster_padding,
            subgraph_css_sizes: &subgraph_css_sizes,
            styled_clusters: &mut styled_clusters,
            work_control,
        };
        layout_graph_with_recursive_clusters(&mut g, &mut recursive_layout_ctx)?;
    }

    let mut leaf_rects: std::collections::HashMap<String, Rect> = std::collections::HashMap::new();
    let mut base_pos: std::collections::HashMap<String, (f64, f64)> =
        std::collections::HashMap::new();
    let mut edge_override_points: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        Vec<LayoutPoint>,
    > = std::collections::HashMap::new();
    let mut edge_override_label: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        Option<LayoutLabel>,
    > = std::collections::HashMap::new();
    let mut edge_override_from_cluster: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        Option<String>,
    > = std::collections::HashMap::new();
    let mut edge_override_to_cluster: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        Option<String>,
    > = std::collections::HashMap::new();
    let mut edge_override_endpoints: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        (String, String),
    > = std::collections::HashMap::new();
    let mut edge_override_self_loop_meta: std::collections::HashMap<
        FlowchartLayoutSegmentKey,
        FlowchartSelfLoopSegmentMeta,
    > = std::collections::HashMap::new();
    let leaf_node_id_work = work_control.checked_add(
        model.nodes.len(),
        work_control.checked_add(self_loop_label_node_ids.len(), empty_subgraph_ids.len())?,
    )?;
    work_control.charge_adapter(leaf_node_id_work)?;
    let mut leaf_node_ids: std::collections::HashSet<String> = model
        .nodes
        .iter()
        .filter(|n| !subgraph_ids.contains(n.id.as_str()))
        .map(|n| n.id.clone())
        .collect();
    for id in &self_loop_label_node_ids {
        leaf_node_ids.insert(id.clone());
    }
    for id in &empty_subgraph_ids {
        leaf_node_ids.insert(id.clone());
    }
    for id in &collapsed_subgraph_ids {
        leaf_node_ids.insert((*id).to_string());
    }

    struct PlaceGraphInputs<'a> {
        extracted_graphs:
            &'a std::collections::HashMap<String, Graph<NodeLabel, EdgeLabel, GraphLabel>>,
        subgraph_ids: &'a std::collections::HashSet<&'a str>,
        leaf_node_ids: &'a std::collections::HashSet<String>,
        styled_clusters: &'a HashMap<String, StyledClusterGeometry>,
    }

    #[derive(Clone, Copy)]
    struct PlacedRecursiveCluster {
        frame: Rect,
        styled_footprint: Option<Rect>,
        base_width: f64,
    }

    struct PlaceGraphOutputs<'a> {
        base_pos: &'a mut std::collections::HashMap<String, (f64, f64)>,
        leaf_rects: &'a mut std::collections::HashMap<String, Rect>,
        cluster_rects_from_graph: &'a mut std::collections::HashMap<String, Rect>,
        extracted_clusters: &'a mut HashMap<String, PlacedRecursiveCluster>,
        edge_override_points:
            &'a mut std::collections::HashMap<FlowchartLayoutSegmentKey, Vec<LayoutPoint>>,
        edge_override_label:
            &'a mut std::collections::HashMap<FlowchartLayoutSegmentKey, Option<LayoutLabel>>,
        edge_override_from_cluster:
            &'a mut std::collections::HashMap<FlowchartLayoutSegmentKey, Option<String>>,
        edge_override_to_cluster:
            &'a mut std::collections::HashMap<FlowchartLayoutSegmentKey, Option<String>>,
        edge_override_endpoints:
            &'a mut std::collections::HashMap<FlowchartLayoutSegmentKey, (String, String)>,
        edge_override_self_loop_meta: &'a mut std::collections::HashMap<
            FlowchartLayoutSegmentKey,
            FlowchartSelfLoopSegmentMeta,
        >,
    }

    fn place_graph(
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        offset: (f64, f64),
        is_root: bool,
        inputs: &PlaceGraphInputs<'_>,
        out: &mut PlaceGraphOutputs<'_>,
        work_control: &mut OperationLayoutWorkControl,
    ) -> Result<()> {
        struct PlaceFrame<'a> {
            graph: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
            offset: (f64, f64),
            is_root: bool,
        }

        fn subtree_rect_iterative(
            graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
            id: &str,
            work_control: &mut OperationLayoutWorkControl,
        ) -> Result<Option<Rect>> {
            let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();
            if !visiting.insert(id.to_string()) {
                return Ok(None);
            }
            let mut out: Option<Rect> = None;
            let child_count = child_id_snapshot_work_upper_bound(graph, id, work_control)?;
            work_control.charge_adapter(child_count)?;
            let mut stack: Vec<String> =
                graph.children(id).into_iter().map(str::to_string).collect();
            while let Some(node) = stack.pop() {
                work_control.charge_adapter(1)?;
                if let Some(n) = graph.node(&node)
                    && let (Some(x), Some(y)) = (n.x, n.y)
                {
                    let r = Rect::from_center(x, y, n.width, n.height);
                    if let Some(ref mut cur) = out {
                        cur.union(r);
                    } else {
                        out = Some(r);
                    }
                }
                let child_count = child_id_snapshot_work_upper_bound(graph, &node, work_control)?;
                work_control.charge_adapter(
                    work_control.checked_add(child_count, graph.child_count(&node))?,
                )?;
                for child in graph.children(&node) {
                    if visiting.insert(child.to_string()) {
                        stack.push(child.to_string());
                    }
                }
            }
            Ok(out)
        }

        let mut stack = vec![PlaceFrame {
            graph,
            offset,
            is_root,
        }];
        while let Some(frame) = stack.pop() {
            let node_snapshot_work = node_id_snapshot_work_upper_bound(frame.graph, work_control)?;
            work_control.charge_adapter(node_snapshot_work)?;
            for id in frame.graph.node_ids() {
                let Some(n) = frame.graph.node(&id) else {
                    continue;
                };
                let x = n.x.unwrap_or(0.0) + frame.offset.0;
                let y = n.y.unwrap_or(0.0) + frame.offset.1;
                if inputs.leaf_node_ids.contains(&id) {
                    out.base_pos.insert(id.clone(), (x, y));
                    out.leaf_rects
                        .insert(id.clone(), Rect::from_center(x, y, n.width, n.height));
                    continue;
                }
            }

            // Capture the layout-computed compound bounds for non-extracted clusters.
            //
            // Upstream Dagre computes compound-node geometry from border nodes and then removes the
            // border dummy nodes (`removeBorderNodes`). Our dugong parity pipeline mirrors that, so
            // prefer the compound node's own x/y/width/height when available.
            work_control.charge_adapter(node_snapshot_work)?;
            for id in frame.graph.node_ids() {
                if !inputs.subgraph_ids.contains(id.as_str()) {
                    continue;
                }
                if inputs.extracted_graphs.contains_key(&id) {
                    continue;
                }
                if out.cluster_rects_from_graph.contains_key(&id) {
                    continue;
                }
                if let Some(n) = frame.graph.node(&id)
                    && let (Some(x), Some(y)) = (n.x, n.y)
                    && n.width > 0.0
                    && n.height > 0.0
                {
                    let mut r = Rect::from_center(x, y, n.width, n.height);
                    r.translate(frame.offset.0, frame.offset.1);
                    out.cluster_rects_from_graph.insert(id, r);
                    continue;
                }

                let Some(mut r) = subtree_rect_iterative(frame.graph, &id, work_control)? else {
                    continue;
                };
                r.translate(frame.offset.0, frame.offset.1);
                out.cluster_rects_from_graph.insert(id, r);
            }

            work_control.charge_adapter(frame.graph.edge_slot_count())?;
            let edge_point_work = frame.graph.edges().try_fold(0usize, |total, edge_key| {
                let points = frame
                    .graph
                    .edge_by_key(edge_key)
                    .map_or(0, |edge| edge.points.len());
                work_control.checked_add(total, points)
            })?;
            let edge_snapshot_work = work_control
                .checked_add(frame.graph.edge_slot_count(), frame.graph.edge_count())?;
            let edge_work = work_control.checked_add(edge_snapshot_work, edge_point_work)?;
            work_control.charge_adapter(edge_work)?;
            for ek in frame.graph.edge_keys() {
                let Some(lbl) = frame.graph.edge_by_key(&ek) else {
                    continue;
                };
                let Some(segment_key) = flowchart_layout_segment_key(lbl) else {
                    continue;
                };
                let owner_index = segment_key.owner_index;

                if let (Some(x), Some(y)) = (lbl.x, lbl.y)
                    && (lbl.width > 0.0 || lbl.height > 0.0)
                {
                    let lx = x + frame.offset.0;
                    let ly = y + frame.offset.1;
                    let leaf_id = format!("edge-label::{owner_index}");
                    out.base_pos.insert(leaf_id.clone(), (lx, ly));
                    out.leaf_rects
                        .insert(leaf_id, Rect::from_center(lx, ly, lbl.width, lbl.height));
                }

                if !frame.is_root {
                    let points = lbl
                        .points
                        .iter()
                        .map(|p| LayoutPoint {
                            x: p.x + frame.offset.0,
                            y: p.y + frame.offset.1,
                        })
                        .collect::<Vec<_>>();
                    let label_pos = match (lbl.x, lbl.y) {
                        (Some(x), Some(y)) if lbl.width > 0.0 || lbl.height > 0.0 => {
                            Some(LayoutLabel {
                                x: x + frame.offset.0,
                                y: y + frame.offset.1,
                                width: lbl.width,
                                height: lbl.height,
                            })
                        }
                        _ => None,
                    };
                    out.edge_override_points.insert(segment_key, points);
                    out.edge_override_label.insert(segment_key, label_pos);
                    let from_cluster = lbl
                        .extras
                        .get("fromCluster")
                        .and_then(|v| v.as_str().map(|s| s.to_string()));
                    let to_cluster = lbl
                        .extras
                        .get("toCluster")
                        .and_then(|v| v.as_str().map(|s| s.to_string()));
                    out.edge_override_from_cluster
                        .insert(segment_key, from_cluster);
                    out.edge_override_to_cluster.insert(segment_key, to_cluster);
                    out.edge_override_endpoints
                        .insert(segment_key, (ek.v.clone(), ek.w.clone()));
                    if let Some(meta) = flowchart_self_loop_segment_meta(lbl) {
                        out.edge_override_self_loop_meta.insert(segment_key, meta);
                    }
                }
            }

            let mut child_frames = Vec::new();
            work_control.charge_adapter(node_snapshot_work)?;
            for id in frame.graph.node_ids() {
                // Only recurse into extracted graphs for leaf cluster nodes ("clusterNode" in Mermaid).
                // The recursively rendered graph itself also contains a node with the same id (the
                // parent cluster node injected before layout), which has children and must not recurse.
                if frame.graph.child_count(&id) != 0 {
                    continue;
                }
                let Some(child) = inputs.extracted_graphs.get(&id) else {
                    continue;
                };
                let Some(n) = frame.graph.node(&id) else {
                    continue;
                };
                let (Some(px), Some(py)) = (n.x, n.y) else {
                    continue;
                };
                let parent_x = px + frame.offset.0;
                let parent_y = py + frame.offset.1;
                let Some(cnode) = child.node(&id) else {
                    continue;
                };
                let (Some(cx), Some(cy)) = (cnode.x, cnode.y) else {
                    continue;
                };
                let footprint = Rect::from_center(parent_x, parent_y, n.width, n.height);
                let (child_offset, placed) = if let Some(geometry) = inputs.styled_clusters.get(&id)
                {
                    let offset = geometry.child_offset(parent_x, parent_y);
                    let mut frame = geometry.frame;
                    frame.translate(offset.0, offset.1);
                    (
                        offset,
                        PlacedRecursiveCluster {
                            frame,
                            styled_footprint: Some(footprint),
                            base_width: cnode.width.max(1.0),
                        },
                    )
                } else {
                    (
                        (parent_x - cx, parent_y - cy),
                        PlacedRecursiveCluster {
                            frame: footprint,
                            styled_footprint: None,
                            base_width: cnode.width.max(1.0),
                        },
                    )
                };
                out.extracted_clusters.insert(id.clone(), placed);
                child_frames.push(PlaceFrame {
                    graph: child,
                    offset: child_offset,
                    is_root: false,
                });
            }
            for child in child_frames.into_iter().rev() {
                stack.push(child);
            }
        }
        Ok(())
    }

    let mut cluster_rects_from_graph: std::collections::HashMap<String, Rect> =
        std::collections::HashMap::new();
    let mut extracted_clusters = HashMap::new();
    {
        let place_graph_inputs = PlaceGraphInputs {
            extracted_graphs: &extracted_graphs,
            subgraph_ids: &cluster_subgraph_ids,
            leaf_node_ids: &leaf_node_ids,
            styled_clusters: &styled_clusters,
        };
        let mut place_graph_outputs = PlaceGraphOutputs {
            base_pos: &mut base_pos,
            leaf_rects: &mut leaf_rects,
            cluster_rects_from_graph: &mut cluster_rects_from_graph,
            extracted_clusters: &mut extracted_clusters,
            edge_override_points: &mut edge_override_points,
            edge_override_label: &mut edge_override_label,
            edge_override_from_cluster: &mut edge_override_from_cluster,
            edge_override_to_cluster: &mut edge_override_to_cluster,
            edge_override_endpoints: &mut edge_override_endpoints,
            edge_override_self_loop_meta: &mut edge_override_self_loop_meta,
        };
        place_graph(
            &g,
            (0.0, 0.0),
            true,
            &place_graph_inputs,
            &mut place_graph_outputs,
            work_control,
        )?;
    }

    let mut extra_children: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    work_control.charge_adapter(render_edges.len())?;
    let labeled_edges: std::collections::HashSet<usize> = render_edges
        .iter()
        .zip(&render_edge_self_loop_meta)
        .zip(&render_edge_owner_indices)
        .filter(|((edge, self_loop), owner_index)| {
            edge_label_is_non_empty(model, **owner_index, edge.as_ref(), self_loop.as_ref())
        })
        .map(|(_, owner_index)| *owner_index)
        .collect();

    fn collect_extra_children(
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        labeled_edges: &std::collections::HashSet<usize>,
        implicit_root: Option<&str>,
        out: &mut std::collections::HashMap<String, Vec<String>>,
        work_control: &mut OperationLayoutWorkControl,
    ) -> Result<()> {
        let edge_snapshot_work =
            work_control.checked_add(graph.edge_slot_count(), graph.edge_count())?;
        work_control.charge_adapter(edge_snapshot_work)?;
        for ek in graph.edge_keys() {
            let Some(label) = graph.edge_by_key(&ek) else {
                continue;
            };
            let Some(owner_index) = flowchart_edge_owner_index(label) else {
                continue;
            };
            if !labeled_edges.contains(&owner_index) {
                continue;
            }
            // Mermaid's recursive cluster extractor removes the root cluster node from the
            // extracted graph. In that case, the "lowest common parent" of edges whose endpoints
            // belong to the extracted cluster becomes `None`, even though the label should still
            // participate in the extracted cluster's bounding box. Use `implicit_root` to map
            // those labels back to the extracted cluster id.
            let parent = lowest_common_parent(graph, &ek.v, &ek.w, work_control)?
                .or_else(|| implicit_root.map(|s| s.to_string()));
            let Some(parent) = parent else {
                continue;
            };
            out.entry(parent)
                .or_default()
                .push(format!("edge-label::{owner_index}"));
        }
        Ok(())
    }

    collect_extra_children(&g, &labeled_edges, None, &mut extra_children, work_control)?;
    for cluster_id in &extracted_order {
        let cg = extracted_graphs
            .get(cluster_id)
            .expect("the extraction order references an extracted graph");
        collect_extra_children(
            cg,
            &labeled_edges,
            Some(cluster_id.as_str()),
            &mut extra_children,
            work_control,
        )?;
    }

    // Ensure Mermaid-style self-loop helper nodes participate in cluster bounding/packing.
    // These nodes are not part of the semantic `subgraph ... end` membership list, but are
    // parented into the same clusters as their base node.
    work_control.charge_adapter(self_loop_label_node_ids.len())?;
    for id in &self_loop_label_node_ids {
        if let Some(p) = g.parent(id) {
            extra_children
                .entry(p.to_string())
                .or_default()
                .push(id.clone());
        }
    }

    let mut out_nodes: Vec<LayoutNode> = Vec::new();
    work_control.charge_adapter(model.nodes.len())?;
    for n in &model.nodes {
        if subgraph_ids.contains(n.id.as_str())
            || model.collapsed_replacement(n.id.as_str()).is_some()
        {
            continue;
        }
        let (x, y) = base_pos
            .get(&n.id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned node {}", n.id),
            })?;
        let (width, height) = leaf_rects
            .get(&n.id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized node {}", n.id),
            })?;
        out_nodes.push(LayoutNode {
            id: n.id.clone(),
            x,
            y,
            width,
            height,
            is_cluster: false,
            label_width: leaf_label_metrics_by_id.get(&n.id).map(|v| v.0),
            label_height: leaf_label_metrics_by_id.get(&n.id).map(|v| v.1),
        });
    }
    work_control.charge_adapter(empty_subgraph_ids.len())?;
    for id in &empty_subgraph_ids {
        let (x, y) = base_pos
            .get(id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned node {id}"),
            })?;
        let (width, height) = leaf_rects
            .get(id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized node {id}"),
            })?;
        out_nodes.push(LayoutNode {
            id: id.clone(),
            x,
            y,
            width,
            height,
            is_cluster: false,
            label_width: leaf_label_metrics_by_id.get(id).map(|v| v.0),
            label_height: leaf_label_metrics_by_id.get(id).map(|v| v.1),
        });
    }
    work_control.charge_adapter(collapsed_subgraph_ids.len())?;
    for id in &collapsed_subgraph_ids {
        let (x, y) = base_pos
            .get(*id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned collapsed subgraph {id}"),
            })?;
        let (width, height) = leaf_rects
            .get(*id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized collapsed subgraph {id}"),
            })?;
        out_nodes.push(LayoutNode {
            id: (*id).to_string(),
            x,
            y,
            width,
            height,
            is_cluster: false,
            label_width: leaf_label_metrics_by_id.get(*id).map(|v| v.0),
            label_height: leaf_label_metrics_by_id.get(*id).map(|v| v.1),
        });
    }
    work_control.charge_adapter(self_loop_label_node_ids.len())?;
    for id in &self_loop_label_node_ids {
        let (x, y) = base_pos
            .get(id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing positioned node {id}"),
            })?;
        let (width, height) = leaf_rects
            .get(id)
            .map(|r| (r.width(), r.height()))
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing sized node {id}"),
            })?;
        out_nodes.push(LayoutNode {
            id: id.clone(),
            x,
            y,
            width,
            height,
            is_cluster: false,
            label_width: None,
            label_height: None,
        });
    }

    let mut clusters: Vec<LayoutCluster> = Vec::new();

    let mut cluster_rects: std::collections::HashMap<String, Rect> =
        std::collections::HashMap::new();
    let mut cluster_base_widths: std::collections::HashMap<String, f64> =
        std::collections::HashMap::new();
    let mut visiting: std::collections::HashSet<String> = std::collections::HashSet::new();
    let cluster_membership_work = model.subgraphs.iter().try_fold(0usize, |total, subgraph| {
        work_control.checked_add(total, subgraph.nodes.len())
    })?;
    let extra_cluster_work = extra_children
        .values()
        .try_fold(0usize, |total, children| {
            work_control.checked_add(total, children.len())
        })?;
    // `compute_cluster_rect` visits every computed subgraph membership once while expanding the
    // explicit stack and once while folding child rectangles. The outer materialization loop and
    // both stack frames add three subgraph-local visits. Extracted/empty subgraphs may do less,
    // but this checked owner-local upper bound admits every scan and stack allocation before the
    // first cluster rectangle is planned.
    let cluster_work = work_control.checked_add(
        work_control.checked_add(
            work_control.checked_mul(model.subgraphs.len(), 3)?,
            work_control.checked_mul(cluster_membership_work, 2)?,
        )?,
        extra_cluster_work,
    )?;
    work_control.charge_adapter(cluster_work)?;

    struct ClusterRectContext<'a> {
        model: &'a FlowchartRenderModelRef<'a>,
        subgraphs_by_id: &'a FlowSubgraphIndex<'a>,
        subgraph_index_by_id: &'a HashMap<&'a str, usize>,
        subgraph_members_by_id: &'a HashMap<&'a str, Vec<&'a str>>,
        class_defs: &'a indexmap::IndexMap<String, Vec<String>>,
        leaf_rects: &'a std::collections::HashMap<String, Rect>,
        extra_children: &'a std::collections::HashMap<String, Vec<String>>,
        measurer: &'a dyn TextMeasurer,
        text_style: &'a TextStyle,
        html_label_text_style: &'a TextStyle,
        title_wrapping_width: f64,
        wrap_mode: WrapMode,
        config: &'a PresentationConfig,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
        cluster_padding: f64,
        title_total_margin: f64,
    }

    struct ClusterRectState<'a> {
        cluster_rects: &'a mut std::collections::HashMap<String, Rect>,
        cluster_base_widths: &'a mut std::collections::HashMap<String, f64>,
        visiting: &'a mut std::collections::HashSet<String>,
    }

    fn compute_cluster_rect(
        id: &str,
        ctx: &ClusterRectContext<'_>,
        state: &mut ClusterRectState<'_>,
    ) -> Result<(Rect, f64)> {
        if let Some(r) = state.cluster_rects.get(id).copied() {
            let base_width = state
                .cluster_base_widths
                .get(id)
                .copied()
                .unwrap_or_else(|| r.width());
            return Ok((r, base_width));
        }

        struct ClusterRectFrame {
            id: String,
            expanded: bool,
        }

        let mut stack = vec![ClusterRectFrame {
            id: id.to_string(),
            expanded: false,
        }];
        while let Some(frame) = stack.pop() {
            if state.cluster_rects.contains_key(&frame.id) {
                continue;
            }

            if !frame.expanded {
                if !state.visiting.insert(frame.id.clone()) {
                    return Err(Error::InvalidModel {
                        message: format!("cycle in subgraph membership involving {}", frame.id),
                    });
                }

                if !ctx.subgraphs_by_id.contains_key(frame.id.as_str()) {
                    return Err(Error::InvalidModel {
                        message: format!("missing subgraph definition for {}", frame.id),
                    });
                }

                stack.push(ClusterRectFrame {
                    id: frame.id.clone(),
                    expanded: true,
                });
                let members = ctx
                    .subgraph_members_by_id
                    .get(frame.id.as_str())
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                for member in members.iter().rev() {
                    if ctx.subgraphs_by_id.contains_key(*member)
                        && !state.cluster_rects.contains_key(*member)
                    {
                        stack.push(ClusterRectFrame {
                            id: (*member).to_string(),
                            expanded: false,
                        });
                    }
                }
                continue;
            }

            let Some(sg) = ctx.subgraphs_by_id.get(frame.id.as_str()) else {
                return Err(Error::InvalidModel {
                    message: format!("missing subgraph definition for {}", frame.id),
                });
            };

            let mut content: Option<Rect> = None;
            let members = ctx
                .subgraph_members_by_id
                .get(frame.id.as_str())
                .map(Vec::as_slice)
                .unwrap_or_default();
            for member in members {
                let member_rect = if let Some(r) = ctx.leaf_rects.get(*member).copied() {
                    Some(r)
                } else if ctx.subgraphs_by_id.contains_key(*member) {
                    Some(state.cluster_rects.get(*member).copied().ok_or_else(|| {
                        Error::InvalidModel {
                            message: format!("missing computed subgraph rect for {member}"),
                        }
                    })?)
                } else {
                    None
                };

                if let Some(r) = member_rect {
                    if let Some(ref mut cur) = content {
                        cur.union(r);
                    } else {
                        content = Some(r);
                    }
                }
            }

            if let Some(extra) = ctx.extra_children.get(&frame.id) {
                for child in extra {
                    if let Some(r) = ctx.leaf_rects.get(child).copied() {
                        if let Some(ref mut cur) = content {
                            cur.union(r);
                        } else {
                            content = Some(r);
                        }
                    }
                }
            }

            let label_type = sg.label_type.as_deref().unwrap_or("text");
            let subgraph_index = ctx
                .subgraph_index_by_id
                .get(frame.id.as_str())
                .copied()
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("missing canonical subgraph index for {}", frame.id),
                })?;
            let title = ctx.model.subgraph_title_for_render(subgraph_index, sg);
            let title_width_limit = (label_type == "markdown").then_some(ctx.title_wrapping_width);
            let base_style = if ctx.wrap_mode == WrapMode::HtmlLike {
                ctx.html_label_text_style
            } else {
                ctx.text_style
            };
            let (classes, styles) = ctx.model.effective_subgraph_css(subgraph_index, sg);
            let text_style = flowchart_effective_text_style_for_classes_with_provenance(
                base_style,
                ctx.class_defs,
                classes,
                styles,
            );
            let owner = Some(FlowchartSvgLabelOwner::SubgraphTitle(subgraph_index));
            let title_metrics = measure_flowchart_svg_label_for_layout_with_source_style(
                ctx.svg_label_sidecar,
                owner,
                owner.map(|_| sg.id.as_str()),
                FlowchartLabelMetricsRequest {
                    measurer: ctx.measurer,
                    raw_label: title,
                    label_type,
                    style: text_style.as_ref(),
                    max_width_px: title_width_limit,
                    wrap_mode: ctx.wrap_mode,
                    config: ctx.config,
                    math_renderer: ctx.math_renderer,
                },
                Some(&text_style),
                FlowchartSvgWidthMode::Bbox,
            );
            let mut rect = if let Some(r) = content {
                r
            } else {
                Rect::from_center(
                    0.0,
                    0.0,
                    title_metrics.width.max(1.0),
                    title_metrics.height.max(1.0),
                )
            };

            // Expand to provide the cluster's internal padding.
            rect.pad(ctx.cluster_padding);

            // Mermaid computes `node.diff` using the pre-widened layout node width, then may widen the
            // rect to fit the label bbox during rendering.
            let base_width = rect.width();

            // Mermaid 11.16 `rendering-elements/clusters.js` sets the rect width to
            // `max(node.width, labelBBox.width + node.padding)`.
            let min_width = title_metrics.width.max(1.0) + ctx.cluster_padding;
            if rect.width() < min_width {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, min_width, rect.height());
            }

            // Extend height to reserve space for subgraph title margins (Mermaid does this after layout).
            if ctx.title_total_margin > 0.0 {
                let (cx, cy) = rect.center();
                rect =
                    Rect::from_center(cx, cy, rect.width(), rect.height() + ctx.title_total_margin);
            }

            // Keep the cluster tall enough to accommodate the title bbox if needed.
            let min_height = title_metrics.height.max(1.0) + ctx.title_total_margin;
            if rect.height() < min_height {
                let (cx, cy) = rect.center();
                rect = Rect::from_center(cx, cy, rect.width(), min_height);
            }

            state.visiting.remove(&frame.id);
            state.cluster_rects.insert(frame.id.clone(), rect);
            state.cluster_base_widths.insert(frame.id, base_width);
        }

        let rect = state
            .cluster_rects
            .get(id)
            .copied()
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing computed subgraph rect for {id}"),
            })?;
        let base_width = state
            .cluster_base_widths
            .get(id)
            .copied()
            .unwrap_or_else(|| rect.width());
        Ok((rect, base_width))
    }

    struct ClusterTitleAdjustContext<'a> {
        model: &'a FlowchartRenderModelRef<'a>,
        class_defs: &'a indexmap::IndexMap<String, Vec<String>>,
        subgraph_index_by_id: &'a HashMap<&'a str, usize>,
        measurer: &'a dyn TextMeasurer,
        text_style: &'a TextStyle,
        html_label_text_style: &'a TextStyle,
        title_wrapping_width: f64,
        wrap_mode: WrapMode,
        config: &'a PresentationConfig,
        math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
        svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecarBuilder>,
        title_total_margin: f64,
        cluster_padding: f64,
    }

    fn adjust_cluster_rect_for_title(
        mut rect: Rect,
        declaration_ordinal: usize,
        sg: &FlowSubgraph,
        title: &str,
        label_type: &str,
        add_title_total_margin: bool,
        ctx: &ClusterTitleAdjustContext<'_>,
    ) -> Rect {
        let title_width_limit = (label_type == "markdown").then_some(ctx.title_wrapping_width);
        let base_style = if ctx.wrap_mode == WrapMode::HtmlLike {
            ctx.html_label_text_style
        } else {
            ctx.text_style
        };
        let (classes, styles) = ctx.model.effective_subgraph_css(declaration_ordinal, sg);
        let text_style = flowchart_effective_text_style_for_classes_with_provenance(
            base_style,
            ctx.class_defs,
            classes,
            styles,
        );
        let owner = ctx
            .subgraph_index_by_id
            .get(sg.id.as_str())
            .copied()
            .map(FlowchartSvgLabelOwner::SubgraphTitle);
        let title_metrics = measure_flowchart_svg_label_for_layout_with_source_style(
            ctx.svg_label_sidecar,
            owner,
            Some(sg.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer: ctx.measurer,
                raw_label: title,
                label_type,
                style: text_style.as_ref(),
                max_width_px: title_width_limit,
                wrap_mode: ctx.wrap_mode,
                config: ctx.config,
                math_renderer: ctx.math_renderer,
            },
            Some(&text_style),
            FlowchartSvgWidthMode::Bbox,
        );
        let title_w = title_metrics.width.max(1.0);
        let title_h = title_metrics.height.max(1.0);

        // Mermaid cluster "rect" widens to fit the raw title bbox (no added padding),
        // even when the cluster bounds come from Dagre border nodes.
        let min_w = title_w + ctx.cluster_padding;
        if rect.width() < min_w {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, min_w, rect.height());
        }

        // Mermaid adds `subGraphTitleTotalMargin` to cluster height after layout.
        if add_title_total_margin && ctx.title_total_margin > 0.0 {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, rect.width(), rect.height() + ctx.title_total_margin);
        }

        // Keep the cluster tall enough for the title bbox (including title margins).
        let min_h = title_h + ctx.title_total_margin;
        if rect.height() < min_h {
            let (cx, cy) = rect.center();
            rect = Rect::from_center(cx, cy, rect.width(), min_h);
        }

        rect
    }

    let cluster_rect_ctx = ClusterRectContext {
        model,
        subgraphs_by_id: &subgraphs_by_id,
        subgraph_index_by_id: &subgraph_index_by_id,
        subgraph_members_by_id: &subgraph_members_by_id,
        class_defs: &model.class_defs,
        leaf_rects: &leaf_rects,
        extra_children: &extra_children,
        measurer,
        text_style: &text_style,
        html_label_text_style: &html_label_text_style,
        title_wrapping_width: cluster_title_wrapping_width,
        wrap_mode: cluster_wrap_mode,
        config: effective_config,
        math_renderer,
        svg_label_sidecar,
        cluster_padding,
        title_total_margin,
    };
    let mut cluster_rect_state = ClusterRectState {
        cluster_rects: &mut cluster_rects,
        cluster_base_widths: &mut cluster_base_widths,
        visiting: &mut visiting,
    };
    let title_adjust_ctx = ClusterTitleAdjustContext {
        model,
        class_defs: &model.class_defs,
        subgraph_index_by_id: &subgraph_index_by_id,
        measurer,
        text_style: &text_style,
        html_label_text_style: &html_label_text_style,
        title_wrapping_width: cluster_title_wrapping_width,
        wrap_mode: cluster_wrap_mode,
        config: effective_config,
        math_renderer,
        svg_label_sidecar,
        title_total_margin,
        cluster_padding,
    };

    for &(subgraph_index, sg) in &canonical_subgraphs_in_order {
        if !cluster_subgraph_ids.contains(sg.id.as_str()) {
            continue;
        }
        let title = model.subgraph_title_for_render(subgraph_index, sg);

        let extracted = extracted_clusters.get(&sg.id);
        let (rect, base_width) = if let Some(placed) = extracted {
            (placed.frame, placed.base_width)
        } else if let Some(rect) = cluster_rects_from_graph.get(&sg.id).copied() {
            (rect, rect.width())
        } else {
            compute_cluster_rect(&sg.id, &cluster_rect_ctx, &mut cluster_rect_state)?
        };
        let footprint = extracted.and_then(|placed| placed.styled_footprint);
        let rect = if footprint.is_none()
            && (extracted.is_some() || cluster_rects_from_graph.contains_key(&sg.id))
        {
            adjust_cluster_rect_for_title(
                rect,
                subgraph_index,
                sg,
                title,
                sg.label_type.as_deref().unwrap_or("text"),
                extracted.is_none(),
                &title_adjust_ctx,
            )
        } else {
            rect
        };
        let (cx, cy) = rect.center();

        let label_type = sg.label_type.as_deref().unwrap_or("text");
        let title_width_limit = (label_type == "markdown").then_some(cluster_title_wrapping_width);
        let base_style = if cluster_wrap_mode == WrapMode::HtmlLike {
            &html_label_text_style
        } else {
            &text_style
        };
        let (classes, styles) = model.effective_subgraph_css(subgraph_index, sg);
        let title_text_style = flowchart_effective_text_style_for_classes_with_provenance(
            base_style,
            &model.class_defs,
            classes,
            styles,
        );
        let title_metrics = measure_flowchart_svg_label_for_layout_with_source_style(
            svg_label_sidecar,
            Some(FlowchartSvgLabelOwner::SubgraphTitle(subgraph_index)),
            Some(sg.id.as_str()),
            FlowchartLabelMetricsRequest {
                measurer,
                raw_label: title,
                label_type,
                style: title_text_style.as_ref(),
                max_width_px: title_width_limit,
                wrap_mode: cluster_wrap_mode,
                config: effective_config,
                math_renderer,
            },
            Some(&title_text_style),
            FlowchartSvgWidthMode::Bbox,
        );
        let title_label = LayoutLabel {
            x: cx,
            y: cy - rect.height() / 2.0 + title_margin_top + title_metrics.height / 2.0,
            width: title_metrics.width,
            height: title_metrics.height,
        };

        // `dagre-wrapper/clusters.js` (shape `rect`) sets `padding = 0 * node.padding`.
        // The cluster label is positioned at `node.x - bbox.width/2`, and `node.diff` is:
        // - `(bbox.width - node.width)/2 - node.padding/2` when the box widens to fit the title
        // - otherwise `-node.padding/2`.
        let title_w = title_metrics.width.max(1.0);
        let diff = if base_width <= title_w {
            (title_w - base_width) / 2.0 - cluster_padding / 2.0
        } else {
            -cluster_padding / 2.0
        };
        let offset_y = title_metrics.height - cluster_padding / 2.0;

        let effective_dir = effective_dir_by_id
            .get(&sg.id)
            .cloned()
            .unwrap_or_else(|| effective_cluster_dir(sg, &diagram_direction, inherit_dir));

        clusters.push(LayoutCluster {
            id: sg.id.clone(),
            x: cx,
            y: cy,
            width: rect.width(),
            height: rect.height(),
            diff,
            offset_y,
            title: sg.title.clone(),
            title_label,
            requested_dir: sg.dir.as_ref().map(|s| normalize_dir(s)),
            effective_dir,
            padding: cluster_padding,
            title_margin_top,
            title_margin_bottom,
        });

        let cluster_node = footprint.unwrap_or(rect);
        let (x, y) = cluster_node.center();
        out_nodes.push(LayoutNode {
            id: sg.id.clone(),
            x,
            y,
            width: cluster_node.width(),
            height: cluster_node.height(),
            is_cluster: true,
            label_width: None,
            label_height: None,
        });
    }
    let cluster_sort_work = checked_n_log_n(clusters.len(), work_control)?;
    work_control.charge_adapter(cluster_sort_work)?;
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let mut out_edge_candidates: Vec<FlowchartLayoutEdgeCandidate> = Vec::new();
    work_control.charge_adapter(g.edge_slot_count())?;
    let root_edge_point_work = g.edges().try_fold(0usize, |total, edge_key| {
        let points = g.edge_by_key(edge_key).map_or(0, |edge| edge.points.len());
        work_control.checked_add(total, points)
    })?;
    let mut extracted_edge_point_work = 0usize;
    for id in &extracted_order {
        let graph = extracted_graphs
            .get(id)
            .expect("the extraction order references an extracted graph");
        work_control.charge_adapter(graph.edge_slot_count())?;
        extracted_edge_point_work =
            graph
                .edges()
                .try_fold(extracted_edge_point_work, |inner_total, edge_key| {
                    let points = graph
                        .edge_by_key(edge_key)
                        .map_or(0, |edge| edge.points.len());
                    work_control.checked_add(inner_total, points)
                })?;
    }
    let edge_projection_work = work_control.checked_add(
        render_edges.len(),
        work_control.checked_add(root_edge_point_work, extracted_edge_point_work)?,
    )?;
    work_control.charge_adapter(edge_projection_work)?;
    for ((e, expected_self_loop_meta), owner_index) in render_edges
        .iter()
        .zip(&render_edge_self_loop_meta)
        .zip(&render_edge_owner_indices)
    {
        let segment_key =
            FlowchartLayoutSegmentKey::new(*owner_index, expected_self_loop_meta.as_ref());
        let (
            points,
            label_pos,
            mut from_cluster,
            mut to_cluster,
            layout_from,
            layout_to,
            actual_self_loop_meta,
        ) = if let Some(points) = edge_override_points.get(&segment_key) {
            let from_cluster = edge_override_from_cluster
                .get(&segment_key)
                .cloned()
                .unwrap_or(None);
            let to_cluster = edge_override_to_cluster
                .get(&segment_key)
                .cloned()
                .unwrap_or(None);
            (
                points.clone(),
                edge_override_label
                    .get(&segment_key)
                    .cloned()
                    .unwrap_or(None),
                from_cluster,
                to_cluster,
                edge_override_endpoints
                    .get(&segment_key)
                    .map(|(from, _)| from.clone())
                    .unwrap_or_else(|| e.from.clone()),
                edge_override_endpoints
                    .get(&segment_key)
                    .map(|(_, to)| to.clone())
                    .unwrap_or_else(|| e.to.clone()),
                edge_override_self_loop_meta.get(&segment_key).cloned(),
            )
        } else {
            let Some((v, w)) = edge_endpoints_by_segment.get(&segment_key).cloned() else {
                continue;
            };
            let Some(edge_key) = edge_key_by_segment.get(&segment_key).map(String::as_str) else {
                continue;
            };
            let Some(label) = g.edge(&v, &w, Some(edge_key)) else {
                continue;
            };
            let from_cluster = label
                .extras
                .get("fromCluster")
                .and_then(|v| v.as_str().map(|s| s.to_string()));
            let to_cluster = label
                .extras
                .get("toCluster")
                .and_then(|v| v.as_str().map(|s| s.to_string()));
            let points = label
                .points
                .iter()
                .map(|p| LayoutPoint { x: p.x, y: p.y })
                .collect::<Vec<_>>();
            let label_pos = match (label.x, label.y) {
                (Some(x), Some(y)) if label.width > 0.0 || label.height > 0.0 => {
                    Some(LayoutLabel {
                        x,
                        y,
                        width: label.width,
                        height: label.height,
                    })
                }
                _ => None,
            };
            let self_loop_meta = flowchart_self_loop_segment_meta(label);
            (
                points,
                label_pos,
                from_cluster,
                to_cluster,
                v,
                w,
                self_loop_meta,
            )
        };

        // Graphlib identifies a multiedge by (v, w, name). Parallel self-loops reuse all three
        // helper keys, so the later logical edge overwrites the earlier triple. Only materialize
        // the helper segments whose metadata survived that overwrite.
        if expected_self_loop_meta.is_some()
            && actual_self_loop_meta.as_ref() != expected_self_loop_meta.as_ref()
        {
            continue;
        }

        // Match Mermaid's dagre adapter: self-loop special edges on group nodes are annotated with
        // `fromCluster` / `toCluster` so downstream renderers can clip routes to the cluster
        // boundary.
        if cluster_subgraph_ids.contains(e.from.as_str())
            && actual_self_loop_meta
                .as_ref()
                .is_some_and(|meta| meta.order == 0)
        {
            from_cluster = Some(e.from.clone());
        }
        if cluster_subgraph_ids.contains(e.to.as_str())
            && actual_self_loop_meta
                .as_ref()
                .is_some_and(|meta| meta.order == 2)
        {
            to_cluster = Some(e.to.clone());
        }

        out_edge_candidates.push(FlowchartLayoutEdgeCandidate {
            owner_index: *owner_index,
            edge: LayoutEdge {
                id: e.id.clone(),
                from: layout_from,
                to: layout_to,
                from_cluster,
                to_cluster,
                points,
                label: label_pos,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: None,
                end_marker: None,
                stroke_dasharray: None,
            },
            self_loop: actual_self_loop_meta,
        });
    }

    let merged_edges = if merge_self_loops {
        let merge_work = work_control.checked_add(
            work_control.checked_add(model.edges.len(), out_edge_candidates.len())?,
            out_nodes.len(),
        )?;
        work_control.charge_adapter(merge_work)?;
        merge_flowchart_self_loop_segments(
            &model.edges,
            &out_nodes,
            &diagram_direction,
            out_edge_candidates,
        )
    } else {
        work_control.charge_adapter(out_edge_candidates.len())?;
        out_edge_candidates
    };
    let edge_owner_keys = merged_edges
        .iter()
        .map(|candidate| match &candidate.self_loop {
            Some(segment) => crate::flowchart::FlowchartEdgeKey::self_loop_segment(
                candidate.owner_index,
                segment.order,
            ),
            None => crate::flowchart::FlowchartEdgeKey::new(candidate.owner_index),
        })
        .collect();
    let mut out_edges = merged_edges
        .into_iter()
        .map(|candidate| candidate.edge)
        .collect::<Vec<_>>();

    // Mermaid's flowchart renderer uses shape-specific intersection functions for edge endpoints
    // (e.g. diamond nodes). Our Dagre-ish layout currently treats all nodes as rectangles, so the
    // first/last points can land on the bounding box rather than the actual polygon boundary.
    //
    // Adjust the first/last edge points to match Mermaid's shape intersection behavior for the
    // shapes that materially differ from rectangles.
    let endpoint_work = work_control.checked_add(
        work_control.checked_add(model.nodes.len(), out_nodes.len())?,
        out_edges.len(),
    )?;
    work_control.charge_adapter(endpoint_work)?;
    let mut node_shape_by_id: HashMap<&str, &str> = HashMap::new();
    for n in &model.nodes {
        if let Some(s) = n.layout_shape.as_deref() {
            node_shape_by_id.insert(n.id.as_str(), s);
        }
    }
    let mut layout_node_by_id: HashMap<&str, &LayoutNode> = HashMap::new();
    for n in &out_nodes {
        layout_node_by_id.insert(n.id.as_str(), n);
    }

    fn diamond_intersection(node: &LayoutNode, toward: &LayoutPoint) -> Option<LayoutPoint> {
        let vx = toward.x - node.x;
        let vy = toward.y - node.y;
        if !(vx.is_finite() && vy.is_finite()) {
            return None;
        }
        if vx.abs() <= 1e-12 && vy.abs() <= 1e-12 {
            return None;
        }
        let hw = (node.width / 2.0).max(1e-9);
        let hh = (node.height / 2.0).max(1e-9);
        let denom = vx.abs() / hw + vy.abs() / hh;
        if !(denom.is_finite() && denom > 0.0) {
            return None;
        }
        let t = 1.0 / denom;
        Some(LayoutPoint {
            x: node.x + vx * t,
            y: node.y + vy * t,
        })
    }

    for e in &mut out_edges {
        if e.points.len() < 2 {
            continue;
        }

        if let Some(node) = layout_node_by_id.get(e.from.as_str())
            && !node.is_cluster
        {
            let shape = node_shape_by_id
                .get(e.from.as_str())
                .copied()
                .unwrap_or("squareRect");
            if matches!(shape, "diamond" | "question" | "diam")
                && let Some(p) = diamond_intersection(node, &e.points[1])
            {
                e.points[0] = p;
            }
        }
        if let Some(node) = layout_node_by_id.get(e.to.as_str())
            && !node.is_cluster
        {
            let shape = node_shape_by_id
                .get(e.to.as_str())
                .copied()
                .unwrap_or("squareRect");
            if matches!(shape, "diamond" | "question" | "diam") {
                let n = e.points.len();
                if let Some(p) = diamond_intersection(node, &e.points[n - 2]) {
                    e.points[n - 1] = p;
                }
            }
        }
    }

    let bounds = compute_bounds_controlled(&out_nodes, &out_edges, |units| {
        work_control.charge_adapter(units)
    })?;

    Ok(FlowchartLayout {
        nodes: out_nodes,
        edges: out_edges,
        edge_owners: crate::flowchart::FlowchartEdgeOwners::new(edge_owner_keys),
        clusters,
        bounds,
        dom_node_order_by_root,
        route_contract: crate::graph_layout::RouteContract::DagreParity,
        placement: Default::default(),
        uses_elk_adapter_dom: false,
    })
}

#[cfg(test)]
mod tests;
