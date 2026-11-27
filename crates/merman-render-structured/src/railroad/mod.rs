//! Railroad layout, presentation binding, and SVG emission.

use merman_model::{families::railroad::RailroadDiagramModel, presentation::PresentationConfig};
use merman_render_core::{Result, text::TextMeasurer};
use merman_theme_contract::v2::FamilyTheme;

mod layout;
mod model;
mod style;
mod svg;

pub use model::{
    RailroadDiagramLayout, RailroadElementLayout, RailroadPathLayout, RailroadRuleLayout,
};
pub use style::RailroadStyle;
pub use svg::render_railroad_svg;

/// Binds the selected Railroad style before measuring and laying out the rules.
pub fn prepare_railroad_diagram(
    model: &RailroadDiagramModel,
    diagram_type: &str,
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    measurer: &dyn TextMeasurer,
) -> Result<(RailroadDiagramLayout, RailroadStyle)> {
    let style = style::resolve_railroad_style(config, native);
    let layout = layout::layout_railroad_diagram_typed(model, diagram_type, &style, measurer)?;
    Ok((layout, style))
}
