use merman_model::{families::c4::C4DiagramModel, family::DiagramFamilyId};
use merman_render_core::root_svg::RootedSvg;
use merman_render_graph::c4::{C4SvgContext, render_c4_diagram_svg_typed as render_c4};

use super::SvgExecution;
use crate::{
    Result,
    c4::{C4ClusterThemePlan, C4TextPaintPlan, C4TypographyThemePlan},
    model::C4DiagramLayout,
};

#[cfg(all(test, feature = "layout-shoal"))]
mod tests;

#[allow(
    clippy::too_many_arguments,
    reason = "Forward the existing typed C4 render inputs."
)]
pub(super) fn render_c4_diagram_svg_typed(
    layout: &C4DiagramLayout,
    model: &C4DiagramModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    typography_theme: &C4TypographyThemePlan,
    cluster_theme: &C4ClusterThemePlan,
    text_paint: &C4TextPaintPlan,
    #[cfg(feature = "layout-shoal")] shoal: Option<&crate::c4::C4ShoalGeometry>,
    options: &SvgExecution<'_>,
) -> Result<RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    render_c4(
        layout,
        model,
        effective_config,
        diagram_title,
        typography_theme,
        cluster_theme,
        text_paint,
        #[cfg(feature = "layout-shoal")]
        shoal.map(crate::c4::C4ShoalGeometry::routed),
        C4SvgContext {
            diagram_id,
            viewport: options.root_viewport(DiagramFamilyId::C4, diagram_id),
            work_meter: options.work_meter(),
            operation: options.family.session().operation_context(),
        },
    )
}
