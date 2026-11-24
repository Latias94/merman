use super::super::*;
use crate::model::PieSliceLayout;
use crate::pie::{PIE_MARGIN_PX, PieLegendPosition};
use merman_model::families::pie::PieDiagramModel;

pub(super) const EMPTY_PIE_WIDTH: f64 = 225.0;
pub(super) const EMPTY_PIE_HEIGHT: f64 = 450.0;

fn pie_polar_xy(radius: f64, angle: f64) -> (f64, f64) {
    let x = radius * angle.cos();
    let y = radius * angle.sin();
    (x, y)
}

fn pie_path_round(value: f64) -> f64 {
    // d3-shape's default pathRound(3) uses Math.round, including negative half ties.
    let scaled = value * 1000.0;
    let lower = scaled.floor();
    let rounded = if scaled - lower >= 0.5 {
        lower + 1.0
    } else {
        lower
    };
    rounded / 1000.0
}

pub(super) fn pie_path_number(value: f64) -> crate::number_format::CanonicalNumber {
    fmt(pie_path_round(value))
}

fn append_pie_arc(out: &mut String, radius: f64, start_angle: f64, end_angle: f64) {
    let (x0, y0) = pie_polar_xy(radius, start_angle);
    let (x1, y1) = pie_polar_xy(radius, end_angle);
    let mut delta = (end_angle - start_angle).abs();
    let sweep = u8::from(end_angle > start_angle);
    let r = pie_path_number(radius);

    // d3-path's circle threshold is independent of d3-shape's full-circle branch.
    if delta > std::f64::consts::TAU - 1e-6 {
        let _ = write!(
            out,
            "A{r},{r},0,1,{sweep},{mx},{my}A{r},{r},0,1,{sweep},{x0},{y0}",
            mx = pie_path_number(-x0),
            my = pie_path_number(-y0),
            x0 = pie_path_number(x0),
            y0 = pie_path_number(y0),
        );
    } else if delta > 1e-6 {
        // d3-path 3.1.1 splits a large arc whose endpoints coincide after rounding.
        // Each donut ring must make this decision using its own radius.
        if delta >= std::f64::consts::PI
            && pie_path_round(x0) == pie_path_round(x1)
            && pie_path_round(y0) == pie_path_round(y1)
        {
            delta /= 2.0;
            let (mx, my) = pie_polar_xy(radius, start_angle + delta);
            let _ = write!(
                out,
                "A{r},{r},0,{large},{sweep},{mx},{my}",
                large = u8::from(delta >= std::f64::consts::PI),
                mx = pie_path_number(mx),
                my = pie_path_number(my),
            );
        }
        let _ = write!(
            out,
            "A{r},{r},0,{large},{sweep},{x1},{y1}",
            large = u8::from(delta >= std::f64::consts::PI),
            x1 = pie_path_number(x1),
            y1 = pie_path_number(y1),
        );
    }
}

pub(super) fn pie_slice_class(highlight: &str, label: &str) -> &'static str {
    if highlight == "hover" {
        "pieCircle highlightedOnHover"
    } else if highlight == label {
        "pieCircle highlighted"
    } else {
        "pieCircle"
    }
}

pub(super) fn pie_slice_path(slice: &PieSliceLayout, radius: f64, inner_radius: f64) -> String {
    if slice.is_full_circle {
        return if inner_radius > 0.0 {
            format!(
                "M0,{nr}A{r},{r},0,1,1,0,{r}A{r},{r},0,1,1,0,{nr}M0,{nir}A{ir},{ir},0,1,0,0,{ir}A{ir},{ir},0,1,0,0,{nir}Z",
                r = pie_path_number(radius),
                nr = pie_path_number(-radius),
                ir = pie_path_number(inner_radius),
                nir = pie_path_number(-inner_radius),
            )
        } else {
            format!(
                "M0,{nr}A{r},{r},0,1,1,0,{r}A{r},{r},0,1,1,0,{nr}Z",
                r = pie_path_number(radius),
                nr = pie_path_number(-radius),
            )
        };
    }
    let start_angle = slice.start_angle - std::f64::consts::FRAC_PI_2;
    let end_angle = slice.end_angle - std::f64::consts::FRAC_PI_2;
    let (x0, y0) = pie_polar_xy(radius, start_angle);
    let mut d = format!(
        "M{x0},{y0}",
        x0 = pie_path_number(x0),
        y0 = pie_path_number(y0),
    );
    append_pie_arc(&mut d, radius, start_angle, end_angle);
    if inner_radius > 0.0 {
        let (ix1, iy1) = pie_polar_xy(inner_radius, end_angle);
        let _ = write!(
            d,
            "L{ix1},{iy1}",
            ix1 = pie_path_number(ix1),
            iy1 = pie_path_number(iy1),
        );
        append_pie_arc(&mut d, inner_radius, end_angle, start_angle);
    } else {
        d.push_str("L0,0");
    }
    d.push('Z');
    d
}

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
    diagram_id: SvgDiagramId<'_>,
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
    out.checkpoint()?;

    Ok(root_document)
}

pub(super) fn pie_group_offset(
    layout: &PieDiagramLayout,
    legend_position: PieLegendPosition,
    content_width: f64,
) -> (f64, f64) {
    let x = if legend_position == PieLegendPosition::Left {
        (content_width - (layout.center_x * 2.0 + PIE_MARGIN_PX)).max(0.0)
    } else {
        0.0
    };
    let y = if legend_position == PieLegendPosition::Top {
        layout.legend_step_y * ((layout.legend_items.len() as f64) + 1.0)
    } else {
        0.0
    };
    (x, y)
}

pub(super) fn write_pie_group_open(
    out: &mut impl SvgOutput,
    layout: &PieDiagramLayout,
    offset: (f64, f64),
) {
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
