mod plot;
mod render;

pub(crate) use render::render_xychart_diagram_with_resolved_policy;

use merman_core::diagrams::xychart::{XyChartDiagramModel, XyChartDisplayPolicy};

#[derive(Clone, Copy)]
pub(crate) struct XyChartView<'a> {
    semantic: &'a XyChartDiagramModel,
    display: XyChartDisplayPolicy,
}

impl<'a> XyChartView<'a> {
    pub(crate) const fn new(
        semantic: &'a XyChartDiagramModel,
        display: XyChartDisplayPolicy,
    ) -> Self {
        Self { semantic, display }
    }
}

impl std::ops::Deref for XyChartView<'_> {
    type Target = XyChartDiagramModel;

    fn deref(&self) -> &Self::Target {
        self.semantic
    }
}
