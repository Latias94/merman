use std::fmt::Write as _;

use merman_model::OperationPhase;

use super::{ErrorDiagramLayout, ErrorTypographyThemePlan};
use crate::{
    Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg::{
        DiagramBounds, RootChrome, RootMaxWidth, RootViewportContext, RootViewportSpec, RootedSvg,
        SvgRootStyleViewBoxOrder,
    },
    svg_css::SvgId,
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    xml::{escape_xml, escape_xml_serialized_text_into},
};

/// Emits the diagnostic SVG before the host canvas and terminal output checks.
pub fn render_error_diagram_svg(
    layout: &ErrorDiagramLayout,
    typography_theme: &ErrorTypographyThemePlan,
    diagram_id: SvgId<'_>,
    root_viewport: &RootViewportContext<'_>,
    work_meter: &OperationWorkMeter,
) -> Result<RootedSvg> {
    let mut out = BoundedSvgOutput::new(work_meter);
    let root_bounds =
        DiagramBounds::from_view_box(0.0, 0.0, layout.viewbox_width, layout.viewbox_height);
    let root_spec = RootViewportSpec::responsive(root_bounds)
        .with_max_width(RootMaxWidth::SvgNumber(layout.max_width_px));
    let mut root_chrome = RootChrome::new(diagram_id, "error");
    root_chrome.dom.style_viewbox_order = SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
    root_chrome.dom.trailing_newline = false;
    let root_document = root_viewport.write_open(&mut out, root_spec, root_chrome)?;
    out.push_str(r#"<style>"#);
    typography_theme
        .css_binding()
        .write(&mut XmlTextOutput::new(&mut out), diagram_id)?;
    out.push_str("</style>");
    out.checkpoint()?;

    out.push_str(r#"<g/>"#);
    work_meter.checkpoint(OperationPhase::Emit)?;
    out.push_str(r#"<g>"#);
    out.checkpoint()?;
    out.push_str(r#"<path class="error-icon" d="m411.313,123.313c6.25-6.25 6.25-16.375 0-22.625s-16.375-6.25-22.625,0l-32,32-9.375,9.375-20.688-20.688c-12.484-12.5-32.766-12.5-45.25,0l-16,16c-1.261,1.261-2.304,2.648-3.31,4.051-21.739-8.561-45.324-13.426-70.065-13.426-105.867,0-192,86.133-192,192s86.133,192 192,192 192-86.133 192-192c0-24.741-4.864-48.327-13.426-70.065 1.402-1.007 2.79-2.049 4.051-3.31l16-16c12.5-12.492 12.5-32.758 0-45.25l-20.688-20.688 9.375-9.375 32.001-31.999zm-219.313,100.687c-52.938,0-96,43.063-96,96 0,8.836-7.164,16-16,16s-16-7.164-16-16c0-70.578 57.422-128 128-128 8.836,0 16,7.164 16,16s-7.164,16-16,16z"/>"#);
    out.checkpoint()?;
    out.push_str(r#"<path class="error-icon" d="m459.02,148.98c-6.25-6.25-16.375-6.25-22.625,0s-6.25,16.375 0,22.625l16,16c3.125,3.125 7.219,4.688 11.313,4.688 4.094,0 8.188-1.563 11.313-4.688 6.25-6.25 6.25-16.375 0-22.625l-16.001-16z"/>"#);
    out.push_str(r#"<path class="error-icon" d="m340.395,75.605c3.125,3.125 7.219,4.688 11.313,4.688 4.094,0 8.188-1.563 11.313-4.688 6.25-6.25 6.25-16.375 0-22.625l-16-16c-6.25-6.25-16.375-6.25-22.625,0s-6.25,16.375 0,22.625l15.999,16z"/>"#);
    out.push_str(r#"<path class="error-icon" d="m400,64c8.844,0 16-7.164 16-16v-32c0-8.836-7.156-16-16-16-8.844,0-16,7.164-16,16v32c0,8.836 7.156,16 16,16z"/>"#);
    out.push_str(r#"<path class="error-icon" d="m496,96.586h-32c-8.844,0-16,7.164-16,16 0,8.836 7.156,16 16,16h32c8.844,0 16-7.164 16-16 0-8.836-7.156-16-16-16z"/>"#);
    out.push_str(r#"<path class="error-icon" d="m436.98,75.605c3.125,3.125 7.219,4.688 11.313,4.688 4.094,0 8.188-1.563 11.313-4.688l32-32c6.25-6.25 6.25-16.375 0-22.625s-16.375-6.25-22.625,0l-32,32c-6.251,6.25-6.251,16.375-0.001,22.625z"/>"#);
    out.checkpoint()?;
    let message = typography_theme.message_geometry();
    let _ = write!(
        &mut out,
        r#"<text class="error-text" x="{}" y="{}" font-size="{}px" style="text-anchor: middle;">{}</text>"#,
        fmt(message.x()),
        fmt(message.y()),
        fmt(message.font_size_px()),
        escape_xml(message.text())
    );
    out.checkpoint()?;

    let version = typography_theme.version_geometry();
    let _ = write!(
        &mut out,
        r#"<text class="error-text" x="{}" y="{}" font-size="{}px" style="text-anchor: middle;">{}</text>"#,
        fmt(version.x()),
        fmt(version.y()),
        fmt(version.font_size_px()),
        escape_xml(version.text())
    );
    out.checkpoint()?;

    for detail in typography_theme.detail_geometry() {
        let _ = write!(
            &mut out,
            r#"<text class="error-text" x="{}" y="{}" font-size="{}px" style="text-anchor: middle;">"#,
            fmt(detail.x()),
            fmt(detail.y()),
            fmt(detail.font_size_px()),
        );
        escape_xml_serialized_text_into(&mut out, detail.text());
        out.push_str("</text>");
        out.checkpoint()?;
    }
    out.push_str("</g></svg>\n");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error_diagram::prepare_error_diagram, resources::RenderResourcePolicy,
        text::DeterministicTextMeasurer,
    };
    use merman_model::{family::DiagramFamilyId, presentation::PresentationConfig};

    fn render_message_with_viewport(message: Option<&str>) -> (String, f64) {
        let (layout, typography) = prepare_error_diagram(
            &PresentationConfig::default(),
            &DeterministicTextMeasurer::default(),
            message,
        );
        let diagram_id = SvgId::new("merman").unwrap();
        let viewport = RootViewportContext::new(DiagramFamilyId::ERROR, diagram_id);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::default());
        let svg = render_error_diagram_svg(&layout, &typography, diagram_id, &viewport, &meter)
            .unwrap()
            .into_string_for(DiagramFamilyId::ERROR)
            .unwrap();
        (svg, layout.viewbox_height)
    }

    fn render_message(message: Option<&str>) -> String {
        render_message_with_viewport(message).0
    }

    #[test]
    fn error_svg_without_a_message_preserves_the_original_graphic() {
        let svg = render_message(None);
        assert!(svg.contains(r#"viewBox="0 0 2412 512""#));
        assert!(svg.contains("Syntax error in text"));
        assert_eq!(svg.matches(r#"font-size="42px""#).count(), 0);
    }

    #[test]
    fn error_svg_shows_literal_error_text_and_grows_the_viewport() {
        let (svg, viewport_height) =
            render_message_with_viewport(Some("Unexpected <bad>& #abcdef; #60;"));
        assert!(svg.contains(&format!(r#"viewBox="0 0 2412 {viewport_height}""#)));
        assert!(svg.contains("Unexpected &lt;bad&gt;&amp; #abcdef; #60;</text>"));
        assert!(svg.contains(r#"x="1440""#));
        assert_eq!(svg.matches(r#"font-size="42px""#).count(), 1);
    }

    #[test]
    fn error_svg_emits_at_most_four_message_lines() {
        let (svg, viewport_height) = render_message_with_viewport(Some(&"x".repeat(500)));
        assert!(svg.contains(&format!(r#"viewBox="0 0 2412 {viewport_height}""#)));
        assert_eq!(svg.matches(r#"font-size="42px""#).count(), 4);
        assert!(svg.contains(&format!("{}...</text>", "x".repeat(72))));
    }
}
