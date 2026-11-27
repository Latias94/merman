//! Resource and CSS checks shared by SVG postprocessing and terminal validation.

mod attr;
mod css;
mod raster_url;

pub use attr::{
    is_mermaid_missing_amount_hsl, is_safe_data_image_url, matches_active_svg_element,
    parsed_attribute_violates_resvg_contract, sanitize_element_attributes_cow_with_checkpoints,
};
pub use css::{
    CssValidationError, matches_external_image_function, sanitize_style_elements,
    validate_resvg_css_declaration_list_with_checkpoints,
    validate_resvg_css_stylesheet_with_checkpoints,
};
pub use raster_url::{inline_raster_url, is_data_url};
