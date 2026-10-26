use super::super::*;
use super::SequenceEmitCheckpoints;
use super::model::SequenceSvgModel;

pub(super) struct SequenceTitleGeometry {
    pub(super) x: f64,
    pub(super) bounds: Bounds,
}

pub(super) fn sequence_title_geometry(
    title: &str,
    layout: &SequenceDiagramLayout,
    config: &serde_json::Value,
    diagram_margin_x: f64,
    measurer: &dyn TextMeasurer,
    checkpoints: SequenceEmitCheckpoints<'_>,
) -> Result<SequenceTitleGeometry> {
    let content_width = layout
        .bounds
        .as_ref()
        .map_or(100.0, |bounds| bounds.max_x - bounds.min_x);
    // Keep Mermaid's start-anchored title position independent of the expanded viewport.
    let x = (content_width - 2.0 * diagram_margin_x) / 2.0 - 2.0 * diagram_margin_x;
    let style = crate::sequence::config::SequenceConfigView::new(config).root_text_style();
    checkpoints.checkpoint()?;
    let (left, right) = measurer.measure_svg_title_bbox_x(title, &style);
    checkpoints.checkpoint()?;
    let advance = measurer.measure_svg_text_computed_length_px(title, &style);
    checkpoints.checkpoint()?;
    let (ascent, descent) = crate::text::svg_text_vertical_extents_px(
        measurer,
        title,
        &style,
        crate::text::svg_title_bbox_vertical_extents_px(&style),
    );
    checkpoints.checkpoint()?;
    Ok(SequenceTitleGeometry {
        x,
        bounds: Bounds {
            // TitleBBoxX is middle-anchored; the emitted raw text is start-anchored.
            min_x: x + advance / 2.0 - left,
            max_x: x + advance / 2.0 + right,
            min_y: -25.0 - ascent,
            max_y: -25.0 + descent,
        },
    })
}

pub(super) fn write_sequence_svg_root_open(
    out: &mut String,
    layout: &SequenceDiagramLayout,
    model: &SequenceSvgModel,
    diagram_id: SvgDiagramId<'_>,
    title_bounds: Option<&Bounds>,
) -> Result<root_svg::RootDocument> {
    let mut bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    if let Some(title) = title_bounds {
        bounds.min_x = bounds.min_x.min(title.min_x);
        bounds.min_y = bounds.min_y.min(title.min_y);
        bounds.max_x = bounds.max_x.max(title.max_x);
        bounds.max_y = bounds.max_y.max(title.max_y);
    }
    let root_bounds = root_svg::DiagramBounds::from_extents(
        bounds.min_x,
        bounds.min_y,
        bounds.max_x,
        bounds.max_y,
        0.0,
    );

    let aria_labelledby = model
        .acc_title
        .as_deref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = model
        .acc_descr
        .as_deref()
        .map(|_| format!("chart-desc-{diagram_id}"));
    let root_spec = root_svg::RootViewportSpec::responsive(root_bounds);
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "sequence");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let document =
        root_svg::RootViewportContext::new(crate::family::RenderFamilyKind::Sequence, diagram_id)
            .write_open(out, root_spec, root_chrome)?;

    if let Some(title) = model.acc_title.as_deref() {
        let _ = write!(
            out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id,
            text = escape_xml_display(title)
        );
    }
    if let Some(desc) = model.acc_descr.as_deref() {
        let _ = write!(
            out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id,
            text = escape_xml_display(desc)
        );
    }

    Ok(document)
}
