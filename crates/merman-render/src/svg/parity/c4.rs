mod render;
#[cfg(feature = "layout-shoal")]
mod shoal;
#[cfg(all(test, feature = "layout-shoal"))]
mod tests;

pub(super) use render::render_c4_diagram_svg_typed;
