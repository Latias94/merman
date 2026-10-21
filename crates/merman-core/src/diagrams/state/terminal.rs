use serde_json::Value;

use super::{StateDiagramModel, StateEdge, StateNode};
#[cfg(feature = "presentation")]
use super::{StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode};

/// Borrowed State fields used by terminal graph adapters.
#[derive(Clone, Copy, Debug)]
pub struct StateTerminalView<'a> {
    pub direction: &'a str,
    pub nodes: StateTerminalNodes<'a>,
    pub edges: StateTerminalEdges<'a>,
}

impl<'a> StateTerminalView<'a> {
    pub fn from_semantic(model: &'a StateDiagramModel) -> Self {
        Self {
            direction: &model.direction,
            nodes: StateTerminalNodes::Semantic(&model.nodes),
            edges: StateTerminalEdges::Semantic(&model.edges),
        }
    }

    #[cfg(feature = "presentation")]
    pub fn from_render(model: &'a StateDiagramRenderModel) -> Self {
        Self {
            direction: &model.direction,
            nodes: StateTerminalNodes::Render(&model.nodes),
            edges: StateTerminalEdges::Render(&model.edges),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum StateTerminalNodes<'a> {
    Semantic(&'a [StateNode]),
    #[cfg(feature = "presentation")]
    Render(&'a [StateDiagramRenderNode]),
}

impl<'a> StateTerminalNodes<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Semantic(nodes) => nodes.len(),
            #[cfg(feature = "presentation")]
            Self::Render(nodes) => nodes.len(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub fn get(self, index: usize) -> Option<StateTerminalNode<'a>> {
        match self {
            Self::Semantic(nodes) => nodes.get(index).map(StateTerminalNode::from_semantic),
            #[cfg(feature = "presentation")]
            Self::Render(nodes) => nodes.get(index).map(StateTerminalNode::from_render),
        }
    }

    pub fn iter(self) -> impl Iterator<Item = StateTerminalNode<'a>> + Clone {
        let semantic = match self {
            Self::Semantic(nodes) => nodes,
            #[cfg(feature = "presentation")]
            Self::Render(_) => &[],
        };
        #[cfg(feature = "presentation")]
        {
            let render = match self {
                Self::Render(nodes) => nodes,
                Self::Semantic(_) => &[],
            };
            semantic
                .iter()
                .map(StateTerminalNode::from_semantic)
                .chain(render.iter().map(StateTerminalNode::from_render))
        }
        #[cfg(not(feature = "presentation"))]
        {
            semantic.iter().map(StateTerminalNode::from_semantic)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum StateTerminalEdges<'a> {
    Semantic(&'a [StateEdge]),
    #[cfg(feature = "presentation")]
    Render(&'a [StateDiagramRenderEdge]),
}

impl<'a> StateTerminalEdges<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Semantic(edges) => edges.len(),
            #[cfg(feature = "presentation")]
            Self::Render(edges) => edges.len(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub fn iter(self) -> impl Iterator<Item = StateTerminalEdge<'a>> + Clone {
        let semantic = match self {
            Self::Semantic(edges) => edges,
            #[cfg(feature = "presentation")]
            Self::Render(_) => &[],
        };
        #[cfg(feature = "presentation")]
        {
            let render = match self {
                Self::Render(edges) => edges,
                Self::Semantic(_) => &[],
            };
            semantic
                .iter()
                .map(StateTerminalEdge::from_semantic)
                .chain(render.iter().map(StateTerminalEdge::from_render))
        }
        #[cfg(not(feature = "presentation"))]
        {
            semantic.iter().map(StateTerminalEdge::from_semantic)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StateTerminalNode<'a> {
    pub id: &'a str,
    pub label: Option<&'a Value>,
    pub description: Option<&'a [String]>,
    pub is_group: bool,
    pub parent_id: Option<&'a str>,
    pub css_compiled_styles: &'a [String],
    pub css_styles: &'a [String],
    pub dir: Option<&'a str>,
    pub explicit_dir: Option<bool>,
    pub shape: &'a str,
    pub position: Option<&'a str>,
    /// Detached render models may carry an additional authored terminal style.
    pub label_style: &'a str,
}

impl<'a> StateTerminalNode<'a> {
    fn from_semantic(node: &'a StateNode) -> Self {
        Self {
            id: &node.id,
            label: node.label.as_ref(),
            description: node.description.as_deref(),
            is_group: node.is_group,
            parent_id: node.parent_id.as_deref(),
            css_compiled_styles: &node.css_compiled_styles,
            css_styles: &node.css_styles,
            dir: node.dir.as_deref(),
            explicit_dir: node.explicit_dir,
            shape: &node.shape,
            position: node.position.as_deref(),
            label_style: "",
        }
    }

    #[cfg(feature = "presentation")]
    fn from_render(node: &'a StateDiagramRenderNode) -> Self {
        Self {
            id: &node.id,
            label: node.label.as_ref(),
            description: node.description.as_deref(),
            is_group: node.is_group,
            parent_id: node.parent_id.as_deref(),
            css_compiled_styles: &node.css_compiled_styles,
            css_styles: &node.css_styles,
            dir: node.dir.as_deref(),
            explicit_dir: node.explicit_dir,
            shape: &node.shape,
            position: node.position.as_deref(),
            label_style: &node.label_style,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct StateTerminalEdge<'a> {
    pub start: &'a str,
    pub end: &'a str,
    pub classes: &'a str,
    pub label: &'a str,
    /// Validated for detached render models; canonical edges have no visual arrow selector.
    pub arrow_type_end: &'a str,
}

impl<'a> StateTerminalEdge<'a> {
    fn from_semantic(edge: &'a StateEdge) -> Self {
        Self {
            start: &edge.start,
            end: &edge.end,
            classes: &edge.classes,
            label: &edge.label,
            arrow_type_end: "",
        }
    }

    #[cfg(feature = "presentation")]
    fn from_render(edge: &'a StateDiagramRenderEdge) -> Self {
        Self {
            start: &edge.start,
            end: &edge.end,
            classes: &edge.classes,
            label: &edge.label,
            arrow_type_end: &edge.arrow_type_end,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_terminal_view_borrows_nodes_and_edges() {
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync("stateDiagram-v2\nA --> B: ready\n")
            .expect("canonical state source should parse");
        let crate::SemanticDiagramModel::State(model) = &parsed.model else {
            panic!("expected State model");
        };
        let view = StateTerminalView::from_semantic(model);
        assert_eq!(view.nodes.len(), model.nodes.len());
        assert_eq!(view.edges.len(), model.edges.len());
        assert!(!view.nodes.is_empty());
        assert!(!view.edges.is_empty());
        for (view, node) in view.nodes.iter().zip(&model.nodes) {
            assert!(std::ptr::eq(view.id.as_ptr(), node.id.as_ptr()));
            assert!(std::ptr::eq(view.shape.as_ptr(), node.shape.as_ptr()));
            assert!(view.label_style.is_empty());
        }
        for (view, edge) in view.edges.iter().zip(&model.edges) {
            assert!(std::ptr::eq(view.start.as_ptr(), edge.start.as_ptr()));
            assert!(std::ptr::eq(view.label.as_ptr(), edge.label.as_ptr()));
            assert!(view.arrow_type_end.is_empty());
        }
        assert!(view.nodes.get(model.nodes.len()).is_none());
    }
}
