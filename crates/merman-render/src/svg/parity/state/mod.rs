#![allow(clippy::too_many_arguments)]

use super::*;
use rustc_hash::FxHashMap;
use std::sync::Arc;
mod context;
mod debug_svg;
mod emitted_bounds;
mod links;
mod rough_cache;
pub(in crate::svg::parity) mod roughjs;
mod style;
mod viewport;

pub use emitted_bounds::{
    SvgEmittedBoundsContributor, SvgEmittedBoundsDebug, debug_svg_emitted_bounds,
};
pub(super) use emitted_bounds::{svg_emitted_bounds_from_svg, svg_emitted_bounds_from_svg_inner};
pub(super) use roughjs::{
    roughjs_ops_to_svg_path_d, roughjs_parse_hex_color_to_srgba, roughjs_paths_for_rect,
};

use roughjs::{
    mermaid_choice_diamond_path_data, mermaid_rounded_rect_path_data, roughjs_circle_path_d,
    roughjs_paths_for_svg_path,
};

// State diagram SVG renderer implementation (split from parity.rs).

use context::*;
use links::*;
use rough_cache::*;
use style::*;
use viewport::*;

type StateSvgModel = merman_core::diagrams::state::StateDiagramRenderModel;
type StateSvgStyleClass = merman_core::diagrams::state::StateDiagramRenderStyleClass;
type StateSvgState = merman_core::diagrams::state::StateDiagramRenderState;
type StateSvgNote = merman_core::diagrams::state::StateDiagramRenderNote;
type StateSvgLink = merman_core::diagrams::state::StateDiagramRenderLink;
type StateSvgLinks = merman_core::diagrams::state::StateDiagramRenderLinks;
type StateSvgNode = merman_core::diagrams::state::StateDiagramRenderNode;
type StateSvgEdge = merman_core::diagrams::state::StateDiagramRenderEdge;

struct StateRenderCtx<'a> {
    diagram_id: String,
    #[allow(dead_code)]
    diagram_title: Option<String>,
    diagram_look: String,
    hand_drawn_seed: u64,
    html_label_wrapping_width: f64,
    state_padding: f64,
    node_order: Vec<&'a str>,
    nodes_by_id: FxHashMap<&'a str, &'a StateSvgNode>,
    layout_nodes_by_id: FxHashMap<&'a str, &'a LayoutNode>,
    layout_edges_by_id: FxHashMap<&'a str, &'a crate::model::LayoutEdge>,
    layout_clusters_by_id: FxHashMap<&'a str, &'a LayoutCluster>,
    parent: FxHashMap<&'a str, &'a str>,
    nested_roots: std::collections::BTreeSet<String>,
    hidden_prefixes: Vec<String>,
    security_level_loose: bool,
    links: &'a std::collections::HashMap<String, StateSvgLinks>,
    states: &'a std::collections::HashMap<String, StateSvgState>,
    edges: &'a [StateSvgEdge],
    include_edges: bool,
    include_nodes: bool,
    measurer: &'a dyn TextMeasurer,
    text_style: crate::text::TextStyle,
    rough_circle_cache: std::cell::RefCell<FxHashMap<StateRoughCacheKey, Arc<String>>>,
    rough_paths_cache:
        std::cell::RefCell<FxHashMap<StateRoughCacheKey, (Arc<String>, Arc<String>)>>,
}

mod render;

pub(super) fn render_state_diagram_v2_svg(
    layout: &StateDiagramV2Layout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    render::render_state_diagram_v2_svg_impl(
        layout,
        semantic,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub(super) fn render_state_diagram_v2_svg_model(
    layout: &StateDiagramV2Layout,
    model: &StateSvgModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    render::render_state_diagram_v2_svg_model_impl(
        layout,
        model,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

pub(super) fn render_state_diagram_v2_debug_svg(
    layout: &StateDiagramV2Layout,
    options: &SvgRenderOptions,
) -> String {
    debug_svg::render_state_diagram_v2_debug_svg(layout, options)
}
