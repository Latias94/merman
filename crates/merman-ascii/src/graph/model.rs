use crate::color::AsciiRgb;
use crate::error::{AsciiError, Result};
use crate::resource::AsciiResourceLimitPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphDirection {
    LeftRight,
    RightLeft,
    TopDown,
    BottomTop,
}

impl GraphDirection {
    pub(crate) fn canonical(self) -> Self {
        match self {
            Self::RightLeft => Self::LeftRight,
            Self::BottomTop => Self::TopDown,
            direction => direction,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphRootPolicy {
    DeclaredFirst,
    IncomingEdges,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsciiGraph {
    pub(super) diagram_type: &'static str,
    pub(super) direction: GraphDirection,
    pub(super) root_policy: GraphRootPolicy,
    pub(super) nodes: Vec<AsciiGraphNode>,
    pub(super) edges: Vec<AsciiGraphEdge>,
    pub(super) groups: Vec<AsciiGraphGroup>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AsciiGraphNode {
    pub(super) id: String,
    pub(super) label: String,
    pub(super) shape: GraphNodeShape,
    pub(super) style: GraphNodeStyle,
    pub(super) semantics: GraphNodeSemantics,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct GraphNodeSemantics {
    pub(crate) compartments: Option<GraphNodeCompartments>,
    pub(crate) side_constraint: Option<GraphNodeSideConstraint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphNodeCompartments {
    pub(super) title: String,
    pub(super) body: String,
}

impl GraphNodeCompartments {
    pub(crate) fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphNodeSideConstraint {
    pub(super) anchor_id: String,
    pub(super) side: GraphNodeSide,
}

impl GraphNodeSideConstraint {
    pub(crate) fn new(anchor_id: impl Into<String>, side: GraphNodeSide) -> Self {
        Self {
            anchor_id: anchor_id.into(),
            side,
        }
    }

    pub(crate) fn anchor_id(&self) -> &str {
        &self.anchor_id
    }

    pub(crate) const fn side(&self) -> GraphNodeSide {
        self.side
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphNodeSide {
    Left,
    Right,
}

impl GraphNodeSide {
    pub(crate) const fn reversed(self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GraphNodeStyle {
    pub(super) text: Option<AsciiRgb>,
    pub(super) border: Option<AsciiRgb>,
    pub(super) background: Option<AsciiRgb>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphNodeShape {
    Rect,
    StateWithTitle,
    Rounded,
    Circle,
    Stadium,
    DoubleCircle,
    Diamond,
    Subroutine,
    Cylinder,
    LeanRight,
    LeanLeft,
    ManualInput,
    Datastore,
    BowTie,
    Document,
    StackedDocument,
    LinedDocument,
    TaggedDocument,
    StackedRect,
    LinedRect,
    TaggedRect,
    PaperTape,
    Text,
    Hexagon,
    Asymmetric,
    Trapezoid,
    TrapezoidAlt,
    StateStart,
    StateEnd,
    ForkJoinHorizontal,
    ForkJoinVertical,
    Choice,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AsciiGraphEdge {
    pub(super) id: Option<String>,
    pub(super) is_user_defined_id: bool,
    pub(super) from: String,
    pub(super) to: String,
    pub(super) label: Option<String>,
    pub(super) stroke: GraphEdgeStroke,
    pub(super) start_marker: GraphEdgeMarker,
    pub(super) end_marker: GraphEdgeMarker,
    pub(super) length: usize,
    pub(super) style: GraphEdgeStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphEdgeAttrs {
    pub(crate) id: Option<String>,
    pub(crate) is_user_defined_id: bool,
    pub(crate) label: Option<String>,
    pub(crate) stroke: GraphEdgeStroke,
    pub(crate) start_marker: GraphEdgeMarker,
    pub(crate) end_marker: GraphEdgeMarker,
    pub(crate) length: usize,
    pub(crate) style: GraphEdgeStyle,
}

impl Default for GraphEdgeAttrs {
    fn default() -> Self {
        Self {
            id: None,
            is_user_defined_id: false,
            label: None,
            stroke: GraphEdgeStroke::Normal,
            start_marker: GraphEdgeMarker::Open,
            end_marker: GraphEdgeMarker::Point,
            length: 1,
            style: GraphEdgeStyle::default(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GraphEdgeStyle {
    pub(super) line: Option<AsciiRgb>,
    pub(super) arrow: Option<AsciiRgb>,
    pub(super) label: Option<AsciiRgb>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AsciiGraphGroup {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) kind: GraphGroupKind,
    pub(super) direction: Option<GraphDirection>,
    pub(super) nodes: Vec<String>,
    pub(super) style: GraphGroupStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphGroupKind {
    Container,
    Divider,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GraphGroupStyle {
    pub(super) title: Option<AsciiRgb>,
    pub(super) border: Option<AsciiRgb>,
    pub(super) background: Option<AsciiRgb>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphEdgeStroke {
    Normal,
    Dotted,
    Thick,
    Invisible,
}

impl GraphEdgeStroke {
    pub(crate) const fn is_visible(self) -> bool {
        !matches!(self, Self::Invisible)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphEdgeMarker {
    Open,
    Point,
    Circle,
    Cross,
}

#[cfg(test)]
pub(crate) type GraphEdgeArrow = GraphEdgeMarker;

impl AsciiGraph {
    pub(crate) fn new(direction: GraphDirection) -> Self {
        Self::new_for_diagram("flowchart", direction)
    }

    pub(crate) fn new_for_diagram(diagram_type: &'static str, direction: GraphDirection) -> Self {
        Self {
            diagram_type,
            direction,
            root_policy: GraphRootPolicy::DeclaredFirst,
            nodes: Vec::new(),
            edges: Vec::new(),
            groups: Vec::new(),
        }
    }

    pub(crate) fn try_reserve_projection(
        &mut self,
        nodes: usize,
        edges: usize,
        groups: usize,
    ) -> Result<()> {
        self.nodes
            .try_reserve(nodes)
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        self.edges
            .try_reserve(edges)
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        self.groups
            .try_reserve(groups)
            .map_err(|_| AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
            })?;
        Ok(())
    }

    pub(crate) fn diagram_type(&self) -> &'static str {
        self.diagram_type
    }

    pub(crate) fn use_incoming_edge_roots(&mut self) {
        self.root_policy = GraphRootPolicy::IncomingEdges;
    }

    #[cfg(test)]
    pub(crate) fn add_node(&mut self, id: impl Into<String>, label: impl Into<String>) {
        self.add_node_with_shape_and_style(
            id,
            label,
            GraphNodeShape::Rect,
            GraphNodeStyle::default(),
        );
    }

    pub(crate) fn add_node_with_shape_and_style(
        &mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        shape: GraphNodeShape,
        style: GraphNodeStyle,
    ) {
        self.add_node_with_semantics(id, label, shape, style, GraphNodeSemantics::default());
    }

    pub(crate) fn add_node_with_semantics(
        &mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        shape: GraphNodeShape,
        style: GraphNodeStyle,
        semantics: GraphNodeSemantics,
    ) {
        self.nodes.push(AsciiGraphNode {
            id: id.into(),
            label: label.into(),
            shape,
            style,
            semantics,
        });
    }

    #[cfg(test)]
    pub(crate) fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) {
        self.add_edge_with_attrs(from, to, GraphEdgeAttrs::default());
    }

    pub(crate) fn add_edge_with_attrs(
        &mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        attrs: GraphEdgeAttrs,
    ) {
        self.edges.push(AsciiGraphEdge {
            id: attrs.id,
            is_user_defined_id: attrs.is_user_defined_id,
            from: from.into(),
            to: to.into(),
            label: attrs.label,
            stroke: attrs.stroke,
            start_marker: attrs.start_marker,
            end_marker: attrs.end_marker,
            length: attrs.length.max(1),
            style: attrs.style,
        });
    }

    pub(crate) fn add_group_with_style(
        &mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        direction: Option<GraphDirection>,
        nodes: Vec<String>,
        style: GraphGroupStyle,
    ) {
        self.add_group_with_kind_and_style(
            id,
            title,
            direction,
            nodes,
            GraphGroupKind::Container,
            style,
        );
    }

    pub(crate) fn add_group_with_kind_and_style(
        &mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        direction: Option<GraphDirection>,
        nodes: Vec<String>,
        kind: GraphGroupKind,
        style: GraphGroupStyle,
    ) {
        self.groups.push(AsciiGraphGroup {
            id: id.into(),
            title: title.into(),
            kind,
            direction,
            nodes,
            style,
        });
    }
}
