#[cfg(feature = "presentation-data")]
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};
use serde_json::Value;

#[cfg(feature = "presentation-data")]
use super::complexity::{FlatDocuments, StateModelFields, StateRecords};
use super::{StateDiagramModel, StateEdge, StateNode};
#[cfg(feature = "presentation-data")]
use super::{StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode};
use crate::resources::ModelComplexity;

#[derive(Clone, Copy, Debug)]
enum StateTerminalOwner<'a> {
    Semantic(&'a StateDiagramModel),
    #[cfg(feature = "presentation-data")]
    Render(&'a StateDiagramRenderModel),
}

/// Paired borrowed State owner for terminal graph and complete fallback consumers.
///
/// Serialization retains the flat statement document and document references in state records.
/// Full owners retain their typed node and edge fields. Unlike Mermaid compatibility JSON,
/// this shape does not duplicate nested documents or synthesize config, look, or theme values.
#[derive(Clone, Copy, Debug)]
pub struct StateTerminalView<'a> {
    owner: StateTerminalOwner<'a>,
}

impl<'a> StateTerminalView<'a> {
    pub fn from_semantic(model: &'a StateDiagramModel) -> Self {
        Self {
            owner: StateTerminalOwner::Semantic(model),
        }
    }

    #[cfg(feature = "presentation-data")]
    pub fn from_render(model: &'a StateDiagramRenderModel) -> Self {
        Self {
            owner: StateTerminalOwner::Render(model),
        }
    }

    pub fn direction(self) -> &'a str {
        match self.owner {
            StateTerminalOwner::Semantic(model) => &model.direction,
            #[cfg(feature = "presentation-data")]
            StateTerminalOwner::Render(model) => &model.direction,
        }
    }

    pub fn nodes(self) -> StateTerminalNodes<'a> {
        match self.owner {
            StateTerminalOwner::Semantic(model) => StateTerminalNodes::Semantic(&model.nodes),
            #[cfg(feature = "presentation-data")]
            StateTerminalOwner::Render(model) => StateTerminalNodes::Render(&model.nodes),
        }
    }

    pub fn edges(self) -> StateTerminalEdges<'a> {
        match self.owner {
            StateTerminalOwner::Semantic(model) => StateTerminalEdges::Semantic(&model.edges),
            #[cfg(feature = "presentation-data")]
            StateTerminalOwner::Render(model) => StateTerminalEdges::Render(&model.edges),
        }
    }

    /// Counts this complete serialized shape, retaining the authored logical nesting bound.
    pub fn complexity(self) -> ModelComplexity {
        let mut complexity = ModelComplexity::from_serializable(&self);
        let document = match self.owner {
            StateTerminalOwner::Semantic(model) => &model.document,
            #[cfg(feature = "presentation-data")]
            StateTerminalOwner::Render(model) => &model.document,
        };
        complexity.nesting_depth = complexity
            .nesting_depth
            .max(document.semantic_nesting_depth());
        complexity
    }
}

impl Serialize for StateTerminalView<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.owner {
            StateTerminalOwner::Semantic(model) => model.terminal_fields().serialize(serializer),
            #[cfg(feature = "presentation-data")]
            StateTerminalOwner::Render(model) => StateModelFields {
                document: FlatDocuments(&model.document),
                direction: &model.direction,
                acc_title: &model.acc_title,
                acc_descr: &model.acc_descr,
                nodes: FullStateNodes(&model.nodes),
                edges: &model.edges,
                relations: &model.relations,
                links: &model.links,
                states: StateRecords {
                    states: &model.states,
                    include_documents: true,
                },
                style_classes: &model.style_classes,
            }
            .serialize(serializer),
        }
    }
}

#[cfg(feature = "presentation-data")]
struct FullStateNodes<'a>(&'a [StateDiagramRenderNode]);

#[cfg(feature = "presentation-data")]
impl Serialize for FullStateNodes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for node in self.0 {
            sequence.serialize_element(&FullStateNode {
                node,
                explicit_dir: node.explicit_dir,
            })?;
        }
        sequence.end()
    }
}

#[cfg(feature = "presentation-data")]
#[derive(Serialize)]
struct FullStateNode<'a> {
    #[serde(flatten)]
    node: &'a StateDiagramRenderNode,
    // Detached full models may have authored direction provenance without a document.
    #[serde(rename = "explicitDir", skip_serializing_if = "Option::is_none")]
    explicit_dir: Option<bool>,
}

#[derive(Clone, Copy, Debug)]
pub enum StateTerminalNodes<'a> {
    Semantic(&'a [StateNode]),
    #[cfg(feature = "presentation-data")]
    Render(&'a [StateDiagramRenderNode]),
}

impl<'a> StateTerminalNodes<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Semantic(nodes) => nodes.len(),
            #[cfg(feature = "presentation-data")]
            Self::Render(nodes) => nodes.len(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub fn get(self, index: usize) -> Option<StateTerminalNode<'a>> {
        match self {
            Self::Semantic(nodes) => nodes.get(index).map(StateTerminalNode::from_semantic),
            #[cfg(feature = "presentation-data")]
            Self::Render(nodes) => nodes.get(index).map(StateTerminalNode::from_render),
        }
    }

    pub fn iter(self) -> impl Iterator<Item = StateTerminalNode<'a>> + Clone {
        let semantic = match self {
            Self::Semantic(nodes) => nodes,
            #[cfg(feature = "presentation-data")]
            Self::Render(_) => &[],
        };
        #[cfg(feature = "presentation-data")]
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
        #[cfg(not(feature = "presentation-data"))]
        {
            semantic.iter().map(StateTerminalNode::from_semantic)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum StateTerminalEdges<'a> {
    Semantic(&'a [StateEdge]),
    #[cfg(feature = "presentation-data")]
    Render(&'a [StateDiagramRenderEdge]),
}

impl<'a> StateTerminalEdges<'a> {
    pub fn len(self) -> usize {
        match self {
            Self::Semantic(edges) => edges.len(),
            #[cfg(feature = "presentation-data")]
            Self::Render(edges) => edges.len(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub fn iter(self) -> impl Iterator<Item = StateTerminalEdge<'a>> + Clone {
        let semantic = match self {
            Self::Semantic(edges) => edges,
            #[cfg(feature = "presentation-data")]
            Self::Render(_) => &[],
        };
        #[cfg(feature = "presentation-data")]
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
        #[cfg(not(feature = "presentation-data"))]
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

    #[cfg(feature = "presentation-data")]
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

    #[cfg(feature = "presentation-data")]
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
