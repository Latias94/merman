mod admission;
mod execute;
#[cfg(feature = "icons")]
mod icons;
mod prepare;
#[cfg(feature = "svg")]
mod svg_pipeline;

pub(crate) use execute::execute_render;
#[cfg(feature = "icons")]
pub(crate) use icons::{resolve_local_icon_paths, validate_icon_source_count};
#[cfg(feature = "markdown")]
pub(crate) use prepare::prepare_render_for_batch;
#[cfg(feature = "svg")]
pub(crate) use prepare::prepare_render_for_mmdc;
pub(crate) use prepare::prepare_render_for_native;
