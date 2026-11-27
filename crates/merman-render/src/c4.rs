#[cfg(feature = "layout-shoal")]
mod shoal;

#[cfg(feature = "layout-shoal")]
pub(crate) use merman_render_graph::c4::{
    C4_FRAMED_FRAME_WIDTH, C4_ROUNDED_RADIUS, C4ConfigView, C4Look, C4Measurement, C4NodeShape,
    C4PersonOutline, c4_cap_radius, c4_node_shape, c4_relation_markers, measure_c4_relation,
    measure_c4_shape, prepare_c4_boundary_layout, title_ink,
};
pub(crate) use merman_render_graph::c4::{
    C4ClusterThemePlan, C4TextPaintPlan, C4TypographyThemePlan, layout_c4_diagram_typed,
};
#[cfg(feature = "layout-shoal")]
pub(crate) use shoal::{C4ShoalGeometry, layout_c4_shoal};
