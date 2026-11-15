//! Whole-scene candidates for compatible common-source branches in flat, canonical scenes.
//! Admission against node, route, label, and marker occupancy remains the caller's job.

use super::layout_allocation_failed;
use super::path::StepDirection;
use super::plan::{NodeAttachment, NodeAttachments};
use crate::error::Result;
use crate::graph::charset::GraphCharset;
use crate::graph::label::try_measure_edge_label;
use crate::graph::layout::{CanvasCoord, GraphLayout, NodeLayout, charge_sort_work};
use crate::graph::model::{AsciiGraph, AsciiGraphEdge, GraphDirection, GraphEdgeMarker};
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_model::OperationPhase;
use rustc_hash::FxHashMap as HashMap;

const READING_MARGIN: usize = 1;
const TURN_CELLS: usize = 1;

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct BranchAllocation {
    branches: Vec<Option<AllocatedBranch>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AllocatedBranch {
    /// The first canonical edge index of this complete common-source group.
    pub(super) group: usize,
    pub(super) attachments: NodeAttachments,
    /// Source berth, source/channel turn, channel/target turn, target berth.
    pub(super) points: [CanvasCoord; 4],
    /// Inclusive host endpoints strictly inside the private target leg. The caller must use
    /// these instead of choosing a label host on the shared source stem or channel.
    /// A missing host is allowed only for an edge without a nonempty normalized label.
    pub(super) label_host: Option<[CanvasCoord; 2]>,
}

#[derive(Clone, Copy)]
struct IndexedEdge {
    canonical: usize,
    source: usize,
    target: usize,
    target_cross: usize,
}

impl IndexedEdge {
    fn unordered_pair(self) -> (usize, usize) {
        (self.source.min(self.target), self.source.max(self.target))
    }
}

#[derive(Clone, Copy)]
struct Port {
    attachment: NodeAttachment,
    berth: CanvasCoord,
}

struct AllocationScene<'a> {
    layout: &'a GraphLayout,
    edges: &'a [AsciiGraphEdge],
    charset: &'a GraphCharset,
    resources: &'a ResourceContext,
    execution: AsciiExecution<'a>,
    horizontal: bool,
}

impl BranchAllocation {
    pub(super) fn try_new(
        graph: &AsciiGraph,
        layout: &GraphLayout,
        canonical_edges: &[AsciiGraphEdge],
        charset: &GraphCharset,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        execution.checkpoint(OperationPhase::Layout)?;
        resources.checkpoint()?;
        if !graph.groups.is_empty()
            || !layout.groups.is_empty()
            || canonical_edges.len() < 2
            || layout.nodes.is_empty()
        {
            return Ok(Self::default());
        }

        // Share the caller's ledger, including during normalized label measurement, even
        // when this entry point is called with an otherwise uncontrolled resource context.
        let controlled = execution.resource_context(resources, OperationPhase::Layout);
        let scene = AllocationScene {
            layout,
            edges: canonical_edges,
            charset,
            resources: &controlled,
            execution,
            horizontal: graph.direction.canonical() == GraphDirection::LeftRight,
        };
        let resources = scene.resources;

        // All model-sized storage is admitted before reservation. Index endpoints once;
        // the sorts below compare only integers, rather than repeatedly scanning IDs.
        resources.charge_layout_work(layout.nodes.len())?;
        let mut node_indices = HashMap::default();
        node_indices
            .try_reserve(layout.nodes.len())
            .map_err(|_| layout_allocation_failed())?;
        for (index, node) in layout.nodes.iter().enumerate() {
            scene.scan()?;
            if node.width == 0 || node.height == 0 {
                return Ok(Self::default());
            }
            // NodeAttachment and the shape semantics use right/bottom/center helpers;
            // establish their arithmetic invariants before any shape resolution or sort.
            resources.checked_grid_add(node.x, node.width - 1)?;
            resources.checked_grid_add(node.y, node.height - 1)?;
            resources.charge_layout_work(node.id.len())?;
            if node_indices.insert(node.id.as_str(), index).is_some() {
                return Ok(Self::default());
            }
        }

        resources.charge_layout_work(layout.nodes.len())?;
        let mut blocked_sources = Vec::new();
        blocked_sources
            .try_reserve_exact(layout.nodes.len())
            .map_err(|_| layout_allocation_failed())?;
        blocked_sources.resize(layout.nodes.len(), false);

        resources.charge_layout_work(canonical_edges.len())?;
        let mut indexed = Vec::new();
        indexed
            .try_reserve_exact(canonical_edges.len())
            .map_err(|_| layout_allocation_failed())?;
        for (canonical, edge) in canonical_edges.iter().enumerate() {
            scene.scan()?;
            resources
                .charge_layout_work(resources.checked_work_add(edge.from.len(), edge.to.len())?)?;
            let (Some(&source), Some(&target)) = (
                node_indices.get(edge.from.as_str()),
                node_indices.get(edge.to.as_str()),
            ) else {
                // An unresolved real endpoint must never alias an existing endpoint.
                return Ok(Self::default());
            };
            blocked_sources[source] |= source == target;
            indexed.push(IndexedEdge {
                canonical,
                source,
                target,
                target_cross: scene.cross_center(&layout.nodes[target]),
            });
        }

        // A repeated unordered pair means duplicate targets, parallel multiplicity, or
        // reciprocal edges. Block every participating source, including invisible edges,
        // without an adjacency matrix or a per-edge scan of the rest of the scene.
        scene.execution.checkpoint(OperationPhase::Layout)?;
        charge_sort_work(indexed.len(), resources)?;
        indexed.sort_unstable_by_key(|edge| (edge.unordered_pair(), edge.source, edge.canonical));
        scene.execution.checkpoint(OperationPhase::Layout)?;
        for pair in indexed.windows(2) {
            scene.scan()?;
            if pair[0].unordered_pair() == pair[1].unordered_pair() {
                blocked_sources[pair[0].source] = true;
                blocked_sources[pair[1].source] = true;
            }
        }

        charge_sort_work(indexed.len(), resources)?;
        indexed.sort_unstable_by_key(|edge| (edge.source, edge.target_cross, edge.canonical));
        scene.execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(canonical_edges.len())?;
        let mut branches = Vec::new();
        branches
            .try_reserve_exact(canonical_edges.len())
            .map_err(|_| layout_allocation_failed())?;
        branches.resize(canonical_edges.len(), None);

        let mut first = 0;
        while first < indexed.len() {
            scene.scan()?;
            let source = indexed[first].source;
            let mut end = resources.checked_work_add(first, 1)?;
            while end < indexed.len() && indexed[end].source == source {
                scene.scan()?;
                end = resources.checked_work_add(end, 1)?;
            }
            if !blocked_sources[source] && end - first >= 2 {
                scene.allocate_group(&indexed[first..end], &mut branches)?;
            }
            first = end;
        }
        execution.checkpoint(OperationPhase::Layout)?;
        Ok(Self { branches })
    }

    pub(super) fn get(&self, canonical_edge_index: usize) -> Option<&AllocatedBranch> {
        self.branches.get(canonical_edge_index)?.as_ref()
    }
}

impl AllocationScene<'_> {
    fn scan(&self) -> Result<()> {
        self.execution.checkpoint(OperationPhase::Layout)?;
        self.resources.charge_layout_work(1)
    }

    fn cross_center(&self, node: &NodeLayout) -> usize {
        if self.horizontal {
            node.center_y()
        } else {
            node.center_x()
        }
    }

    fn main(&self, coord: CanvasCoord) -> usize {
        if self.horizontal { coord.x } else { coord.y }
    }

    fn coord(&self, main: usize, cross: usize) -> CanvasCoord {
        if self.horizontal {
            CanvasCoord { x: main, y: cross }
        } else {
            CanvasCoord { x: cross, y: main }
        }
    }

    fn port(&self, node: &NodeLayout, source: bool) -> Result<Option<Port>> {
        self.scan()?;
        let cross = self.cross_center(node);
        let (probe_main, inward) = match (self.horizontal, source) {
            (true, true) => (
                Some(self.resources.checked_grid_add(node.right(), 1)?),
                StepDirection::Left,
            ),
            (false, true) => (
                Some(self.resources.checked_grid_add(node.bottom(), 1)?),
                StepDirection::Up,
            ),
            (true, false) => (node.x.checked_sub(1), StepDirection::Right),
            (false, false) => (node.y.checked_sub(1), StepDirection::Down),
        };
        let Some(probe_main) = probe_main else {
            return Ok(None);
        };
        let Some(attachment) =
            NodeAttachment::resolve(node, self.coord(probe_main, cross), inward, self.charset)
        else {
            return Ok(None);
        };
        let contact_main = self.main(attachment.contact);
        let berth_main = if source {
            self.resources.checked_grid_add(contact_main, 1)?
        } else {
            let Some(main) = contact_main.checked_sub(1) else {
                return Ok(None);
            };
            main
        };
        Ok(Some(Port {
            attachment,
            berth: self.coord(berth_main, cross),
        }))
    }

    fn allocate_group(
        &self,
        group: &[IndexedEdge],
        branches: &mut [Option<AllocatedBranch>],
    ) -> Result<()> {
        let mut first_canonical = usize::MAX;
        let mut previous_cross = None;
        for indexed in group {
            self.scan()?;
            let edge = &self.edges[indexed.canonical];
            let baseline = &self.edges[group[0].canonical];
            if !edge.stroke.is_visible()
                || edge.start_marker != GraphEdgeMarker::Open
                || edge.stroke != baseline.stroke
                || edge.style != baseline.style
                // Distinct targets on the same intercept would share their target legs.
                || previous_cross == Some(indexed.target_cross)
            {
                return Ok(());
            }
            previous_cross = Some(indexed.target_cross);
            first_canonical = first_canonical.min(indexed.canonical);
        }

        let source = &self.layout.nodes[group[0].source];
        let Some(start) = self.port(source, true)? else {
            return Ok(());
        };
        let source_far = if self.horizontal {
            source.right()
        } else {
            source.bottom()
        };
        let clearance = self
            .resources
            .checked_grid_add(READING_MARGIN, TURN_CELLS)?;
        // Use the first available channel to maximize the private target label spans.
        // Bounding envelopes conservatively contain every actual shape contour.
        let channel = self.resources.checked_grid_add(source_far, clearance)?;
        for (position, indexed) in group.iter().enumerate() {
            self.scan()?;
            let branch = self.branch(*indexed, first_canonical, start, source_far, channel)?;
            let Some(branch) = branch else {
                // A group is all-or-nothing; never leave a compatible subset after a late
                // target, authored-length, or full-label fit failure.
                for previous in &group[..position] {
                    self.scan()?;
                    branches[previous.canonical] = None;
                }
                return Ok(());
            };
            branches[indexed.canonical] = Some(branch);
        }
        Ok(())
    }

    fn branch(
        &self,
        indexed: IndexedEdge,
        group: usize,
        start: Port,
        source_far: usize,
        channel: usize,
    ) -> Result<Option<AllocatedBranch>> {
        let edge = &self.edges[indexed.canonical];
        let target = &self.layout.nodes[indexed.target];
        let target_near = if self.horizontal { target.x } else { target.y };
        // Match layout's two additional main-axis cells per extra authored length.
        // The base floor reserves a source reading cell, a channel turn, and a target
        // reading cell between the contours. Cross-axis detours never satisfy minlen.
        let extra_length = self
            .resources
            .checked_grid_mul(edge.length.saturating_sub(1), 2)?;
        let clearance = self
            .resources
            .checked_grid_add(READING_MARGIN, TURN_CELLS)?;
        let minimum_span = self
            .resources
            .checked_grid_add(self.resources.checked_grid_mul(clearance, 2)?, extra_length)?;
        if target_near
            .checked_sub(source_far)
            .is_none_or(|span| span < minimum_span)
        {
            return Ok(None);
        }
        let Some(end) = self.port(target, false)? else {
            return Ok(None);
        };
        let points = [
            start.berth,
            self.coord(
                channel,
                self.cross_center(&self.layout.nodes[indexed.source]),
            ),
            self.coord(channel, indexed.target_cross),
            end.berth,
        ];

        // Exclude the channel turn and the target's reading/marker berth. These hosts
        // cannot overwrite the shared channel, even when a target aligns with the source.
        let host_first = self.resources.checked_grid_add(channel, TURN_CELLS)?;
        let host_last =
            target_near.checked_sub(self.resources.checked_grid_add(READING_MARGIN, 1)?);
        let label_host = host_last.filter(|last| *last >= host_first).map(|last| {
            [
                self.coord(host_first, indexed.target_cross),
                self.coord(last, indexed.target_cross),
            ]
        });
        if let Some(raw) = edge.label.as_deref() {
            self.scan()?;
            if let Some(metrics) =
                try_measure_edge_label(raw, self.charset.width_profile, self.resources)?
            {
                let Some([first, last]) = label_host else {
                    return Ok(None);
                };
                let capacity = self
                    .resources
                    .checked_grid_add(self.main(last) - self.main(first), 1)?;
                let required = if self.horizontal {
                    metrics.max_width
                } else {
                    metrics.line_count
                };
                if required > capacity {
                    return Ok(None);
                }
            }
        }
        Ok(Some(AllocatedBranch {
            group,
            attachments: NodeAttachments {
                start: start.attachment,
                end: end.attachment,
            },
            points,
            label_host,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AsciiRenderOptions;
    use crate::color::AsciiRgb;
    use crate::error::AsciiError;
    use crate::graph::layout::{GridCoord, layout_graph};
    use crate::graph::model::{GraphEdgeStroke, GraphGroupStyle, GraphNodeShape};
    use crate::resource::{AsciiResourceLimitCause, AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_model::{CancelReason, OperationControl};

    const DIRECTIONS: [GraphDirection; 2] = [GraphDirection::LeftRight, GraphDirection::TopDown];

    fn fanout(
        direction: GraphDirection,
        count: usize,
        shape: GraphNodeShape,
    ) -> (AsciiGraph, GraphLayout, GraphCharset) {
        let mut graph = AsciiGraph::new(direction);
        graph.add_node("source", "Source");
        for index in 0..count {
            let id = format!("target-{index:02}");
            graph.add_node(&id, &id);
            graph.add_edge("source", &id);
        }
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut layout = layout_graph(&graph, &options);
        for node in &mut layout.nodes {
            let (main, cross) = if node.id == "source" {
                (10, 8 + (count - 1) * 6)
            } else {
                let index = node
                    .id
                    .strip_prefix("target-")
                    .unwrap()
                    .parse::<usize>()
                    .unwrap();
                (60 + index % 3 * 4, 8 + index * 12)
            };
            (node.x, node.y, node.width, node.height) = if direction == GraphDirection::LeftRight {
                (main, cross, 9, 5)
            } else {
                (cross, main, 5, 9)
            };
            node.shape = shape;
            // Deliberately unrelated to the terminal positions and even to each other.
            node.grid = GridCoord { x: 999, y: 999 };
        }
        layout.grid_aligned = false;
        (graph, layout, charset)
    }

    fn allocate(
        graph: &AsciiGraph,
        layout: &GraphLayout,
        charset: &GraphCharset,
    ) -> BranchAllocation {
        let policy = AsciiResourcePolicy::unbounded();
        BranchAllocation::try_new(
            graph,
            layout,
            &graph.edges,
            charset,
            &ResourceContext::new(policy),
            AsciiExecution::for_test(&policy),
        )
        .unwrap()
    }

    fn assert_unallocated(allocation: &BranchAllocation, edge_count: usize) {
        for index in 0..=edge_count {
            assert!(
                allocation.get(index).is_none(),
                "unexpected branch at {index}"
            );
        }
    }

    #[test]
    fn two_eight_twelve_and_sixteen_branches_share_one_channel_in_both_orientations() {
        for direction in DIRECTIONS {
            for count in [2, 8, 12, 16] {
                let (graph, layout, charset) = fanout(direction, count, GraphNodeShape::Rect);
                let allocation = allocate(&graph, &layout, &charset);
                assert_eq!(allocation, allocate(&graph, &layout, &charset));
                let horizontal = direction == GraphDirection::LeftRight;
                let main = |coord: CanvasCoord| if horizontal { coord.x } else { coord.y };
                let cross = |coord: CanvasCoord| if horizontal { coord.y } else { coord.x };
                let first = allocation
                    .get(0)
                    .expect("the entire fanout should allocate");
                for (index, edge) in graph.edges.iter().enumerate() {
                    let branch = allocation.get(index).expect("every target needs a branch");
                    let target = layout.nodes.iter().find(|node| node.id == edge.to).unwrap();
                    let target_near = if horizontal { target.x } else { target.y };
                    assert_eq!(branch.group, 0);
                    assert_eq!(branch.attachments.start, first.attachments.start);
                    assert_eq!(branch.points[0], first.points[0]);
                    assert_eq!(branch.points[1], first.points[1]);
                    assert_eq!(main(branch.points[1]), main(branch.points[2]));
                    assert_eq!(main(branch.points[1]), 20);
                    assert_eq!(cross(branch.points[2]), cross(branch.points[3]));
                    assert!(main(branch.points[0]) < main(branch.points[1]));
                    assert!(target_near - main(branch.points[2]) >= 2);
                    assert!(main(branch.points[2]) < main(branch.points[3]));
                    for pair in branch.points.windows(2) {
                        assert!(pair[0].x == pair[1].x || pair[0].y == pair[1].y);
                    }
                    let [start, end] = branch.label_host.unwrap();
                    assert!(main(start) > main(branch.points[2]));
                    assert!(main(end) < target_near - 1);
                    assert_eq!(cross(start), cross(branch.points[3]));
                    assert_eq!(cross(end), cross(branch.points[3]));
                }
                assert!(allocation.get(count).is_none());
            }
        }
    }

    #[test]
    fn contacts_and_one_cell_berths_follow_actual_shapes() {
        for direction in DIRECTIONS {
            for shape in [
                GraphNodeShape::Diamond,
                GraphNodeShape::Choice,
                GraphNodeShape::LeanLeft,
                GraphNodeShape::LeanRight,
                GraphNodeShape::Stadium,
                GraphNodeShape::Circle,
                GraphNodeShape::Text,
            ] {
                let (graph, layout, charset) = fanout(direction, 8, shape);
                let allocation = allocate(&graph, &layout, &charset);
                let source = layout
                    .nodes
                    .iter()
                    .find(|node| node.id == "source")
                    .unwrap();
                let (departure_inward, arrival_inward) = if direction == GraphDirection::LeftRight {
                    (StepDirection::Left, StepDirection::Right)
                } else {
                    (StepDirection::Up, StepDirection::Down)
                };
                for (index, edge) in graph.edges.iter().enumerate() {
                    let branch = allocation
                        .get(index)
                        .expect("central shape contacts should allocate");
                    let target = layout.nodes.iter().find(|node| node.id == edge.to).unwrap();
                    for (node, berth, inward, attachment) in [
                        (
                            source,
                            branch.points[0],
                            departure_inward,
                            branch.attachments.start,
                        ),
                        (
                            target,
                            branch.points[3],
                            arrival_inward,
                            branch.attachments.end,
                        ),
                    ] {
                        assert_eq!(
                            NodeAttachment::resolve(node, berth, inward, &charset),
                            Some(attachment)
                        );
                        assert!(attachment.allows_escape(berth));
                        assert_eq!(
                            attachment.contact.x.abs_diff(berth.x)
                                + attachment.contact.y.abs_diff(berth.y),
                            1
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn mixed_strokes_styles_and_visibility_reject_the_whole_source_group() {
        for direction in DIRECTIONS {
            for mismatch in 0..5 {
                let (mut graph, layout, charset) = fanout(direction, 8, GraphNodeShape::Rect);
                let edge = &mut graph.edges[7];
                match mismatch {
                    0 => edge.stroke = GraphEdgeStroke::Dotted,
                    1 => edge.stroke = GraphEdgeStroke::Invisible,
                    2 => edge.style.line = Some(AsciiRgb::new(20, 40, 60)),
                    3 => edge.style.arrow = Some(AsciiRgb::new(20, 40, 60)),
                    _ => edge.style.label = Some(AsciiRgb::new(20, 40, 60)),
                }
                assert_unallocated(&allocate(&graph, &layout, &charset), graph.edges.len());
            }
        }
    }

    #[test]
    fn marked_sources_reject_the_group_but_distinct_target_markers_are_preserved() {
        for direction in DIRECTIONS {
            let (mut graph, layout, charset) = fanout(direction, 8, GraphNodeShape::Rect);
            for (edge, marker) in graph.edges.iter_mut().zip([
                GraphEdgeMarker::Open,
                GraphEdgeMarker::Point,
                GraphEdgeMarker::Circle,
                GraphEdgeMarker::Cross,
                GraphEdgeMarker::Open,
                GraphEdgeMarker::Point,
                GraphEdgeMarker::Circle,
                GraphEdgeMarker::Cross,
            ]) {
                edge.end_marker = marker;
            }
            let original = graph.clone();
            let allocation = allocate(&graph, &layout, &charset);
            assert!((0..8).all(|index| allocation.get(index).is_some()));
            assert_eq!(graph, original);
            for marker in [
                GraphEdgeMarker::Point,
                GraphEdgeMarker::Circle,
                GraphEdgeMarker::Cross,
            ] {
                graph.edges[7].start_marker = marker;
                assert_unallocated(&allocate(&graph, &layout, &charset), graph.edges.len());
            }
        }
    }

    #[test]
    fn duplicate_targets_reciprocal_pairs_and_self_loops_reject_the_group() {
        for direction in DIRECTIONS {
            for complication in 0..3 {
                let (mut graph, layout, charset) = fanout(direction, 2, GraphNodeShape::Rect);
                match complication {
                    0 => graph.add_edge("source", "target-00"),
                    1 => graph.add_edge("target-00", "source"),
                    _ => graph.add_edge("source", "source"),
                }
                assert_unallocated(&allocate(&graph, &layout, &charset), graph.edges.len());
            }
        }
    }

    #[test]
    fn distinct_real_targets_must_have_private_intercepts() {
        for direction in DIRECTIONS {
            let (graph, mut layout, charset) = fanout(direction, 2, GraphNodeShape::Rect);
            let first = layout
                .nodes
                .iter()
                .find(|node| node.id == "target-00")
                .unwrap()
                .clone();
            let second = layout
                .nodes
                .iter_mut()
                .find(|node| node.id == "target-01")
                .unwrap();
            if direction == GraphDirection::LeftRight {
                second.y = first.y;
            } else {
                second.x = first.x;
            }
            assert_unallocated(&allocate(&graph, &layout, &charset), graph.edges.len());
        }
    }

    #[test]
    fn incoming_edges_do_not_block_unrelated_fanout_and_group_ids_use_canonical_indices() {
        let (mut graph, mut layout, charset) =
            fanout(GraphDirection::LeftRight, 2, GraphNodeShape::Rect);
        graph.add_node("upstream", "Upstream");
        graph.add_edge("upstream", "source");
        let mut upstream = layout.nodes[0].clone();
        upstream.id = "upstream".into();
        upstream.x = 0;
        layout.nodes.push(upstream);
        let canonical = [
            graph.edges[2].clone(),
            graph.edges[1].clone(),
            graph.edges[0].clone(),
        ];
        let policy = AsciiResourcePolicy::unbounded();
        let allocation = BranchAllocation::try_new(
            &graph,
            &layout,
            &canonical,
            &charset,
            &ResourceContext::new(policy),
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        assert!(allocation.get(0).is_none());
        for index in [1, 2] {
            let branch = allocation.get(index).unwrap();
            assert_eq!(branch.group, 1);
            let target = layout
                .nodes
                .iter()
                .find(|node| node.id == canonical[index].to)
                .unwrap();
            assert_eq!(branch.attachments.end.contact.y, target.center_y());
        }
    }

    #[test]
    fn labels_fit_exactly_on_private_legs_and_never_fall_back_to_the_shared_channel() {
        for direction in DIRECTIONS {
            let (mut graph, layout, charset) = fanout(direction, 8, GraphNodeShape::Rect);
            let initial = allocate(&graph, &layout, &charset);
            let [first, last] = initial.get(7).unwrap().label_host.unwrap();
            let capacity = first.x.abs_diff(last.x) + first.y.abs_diff(last.y) + 1;
            graph.edges[7].label = Some(if direction == GraphDirection::LeftRight {
                "x".repeat(capacity)
            } else {
                std::iter::repeat_n("x", capacity)
                    .collect::<Vec<_>>()
                    .join("\n")
            });
            let exact = allocate(&graph, &layout, &charset);
            assert!((0..8).all(|index| exact.get(index).is_some()));
            assert_eq!(exact.get(7).unwrap().label_host, Some([first, last]));
            graph.edges[7].label.as_mut().unwrap().push_str(
                if direction == GraphDirection::LeftRight {
                    "x"
                } else {
                    "\nx"
                },
            );
            assert_unallocated(&allocate(&graph, &layout, &charset), graph.edges.len());
        }
    }

    #[test]
    fn authored_length_requires_main_axis_span_even_with_a_large_cross_axis_detour() {
        for direction in DIRECTIONS {
            let (mut graph, mut layout, charset) = fanout(direction, 2, GraphNodeShape::Rect);
            for edge in &mut graph.edges {
                edge.length = 20;
            }
            let allocation = allocate(&graph, &layout, &charset);
            assert!((0..2).all(|index| allocation.get(index).is_some()));
            graph.edges[1].length = 23;
            let target = layout
                .nodes
                .iter_mut()
                .find(|node| node.id == "target-01")
                .unwrap();
            if direction == GraphDirection::LeftRight {
                target.y += 10_000;
            } else {
                target.x += 10_000;
            }
            assert_unallocated(&allocate(&graph, &layout, &charset), graph.edges.len());
        }
    }

    #[test]
    fn short_corridors_backward_edges_and_containers_are_outside_the_scope() {
        for direction in DIRECTIONS {
            for target_main in [9, 18, 21] {
                let (graph, mut layout, charset) = fanout(direction, 2, GraphNodeShape::Rect);
                let target = layout
                    .nodes
                    .iter_mut()
                    .find(|node| node.id == "target-01")
                    .unwrap();
                if direction == GraphDirection::LeftRight {
                    target.x = target_main;
                } else {
                    target.y = target_main;
                }
                assert_unallocated(&allocate(&graph, &layout, &charset), graph.edges.len());
            }
            let (mut graph, layout, charset) = fanout(direction, 2, GraphNodeShape::Rect);
            graph.add_group_with_style(
                "container",
                "Container",
                None,
                vec!["target-00".into()],
                GraphGroupStyle::default(),
            );
            assert_unallocated(&allocate(&graph, &layout, &charset), graph.edges.len());
        }
    }

    #[test]
    fn exact_work_is_admitted_and_one_unit_less_fails() {
        let (graph, layout, charset) =
            fanout(GraphDirection::TopDown, 16, GraphNodeShape::LeanLeft);
        let policy = AsciiResourcePolicy::unbounded();
        let measured = ResourceContext::new(policy);
        let expected = BranchAllocation::try_new(
            &graph,
            &layout,
            &graph.edges,
            &charset,
            &measured,
            AsciiExecution::for_test(&policy),
        )
        .unwrap();
        let work = measured.layout_work_used();
        for (limit, succeeds) in [(work, true), (work - 1, false)] {
            let policy = policy
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap();
            let resources = ResourceContext::new(policy);
            let result = BranchAllocation::try_new(
                &graph,
                &layout,
                &graph.edges,
                &charset,
                &resources,
                AsciiExecution::for_test(&policy),
            );
            if succeeds {
                assert_eq!(result.unwrap(), expected);
                assert_eq!(resources.layout_work_used(), work);
            } else {
                assert!(
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                    if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                        && details.actual == work && details.max == limit)
                );
            }
        }
    }

    #[test]
    fn storage_admission_precedes_reservation_and_cancellation_precedes_work() {
        let (graph, layout, charset) = fanout(GraphDirection::LeftRight, 16, GraphNodeShape::Rect);
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap();
        let resources = ResourceContext::new(policy);
        let result = BranchAllocation::try_new(
            &graph,
            &layout,
            &graph.edges,
            &charset,
            &resources,
            AsciiExecution::for_test(&policy),
        );
        assert!(
            matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
            if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                && details.actual == layout.nodes.len())
        );
        assert_eq!(resources.layout_work_used(), 0);

        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel();
        let result = BranchAllocation::try_new(
            &graph,
            &layout,
            &graph.edges,
            &charset,
            &resources,
            AsciiExecution::new(&control, &policy),
        );
        assert!(matches!(result, Err(AsciiError::Cancelled(cancelled))
            if cancelled.phase == OperationPhase::Layout && cancelled.reason == CancelReason::Requested));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn cancellation_interrupts_scene_scanning_with_uncontrolled_input_resources() {
        let (graph, layout, charset) = fanout(GraphDirection::TopDown, 16, GraphNodeShape::Rect);
        let policy = AsciiResourcePolicy::unbounded();
        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(20);
        let result = BranchAllocation::try_new(
            &graph,
            &layout,
            &graph.edges,
            &charset,
            &resources,
            AsciiExecution::new(&control, &policy),
        );
        assert!(matches!(result, Err(AsciiError::Cancelled(cancelled))
            if cancelled.phase == OperationPhase::Layout && cancelled.reason == CancelReason::Requested));
        assert!(resources.layout_work_used() > 0);
    }

    #[test]
    fn invalid_endpoint_geometry_and_overflow_never_reach_shape_resolution() {
        let (mut graph, mut layout, charset) =
            fanout(GraphDirection::LeftRight, 2, GraphNodeShape::Rect);
        let policy = AsciiResourcePolicy::unbounded();
        graph.edges[0].length = usize::MAX;
        let result = BranchAllocation::try_new(
            &graph,
            &layout,
            &graph.edges,
            &charset,
            &ResourceContext::new(policy),
            AsciiExecution::for_test(&policy),
        );
        assert!(
            matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
            if details.cause == AsciiResourceLimitCause::ArithmeticOverflow)
        );
        graph.edges[0].length = 1;
        layout.nodes[0].width = 0;
        assert_unallocated(&allocate(&graph, &layout, &charset), graph.edges.len());
        layout.nodes[0].width = 9;
        layout.nodes[0].x = usize::MAX;
        let result = BranchAllocation::try_new(
            &graph,
            &layout,
            &graph.edges,
            &charset,
            &ResourceContext::new(policy),
            AsciiExecution::for_test(&policy),
        );
        assert!(
            matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
            if details.limit == AsciiResourceLimitId::MaxGridCells
                && details.cause == AsciiResourceLimitCause::ArithmeticOverflow)
        );
    }
}
