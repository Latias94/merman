use super::super::*;
use super::model::SequenceSvgModel;

#[allow(clippy::too_many_arguments)]
pub(super) fn write_sequence_svg_root_open(
    out: &mut impl SvgOutput,
    layout: &SequenceDiagramLayout,
    model: &SequenceSvgModel,
    diagram_id: SvgDiagramId<'_>,
    resources: crate::resources::RenderResourcePolicy,
    stroke_outset: f64,
    paint_bounds: &[Option<&Bounds>],
    defer_bounds: bool,
) -> Result<root_svg::RootDocument> {
    let aria_labelledby = model
        .acc_title
        .as_deref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = model
        .acc_descr
        .as_deref()
        .map(|_| format!("chart-desc-{diagram_id}"));
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "sequence");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let context = root_svg::RootViewportContext::new(crate::DiagramFamilyId::SEQUENCE, diagram_id)
        .with_resource_policy(resources);
    let document = if defer_bounds {
        context.begin_document(out, root_svg::DeferredRootSpec::responsive(), root_chrome)?
    } else {
        let bounds = sequence_root_bounds(layout, stroke_outset, paint_bounds);
        context.write_open(
            out,
            root_svg::RootViewportSpec::responsive(bounds),
            root_chrome,
        )?
    };

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

pub(super) fn sequence_root_bounds(
    layout: &SequenceDiagramLayout,
    stroke_outset: f64,
    paint_bounds: &[Option<&Bounds>],
) -> root_svg::DiagramBounds {
    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let mut root_bounds = root_svg::DiagramBounds::from_extents(
        bounds.min_x,
        bounds.min_y,
        bounds.max_x,
        bounds.max_y,
        stroke_outset,
    );

    for paint in paint_bounds.iter().flatten() {
        root_bounds = root_svg::DiagramBounds::from_extents(
            root_bounds.min_x.min(paint.min_x),
            root_bounds.min_y.min(paint.min_y),
            (root_bounds.min_x + root_bounds.width).max(paint.max_x),
            (root_bounds.min_y + root_bounds.height).max(paint.max_y),
            0.0,
        );
    }
    root_bounds
}
