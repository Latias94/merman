mod render;

#[cfg(feature = "layout-shoal")]
pub(crate) use merman_render_graph::er::er_canonical_marker;
pub(super) use render::render_er_diagram_svg_model;
