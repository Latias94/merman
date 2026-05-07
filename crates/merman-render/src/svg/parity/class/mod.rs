#![allow(clippy::too_many_arguments)]

use super::*;

// Class diagram SVG renderer implementation (split from parity.rs).

type Rect = merman_core::geom::Box2;

mod bounds;

mod debug_svg;
pub(super) fn render_class_diagram_v2_debug_svg(
    layout: &ClassDiagramV2Layout,
    options: &SvgRenderOptions,
) -> String {
    debug_svg::render_class_diagram_v2_debug_svg(layout, options)
}

mod defs;
use defs::{class_marker_name, class_markers};

mod edge;

mod interface;

mod label;
use label::{
    class_apply_inline_styles, class_html_div_style, class_html_label_max_width_px,
    class_html_label_metrics, class_html_title_metrics, render_class_html_label,
};

mod namespace;

mod node;

mod note;

mod rough;
use rough::class_rough_line_double_path_and_bounds;

type ClassSvgModel = merman_core::models::class_diagram::ClassDiagram;
type ClassSvgNode = merman_core::models::class_diagram::ClassNode;
type ClassSvgRelation = merman_core::models::class_diagram::ClassRelation;
type ClassSvgNote = merman_core::models::class_diagram::ClassNote;
type ClassSvgInterface = merman_core::models::class_diagram::ClassInterface;

mod render;

pub(super) fn render_class_diagram_v2_svg(
    layout: &ClassDiagramV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    render::render_class_diagram_v2_svg_impl(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub(super) fn render_class_diagram_v2_svg_model(
    layout: &ClassDiagramV2Layout,
    model: &ClassSvgModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    render::render_class_diagram_v2_svg_model_impl(
        layout,
        model,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}
