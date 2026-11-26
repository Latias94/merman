use merman_model::{families::pie::PieDiagramModel, presentation::PresentationConfig};
use merman_render_core::{Result, root_svg::RootedSvg, svg_css::SvgId};

use super::SvgExecution;
use crate::{
    DiagramFamilyId,
    model::PieDiagramLayout,
    pie::{PieAppearance, PieSvgContext, render_pie_svg},
};

const DEFAULT_PIE_DIAGRAM_ID: SvgId<'static> = match SvgId::new("merman") {
    Some(id) => id,
    None => panic!("invalid fallback Pie ID"),
};

pub(super) fn render_pie_svg_artifact(
    layout: &PieDiagramLayout,
    model: &PieDiagramModel,
    appearance: &PieAppearance,
    effective_config: &PresentationConfig,
    execution: &SvgExecution<'_>,
) -> Result<RootedSvg> {
    let stylesheet_id = execution
        .validated_diagram_id
        .unwrap_or(DEFAULT_PIE_DIAGRAM_ID);
    let diagram_id = execution.project_svg_id(stylesheet_id);
    let session = execution.family.session();
    render_pie_svg(
        layout,
        model,
        appearance,
        effective_config,
        &PieSvgContext {
            stylesheet_id,
            diagram_id,
            viewport: execution.root_viewport(DiagramFamilyId::PIE, diagram_id),
            work_meter: execution.work_meter(),
            canvas_background: session.canvas_background(),
            appearance_and_backdrop: session
                .native_theme()
                .map(|theme| (theme.resolved().appearance(), theme.export_backdrop())),
        },
    )
}
