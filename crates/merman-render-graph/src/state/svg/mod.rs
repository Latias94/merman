//! Complete, bounded Mermaid-compatible State SVG emission.

use std::fmt::Write as _;

use merman_model::runtime::OperationContext;
#[cfg(feature = "layout-shoal")]
use merman_render_core::route_paint::CanonicalPaintScratch;
use merman_render_core::{
    Bounds, LayoutCluster, LayoutEdge, LayoutNode, LayoutPoint, Result,
    html::html_label_overflow_attr,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg,
    rough_svg::{
        RoughRectSpec as StateRoughRectSpec, roughjs_circle_path_d, roughjs_paths_for_rect,
    },
    svg_css::{CssSelectorDiagramId, SvgDiagramIdValue, SvgId as SvgDiagramId, scoped_drop_shadow},
    svg_output::{BoundedSvgOutput, SvgOutput, timing},
    text::{TextMeasurer, WrapMode},
    xml::{escape_attr, escape_attr_display, escape_xml, escape_xml_display},
};
use merman_theme_contract::v2::{Elevation, FamilyTheme};
use rustc_hash::FxHashMap;

use super::{
    StateDiagramLayout, state_edge_label_xhtml as state_edge_label_html,
    state_measured_svg_text_label, state_native_svg_text_label, state_node_label_html_with_style,
    state_node_label_plain_html, state_node_label_text, state_svg_text_label,
    state_value_to_label_text,
};
use merman_render_core::number_format::canonical_number as fmt_display;

mod composite_paint;
mod context;
mod edge;
mod effect;
mod node;
mod rough_cache;
mod roughjs;
mod style;
mod viewport;

use roughjs::{
    mermaid_choice_diamond_path_data, mermaid_rounded_rect_path_data, roughjs_paths_for_svg_path,
};

#[cfg(feature = "layout-shoal")]
pub use style::state_canonical_marker;

use context::*;
use edge::*;
use effect::*;
use node::*;
use rough_cache::*;
use style::*;
use viewport::*;

type StateSvgModel = merman_model::families::state::StateDiagramRenderModel;
type StateSvgState = merman_model::families::state::StateRecord;
type StateSvgLink = merman_model::families::state::StateLink;
type StateSvgLinks = merman_model::families::state::StateLinks;
type StateSvgNode = merman_model::families::state::StateDiagramRenderNode;
type StateSvgEdge = merman_model::families::state::StateDiagramRenderEdge;

/// One existing canonical edge projection, rendered only at its original emit site.
#[cfg(feature = "layout-shoal")]
pub struct StateCanonicalEdge<'points> {
    pub d: String,
    pub bounds: Bounds,
    pub data_points: &'points [LayoutPoint],
    pub marker_points: [LayoutPoint; 4],
    pub end_marker: merman_render_core::route_types::Marker,
    pub rough_d: Option<String>,
}

#[cfg(feature = "layout-shoal")]
pub type StateCanonicalEdgePainter<'a> = dyn for<'points> Fn(
        &LayoutEdge,
        kurbo::Vec2,
        f64,
        Option<&roughr::core::RoughRandomness>,
        &'points mut CanonicalPaintScratch,
    ) -> Result<StateCanonicalEdge<'points>>
    + 'a;

/// Host-owned ID, viewport policy, and operation controls used by the complete writer.
pub struct StateSvgContext<'a> {
    pub diagram_id: SvgDiagramId<'a>,
    pub viewport: root_svg::RootViewportContext<'a>,
    pub elk_parity: bool,
    #[cfg(feature = "layout-shoal")]
    pub canonical_routes: Option<&'a StateCanonicalEdgePainter<'a>>,
    pub work_meter: &'a OperationWorkMeter,
    pub operation: &'a OperationContext,
    pub render_seed: u64,
    pub timing: timing::RenderTiming,
    pub include_edges: bool,
    pub include_nodes: bool,
    pub native_theme: Option<&'a FamilyTheme>,
}

struct StateRenderCtx<'a> {
    work_meter: &'a OperationWorkMeter,
    diagram_id: SvgDiagramId<'a>,
    /// The normalized look used for renderer behavior (`default` behaves as `classic`).
    diagram_look: String,
    serialized_diagram_look: String,
    palette_size: usize,
    uses_elk_adapter_dom: bool,
    elk_edge_paths: &'a std::collections::HashMap<String, Vec<LayoutPoint>>,
    elk_line_hop_paths: FxHashMap<String, String>,
    hand_drawn_seed: roughr::core::RoughRandomness,
    html_labels: bool,
    html_label_wrapping_width: f64,
    label_min_width: f64,
    state_padding: f64,
    node_order: Vec<&'a str>,
    nodes_by_id: FxHashMap<&'a str, &'a StateSvgNode>,
    layout_nodes_by_id: FxHashMap<&'a str, &'a LayoutNode>,
    layout_edges_by_id: FxHashMap<&'a str, &'a LayoutEdge>,
    layout_clusters_by_id: FxHashMap<&'a str, &'a LayoutCluster>,
    composite_paints: composite_paint::StateCompositePaints<'a>,
    #[cfg(feature = "layout-shoal")]
    canonical_routes: Option<&'a StateCanonicalEdgePainter<'a>>,
    parent: FxHashMap<&'a str, &'a str>,
    nested_roots: std::collections::BTreeSet<String>,
    hidden_prefixes: Vec<String>,
    security_level_loose: bool,
    links: &'a std::collections::HashMap<String, StateSvgLinks>,
    states: &'a std::collections::HashMap<String, StateSvgState>,
    edges: &'a [StateSvgEdge],
    include_edges: bool,
    include_nodes: bool,
    has_node_shadows: bool,
    native_elevation: Option<&'a Elevation>,
    measurer: &'a dyn TextMeasurer,
    label_sidecar: &'a crate::state::StateLabelSidecar,
    style_plan: &'a crate::state::StateStylePlan,
    rough_cache: StateRoughCache,
}

impl StateRenderCtx<'_> {
    fn uses_canonical_routes(&self) -> bool {
        #[cfg(feature = "layout-shoal")]
        {
            self.canonical_routes.is_some()
        }
        #[cfg(not(feature = "layout-shoal"))]
        {
            false
        }
    }
}

mod render;
pub use render::render_state_diagram_svg_model;
