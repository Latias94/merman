use crate::Result;
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use crate::safe_text::charge_text_layout;
use merman_core::MermaidConfig;
use merman_core::diagrams::mindmap::{
    MindmapDiagramModel, MindmapDiagramRenderModel, MindmapDisplayPolicy,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum NodeId<'a> {
    Canonical(i32),
    Detached(&'a str),
}

impl NodeId<'_> {
    pub(super) fn charge_text(self, resources: &ResourceContext) -> Result<()> {
        match self {
            Self::Detached(text) => charge_text_layout(resources, text),
            Self::Canonical(id) => {
                // The old numeric render ID contains only single-byte ASCII graphemes.
                // Keep its scan admission without allocating a temporary decimal string.
                resources.check_grapheme_bytes(1)?;
                let digits = id.unsigned_abs().checked_ilog10().unwrap_or(0) as usize + 1;
                resources.charge_layout_work(1 + digits + usize::from(id < 0))
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct NodeView<'a> {
    pub(super) id: NodeId<'a>,
    pub(super) node_id: &'a str,
    pub(super) label: &'a str,
    pub(super) shape: &'a str,
    pub(super) icon: Option<&'a str>,
    pub(super) section: Option<i32>,
    authored_level: Option<i32>,
}

#[derive(Clone, Copy)]
pub(super) struct EdgeView<'a> {
    pub(super) id: &'a str,
    pub(super) start: NodeId<'a>,
    pub(super) end: NodeId<'a>,
}

/// Borrows either canonical tree records or explicitly supplied full-model fields.
#[derive(Clone, Copy)]
pub(crate) enum MindmapView<'a> {
    Canonical(&'a MindmapDiagramModel, MindmapDisplayPolicy),
    Full(&'a MindmapDiagramRenderModel),
}

impl<'a> From<&'a MindmapDiagramRenderModel> for MindmapView<'a> {
    fn from(model: &'a MindmapDiagramRenderModel) -> Self {
        Self::Full(model)
    }
}

impl<'a> From<(&'a MindmapDiagramModel, &'a MermaidConfig)> for MindmapView<'a> {
    fn from((model, config): (&'a MindmapDiagramModel, &'a MermaidConfig)) -> Self {
        Self::Canonical(model, MindmapDisplayPolicy::from_config(config))
    }
}

impl<'a> MindmapView<'a> {
    pub(super) fn node_count(self) -> usize {
        match self {
            Self::Canonical(model, _) => model.nodes.len(),
            Self::Full(model) => model.nodes.len(),
        }
    }

    pub(super) fn nodes(self) -> impl ExactSizeIterator<Item = NodeView<'a>> {
        (0..self.node_count()).map(move |index| self.node(index))
    }

    pub(super) fn node(self, index: usize) -> NodeView<'a> {
        match self {
            Self::Canonical(model, display) => {
                let node = &model.nodes[index];
                NodeView {
                    id: NodeId::Canonical(node.id),
                    node_id: &node.node_id,
                    label: &node.descr,
                    shape: display.shape(node.node_type),
                    icon: node.icon.as_deref(),
                    section: None,
                    authored_level: Some(node.level),
                }
            }
            Self::Full(model) => {
                let node = &model.nodes[index];
                NodeView {
                    id: NodeId::Detached(&node.id),
                    node_id: &node.node_id,
                    label: &node.label,
                    shape: &node.shape,
                    icon: node.icon.as_deref(),
                    section: node.section,
                    authored_level: None,
                }
            }
        }
    }

    pub(super) fn edge_count(
        self,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<usize> {
        match self {
            Self::Full(model) => Ok(model.edges.len()),
            Self::Canonical(model, _) => {
                let mut count = 0;
                for node in &model.nodes {
                    execution.checkpoint(merman_core::OperationPhase::Layout)?;
                    resources.charge_layout_work(1)?;
                    count = resources.checked_work_add(count, node.children.len())?;
                }
                Ok(count)
            }
        }
    }

    pub(super) fn visit_edges(
        self,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
        mut visit: impl FnMut(EdgeView<'a>) -> Result<()>,
    ) -> Result<()> {
        match self {
            Self::Canonical(model, _) => {
                for node in &model.nodes {
                    execution.checkpoint(merman_core::OperationPhase::Layout)?;
                    resources.charge_layout_work(1)?;
                    for child in &node.children {
                        execution.checkpoint(merman_core::OperationPhase::Layout)?;
                        visit(EdgeView {
                            id: "",
                            start: NodeId::Canonical(node.id),
                            end: NodeId::Canonical(*child),
                        })?;
                    }
                }
            }
            Self::Full(model) => {
                for edge in &model.edges {
                    execution.checkpoint(merman_core::OperationPhase::Layout)?;
                    visit(EdgeView {
                        id: &edge.id,
                        start: NodeId::Detached(&edge.start),
                        end: NodeId::Detached(&edge.end),
                    })?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn section(
        self,
        node: NodeView<'_>,
        inherited: Option<i32>,
        depth: usize,
    ) -> Option<i32> {
        match (self, node.authored_level) {
            (Self::Canonical(_, display), Some(level)) => display.section(level, inherited, depth),
            _ => node.section,
        }
    }

    pub(super) fn child_section(
        self,
        node: NodeView<'_>,
        child_index: usize,
        inherited: Option<i32>,
    ) -> Option<i32> {
        match (self, node.authored_level) {
            (Self::Canonical(_, display), Some(level)) => {
                display.child_section(level, child_index, inherited)
            }
            _ => None,
        }
    }
}
