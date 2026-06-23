use crate::color::AsciiRgb;

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
    Rounded,
    Diamond,
    Subroutine,
    Cylinder,
    StateStart,
    StateEnd,
    ForkJoinHorizontal,
    ForkJoinVertical,
    Choice,
}

impl GraphNodeShape {
    pub(super) fn is_diamond_like(self) -> bool {
        matches!(self, Self::Diamond | Self::Choice)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AsciiGraphEdge {
    pub(super) from: String,
    pub(super) to: String,
    pub(super) label: Option<String>,
    pub(super) stroke: GraphEdgeStroke,
    pub(super) arrow: GraphEdgeArrow,
    pub(super) length: usize,
    pub(super) style: GraphEdgeStyle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GraphEdgeAttrs {
    pub(crate) label: Option<String>,
    pub(crate) stroke: GraphEdgeStroke,
    pub(crate) arrow: GraphEdgeArrow,
    pub(crate) length: usize,
    pub(crate) style: GraphEdgeStyle,
}

impl Default for GraphEdgeAttrs {
    fn default() -> Self {
        Self {
            label: None,
            stroke: GraphEdgeStroke::Normal,
            arrow: GraphEdgeArrow::Point,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphEdgeArrow {
    Open,
    Point,
}

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
        self.nodes.push(AsciiGraphNode {
            id: id.into(),
            label: label.into(),
            shape,
            style,
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
            from: from.into(),
            to: to.into(),
            label: attrs.label,
            stroke: attrs.stroke,
            arrow: attrs.arrow,
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
