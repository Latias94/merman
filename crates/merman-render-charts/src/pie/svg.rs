use merman_model::{OperationPhase, families::pie::PieDiagramModel};
use merman_render_core::{
    Result,
    config::ConfigLookup,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg::{RootViewportContext, RootedSvg},
    svg_css::SvgId,
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    text::svg_title_bbox_vertical_extents_px,
    xml::escape_xml,
};
use merman_theme_contract::v2::{Appearance, Color};

use super::{PieAppearance, PieConfigView, PieDiagramLayout, PieThemePlan};

mod mermaid;
mod native;
mod root;
#[cfg(test)]
mod tests;

pub use mermaid::{write_mermaid_css_rules, write_mermaid_svg_body};
pub use native::{PieNativeSvgContext, write_native_svg_body};

/// Host scope and policy for the family document, before shared SVG finalization.
pub struct PieSvgContext<'a> {
    /// The normalized ID without occurrence projection, as used by classic Pie CSS.
    pub stylesheet_id: SvgId<'a>,
    /// The same ID with the operation's projection for root, ARIA and native references.
    pub diagram_id: SvgId<'a>,
    pub viewport: RootViewportContext<'a>,
    pub work_meter: &'a OperationWorkMeter,
    pub canvas_background: Option<Color>,
    pub appearance_and_backdrop: Option<(Appearance, Option<Color>)>,
}

/// Emits the complete Pie document from its paired layout and appearance.
pub fn render_pie_svg(
    layout: &PieDiagramLayout,
    model: &PieDiagramModel,
    appearance: &PieAppearance,
    effective_config: &impl ConfigLookup,
    context: &PieSvgContext<'_>,
) -> Result<RootedSvg> {
    let settings = PieConfigView::new(effective_config).render_settings();
    let mut out = BoundedSvgOutput::new(context.work_meter);
    let root_document = root::open_pie_svg(
        &mut out,
        model,
        context.diagram_id,
        root::pie_viewport(layout, model),
        settings.use_max_width,
        &context.viewport,
    )?;
    match appearance {
        PieAppearance::Mermaid(plan) => {
            out.push_str("<style>");
            out.checkpoint()?;
            write_pie_css(
                &mut XmlTextOutput::new(&mut out),
                context.stylesheet_id,
                plan,
            )?;
            out.push_str(r#"</style><g/>"#);
            out.checkpoint()?;

            write_mermaid_svg_body(
                &mut out,
                layout,
                model,
                plan.mermaid_style(),
                settings,
                || {
                    context
                        .work_meter
                        .checkpoint(OperationPhase::Emit)
                        .map_err(Into::into)
                },
            )?;
        }
        PieAppearance::Native(style) => {
            write_native_svg_body(
                &mut out,
                layout,
                model,
                style,
                settings,
                context.diagram_id,
                &PieNativeSvgContext {
                    title_vertical_extents: svg_title_bbox_vertical_extents_px,
                    canvas_background: context.canvas_background,
                    appearance_and_backdrop: context.appearance_and_backdrop,
                },
            )?;
        }
    }
    out.push_str("</g></svg>\n");
    root_document.complete(out.finish()?)
}

fn write_pie_css(
    out: &mut XmlTextOutput<'_, impl SvgOutput>,
    diagram_id: SvgId<'_>,
    paint_plan: &PieThemePlan,
) -> Result<()> {
    let common = paint_plan.common_css();
    common.write_prefix(out, diagram_id)?;
    write_mermaid_css_rules(
        out,
        diagram_id,
        paint_plan.mermaid_style().css_binding(),
        common.font_family(),
    )?;
    common.write_root(out, diagram_id, diagram_id)
}

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
