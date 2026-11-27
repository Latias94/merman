//! ER configuration, style preparation, and SVG geometry, markers, and stylesheets.

mod binding;
mod config;
mod geometry;
mod markers;
mod native_theme;
mod source_style;
mod stylesheet;

pub use binding::ErCssBinding;
pub use config::{ErConfigView, ErEntityMeasurementSettings, ErLayoutSettings, ErRenderSettings};
pub use geometry::{ErSvgGeometry, ErSvgLayoutView, ErSvgRouteKind, er_rel_idx_from_edge_id};
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
