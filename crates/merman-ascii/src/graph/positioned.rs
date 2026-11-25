//! Fixed terminal geometry supplied by a family-owned layout.
//!
//! This builder measures labels and shapes using the shared graph rules, but never invokes a
//! graph ranker. Its private graph/layout pair enters the normal validated route and paint path.

use super::label::{
    DeferredGraphNodeLabelPlan, GraphLabel, GraphNodeLabelPlan, PreparedGraphNodeText,
    try_measure_edge_label,
};
use super::layout::{GraphLayout, GridCoord, GroupLayout, NodeLayout};
use super::model::{
    AsciiGraph, AsciiGraphNode, GraphDirection, GraphEdgeAttrs, GraphGroupKind, GraphGroupStyle,
    GraphNodeSemantics, GraphNodeStyle,
};
use super::shape::GraphNodeShapeSemantics;
use super::surface::OutputTransform;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, GraphLayoutPolicy, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::LabelBreakPolicy;

pub(crate) use super::model::{
    GraphEdgeMarker as PositionedMarker, GraphEdgeStroke as PositionedStroke,
    GraphNodeShape as PositionedShape,
};
#[cfg(feature = "diagram-architecture")]
pub(crate) use super::model::{
    GraphEndpointPort as PositionedPort, GraphPortSide as PositionedSide,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PositionedSize {
    pub(crate) width: usize,
    pub(crate) height: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PositionedBounds {
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PositionedElement(usize);

pub(crate) struct PositionedEdge<'a> {
    pub(crate) id: &'a str,
    pub(crate) from: &'a str,
    pub(crate) to: &'a str,
    pub(crate) label: &'a str,
    pub(crate) stroke: PositionedStroke,
    pub(crate) start_marker: PositionedMarker,
    pub(crate) end_marker: PositionedMarker,
}

/// Borrowed aggregate admission before a family creates any owned graph projection strings.
#[derive(Default)]
pub(crate) struct PositionedTextAdmission {
    copy_work: usize,
    document_cells: usize,
    materialized_bytes: usize,
}

impl PositionedTextAdmission {
    pub(crate) fn include_label(
        &mut self,
        plan: &DeferredGraphNodeLabelPlan<'_>,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.include_work(plan.source_materialization_work_units(), resources)?;
        self.include_metrics(
            plan.document_cells(),
            plan.materialized_bytes()
                .max(plan.source_materialization_work_units()),
            resources,
        )
    }

    pub(crate) fn include_edge_label(
        &mut self,
        text: &str,
        profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.include_work(text.len(), resources)?;
        let mut materialized_bytes = text.len();
        if let Some(metrics) = try_measure_edge_label(
            text,
            profile,
            resources,
            LabelBreakPolicy::MermaidLabelBreaks,
        )? {
            materialized_bytes = materialized_bytes.max(metrics.materialized_bytes);
            self.include_metrics(metrics.document_cells, 0, resources)?;
        }
        self.include_metrics(0, materialized_bytes, resources)
    }

    pub(crate) fn include_copy_work(
        &mut self,
        bytes: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.include_work(bytes, resources)?;
        self.include_metrics(0, bytes, resources)
    }

    pub(crate) fn include_work(&mut self, units: usize, resources: &ResourceContext) -> Result<()> {
        self.copy_work = resources.checked_work_add(self.copy_work, units)?;
        Ok(())
    }

    pub(crate) fn include_retained_bytes(
        &mut self,
        bytes: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.include_metrics(0, bytes, resources)
    }

    fn include_metrics(
        &mut self,
        cells: usize,
        bytes: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.document_cells = self
            .document_cells
            .checked_add(cells)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
        self.materialized_bytes = self
            .materialized_bytes
            .checked_add(bytes)
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        Ok(())
    }

    pub(crate) fn admit(self, resources: &ResourceContext) -> Result<()> {
        resources.check_usage(self.copy_work, self.document_cells)?;
        resources.check(
            AsciiResourceLimitId::MaxOutputBytes,
            self.materialized_bytes,
        )?;
        resources.charge_layout_work(self.copy_work)
    }
}

enum ElementKind {
    Node(usize),
    Frame(usize),
}

struct Element {
    kind: ElementKind,
    minimum: PositionedSize,
    placed: bool,
}

pub(crate) struct PositionedGraph {
    graph: AsciiGraph,
    layout: GraphLayout,
    elements: Vec<Element>,
    frame_label_plans: Vec<GraphNodeLabelPlan>,
    labels_materialized: bool,
    policy: GraphLayoutPolicy,
}

impl PositionedGraph {
    pub(crate) fn try_new(
        diagram_type: &'static str,
        policy: GraphLayoutPolicy,
        capacity: usize,
        edges: usize,
        resources: &ResourceContext,
    ) -> Result<Self> {
        resources.charge_layout_work(resources.checked_work_add(capacity, edges)?)?;
        let mut graph = AsciiGraph::new_for_diagram(diagram_type, GraphDirection::TopDown);
        graph.try_reserve_projection(capacity, edges, capacity)?;
        let mut layout = GraphLayout {
            nodes: Vec::new(),
            groups: Vec::new(),
            group_background_order: Vec::new(),
        };
        reserve(&mut layout.nodes, capacity)?;
        reserve(&mut layout.groups, capacity)?;
        reserve(&mut layout.group_background_order, capacity)?;
        let mut elements = Vec::new();
        reserve(&mut elements, capacity)?;
        let mut frame_label_plans = Vec::new();
        reserve(&mut frame_label_plans, capacity)?;
        Ok(Self {
            graph,
            layout,
            elements,
            frame_label_plans,
            labels_materialized: false,
            policy,
        })
    }

    pub(crate) fn try_add_node(
        &mut self,
        id: &str,
        text: PreparedGraphNodeText,
        shape: PositionedShape,
        resources: &ResourceContext,
    ) -> Result<(PositionedElement, PositionedSize)> {
        // Source text and IDs are covered by the caller's admitted copy batch. Row replay
        // is deliberately deferred until every source in that batch has been copied.
        let metrics = text.plan().metrics();
        let node = AsciiGraphNode::new_with_prepared_text(
            copy_text(id, resources)?,
            text,
            shape,
            GraphNodeStyle::default(),
            GraphNodeSemantics::default(),
        );
        let size = GraphNodeShapeSemantics::new(shape).try_size_for_label_metrics(
            metrics.width,
            metrics.content_height,
            &self.policy,
            resources,
        )?;
        resources.grid_extent(size.width, size.height)?;
        reserve(&mut self.graph.nodes, 1)?;
        reserve(&mut self.layout.nodes, 1)?;
        let index = self.layout.nodes.len();
        self.layout.nodes.push(NodeLayout {
            id: copy_text(id, resources)?,
            label: GraphLabel::unmaterialized_with_profile(self.policy.terminal_width_profile),
            shape,
            style: GraphNodeStyle::default(),
            grid: GridCoord { x: 0, y: 0 },
            x: 0,
            y: 0,
            width: size.width,
            height: size.height,
        });
        self.graph.nodes.push(node);
        self.add_element(
            ElementKind::Node(index),
            PositionedSize {
                width: size.width,
                height: size.height,
            },
        )
    }

    /// The returned minimum height includes a title band and two bottom padding cells.
    pub(crate) fn try_add_frame<'a>(
        &mut self,
        id: &str,
        text: PreparedGraphNodeText,
        members: impl IntoIterator<Item = &'a str>,
        resources: &ResourceContext,
    ) -> Result<(PositionedElement, PositionedSize)> {
        let (title, title_plan) = text.into_single_label_plan()?;
        let metrics = title_plan.metrics();
        let minimum = PositionedSize {
            width: resources.checked_grid_add(metrics.width, 4)?.max(5),
            height: if title.is_empty() {
                4
            } else {
                resources.checked_grid_add(metrics.content_height, 5)?
            },
        };
        resources.grid_extent(minimum.width, minimum.height)?;
        let mut member_ids = Vec::new();
        for member in members {
            resources.checkpoint()?;
            reserve(&mut member_ids, 1)?;
            member_ids.push(copy_text(member, resources)?);
        }
        reserve(&mut self.graph.groups, 1)?;
        reserve(&mut self.layout.groups, 1)?;
        reserve(&mut self.layout.group_background_order, 1)?;
        reserve(&mut self.frame_label_plans, 1)?;
        let index = self.layout.groups.len();
        self.graph.add_group_with_style(
            copy_text(id, resources)?,
            title,
            None,
            member_ids,
            GraphGroupStyle::default(),
        );
        self.layout.groups.push(GroupLayout {
            id: copy_text(id, resources)?,
            kind: GraphGroupKind::Container,
            title: GraphLabel::unmaterialized_with_profile(self.policy.terminal_width_profile),
            title_transform: OutputTransform::Identity,
            style: GraphGroupStyle::default(),
            divider_span: None,
            x: 0,
            y: 0,
            width: minimum.width,
            height: minimum.height,
        });
        // Callers add enclosing frames before their descendants.
        self.layout.group_background_order.push(index);
        self.frame_label_plans.push(title_plan);
        self.add_element(ElementKind::Frame(index), minimum)
    }

    fn add_element(
        &mut self,
        kind: ElementKind,
        minimum: PositionedSize,
    ) -> Result<(PositionedElement, PositionedSize)> {
        reserve(&mut self.elements, 1)?;
        let key = PositionedElement(self.elements.len());
        self.elements.push(Element {
            kind,
            minimum,
            placed: false,
        });
        Ok((key, minimum))
    }

    pub(crate) fn set_bounds(
        &mut self,
        element: PositionedElement,
        bounds: PositionedBounds,
        resources: &ResourceContext,
    ) -> Result<()> {
        resources.charge_layout_work(1)?;
        let Some(element) = self.elements.get_mut(element.0) else {
            return Err(invalid_geometry(self.graph.diagram_type()));
        };
        if element.placed
            || bounds.width < element.minimum.width
            || bounds.height < element.minimum.height
        {
            return Err(invalid_geometry(self.graph.diagram_type()));
        }
        let right = resources.checked_grid_add(bounds.x, bounds.width)?;
        let bottom = resources.checked_grid_add(bounds.y, bounds.height)?;
        resources.grid_extent(right, bottom)?;
        match element.kind {
            ElementKind::Node(index) => {
                let node = &mut self.layout.nodes[index];
                node.x = bounds.x;
                node.y = bounds.y;
                node.width = bounds.width;
                node.height = bounds.height;
                node.grid = GridCoord {
                    x: bounds.x,
                    y: bounds.y,
                };
            }
            ElementKind::Frame(index) => {
                let group = &mut self.layout.groups[index];
                group.x = bounds.x;
                group.y = bounds.y;
                group.width = bounds.width;
                group.height = bounds.height;
            }
        }
        element.placed = true;
        Ok(())
    }

    pub(crate) fn try_add_edge(
        &mut self,
        edge: PositionedEdge<'_>,
        resources: &ResourceContext,
    ) -> Result<()> {
        resources.checkpoint()?;
        reserve(&mut self.graph.edges, 1)?;
        self.graph.add_edge_with_attrs(
            copy_text(edge.from, resources)?,
            copy_text(edge.to, resources)?,
            GraphEdgeAttrs {
                id: Some(copy_text(edge.id, resources)?),
                is_user_defined_id: true,
                label: if edge.label.is_empty() {
                    None
                } else {
                    Some(copy_text(edge.label, resources)?)
                },
                stroke: edge.stroke,
                start_marker: edge.start_marker,
                end_marker: edge.end_marker,
                ..GraphEdgeAttrs::default()
            },
        );
        Ok(())
    }

    #[cfg(feature = "diagram-architecture")]
    pub(crate) fn try_add_port_edge(
        &mut self,
        edge: PositionedEdge<'_>,
        source: (PositionedSide, Option<&str>),
        target: (PositionedSide, Option<&str>),
        resources: &ResourceContext,
    ) -> Result<()> {
        self.try_add_edge(edge, resources)?;
        let source = PositionedPort {
            side: source.0,
            anchor: source.1.map(|id| copy_text(id, resources)).transpose()?,
        };
        let target = PositionedPort {
            side: target.0,
            anchor: target.1.map(|id| copy_text(id, resources)).transpose()?,
        };
        let edge = self
            .graph
            .edges
            .last_mut()
            .ok_or_else(|| invalid_geometry("architecture"))?;
        edge.source_port = Some(source);
        edge.target_port = Some(target);
        Ok(())
    }

    /// Replays prepared rows only after the complete source/identity copy batch has finished.
    pub(crate) fn materialize_labels(&mut self, resources: &ResourceContext) -> Result<()> {
        if self.labels_materialized {
            return Err(invalid_geometry(self.graph.diagram_type()));
        }
        for (node, layout) in self.graph.nodes.iter().zip(&mut self.layout.nodes) {
            resources.charge_layout_work(1)?;
            let plan = GraphNodeLabelPlan::try_for_node(
                node,
                None,
                self.graph.diagram_type(),
                self.policy.terminal_width_profile,
                resources,
            )?;
            layout.label = plan.materialize(node, resources)?;
        }
        for ((group, layout), plan) in self
            .graph
            .groups
            .iter()
            .zip(&mut self.layout.groups)
            .zip(&self.frame_label_plans)
        {
            resources.charge_layout_work(1)?;
            layout.title = if group.title.is_empty() {
                GraphLabel::empty_with_profile(self.policy.terminal_width_profile)
            } else {
                plan.materialize_single_label(&group.title, resources)?
            };
        }
        self.labels_materialized = true;
        Ok(())
    }

    pub(crate) fn render(
        mut self,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<String> {
        self.materialize_labels(resources)?;
        for element in &self.elements {
            resources.charge_layout_work(1)?;
            if !element.placed {
                return Err(invalid_geometry(self.graph.diagram_type()));
            }
        }
        super::draw::render_positioned_graph(
            &self.graph,
            self.layout,
            options,
            &self.policy,
            resources,
            execution,
        )
    }
}

fn copy_text(text: &str, resources: &ResourceContext) -> Result<String> {
    // All source copies, including IDs and membership references, are charged together by
    // PositionedTextAdmission. Replaying rows must not consume later copies' allowance.
    resources.checkpoint()?;
    let mut value = String::new();
    value
        .try_reserve(text.len())
        .map_err(|_| AsciiError::allocation_failed("ascii_layout_work"))?;
    let mut start = 0;
    while start < text.len() {
        resources.checkpoint()?;
        let mut end = start.saturating_add(8 * 1024).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        value.push_str(&text[start..end]);
        start = end;
    }
    Ok(value)
}

fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values
        .try_reserve(additional)
        .map_err(|_| AsciiError::allocation_failed("ascii_layout_work"))
}

fn invalid_geometry(diagram_type: &'static str) -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type,
        feature: "invalid positioned graph geometry",
    }
}

#[cfg(test)]
mod tests {
    use super::super::label::DeferredGraphLabelSectionPlan;
    use super::*;
    use crate::resource::AsciiResourcePolicy;

    fn policy(profile: TerminalWidthProfile) -> GraphLayoutPolicy {
        GraphLayoutPolicy {
            routing_space: Default::default(),
            node_padding_x: 1,
            node_padding_y: 0,
            rank_gap_x: 5,
            rank_gap_y: 3,
            group_padding_x: 2,
            group_padding_y: 2,
            group_title_clearance: 3,
            edge_label_lane_radius: GraphLayoutPolicy::DEFAULT_EDGE_LABEL_LANE_RADIUS,
            structural_charset: crate::AsciiCharset::Ascii,
            terminal_width_profile: profile,
        }
    }

    fn plan<'a>(
        label: &'a str,
        profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> DeferredGraphNodeLabelPlan<'a> {
        DeferredGraphNodeLabelPlan::single(
            DeferredGraphLabelSectionPlan::try_single(label, None, "block", profile, resources)
                .unwrap(),
            None,
            "block",
            profile,
            resources,
        )
        .unwrap()
    }

    #[test]
    fn deferred_rows_keep_all_existing_block_shape_sizes_and_labels() {
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            let policy = policy(profile);
            for shape in [
                PositionedShape::Rect,
                PositionedShape::Rounded,
                PositionedShape::Circle,
                PositionedShape::DoubleCircle,
                PositionedShape::Stadium,
                PositionedShape::Subroutine,
                PositionedShape::Cylinder,
                PositionedShape::Diamond,
                PositionedShape::Hexagon,
                PositionedShape::LeanRight,
                PositionedShape::LeanLeft,
                PositionedShape::Trapezoid,
                PositionedShape::TrapezoidAlt,
            ] {
                for source in [
                    "Alpha<br/>客户\\n👩‍💻",
                    "[block-arrow:right+down] Turn\n客户",
                    "[rect_left_inv_arrow] ",
                ] {
                    let reference_resources =
                        ResourceContext::new(AsciiResourcePolicy::unbounded());
                    let reference = GraphLabel::try_with_break_policy(
                        source,
                        None,
                        profile,
                        LabelBreakPolicy::MermaidLabelBreaks,
                        &reference_resources,
                    )
                    .unwrap();
                    let expected = GraphNodeShapeSemantics::new(shape)
                        .try_size_for_label_metrics(
                            reference.width(),
                            reference.content_height(),
                            &policy,
                            &reference_resources,
                        )
                        .unwrap();
                    let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
                    let mut graph =
                        PositionedGraph::try_new("block", policy, 1, 0, &resources).unwrap();
                    let plan = plan(source, profile, &resources);
                    let mut admission = PositionedTextAdmission::default();
                    admission.include_label(&plan, &resources).unwrap();
                    admission
                        .include_copy_work("id".len() * 2, &resources)
                        .unwrap();
                    admission.admit(&resources).unwrap();
                    let prepared = plan.materialize_after_admission(&resources).unwrap();
                    let (element, minimum) = graph
                        .try_add_node("id", prepared, shape, &resources)
                        .unwrap();
                    assert_eq!(
                        (minimum.width, minimum.height),
                        (expected.width, expected.height)
                    );
                    assert_eq!(graph.graph.nodes[0].label(), source);
                    assert!(
                        graph.layout.nodes[0].label.lines().is_empty(),
                        "source copying must not replay rows"
                    );
                    graph
                        .set_bounds(
                            element,
                            PositionedBounds {
                                x: 7,
                                y: 11,
                                width: minimum.width,
                                height: minimum.height,
                            },
                            &resources,
                        )
                        .unwrap();
                    graph.materialize_labels(&resources).unwrap();
                    let node = &graph.layout.nodes[0];
                    assert_eq!(node.label, reference);
                    assert_eq!(
                        (node.x, node.y, node.width, node.height),
                        (7, 11, expected.width, expected.height)
                    );
                }
            }
        }
    }

    #[test]
    fn deferred_frame_titles_keep_the_existing_title_band_and_empty_frame_geometry() {
        for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
            for source in ["", "Frame α<br/>客户👩‍💻", "  Frame\\nsecond  "] {
                let reference_resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
                let reference = if source.is_empty() {
                    GraphLabel::empty_with_profile(profile)
                } else {
                    GraphLabel::try_with_break_policy(
                        source,
                        None,
                        profile,
                        LabelBreakPolicy::MermaidLabelBreaks,
                        &reference_resources,
                    )
                    .unwrap()
                };
                let expected = PositionedSize {
                    width: (reference.width() + 4).max(5),
                    height: if source.is_empty() {
                        4
                    } else {
                        reference.content_height() + 5
                    },
                };
                let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
                let mut graph =
                    PositionedGraph::try_new("block", policy(profile), 1, 0, &resources).unwrap();
                let plan = plan(source, profile, &resources);
                let mut admission = PositionedTextAdmission::default();
                admission.include_label(&plan, &resources).unwrap();
                admission
                    .include_copy_work("frame".len() * 2, &resources)
                    .unwrap();
                admission.admit(&resources).unwrap();
                let prepared = plan.materialize_after_admission(&resources).unwrap();
                let (element, minimum) = graph
                    .try_add_frame("frame", prepared, std::iter::empty(), &resources)
                    .unwrap();
                assert_eq!(minimum, expected);
                assert_eq!(graph.graph.groups[0].title, source);
                assert!(graph.layout.groups[0].title.lines().is_empty());
                graph
                    .set_bounds(
                        element,
                        PositionedBounds {
                            x: 7,
                            y: 11,
                            width: minimum.width,
                            height: minimum.height,
                        },
                        &resources,
                    )
                    .unwrap();
                graph.materialize_labels(&resources).unwrap();
                let frame = &graph.layout.groups[0];
                assert_eq!(frame.title, reference);
                assert_eq!(
                    (frame.x, frame.y, frame.width, frame.height),
                    (7, 11, expected.width, expected.height)
                );
            }
        }
    }
}
