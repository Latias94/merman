use merman_model::{OperationPhase, runtime::OperationContext};
use merman_render_core::{
    Result, resources::OperationWorkMeter, root_svg::RootViewportContext, svg_css::SvgId,
    text::TextMeasurer,
};

mod native;
mod palette;
mod render;

pub use render::render_block_diagram_svg_model_with_theme;

/// Borrowed operation services for the complete Block SVG writer.
/// The viewport and diagram ID share the caller's projection, and the layout, labels,
/// node bindings and optional routes belong to the same prepared Block occurrence order.
pub struct BlockSvgContext<'a> {
    pub diagram_id: SvgId<'a>,
    pub viewport: RootViewportContext<'a>,
    pub work_meter: &'a OperationWorkMeter,
    pub text_measurer: &'a dyn TextMeasurer,
    pub operation: &'a OperationContext,
    pub shoal: bool,
}

impl BlockSvgContext<'_> {
    fn checkpoint_emit(&self) -> Result<()> {
        self.work_meter
            .checkpoint(OperationPhase::Emit)
            .map_err(Into::into)
    }
}
