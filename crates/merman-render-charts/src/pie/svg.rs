use merman_model::families::pie::PieDiagramModel;
use merman_render_core::{
    Result, number_format::canonical_number as fmt, svg_css::SvgId, svg_output::SvgOutput,
    xml::escape_xml,
};

use super::PieDiagramLayout;

mod mermaid;
mod native;

pub use mermaid::{write_mermaid_css_rules, write_mermaid_svg_body};
pub use native::{PieNativeSvgContext, write_native_svg_body};

/// Writes Pie's accessible title and description into an open SVG.
pub fn write_svg_accessibility(
    out: &mut impl SvgOutput,
    model: &PieDiagramModel,
    diagram_id: SvgId<'_>,
) -> Result<()> {
    if let Some(t) = model.acc_title.as_deref() {
        let _ = write!(
            out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id,
            text = escape_xml(t)
        );
    }
    if let Some(d) = model.acc_descr.as_deref() {
        let _ = write!(
            out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id,
            text = escape_xml(d)
        );
    }
    out.checkpoint()
}

fn write_pie_group_open(out: &mut impl SvgOutput, layout: &PieDiagramLayout, offset: (f64, f64)) {
    let _ = write!(
        out,
        r#"<g transform="translate({x},{y})">"#,
        x = fmt(layout.center_x),
        y = fmt(layout.center_y)
    );

    let (pie_offset_x, pie_offset_y) = offset;
    let has_pie_offset = pie_offset_x != 0.0 || pie_offset_y != 0.0;
    if has_pie_offset {
        let _ = write!(
            out,
            r#"<g transform="translate({x},{y})">"#,
            x = fmt(pie_offset_x),
            y = fmt(pie_offset_y)
        );
    } else {
        out.push_str("<g>");
    }
}
