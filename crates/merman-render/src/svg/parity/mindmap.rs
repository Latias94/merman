use merman_model::{family::DiagramFamilyId, presentation::PresentationConfig};
use merman_render_core::root_svg::RootedSvg;
use merman_render_graph::mindmap::{MindmapSvgContext, render_mindmap_diagram_svg};

use super::SvgExecution;
use crate::{Result, family::MindmapFamilyArtifact};

pub(super) fn render_mindmap(
    artifact: &MindmapFamilyArtifact<'_>,
    config: &PresentationConfig,
    options: &SvgExecution<'_>,
) -> Result<RootedSvg> {
    let diagram_id = options.diagram_id_or("mindmap");
    render_mindmap_diagram_svg(
        artifact.semantic(),
        config,
        artifact.layout(),
        MindmapSvgContext {
            diagram_id,
            viewport: options.root_viewport(DiagramFamilyId::MINDMAP, diagram_id),
            text_measurer: options.text_measurer(),
            math_renderer: options.math_renderer(),
            work_meter: options.work_meter(),
            timing: options.timing(),
        },
    )
}
