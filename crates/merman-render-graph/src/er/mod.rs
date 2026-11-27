//! ER configuration, prepared labels and styles, Dagre and ELK layout, and complete SVG emission.

mod binding;
mod config;
#[cfg(feature = "layout-dagre")]
mod dagre;
#[cfg(feature = "layout-elk")]
mod elk;
mod geometry;
mod labels;
mod markers;
mod native_theme;
mod source_style;
mod stylesheet;
mod svg;
mod swimlane;
mod theme;

pub use binding::ErCssBinding;
pub use config::{ErConfigView, ErEntityMeasurementSettings, ErLayoutSettings, ErRenderSettings};
#[cfg(feature = "layout-dagre")]
pub use dagre::{ErDagreLayout, layout as layout_dagre};
#[cfg(feature = "layout-elk")]
pub use elk::{ErElkLayout, layout as layout_elk};
#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
use geometry::calc_label_position;
pub use geometry::{
    ErSvgGeometry, ErSvgLayoutView, ErSvgRouteKind, er_layout_bounds, er_rel_idx_from_edge_id,
};
pub use labels::{
    ErBoxLabel, ErPreparedLabels, ErPreparedRelationshipLabel, er_subgraph_title_metrics,
    er_swimlane_title_metrics,
};
pub use markers::{ErMarkerDefs, er_marker_id, er_unified_marker_id, write_er_marker_definitions};
#[cfg(feature = "layout-shoal")]
pub use markers::{er_canonical_marker, er_marker_paint_bounds};
pub use native_theme::ErNativeStyle;
pub use source_style::{
    ErEntitySourceStyle, ErRowVisual, ErSourceVisual, ErSubgraphSourceStyles,
    compile_er_entity_source_style, prepare_er_source_visuals, prepare_er_subgraph_source_styles,
};
pub use stylesheet::{
    er_color_indices, er_css_with_resolved_typography, er_redux_color_id, write_er_stylesheet,
};
#[cfg(feature = "layout-shoal")]
pub use svg::{ErCanonicalEdge, ErCanonicalEdgePainter};
pub use svg::{ErPreparedSvgLayout, ErSvgContext, render_er_diagram_svg_model};
pub use swimlane::{
    ErSwimlanePaintLayout, relationship_html_source as swimlane_relationship_html_source,
};
pub use theme::ErEntityThemePlan;
