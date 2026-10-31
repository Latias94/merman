use super::{MindmapDiagramModel, MindmapDisplayPolicy, MindmapNodeModel};
#[cfg(feature = "presentation-data")]
use super::{MindmapDiagramRenderModel, MindmapDiagramRenderNode};
use crate::MermaidConfig;
use serde::Serialize;

/// Terminal graph identity, preserving numeric canonical IDs and detached string IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MindmapTerminalNodeId<'a> {
    Canonical(i32),
    Detached(&'a str),
}

/// Borrowed authored fields used by terminal Mindmap consumers.
#[derive(Debug, Clone, Copy)]
pub struct MindmapTerminalNode<'a> {
    pub id: MindmapTerminalNodeId<'a>,
    pub node_id: &'a str,
    pub label: &'a str,
    pub shape: &'a str,
    pub icon: Option<&'a str>,
    pub section: Option<i32>,
    authored_level: Option<i32>,
}

impl<'a> MindmapTerminalNode<'a> {
    fn from_semantic(node: &'a MindmapNodeModel, display: MindmapDisplayPolicy) -> Self {
        Self {
            id: MindmapTerminalNodeId::Canonical(node.id),
            node_id: &node.node_id,
            label: &node.descr,
            shape: display.shape(node.node_type),
            icon: node.icon.as_deref(),
            section: None,
            authored_level: Some(node.level),
        }
    }

    #[cfg(feature = "presentation-data")]
    fn from_render(node: &'a MindmapDiagramRenderNode) -> Self {
        Self {
            id: MindmapTerminalNodeId::Detached(&node.id),
            node_id: &node.node_id,
            label: &node.label,
            shape: &node.shape,
            icon: node.icon.as_deref(),
            section: node.section,
            authored_level: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MindmapTerminalEdge<'a> {
    pub id: &'a str,
    pub start: MindmapTerminalNodeId<'a>,
    pub end: MindmapTerminalNodeId<'a>,
}

#[derive(Debug, Clone, Copy)]
enum MindmapTerminalBacking<'a> {
    Semantic(&'a MindmapDiagramModel, MindmapDisplayPolicy),
    #[cfg(feature = "presentation-data")]
    Render(&'a MindmapDiagramRenderModel),
}

/// Borrows terminal facts from one owning Mindmap model.
#[derive(Debug, Clone, Copy)]
pub struct MindmapTerminalView<'a> {
    backing: MindmapTerminalBacking<'a>,
}

impl<'a> MindmapTerminalView<'a> {
    pub fn from_semantic(model: &'a MindmapDiagramModel, config: &MermaidConfig) -> Self {
        Self {
            backing: MindmapTerminalBacking::Semantic(
                model,
                MindmapDisplayPolicy::from_config(config),
            ),
        }
    }

    #[cfg(feature = "presentation-data")]
    pub fn from_render(model: &'a MindmapDiagramRenderModel) -> Self {
        Self {
            backing: MindmapTerminalBacking::Render(model),
        }
    }

    pub fn node_count(self) -> usize {
        match self.backing {
            MindmapTerminalBacking::Semantic(model, _) => model.nodes.len(),
            #[cfg(feature = "presentation-data")]
            MindmapTerminalBacking::Render(model) => model.nodes.len(),
        }
    }

    pub fn node(self, index: usize) -> Option<MindmapTerminalNode<'a>> {
        match self.backing {
            MindmapTerminalBacking::Semantic(model, display) => model
                .nodes
                .get(index)
                .map(|node| MindmapTerminalNode::from_semantic(node, display)),
            #[cfg(feature = "presentation-data")]
            MindmapTerminalBacking::Render(model) => {
                model.nodes.get(index).map(MindmapTerminalNode::from_render)
            }
        }
    }

    pub fn nodes(self) -> impl Iterator<Item = MindmapTerminalNode<'a>> + Clone {
        let (semantic, display) = match self.backing {
            MindmapTerminalBacking::Semantic(model, display) => {
                (model.nodes.as_slice(), Some(display))
            }
            #[cfg(feature = "presentation-data")]
            MindmapTerminalBacking::Render(_) => (&[][..], None),
        };
        let semantic_nodes = semantic.iter().filter_map(move |node| {
            display.map(|display| MindmapTerminalNode::from_semantic(node, display))
        });
        #[cfg(feature = "presentation-data")]
        {
            let render = match self.backing {
                MindmapTerminalBacking::Render(model) => model.nodes.as_slice(),
                MindmapTerminalBacking::Semantic(_, _) => &[],
            };
            semantic_nodes.chain(render.iter().map(MindmapTerminalNode::from_render))
        }
        #[cfg(not(feature = "presentation-data"))]
        {
            semantic_nodes
        }
    }

    /// Returns a ready edge count only when the backing owner already stores flat edges.
    pub fn edge_count_hint(self) -> Option<usize> {
        match self.backing {
            MindmapTerminalBacking::Semantic(_, _) => None,
            #[cfg(feature = "presentation-data")]
            MindmapTerminalBacking::Render(model) => Some(model.edges.len()),
        }
    }

    /// Calls the visitor once per canonical node, including empty child lists.
    ///
    /// The visitor can checkpoint, charge the scan, check the sum, or stop before the next node.
    /// Flat render edges require no node scan; their count is available from `edge_count_hint`.
    pub fn try_visit_child_counts<E>(
        self,
        mut visit: impl FnMut(usize) -> std::result::Result<(), E>,
    ) -> std::result::Result<(), E> {
        match self.backing {
            MindmapTerminalBacking::Semantic(model, _) => {
                for node in &model.nodes {
                    visit(node.children.len())?;
                }
            }
            #[cfg(feature = "presentation-data")]
            MindmapTerminalBacking::Render(_) => {}
        }
        Ok(())
    }

    /// Visits edges in owner order without materializing a flat edge vector.
    ///
    /// `scan_node` runs before each canonical node's child scan, including empty lists.
    /// `visit_edge` runs for every edge. Either callback can stop traversal immediately;
    /// consumers own cancellation and resource admission in these callbacks.
    /// Flat render owners invoke only `visit_edge`.
    pub fn try_visit_edges<E>(
        self,
        mut scan_node: impl FnMut() -> std::result::Result<(), E>,
        mut visit_edge: impl FnMut(MindmapTerminalEdge<'a>) -> std::result::Result<(), E>,
    ) -> std::result::Result<(), E> {
        match self.backing {
            MindmapTerminalBacking::Semantic(model, _) => {
                for node in &model.nodes {
                    scan_node()?;
                    for child in &node.children {
                        visit_edge(MindmapTerminalEdge {
                            id: "",
                            start: MindmapTerminalNodeId::Canonical(node.id),
                            end: MindmapTerminalNodeId::Canonical(*child),
                        })?;
                    }
                }
            }
            #[cfg(feature = "presentation-data")]
            MindmapTerminalBacking::Render(model) => {
                for edge in &model.edges {
                    visit_edge(MindmapTerminalEdge {
                        id: &edge.id,
                        start: MindmapTerminalNodeId::Detached(&edge.start),
                        end: MindmapTerminalNodeId::Detached(&edge.end),
                    })?;
                }
            }
        }
        Ok(())
    }

    pub fn section(
        self,
        node: MindmapTerminalNode<'_>,
        inherited: Option<i32>,
        depth: usize,
    ) -> Option<i32> {
        match (self.backing, node.authored_level) {
            (MindmapTerminalBacking::Semantic(_, display), Some(level)) => {
                display.section(level, inherited, depth)
            }
            _ => node.section,
        }
    }

    pub fn child_section(
        self,
        node: MindmapTerminalNode<'_>,
        child_index: usize,
        inherited: Option<i32>,
    ) -> Option<i32> {
        match (self.backing, node.authored_level) {
            (MindmapTerminalBacking::Semantic(_, display), Some(level)) => {
                display.child_section(level, child_index, inherited)
            }
            _ => None,
        }
    }

    /// Counts the canonical graph and generated display text, without counting display fields
    /// as additional graph nodes, edges, or nesting levels.
    pub fn complexity(self) -> crate::resources::ModelComplexity {
        use crate::resources::MindmapComplexity;
        match self.backing {
            MindmapTerminalBacking::Semantic(model, display) => {
                let mut complexity =
                    MindmapComplexity::from_semantic_model(model).as_model_complexity();
                complexity.text_bytes = complexity
                    .text_bytes
                    .saturating_add(display.shape(super::NODE_TYPE_DEFAULT).len());
                for node in &model.nodes {
                    complexity.text_bytes = complexity
                        .text_bytes
                        .saturating_add(display.shape(node.node_type).len());
                }
                complexity
            }
            #[cfg(feature = "presentation-data")]
            MindmapTerminalBacking::Render(model) => {
                MindmapComplexity::from_model(model).as_model_complexity()
            }
        }
    }
}

impl Serialize for MindmapTerminalView<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.backing {
            MindmapTerminalBacking::Semantic(model, display) => {
                #[derive(Serialize)]
                struct Wire<'a> {
                    nodes: CanonicalNodes<'a>,
                    display: MindmapDisplayPolicy,
                }
                Wire {
                    nodes: CanonicalNodes {
                        nodes: &model.nodes,
                        display,
                    },
                    display,
                }
                .serialize(serializer)
            }
            #[cfg(feature = "presentation-data")]
            MindmapTerminalBacking::Render(model) => model.serialize(serializer),
        }
    }
}

struct CanonicalNodes<'a> {
    nodes: &'a [MindmapNodeModel],
    display: MindmapDisplayPolicy,
}

impl Serialize for CanonicalNodes<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Node<'a> {
            #[serde(flatten)]
            authored: &'a MindmapNodeModel,
            shape: &'static str,
        }
        serializer.collect_seq(self.nodes.iter().map(|node| Node {
            authored: node,
            shape: self.display.shape(node.node_type),
        }))
    }
}
