//! State diagram configuration, measurement, Dagre layout, styling, and SVG emission.

type StateNode = merman_model::families::state::StateDiagramRenderNode;

mod compatibility;
mod config;
#[cfg(feature = "layout-dagre")]
mod dagre;
#[cfg(feature = "layout-elk")]
mod elk;
#[cfg(any(feature = "layout-grid", feature = "layout-layered"))]
mod flat_input;
mod geometry;
#[cfg(feature = "layout-grid")]
mod grid;
#[cfg(feature = "layout-layered")]
mod layered;
mod layout_input;
mod measure;
mod style_plan;
mod svg;

mod label;
mod label_artifact;
mod label_geometry;
mod label_svg;

pub use compatibility::StateCompatibilityPlan;
use compatibility::{StateNodeKind, StateTerminalPaintProperty, StateTerminalSurface};
pub use config::{
    StateConfigView, StateLayoutSettings, normalize_dir, state_node_is_effective_group,
};
#[cfg(feature = "layout-dagre")]
pub use dagre::{
    debug_build_state_diagram_dagre_graph, debug_extract_state_diagram_cluster_graph,
    layout as layout_dagre,
};
#[cfg(feature = "layout-elk")]
pub use elk::layout as layout_elk;
use geometry::RectWithTitleGeometry;
pub use geometry::StateDiagramLayout;
#[cfg(feature = "layout-grid")]
pub use grid::layout as layout_grid;
#[cfg(feature = "layout-layered")]
pub use layered::{StateLayeredError, StateLayeredPaint, layout as layout_layered};
#[cfg(feature = "layout-shoal")]
pub use layout_input::note_group_container;
pub use layout_input::{
    HiddenPrefixMatcher, note_group_owner_id, state_hidden_prefixes, state_layout_adapter_work,
    validate_state_parent_cycles,
};
pub use measure::{
    edge_label_metrics, state_fork_join_painted_dimensions, state_node_dimensions,
    title_label_metrics,
};
use style_plan::StateCssDeclaration;
pub use style_plan::{ResolvedLabelTypography, StateNodeStylePlan, StateStylePlan};
#[cfg(feature = "layout-shoal")]
pub use svg::{StateCanonicalEdge, StateCanonicalEdgePainter, state_canonical_marker};
pub use svg::{StateSvgContext, render_state_diagram_svg_model};

pub use label::{
    StateLabelMeasurement, measure_state_markdown_label, state_edge_label_xhtml,
    state_value_to_label_text,
};
pub use label_artifact::{
    StateLabelMetricsRequest, StateLabelOwner, StateLabelSidecar, StateLabelSidecarBuilder,
    StateLabelSourceKind,
};
pub use label_geometry::StateNativeLabelGeometry;
pub use label_svg::{
    state_measured_svg_text_label, state_native_svg_text_label, state_node_label_html_with_style,
    state_node_label_plain_html, state_node_label_text, state_svg_text_label,
};
