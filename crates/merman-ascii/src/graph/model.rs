#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphDirection {
    LeftRight,
    TopDown,
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AsciiGraphGroup {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) nodes: Vec<String>,
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
        self.add_node_with_shape(id, label, GraphNodeShape::Rect);
    }

    pub(super) fn add_node_with_shape(
        &mut self,
        id: impl Into<String>,
        label: impl Into<String>,
        shape: GraphNodeShape,
    ) {
        self.nodes.push(AsciiGraphNode {
            id: id.into(),
            label: label.into(),
            shape,
        });
    }

    #[cfg(test)]
    pub(crate) fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) {
        self.add_edge_with_attrs(
            from,
            to,
            None,
            GraphEdgeStroke::Normal,
            GraphEdgeArrow::Point,
            1,
        );
    }

    pub(super) fn add_edge_with_attrs(
        &mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        label: Option<String>,
        stroke: GraphEdgeStroke,
        arrow: GraphEdgeArrow,
        length: usize,
    ) {
        self.edges.push(AsciiGraphEdge {
            from: from.into(),
            to: to.into(),
            label,
            stroke,
            arrow,
            length: length.max(1),
        });
    }

    pub(super) fn add_group(
        &mut self,
        id: impl Into<String>,
        title: impl Into<String>,
        nodes: Vec<String>,
    ) {
        self.groups.push(AsciiGraphGroup {
            id: id.into(),
            title: title.into(),
            nodes,
        });
    }
}
