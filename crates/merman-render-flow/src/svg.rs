//! Operation services used by the Flow-family SVG renderer.

use merman_model::runtime::OperationContext;
use merman_render_core::{
    icon_registry::IconRegistry, math::MathRenderer, resources::OperationWorkMeter,
    root_svg::RootViewportContext, svg_css::SvgId,
    svg_edge_parity::trace::FlowchartEdgeTraceCollector, svg_output::timing::RenderTiming,
    text::TextMeasurer,
};
use merman_theme_contract::v2::{Canvas, Color, FamilyTheme};

mod layout;
pub use layout::{FlowchartSvgLayout, FlowchartSvgRoute, SwimlaneSvgLayout};

/// Services borrowed from the same admitted render operation as the layout and label sidecars.
pub struct FlowchartSvgServices<'a> {
    pub diagram_id: SvgId<'a>,
    pub root_viewport: RootViewportContext<'a>,
    pub operation: &'a OperationContext,
    pub work_meter: &'a OperationWorkMeter,
    pub render_seed: u64,
    pub timing: RenderTiming,
    pub measurer: &'a dyn TextMeasurer,
    pub math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    pub icon_registry: Option<&'a IconRegistry>,
    pub defer_palette_stylesheet: bool,
    pub native_canvas: Option<&'a Canvas>,
    pub canvas_background: Option<Color>,
    pub native_family_theme: Option<&'a FamilyTheme>,
    pub edge_trace: Option<(&'a str, &'a FlowchartEdgeTraceCollector)>,
}
