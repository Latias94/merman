use merman_render_core::{
    Error, Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg::{DiagramBounds, RootChrome, RootViewportContext, RootViewportSpec, RootedSvg},
    svg_css::SvgId,
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    theme_svg::write_color,
    xml::{escape_attr, escape_xml},
};

use super::{
    INFO_BASELINE_HEIGHT_PX, InfoAppearance, InfoClassicStyle, InfoDiagramLayout, InfoNativeStyle,
};

/// Emits the complete Info SVG, before host canvas and terminal SVG admission.
pub fn render_info_svg(
    layout: &InfoDiagramLayout,
    appearance: &InfoAppearance,
    diagram_id: SvgId<'_>,
    root_viewport: &RootViewportContext<'_>,
    work_meter: &OperationWorkMeter,
) -> Result<RootedSvg> {
    match appearance {
        InfoAppearance::Mermaid(style) => {
            render_classic_info_svg(layout, style, diagram_id, root_viewport, work_meter)
        }
        InfoAppearance::Native(style) => {
            render_native_info_svg(layout, style, diagram_id, root_viewport, work_meter)
        }
    }
}

fn render_native_info_svg(
    layout: &InfoDiagramLayout,
    style: &InfoNativeStyle,
    diagram_id: SvgId<'_>,
    root_viewport: &RootViewportContext<'_>,
    work_meter: &OperationWorkMeter,
) -> Result<RootedSvg> {
    let bounds = layout.bounds.as_ref().ok_or_else(|| Error::InvalidModel {
        message: "Info layout is missing its prepared viewport bounds".to_owned(),
    })?;
    let viewport = RootViewportSpec::responsive(DiagramBounds::from_extents(
        bounds.min_x,
        bounds.min_y,
        bounds.max_x,
        bounds.max_y,
        0.0,
    ));
    let mut out = BoundedSvgOutput::new(work_meter);
    let root = root_viewport.write_open(&mut out, viewport, RootChrome::new(diagram_id, "info"))?;
    style.write_svg_tail(&mut out, layout);
    root.complete(out.finish()?)
}

fn render_classic_info_svg(
    layout: &InfoDiagramLayout,
    style: &InfoClassicStyle,
    diagram_id: SvgId<'_>,
    root_viewport: &RootViewportContext<'_>,
    work_meter: &OperationWorkMeter,
) -> Result<RootedSvg> {
    let mut out = BoundedSvgOutput::new(work_meter);
    let bounds = layout.bounds.as_ref().ok_or_else(|| Error::InvalidModel {
        message: "Info layout is missing its prepared viewport bounds".to_string(),
    })?;
    let mut root_spec =
        RootViewportSpec::responsive_without_view_box((bounds.max_x - bounds.min_x).max(0.0));
    let height = (bounds.max_y - bounds.min_y).max(0.0);
    if height > INFO_BASELINE_HEIGHT_PX {
        root_spec = root_spec.with_mermaid_responsive_height(true, height);
    }
    let mut root_chrome = RootChrome::new(diagram_id, "info");
    root_chrome.dom.trailing_newline = false;
    let root_document = root_viewport.write_open(&mut out, root_spec, root_chrome)?;
    style.write_svg_contents(&mut out, layout, diagram_id.semantic_str())?;
    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
}

impl InfoClassicStyle {
    fn write_svg_contents(
        &self,
        out: &mut impl SvgOutput,
        layout: &InfoDiagramLayout,
        diagram_id: &str,
    ) -> Result<()> {
        out.push_str("<style>");
        self.css.write(&mut XmlTextOutput::new(out), diagram_id)?;
        out.push_str("</style>");
        out.checkpoint()?;
        out.push_str("<g/>");
        out.checkpoint()?;
        let version = &self.version;
        let _ = write!(
            out,
            r#"<g><text x="{}" y="{}" class="version" font-size="{}" style="text-anchor: middle;">{}</text></g>"#,
            fmt(version.x),
            fmt(version.y),
            fmt(version.font_size_px),
            escape_xml(&layout.version)
        );
        out.checkpoint()
    }
}

impl InfoNativeStyle {
    fn write_svg_tail(&self, out: &mut impl SvgOutput, layout: &InfoDiagramLayout) {
        let version = &self.version;
        let _ = write!(
            out,
            r#"<g><text x="{}" y="{}" class="version" font-size="{}" font-family="{}" "#,
            fmt(version.x),
            fmt(version.y),
            fmt(version.font_size_px),
            escape_attr(&self.font_family),
        );
        let _ = write_color(out, "fill", self.text_color, 1.0);
        // Keep these closing tags in one write for the existing output admission boundary.
        let _ = write!(
            out,
            r#" text-anchor="middle">{}</text></g></svg>"#,
            escape_xml(&layout.version)
        );
    }
}
