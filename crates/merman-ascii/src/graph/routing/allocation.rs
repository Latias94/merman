//! Whole-scene candidates for compatible branching and merging channels in flat scenes.
//! Admission against node, route, label, and marker occupancy remains the caller's job.

use super::layout_allocation_failed;
use super::path::StepDirection;
use super::plan::{MarkerEndpoint, NodeAttachment, NodeAttachments};
use crate::error::Result;
use crate::graph::charset::GraphCharset;
use crate::graph::label::try_measure_edge_label;
use crate::graph::layout::{CanvasCoord, GraphLayout, NodeLayout, charge_sort_work};
use crate::graph::model::{AsciiGraph, AsciiGraphEdge, GraphDirection};
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
    /// The first canonical edge index of this complete branch or merge group.
    pub(super) group: usize,
    pub(super) shared_endpoint: MarkerEndpoint,
    pub(super) attachments: NodeAttachments,
    /// Source berth, source/channel turn, channel/target turn, target berth.
    pub(super) points: [CanvasCoord; 4],
    /// Inclusive host endpoints strictly inside the private leg. The caller must use
    /// these instead of choosing a label host on the shared stem or channel.
    /// A missing host is allowed only for an edge without a nonempty normalized label.
    pub(super) label_host: Option<[CanvasCoord; 2]>,
}

#[derive(Clone, Copy)]
struct IndexedEdge {
    canonical: usize,
    source: usize,
    target: usize,
    source_cross: usize,
    target_cross: usize,
}

impl IndexedEdge {
    fn unordered_pair(self) -> (usize, usize) {
        (self.source.min(self.target), self.source.max(self.target))
    }

    fn shared_node(self, endpoint: MarkerEndpoint) -> usize {
        match endpoint {
            MarkerEndpoint::Start => self.source,
            MarkerEndpoint::End => self.target,
        }
    }

    fn private_cross(self, endpoint: MarkerEndpoint) -> usize {
        match endpoint {
            MarkerEndpoint::Start => self.target_cross,
            MarkerEndpoint::End => self.source_cross,
        }
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

        resources.charge_layout_work_product(layout.nodes.len(), 2)?;
        let mut blocked_endpoints = Vec::new();
        blocked_endpoints
            .try_reserve_exact(layout.nodes.len())
            .map_err(|_| layout_allocation_failed())?;
        blocked_endpoints.resize(layout.nodes.len(), [false; 2]);

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
            // Invisible edges constrain layout but consume no route or endpoint cells.
            // Match SceneDemand's visible topology, retaining canonical edge indices.
            if !edge.stroke.is_visible() {
                continue;
            }
            if source == target {
                blocked_endpoints[source] = [true; 2];
            }
            indexed.push(IndexedEdge {
                canonical,
                source,
                target,
                source_cross: scene.cross_center(&layout.nodes[source]),
                target_cross: scene.cross_center(&layout.nodes[target]),
            });
        }

        // A repeated visible unordered pair means duplicate targets, parallel
        // multiplicity, or reciprocal edges. Block every participating endpoint without
        // an adjacency matrix or a per-edge scan of the rest of the scene.
        scene.execution.checkpoint(OperationPhase::Layout)?;
        charge_sort_work(indexed.len(), resources)?;
        indexed.sort_unstable_by_key(|edge| (edge.unordered_pair(), edge.source, edge.canonical));
        scene.execution.checkpoint(OperationPhase::Layout)?;
        for pair in indexed.windows(2) {
            scene.scan()?;
            if pair[0].unordered_pair() == pair[1].unordered_pair() {
                for edge in pair {
                    blocked_endpoints[edge.source][0] = true;
                    blocked_endpoints[edge.target][1] = true;
                }
            }
        }

        resources.charge_layout_work(canonical_edges.len())?;
        let mut branches = Vec::new();
        branches
            .try_reserve_exact(canonical_edges.len())
            .map_err(|_| layout_allocation_failed())?;
        branches.resize(canonical_edges.len(), None);

        // An edge belongs to at most one channel group. Source groups have deterministic
        // precedence; merging two branch trees could otherwise imply all-to-all relations.
        for (endpoint_index, endpoint) in [MarkerEndpoint::Start, MarkerEndpoint::End]
            .into_iter()
            .enumerate()
        {
            charge_sort_work(indexed.len(), resources)?;
            indexed.sort_unstable_by_key(|edge| {
                (
                    edge.shared_node(endpoint),
                    edge.private_cross(endpoint),
                    edge.canonical,
                )
            });
            scene.execution.checkpoint(OperationPhase::Layout)?;
            let mut first = 0;
            while first < indexed.len() {
                scene.scan()?;
                let shared = indexed[first].shared_node(endpoint);
                let mut end = resources.checked_work_add(first, 1)?;
                while end < indexed.len() && indexed[end].shared_node(endpoint) == shared {
                    scene.scan()?;
                    end = resources.checked_work_add(end, 1)?;
                }
                if !blocked_endpoints[shared][endpoint_index] && end - first >= 2 {
                    scene.allocate_group(&indexed[first..end], endpoint, &mut branches)?;
                }
                first = end;
            }
        }
        charge_sort_work(indexed.len(), resources)?;
        indexed.sort_unstable_by_key(|edge| (edge.source, edge.target, edge.canonical));
        let mut first = 0;
        while first < indexed.len() {
            scene.scan()?;
            let mut end = first + 1;
            while end < indexed.len()
                && (indexed[end].source, indexed[end].target)
                    == (indexed[first].source, indexed[first].target)
            {
                scene.scan()?;
                end += 1;
            }
            if end - first > 1 && indexed[first].source != indexed[first].target {
                scene.allocate_parallel(&indexed[first..end], &mut branches)?;
            }
            first = end;
        }
        charge_sort_work(indexed.len(), resources)?;
        indexed.sort_unstable_by_key(|edge| (edge.unordered_pair(), edge.canonical));
        let mut first = 0;
        while first < indexed.len() {
            let mut end = first + 1;
            while end < indexed.len()
                && indexed[end].unordered_pair() == indexed[first].unordered_pair()
            {
                scene.scan()?;
                end += 1;
            }
            if end - first > 2 {
                scene.allocate_pair_returns(&indexed[first..end], &mut branches)?;
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
    fn allocate_pair_returns(
        &self,
        group: &[IndexedEdge],
        branches: &mut [Option<AllocatedBranch>],
    ) -> Result<()> {
        self.resources.charge_layout_work_product(group.len(), 2)?;
        if group
            .iter()
            .any(|edge| branches[edge.canonical].is_some() || edge.source == edge.target)
            || !group
                .iter()
                .any(|edge| edge.source == group[0].target && edge.target == group[0].source)
        {
            return Ok(());
        }
        let first = &self.layout.nodes[group[0].source];
        let second = &self.layout.nodes[group[0].target];
        let cross_near = |node: &NodeLayout| if self.horizontal { node.y } else { node.x };
        let cross_far = |node: &NodeLayout| {
            if self.horizontal {
                node.bottom()
            } else {
                node.right()
            }
        };
        let main_near = |node: &NodeLayout| if self.horizontal { node.x } else { node.y };
        let main_far = |node: &NodeLayout| {
            if self.horizontal {
                node.right()
            } else {
                node.bottom()
            }
        };
        if cross_near(first) != cross_near(second)
            || cross_far(first) != cross_far(second)
            || main_far(first) >= main_near(second)
        {
            return Ok(());
        }
        let contacts = (main_far(first) - main_near(first))
            .min(main_far(second) - main_near(second))
            .saturating_sub(1);
        if group.len() - 1 > contacts {
            return Ok(());
        }
        let mut proposals = Vec::new();
        proposals
            .try_reserve_exact(group.len())
            .map_err(|_| layout_allocation_failed())?;
        let source = self.port(first, true)?;
        let target = self.port(second, false)?;
        let (Some(source), Some(target)) = (source, target) else {
            return Ok(());
        };
        let first_edge = &self.edges[group[0].canonical];
        let span = self
            .main(target.berth)
            .saturating_sub(self.main(source.berth));
        let extra_length = self
            .resources
            .checked_grid_mul(first_edge.length.saturating_sub(1), 2)?;
        if span < self.resources.checked_grid_add(2, extra_length)? {
            return Ok(());
        }
        proposals.push(AllocatedBranch {
            group: group[0].canonical,
            shared_endpoint: MarkerEndpoint::Start,
            attachments: NodeAttachments {
                start: source.attachment,
                end: target.attachment,
            },
            points: [source.berth, source.berth, target.berth, target.berth],
            label_host: Some([source.berth, target.berth]),
        });
        let mut channel = cross_far(first).max(cross_far(second));
        for (ordinal, indexed) in group.iter().enumerate().skip(1) {
            self.scan()?;
            let edge = &self.edges[indexed.canonical];
            let metrics = edge
                .label
                .as_deref()
                .map(|raw| {
                    try_measure_edge_label(
                        raw,
                        self.charset.width_profile,
                        self.resources,
                        edge.label_break_policy,
                    )
                })
                .transpose()?
                .flatten();
            let label_cross = metrics.map_or(0, |metrics| {
                if self.horizontal {
                    metrics.line_count
                } else {
                    metrics.max_width
                }
            });
            channel = self
                .resources
                .checked_grid_add(channel, self.resources.checked_grid_add(label_cross, 3)?)?;
            let from = &self.layout.nodes[indexed.source];
            let to = &self.layout.nodes[indexed.target];
            let outward = if self.horizontal {
                StepDirection::Down
            } else {
                StepDirection::Right
            };
            let start = self.coord(
                self.resources.checked_grid_add(main_near(from), ordinal)?,
                self.resources.checked_grid_add(cross_far(from), 1)?,
            );
            let end = self.coord(
                self.resources.checked_grid_add(main_near(to), ordinal)?,
                self.resources.checked_grid_add(cross_far(to), 1)?,
            );
            let (Some(start_attachment), Some(end_attachment)) = (
                NodeAttachment::resolve(from, start, outward.opposite(), self.charset),
                NodeAttachment::resolve(to, end, outward.opposite(), self.charset),
            ) else {
                return Ok(());
            };
            let start_turn = self.coord(self.main(start), channel);
            let end_turn = self.coord(self.main(end), channel);
            let along = self.main(start_turn).abs_diff(self.main(end_turn));
            let label_along = metrics.map_or(0, |metrics| {
                if self.horizontal {
                    metrics.max_width
                } else {
                    metrics.line_count
                }
            });
            if label_along > along.saturating_sub(2) {
                return Ok(());
            }
            proposals.push(AllocatedBranch {
                group: indexed.canonical,
                shared_endpoint: MarkerEndpoint::Start,
                attachments: NodeAttachments {
                    start: start_attachment,
                    end: end_attachment,
                },
                points: [start, start_turn, end_turn, end],
                label_host: metrics.map(|_| [start_turn, end_turn]),
            });
            channel = self.resources.checked_grid_add(channel, label_cross)?;
        }
        for (edge, proposal) in group.iter().zip(proposals) {
            self.scan()?;
            branches[edge.canonical] = Some(proposal);
        }
        Ok(())
    }

    fn allocate_parallel(
        &self,
        group: &[IndexedEdge],
        branches: &mut [Option<AllocatedBranch>],
    ) -> Result<()> {
        let source = &self.layout.nodes[group[0].source];
        let target = &self.layout.nodes[group[0].target];
        let (first, last, source_main, target_main, outward) = if self.horizontal {
            (
                source.y.max(target.y),
                source.bottom().min(target.bottom()),
                source.right(),
                target.x,
                StepDirection::Right,
            )
        } else {
            (
                source.x.max(target.x),
                source.right().min(target.right()),
                source.bottom(),
                target.y,
                StepDirection::Down,
            )
        };
        let pitch = READING_MARGIN * 2 + 1;
        let span = self.resources.checked_grid_add(
            self.resources
                .checked_grid_mul(group.len().saturating_sub(1), pitch)?,
            4,
        )?;
        if last.saturating_sub(first) < span || source_main.saturating_add(2) >= target_main {
            return Ok(());
        }
        let mut allocated = Vec::new();
        self.resources.charge_layout_work(group.len())?;
        allocated
            .try_reserve_exact(group.len())
            .map_err(|_| layout_allocation_failed())?;
        for (ordinal, indexed) in group.iter().enumerate() {
            self.scan()?;
            let edge = &self.edges[indexed.canonical];
            if branches[indexed.canonical].is_some()
                || edge
                    .label
                    .as_deref()
                    .is_some_and(|label| !label.trim().is_empty())
            {
                return Ok(());
            }
            let cross = self.resources.checked_grid_add(
                self.resources.checked_grid_add(first, 2)?,
                self.resources.checked_grid_mul(ordinal, pitch)?,
            )?;
            let start = self.coord(self.resources.checked_grid_add(source_main, 1)?, cross);
            let end = self.coord(target_main - 1, cross);
            let (Some(start_attachment), Some(end_attachment)) = (
                NodeAttachment::resolve(source, start, outward.opposite(), self.charset),
                NodeAttachment::resolve(target, end, outward, self.charset),
            ) else {
                return Ok(());
            };
            allocated.push(AllocatedBranch {
                group: indexed.canonical,
                shared_endpoint: MarkerEndpoint::Start,
                attachments: NodeAttachments {
                    start: start_attachment,
                    end: end_attachment,
                },
                points: [start, start, end, end],
                label_host: None,
            });
        }
        for (indexed, channel) in group.iter().zip(allocated) {
            self.scan()?;
            branches[indexed.canonical] = Some(channel);
        }
        Ok(())
    }

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
        shared_endpoint: MarkerEndpoint,
        branches: &mut [Option<AllocatedBranch>],
    ) -> Result<()> {
        let mut first_canonical = usize::MAX;
        let mut previous_cross = None;
        for indexed in group {
            self.scan()?;
            let edge = &self.edges[indexed.canonical];
            let baseline = &self.edges[group[0].canonical];
            if branches[indexed.canonical].is_some()
                || match shared_endpoint {
                    MarkerEndpoint::Start => edge.start_marker != baseline.start_marker,
                    MarkerEndpoint::End => edge.end_marker != baseline.end_marker,
                }
                || edge.stroke != baseline.stroke
                || edge.style != baseline.style
                // Private legs cannot share an intercept and erase endpoint identity.
                || previous_cross == Some(indexed.private_cross(shared_endpoint))
            {
                return Ok(());
            }
            previous_cross = Some(indexed.private_cross(shared_endpoint));
            first_canonical = first_canonical.min(indexed.canonical);
        }

        let shared = &self.layout.nodes[group[0].shared_node(shared_endpoint)];
        let is_source = shared_endpoint == MarkerEndpoint::Start;
        let Some(shared_port) = self.port(shared, is_source)? else {
            return Ok(());
        };
        let shared_boundary = if is_source {
            if self.horizontal {
                shared.right()
            } else {
                shared.bottom()
            }
        } else if self.horizontal {
            shared.x
        } else {
            shared.y
        };
        let clearance = self
            .resources
            .checked_grid_add(READING_MARGIN, TURN_CELLS)?;
        // Use the closest channel to maximize the private label spans.
        // Bounding envelopes conservatively contain every actual shape contour.
        let channel = if is_source {
            self.resources
                .checked_grid_add(shared_boundary, clearance)?
        } else {
            let Some(channel) = shared_boundary.checked_sub(clearance) else {
                return Ok(());
            };
            channel
        };
        for (position, indexed) in group.iter().enumerate() {
            self.scan()?;
            let branch = self.branch(
                *indexed,
                first_canonical,
                shared_endpoint,
                shared_port,
                channel,
            )?;
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
        shared_endpoint: MarkerEndpoint,
        shared_port: Port,
        channel: usize,
    ) -> Result<Option<AllocatedBranch>> {
        let edge = &self.edges[indexed.canonical];
        let source = &self.layout.nodes[indexed.source];
        let source_far = if self.horizontal {
            source.right()
        } else {
            source.bottom()
        };
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
        let private_port = match shared_endpoint {
            MarkerEndpoint::Start => self.port(target, false)?,
            MarkerEndpoint::End => self.port(source, true)?,
        };
        let Some(private_port) = private_port else {
            return Ok(None);
        };
        let (start, end) = match shared_endpoint {
            MarkerEndpoint::Start => (shared_port, private_port),
            MarkerEndpoint::End => (private_port, shared_port),
        };
        let points = [
            start.berth,
            self.coord(channel, indexed.source_cross),
            self.coord(channel, indexed.target_cross),
            end.berth,
        ];

        // Exclude the channel turn and the private terminal's reading/marker berth.
        let (host_first, host_last) = match shared_endpoint {
            MarkerEndpoint::Start => (
                self.resources.checked_grid_add(channel, TURN_CELLS)?,
                target_near.checked_sub(clearance),
            ),
            MarkerEndpoint::End => (
                self.resources.checked_grid_add(source_far, clearance)?,
                channel.checked_sub(TURN_CELLS),
            ),
        };
        let private_cross = indexed.private_cross(shared_endpoint);
        let label_host = host_last.filter(|last| *last >= host_first).map(|last| {
            [
                self.coord(host_first, private_cross),
                self.coord(last, private_cross),
            ]
        });
        if let Some(raw) = edge.label.as_deref() {
            self.scan()?;
            if let Some(metrics) = try_measure_edge_label(
                raw,
                self.charset.width_profile,
                self.resources,
                edge.label_break_policy,
            )? {
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
            shared_endpoint,
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
    use crate::graph::model::{GraphEdgeMarker, GraphEdgeStroke, GraphGroupStyle, GraphNodeShape};
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
        (graph, layout, charset)
    }

    fn fanin(
        direction: GraphDirection,
        count: usize,
        shape: GraphNodeShape,
    ) -> (AsciiGraph, GraphLayout, GraphCharset) {
        let (mut graph, mut layout, charset) = fanout(direction, count, shape);
        for edge in &mut graph.edges {
            std::mem::swap(&mut edge.from, &mut edge.to);
        }
        for node in &mut layout.nodes {
            if direction == GraphDirection::LeftRight {
                node.x = 128 - node.x - node.width;
            } else {
                node.y = 128 - node.y - node.height;
            }
        }
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
    fn parallel_marked_relationships_receive_distinct_measured_channels() {
        let mut graph = AsciiGraph::new(GraphDirection::TopDown);
        graph.add_node_with_shape_and_style(
            "source",
            "",
            GraphNodeShape::ForkJoinHorizontal,
            Default::default(),
        );
        graph.add_node("target", "Target");
        for _ in 0..97 {
            graph.add_edge("source", "target");
            let edge = graph.edges.last_mut().unwrap();
            edge.start_marker = GraphEdgeMarker::Circle;
            edge.end_marker = GraphEdgeMarker::Open;
        }
        let options = AsciiRenderOptions::ascii();
        let charset = GraphCharset::for_options(&options);
        let mut layout = layout_graph(&graph, &options);
        for (node, top) in layout.nodes.iter_mut().zip([8, 24]) {
            (node.x, node.y, node.width, node.height) = (8, top, 293, 3);
        }
        let allocation = allocate(&graph, &layout, &charset);
        let mut contacts = std::collections::BTreeSet::new();
        let mut groups = std::collections::BTreeSet::new();
        for index in 0..97 {
            let channel = allocation
                .get(index)
                .expect("independent parallel channels fit the measured contours");
            assert!(contacts.insert(channel.points[0].x));
            assert!(groups.insert(channel.group));
            assert_eq!(channel.attachments.start.outward, StepDirection::Down);
            assert_eq!(channel.attachments.end.outward, StepDirection::Up);
            assert_eq!(channel.points[0].x, channel.points[3].x);
            assert!(channel.label_host.is_none());
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
    fn merging_preserves_private_source_markers_and_labels_for_every_shared_marker_kind() {
        for direction in DIRECTIONS {
            for marker in [
                GraphEdgeMarker::Open,
                GraphEdgeMarker::Point,
                GraphEdgeMarker::Circle,
                GraphEdgeMarker::Cross,
            ] {
                let (mut graph, layout, charset) = fanin(direction, 12, GraphNodeShape::Diamond);
                for (index, edge) in graph.edges.iter_mut().enumerate() {
                    edge.end_marker = marker;
                    edge.start_marker = if index % 2 == 0 {
                        GraphEdgeMarker::Circle
                    } else {
                        GraphEdgeMarker::Cross
                    };
                    edge.label = Some(format!("if-{index}"));
                }
                let original = graph.clone();
                let allocation = allocate(&graph, &layout, &charset);
                assert_eq!(graph, original);
                assert_eq!(allocation, allocate(&graph, &layout, &charset));
                let common = allocation.get(0).unwrap();
                for (index, edge) in graph.edges.iter().enumerate() {
                    let branch = allocation.get(index).unwrap();
                    assert_eq!(branch.shared_endpoint, MarkerEndpoint::End);
                    assert_eq!(branch.group, 0);
                    assert_eq!(branch.points[2..], common.points[2..]);
                    assert_eq!(branch.attachments.end, common.attachments.end);
                    let source = layout
                        .nodes
                        .iter()
                        .find(|node| node.id == edge.from)
                        .unwrap();
                    let (source_far, channel, host_start, host_end) =
                        if direction == GraphDirection::LeftRight {
                            let [first, last] = branch.label_host.unwrap();
                            assert_eq!(first.y, source.center_y());
                            assert_eq!(last.y, source.center_y());
                            (source.right(), branch.points[1].x, first.x, last.x)
                        } else {
                            let [first, last] = branch.label_host.unwrap();
                            assert_eq!(first.x, source.center_x());
                            assert_eq!(last.x, source.center_x());
                            (source.bottom(), branch.points[1].y, first.y, last.y)
                        };
                    assert!(host_start > source_far + 1);
                    assert!(host_end < channel);
                }
            }
        }
    }

    #[test]
    fn merging_rejects_incompatible_markers_styles_and_parallel_or_reciprocal_edges() {
        for direction in DIRECTIONS {
            for mismatch in 0..6 {
                let (mut graph, layout, charset) = fanin(direction, 8, GraphNodeShape::Rect);
                match mismatch {
                    0 => graph.edges[7].end_marker = GraphEdgeMarker::Circle,
                    1 => graph.edges[7].stroke = GraphEdgeStroke::Dotted,
                    2 => graph.edges[7].style.arrow = Some(AsciiRgb::new(20, 40, 60)),
                    3 => graph.add_edge("target-00", "source"),
                    4 => graph.add_edge("source", "target-00"),
                    _ => graph.add_edge("source", "source"),
                }
                assert_unallocated(&allocate(&graph, &layout, &charset), graph.edges.len());
            }
        }
    }

    #[test]
    fn a_route_cannot_connect_two_independent_shared_channel_groups() {
        let (mut graph, mut layout, charset) =
            fanout(GraphDirection::LeftRight, 2, GraphNodeShape::Rect);
        graph.add_node("other", "Other");
        graph.add_edge("other", "target-00");
        let mut other = layout.nodes[0].clone();
        other.id = "other".into();
        other.y += 30;
        layout.nodes.push(other);
        let allocation = allocate(&graph, &layout, &charset);
        for index in [0, 1] {
            let branch = allocation.get(index).unwrap();
            assert_eq!(branch.group, 0);
            assert_eq!(branch.shared_endpoint, MarkerEndpoint::Start);
        }
        assert!(allocation.get(2).is_none());
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
    fn mixed_visible_strokes_and_styles_reject_the_whole_source_group() {
        for direction in DIRECTIONS {
            for mismatch in 0..4 {
                let (mut graph, layout, charset) = fanout(direction, 8, GraphNodeShape::Rect);
                let edge = &mut graph.edges[7];
                match mismatch {
                    0 => edge.stroke = GraphEdgeStroke::Dotted,
                    1 => edge.style.line = Some(AsciiRgb::new(20, 40, 60)),
                    2 => edge.style.arrow = Some(AsciiRgb::new(20, 40, 60)),
                    _ => edge.style.label = Some(AsciiRgb::new(20, 40, 60)),
                }
                assert_unallocated(&allocate(&graph, &layout, &charset), graph.edges.len());
            }
        }
    }

    #[test]
    fn invisible_constraints_preserve_visible_channels_and_canonical_edge_indices() {
        for direction in DIRECTIONS {
            for merging in [false, true] {
                let (mut graph, layout, charset) = if merging {
                    fanin(direction, 9, GraphNodeShape::Rect)
                } else {
                    fanout(direction, 9, GraphNodeShape::Rect)
                };
                for edge in &mut graph.edges {
                    if merging {
                        edge.end_marker = GraphEdgeMarker::Circle;
                    } else {
                        edge.start_marker = GraphEdgeMarker::Circle;
                    }
                }
                let expected = allocate(&graph, &layout, &charset);
                let mut invisible = graph.edges[0].clone();
                invisible.stroke = GraphEdgeStroke::Invisible;
                invisible.start_marker = GraphEdgeMarker::Cross;
                invisible.end_marker = GraphEdgeMarker::Cross;
                invisible.style.line = Some(AsciiRgb::new(20, 40, 60));
                // A leading duplicate, a reciprocal constraint, and a self-loop must
                // neither consume a port nor change visible endpoint compatibility.
                graph.edges.insert(0, invisible.clone());
                std::mem::swap(&mut invisible.from, &mut invisible.to);
                graph.edges.push(invisible.clone());
                invisible.from = "source".into();
                invisible.to = "source".into();
                graph.edges.push(invisible);

                let allocation = allocate(&graph, &layout, &charset);
                for index in 0..9 {
                    let before = expected.get(index).unwrap();
                    let after = allocation
                        .get(index + 1)
                        .expect("invisible constraints must preserve the visible channel");
                    assert_eq!(after.group, 1);
                    assert_eq!(after.shared_endpoint, before.shared_endpoint);
                    assert_eq!(after.attachments, before.attachments);
                    assert_eq!(after.points, before.points);
                    assert_eq!(after.label_host, before.label_host);
                }
                for index in [0, 10, 11] {
                    assert!(allocation.get(index).is_none());
                }

                // Removing one visible branch leaves the remaining compatible group.
                graph.edges[9].stroke = GraphEdgeStroke::Invisible;
                let reduced = allocate(&graph, &layout, &charset);
                for index in 1..9 {
                    assert_eq!(reduced.get(index), allocation.get(index));
                }
                assert!(reduced.get(9).is_none());
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
            for merging in [false, true] {
                let (mut graph, layout, charset) = if merging {
                    fanin(direction, 8, GraphNodeShape::Rect)
                } else {
                    fanout(direction, 8, GraphNodeShape::Rect)
                };
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
        for merging in [false, true] {
            let (mut graph, layout, charset) = if merging {
                fanin(GraphDirection::TopDown, 16, GraphNodeShape::LeanLeft)
            } else {
                fanout(GraphDirection::TopDown, 16, GraphNodeShape::LeanLeft)
            };
            let mut invisible = graph.edges[0].clone();
            invisible.stroke = GraphEdgeStroke::Invisible;
            graph.edges.push(invisible);
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
            assert!(expected.get(0).is_some());
            assert!(expected.get(graph.edges.len() - 1).is_none());
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
