//! State diagram configuration, measurement, styling, and SVG emission.

type StateNode = merman_model::families::state::StateDiagramRenderNode;

mod compatibility;
mod config;
mod geometry;
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
use geometry::RectWithTitleGeometry;
pub use geometry::StateDiagramLayout;
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
