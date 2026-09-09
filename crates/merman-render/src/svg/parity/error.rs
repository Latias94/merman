mod contract;
mod render;

pub(super) use contract::{
    ErrorProjection, canonical_error_css, validate_error_projection_contract, write_error_path,
    write_error_text,
};
pub(super) use render::render_error_diagram_svg_model;
