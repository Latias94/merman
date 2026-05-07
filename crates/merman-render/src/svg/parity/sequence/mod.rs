#![allow(clippy::too_many_arguments)]

use super::*;

mod activation;
mod actors;
mod css;
mod debug;
mod frames;
mod geometry;
mod model;
mod render;

pub(super) fn render_sequence_diagram_debug_svg(
    layout: &SequenceDiagramLayout,
    options: &SvgRenderOptions,
) -> String {
    debug::render_sequence_diagram_debug_svg(layout, options)
}

pub(super) fn render_sequence_diagram_svg(
    layout: &SequenceDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    render::render_sequence_diagram_svg(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub(super) fn render_sequence_diagram_svg_with_config(
    layout: &SequenceDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    render::render_sequence_diagram_svg_with_config(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub(super) fn render_sequence_diagram_svg_model_with_config(
    layout: &SequenceDiagramLayout,
    model: &merman_core::diagrams::sequence::SequenceDiagramRenderModel,
    effective_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
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
