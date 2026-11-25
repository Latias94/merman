use super::super::*;
use merman_model::families::pie::PieDiagramModel;
use merman_render_charts::pie::write_svg_accessibility;

pub(super) const EMPTY_PIE_WIDTH: f64 = 225.0;
pub(super) const EMPTY_PIE_HEIGHT: f64 = 450.0;
pub(super) const DEFAULT_PIE_DIAGRAM_ID: SvgId<'static> = match SvgId::new("merman") {
    Some(id) => id,
    None => panic!("invalid fallback Pie ID"),
};

pub(super) fn empty_pie_root_viewport_fallback(
    model: &PieDiagramModel,
    min_x: f64,
    min_y: f64,
    width: f64,
    height: f64,
) -> Option<(root_svg::DiagramBounds, root_svg::RootMaxWidth)> {
    if !model.sections.is_empty() {
        return None;
    }

    let computed_root_is_finite =
        min_x.is_finite() && min_y.is_finite() && width.is_finite() && height.is_finite();
    if computed_root_is_finite {
        return None;
    }

    // Mermaid can produce an invalid `-Infinity` viewport when no sections are drawn. Root
    // Viewport requires finite bounds, but valid title-widened layout bounds remain authoritative.
    Some((
        root_svg::DiagramBounds::from_view_box(0.0, 0.0, EMPTY_PIE_WIDTH, EMPTY_PIE_HEIGHT),
        root_svg::RootMaxWidth::SvgNumber(EMPTY_PIE_WIDTH),
    ))
}

pub(super) fn pie_viewport(
    layout: &PieDiagramLayout,
    model: &PieDiagramModel,
) -> (root_svg::DiagramBounds, root_svg::RootMaxWidth) {
    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 450.0,
        max_y: 450.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    empty_pie_root_viewport_fallback(model, vb_min_x, vb_min_y, vb_w, vb_h).unwrap_or_else(|| {
        (
            root_svg::DiagramBounds::from_view_box(vb_min_x, vb_min_y, vb_w, vb_h),
            root_svg::RootMaxWidth::CssSixSignificant(vb_w),
        )
    })
}

pub(super) fn open_pie_svg(
    out: &mut impl SvgOutput,
    model: &PieDiagramModel,
    diagram_id: SvgId<'_>,
    (root_bounds, root_max_width): (root_svg::DiagramBounds, root_svg::RootMaxWidth),
    use_max_width: bool,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootDocument> {
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, use_max_width)
        .with_max_width(root_max_width);

    let aria_labelledby = model
        .acc_title
        .as_deref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = model
        .acc_descr
        .as_deref()
        .map(|_| format!("chart-desc-{diagram_id}"));
    let root_document = options
        .root_viewport(crate::DiagramFamilyId::PIE, diagram_id)
        .write_open(
            out,
            root_spec,
            root_svg::RootChrome {
                aria_labelledby: aria_labelledby.as_deref(),
                aria_describedby: aria_describedby.as_deref(),
                dom: root_svg::RootDomProfile {
                    style_viewbox_order: root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle,
                    fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
                    fixed_style_placement: root_svg::RootStylePlacement::Tail,
                    trailing_newline: false,
                    ..Default::default()
                },
                ..root_svg::RootChrome::new(diagram_id, "pie")
            },
        )?;

    write_svg_accessibility(out, model, diagram_id)?;

    Ok(root_document)
}
