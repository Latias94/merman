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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsciiGraph {
    pub(super) direction: GraphDirection,
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
pub(super) struct GraphNodeStyle {
    pub(super) text: Option<AsciiRgb>,
    pub(super) border: Option<AsciiRgb>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GraphNodeShape {
    Rect,
    Rounded,
    Diamond,
    Subroutine,
    Cylinder,
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
pub(super) struct GraphEdgeAttrs {
    pub(super) label: Option<String>,
    pub(super) stroke: GraphEdgeStroke,
    pub(super) arrow: GraphEdgeArrow,
    pub(super) length: usize,
    pub(super) style: GraphEdgeStyle,
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
pub(super) struct GraphEdgeStyle {
    pub(super) line: Option<AsciiRgb>,
    pub(super) arrow: Option<AsciiRgb>,
    pub(super) label: Option<AsciiRgb>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AsciiGraphGroup {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) direction: Option<GraphDirection>,
    pub(super) nodes: Vec<String>,
    pub(super) style: GraphGroupStyle,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct GraphGroupStyle {
    pub(super) title: Option<AsciiRgb>,
    pub(super) border: Option<AsciiRgb>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GraphEdgeStroke {
    Normal,
    Dotted,
    Thick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GraphEdgeArrow {
    Open,
    Point,
}

impl AsciiGraph {
    pub(crate) fn new(direction: GraphDirection) -> Self {
        Self {
            direction,
            nodes: Vec::new(),
            edges: Vec::new(),
            groups: Vec::new(),
        }
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

    pub(super) fn add_node_with_shape_and_style(
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

    pub(super) fn add_edge_with_attrs(
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

    pub(super) fn add_group_with_style(
        &mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        direction: Option<GraphDirection>,
        nodes: Vec<String>,
        style: GraphGroupStyle,
    ) {
        self.groups.push(AsciiGraphGroup {
            id: id.into(),
            title: title.into(),
            direction,
            nodes,
            style,
        });
    }
}
