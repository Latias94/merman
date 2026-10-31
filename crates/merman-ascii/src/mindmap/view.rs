use crate::Result;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use crate::safe_text::charge_text_layout;
use merman_model::families::mindmap::{MindmapTerminalEdge as EdgeView, MindmapTerminalView};
pub(super) use merman_model::families::mindmap::{
    MindmapTerminalNode as NodeView, MindmapTerminalNodeId as NodeId,
};

pub(super) fn charge_node_id(id: NodeId<'_>, resources: &ResourceContext) -> Result<()> {
    match id {
        NodeId::Detached(text) => charge_text_layout(resources, text),
        NodeId::Canonical(id) => {
            // Numeric render IDs contain only single-byte ASCII graphemes.
            resources.check_grapheme_bytes(1)?;
            let digits = id.unsigned_abs().checked_ilog10().unwrap_or(0) as usize + 1;
            resources.charge_layout_work(1 + digits + usize::from(id < 0))
        }
    }
}

/// Adds terminal resource admission to the family-owned borrowed view.
#[derive(Clone, Copy)]
pub(crate) struct MindmapView<'a> {
    model: MindmapTerminalView<'a>,
}

impl<'a> From<MindmapTerminalView<'a>> for MindmapView<'a> {
    fn from(model: MindmapTerminalView<'a>) -> Self {
        Self { model }
    }
}

#[cfg(test)]
impl<'a> From<&'a merman_model::families::mindmap::MindmapDiagramRenderModel> for MindmapView<'a> {
    fn from(model: &'a merman_model::families::mindmap::MindmapDiagramRenderModel) -> Self {
        MindmapTerminalView::from_render(model).into()
    }
}

#[cfg(test)]
impl<'a>
    From<(
        &'a merman_model::families::mindmap::MindmapDiagramModel,
        &'a merman_model::MermaidConfig,
    )> for MindmapView<'a>
{
    fn from(
        (model, config): (
            &'a merman_model::families::mindmap::MindmapDiagramModel,
            &'a merman_model::MermaidConfig,
        ),
    ) -> Self {
        MindmapTerminalView::from_semantic(model, config).into()
    }
}

impl<'a> MindmapView<'a> {
    pub(super) fn node_count(self) -> usize {
        self.model.node_count()
    }

    pub(super) fn nodes(self) -> impl Iterator<Item = NodeView<'a>> + Clone {
        self.model.nodes()
    }

    pub(super) fn node(self, index: usize) -> Result<NodeView<'a>> {
        self.model
            .node(index)
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type: "mindmap",
                feature: "invalid node index",
            })
    }

    pub(super) fn edge_count(
        self,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<usize> {
        if let Some(count) = self.model.edge_count_hint() {
            return Ok(count);
        }
        let mut count = 0;
        self.model.try_visit_child_counts(|child_count| {
            execution.checkpoint(merman_model::OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            count = resources.checked_work_add(count, child_count)?;
            Ok::<(), AsciiError>(())
        })?;
        Ok(count)
    }

    pub(super) fn visit_edges(
        self,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
        mut visit: impl FnMut(EdgeView<'a>) -> Result<()>,
    ) -> Result<()> {
        self.model.try_visit_edges(
            || {
                execution.checkpoint(merman_model::OperationPhase::Layout)?;
                resources.charge_layout_work(1)
            },
            |edge| {
                execution.checkpoint(merman_model::OperationPhase::Layout)?;
                visit(edge)
            },
        )
    }

    pub(super) fn section(
        self,
        node: NodeView<'_>,
        inherited: Option<i32>,
        depth: usize,
    ) -> Option<i32> {
        self.model.section(node, inherited, depth)
    }

    pub(super) fn child_section(
        self,
        node: NodeView<'_>,
        child_index: usize,
        inherited: Option<i32>,
    ) -> Option<i32> {
        self.model.child_section(node, child_index, inherited)
    }
}
