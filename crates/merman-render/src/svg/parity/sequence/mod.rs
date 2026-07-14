use super::*;

mod activation;
mod actor_man;
mod actor_man_glyphs;
mod actor_popup;
mod actor_shapes;
mod actors;
mod block_collection;
mod block_geometry;
mod block_text;
mod blocks;
mod css;
mod frames;
mod geometry;
mod interactions;
mod math_label;
mod messages;
mod model;
mod notes;
mod render;
mod root;
mod settings;

pub(super) fn render_sequence_diagram_svg_model_with_config(
    layout: &SequenceDiagramLayout,
    model: &merman_core::diagrams::sequence::SequenceDiagramRenderModel,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<String> {
    render::render_sequence_diagram_svg_model_with_config(
        layout,
        model,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}
