use merman_model::{families::zenuml::ZenumlDiagramModel, family::DiagramFamilyId};
use merman_render_core::root_svg::RootedSvg;
use merman_render_sequence::zenuml::{
    ZenumlSvgContext, render_zenuml_diagram_svg_model as render_svg,
};

use super::SvgExecution;
use crate::{
    Result,
    zenuml::{ZenumlDiagramLayout, ZenumlSourceStyle},
};

pub(super) fn render_zenuml_diagram_svg_model(
    layout: &ZenumlDiagramLayout,
    model: &ZenumlDiagramModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    theme: &ZenumlSourceStyle,
    options: &SvgExecution<'_>,
) -> Result<RootedSvg> {
    let diagram_id = options.diagram_id_or("zenuml");
    render_svg(
        layout,
        model,
        effective_config,
        diagram_title,
        theme,
        &ZenumlSvgContext {
            diagram_id,
            viewport: options.root_viewport(DiagramFamilyId::ZENUML, diagram_id),
            work_meter: options.work_meter(),
        },
    )
}
