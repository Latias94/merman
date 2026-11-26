use merman_model::{
    families::sequence::SequenceDiagramModel, family::DiagramFamilyId,
    presentation::PresentationConfig,
};
use merman_render_sequence::{
    preparation::SequencePreparedArtifact,
    svg::{SequenceSvgRequest, render_parity},
};

use super::SvgExecution;
use crate::{Result, text::TextMeasurer};

pub(super) fn render_sequence_diagram_svg_model_with_config(
    prepared: &SequencePreparedArtifact,
    model: &SequenceDiagramModel,
    config: &PresentationConfig,
    measurer: &dyn TextMeasurer,
    execution: &SvgExecution<'_>,
) -> Result<merman_render_core::root_svg::RootedSvg> {
    render_parity(svg_request(prepared, model, config, measurer, execution))
}

fn svg_request<'a>(
    prepared: &'a SequencePreparedArtifact,
    semantic: &'a SequenceDiagramModel,
    config: &'a PresentationConfig,
    measurer: &'a dyn TextMeasurer,
    execution: &'a SvgExecution<'_>,
) -> SequenceSvgRequest<'a> {
    let session = execution.family.session();
    let diagram_id = execution.diagram_id_or("merman");
    SequenceSvgRequest {
        prepared,
        semantic,
        config,
        measurer,
        diagram_id,
        viewport: execution.root_viewport(DiagramFamilyId::SEQUENCE, diagram_id),
        work_meter: session.work_meter(),
        canvas_background: session.canvas_background(),
    }
}

#[cfg(feature = "native-compact")]
pub(super) mod compact {
    use merman_render_sequence::svg::{SequenceCompactPolicy, render_compact};

    pub(in crate::svg::parity) use merman_render_sequence::svg::SequenceCompactError;

    use super::*;

    pub(in crate::svg::parity) fn render_sequence_compact(
        prepared: &SequencePreparedArtifact,
        model: &SequenceDiagramModel,
        config: &PresentationConfig,
        execution: &SvgExecution<'_>,
    ) -> std::result::Result<String, SequenceCompactError> {
        let session = execution.family.session();
        let native = session.native_theme().map(|theme| theme.resolved());
        render_compact(
            &svg_request(
                prepared,
                model,
                config,
                execution.text_measurer(),
                execution,
            ),
            SequenceCompactPolicy {
                canvas: native.map(|theme| &theme.family("sequence").roles.canvas),
                textures: native.map_or(&[], |theme| theme.canvas().textures.as_slice()),
                allows_trusted_theme_css: session.allows_trusted_theme_css(),
            },
        )
    }
}
