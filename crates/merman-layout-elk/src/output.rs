//! Project provider results into adapter coordinates, then join scoped edge segments.

use std::collections::{HashMap, HashSet};

use super::{
    Algorithm, EdgeLabelLayout, EdgeLayout, HierarchyIndex, LayoutResult, NodeLayout, Point,
    Result, ScopeLayout, ScopedEndpoint, TerminalLabelKey, WorkControl, WorkError, flat,
    source_port,
};
use source_port::{LGraph, LNodeKind, LPoint, PortRef};

impl HierarchyIndex<'_> {
    pub(super) fn complete_flat_hierarchy_edges(
        &self,
        scope: usize,
        arena: &[Option<ScopeLayout>],
        output: &mut flat::FlatLayout,
        work: &mut dyn WorkControl,
    ) -> Result<()> {
        let hierarchical = self.scopes[scope]
            .edges
            .iter()
            .filter(|edge| self.is_flat_hierarchy_edge(scope, edge))
            .collect::<Vec<_>>();
        if hierarchical.is_empty() {
            return Ok(());
        }
        work.check(hierarchical.len())?;
        work.charge(hierarchical.len())?;
        let spore = self.scopes[scope].algorithm == Algorithm::SporeOverlap;
        let mut geometry = HashMap::new();
        if spore {
            let mut required_scopes = HashSet::new();
            for edge in &hierarchical {
                let (ScopedEndpoint::Node(source), ScopedEndpoint::Node(target)) =
                    (edge.source, edge.target)
                else {
                    unreachable!("flat hierarchy endpoints are original nodes")
                };
                for node in [source, target] {
                    if Some(node) == self.scopes[scope].anchor {
                        continue;
                    }
                    required_scopes.insert(self.node_scope[node]);
                    if let Some(parent) = self.parent[node]
                        && Some(parent) != self.scopes[scope].anchor
                    {
                        required_scopes.insert(self.node_scope[parent]);
                    }
                }
            }
            for required in required_scopes {
                let nodes = if required == scope {
                    &output.layout.nodes
                } else {
                    &arena[required]
                        .as_ref()
                        .expect("descendant scopes are complete")
                        .layout
                        .nodes
                };
                work.check(nodes.len())?;
                work.charge(nodes.len())?;
                geometry.extend(nodes.iter().map(|node| (node.id.as_str(), node)));
            }
        }
        let mut completed = Vec::with_capacity(hierarchical.len());
        for edge in hierarchical {
            let original = &self.graph.edges[edge.original];
            let mut points = Vec::new();
            if spore {
                let endpoint = |endpoint| {
                    let ScopedEndpoint::Node(index) = endpoint else {
                        unreachable!("flat endpoint")
                    };
                    if Some(index) == self.scopes[scope].anchor {
                        // An edge to the layout parent is contained in that parent's inner
                        // graph. At this export point the parent has not been positioned yet.
                        return (
                            Point {
                                x: output.size.width / 2.0,
                                y: output.size.height / 2.0,
                            },
                            output.size.width,
                            output.size.height,
                        );
                    }
                    let node = geometry[self.graph.nodes[index].id.as_str()];
                    let mut center = Point {
                        x: node.x,
                        y: node.y,
                    };
                    if let Some(parent) = self.parent[index]
                        && Some(parent) != self.scopes[scope].anchor
                    {
                        if self.node_scope[parent] == self.node_scope[index] {
                            let parent = geometry[self.graph.nodes[parent].id.as_str()];
                            center.x -= parent.x - parent.width / 2.0;
                            center.y -= parent.y - parent.height / 2.0;
                        } else {
                            let parent_scope = self.node_scope[parent];
                            let shifts = if parent_scope == scope {
                                &output.content_shifts
                            } else {
                                &arena[parent_scope]
                                    .as_ref()
                                    .expect("parent scope is complete")
                                    .content_shifts
                            };
                            if let Some(shift) = shifts.get(&self.graph.nodes[parent].id) {
                                center.x += shift.x;
                                center.y += shift.y;
                            }
                        }
                    }
                    (center, node.width, node.height)
                };
                let (source, sw, sh) = endpoint(edge.source);
                let (target, tw, th) = endpoint(edge.target);
                // SPOrE ElkGraphImporter.applyPositions reads each endpoint's own local
                // coordinates, including for hierarchical edges. Do not add ancestor offsets.
                let start = flat::clip_spore_endpoint(source, target, sw, sh);
                let end = flat::clip_spore_endpoint(target, start, tw, th);
                points.extend([start, end]);
            }
            completed.push(EdgeLayout {
                id: original.id.clone(),
                points,
                labels: original
                    .label
                    .into_iter()
                    .map(|label| EdgeLabelLayout {
                        terminal: None,
                        x: output.edge_translation.x,
                        y: output.edge_translation.y,
                        width: label.width,
                        height: label.height,
                    })
                    .collect(),
            });
        }
        output.layout.edges.extend(completed);
        Ok(())
    }
}

pub(super) fn flatten_scope_layouts(
    index: &HierarchyIndex<'_>,
    mut arena: Vec<Option<ScopeLayout>>,
    work_control: &mut dyn WorkControl,
) -> Result<LayoutResult> {
    // Charge the planning scan before reading per-scope output geometry. The detailed tranche is
    // then charged before translation and compound-segment merging touch those points and labels.
    work_control.check(index.scopes.len())?;
    work_control.charge(index.scopes.len())?;
    let scoped_edge_count = index.scopes.iter().try_fold(0usize, |total, scope| {
        total
            .checked_add(scope.edges.len())
            .ok_or(WorkError::ArithmeticOverflow)
    })?;
    let flatten_work = index
        .scopes
        .len()
        .checked_mul(3)
        .and_then(|scope_work| scope_work.checked_add(index.graph.nodes.len()))
        .and_then(|work| work.checked_add(scoped_edge_count))
        .and_then(|work| work.checked_add(index.graph.edges.len().checked_mul(3)?))
        .ok_or(WorkError::ArithmeticOverflow)?;

    work_control.check(scoped_edge_count)?;
    work_control.charge(scoped_edge_count)?;
    let geometry_items = arena.iter().try_fold(0usize, |total, layout| {
        let layout = layout.as_ref().expect("postorder fills every scope result");
        layout.layout.edges.iter().try_fold(total, |total, edge| {
            total
                .checked_add(edge.points.len())
                .and_then(|total| total.checked_add(edge.labels.len()))
                .and_then(|total| total.checked_add(1))
                .ok_or(WorkError::ArithmeticOverflow)
        })
    })?;
    let geometry_work = geometry_items
        .checked_mul(2)
        .ok_or(WorkError::ArithmeticOverflow)?;
    let execution_work = flatten_work
        .checked_add(geometry_work)
        .ok_or(WorkError::ArithmeticOverflow)?;
    work_control.check(execution_work)?;
    work_control.charge(execution_work)?;

    let mut offsets = vec![Point { x: 0.0, y: 0.0 }; index.scopes.len()];
    let mut order = Vec::with_capacity(index.scopes.len());
    let mut stack = vec![0usize];
    while let Some(scope) = stack.pop() {
        order.push(scope);
        let scope_result = arena[scope]
            .as_ref()
            .expect("scope layout exists before flatten");
        let layout = &scope_result.layout;
        let child_by_id = index.scopes[scope]
            .children
            .iter()
            .map(|child| {
                let anchor = index.scopes[*child]
                    .anchor
                    .expect("child scope must have an anchor");
                (index.graph.nodes[anchor].id.as_str(), *child)
            })
            .collect::<HashMap<_, _>>();
        let mut children = Vec::new();
        for node in &layout.nodes {
            let Some(&child) = child_by_id.get(node.id.as_str()) else {
                continue;
            };
            offsets[child] = Point {
                // Mermaid accumulates nested coordinates from group top-left positions, while the
                // public Merman node layout is center-based.
                x: offsets[scope].x + node.x - node.width / 2.0
                    + scope_result
                        .content_shifts
                        .get(&node.id)
                        .map_or(0.0, |shift| shift.x),
                y: offsets[scope].y + node.y - node.height / 2.0
                    + scope_result
                        .content_shifts
                        .get(&node.id)
                        .map_or(0.0, |shift| shift.y),
            };
            children.push(child);
        }
        stack.extend(children.into_iter().rev());
    }
    let node_capacity = arena.iter().try_fold(0usize, |total, layout| {
        total
            .checked_add(
                layout
                    .as_ref()
                    .expect("postorder fills every scope result")
                    .layout
                    .nodes
                    .len(),
            )
            .ok_or(WorkError::ArithmeticOverflow)
    })?;
    let mut nodes = Vec::with_capacity(node_capacity);
    let mut edge_slots = std::iter::repeat_with(|| None)
        .take(index.graph.edges.len())
        .collect::<Vec<Option<EdgeLayout>>>();
    let mut compound_edges = std::iter::repeat_with(|| None)
        .take(index.graph.edges.len())
        .collect::<Vec<Option<Vec<Option<CompoundEdgeLayoutSegment>>>>>();
    for &scope in &order {
        let offset = offsets[scope];
        let mut scope_layout = arena[scope].take().expect("each scope is flattened once");
        for node in &mut scope_layout.layout.nodes {
            node.x += offset.x;
            node.y += offset.y;
        }
        nodes.append(&mut scope_layout.layout.nodes);
        for mut edge in scope_layout.layout.edges {
            translate_edge_layout(&mut edge, offset);
            let metadata = scope_layout
                .edge_metadata
                .get(edge.id.as_str())
                .copied()
                .expect("scope output edge must retain its source edge metadata");
            if let Some(segment) = metadata.segment {
                let entry = compound_edges[metadata.original]
                    .get_or_insert_with(|| vec![None; segment.count]);
                debug_assert_eq!(entry.len(), segment.count);
                entry[segment.order] = Some(CompoundEdgeLayoutSegment {
                    original_edge_id: edge.id.clone(),
                    segment: segment.segment,
                    model_order: segment.model_order,
                    edge,
                });
            } else {
                debug_assert!(edge_slots[metadata.original].is_none());
                edge_slots[metadata.original] = Some(edge);
            }
        }
    }

    for (original, segments) in compound_edges.into_iter().enumerate() {
        let Some(segments) = segments else {
            continue;
        };
        let segments = segments.into_iter().flatten().collect::<Vec<_>>();
        if let Some(edge) = merge_compound_edge_segments(
            &segments,
            index.graph.options.layered.unnecessary_bendpoints,
        ) {
            debug_assert!(edge_slots[original].is_none());
            edge_slots[original] = Some(edge);
        }
    }

    // ELK derives importer/model order from an edge's containing graph before moving an
    // ancestor-descendant edge into that endpoint's inner graph. A direct parent-child edge can
    // therefore be owned by the parent scope without materializing a segment there. Emit from
    // `owned_edges`, not first-seen segment order; the nested iteration also avoids an O(E) copy.
    let mut edges = Vec::with_capacity(index.graph.edges.len());
    for &scope in &order {
        for &original in &index.scopes[scope].owned_edges {
            if let Some(edge) = edge_slots[original].take() {
                edges.push(edge);
            }
        }
    }
    // Valid adapter inputs should have announced every rendered edge from its owner scope. Keep a
    // deterministic tail in release builds if a future source processor suppresses that marker.
    edges.extend(edge_slots.into_iter().flatten());
    Ok(LayoutResult { nodes, edges })
}

fn translate_edge_layout(edge: &mut EdgeLayout, offset: Point) {
    for point in &mut edge.points {
        point.x += offset.x;
        point.y += offset.y;
    }
    for label in &mut edge.labels {
        label.x += offset.x;
        label.y += offset.y;
    }
}

pub(super) fn actual_source_graph_size(graph: &LGraph) -> source_port::LSize {
    graph.exported_root_size()
}

pub(super) fn source_graph_output_work_units(
    graph: &LGraph,
) -> std::result::Result<usize, WorkError> {
    let mut total = 0usize;
    let mut output_edge_upper_bound = 0usize;
    let mut stack = vec![graph];
    while let Some(current) = stack.pop() {
        // A merged hierarchy edge may expand back into several original rendered edges. Source
        // edges plus explicit cross-hierarchy records conservatively bound the final stable sort.
        output_edge_upper_bound = output_edge_upper_bound
            .checked_add(current.edges.len())
            .and_then(|total| total.checked_add(current.cross_hierarchy_edges.len()))
            .ok_or(WorkError::ArithmeticOverflow)?;
        total = total
            .checked_add(current.layerless_nodes.len())
            .and_then(|total| total.checked_add(current.edges.len()))
            .and_then(|total| total.checked_add(current.cross_hierarchy_edges.len()))
            .ok_or(WorkError::ArithmeticOverflow)?;
        for edge in &current.edges {
            let geometry = edge
                .bend_points
                .len()
                .checked_add(edge.labels.len())
                .and_then(|geometry| geometry.checked_add(3))
                .ok_or(WorkError::ArithmeticOverflow)?;
            total = total
                .checked_add(geometry)
                .ok_or(WorkError::ArithmeticOverflow)?;
            if edge.compound_segment.is_some() {
                total = total
                    .checked_add(
                        geometry
                            .checked_mul(2)
                            .ok_or(WorkError::ArithmeticOverflow)?,
                    )
                    .ok_or(WorkError::ArithmeticOverflow)?;
            }
        }
        for segment in &current.cross_hierarchy_edges {
            let edge = current
                .edges
                .get(segment.edge)
                .expect("compound segment must reference a graph edge");
            let geometry = edge
                .bend_points
                .len()
                .checked_add(edge.labels.len())
                .and_then(|geometry| geometry.checked_add(3))
                .and_then(|geometry| geometry.checked_mul(2))
                .ok_or(WorkError::ArithmeticOverflow)?;
            total = total
                .checked_add(geometry)
                .ok_or(WorkError::ArithmeticOverflow)?;
        }
        stack.extend(
            current
                .layerless_nodes
                .iter()
                .filter_map(|node| node.nested_graph.as_deref()),
        );
    }
    total = total
        .checked_add(comparison_sort_work_units(output_edge_upper_bound)?)
        .ok_or(WorkError::ArithmeticOverflow)?;
    Ok(total.max(1))
}

pub(super) fn comparison_sort_work_units(items: usize) -> std::result::Result<usize, WorkError> {
    if items < 2 {
        return Ok(0);
    }
    let levels = usize::BITS as usize - (items - 1).leading_zeros() as usize;
    items
        .checked_mul(levels)
        .ok_or(WorkError::ArithmeticOverflow)
}

pub(super) fn source_graph_to_layout_result(graph: &LGraph) -> LayoutResult {
    source_graph_to_layout_result_with_ordering(graph, true)
}

fn source_graph_to_layout_result_with_ordering(graph: &LGraph, order_edges: bool) -> LayoutResult {
    let mut result = SourceLayoutAccumulator {
        add_unnecessary_bendpoints: graph.options.unnecessary_bendpoints,
        ..Default::default()
    };
    append_source_graph_layout(graph, LPoint::default(), &mut result);
    result.into_layout_result(order_edges)
}

#[derive(Debug, Default)]
struct SourceLayoutAccumulator {
    nodes: Vec<NodeLayout>,
    node_ids: HashSet<String>,
    edges: Vec<OrderedEdgeLayout>,
    compound_edges: HashMap<String, Vec<CompoundEdgeLayoutSegment>>,
    add_unnecessary_bendpoints: bool,
}

impl SourceLayoutAccumulator {
    fn into_layout_result(mut self, order_edges: bool) -> LayoutResult {
        for segments in self.compound_edges.values_mut() {
            order_compound_layout_segments(segments);
            if let Some(edge) =
                merge_compound_edge_segments(segments, self.add_unnecessary_bendpoints)
            {
                self.edges.push(OrderedEdgeLayout {
                    model_order: segments
                        .iter()
                        .filter_map(|segment| segment.model_order)
                        .min(),
                    edge,
                });
            }
        }
        if order_edges {
            self.edges.sort_by(|left, right| {
                left.model_order
                    .unwrap_or(usize::MAX)
                    .cmp(&right.model_order.unwrap_or(usize::MAX))
                    .then_with(|| left.edge.id.cmp(&right.edge.id))
            });
        }

        LayoutResult {
            nodes: self.nodes,
            edges: self.edges.into_iter().map(|ordered| ordered.edge).collect(),
        }
    }
}

pub(super) fn order_compound_layout_segments(segments: &mut Vec<CompoundEdgeLayoutSegment>) {
    if segments.len() < 2 {
        return;
    }

    // ELK's compound postprocessor orders Output segments by descending hierarchy depth, then
    // Input segments by ascending depth before concatenation. Imported paths make each side's
    // depths unique and contiguous, so direct placement preserves that order without O(n log n)
    // comparison sorting.
    let mut output_min = usize::MAX;
    let mut output_max = 0usize;
    let mut output_count = 0usize;
    let mut input_min = usize::MAX;
    let mut input_max = 0usize;
    let mut input_count = 0usize;
    for segment in segments.iter() {
        match segment.segment {
            source_port::CompoundEdgeSegment::Output { depth } => {
                output_min = output_min.min(depth);
                output_max = output_max.max(depth);
                output_count += 1;
            }
            source_port::CompoundEdgeSegment::Input { depth } => {
                input_min = input_min.min(depth);
                input_max = input_max.max(depth);
                input_count += 1;
            }
        }
    }

    let output_span = if output_count == 0 {
        0
    } else {
        output_max - output_min + 1
    };
    let input_span = if input_count == 0 {
        0
    } else {
        input_max - input_min + 1
    };
    assert_eq!(
        output_span, output_count,
        "compound output segments must have unique contiguous depths"
    );
    assert_eq!(
        input_span, input_count,
        "compound input segments must have unique contiguous depths"
    );

    let mut ordered = std::iter::repeat_with(|| None)
        .take(segments.len())
        .collect::<Vec<Option<CompoundEdgeLayoutSegment>>>();
    for segment in std::mem::take(segments) {
        let position = match segment.segment {
            source_port::CompoundEdgeSegment::Output { depth } => output_max - depth,
            source_port::CompoundEdgeSegment::Input { depth } => output_count + depth - input_min,
        };
        assert!(
            ordered[position].replace(segment).is_none(),
            "compound segment path position must be unique"
        );
    }
    segments.extend(
        ordered
            .into_iter()
            .map(|segment| segment.expect("compound segment path must be contiguous")),
    );
}

#[derive(Debug, Clone)]
struct OrderedEdgeLayout {
    model_order: Option<usize>,
    edge: EdgeLayout,
}

fn append_source_graph_layout(
    graph: &LGraph,
    parent_origin: LPoint,
    result: &mut SourceLayoutAccumulator,
) {
    enum Frame<'a> {
        Enter {
            graph: &'a LGraph,
            parent_origin: LPoint,
        },
        Exit {
            graph: &'a LGraph,
            graph_origin: LPoint,
        },
    }

    let mut frames = vec![Frame::Enter {
        graph,
        parent_origin,
    }];
    while let Some(frame) = frames.pop() {
        let (graph, parent_origin) = match frame {
            Frame::Enter {
                graph,
                parent_origin,
            } => (graph, parent_origin),
            Frame::Exit {
                graph,
                graph_origin,
            } => {
                append_source_graph_edges(graph, graph_origin, result);
                continue;
            }
        };
        let graph_origin = LPoint {
            x: parent_origin.x + graph.offset.x + graph.padding.left,
            y: parent_origin.y + graph.offset.y + graph.padding.top,
        };
        for node in graph
            .layerless_nodes
            .iter()
            .filter(|node| node.kind == LNodeKind::Normal)
        {
            result.node_ids.insert(node.id.clone());
            result.nodes.push(NodeLayout {
                id: node.id.clone(),
                x: graph_origin.x + node.position.x + node.size.width / 2.0,
                y: graph_origin.y + node.position.y + node.size.height / 2.0,
                width: node.size.width,
                height: node.size.height,
            });
        }
        frames.push(Frame::Exit {
            graph,
            graph_origin,
        });
        frames.extend(graph.layerless_nodes.iter().rev().filter_map(|node| {
            node.nested_graph
                .as_deref()
                .map(|nested_graph| Frame::Enter {
                    graph: nested_graph,
                    parent_origin: LPoint {
                        x: graph_origin.x + node.position.x,
                        y: graph_origin.y + node.position.y,
                    },
                })
        }));
    }
}

fn append_source_graph_edges(
    graph: &LGraph,
    graph_origin: LPoint,
    result: &mut SourceLayoutAccumulator,
) {
    let mut compound_segments_by_edge: HashMap<usize, Vec<CompoundLayoutSegment>> = HashMap::new();
    for segment in &graph.cross_hierarchy_edges {
        compound_segments_by_edge
            .entry(segment.edge)
            .or_default()
            .push(CompoundLayoutSegment {
                original_edge_id: segment.original_edge_id.clone(),
                model_order: segment.original_model_order,
                segment: segment.segment,
            });
    }

    for (edge_index, edge) in graph.edges.iter().enumerate() {
        let compound_segments = compound_segments_by_edge
            .remove(&edge_index)
            .or_else(|| {
                edge.compound_segment.map(|segment| {
                    vec![CompoundLayoutSegment {
                        original_edge_id: edge.id.clone(),
                        model_order: edge.model_order,
                        segment,
                    }]
                })
            })
            .unwrap_or_default();
        if compound_segments.is_empty() {
            if !edge_has_layout_endpoints(graph, result, edge_index, edge) {
                continue;
            }
        } else if !graph.edge_source_attached(edge_index) || !graph.edge_target_attached(edge_index)
        {
            continue;
        }

        let edge_layout = EdgeLayout {
            id: edge.id.clone(),
            points: edge_points(graph, edge)
                .into_iter()
                .map(|point| Point {
                    x: graph_origin.x + point.x,
                    y: graph_origin.y + point.y,
                })
                .collect(),
            labels: edge_labels(graph_origin, edge),
        };

        if !compound_segments.is_empty() {
            for segment in compound_segments {
                let original_edge_id = segment.original_edge_id.clone();
                let edge_layout = edge_layout_for_original_edge(
                    &edge_layout,
                    graph_origin,
                    edge,
                    original_edge_id.as_str(),
                );
                result
                    .compound_edges
                    .entry(original_edge_id)
                    .or_default()
                    .push(CompoundEdgeLayoutSegment {
                        original_edge_id: segment.original_edge_id,
                        segment: segment.segment,
                        model_order: segment.model_order.or(edge.model_order),
                        edge: edge_layout,
                    });
            }
        } else {
            result.edges.push(OrderedEdgeLayout {
                model_order: edge.model_order,
                edge: edge_layout,
            });
        }
    }
}

fn edge_layout_for_original_edge(
    edge: &EdgeLayout,
    graph_origin: LPoint,
    source_edge: &source_port::LayeredEdge,
    original_edge_id: &str,
) -> EdgeLayout {
    let mut edge = edge.clone();
    edge.id = original_edge_id.to_string();
    edge.labels = edge_labels_for_original_edge(graph_origin, source_edge, original_edge_id);
    edge
}

fn edge_labels_for_original_edge(
    graph_origin: LPoint,
    edge: &source_port::LayeredEdge,
    original_edge_id: &str,
) -> Vec<EdgeLabelLayout> {
    edge.labels
        .iter()
        .filter(|label| {
            label
                .original_label_edge
                .as_deref()
                .unwrap_or(original_edge_id)
                == original_edge_id
        })
        .map(|label| EdgeLabelLayout {
            terminal: label
                .source_index
                .and_then(TerminalLabelKey::from_source_index),
            x: graph_origin.x + label.position.x,
            y: graph_origin.y + label.position.y,
            width: label.size.width,
            height: label.size.height,
        })
        .collect()
}

#[derive(Debug, Clone)]
struct CompoundLayoutSegment {
    original_edge_id: String,
    model_order: Option<usize>,
    segment: source_port::CompoundEdgeSegment,
}

#[derive(Debug, Clone)]
pub(super) struct CompoundEdgeLayoutSegment {
    pub(super) original_edge_id: String,
    pub(super) segment: source_port::CompoundEdgeSegment,
    pub(super) model_order: Option<usize>,
    pub(super) edge: EdgeLayout,
}

/// Merge hierarchy-local edge segments following ELK's compound postprocessor.
///
/// Source:
/// https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/compound/CompoundGraphPostprocessor.java
fn merge_compound_edge_segments(
    segments: &[CompoundEdgeLayoutSegment],
    add_unnecessary_bendpoints: bool,
) -> Option<EdgeLayout> {
    let first = segments.first()?;
    let mut points = Vec::new();
    let mut labels = Vec::new();
    let mut last_point = None;

    if let Some(source) = first.edge.points.first().copied() {
        push_distinct_point(&mut points, source);
    }

    for segment in segments {
        let segment_points = &segment.edge.points;
        if segment_points.is_empty() {
            continue;
        }

        let bend_points = if segment_points.len() > 2 {
            &segment_points[1..segment_points.len() - 1]
        } else {
            &[][..]
        };

        if let (Some(previous), Some(next)) = (
            last_point,
            bend_points.first().or_else(|| segment_points.last()),
        ) && compound_boundary_needs_bendpoint(previous, *next, add_unnecessary_bendpoints)
            && let Some(source) = segment_points.first()
        {
            push_distinct_point(&mut points, *source);
        }

        for point in bend_points {
            push_distinct_point(&mut points, *point);
        }
        labels.extend(segment.edge.labels.iter().cloned());
        last_point = bend_points
            .last()
            .copied()
            .or_else(|| segment_points.first().copied());
    }

    if let Some(target) = segments
        .last()
        .and_then(|segment| segment.edge.points.last())
        .copied()
    {
        push_distinct_point(&mut points, target);
    }

    Some(EdgeLayout {
        id: first.original_edge_id.clone(),
        points,
        labels,
    })
}

fn compound_boundary_needs_bendpoint(
    previous: Point,
    next: Point,
    add_unnecessary_bendpoints: bool,
) -> bool {
    const ORTHOGONAL_TOLERANCE: f64 = 0.001;
    let x_diff_enough = (previous.x - next.x).abs() > ORTHOGONAL_TOLERANCE;
    let y_diff_enough = (previous.y - next.y).abs() > ORTHOGONAL_TOLERANCE;
    if add_unnecessary_bendpoints {
        x_diff_enough || y_diff_enough
    } else {
        x_diff_enough && y_diff_enough
    }
}

fn push_distinct_point(points: &mut Vec<Point>, point: Point) {
    if points.last().is_some_and(|last| *last == point) {
        return;
    }
    points.push(point);
}

fn edge_has_layout_endpoints(
    graph: &LGraph,
    result: &SourceLayoutAccumulator,
    edge_index: usize,
    edge: &source_port::LayeredEdge,
) -> bool {
    if !graph.edge_source_attached(edge_index) || !graph.edge_target_attached(edge_index) {
        return false;
    }

    endpoint_has_layout(graph, result, edge.source, edge.source_node_id.as_str())
        && endpoint_has_layout(graph, result, edge.target, edge.target_node_id.as_str())
}

fn endpoint_has_layout(
    graph: &LGraph,
    result: &SourceLayoutAccumulator,
    endpoint: PortRef,
    original_node_id: &str,
) -> bool {
    graph
        .layerless_nodes
        .get(endpoint.node)
        .is_some_and(|node| node.kind == LNodeKind::Normal)
        || result.node_ids.contains(original_node_id)
}

fn edge_points(graph: &LGraph, edge: &source_port::LayeredEdge) -> Vec<source_port::LPoint> {
    let mut points = Vec::with_capacity(edge.bend_points.len() + 2);
    points.push(port_anchor(graph, edge.source));
    points.extend(edge.bend_points.iter().copied());
    points.push(port_anchor(graph, edge.target));
    points
}

fn edge_labels(graph_origin: LPoint, edge: &source_port::LayeredEdge) -> Vec<EdgeLabelLayout> {
    edge.labels
        .iter()
        .map(|label| EdgeLabelLayout {
            terminal: label
                .source_index
                .and_then(TerminalLabelKey::from_source_index),
            x: graph_origin.x + label.position.x,
            y: graph_origin.y + label.position.y,
            width: label.size.width,
            height: label.size.height,
        })
        .collect()
}

fn port_anchor(graph: &LGraph, port_ref: PortRef) -> source_port::LPoint {
    let node = &graph.layerless_nodes[port_ref.node];
    let port = &node.ports[port_ref.port];
    source_port::LPoint {
        x: node.position.x + port.position.x + port.anchor.x,
        y: node.position.y + port.position.y + port.anchor.y,
    }
}
