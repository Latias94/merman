use super::*;

pub(super) fn render_packet_family_artifact(
    artifact: &crate::family::PacketFamilyArtifact<'_>,
    diagram_title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let layout = artifact.pair().layout();
    let model = artifact.pair().semantic();
    let style = artifact.style();
    let diagram_id = options.diagram_id_or("merman");
    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: layout.width.max(1.0),
        max_y: layout.height.max(1.0),
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let aria_labelledby = model
        .acc_title
        .as_deref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = model
        .acc_descr
        .as_deref()
        .map(|_| format!("chart-desc-{diagram_id}"));
    let root_bounds = root_svg::DiagramBounds::from_view_box(vb_min_x, vb_min_y, vb_w, vb_h);
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, style.use_max_width());
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "packet");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
    root_chrome.dom.trailing_newline = false;
    let root_document = options
        .root_viewport(crate::DiagramFamilyId::PACKET, diagram_id)
        .write_open(&mut out, root_spec, root_chrome)?;

    crate::packet::write_svg_contents(&mut out, layout, model, style, diagram_title, diagram_id)?;
    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
}
