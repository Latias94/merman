#![allow(clippy::too_many_arguments)]

use super::*;
use rustc_hash::FxHashMap;

// Class diagram SVG renderer implementation (split from parity.rs).

type Rect = merman_core::geom::Box2;

mod debug_svg;
pub(super) fn render_class_diagram_v2_debug_svg(
    layout: &ClassDiagramV2Layout,
    options: &SvgRenderOptions,
) -> String {
    debug_svg::render_class_diagram_v2_debug_svg(layout, options)
}

mod defs;
use defs::{class_marker_name, class_markers};

mod label;
use label::{class_apply_inline_styles, render_class_html_label};

mod rough;
use rough::{
    class_rough_line_double_path_and_bounds, class_rough_rect_stroke_path_and_bounds,
    class_rough_seed,
};

type ClassSvgModel = merman_core::models::class_diagram::ClassDiagram;
type ClassSvgNode = merman_core::models::class_diagram::ClassNode;
type ClassSvgRelation = merman_core::models::class_diagram::ClassRelation;
type ClassSvgNote = merman_core::models::class_diagram::ClassNote;
type ClassSvgInterface = merman_core::models::class_diagram::ClassInterface;

fn class_edge_dom_id(
    edge: &crate::model::LayoutEdge,
    relation_index_by_id: &FxHashMap<&str, usize>,
) -> String {
    let mut out = String::new();
    class_edge_dom_id_into(&mut out, edge, relation_index_by_id);
    out
}

fn class_edge_dom_id_into(
    out: &mut String,
    edge: &crate::model::LayoutEdge,
    relation_index_by_id: &FxHashMap<&str, usize>,
) {
    out.clear();
    if edge.id.starts_with("edgeNote") {
        // Mermaid numbers note edges as `edgeNote<N>` where `<N>` follows the `note<N-1>` id.
        // (This is independent from the relation edge counter.)
        if let Some(note_idx) = edge
            .from
            .strip_prefix("note")
            .and_then(|rest| rest.parse::<usize>().ok())
        {
            let _ = write!(out, "edgeNote{}", note_idx + 1);
            return;
        }
        out.push_str(edge.id.as_str());
        return;
    }
    // Mermaid uses `getEdgeId` with prefix `id`.
    let idx = relation_index_by_id
        .get(edge.id.as_str())
        .copied()
        .unwrap_or(1);
    out.push_str("id_");
    out.push_str(edge.from.as_str());
    out.push('_');
    out.push_str(edge.to.as_str());
    out.push('_');
    let _ = write!(out, "{idx}");
}

fn class_edge_pattern(line_type: i32) -> &'static str {
    // Mermaid class diagram `lineType` uses "dottedLine" for `..` which maps to the dashed pattern.
    if line_type == 1 {
        "edge-pattern-dashed"
    } else {
        "edge-pattern-solid"
    }
}

fn class_note_edge_pattern() -> &'static str {
    "edge-pattern-dotted"
}

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
