use super::super::*;

// Error diagram SVG renderer implementation (split from parity.rs).

pub(crate) fn render_error_diagram_svg_model(
    layout: &ErrorDiagramLayout,
    _semantic: &merman_core::diagrams::error_diagram::ErrorDiagramRenderModel,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    render_error_diagram_svg_inner(layout, effective_config, options)
}

fn render_error_diagram_svg_inner(
    layout: &ErrorDiagramLayout,
    effective_config: &serde_json::Value,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");

    let mut out = String::new();
    let root_bounds = root_svg::DiagramBounds::from_view_box(
        0.0,
        0.0,
        layout.viewbox_width,
        layout.viewbox_height,
    );
    let root_spec = root_svg::RootViewportSpec::responsive(root_bounds)
        .with_max_width(root_svg::RootMaxWidth::SvgNumber(layout.max_width_px))
        .without_background();
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "error");
    root_chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
    root_chrome.dom.trailing_newline = false;
    let root_document =
        root_svg::RootViewportContext::new(crate::family::RenderFamilyKind::Error, diagram_id)
            .write_open(&mut out, root_spec, root_chrome)?;
    options.checkpoint_emit()?;
    let css = info_css_with_config(diagram_id, effective_config);
    let _ = write!(
        &mut out,
        r#"<style xmlns="http://www.w3.org/1999/xhtml">{}</style>"#,
        css
    );
    let body = crate::drawing_list::ErrorSvgBody::new();
    body.write_into(&mut out);
    options.checkpoint_emit()?;
    out.push_str("</svg>\n");
    options.checkpoint_emit()?;
    root_document.complete(out)
}
