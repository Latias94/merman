use super::super::*;
use super::model::SequenceSvgModel;

pub(super) struct SequenceRootMetrics {
    pub(super) viewbox_width: f64,
}

pub(super) fn write_sequence_svg_root_open(
    out: &mut String,
    layout: &SequenceDiagramLayout,
    model: &SequenceSvgModel,
    diagram_id: &str,
    root_viewport_override_policy: RootViewportOverridePolicy,
) -> Result<SequenceRootMetrics> {
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    // Upstream Mermaid viewports are driven by browser layout pipelines and often land on an `f32`
    // lattice (e.g. `...49998474121094`). Mirror that by quantizing the extrema to `f32` first,
    // then computing width/height in `f32` space.
    let min_x_f32 = bounds.min_x as f32;
    let min_y_f32 = bounds.min_y as f32;
    let max_x_f32 = bounds.max_x as f32;
    let max_y_f32 = bounds.max_y as f32;

    let vb_min_x = min_x_f32 as f64;
    let vb_min_y = min_y_f32 as f64;
    let vb_w = ((max_x_f32 - min_x_f32).max(1.0)) as f64;
    let vb_h = ((max_y_f32 - min_y_f32).max(1.0)) as f64;

    let aria_labelledby = model
        .acc_title
        .as_deref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = model
        .acc_descr
        .as_deref()
        .map(|_| format!("chart-desc-{diagram_id}"));
    let root_bounds = root_svg::DiagramBounds::from_view_box(vb_min_x, vb_min_y, vb_w, vb_h);
    let root_spec = root_svg::RootViewportSpec::responsive(root_bounds);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "sequence");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    root_svg::RootViewportContext::new(
        crate::family::RenderFamilyKind::Sequence,
        diagram_id,
        root_viewport_override_policy,
    )
    .write_open(out, root_spec, root_chrome)?;

    if let Some(title) = model.acc_title.as_deref() {
        let _ = write!(
            out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml_display(title)
        );
    }
    if let Some(desc) = model.acc_descr.as_deref() {
        let _ = write!(
            out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml_display(desc)
        );
    }

    Ok(SequenceRootMetrics {
        viewbox_width: vb_w,
    })
}
