//! Complete SVG emission from the paired Architecture semantic and layout artifacts.

use merman_render_core::icon_registry::IconRegistry;
use merman_render_core::{
    Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg::RootViewportContext,
    svg_css::SvgId as SvgDiagramId,
    svg_output::{SvgOutput, timing::RenderTiming},
    text::TextMeasurer,
    xml::{escape_attr_into, escape_xml_into},
};

#[cfg(feature = "layout-shoal")]
mod canonical;
mod edges;
#[cfg(test)]
mod foreign_object;
mod geometry;
mod icons;
mod labels;
mod nodes;
mod render;
mod root;
mod settings;
mod viewport;

#[cfg(feature = "layout-shoal")]
pub use canonical::{
    ArchitectureCanonicalEdge, ArchitectureCanonicalGroup, ArchitectureCanonicalSvg,
    ArchitecturePlacedText,
};
pub use render::render_architecture_diagram_svg_typed_with_config;

/// Host-owned context for a single Architecture render. The viewport and diagram ID share the
/// same operation projection.
pub struct ArchitectureSvgContext<'a> {
    pub diagram_id: SvgDiagramId<'a>,
    pub viewport: RootViewportContext<'a>,
    pub work_meter: &'a OperationWorkMeter,
    pub text_measurer: &'a dyn TextMeasurer,
    pub icon_registry: Option<&'a IconRegistry>,
    pub timing: RenderTiming,
    #[cfg(feature = "layout-shoal")]
    pub operation: &'a merman_model::runtime::OperationContext,
    #[cfg(feature = "layout-shoal")]
    pub canvas_background: Option<merman_theme_contract::v2::Color>,
    #[cfg(feature = "layout-shoal")]
    pub canonical: Option<&'a dyn ArchitectureCanonicalSvg>,
}

/// Non-billing terminal replay shared by Architecture SVG emission passes.
#[derive(Clone, Copy)]
struct ArchitectureEmitCheckpoints<'a> {
    work_meter: &'a OperationWorkMeter,
}

impl<'a> ArchitectureEmitCheckpoints<'a> {
    const fn new(work_meter: &'a OperationWorkMeter) -> Self {
        Self { work_meter }
    }

    fn checkpoint(self) -> Result<()> {
        self.work_meter
            .checkpoint(merman_model::OperationPhase::Emit)
            .map_err(Into::into)
    }
}

fn fmt_string(v: f64) -> String {
    let mut out = String::new();
    fmt_into(&mut out, v);
    out
}

fn fmt_into(out: &mut impl SvgOutput, v: f64) {
    let _ = write!(out, "{}", fmt(v));
}

#[cfg(feature = "layout-shoal")]
fn write_geometry_attrs(out: &mut impl SvgOutput, kind: &str, id: &str) {
    let _ = write!(out, r#" data-et="{kind}" data-id=""#);
    escape_attr_into(out, id);
    out.push('"');
}
