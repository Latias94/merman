//! Supplies the operation services and prepared routes to the Structured Block writer.

use super::SvgExecution;
use crate::{
    DiagramFamilyId, Result,
    block::{BlockEdgeRoutes, BlockNodeBindings, BlockPreparedLabels, BlockTypographyThemePlan},
    model::BlockDiagramLayout,
};
use merman_model::families::block::BlockDiagramRenderModel;
use merman_render_core::root_svg::RootedSvg;
use merman_render_structured::block::{self, BlockSvgContext};

#[cfg(all(test, feature = "layout-shoal"))]
mod orthogonal_tests;

pub(super) fn render_block_diagram_svg_model_with_theme(
    layout: &BlockDiagramLayout,
    model: &BlockDiagramRenderModel,
    node_bindings: &BlockNodeBindings,
    edge_routes: &BlockEdgeRoutes,
    typography_theme: &BlockTypographyThemePlan,
    prepared_labels: &BlockPreparedLabels,
    options: &SvgExecution<'_>,
) -> Result<RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    block::render_block_diagram_svg_model_with_theme(
        layout,
        model,
        node_bindings,
        edge_routes.as_slice(),
        typography_theme,
        prepared_labels,
        &BlockSvgContext {
            diagram_id,
            viewport: options.root_viewport(DiagramFamilyId::BLOCK, diagram_id),
            work_meter: options.work_meter(),
            text_measurer: options.text_measurer(),
            operation: options.family.session().operation_context(),
            shoal: edge_routes.is_shoal(),
        },
    )
}
