//! Dagre compound graph construction, solving, and ER relationship projection.

use super::{
    ErLayoutSettings, ErPreparedLabels, ErPreparedRelationshipLabel, calc_label_position,
    er_layout_bounds, er_marker_id, er_rel_idx_from_edge_id, er_subgraph_title_metrics,
};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
use merman_model::families::er::{ErDiagramModel, ErEntity};
use merman_render_core::graph_layout::dagre_rankdir;
use merman_render_core::layout_work::OperationLayoutWorkControl;
use merman_render_core::text::TextMeasurer;
use merman_render_core::{
    Bounds, Error, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, Result,
};
use std::collections::{HashMap, HashSet};

#[cfg(test)]
mod tests;

/// Solved Dagre geometry with distinct layout and SVG edge projections.
#[derive(Debug)]
pub struct ErDagreLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub render_edges: Vec<LayoutEdge>,
    pub clusters: Vec<LayoutCluster>,
    pub bounds: Option<Bounds>,
}

pub fn layout(
    model: &ErDiagramModel,
    measurer: &dyn TextMeasurer,
    settings: ErLayoutSettings,
    prepared_labels: &ErPreparedLabels,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<ErDagreLayout> {
    let membership_count = model.subgraphs.iter().try_fold(0usize, |count, subgraph| {
        work_control.checked_add(count, subgraph.nodes.len())
    })?;
    let helper_count = work_control.checked_mul(
        model
            .relationships
            .iter()
            .filter(|r| r.entity_a == r.entity_b)
            .count(),
        2,
    )?;
    let parent_count = work_control.checked_add(membership_count, helper_count)?;
    if parent_count != 0 {
        let slots = work_control.checked_add(
            work_control.checked_add(model.entities.len(), model.subgraphs.len())?,
            helper_count,
        )?;
        // Only Dagre builds this parent plan. Admit its scratch, checked forest batch and
        // numeric-key updates conservatively before allocating any of that derived graph.
        let height = usize::BITS as usize - slots.saturating_sub(1).leading_zeros() as usize + 1;
        let parent_work = work_control.checked_mul(
            work_control.checked_add(slots, parent_count)?,
            work_control.checked_mul(height, 12)?,
        )?;
        work_control.charge_adapter(parent_work)?;
    }

    let ErLayoutSettings {
        direction,
        node_spacing,
        rank_spacing,
        label_style,
        relationship_html_labels,
        ..
    } = settings;

    let mut g = Graph::<NodeLabel, EdgeLabel, GraphLabel>::new(GraphOptions {
        directed: true,
        multigraph: true,
        // Mermaid's Dagre adapter always enables `compound: true`, even without clusters.
        compound: true,
    });
    let rankdir = dagre_rankdir(direction);
    g.set_graph(GraphLabel {
        rankdir,
        nodesep: node_spacing,
        ranksep: rank_spacing,
        // Mermaid adds an 8px margin during Dagre layout and again around the SVG bounding box.
        marginx: 8.0,
        marginy: 8.0,
        ..Default::default()
    });

    fn parse_entity_counter_from_id(id: &str) -> Option<usize> {
        let (_prefix, tail) = id.rsplit_once('-')?;
        tail.parse::<usize>().ok()
    }

    // Groups are first-class compound nodes in Mermaid's ER graph. Keep the source names in the
    // semantic model and translate entity members to their renderer-facing `entity-*` ids only
    // at this boundary.
    let subgraph_ids: HashSet<&str> = model
        .subgraphs
        .iter()
        .map(|subgraph| subgraph.id.as_str())
        .collect();
    let entity_id_by_name: HashMap<&str, &str> = model
        .entities
        .iter()
        .map(|(name, entity)| (name.as_str(), entity.id.as_str()))
        .collect();
    let entity_name_by_id: HashMap<&str, &str> = model
        .entities
        .iter()
        .map(|(name, entity)| (entity.id.as_str(), name.as_str()))
        .collect();
    let mut subgraph_title_metrics: HashMap<String, (f64, f64)> = HashMap::new();

    // Insert groups in reverse declaration order, matching Mermaid's `getData()` projection and
    // keeping nested group order deterministic for Dagre's compound ranking.
    for subgraph in model.subgraphs.iter().rev() {
        let metrics =
            er_subgraph_title_metrics(subgraph, measurer, &label_style, relationship_html_labels);
        let has_children = subgraph.nodes.iter().any(|member| {
            subgraph_ids.contains(member.as_str())
                || entity_id_by_name.contains_key(member.as_str())
        });
        let node_label = if has_children {
            NodeLabel::default()
        } else {
            NodeLabel {
                width: (metrics.width + 16.0).max(16.0),
                height: (metrics.height + 16.0).max(16.0),
                ..Default::default()
            }
        };
        subgraph_title_metrics.insert(
            subgraph.id.clone(),
            (metrics.width.max(0.0), metrics.height.max(0.0)),
        );
        g.set_node(subgraph.id.clone(), node_label);
    }

    // Nodes.
    let mut entities_in_layout_order: Vec<&ErEntity> = model.entities.values().collect();
    entities_in_layout_order.sort_by(|a, b| {
        let a_key = (parse_entity_counter_from_id(&a.id), a.id.as_str());
        let b_key = (parse_entity_counter_from_id(&b.id), b.id.as_str());
        a_key.cmp(&b_key)
    });

    for e in entities_in_layout_order {
        // Mermaid's `getData()` omits an entity whose source name collides with a subgraph id.
        if entity_name_by_id
            .get(e.id.as_str())
            .is_some_and(|name| subgraph_ids.contains(*name))
        {
            continue;
        }
        let measure = prepared_labels
            .entity(&e.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("ER prepared labels are missing entity {}", e.id),
            })?;
        g.set_node(
            e.id.clone(),
            NodeLabel {
                width: measure.width,
                height: measure.height,
                ..Default::default()
            },
        );
    }

    // Retain the last membership assignment in its original mutation order. Graphlib's checked
    // batch requires each child once; replaying only its last assignment preserves reparenting and
    // ordinary child-key order without a potentially quadratic ancestor scan per declaration.
    let mut parent_assignments = Vec::new();
    let mut last_parent_assignment = HashMap::new();
    for subgraph in &model.subgraphs {
        for member in &subgraph.nodes {
            let child_id = if subgraph_ids.contains(member.as_str()) {
                member.clone()
            } else if let Some(entity_id) = entity_id_by_name.get(member.as_str()) {
                (*entity_id).to_string()
            } else {
                continue;
            };
            if child_id != subgraph.id
                && let (Some(child), Some(parent)) = (g.node_ix(&child_id), g.node_ix(&subgraph.id))
            {
                last_parent_assignment.insert(child, parent_assignments.len());
                parent_assignments.push((child, parent));
            }
        }
    }

    // Edges. Mermaid ER uses edge labels ("roleA") and the unified renderer routes through the
    // generic dagre pipeline, which accounts for label bbox in spacing. Mirror that by giving
    // dagre real label sizes here.
    for (idx, r) in model.relationships.iter().enumerate() {
        if g.node(&r.entity_a).is_none() || g.node(&r.entity_b).is_none() {
            return Err(Error::InvalidModel {
                message: format!(
                    "relationship references missing ER nodes: {} -> {}",
                    r.entity_a, r.entity_b
                ),
            });
        }

        // Mermaid's dagre renderer splits self-loops into three edges and introduces two helper
        // nodes (labelRect). Mermaid initializes them at 10x10, but after `updateNodeBounds(...)`
        // an empty labelRect collapses to ~0.1x0.1 and that is what Dagre uses for spacing.
        // Match that here for layout parity.
        if r.entity_a == r.entity_b {
            let node_id = r.entity_a.as_str();
            let special_1 = format!("{node_id}---{node_id}---1");
            let special_2 = format!("{node_id}---{node_id}---2");
            let parent = g
                .node_ix(node_id)
                .and_then(|child| last_parent_assignment.get(&child))
                .map(|&index| parent_assignments[index].1);

            if g.node(&special_1).is_none() {
                g.set_node(
                    special_1.clone(),
                    NodeLabel {
                        width: 0.1,
                        height: 0.1,
                        ..Default::default()
                    },
                );
            }
            if g.node(&special_2).is_none() {
                g.set_node(
                    special_2.clone(),
                    NodeLabel {
                        width: 0.1,
                        height: 0.1,
                        ..Default::default()
                    },
                );
            }
            if let Some(parent) = parent {
                for helper in [&special_1, &special_2] {
                    if let Some(child) = g.node_ix(helper) {
                        last_parent_assignment.insert(child, parent_assignments.len());
                        parent_assignments.push((child, parent));
                    }
                }
            }

            let (label_w, label_h) = prepared_labels
                .relationship(idx)
                .map(ErPreparedRelationshipLabel::dimensions)
                .unwrap_or_default();

            // First segment: keep start marker, no label.
            g.set_edge_named(
                r.entity_a.clone(),
                special_1.clone(),
                Some(format!("er-rel-{idx}-cyclic-0")),
                Some(EdgeLabel {
                    width: 0.0,
                    height: 0.0,
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            // Mid segment: carries the relationship label, no markers.
            g.set_edge_named(
                special_1.clone(),
                special_2.clone(),
                Some(format!("er-rel-{idx}")),
                Some(EdgeLabel {
                    width: label_w.max(0.0),
                    height: label_h.max(0.0),
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            // Last segment: keep end marker, no label.
            g.set_edge_named(
                special_2.clone(),
                r.entity_a.clone(),
                Some(format!("er-rel-{idx}-cyclic-2")),
                Some(EdgeLabel {
                    width: 0.0,
                    height: 0.0,
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            continue;
        }

        let name = format!("er-rel-{idx}");
        let (label_w, label_h) = prepared_labels
            .relationship(idx)
            .map(ErPreparedRelationshipLabel::dimensions)
            .unwrap_or_default();
        g.set_edge_named(
            r.entity_a.clone(),
            r.entity_b.clone(),
            Some(name),
            Some(EdgeLabel {
                width: label_w.max(0.0),
                height: label_h.max(0.0),
                labelpos: LabelPos::C,
                labeloffset: 10.0,
                minlen: 1,
                weight: 1.0,
                ..Default::default()
            }),
        );
    }

    // The adapter reservation precedes this scratch and all helper expansion. Check the sticky
    // operation terminal again after label measurement, before reporting an invalid forest.
    work_control.checked_add(0, 0)?;
    let mut ordinal = 0;
    parent_assignments.retain(|(child, _)| {
        let keep = last_parent_assignment.get(child) == Some(&ordinal);
        ordinal += 1;
        keep
    });
    if let Err(error) = g.try_set_unparented_parents_ix(&parent_assignments) {
        let message = match error {
            dugong::graphlib::GraphError::ParentCycle {
                child_ix,
                parent_ix,
            } => format!(
                "ER parent cycle setting {} as parent of {}",
                g.node_id_by_ix(parent_ix).unwrap_or("<removed>"),
                g.node_id_by_ix(child_ix).unwrap_or("<removed>"),
            ),
            error => format!("invalid ER parent assignments: {error}"),
        };
        return Err(Error::InvalidModel { message });
    }

    let solved = {
        #[cfg(feature = "benchmark-stages")]
        let _stage = merman_model::benchmark_stages::enter(
            merman_model::benchmark_stages::Stage::LayoutSolve,
        );
        dugong::layout_controlled(&mut g, work_control)
    };
    solved.map_err(|error| work_control.map_dugong_error(error))?;

    let mut nodes: Vec<LayoutNode> = Vec::new();
    let mut clusters = Vec::new();
    for id in g.node_ids() {
        let Some(n) = g.node(&id) else {
            continue;
        };
        let is_cluster = subgraph_ids.contains(id.as_str());
        let x = n.x.unwrap_or(0.0);
        let y = n.y.unwrap_or(0.0);
        let mut width = n.width.max(1.0);
        let mut height = n.height.max(1.0);
        if is_cluster {
            let (title_width, title_height) = subgraph_title_metrics
                .get(&id)
                .copied()
                .unwrap_or((0.0, 0.0));
            // Mermaid's ER group renderer keeps an 8px title inset and widens empty groups to
            // contain their title. Compound Dagre groups already include their children; only
            // the title-driven minimum is added here.
            width = width.max(title_width + 16.0);
            height = height.max(title_height + 16.0);
            if let Some(subgraph) = model.subgraphs.iter().find(|subgraph| subgraph.id == id) {
                let padding = 8.0;
                let title_label = LayoutLabel {
                    x,
                    y: y - height / 2.0 + padding + title_height / 2.0,
                    width: title_width,
                    height: title_height,
                };
                let effective_dir = subgraph
                    .dir
                    .clone()
                    .unwrap_or_else(|| model.direction.clone());
                clusters.push(LayoutCluster {
                    id: id.clone(),
                    x,
                    y,
                    width,
                    height,
                    diff: (title_width - width) / 2.0 - padding / 2.0,
                    offset_y: title_height - padding / 2.0,
                    title: subgraph.title.clone(),
                    title_label,
                    requested_dir: subgraph.dir.clone(),
                    effective_dir,
                    padding,
                    title_margin_top: 0.0,
                    title_margin_bottom: 0.0,
                });
            }
        }
        nodes.push(LayoutNode {
            id: id.clone(),
            x,
            y,
            width,
            height,
            is_cluster,
            label_width: None,
            label_height: None,
        });
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    clusters.sort_by(|a, b| a.id.cmp(&b.id));

    let mut node_rect_by_id = HashMap::new();
    for n in &nodes {
        let half_width = n.width / 2.0;
        let half_height = n.height / 2.0;
        node_rect_by_id.insert(
            n.id.clone(),
            Bounds {
                min_x: n.x - half_width,
                min_y: n.y - half_height,
                max_x: n.x + half_width,
                max_y: n.y + half_height,
            },
        );
    }

    let mut edges_by_id: HashMap<String, LayoutEdgeParts> = HashMap::new();
    for key in g.edge_keys() {
        let Some(e) = g.edge_by_key(&key) else {
            continue;
        };
        let mut points = e
            .points
            .iter()
            .map(|p| LayoutPoint { x: p.x, y: p.y })
            .collect::<Vec<_>>();

        let id = key
            .name
            .clone()
            .unwrap_or_else(|| format!("edge:{}:{}", key.v, key.w));

        let rel_idx = key
            .name
            .as_ref()
            .and_then(|name| er_rel_idx_from_edge_id(name))
            .and_then(|idx| model.relationships.get(idx).map(|_| idx));

        let rel = rel_idx.and_then(|idx| model.relationships.get(idx));
        let (base_start_marker, base_end_marker, stroke_dasharray) = if let Some(rel) = rel {
            let card_a = rel.rel_spec.card_a.as_str();
            let card_b = rel.rel_spec.card_b.as_str();
            let rel_type = rel.rel_spec.rel_type.as_str();
            let start_marker = er_marker_id(card_b, "START");
            let end_marker = er_marker_id(card_a, "END");
            let stroke_dasharray = if rel_type == "NON_IDENTIFYING" {
                Some("8,8".to_string())
            } else {
                None
            };
            (start_marker, end_marker, stroke_dasharray)
        } else {
            (None, None, None)
        };

        if !is_er_self_loop_dummy_node_id(&key.v)
            && !is_er_self_loop_dummy_node_id(&key.w)
            && let (Some(from_rect), Some(to_rect)) =
                (node_rect_by_id.get(&key.v), node_rect_by_id.get(&key.w))
        {
            clip_edge_endpoints(&mut points, from_rect, to_rect);
        }

        let (start_marker, end_marker) =
            if is_er_self_loop_dummy_node_id(&key.v) && is_er_self_loop_dummy_node_id(&key.w) {
                (None, None)
            } else if id.ends_with("-cyclic-0") {
                (base_start_marker, None)
            } else if id.ends_with("-cyclic-2") {
                (None, base_end_marker)
            } else {
                (base_start_marker, base_end_marker)
            };

        let (label_width, label_height) = rel_idx
            .and_then(|index| prepared_labels.relationship(index))
            .map(ErPreparedRelationshipLabel::dimensions)
            .unwrap_or_default();
        let label = if label_width <= 0.0
            || label_height <= 0.0
            || id.ends_with("-cyclic-0")
            || id.ends_with("-cyclic-2")
        {
            None
        } else {
            // Mermaid uses Dagre's computed edge label center (`edge.x/edge.y`) rather than a
            // polyline midpoint. Prefer those coordinates when present.
            let (x, y) =
                e.x.zip(e.y)
                    .or_else(|| calc_label_position(&points))
                    .unwrap_or((0.0, 0.0));
            Some(LayoutLabel {
                x,
                y,
                width: label_width,
                height: label_height,
            })
        };

        edges_by_id.insert(
            id,
            LayoutEdgeParts {
                id: key
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("edge:{}:{}", key.v, key.w)),
                from: key.v.clone(),
                to: key.w.clone(),
                points,
                label,
                start_marker,
                end_marker,
                stroke_dasharray,
            },
        );
    }

    // Keep the Dagre graph projection intact for layout artifacts. Mermaid's common painter
    // consumes a second projection where self-loop segments are merged into one visible edge;
    // build that projection below without discarding the helper ranks from the canonical layout.
    let to_layout_edge = |edge: LayoutEdgeParts| LayoutEdge {
        id: edge.id,
        from: edge.from,
        to: edge.to,
        from_cluster: None,
        to_cluster: None,
        points: edge.points,
        label: edge.label,
        start_label_left: None,
        start_label_right: None,
        end_label_left: None,
        end_label_right: None,
        start_marker: edge.start_marker,
        end_marker: edge.end_marker,
        stroke_dasharray: edge.stroke_dasharray,
    };

    let mut layout_edges = edges_by_id
        .values()
        .cloned()
        .map(to_layout_edge)
        .collect::<Vec<_>>();
    layout_edges.sort_by(|left, right| left.id.cmp(&right.id));

    let mut render_edges: Vec<LayoutEdge> = Vec::with_capacity(model.relationships.len());
    for (idx, relationship) in model.relationships.iter().enumerate() {
        let edge_id = format!("er-rel-{idx}");
        if relationship.entity_a == relationship.entity_b {
            let first_id = format!("{edge_id}-cyclic-0");
            let last_id = format!("{edge_id}-cyclic-2");
            let first = edges_by_id.get(&first_id).cloned();
            let middle = edges_by_id.get(&edge_id).cloned();
            let last = edges_by_id.get(&last_id).cloned();

            let Some(node) = g.node(&relationship.entity_a).cloned() else {
                if let Some(edge) = middle.or(first).or(last) {
                    render_edges.push(to_layout_edge(edge));
                }
                continue;
            };
            let dummy_ids = [
                format!("{}---{}---1", relationship.entity_a, relationship.entity_a),
                format!("{}---{}---2", relationship.entity_a, relationship.entity_a),
            ];
            let mut hints = dummy_ids
                .iter()
                .filter_map(|id| g.node(id))
                .filter_map(|node| Some((node.x?, node.y?)))
                .collect::<Vec<_>>();
            if hints.is_empty() {
                for part in [first.as_ref(), middle.as_ref(), last.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    hints.extend(part.points.iter().map(|point| (point.x, point.y)));
                }
            }
            let side = er_self_loop_side(&node, hints, rankdir);
            let label = middle
                .as_ref()
                .and_then(|part| part.label.clone())
                .or_else(|| first.as_ref().and_then(|part| part.label.clone()))
                .or_else(|| last.as_ref().and_then(|part| part.label.clone()));
            let label_width = label.as_ref().map(|label| label.width).unwrap_or(0.0);
            let points = er_self_loop_points(&node, side, label_width);
            let label = label.map(|mut label| {
                let (x, y) =
                    er_self_loop_label_position(&points, side, label.width, label.height, &node);
                label.x = x;
                label.y = y;
                label
            });
            let self_loop_cluster = subgraph_ids
                .contains(relationship.entity_a.as_str())
                .then(|| relationship.entity_a.clone());

            render_edges.push(LayoutEdge {
                id: edge_id.clone(),
                from: relationship.entity_a.clone(),
                to: relationship.entity_b.clone(),
                from_cluster: self_loop_cluster.clone(),
                to_cluster: self_loop_cluster,
                points,
                label,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: er_marker_id(relationship.rel_spec.card_b.as_str(), "START"),
                end_marker: er_marker_id(relationship.rel_spec.card_a.as_str(), "END"),
                stroke_dasharray: (relationship.rel_spec.rel_type == "NON_IDENTIFYING")
                    .then(|| "8,8".to_string()),
            });
            continue;
        } else if let Some(edge) = edges_by_id.get(&edge_id).cloned() {
            render_edges.push(to_layout_edge(edge));
        }
    }

    let bounds = er_layout_bounds(&nodes, &layout_edges);

    Ok(ErDagreLayout {
        nodes,
        edges: layout_edges,
        render_edges,
        clusters,
        bounds,
    })
}

fn is_er_self_loop_dummy_node_id(id: &str) -> bool {
    // Mermaid's dagre renderer creates self-loop helper nodes using `${nodeId}---${nodeId}---{1|2}`.
    let Some((base, suffix)) = id.rsplit_once("---") else {
        return false;
    };
    if !matches!(suffix, "1" | "2") {
        return false;
    }
    let Some((left, right)) = base.split_once("---") else {
        return false;
    };
    left == right
}

#[derive(Debug, Clone)]
struct LayoutEdgeParts {
    id: String,
    from: String,
    to: String,
    points: Vec<LayoutPoint>,
    label: Option<LayoutLabel>,
    start_marker: Option<String>,
    end_marker: Option<String>,
    stroke_dasharray: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErSelfLoopSide {
    Top,
    Bottom,
    Left,
    Right,
}

fn er_default_self_loop_side(rankdir: RankDir) -> ErSelfLoopSide {
    match rankdir {
        RankDir::BT => ErSelfLoopSide::Bottom,
        RankDir::LR => ErSelfLoopSide::Right,
        RankDir::RL => ErSelfLoopSide::Left,
        RankDir::TB => ErSelfLoopSide::Top,
    }
}

fn er_self_loop_side(
    node: &NodeLabel,
    hints: impl IntoIterator<Item = (f64, f64)>,
    rankdir: RankDir,
) -> ErSelfLoopSide {
    let hints = hints.into_iter().collect::<Vec<_>>();
    if hints.is_empty() {
        return er_default_self_loop_side(rankdir);
    }

    let (sum_x, sum_y) = hints.iter().fold((0.0, 0.0), |(x, y), (hint_x, hint_y)| {
        (x + hint_x, y + hint_y)
    });
    let count = hints.len() as f64;
    let dx = sum_x / count - node.x.unwrap_or(0.0);
    let dy = sum_y / count - node.y.unwrap_or(0.0);
    if dx.abs() > dy.abs() {
        if dx > 0.0 {
            ErSelfLoopSide::Right
        } else {
            ErSelfLoopSide::Left
        }
    } else if dy > 0.0 {
        ErSelfLoopSide::Bottom
    } else if dy < 0.0 {
        ErSelfLoopSide::Top
    } else {
        er_default_self_loop_side(rankdir)
    }
}

fn er_self_loop_points(
    node: &NodeLabel,
    side: ErSelfLoopSide,
    label_width: f64,
) -> Vec<LayoutPoint> {
    let x = node.x.unwrap_or(0.0);
    let y = node.y.unwrap_or(0.0);
    let half_width = node.width / 2.0;
    let half_height = node.height / 2.0;
    let max_span = (node.width * 0.8).clamp(36.0, 100.0);
    let span = label_width.max(node.width * 0.35).clamp(36.0, max_span);
    let depth = node
        .width
        .min(node.height)
        .mul_add(0.45, 0.0)
        .clamp(24.0, 48.0);

    let mut points = match side {
        ErSelfLoopSide::Bottom => vec![
            LayoutPoint {
                x: x - span / 2.0,
                y: y + half_height,
            },
            LayoutPoint {
                x: x - span / 2.0,
                y: y + half_height + depth,
            },
            LayoutPoint {
                x: x + span / 2.0,
                y: y + half_height + depth,
            },
            LayoutPoint {
                x: x + span / 2.0,
                y: y + half_height,
            },
        ],
        ErSelfLoopSide::Right => vec![
            LayoutPoint {
                x: x + half_width,
                y: y - span / 2.0,
            },
            LayoutPoint {
                x: x + half_width + depth,
                y: y - span / 2.0,
            },
            LayoutPoint {
                x: x + half_width + depth,
                y: y + span / 2.0,
            },
            LayoutPoint {
                x: x + half_width,
                y: y + span / 2.0,
            },
        ],
        ErSelfLoopSide::Left => vec![
            LayoutPoint {
                x: x - half_width,
                y: y - span / 2.0,
            },
            LayoutPoint {
                x: x - half_width - depth,
                y: y - span / 2.0,
            },
            LayoutPoint {
                x: x - half_width - depth,
                y: y + span / 2.0,
            },
            LayoutPoint {
                x: x - half_width,
                y: y + span / 2.0,
            },
        ],
        ErSelfLoopSide::Top => vec![
            LayoutPoint {
                x: x - span / 2.0,
                y: y - half_height,
            },
            LayoutPoint {
                x: x - span / 2.0,
                y: y - half_height - depth,
            },
            LayoutPoint {
                x: x + span / 2.0,
                y: y - half_height - depth,
            },
            LayoutPoint {
                x: x + span / 2.0,
                y: y - half_height,
            },
        ],
    };

    // Mermaid's edge painter clips the first/last points against the node by shooting a ray from
    // the node center toward the adjacent inner bend. Keep the compact route's inner points
    // intact, but expose the same boundary intersections in the public LayoutEdge so the
    // headless SVG path/data-points match the browser renderer.
    if points.len() >= 4 {
        let first_inner = points[1].clone();
        let last_index = points.len() - 1;
        let last_inner = points[last_index - 1].clone();
        points[0] = er_intersect_node_rect(node, &first_inner);
        let last_boundary = er_intersect_node_rect(node, &last_inner);
        points[last_index] = last_boundary;
    }
    points
}

fn er_intersect_node_rect(node: &NodeLabel, point: &LayoutPoint) -> LayoutPoint {
    // Port of Mermaid's intersect.rect: the ray starts at the node center and ends at the
    // requested route point. This deliberately handles the self-loop endpoint case separately
    // from `clip_edge_endpoints`, whose segment clipping assumes the point is already inside the
    // rectangle and would leave raw boundary coordinates unchanged.
    let x = node.x.unwrap_or(0.0);
    let y = node.y.unwrap_or(0.0);
    let dx = point.x - x;
    let dy = point.y - y;
    let mut half_width = node.width / 2.0;
    let mut half_height = node.height / 2.0;

    let (sx, sy) = if dy.abs() * half_width > dx.abs() * half_height {
        if dy < 0.0 {
            half_height = -half_height;
        }
        let sx = if dy == 0.0 {
            0.0
        } else {
            (half_height * dx) / dy
        };
        (sx, half_height)
    } else {
        if dx < 0.0 {
            half_width = -half_width;
        }
        let sy = if dx == 0.0 {
            0.0
        } else {
            (half_width * dy) / dx
        };
        (half_width, sy)
    };

    LayoutPoint {
        x: x + sx,
        y: y + sy,
    }
}

fn er_self_loop_label_position(
    points: &[LayoutPoint],
    side: ErSelfLoopSide,
    label_width: f64,
    label_height: f64,
    node: &NodeLabel,
) -> (f64, f64) {
    let x = node.x.unwrap_or(0.0);
    let y = node.y.unwrap_or(0.0);
    let gap = 4.0;
    match side {
        ErSelfLoopSide::Bottom => (
            x,
            points
                .iter()
                .map(|point| point.y)
                .fold(f64::NEG_INFINITY, f64::max)
                + label_height / 2.0
                + gap,
        ),
        ErSelfLoopSide::Right => (
            points
                .iter()
                .map(|point| point.x)
                .fold(f64::NEG_INFINITY, f64::max)
                + label_width / 2.0
                + gap,
            y,
        ),
        ErSelfLoopSide::Left => (
            points
                .iter()
                .map(|point| point.x)
                .fold(f64::INFINITY, f64::min)
                - label_width / 2.0
                - gap,
            y,
        ),
        ErSelfLoopSide::Top => (
            x,
            points
                .iter()
                .map(|point| point.y)
                .fold(f64::INFINITY, f64::min)
                - label_height / 2.0
                - gap,
        ),
    }
}

fn intersect_segment_with_rect(
    p0: &LayoutPoint,
    p1: &LayoutPoint,
    rect: &Bounds,
) -> Option<LayoutPoint> {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    if dx == 0.0 && dy == 0.0 {
        return None;
    }

    let mut candidates: Vec<(f64, LayoutPoint)> = Vec::new();
    let eps = 1e-9;
    let min_x = rect.min_x;
    let max_x = rect.max_x;
    let min_y = rect.min_y;
    let max_y = rect.max_y;

    if dx.abs() > eps {
        for x_edge in [min_x, max_x] {
            let t = (x_edge - p0.x) / dx;
            if t < -eps || t > 1.0 + eps {
                continue;
            }
            let y = p0.y + t * dy;
            if y + eps >= min_y && y <= max_y + eps {
                candidates.push((t, LayoutPoint { x: x_edge, y }));
            }
        }
    }

    if dy.abs() > eps {
        for y_edge in [min_y, max_y] {
            let t = (y_edge - p0.y) / dy;
            if t < -eps || t > 1.0 + eps {
                continue;
            }
            let x = p0.x + t * dx;
            if x + eps >= min_x && x <= max_x + eps {
                candidates.push((t, LayoutPoint { x, y: y_edge }));
            }
        }
    }

    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    candidates
        .into_iter()
        .find(|(t, _)| *t >= 0.0)
        .map(|(_, p)| p)
}

fn contains_point(rect: &Bounds, point: &LayoutPoint) -> bool {
    point.x >= rect.min_x && point.x <= rect.max_x && point.y >= rect.min_y && point.y <= rect.max_y
}

fn clip_edge_endpoints(points: &mut [LayoutPoint], from: &Bounds, to: &Bounds) {
    if points.len() < 2 {
        return;
    }
    if contains_point(from, &points[0])
        && let Some(p) = intersect_segment_with_rect(&points[0], &points[1], from)
    {
        points[0] = p;
    }
    let last = points.len() - 1;
    if contains_point(to, &points[last])
        && let Some(p) = intersect_segment_with_rect(&points[last], &points[last - 1], to)
    {
        points[last] = p;
    }
}
