use std::fmt::Write as _;

use merman_model::{
    OperationPhase,
    families::c4::{C4Boundary, C4DiagramModel, C4Shape},
    runtime::OperationContext,
};
#[cfg(feature = "layout-shoal")]
use merman_render_core::text::{
    TextMeasurer, svg_text_vertical_extents_px, svg_title_bbox_vertical_extents_px,
};
use merman_render_core::{
    Bounds, Error, Result, entities,
    number_format::{canonical_number as fmt, canonical_number_with_nonfinite},
    resources::OperationWorkMeter,
    root_svg::{self, RootViewportContext},
    rough_svg::{
        closed_path_d_from_points, operation_randomness,
        paths::roughjs_hand_drawn_stroke_path_for_svg_path,
    },
    svg_css::{PreparedLookDefs, SvgId, scoped_svg_id, scoped_svg_url},
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    text::line_break::split_mermaid_common_br_lines,
    xml::{escape_attr, escape_attr_display, escape_xml, escape_xml_display},
};
use merman_theme_contract::v2::{EdgeStyle, MarkerStyle};

#[cfg(feature = "layout-shoal")]
use super::C4RoutedGeometry;
use super::{
    C4_ELEMENT_TYPES, C4_FRAMED_FRAME_WIDTH, C4_ROUNDED_RADIUS, C4_TECHNOLOGY_GAP,
    C4ClusterThemePlan, C4ConfigView, C4DiagramLayout, C4Look, C4NativeStyle, C4NodeShape,
    C4PersonOutline, C4ShapeLayout, C4TextBlockLayout, C4TextPaintPlan, C4TypographyThemePlan,
    c4_cap_radius, c4_node_shape, c4_relation_markers, c4_relation_offset, c4_text_row_offset,
};

#[cfg(feature = "layout-shoal")]
mod routed;

type C4SvgModelShape = C4Shape;
type C4SvgModelBoundary = C4Boundary;

// C4 diagram SVG renderer implementation (split from parity.rs).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum C4PaintItem {
    Shape(usize),
    BoundaryBackground(usize),
    Boundary(usize),
}

#[derive(Debug, Clone, Copy)]
enum C4PaintVisit {
    EnterBoundary(usize),
    PaintBoundary(usize),
}

fn c4_paint_order(layout: &C4DiagramLayout) -> Result<Vec<C4PaintItem>> {
    let mut shapes_by_parent: std::collections::HashMap<&str, Vec<usize>> =
        std::collections::HashMap::new();
    for (index, shape) in layout.shapes.iter().enumerate() {
        shapes_by_parent
            .entry(shape.parent_boundary.as_str())
            .or_default()
            .push(index);
    }

    let mut boundaries_by_parent: std::collections::HashMap<&str, Vec<usize>> =
        std::collections::HashMap::new();
    for (index, boundary) in layout.boundaries.iter().enumerate() {
        boundaries_by_parent
            .entry(boundary.parent_boundary.as_str())
            .or_default()
            .push(index);
    }

    let mut global_boundaries = layout
        .boundaries
        .iter()
        .enumerate()
        .filter_map(|(index, boundary)| (boundary.alias == "global").then_some(index));
    let Some(global_boundary) = global_boundaries.next() else {
        return Err(Error::InvalidModel {
            message: "c4: expected the implicit global boundary while painting".to_string(),
        });
    };
    if global_boundaries.next().is_some() {
        return Err(Error::InvalidModel {
            message: "c4: expected exactly one implicit global boundary while painting".to_string(),
        });
    }

    let mut order = Vec::with_capacity(layout.shapes.len() + layout.boundaries.len() - 1);
    let mut painted_shapes = vec![false; layout.shapes.len()];
    let mut visited_boundaries = vec![false; layout.boundaries.len()];
    let mut stack = vec![C4PaintVisit::EnterBoundary(global_boundary)];

    while let Some(visit) = stack.pop() {
        match visit {
            C4PaintVisit::EnterBoundary(index) => {
                if std::mem::replace(&mut visited_boundaries[index], true) {
                    return Err(Error::InvalidModel {
                        message: format!(
                            "c4: boundary {} is reachable more than once while painting",
                            layout.boundaries[index].alias
                        ),
                    });
                }

                let boundary = &layout.boundaries[index];
                if boundary.alias != "global" {
                    order.push(C4PaintItem::BoundaryBackground(index));
                    stack.push(C4PaintVisit::PaintBoundary(index));
                }
                if let Some(children) = boundaries_by_parent.get(boundary.alias.as_str()) {
                    stack.extend(
                        children
                            .iter()
                            .rev()
                            .copied()
                            .map(C4PaintVisit::EnterBoundary),
                    );
                }
                if let Some(shapes) = shapes_by_parent.get(boundary.alias.as_str()) {
                    for &shape_index in shapes {
                        painted_shapes[shape_index] = true;
                        order.push(C4PaintItem::Shape(shape_index));
                    }
                }
            }
            C4PaintVisit::PaintBoundary(index) => {
                order.push(C4PaintItem::Boundary(index));
            }
        }
    }

    if visited_boundaries.iter().any(|visited| !visited) {
        return Err(Error::InvalidModel {
            message: "c4: found an orphaned or cyclic boundary while painting".to_string(),
        });
    }
    if painted_shapes.iter().any(|painted| !painted) {
        return Err(Error::InvalidModel {
            message: "c4: found a shape outside the global boundary tree while painting"
                .to_string(),
        });
    }

    Ok(order)
}

fn write_c4_css_with_typography(
    out: &mut impl SvgOutput,
    diagram_id: impl std::fmt::Display + Copy,
    typography: &C4TypographyThemePlan,
) -> Result<()> {
    let common = typography.common_css();
    common.write_prefix(out, diagram_id)?;
    out.checkpoint()?;
    let person_border = typography.person_border();
    let person_bkg = typography.person_background();
    let _ = write!(
        out,
        r#"#{} .person{{stroke:{};fill:{};}}"#,
        diagram_id, person_border, person_bkg
    );
    out.checkpoint()?;
    // Mermaid's C4 stylesheet emits one type-specific label rule before the shared label rules.
    // The unified label renderer below relies on these selectors for the configured per-element
    // font family, size and weight; the inherited theme typography remains the root fallback.
    for type_name in C4_ELEMENT_TYPES {
        let style = typography
            .element_style(type_name)
            .expect("known prepared C4 type");
        let font = &style.font.measurement_style;
        let family = &style.font.css_font_family;
        let weight = font.font_weight.as_deref().unwrap_or("normal");
        let _ = write!(
            out,
            r#"#{} .c4-shape.c4-{} .label{{"#,
            diagram_id, type_name,
        );
        if !family.is_empty() {
            let _ = write!(out, "font-family:{family};");
        }
        if let Some(size) = style.font.element_font_size_css() {
            let _ = write!(out, "font-size:{size};");
        }
        let _ = write!(out, "font-weight:{weight};}}");
        out.checkpoint()?;
    }
    let _ = write!(
        out,
        r#"#{} .c4-shape .label,#{} .c4-shape .label text{{color:inherit;fill:currentColor;}}#{} .c4-shape .label .c4-name{{font-weight:bold;}}#{} .c4-shape .label .c4-type{{font-size:0.75em;}}#{} .c4-shape .label .c4-descr{{font-size:0.82em;}}#{} .c4-shape .basic,#{} .c4-shape rect,#{} .c4-shape path,#{} .c4-shape circle,#{} .c4-shape ellipse,#{} .c4-shape line{{stroke-width:2px;}}"#,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
    );
    out.checkpoint()?;
    if let Some(native) = typography.native_style() {
        let _ = write!(
            out,
            "#{diagram_id} .c4-shape .basic,#{diagram_id} .c4-shape rect,\
             #{diagram_id} .c4-shape path,#{diagram_id} .c4-shape circle,\
             #{diagram_id} .c4-shape ellipse,#{diagram_id} .c4-shape line{{stroke-width:{}px;}}",
            fmt(native.shape_width),
        );
        out.checkpoint()?;
    }
    common.write_root(out, diagram_id, diagram_id)?;
    out.checkpoint()
}

struct C4TspanText<'a> {
    /// Prepared wrapLabel output; unlike unified labels, these rows carry no word markup.
    content: &'a str,
    line_count: usize,
    x: f64,
    y: f64,
    width: f64,
    font_family: &'a str,
    font_size: f64,
    font_weight: &'a str,
    attrs: &'a [(&'a str, &'a str)],
    shoal: bool,
}

fn c4_write_text_by_tspan(out: &mut impl SvgOutput, text: C4TspanText<'_>) -> Result<()> {
    let C4TspanText {
        content,
        line_count,
        x,
        y,
        width,
        font_family,
        font_size,
        font_weight,
        attrs,
        shoal,
    } = text;
    let x = x + width / 2.0;

    out.checkpoint()?;
    let lines = split_mermaid_common_br_lines(content);
    if lines.len() != line_count {
        return Err(Error::InvalidModel {
            message: "C4 legacy text rows do not match their prepared layout".to_owned(),
        });
    }
    let alignment = if shoal {
        ""
    } else {
        r#" alignment-baseline="mathematical""#
    };

    for (i, line) in lines.iter().enumerate() {
        let dy = c4_text_row_offset(i, line_count, font_size);
        let dy_s = fmt(dy);

        let _ = write!(
            out,
            r#"<text x="{}" y="{}" dominant-baseline="middle""#,
            canonical_number_with_nonfinite(x),
            canonical_number_with_nonfinite(y)
        );
        out.checkpoint()?;
        for (k, v) in attrs {
            let _ = write!(out, r#" {k}="{}""#, escape_attr_display(v));
            out.checkpoint()?;
        }
        let _ = write!(
            out,
            r#" style="text-anchor: middle; font-size: {}px; font-weight: {}; font-family: {};"><tspan dy="{}"{alignment}>{}</tspan></text>"#,
            font_size,
            escape_attr_display(font_weight),
            escape_attr_display(font_family),
            dy_s,
            escape_xml_display(line)
        );
        out.checkpoint()?;
    }

    Ok(())
}

fn write_c4_base_defs(out: &mut impl SvgOutput, diagram_id: SvgId<'_>) -> Result<()> {
    const PINNED_C4_DATABASE_SYMBOL_D: &str = include_str!("c4_database_d_11_16_0.txt");

    let _ = write!(
        out,
        r#"<defs><symbol id="{}" width="24" height="24"><path transform="scale(.5)" d="M2 2v13h20v-13h-20zm18 11h-16v-9h16v9zm-10.228 6l.466-1h3.524l.467 1h-4.457zm14.228 3h-24l2-6h2.104l-1.33 4h18.45l-1.297-4h2.073l2 6zm-5-10h-14v-7h14v7z"/></symbol></defs>"#,
        escape_attr_display(scoped_svg_id(diagram_id, "computer"))
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"<defs><symbol id="{}" fill-rule="evenodd" clip-rule="evenodd"><path transform="scale(.5)" d="{}"/></symbol></defs>"#,
        escape_attr_display(scoped_svg_id(diagram_id, "database")),
        escape_attr(PINNED_C4_DATABASE_SYMBOL_D.trim())
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"<defs><symbol id="{}" width="24" height="24"><path transform="scale(.5)" d="M12 2c5.514 0 10 4.486 10 10s-4.486 10-10 10-10-4.486-10-10 4.486-10 10-10zm0-2c-6.627 0-12 5.373-12 12s5.373 12 12 12 12-5.373 12-12-5.373-12-12-12zm5.848 12.459c.202.038.202.333.001.372-1.907.361-6.045 1.111-6.547 1.111-.719 0-1.301-.582-1.301-1.301 0-.512.77-5.447 1.125-7.445.034-.192.312-.181.343.014l.985 6.238 5.394 1.011z"/></symbol></defs>"#,
        escape_attr_display(scoped_svg_id(diagram_id, "clock"))
    );
    out.checkpoint()
}

fn write_c4_relation_arrows(
    out: &mut impl SvgOutput,
    diagram_id: SvgId<'_>,
    [head, end]: [&str; 2],
    fill: Option<&str>,
) -> Result<()> {
    for (id, ref_x, path) in [
        (head, 9, "M 0 0 L 10 5 L 0 10 z"),
        (end, 1, "M 10 0 L 0 5 L 10 10 z"),
    ] {
        let _ = write!(
            out,
            r#"<defs><marker id="{}" refX="{ref_x}" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="{path}""#,
            escape_attr_display(scoped_svg_id(diagram_id, id))
        );
        if let Some(fill) = fill {
            let _ = write!(out, r#" fill="{}""#, escape_attr_display(fill));
        }
        out.push_str("/></marker></defs>");
        out.checkpoint()?;
    }
    Ok(())
}

fn write_c4_relation_defs(
    out: &mut impl SvgOutput,
    diagram_id: SvgId<'_>,
    fill: Option<&str>,
) -> Result<()> {
    write_c4_relation_arrows(out, diagram_id, ["arrowhead", "arrowend"], fill)?;
    let _ = write!(
        out,
        r##"<defs><marker id="{}" markerWidth="15" markerHeight="8" orient="auto" refX="16" refY="4"><path fill="black" stroke="#000000" stroke-width="1px" d="M 9,2 V 6 L16,4 Z" style="stroke-dasharray: 0, 0;"/><path fill="none" stroke="#000000" stroke-width="1px" d="M 0,1 L 6,7 M 6,1 L 0,7" style="stroke-dasharray: 0, 0;"/></marker></defs>"##,
        escape_attr_display(scoped_svg_id(diagram_id, "crosshead"))
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"<defs><marker id="{}" refX="18" refY="7" markerWidth="20" markerHeight="28" orient="auto"><path d="M 18,7 L9,13 L14,7 L9,1 Z"/></marker></defs>"#,
        escape_attr_display(scoped_svg_id(diagram_id, "filled-head"))
    );
    out.checkpoint()
}

fn available_local_id(
    base: String,
    shapes: &std::collections::HashMap<&str, &C4SvgModelShape>,
    reserved: impl Fn(&str) -> bool,
    work: &OperationWorkMeter,
) -> Result<String> {
    work.charge(base.len())?;
    if !shapes.contains_key(base.as_str()) && !reserved(&base) {
        return Ok(base);
    }
    // At most the authored aliases and two accessibility IDs can occupy this node prefix.
    for suffix in 0..=shapes.len().saturating_add(2) {
        let candidate = format!("{base}-{suffix}");
        work.charge(candidate.len())?;
        if !shapes.contains_key(candidate.as_str()) && !reserved(&candidate) {
            return Ok(candidate);
        }
    }
    Err(Error::InvalidModel {
        message: "c4: exhausted local SVG IDs".into(),
    })
}

fn reserved_definition_id(id: &str) -> bool {
    matches!(
        id,
        "computer"
            | "database"
            | "clock"
            | "arrowhead"
            | "arrowend"
            | "crosshead"
            | "filled-head"
            | "gradient"
            | "drop-shadow"
            | "drop-shadow-small"
    ) || id
        .strip_prefix("arrowhead-")
        .or_else(|| id.strip_prefix("arrowend-"))
        .or_else(|| id.strip_prefix("c4-rel-")?.strip_suffix("-label-cutout"))
        .is_some_and(|index| !index.is_empty() && index.bytes().all(|b| b.is_ascii_digit()))
}

fn c4_shape_classes(type_c4_shape: &str) -> String {
    let mut classes = format!("c4-shape c4-{type_c4_shape}");
    if type_c4_shape.starts_with("external_") {
        classes.push_str(" c4-external");
    }
    classes
}

#[allow(clippy::too_many_arguments)]
fn c4_write_unified_section(
    out: &mut impl SvgOutput,
    class: &str,
    block: &C4TextBlockLayout,
    total_width: f64,
    section_y: f64,
    color: &str,
) {
    let Some(plan) = block.render_plan.as_ref() else {
        return;
    };
    if plan.rows.is_empty() || block.text.trim().is_empty() {
        return;
    }

    let section_x = total_width / 2.0 - plan.bbox_x - block.width / 2.0;
    let section_y = section_y - plan.bbox_y;
    let _ = write!(
        out,
        r#"<g class="{}" transform="translate({}, {})"><g><rect class="background" style="stroke: none"/><text y="-10.1" style="fill:{} !important">"#,
        class,
        fmt(section_x),
        fmt(section_y),
        escape_attr(color),
    );
    for (index, row) in plan.rows.iter().enumerate() {
        let y = index as f64 * 1.1 - 0.1;
        let _ = write!(
            out,
            r#"<tspan class="text-outer-tspan row" x="0" y="{}em" dy="1.1em" text-anchor="middle">"#,
            fmt(y),
        );
        for (word_index, word) in row.words.iter().enumerate() {
            let visible = entities::decode_svg_text_content_entities(word);
            let prefix = if word_index == 0 { "" } else { " " };
            let _ = write!(
                out,
                r#"<tspan class="text-inner-tspan">{}{}</tspan>"#,
                prefix,
                escape_xml(visible.as_ref()),
            );
        }
        out.push_str("</tspan>");
    }
    out.push_str("</text></g></g>");
}

const C4_HAND_DRAWN_ROUGHNESS: f32 = 0.7;

fn c4_rounded_rect_path_d(width: f64, height: f64, radius: f64) -> String {
    let width = width.max(1.0);
    let height = height.max(1.0);
    let radius = radius.min(width / 2.0).min(height / 2.0).max(0.0);
    let left = -width / 2.0;
    let right = width / 2.0;
    let top = -height / 2.0;
    let bottom = height / 2.0;
    let mut points = vec![(left + radius, top), (right - radius, top)];

    let mut append_arc = |cx: f64, cy: f64, start: f64, end: f64| {
        for step in 1..=6 {
            let t = step as f64 / 6.0;
            let angle = start + (end - start) * t;
            points.push((cx + radius * angle.cos(), cy + radius * angle.sin()));
        }
    };

    append_arc(
        right - radius,
        top + radius,
        -std::f64::consts::FRAC_PI_2,
        0.0,
    );
    append_arc(
        right - radius,
        bottom - radius,
        0.0,
        std::f64::consts::FRAC_PI_2,
    );
    append_arc(
        left + radius,
        bottom - radius,
        std::f64::consts::FRAC_PI_2,
        std::f64::consts::PI,
    );
    append_arc(
        left + radius,
        top + radius,
        std::f64::consts::PI,
        std::f64::consts::PI * 1.5,
    );

    closed_path_d_from_points(&points)
}

fn c4_hand_drawn_stroke(
    path_data: &str,
    work_meter: &OperationWorkMeter,
    randomness: &roughr::core::RoughRandomness,
) -> Result<String> {
    let path = roughjs_hand_drawn_stroke_path_for_svg_path(
        path_data,
        C4_HAND_DRAWN_ROUGHNESS,
        work_meter,
        randomness,
    );
    work_meter.checkpoint(merman_model::OperationPhase::Emit)?;
    path.ok_or_else(|| Error::InvalidModel {
        message: "c4: failed to generate handDrawn stroke geometry".to_string(),
    })
}

fn c4_write_hand_drawn_shape(
    out: &mut impl SvgOutput,
    silhouette: &str,
    fill: &str,
    stroke: &str,
    stroke_width: f64,
    work_meter: &OperationWorkMeter,
    randomness: &roughr::core::RoughRandomness,
) -> Result<()> {
    let outline = c4_hand_drawn_stroke(silhouette, work_meter, randomness)?;
    let _ = write!(
        out,
        r#"<path class="rough-fill" d="{}" fill="{}" stroke="none"/><path class="rough-stroke" d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="stroke-width:{}px !important"/>"#,
        escape_attr(silhouette),
        escape_attr(fill),
        escape_attr(&outline),
        escape_attr(stroke),
        fmt(stroke_width),
        fmt(stroke_width),
    );
    out.checkpoint()
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes geometry, resolved styles, and drawing context separately."
)]
fn c4_write_unified_shape(
    out: &mut impl SvgOutput,
    shape: &C4ShapeLayout,
    node_shape: C4NodeShape,
    fill: &str,
    stroke: &str,
    look: C4Look,
    work_meter: &OperationWorkMeter,
    randomness: &roughr::core::RoughRandomness,
    native: Option<&C4NativeStyle>,
) -> Result<()> {
    let width = shape.width.max(1.0);
    let height = shape.height.max(1.0);
    let radius = native.map_or(C4_ROUNDED_RADIUS, |theme| theme.radius);
    let stroke_width = native.map_or(2.0, |theme| theme.shape_width);

    if look.is_hand_drawn() {
        match node_shape {
            C4NodeShape::Rounded => {
                let path = c4_rounded_rect_path_d(width, height, radius);
                out.push_str(r#"<g class="basic label-container">"#);
                out.checkpoint()?;
                c4_write_hand_drawn_shape(
                    out,
                    &path,
                    fill,
                    stroke,
                    stroke_width,
                    work_meter,
                    randomness,
                )?;
                out.push_str("</g>");
                return out.checkpoint();
            }
            C4NodeShape::Framed => {
                let path = c4_rounded_rect_path_d(width, height, radius);
                out.push_str(r#"<g class="basic label-container">"#);
                out.checkpoint()?;
                c4_write_hand_drawn_shape(
                    out,
                    &path,
                    fill,
                    stroke,
                    stroke_width,
                    work_meter,
                    randomness,
                )?;
                let frame_x = width / 2.0 - C4_FRAMED_FRAME_WIDTH;
                let frame_path = format!(
                    "M{} {} L{} {} M{} {} L{} {}",
                    fmt(-frame_x),
                    fmt(-height / 2.0),
                    fmt(-frame_x),
                    fmt(height / 2.0),
                    fmt(frame_x),
                    fmt(-height / 2.0),
                    fmt(frame_x),
                    fmt(height / 2.0),
                );
                let frame_d = c4_hand_drawn_stroke(&frame_path, work_meter, randomness)?;
                let _ = write!(
                    out,
                    r#"<path class="rough-frame" d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="stroke-width:{}px !important"/>"#,
                    escape_attr(&frame_d),
                    escape_attr(stroke),
                    fmt(stroke_width),
                    fmt(stroke_width),
                );
                out.checkpoint()?;
                out.push_str("</g>");
                return out.checkpoint();
            }
            C4NodeShape::Person => {
                let C4PersonOutline {
                    head_radius,
                    head_y,
                    body_height,
                    body_radius,
                    body_top,
                } = C4PersonOutline::new(width, height);
                // The path helper is centered, so translate the body into the person silhouette
                // after generation instead of baking the offset into the shape's layout box.
                let body_path = c4_rounded_rect_path_d(width, body_height, body_radius);
                let head_path = format!(
                    "M{r},0 A{r},{r} 0,0,1 {left},0 A{r},{r} 0,0,1 {r},0 Z",
                    r = fmt(head_radius),
                    left = fmt(-head_radius),
                );
                out.push_str(r#"<g class="basic label-container">"#);
                out.checkpoint()?;
                let _ = write!(
                    out,
                    r#"<g transform="translate(0,{})">"#,
                    fmt(body_top + body_height / 2.0)
                );
                out.checkpoint()?;
                c4_write_hand_drawn_shape(
                    out,
                    &body_path,
                    fill,
                    stroke,
                    stroke_width,
                    work_meter,
                    randomness,
                )?;
                out.push_str("</g>");
                let _ = write!(out, r#"<g transform="translate(0,{})">"#, fmt(head_y));
                out.checkpoint()?;
                c4_write_hand_drawn_shape(
                    out,
                    &head_path,
                    fill,
                    stroke,
                    stroke_width,
                    work_meter,
                    randomness,
                )?;
                out.push_str("</g></g>");
                return out.checkpoint();
            }
            C4NodeShape::Cylinder => {
                let rx = width / 2.0;
                let ry = c4_cap_radius(width);
                let body_height = (height - 2.0 * ry).max(1.0);
                // Fill the closed silhouette before painting the open interior rim.
                let body_path = format!(
                    "M0,{ry} a{rx},{ry} 0,0,1 {w},0 v{h} a{rx},{ry} 0,0,1 {mw},0 Z",
                    ry = fmt(ry),
                    rx = fmt(rx),
                    w = fmt(width),
                    h = fmt(body_height),
                    mw = fmt(-width),
                );
                let rim_path = format!(
                    "M0,{ry} a{rx},{ry} 0,0,0 {w},0",
                    ry = fmt(ry),
                    rx = fmt(rx),
                    w = fmt(width),
                );
                out.push_str(r#"<g class="basic label-container" transform="translate("#);
                let _ = write!(
                    out,
                    "{}, {})\">",
                    fmt(-width / 2.0),
                    fmt(-(body_height / 2.0 + ry))
                );
                out.checkpoint()?;
                c4_write_hand_drawn_shape(
                    out,
                    &body_path,
                    fill,
                    stroke,
                    stroke_width,
                    work_meter,
                    randomness,
                )?;
                let rim = c4_hand_drawn_stroke(&rim_path, work_meter, randomness)?;
                let _ = write!(
                    out,
                    r#"<path class="rough-stroke" d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="fill:none !important;stroke:{} !important;stroke-width:{}px !important"/>"#,
                    escape_attr(&rim),
                    escape_attr(stroke),
                    fmt(stroke_width),
                    escape_attr(stroke),
                    fmt(stroke_width),
                );
                out.push_str("</g>");
                return out.checkpoint();
            }
            C4NodeShape::HorizontalCylinder => {
                let h = height.max(1.0);
                let ry = h / 2.0;
                let rx = c4_cap_radius(h);
                let body_path = format!(
                    "M0,0 a{rx},{ry} 0,0,1 0,{mh} h{w} a{rx},{ry} 0,0,1 0,{h} Z",
                    rx = fmt(rx),
                    ry = fmt(ry),
                    mh = fmt(-h),
                    w = fmt(width),
                    h = fmt(h),
                );
                let rim_path = format!(
                    "M{w},{mh} a{rx},{ry} 0,0,0 0,{h}",
                    w = fmt(width),
                    mh = fmt(-h),
                    rx = fmt(rx),
                    ry = fmt(ry),
                    h = fmt(h),
                );
                out.push_str(r#"<g class="basic label-container" transform="translate("#);
                let _ = write!(out, "{}, {})\">", fmt(-width / 2.0), fmt(h / 2.0));
                out.checkpoint()?;
                c4_write_hand_drawn_shape(
                    out,
                    &body_path,
                    fill,
                    stroke,
                    stroke_width,
                    work_meter,
                    randomness,
                )?;
                let rim = c4_hand_drawn_stroke(&rim_path, work_meter, randomness)?;
                let _ = write!(
                    out,
                    r#"<path class="rough-stroke" d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="fill:none !important;stroke:{} !important;stroke-width:{}px !important"/>"#,
                    escape_attr(&rim),
                    escape_attr(stroke),
                    fmt(stroke_width),
                    escape_attr(stroke),
                    fmt(stroke_width),
                );
                out.push_str("</g>");
                return out.checkpoint();
            }
        }
    }

    let shape_style = |with_radius: bool| {
        if with_radius {
            format!(
                "fill:{} !important;stroke:{} !important;rx:{radius}px !important;ry:{radius}px !important",
                fill,
                stroke,
                radius = fmt(radius),
            )
        } else {
            format!("fill:{} !important;stroke:{} !important", fill, stroke)
        }
    };
    match node_shape {
        C4NodeShape::Rounded => {
            let radius_attr = native.map_or(5.0, |theme| theme.radius);
            let _ = write!(
                out,
                r#"<rect class="basic label-container" style="{}" rx="{radius}" ry="{radius}" x="{}" y="{}" width="{}" height="{}"/>"#,
                escape_attr(&shape_style(true)),
                fmt(-width / 2.0),
                fmt(-height / 2.0),
                fmt(width),
                fmt(height),
                radius = fmt(radius_attr),
            );
        }
        C4NodeShape::Framed => {
            let frame_width = C4_FRAMED_FRAME_WIDTH;
            let points = format!(
                "0,0 {},0 {},-{} 0,-{} 0,0 -8,0 {},0 {},-{} -8,-{} -8,0",
                fmt(width - 2.0 * frame_width),
                fmt(width - 2.0 * frame_width),
                fmt(height),
                fmt(height),
                fmt(width - frame_width),
                fmt(width - frame_width),
                fmt(height),
                fmt(height),
            );
            let _ = write!(
                out,
                r#"<polygon points="{}" class="label-container" transform="translate({}, {})" style="{}"/>"#,
                points,
                fmt(-width / 2.0),
                fmt(height / 2.0),
                // The C4 adapter supplies the same rx/ry node overrides for rounded and
                // framed elements; preserve those inline styles even though SVG polygons ignore
                // the radius properties.
                escape_attr(&shape_style(true)),
            );
        }
        C4NodeShape::Person => {
            let C4PersonOutline {
                head_radius,
                head_y,
                body_height,
                body_radius,
                body_top,
            } = C4PersonOutline::new(width, height);
            let _ = write!(
                out,
                r#"<g class="basic label-container"><rect x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}" style="{}"/><circle cx="0" cy="{}" r="{}" style="{}"/></g>"#,
                fmt(-width / 2.0),
                fmt(body_top),
                fmt(width),
                fmt(body_height),
                fmt(body_radius),
                fmt(body_radius),
                escape_attr(&shape_style(false)),
                fmt(head_y),
                fmt(head_radius),
                escape_attr(&shape_style(false))
            );
        }
        C4NodeShape::Cylinder => {
            let rx = width / 2.0;
            let ry = c4_cap_radius(width);
            let body_height = (height - 2.0 * ry).max(1.0);
            let path = format!(
                "M0,{}a{},{} 0,0,0 {},0a{},{} 0,0,0 {},0l0,{}a{},{} 0,0,0 {},0l0,{}",
                fmt(ry),
                fmt(rx),
                fmt(ry),
                fmt(width),
                fmt(rx),
                fmt(ry),
                fmt(-width),
                fmt(body_height),
                fmt(rx),
                fmt(ry),
                fmt(width),
                fmt(-body_height)
            );
            let _ = write!(
                out,
                r#"<path class="basic label-container outer-path" d="{}" transform="translate({}, {})" style="{}"/>"#,
                escape_attr(&path),
                fmt(-width / 2.0),
                fmt(-(body_height / 2.0 + ry)),
                escape_attr(&shape_style(false))
            );
        }
        C4NodeShape::HorizontalCylinder => {
            let h = height.max(1.0);
            let ry = h / 2.0;
            let rx = c4_cap_radius(h);
            let path = format!(
                "M0,0\n    a{},{} 0,0,1 0,-{}\n    l{},0\n    a{},{} 0,0,1 0,{}\n    M{},-{}\n    a{},{} 0,0,0 0,{}\n    l-{},0",
                fmt(rx),
                fmt(ry),
                fmt(h),
                fmt(width),
                fmt(rx),
                fmt(ry),
                fmt(h),
                fmt(width),
                fmt(h),
                fmt(rx),
                fmt(ry),
                fmt(h),
                fmt(width)
            );
            let _ = write!(
                out,
                r#"<path class="basic label-container outer-path" d="{}" transform="translate({}, {})" style="{}"/>"#,
                escape_attr(&path),
                fmt(-width / 2.0),
                fmt(h / 2.0),
                escape_attr(&shape_style(false))
            );
        }
    }
    out.checkpoint()
}

/// The projected SVG identity, root policy, and resources of the current operation.
pub struct C4SvgContext<'a> {
    pub diagram_id: SvgId<'a>,
    pub viewport: RootViewportContext<'a>,
    pub work_meter: &'a OperationWorkMeter,
    pub operation: &'a OperationContext,
    #[cfg(feature = "layout-shoal")]
    pub text_measurer: &'a dyn TextMeasurer,
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes the layout, source model, and resolved styles separately."
)]
pub fn render_c4_diagram_svg_typed(
    layout: &C4DiagramLayout,
    model: &C4DiagramModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    typography_theme: &C4TypographyThemePlan,
    cluster_theme: &C4ClusterThemePlan,
    text_paint: &C4TextPaintPlan,
    #[cfg(feature = "layout-shoal")] shoal: Option<&C4RoutedGeometry>,
    context: C4SvgContext<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = context.diagram_id;
    let native = typography_theme.native_style();
    #[cfg(feature = "layout-shoal")]
    let is_shoal = shoal.is_some();
    #[cfg(not(feature = "layout-shoal"))]
    let is_shoal = false;

    let c4_cfg = C4ConfigView::new(effective_config);
    let look = c4_cfg.look();
    let diagram_margin_x = c4_cfg.diagram_margin_x();
    let diagram_margin_y = c4_cfg.diagram_margin_y();
    let use_max_width = layout.use_max_width;
    let look_defs = PreparedLookDefs::new(effective_config);
    let has_gradient = look_defs.uses_gradient();

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: diagram_margin_x,
        min_y: diagram_margin_y,
        max_x: diagram_margin_x + layout.width.max(1.0),
        max_y: diagram_margin_y + layout.height.max(1.0),
    });
    let box_w = (bounds.max_x - bounds.min_x).max(1.0);
    let box_h = (bounds.max_y - bounds.min_y).max(1.0);
    let width = (box_w + 2.0 * diagram_margin_x).max(1.0);
    let height = (box_h + 2.0 * diagram_margin_y).max(1.0);

    let title = diagram_title
        .map(|s| s.to_string())
        .or_else(|| layout.title.clone())
        .or_else(|| model.title.clone())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let extra_vert_for_title = if title.is_some() { 60.0 } else { 0.0 };
    let (title_x, title_y) = if is_shoal {
        (
            bounds.min_x + box_w / 2.0,
            bounds.min_y - diagram_margin_y - extra_vert_for_title / 2.0,
        )
    } else {
        (
            (width - 2.0 * diagram_margin_x) / 2.0 - 4.0 * diagram_margin_x,
            bounds.min_y + diagram_margin_y,
        )
    };

    let viewbox_x = bounds.min_x - diagram_margin_x;
    let viewbox_y = if is_shoal {
        bounds.min_y - (diagram_margin_y + extra_vert_for_title)
    } else {
        -(diagram_margin_y + extra_vert_for_title)
    };
    let hand_drawn_outset = if look.is_hand_drawn() { 8.0 } else { 0.0 };
    let root_bounds = root_svg::DiagramBounds::from_view_box(
        viewbox_x - hand_drawn_outset,
        viewbox_y - hand_drawn_outset,
        width + 2.0 * hand_drawn_outset,
        height + extra_vert_for_title + 2.0 * hand_drawn_outset,
    );
    #[cfg(feature = "layout-shoal")]
    let root_bounds = if is_shoal && let Some(title) = &title {
        context.work_meter.charge(title.len())?;
        let measurer = context.text_measurer;
        let style = typography_theme.title_text_style();
        let (left, right) = measurer.measure_svg_title_bbox_x(title, &style);
        let (ascent, descent) = svg_text_vertical_extents_px(
            measurer,
            title,
            &style,
            svg_title_bbox_vertical_extents_px(&style),
        );
        let title_rect = kurbo::Rect::new(
            title_x - left - diagram_margin_x,
            title_y - ascent,
            title_x + right + diagram_margin_x,
            title_y + descent,
        )
        .inflate(hand_drawn_outset, hand_drawn_outset);
        let viewport = kurbo::Rect::new(
            root_bounds.min_x,
            root_bounds.min_y,
            root_bounds.min_x + root_bounds.width,
            root_bounds.min_y + root_bounds.height,
        );
        let merged = viewport.union(title_rect);
        if merged == viewport {
            root_bounds
        } else {
            root_svg::DiagramBounds::from_view_box(
                merged.x0,
                merged.y0,
                merged.width(),
                merged.height(),
            )
        }
    } else {
        root_bounds
    };

    let aria_roledescription = "c4";

    let acc_descr = model.acc_descr.as_deref().filter(|s| !s.is_empty());
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let aria_labelledby = model
        .acc_title
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|_| format!("chart-title-{diagram_id}"));
    let a11y_suffixes = [&aria_labelledby, &aria_describedby].map(|id| {
        id.as_deref()?
            .strip_prefix(diagram_id.semantic_str())?
            .strip_prefix('-')
    });
    let reserved = |alias: &str| {
        if is_shoal || native.is_some() {
            reserved_definition_id(alias) || a11y_suffixes.contains(&Some(alias))
        } else {
            has_gradient && alias == "gradient"
        }
    };

    let mut out = BoundedSvgOutput::new(context.work_meter);
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, use_max_width)
        .with_max_width(root_svg::RootMaxWidth::SvgNumber(root_bounds.width));
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, aria_roledescription);
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let root_document = context
        .viewport
        .write_open(&mut out, root_spec, root_chrome)?;
    context.work_meter.checkpoint(OperationPhase::Emit)?;

    if let Some(title) = model
        .acc_title
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
    {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id,
            text = escape_xml(title)
        );
        out.checkpoint()?;
    }
    if let Some(descr) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id,
            text = escape_xml(descr)
        );
        out.checkpoint()?;
    }

    out.push_str("<style>");
    out.checkpoint()?;
    {
        let mut css_out = XmlTextOutput::new(&mut out);
        write_c4_css_with_typography(&mut css_out, diagram_id, typography_theme)?;
    }
    out.push_str("</style>");
    out.checkpoint()?;
    out.push_str("<g/>");
    out.checkpoint()?;
    if is_shoal && look == C4Look::Neo {
        look_defs.write_shadow_defs(&mut out, diagram_id)?;
    }
    if has_gradient {
        out.push_str("<defs>");
        look_defs.write_gradient(&mut out, diagram_id)?;
        out.push_str("</defs>");
        out.checkpoint()?;
    }
    write_c4_base_defs(&mut out, diagram_id)?;

    let mut shape_meta: std::collections::HashMap<&str, &C4SvgModelShape> =
        std::collections::HashMap::new();
    for s in &model.shapes {
        shape_meta.insert(s.alias.as_str(), s);
    }
    let mut boundary_meta: std::collections::HashMap<&str, &C4SvgModelBoundary> =
        std::collections::HashMap::new();
    for b in &model.boundaries {
        boundary_meta.insert(b.alias.as_str(), b);
    }
    let hand_drawn_randomness = operation_randomness(
        context.operation,
        effective_config
            .get("handDrawnSeed")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0),
        "render.c4.roughjs",
    );

    for item in c4_paint_order(layout)? {
        context.work_meter.checkpoint(OperationPhase::Emit)?;
        match item {
            C4PaintItem::BoundaryBackground(index) => {
                let boundary = &layout.boundaries[index];
                if !cluster_theme.has_separate_background(&boundary.alias) {
                    continue;
                }
                let (fill, _, radius) = cluster_theme
                    .boundary_tokens(&boundary.alias)
                    .expect("prepared boundary background");
                let _ = write!(
                    out,
                    r#"<rect class="c4-boundary-background" x="{}" y="{}" fill="{}" stroke="none" width="{}" height="{}" rx="{}" ry="{}"/>"#,
                    fmt(boundary.x),
                    fmt(boundary.y),
                    escape_attr(fill),
                    fmt(boundary.width),
                    fmt(boundary.height),
                    radius,
                    radius,
                );
                out.checkpoint()?;
            }
            C4PaintItem::Shape(index) => {
                let s = &layout.shapes[index];
                let meta = shape_meta.get(s.alias.as_str()).copied();
                let paint = text_paint
                    .element_paint(index)
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("c4: missing prepared element paint {}", s.alias),
                    })?;
                let bg_color = paint.background.as_str();
                let border_color = paint.border.as_str();
                let font_color = paint.text.as_str();
                let Some(meta) = meta else {
                    return Err(Error::InvalidModel {
                        message: format!("c4: missing model shape {}", s.alias),
                    });
                };
                let node_shape = c4_node_shape(meta);
                let classes = c4_shape_classes(s.type_c4_shape.as_str());
                let disambiguated = reserved(&s.alias)
                    .then(|| {
                        available_local_id(
                            format!("c4-node-{index}"),
                            &shape_meta,
                            reserved,
                            context.work_meter,
                        )
                    })
                    .transpose()?;
                let node_id = disambiguated.as_deref().unwrap_or(&s.alias);
                let _ = write!(
                    &mut out,
                    r#"<g transform="translate({}, {})"><g id="{}" class="node {}""#,
                    fmt(s.x + s.width / 2.0),
                    fmt(s.y + s.height / 2.0),
                    escape_attr_display(scoped_svg_id(diagram_id, node_id)),
                    classes,
                );
                if look != C4Look::Classic {
                    let _ = write!(out, r#" data-look="{}""#, look.as_str());
                }
                if is_shoal {
                    let _ = write!(
                        out,
                        r#" data-et="node" data-id="{}""#,
                        escape_attr(&s.alias)
                    );
                }
                out.push_str(">");
                out.checkpoint()?;

                c4_write_unified_shape(
                    &mut out,
                    s,
                    node_shape,
                    bg_color,
                    border_color,
                    look,
                    context.work_meter,
                    &hand_drawn_randomness,
                    native,
                )?;

                let sections = [
                    (
                        "c4-name",
                        (!s.label.text.trim().is_empty()).then_some(&s.label),
                    ),
                    ("c4-type", Some(&s.type_block)),
                    ("c4-descr", s.descr.as_ref()),
                ];
                let total_width = sections
                    .iter()
                    .filter_map(|(_, block)| block.as_ref())
                    .map(|block| block.width)
                    .fold(0.0, f64::max);
                let total_height = sections
                    .iter()
                    .filter_map(|(_, block)| block.as_ref())
                    .map(|block| block.height)
                    .sum::<f64>()
                    + 3.0
                        * sections
                            .iter()
                            .filter(|(_, block)| block.is_some())
                            .count()
                            .saturating_sub(1) as f64;
                let padding = c4_cfg.shape_padding();
                let label_transform = match node_shape {
                    C4NodeShape::Person => {
                        let head_radius = (s.width * 0.23).clamp(16.0, 56.0);
                        let overlap = head_radius * 0.27;
                        let body_height = (s.height - 2.0 * head_radius + overlap).max(1.0);
                        let body_top = -s.height / 2.0 + 2.0 * head_radius - overlap;
                        (
                            -total_width / 2.0,
                            body_top + body_height / 2.0 - total_height / 2.0,
                        )
                    }
                    C4NodeShape::Cylinder => {
                        (-total_width / 2.0, -total_height / 2.0 + padding / 1.5)
                    }
                    C4NodeShape::HorizontalCylinder => {
                        let ry = s.height / 2.0;
                        let rx = ry / (2.5 + s.height / 50.0);
                        (-total_width / 2.0 - rx, -total_height / 2.0)
                    }
                    C4NodeShape::Rounded | C4NodeShape::Framed => {
                        (-total_width / 2.0, -total_height / 2.0)
                    }
                };
                let _ = write!(
                    &mut out,
                    r#"<g class="label" style="color:{} !important" transform="translate({}, {})"><rect/>"#,
                    escape_attr(font_color),
                    fmt(label_transform.0),
                    fmt(label_transform.1),
                );
                out.checkpoint()?;
                let mut section_y = 0.0;
                for (class, block) in sections {
                    if let Some(block) = block {
                        c4_write_unified_section(
                            &mut out,
                            class,
                            block,
                            total_width,
                            section_y,
                            font_color,
                        );
                        out.checkpoint()?;
                        section_y += block.height + 3.0;
                    }
                }
                out.push_str("</g></g></g>");
                out.checkpoint()?;
            }
            C4PaintItem::Boundary(index) => {
                let b = &layout.boundaries[index];
                let meta = boundary_meta.get(b.alias.as_str()).copied();
                let font_color =
                    text_paint
                        .boundary_text_color(index)
                        .ok_or_else(|| Error::InvalidModel {
                            message: format!("c4: missing prepared boundary paint {}", b.alias),
                        })?;
                let is_node_type = meta.and_then(|m| m.node_type.as_deref()).is_some();
                let Some((fill_color, stroke_color, boundary_radius_attr)) =
                    cluster_theme.boundary_tokens(&b.alias)
                else {
                    return Err(Error::InvalidModel {
                        message: format!("C4 cluster theme plan is missing boundary `{}`", b.alias),
                    });
                };
                let terminal_fill = if cluster_theme.has_separate_background(&b.alias) {
                    "none"
                } else {
                    fill_color
                };

                if is_shoal {
                    let _ = write!(
                        out,
                        r#"<g data-et="cluster" data-id="{}">"#,
                        escape_attr(&b.alias)
                    );
                } else {
                    out.push_str("<g>");
                }
                out.checkpoint()?;
                if is_node_type {
                    let _ = write!(
                        &mut out,
                        r#"<rect x="{}" y="{}" fill="{}" stroke="{}" width="{}" height="{}" rx="{}" ry="{}" stroke-width="{}"{dash}/>"#,
                        fmt(b.x),
                        fmt(b.y),
                        escape_attr(terminal_fill),
                        escape_attr(stroke_color),
                        fmt(b.width),
                        fmt(b.height),
                        boundary_radius_attr,
                        boundary_radius_attr,
                        fmt(native.map_or(1.0, |theme| theme.shape_width)),
                        dash = if native.is_some() {
                            r#" stroke-dasharray="7.0,7.0""#
                        } else {
                            ""
                        },
                    );
                } else {
                    let _ = write!(
                        &mut out,
                        r#"<rect x="{}" y="{}" fill="{}" stroke="{}" width="{}" height="{}" rx="{}" ry="{}" stroke-width="{}" stroke-dasharray="7.0,7.0"/>"#,
                        fmt(b.x),
                        fmt(b.y),
                        escape_attr(terminal_fill),
                        escape_attr(stroke_color),
                        fmt(b.width),
                        fmt(b.height),
                        boundary_radius_attr,
                        boundary_radius_attr,
                        fmt(native.map_or(1.0, |theme| theme.shape_width)),
                    );
                }
                out.checkpoint()?;
                if is_shoal {
                    out.push_str(r#"<g class="cluster-label">"#);
                    out.checkpoint()?;
                }
                #[cfg(feature = "layout-shoal")]
                let title_left = shoal
                    .and_then(|geometry| geometry.boundary_title_x.get(index).copied().flatten())
                    .map_or(b.x, |center| center - b.width / 2.0);
                #[cfg(not(feature = "layout-shoal"))]
                let title_left = b.x;
                let boundary_fonts = typography_theme.boundary_fonts();
                let name_font = &boundary_fonts.name;
                c4_write_text_by_tspan(
                    &mut out,
                    C4TspanText {
                        content: &b.label.text,
                        line_count: b.label.line_count,
                        x: title_left,
                        y: b.y + b.label.y,
                        width: b.width,
                        font_family: &name_font.css_font_family,
                        font_size: name_font.css_font_size_px.expect("legacy name size"),
                        font_weight: name_font
                            .measurement_style
                            .font_weight
                            .as_deref()
                            .unwrap_or("bold"),
                        attrs: &[("fill", font_color)],
                        shoal: is_shoal,
                    },
                )?;
                if let Some(ty) = &b.ty
                    && !ty.text.trim().is_empty()
                {
                    let ty_font = &boundary_fonts.ty;
                    c4_write_text_by_tspan(
                        &mut out,
                        C4TspanText {
                            content: &ty.text,
                            line_count: ty.line_count,
                            x: title_left,
                            y: b.y + ty.y,
                            width: b.width,
                            font_family: &ty_font.css_font_family,
                            font_size: ty_font.css_font_size_px.expect("legacy type size"),
                            font_weight: ty_font
                                .measurement_style
                                .font_weight
                                .as_deref()
                                .unwrap_or("normal"),
                            attrs: &[("fill", font_color)],
                            shoal: is_shoal,
                        },
                    )?;
                }
                if let Some(descr) = &b.descr
                    && !descr.text.trim().is_empty()
                {
                    let descr_font = &boundary_fonts.descr;
                    c4_write_text_by_tspan(
                        &mut out,
                        C4TspanText {
                            content: &descr.text,
                            line_count: descr.line_count,
                            x: title_left,
                            y: b.y + descr.y,
                            width: b.width,
                            font_family: &descr_font.css_font_family,
                            font_size: descr_font
                                .css_font_size_px
                                .expect("legacy description size"),
                            font_weight: descr_font
                                .measurement_style
                                .font_weight
                                .as_deref()
                                .unwrap_or("normal"),
                            attrs: &[("fill", font_color)],
                            shoal: is_shoal,
                        },
                    )?;
                }

                if is_shoal {
                    out.push_str("</g>");
                }
                out.push_str("</g>");
                out.checkpoint()?;
            }
        }
    }
    let classic_shoal = is_shoal && native.is_none();
    let relationship_line = text_paint.relationship_line();
    write_c4_relation_defs(
        &mut out,
        diagram_id,
        classic_shoal.then_some(relationship_line),
    )?;

    out.push_str("<g>");
    out.checkpoint()?;
    #[cfg(feature = "layout-shoal")]
    let mut relation_points = Vec::new();
    for (idx, rel) in layout.rels.iter().enumerate() {
        let (text_color, stroke_color) =
            text_paint
                .relation_paint(idx)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "c4: missing prepared relation paint {} -> {}",
                        rel.from, rel.to
                    ),
                })?;
        let custom_classic_color = classic_shoal && stroke_color != relationship_line;
        let indexed = (native.is_some() || custom_classic_color)
            .then(|| [format!("arrowhead-{idx}"), format!("arrowend-{idx}")]);
        let [arrowhead, arrowend] = indexed.as_ref().map_or(["arrowhead", "arrowend"], |ids| {
            ids.each_ref().map(String::as_str)
        });
        if let Some(native) = native {
            let (head, end, fill) = match native.marker_style {
                MarkerStyle::Stealth => (
                    "M 0 0 L 10 5 L 0 10 L 3 5 Z",
                    "M 10 0 L 0 5 L 10 10 L 7 5 Z",
                    stroke_color,
                ),
                MarkerStyle::Open => ("M 0 0 L 10 5 L 0 10", "M 10 0 L 0 5 L 10 10", "none"),
                MarkerStyle::Classic | MarkerStyle::Filled => (
                    "M 0 0 L 10 5 L 0 10 z",
                    "M 10 0 L 0 5 L 10 10 z",
                    stroke_color,
                ),
            };
            let stroke = stroke_color.trim();
            let visible_stroke = !stroke.is_empty()
                && !stroke.eq_ignore_ascii_case("none")
                && !stroke.eq_ignore_ascii_case("transparent");
            let (end_ref, start_ref, marker_geometry, stroke_geometry) =
                if is_shoal && visible_stroke {
                    (
                        "9.5",
                        "0.5",
                        r#" overflow="visible""#,
                        r#" stroke-linejoin="round" stroke-width="1""#,
                    )
                } else {
                    ("9", "1", "", "")
                };
            let _ = write!(
                out,
                r#"<defs><marker id="{}" refX="{end_ref}" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"{marker_geometry}><path d="{head}" fill="{}" stroke="{}"{stroke_geometry}/></marker><marker id="{}" refX="{start_ref}" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"{marker_geometry}><path d="{end}" fill="{}" stroke="{}"{stroke_geometry}/></marker></defs>"#,
                escape_attr_display(scoped_svg_id(diagram_id, arrowhead)),
                escape_attr(fill),
                escape_attr(stroke_color),
                escape_attr_display(scoped_svg_id(diagram_id, arrowend)),
                escape_attr(fill),
                escape_attr(stroke_color)
            );
            out.checkpoint()?;
        } else if custom_classic_color {
            write_c4_relation_arrows(
                &mut out,
                diagram_id,
                [arrowhead, arrowend],
                Some(stroke_color),
            )?;
        }
        let arrowhead_url = scoped_svg_url(diagram_id, arrowhead);
        let arrowend_url = scoped_svg_url(diagram_id, arrowend);
        let offset_x = c4_relation_offset(rel.offset_x.as_ref());
        let offset_y = c4_relation_offset(rel.offset_y.as_ref());
        let connector_width = native.map_or(1.0, |theme| theme.connector_width);
        let edge_style = native.map(|theme| theme.edge_style);
        let straight = !is_shoal
            && match edge_style {
                Some(EdgeStyle::Straight) => true,
                Some(EdgeStyle::Curved | EdgeStyle::Orthogonal) => false,
                Some(EdgeStyle::Sketch) | None => idx == 0,
            };

        #[cfg(feature = "layout-shoal")]
        let canonical = shoal
            .map(|geometry| {
                geometry
                    .relations
                    .get(idx)
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("c4: missing canonical relation {idx}"),
                    })
            })
            .transpose()?;
        #[cfg(feature = "layout-shoal")]
        if let Some(route) = canonical {
            let clip_id = route
                .shaft_cutout
                .map(|_| format!("c4-rel-{idx}-label-cutout"));
            routed::open_shaft(
                &mut out,
                route,
                stroke_color,
                connector_width,
                diagram_id,
                clip_id.as_deref().map(|id| (id, root_bounds)),
                &mut relation_points,
            )?;
        }
        if straight {
            let _ = write!(
                &mut out,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke-width="{}" stroke="{}""#,
                fmt(rel.start_point.x),
                fmt(rel.start_point.y),
                fmt(rel.end_point.x),
                fmt(rel.end_point.y),
                fmt(connector_width),
                escape_attr(stroke_color)
            );
            out.checkpoint()?;
        } else if !is_shoal {
            let cx = rel.start_point.x + (rel.end_point.x - rel.start_point.x) / 2.0
                - (rel.end_point.x - rel.start_point.x) / 4.0;
            let cy = rel.start_point.y + (rel.end_point.y - rel.start_point.y) / 2.0;
            let d = if edge_style == Some(EdgeStyle::Orthogonal) {
                let mid_x = (rel.start_point.x + rel.end_point.x) / 2.0;
                format!(
                    "M{} {} H{} V{} H{}",
                    fmt(rel.start_point.x),
                    fmt(rel.start_point.y),
                    fmt(mid_x),
                    fmt(rel.end_point.y),
                    fmt(rel.end_point.x)
                )
            } else {
                format!(
                    "M{} {} Q{} {} {} {}",
                    fmt(rel.start_point.x),
                    fmt(rel.start_point.y),
                    fmt(cx),
                    fmt(cy),
                    fmt(rel.end_point.x),
                    fmt(rel.end_point.y)
                )
            };
            let _ = write!(
                &mut out,
                r#"<path fill="none" stroke-width="{}" stroke="{}" d="{}""#,
                fmt(connector_width),
                escape_attr(stroke_color),
                escape_attr(&d)
            );
            out.checkpoint()?;
        }
        let (marker_start, marker_end) = c4_relation_markers(&rel.rel_type);
        if marker_end {
            let _ = write!(
                out,
                r#" marker-end="{}""#,
                escape_attr_display(arrowhead_url)
            );
            out.checkpoint()?;
        }
        if marker_start {
            let _ = write!(
                out,
                r#" marker-start="{}""#,
                escape_attr_display(arrowend_url)
            );
            out.checkpoint()?;
        }
        if straight {
            out.push_str(r#" style="fill: none;"/>"#);
        } else {
            out.push_str("/>");
        }
        out.checkpoint()?;
        context.work_meter.checkpoint(OperationPhase::Emit)?;

        #[cfg(feature = "layout-shoal")]
        if let Some(route) = canonical {
            let Some(label_origin) = &route.label_origin else {
                continue;
            };
            let _ = write!(
                out,
                r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate({}, {})">"#,
                escape_attr(&route.id),
                fmt(label_origin.x),
                fmt(label_origin.y)
            );
            out.checkpoint()?;
        }
        let midx = if is_shoal {
            -rel.label.width / 2.0
        } else {
            rel.start_point.x.min(rel.end_point.x)
                + (rel.end_point.x - rel.start_point.x).abs() / 2.0
                + offset_x
        };
        let midy = if is_shoal {
            rel.label.y
        } else {
            rel.start_point.y.min(rel.end_point.y)
                + (rel.end_point.y - rel.start_point.y).abs() / 2.0
                + offset_y
        };

        let message_style = typography_theme.message_font();
        let message_font = &message_style.measurement_style;
        let message_family = message_style.css_font_family.as_str();
        let message_weight = message_font.font_weight.as_deref().unwrap_or("normal");
        let message_size = message_style.css_font_size_px.expect("legacy message size");
        #[cfg(feature = "layout-shoal")]
        if let Some(reference) = canonical.and_then(|route| route.label_reference.as_deref()) {
            c4_write_text_by_tspan(
                &mut out,
                C4TspanText {
                    content: &reference.text,
                    line_count: reference.line_count,
                    x: -reference.width / 2.0,
                    y: reference.y,
                    width: reference.width,
                    font_family: message_family,
                    font_size: message_size,
                    font_weight: message_weight,
                    attrs: &[("fill", text_color)],
                    shoal: true,
                },
            )?;
        }
        c4_write_text_by_tspan(
            &mut out,
            C4TspanText {
                content: &rel.label.text,
                line_count: rel.label.line_count,
                x: midx,
                y: midy,
                width: rel.label.width,
                font_family: message_family,
                font_size: message_size,
                font_weight: message_weight,
                attrs: &[("fill", text_color)],
                shoal: is_shoal,
            },
        )?;
        if let Some(techn) = &rel.techn
            && !techn.text.trim().is_empty()
        {
            let techn_text = format!("[{}]", techn.text);
            let width = rel.label.width.max(techn.width);
            c4_write_text_by_tspan(
                &mut out,
                C4TspanText {
                    content: &techn_text,
                    line_count: techn.line_count,
                    x: if is_shoal { -width / 2.0 } else { midx },
                    y: if is_shoal {
                        techn.y
                    } else {
                        midy + message_size + C4_TECHNOLOGY_GAP
                    },
                    width,
                    font_family: message_family,
                    font_size: message_size,
                    font_weight: message_weight,
                    attrs: &[("fill", text_color), ("font-style", "italic")],
                    shoal: is_shoal,
                },
            )?;
        }
        if is_shoal {
            out.push_str("</g></g>");
            out.checkpoint()?;
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;

    if let Some(title) = title {
        let _ = write!(
            &mut out,
            r#"<text x="{}" y="{}""#,
            fmt(title_x),
            fmt(title_y),
        );
        if is_shoal {
            out.push_str(r#" text-anchor="middle""#);
        }
        if let Some(native) = native {
            let _ = write!(
                out,
                r#" style="fill:{};font-family:{};font-weight:{}""#,
                escape_attr(&native.title_ink),
                escape_attr(&native.title_family),
                native.title_weight,
            );
        }
        let _ = write!(&mut out, ">{}</text>", escape_xml(&title));
        out.checkpoint()?;
    }

    out.push_str("</svg>");
    context.work_meter.checkpoint(OperationPhase::Emit)?;
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fmt;
    use std::ops::Range;

    #[derive(Default)]
    struct RejectAfterFirstWrite {
        write_attempts: usize,
        rejected: bool,
        retained: String,
    }

    impl RejectAfterFirstWrite {
        fn record_write(&mut self, value: &str) -> fmt::Result {
            self.write_attempts += 1;
            if self.write_attempts == 1 {
                self.rejected = true;
                return Err(fmt::Error);
            }
            self.retained.push_str(value);
            Ok(())
        }
    }

    impl fmt::Write for RejectAfterFirstWrite {
        fn write_str(&mut self, value: &str) -> fmt::Result {
            self.record_write(value)
        }
    }

    impl SvgOutput for RejectAfterFirstWrite {
        fn push_str(&mut self, value: &str) {
            let _ = self.record_write(value);
        }

        fn push(&mut self, value: char) {
            let mut encoded = [0u8; 4];
            let _ = self.record_write(value.encode_utf8(&mut encoded));
        }

        fn len(&self) -> usize {
            self.retained.len()
        }

        fn as_str(&self) -> &str {
            self.retained.as_str()
        }

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> Result<()> {
            if self.rejected {
                Err(Error::InvalidModel {
                    message: "test SVG sink rejected the first write".to_string(),
                })
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn c4_text_lines_stop_after_the_first_svg_sink_failure() {
        let mut out = RejectAfterFirstWrite::default();

        let error = c4_write_text_by_tspan(
            &mut out,
            C4TspanText {
                content: "first<br>second<br>third",
                line_count: 3,
                x: 0.0,
                y: 0.0,
                width: 100.0,
                font_family: "sans-serif",
                font_size: 16.0,
                font_weight: "normal",
                attrs: &[("fill", "#444444")],
                shoal: false,
            },
        )
        .expect_err("the rejecting sink must stop C4 text emission");

        assert!(matches!(error, Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "C4 text emission must stop at the first failed sink checkpoint"
        );
    }

    #[test]
    fn c4_css_honors_mermaid_11_16_person_and_common_theme_options() {
        let mut css = String::new();
        write_c4_css_with_typography(
            &mut css,
            "c4",
            &prepared_css_for_test(&json!({
                "themeVariables": {
                    "personBorder": "#112233",
                    "personBkg": "#445566",
                    "textColor": "#778899",
                    "nodeBorder": "#aabbcc",
                    "strokeWidth": 2
                }
            })),
        )
        .expect("write C4 CSS");

        assert!(css.contains("#c4{"));
        assert!(css.contains("fill:#778899;"));
        assert!(css.contains("#c4 .person{stroke:#112233;fill:#445566;}"));
        assert!(
            css.contains(r#"#c4 [data-look="neo"].node path{stroke:#aabbcc;stroke-width:2px;}"#)
        );
        assert!(
            css.find("#c4 .person") < css.find(r#"#c4 [data-look="neo"].node path"#),
            "diagram-specific C4 rules must precede Mermaid's common Neo suffix"
        );
        assert!(css.ends_with(
            r#"#c4 :root{--mermaid-font-family:"trebuchet ms",verdana,arial,sans-serif;}"#
        ));
    }

    #[test]
    fn c4_css_does_not_treat_authored_font_family_as_an_internal_placeholder() {
        let authored_font_family = "__MERMAN_C4_DIAGRAM_ID_PROJECTION__";
        let mut css = String::new();
        write_c4_css_with_typography(
            &mut css,
            "c4",
            &prepared_css_for_test(&json!({
                "fontFamily": authored_font_family,
            })),
        )
        .expect("write C4 CSS");

        assert!(css.contains(authored_font_family));
    }

    fn prepared_css_for_test(config: &serde_json::Value) -> C4TypographyThemePlan {
        let model = serde_json::from_value(json!({})).expect("empty C4 model");
        let mut plan = C4TypographyThemePlan::resolve(
            None,
            &super::super::test_config(config.clone()),
            &model,
        );
        plan.use_config_root_css_for_test(config);
        plan
    }
}
