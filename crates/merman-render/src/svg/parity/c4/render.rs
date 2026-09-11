use super::super::*;
use crate::c4::{C4_DEFAULT_FONT_FAMILY, C4_ELEMENT_TYPES, C4_FRAMED_FRAME_WIDTH, C4ConfigView};
use crate::svg::parity::flowchart::{
    roughjs_hand_drawn_stroke_path_for_svg_path, roughjs_paths_for_circle,
    roughjs_paths_for_hand_drawn_svg_path,
};
use crate::svg::parity::roughjs_common::closed_path_d_from_points;
use merman_core::diagrams::c4::{
    C4BoundaryRenderModel, C4DiagramRenderModel, C4RelRenderModel, C4ShapeRenderModel,
};
type C4SvgModelShape = C4ShapeRenderModel;
type C4SvgModelBoundary = C4BoundaryRenderModel;
type C4SvgModelRel = C4RelRenderModel;

// C4 diagram SVG renderer implementation (split from parity.rs).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum C4PaintItem {
    Shape(usize),
    Boundary(usize),
}

#[derive(Debug, Clone, Copy)]
enum C4PaintVisit {
    EnterBoundary(usize),
    PaintBoundary(usize),
}

fn c4_paint_order(layout: &crate::model::C4DiagramLayout) -> Result<Vec<C4PaintItem>> {
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
        return Err(crate::Error::InvalidModel {
            message: "c4: expected the implicit global boundary while painting".to_string(),
        });
    };
    if global_boundaries.next().is_some() {
        return Err(crate::Error::InvalidModel {
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
                    return Err(crate::Error::InvalidModel {
                        message: format!(
                            "c4: boundary {} is reachable more than once while painting",
                            layout.boundaries[index].alias
                        ),
                    });
                }

                let boundary = &layout.boundaries[index];
                if boundary.alias != "global" {
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
        return Err(crate::Error::InvalidModel {
            message: "c4: found an orphaned or cyclic boundary while painting".to_string(),
        });
    }
    if painted_shapes.iter().any(|painted| !painted) {
        return Err(crate::Error::InvalidModel {
            message: "c4: found a shape outside the global boundary tree while painting"
                .to_string(),
        });
    }

    Ok(order)
}

#[derive(Debug, Clone)]
struct C4CssEmission {
    font_family: String,
    font_size_css: String,
    base_typography_emitted: bool,
    root_typography_emitted: bool,
}

fn write_c4_css_with_typography(
    out: &mut impl SvgOutput,
    diagram_id: impl std::fmt::Display + Copy,
    effective_config: &serde_json::Value,
    typography: Option<&crate::c4::C4TypographyThemePlan>,
) -> Result<C4CssEmission> {
    let parts = typography.map_or_else(
        || info_css_parts_with_config(diagram_id, effective_config),
        |typography| {
            info_css_parts_with_resolved_typography(
                diagram_id,
                effective_config,
                typography.font_family_css(),
                typography.font_size_css(),
            )
        },
    );
    out.push_str(&parts.css_prefix);
    out.checkpoint()?;
    let person_border = theme_token(
        effective_config,
        "personBorder",
        "hsl(240, 60%, 86.2745098039%)",
    );
    let person_bkg = theme_token(effective_config, "personBkg", "#ECECFF");
    let _ = write!(
        out,
        r#"#{} .person{{stroke:{};fill:{};}}"#,
        diagram_id, person_border, person_bkg
    );
    out.checkpoint()?;
    // Mermaid's C4 stylesheet emits one type-specific label rule before the shared label rules.
    // The unified label renderer below relies on these selectors for the configured per-element
    // font family, size and weight; the inherited theme typography remains the root fallback.
    let c4_cfg = C4ConfigView::new(effective_config);
    for type_name in C4_ELEMENT_TYPES {
        let font = c4_cfg.shape_font(type_name);
        let family = crate::config::normalize_css_font_family(
            font.font_family
                .as_deref()
                .unwrap_or(C4_DEFAULT_FONT_FAMILY),
        );
        let weight = font.font_weight.as_deref().unwrap_or("normal");
        let _ = write!(
            out,
            r#"#{} .c4-shape.c4-{} .label{{font-family:{};font-size:{}px;font-weight:{};}}"#,
            diagram_id,
            type_name,
            family,
            fmt(font.font_size),
            weight,
        );
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
    let root_typography_emitted = !parts.root_rule.is_empty();
    out.push_str(&parts.root_rule);
    out.checkpoint().map(|()| C4CssEmission {
        font_family: parts.font_family,
        font_size_css: parts.font_size_css,
        base_typography_emitted: parts.base_typography_emitted,
        root_typography_emitted,
    })
}

struct C4TspanText<'a> {
    content: &'a str,
    x: f64,
    y: f64,
    width: f64,
    font_family: &'a str,
    font_size: f64,
    font_weight: &'a str,
    attrs: &'a [(&'a str, &'a str)],
}

fn c4_write_text_by_tspan(out: &mut impl SvgOutput, text: C4TspanText<'_>) -> Result<bool> {
    let C4TspanText {
        content,
        x,
        y,
        width,
        font_family,
        font_size,
        font_weight,
        attrs,
    } = text;
    let x = x + width / 2.0;
    let mut style = String::new();
    let _ = write!(
        &mut style,
        "text-anchor: middle; font-size: {}px; font-weight: {}; font-family: {};",
        fmt(font_size.max(1.0)),
        font_weight,
        font_family
    );

    let normalized = content
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("<br>", "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let n = lines.len().max(1) as f64;
    let mut visible_text = false;

    for (i, line) in lines.iter().enumerate() {
        let dy = (i as f64) * font_size - (font_size * (n - 1.0)) / 2.0;
        let dy_s = fmt(dy);

        let _ = write!(
            out,
            r#"<text x="{}" y="{}" dominant-baseline="middle""#,
            fmt(x),
            fmt(y)
        );
        out.checkpoint()?;
        for (k, v) in attrs {
            let _ = write!(out, r#" {k}="{v}""#);
            out.checkpoint()?;
        }
        let _ = write!(
            out,
            r#" style="{}"><tspan dy="{}" alignment-baseline="mathematical">{}</tspan></text>"#,
            escape_attr(&style),
            dy_s,
            escape_xml(line)
        );
        out.checkpoint()?;
        visible_text = visible_text || !line.trim().is_empty();
    }

    Ok(visible_text)
}

fn write_c4_base_defs(out: &mut impl SvgOutput, diagram_id: SvgDiagramId<'_>) -> Result<()> {
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

fn write_c4_relation_defs(out: &mut impl SvgOutput, diagram_id: SvgDiagramId<'_>) -> Result<()> {
    let _ = write!(
        out,
        r#"<defs><marker id="{}" refX="9" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 0 0 L 10 5 L 0 10 z"/></marker></defs>"#,
        escape_attr_display(scoped_svg_id(diagram_id, "arrowhead"))
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"<defs><marker id="{}" refX="1" refY="5" markerUnits="userSpaceOnUse" markerWidth="12" markerHeight="12" orient="auto"><path d="M 10 0 L 0 5 L 10 10 z"/></marker></defs>"#,
        escape_attr_display(scoped_svg_id(diagram_id, "arrowend"))
    );
    out.checkpoint()?;
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
    block: &crate::model::C4TextBlockLayout,
    total_width: f64,
    section_y: f64,
    color: &str,
) -> bool {
    let Some(plan) = block.render_plan.as_ref() else {
        return false;
    };
    if plan.rows.is_empty() || block.text.trim().is_empty() {
        return false;
    }

    let mut visible_text = false;

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
            let visible = crate::entities::decode_svg_text_content_entities(word);
            visible_text = visible_text || !visible.trim().is_empty();
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
    visible_text
}

const C4_HAND_DRAWN_ROUGHNESS: f32 = 0.7;
const C4_HAND_DRAWN_FILL_WEIGHT: f64 = 4.0;

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

fn c4_hand_drawn_paths(
    path_data: &str,
    fill: &str,
    stroke: &str,
    stroke_width: f32,
    work_meter: &crate::resources::OperationWorkMeter,
    randomness: &roughr::core::RoughRandomness,
) -> Result<(String, String)> {
    roughjs_paths_for_hand_drawn_svg_path(
        path_data,
        fill,
        stroke,
        stroke_width,
        "0 0",
        work_meter,
        randomness,
    )
    .ok_or_else(|| crate::Error::InvalidModel {
        message: "c4: handDrawn shape colors must be representable by the RoughJS adapter"
            .to_string(),
    })
}

fn c4_hand_drawn_stroke(
    path_data: &str,
    randomness: &roughr::core::RoughRandomness,
) -> Result<String> {
    roughjs_hand_drawn_stroke_path_for_svg_path(path_data, C4_HAND_DRAWN_ROUGHNESS, randomness)
        .ok_or_else(|| crate::Error::InvalidModel {
            message: "c4: failed to generate handDrawn stroke geometry".to_string(),
        })
}

fn c4_write_hand_drawn_pair(
    out: &mut impl SvgOutput,
    pair: &(String, String),
    fill: &str,
    stroke: &str,
    stroke_width: f32,
) -> Result<()> {
    let (fill_d, stroke_d) = pair;
    let _ = write!(
        out,
        r#"<path class="rough-fill" d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="stroke-width:{}px !important"/><path class="rough-stroke" d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="stroke-width:{}px !important"/>"#,
        escape_attr(fill_d),
        escape_attr(fill),
        fmt(C4_HAND_DRAWN_FILL_WEIGHT),
        fmt(C4_HAND_DRAWN_FILL_WEIGHT),
        escape_attr(stroke_d),
        escape_attr(stroke),
        fmt(stroke_width as f64),
        fmt(stroke_width as f64),
    );
    out.checkpoint()
}

fn c4_write_unified_shape(
    out: &mut impl SvgOutput,
    shape: &crate::model::C4ShapeLayout,
    node_shape: crate::c4::C4NodeShape,
    fill: &str,
    stroke: &str,
    look: crate::c4::C4Look,
    work_meter: &crate::resources::OperationWorkMeter,
    randomness: &roughr::core::RoughRandomness,
) -> Result<()> {
    let width = shape.width.max(1.0);
    let height = shape.height.max(1.0);

    if look.is_hand_drawn() {
        match node_shape {
            crate::c4::C4NodeShape::Rounded => {
                let path = c4_rounded_rect_path_d(width, height, 12.0);
                let pair = c4_hand_drawn_paths(&path, fill, stroke, 2.0, work_meter, randomness)?;
                out.push_str(r#"<g class="basic label-container">"#);
                out.checkpoint()?;
                c4_write_hand_drawn_pair(out, &pair, fill, stroke, 2.0)?;
                out.push_str("</g>");
                return out.checkpoint();
            }
            crate::c4::C4NodeShape::Framed => {
                let path = c4_rounded_rect_path_d(width, height, 12.0);
                let pair = c4_hand_drawn_paths(&path, fill, stroke, 2.0, work_meter, randomness)?;
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
                let frame_d = c4_hand_drawn_stroke(&frame_path, randomness)?;
                out.push_str(r#"<g class="basic label-container">"#);
                out.checkpoint()?;
                c4_write_hand_drawn_pair(out, &pair, fill, stroke, 2.0)?;
                let _ = write!(
                    out,
                    r#"<path class="rough-frame" d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0" style="stroke-width:{}px !important"/>"#,
                    escape_attr(&frame_d),
                    escape_attr(stroke),
                    fmt(2.0),
                    fmt(2.0),
                );
                out.checkpoint()?;
                out.push_str("</g>");
                return out.checkpoint();
            }
            crate::c4::C4NodeShape::Person => {
                let head_radius = (width * 0.23).clamp(16.0, 56.0);
                let overlap = head_radius * 0.27;
                let body_height = (height - (2.0 * head_radius - overlap)).max(1.0);
                let body_radius = (width * 0.177).min(body_height * 0.45);
                let total_height = body_height + 2.0 * head_radius - overlap;
                let top = -total_height / 2.0;
                let body_top = top + 2.0 * head_radius - overlap;
                // The path helper is centered, so translate the body into the person silhouette
                // after generation instead of baking the offset into the shape's layout box.
                let body_pair = c4_hand_drawn_paths(
                    &c4_rounded_rect_path_d(width, body_height, body_radius),
                    fill,
                    stroke,
                    2.0,
                    work_meter,
                    randomness,
                )?;
                let head_pair = roughjs_paths_for_circle(
                    head_radius * 2.0,
                    fill,
                    stroke,
                    2.0,
                    "0 0",
                    true,
                    work_meter,
                    randomness,
                )
                .ok_or_else(|| crate::Error::InvalidModel {
                    message:
                        "c4: handDrawn person colors must be representable by the RoughJS adapter"
                            .to_string(),
                })?;
                out.push_str(r#"<g class="basic label-container">"#);
                out.checkpoint()?;
                let _ = write!(out, r#"<g transform="translate(0,{})">"#, fmt(body_top));
                out.checkpoint()?;
                c4_write_hand_drawn_pair(out, &body_pair, fill, stroke, 2.0)?;
                out.push_str("</g>");
                let _ = write!(
                    out,
                    r#"<g transform="translate(0,{})">"#,
                    fmt(top + head_radius)
                );
                out.checkpoint()?;
                c4_write_hand_drawn_pair(out, &head_pair, fill, stroke, 2.0)?;
                out.push_str("</g></g>");
                return out.checkpoint();
            }
            crate::c4::C4NodeShape::Cylinder => {
                let rx = width / 2.0;
                let ry = rx / (2.5 + width / 50.0);
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
                let pair = c4_hand_drawn_paths(&path, fill, stroke, 2.0, work_meter, randomness)?;
                out.push_str(r#"<g class="basic label-container" transform="translate("#);
                let _ = write!(
                    out,
                    "{}, {})\">",
                    fmt(-width / 2.0),
                    fmt(-(body_height / 2.0 + ry))
                );
                out.checkpoint()?;
                c4_write_hand_drawn_pair(out, &pair, fill, stroke, 2.0)?;
                out.push_str("</g>");
                return out.checkpoint();
            }
            crate::c4::C4NodeShape::HorizontalCylinder => {
                let h = height.max(1.0);
                let ry = h / 2.0;
                let rx = ry / (2.5 + h / 50.0);
                let path = format!(
                    "M0,0 a{},{} 0,0,1 0,-{} l{},0 a{},{} 0,0,1 0,{} M{},-{} a{},{} 0,0,0 0,{} l-{},0",
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
                    fmt(width),
                );
                let pair = c4_hand_drawn_paths(&path, fill, stroke, 2.0, work_meter, randomness)?;
                out.push_str(r#"<g class="basic label-container" transform="translate("#);
                let _ = write!(out, "{}, {})\">", fmt(-width / 2.0), fmt(h / 2.0));
                out.checkpoint()?;
                c4_write_hand_drawn_pair(out, &pair, fill, stroke, 2.0)?;
                out.push_str("</g>");
                return out.checkpoint();
            }
        }
    }

    let shape_style = |with_radius: bool| {
        if with_radius {
            format!(
                "fill:{} !important;stroke:{} !important;rx:12px !important;ry:12px !important",
                fill, stroke
            )
        } else {
            format!("fill:{} !important;stroke:{} !important", fill, stroke)
        }
    };
    match node_shape {
        crate::c4::C4NodeShape::Rounded => {
            let _ = write!(
                out,
                r#"<rect class="basic label-container" style="{}" rx="5" ry="5" x="{}" y="{}" width="{}" height="{}"/>"#,
                escape_attr(&shape_style(true)),
                fmt(-width / 2.0),
                fmt(-height / 2.0),
                fmt(width),
                fmt(height),
            );
        }
        crate::c4::C4NodeShape::Framed => {
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
        crate::c4::C4NodeShape::Person => {
            let head_radius = (width * 0.23).clamp(16.0, 56.0);
            let overlap = head_radius * 0.27;
            let body_height = (height - (2.0 * head_radius - overlap)).max(1.0);
            let body_radius = (width * 0.177).min(body_height * 0.45);
            let total_height = body_height + 2.0 * head_radius - overlap;
            let top = -total_height / 2.0;
            let body_top = top + 2.0 * head_radius - overlap;
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
                fmt(top + head_radius),
                fmt(head_radius),
                escape_attr(&shape_style(false))
            );
        }
        crate::c4::C4NodeShape::Cylinder => {
            let rx = width / 2.0;
            let ry = rx / (2.5 + width / 50.0);
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
        crate::c4::C4NodeShape::HorizontalCylinder => {
            let h = height.max(1.0);
            let ry = h / 2.0;
            let rx = ry / (2.5 + h / 50.0);
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

pub(crate) fn render_c4_diagram_svg_typed(
    layout: &crate::model::C4DiagramLayout,
    model: &C4DiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    typography_theme: &crate::c4::C4TypographyThemePlan,
    cluster_theme: &crate::c4::C4ClusterThemePlan,
    text_paint: &crate::c4::C4TextPaintPlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");

    let c4_cfg = C4ConfigView::new(effective_config);
    let look = c4_cfg.look();
    let diagram_margin_x = c4_cfg.diagram_margin_x();
    let diagram_margin_y = c4_cfg.diagram_margin_y();
    let use_max_width = layout.use_max_width;

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
    let mut typography_receipt = typography_theme.begin_terminal_receipt();
    let mut text_paint_receipt = text_paint.begin_terminal_receipt();
    let extra_vert_for_title = if title.is_some() { 60.0 } else { 0.0 };

    let viewbox_x = bounds.min_x - diagram_margin_x;
    let viewbox_y = -(diagram_margin_y + extra_vert_for_title);
    let hand_drawn_outset = if look.is_hand_drawn() { 8.0 } else { 0.0 };
    let root_width = width + 2.0 * hand_drawn_outset;
    let root_height = height + extra_vert_for_title + 2.0 * hand_drawn_outset;

    let aria_roledescription = "c4";

    let aria_describedby = model
        .acc_descr
        .as_ref()
        .map(|s| s.trim_end_matches('\n'))
        .filter(|s| !s.trim().is_empty())
        .map(|_| format!("chart-desc-{diagram_id}"));
    let aria_labelledby = model
        .acc_title
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|_| format!("chart-title-{diagram_id}"));

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let root_bounds = root_svg::DiagramBounds::from_view_box(
        viewbox_x - hand_drawn_outset,
        viewbox_y - hand_drawn_outset,
        root_width,
        root_height,
    );
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, use_max_width)
        .with_max_width(root_svg::RootMaxWidth::SvgNumber(root_width));
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, aria_roledescription);
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let root_document = root_svg::RootViewportContext::new(crate::DiagramFamilyId::C4, diagram_id)
        .write_open(&mut out, root_spec, root_chrome)?;
    options.checkpoint_emit()?;

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
    if let Some(descr) = model
        .acc_descr
        .as_deref()
        .map(|s| s.trim_end_matches('\n'))
        .filter(|s| !s.trim().is_empty())
    {
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
    let css_emission = write_c4_css_with_typography(
        &mut out,
        diagram_id,
        effective_config,
        Some(typography_theme),
    )?;
    if let Some(receipt) = typography_receipt.as_mut() {
        receipt.record_css_emission(
            &css_emission.font_family,
            &css_emission.font_size_css,
            css_emission.base_typography_emitted,
            css_emission.root_typography_emitted,
        );
    }
    // Preserve the root inheritance used by source-directed colors as well as the title.
    if let Some(fill) = text_paint.fill_css() {
        let _ = write!(&mut out, "#{}{{fill:{};}}", diagram_id, fill);
        out.checkpoint()?;
    }
    if let Some(receipt) = text_paint_receipt.as_mut() {
        receipt.record_css(text_paint.fill_css());
    }
    out.push_str("</style>");
    out.checkpoint()?;
    out.push_str("<g/>");
    out.checkpoint()?;
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
    let mut rel_meta: std::collections::HashMap<(&str, &str), &C4SvgModelRel> =
        std::collections::HashMap::new();
    for r in &model.rels {
        rel_meta.insert((r.from_alias.as_str(), r.to_alias.as_str()), r);
    }
    let mut cluster_theme_receipt = cluster_theme.begin_terminal_receipt();
    let mut boundary_emission_ordinal = 0usize;
    let hand_drawn_randomness = options.rough_randomness(
        effective_config
            .get("handDrawnSeed")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0),
        "render.c4.roughjs",
    );

    for item in c4_paint_order(layout)? {
        options.checkpoint_emit()?;
        match item {
            C4PaintItem::Shape(index) => {
                let s = &layout.shapes[index];
                let meta = shape_meta.get(s.alias.as_str()).copied();
                let (default_bg_color, default_border_color) =
                    if s.type_c4_shape.starts_with("external_") {
                        ("#999999", "#8A8A8A")
                    } else {
                        ("#08427B", "#073B6F")
                    };
                let bg_color = meta.and_then(|m| m.bg_color.clone()).unwrap_or_else(|| {
                    c4_cfg.color(&format!("{}_bg_color", s.type_c4_shape), default_bg_color)
                });
                let border_color = meta
                    .and_then(|m| m.border_color.clone())
                    .unwrap_or_else(|| {
                        c4_cfg.color(
                            &format!("{}_border_color", s.type_c4_shape),
                            default_border_color,
                        )
                    });
                let font_color = meta
                    .and_then(|m| m.font_color.clone())
                    .unwrap_or_else(|| "#FFFFFF".to_string());
                let Some(meta) = meta else {
                    return Err(crate::Error::InvalidModel {
                        message: format!("c4: missing model shape {}", s.alias),
                    });
                };
                let node_shape = crate::c4::c4_node_shape(meta);
                let classes = c4_shape_classes(s.type_c4_shape.as_str());
                let _ = write!(
                    &mut out,
                    r#"<g transform="translate({}, {})"><g id="{}" class="node {}""#,
                    fmt(s.x + s.width / 2.0),
                    fmt(s.y + s.height / 2.0),
                    escape_attr_display(scoped_svg_id(diagram_id, &s.alias)),
                    classes,
                );
                if look != crate::c4::C4Look::Classic {
                    let _ = write!(out, r#" data-look="{}""#, look.as_str());
                }
                out.push_str(">");
                out.checkpoint()?;

                c4_write_unified_shape(
                    &mut out,
                    s,
                    node_shape,
                    &bg_color,
                    &border_color,
                    look,
                    options.work_meter(),
                    &hand_drawn_randomness,
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
                let padding = c4_cfg.layout_settings().c4_shape_padding;
                let label_transform = match node_shape {
                    crate::c4::C4NodeShape::Person => {
                        let head_radius = (s.width * 0.23).clamp(16.0, 56.0);
                        let overlap = head_radius * 0.27;
                        let body_height = (s.height - 2.0 * head_radius + overlap).max(1.0);
                        let body_top = -s.height / 2.0 + 2.0 * head_radius - overlap;
                        (
                            -total_width / 2.0,
                            body_top + body_height / 2.0 - total_height / 2.0,
                        )
                    }
                    crate::c4::C4NodeShape::Cylinder => {
                        (-total_width / 2.0, -total_height / 2.0 + padding / 1.5)
                    }
                    crate::c4::C4NodeShape::HorizontalCylinder => {
                        let ry = s.height / 2.0;
                        let rx = ry / (2.5 + s.height / 50.0);
                        (-total_width / 2.0 - rx, -total_height / 2.0)
                    }
                    crate::c4::C4NodeShape::Rounded | crate::c4::C4NodeShape::Framed => {
                        (-total_width / 2.0, -total_height / 2.0)
                    }
                };
                let _ = write!(
                    &mut out,
                    r#"<g class="label" style="color:{} !important" transform="translate({}, {})"><rect/>"#,
                    escape_attr(&font_color),
                    fmt(label_transform.0),
                    fmt(label_transform.1),
                );
                out.checkpoint()?;
                let mut section_y = 0.0;
                let mut shape_text_visible = false;
                for (class, block) in sections {
                    if let Some(block) = block {
                        shape_text_visible |= c4_write_unified_section(
                            &mut out,
                            class,
                            block,
                            total_width,
                            section_y,
                            &font_color,
                        );
                        out.checkpoint()?;
                        section_y += block.height + 3.0;
                    }
                }
                out.push_str("</g></g></g>");
                out.checkpoint()?;
                if shape_text_visible && let Some(receipt) = text_paint_receipt.as_mut() {
                    receipt.record_owned_color(&font_color);
                }
            }
            C4PaintItem::Boundary(index) => {
                let b = &layout.boundaries[index];
                let meta = boundary_meta.get(b.alias.as_str()).copied();
                let is_node_type = meta.and_then(|m| m.node_type.as_deref()).is_some();
                let Some((fill_color, stroke_color, boundary_radius_attr)) =
                    cluster_theme.boundary_tokens(&b.alias)
                else {
                    return Err(crate::Error::InvalidModel {
                        message: format!("C4 cluster theme plan is missing boundary `{}`", b.alias),
                    });
                };

                out.push_str("<g>");
                out.checkpoint()?;
                if is_node_type {
                    let _ = write!(
                        &mut out,
                        r#"<rect x="{}" y="{}" fill="{}" stroke="{}" width="{}" height="{}" rx="{}" ry="{}" stroke-width="1"/>"#,
                        fmt(b.x),
                        fmt(b.y),
                        escape_attr(fill_color),
                        escape_attr(stroke_color),
                        fmt(b.width),
                        fmt(b.height),
                        boundary_radius_attr,
                        boundary_radius_attr,
                    );
                } else {
                    let _ = write!(
                        &mut out,
                        r#"<rect x="{}" y="{}" fill="{}" stroke="{}" width="{}" height="{}" rx="{}" ry="{}" stroke-width="1" stroke-dasharray="7.0,7.0"/>"#,
                        fmt(b.x),
                        fmt(b.y),
                        escape_attr(fill_color),
                        escape_attr(stroke_color),
                        fmt(b.width),
                        fmt(b.height),
                        boundary_radius_attr,
                        boundary_radius_attr,
                    );
                }
                out.checkpoint()?;
                if let Some(receipt) = cluster_theme_receipt.as_mut() {
                    receipt.record_checkpointed_boundary(
                        boundary_emission_ordinal,
                        &b.alias,
                        fill_color,
                        stroke_color,
                        boundary_radius_attr,
                        boundary_radius_attr,
                    );
                }
                boundary_emission_ordinal = boundary_emission_ordinal.saturating_add(1);

                let boundary_font = c4_cfg.boundary_font();
                let boundary_family = boundary_font
                    .font_family
                    .as_deref()
                    .unwrap_or(C4_DEFAULT_FONT_FAMILY);
                let boundary_weight = "bold";
                let boundary_size = boundary_font.font_size + 2.0;
                let mut boundary_text_visible = c4_write_text_by_tspan(
                    &mut out,
                    C4TspanText {
                        content: &b.label.text,
                        x: b.x,
                        y: b.y + b.label.y,
                        width: b.width,
                        font_family: boundary_family,
                        font_size: boundary_size,
                        font_weight: boundary_weight,
                        attrs: &[("fill", "#444444")],
                    },
                )?;
                if let Some(ty) = &b.ty
                    && !ty.text.trim().is_empty()
                {
                    let boundary_type_weight =
                        boundary_font.font_weight.as_deref().unwrap_or("normal");
                    let boundary_type_size = boundary_font.font_size;
                    boundary_text_visible |= c4_write_text_by_tspan(
                        &mut out,
                        C4TspanText {
                            content: &ty.text,
                            x: b.x,
                            y: b.y + ty.y,
                            width: b.width,
                            font_family: boundary_family,
                            font_size: boundary_type_size,
                            font_weight: boundary_type_weight,
                            attrs: &[("fill", "#444444")],
                        },
                    )?;
                }
                if let Some(descr) = &b.descr
                    && !descr.text.trim().is_empty()
                {
                    let descr_weight = boundary_font.font_weight.as_deref().unwrap_or("normal");
                    let descr_size = (boundary_font.font_size - 2.0).max(1.0);
                    boundary_text_visible |= c4_write_text_by_tspan(
                        &mut out,
                        C4TspanText {
                            content: &descr.text,
                            x: b.x,
                            y: b.y + descr.y,
                            width: b.width,
                            font_family: boundary_family,
                            font_size: descr_size,
                            font_weight: descr_weight,
                            attrs: &[("fill", "#444444")],
                        },
                    )?;
                }

                out.push_str("</g>");
                out.checkpoint()?;
                if boundary_text_visible && let Some(receipt) = text_paint_receipt.as_mut() {
                    receipt.record_owned_color("#444444");
                }
            }
        }
    }
    let arrowhead_url = scoped_svg_url(diagram_id, "arrowhead");
    let arrowend_url = scoped_svg_url(diagram_id, "arrowend");
    write_c4_relation_defs(&mut out, diagram_id)?;

    out.push_str("<g>");
    out.checkpoint()?;
    for (idx, rel) in layout.rels.iter().enumerate() {
        let meta = rel_meta.get(&(rel.from.as_str(), rel.to.as_str())).copied();
        let text_color = meta
            .and_then(|m| m.text_color.clone())
            .unwrap_or_else(|| "#444444".to_string());
        let stroke_color = meta
            .and_then(|m| m.line_color.clone())
            .unwrap_or_else(|| "#444444".to_string());
        let offset_x = rel.offset_x.unwrap_or(0) as f64;
        let offset_y = rel.offset_y.unwrap_or(0) as f64;

        if idx == 0 {
            let _ = write!(
                &mut out,
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke-width="1" stroke="{}""#,
                fmt(rel.start_point.x),
                fmt(rel.start_point.y),
                fmt(rel.end_point.x),
                fmt(rel.end_point.y),
                escape_attr(&stroke_color)
            );
            out.checkpoint()?;
            if rel.rel_type != "rel_b" {
                let _ = write!(
                    &mut out,
                    r#" marker-end="{}""#,
                    escape_attr_display(arrowhead_url)
                );
                out.checkpoint()?;
            }
            if rel.rel_type == "birel" || rel.rel_type == "rel_b" {
                let _ = write!(
                    &mut out,
                    r#" marker-start="{}""#,
                    escape_attr_display(arrowend_url)
                );
                out.checkpoint()?;
            }
            out.push_str(r#" style="fill: none;"/>"#);
            out.checkpoint()?;
        } else {
            let cx = rel.start_point.x + (rel.end_point.x - rel.start_point.x) / 2.0
                - (rel.end_point.x - rel.start_point.x) / 4.0;
            let cy = rel.start_point.y + (rel.end_point.y - rel.start_point.y) / 2.0;
            let d = format!(
                "M{} {} Q{} {} {} {}",
                fmt(rel.start_point.x),
                fmt(rel.start_point.y),
                fmt(cx),
                fmt(cy),
                fmt(rel.end_point.x),
                fmt(rel.end_point.y)
            );
            let _ = write!(
                &mut out,
                r#"<path fill="none" stroke-width="1" stroke="{}" d="{}""#,
                escape_attr(&stroke_color),
                escape_attr(&d)
            );
            out.checkpoint()?;
            if rel.rel_type != "rel_b" {
                let _ = write!(
                    &mut out,
                    r#" marker-end="{}""#,
                    escape_attr_display(arrowhead_url)
                );
                out.checkpoint()?;
            }
            if rel.rel_type == "birel" || rel.rel_type == "rel_b" {
                let _ = write!(
                    &mut out,
                    r#" marker-start="{}""#,
                    escape_attr_display(arrowend_url)
                );
                out.checkpoint()?;
            }
            out.push_str("/>");
            out.checkpoint()?;
        }
        options.checkpoint_emit()?;

        let midx = rel.start_point.x.min(rel.end_point.x)
            + (rel.end_point.x - rel.start_point.x).abs() / 2.0
            + offset_x;
        let midy = rel.start_point.y.min(rel.end_point.y)
            + (rel.end_point.y - rel.start_point.y).abs() / 2.0
            + offset_y;

        let message_font = c4_cfg.message_font();
        let message_family = message_font
            .font_family
            .as_deref()
            .unwrap_or(C4_DEFAULT_FONT_FAMILY);
        let message_weight = message_font.font_weight.as_deref().unwrap_or("normal");
        let message_size = message_font.font_size;
        let label_visible = c4_write_text_by_tspan(
            &mut out,
            C4TspanText {
                content: &rel.label.text,
                x: midx,
                y: midy,
                width: rel.label.width,
                font_family: message_family,
                font_size: message_size,
                font_weight: message_weight,
                attrs: &[("fill", &text_color)],
            },
        )?;
        if label_visible && let Some(receipt) = text_paint_receipt.as_mut() {
            receipt.record_owned_color(&text_color);
        }

        if let Some(techn) = &rel.techn
            && !techn.text.trim().is_empty()
        {
            let techn_text = format!("[{}]", techn.text);
            let techn_visible = c4_write_text_by_tspan(
                &mut out,
                C4TspanText {
                    content: &techn_text,
                    x: midx,
                    y: midy + message_size + 5.0,
                    width: rel.label.width.max(techn.width),
                    font_family: message_family,
                    font_size: message_size,
                    font_weight: message_weight,
                    attrs: &[("fill", &text_color), ("font-style", "italic")],
                },
            )?;
            if techn_visible && let Some(receipt) = text_paint_receipt.as_mut() {
                receipt.record_owned_color(&text_color);
            }
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;

    if let Some(title) = title {
        let title_x = (width - 2.0 * diagram_margin_x) / 2.0 - 4.0 * diagram_margin_x;
        let title_y = bounds.min_y + diagram_margin_y;
        let _ = write!(
            &mut out,
            r#"<text x="{}" y="{}""#,
            fmt(title_x),
            fmt(title_y),
        );
        if let Some(fill) = text_paint.fill_css() {
            let _ = write!(&mut out, r#" fill="{}""#, escape_attr(fill));
        }
        let _ = write!(&mut out, ">{}</text>", escape_xml(&title));
        out.checkpoint()?;
        if let Some(receipt) = text_paint_receipt.as_mut() {
            receipt.record_title(&title, text_paint.fill_css());
        }
        if let Some(receipt) = typography_receipt.as_mut() {
            receipt.record_title_text(&title);
        }
    }

    out.push_str("</svg>");
    options.checkpoint_emit()?;
    let rooted_svg = root_document.complete(out.finish()?)?;
    if cluster_theme_receipt.is_some_and(|receipt| !cluster_theme.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "C4 cluster theme receipt did not match the terminal SVG".to_string(),
        });
    }
    if typography_receipt.is_some_and(|receipt| !typography_theme.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "C4 typography receipt did not match the terminal SVG".to_string(),
        });
    }
    if let Some(receipt) = text_paint_receipt
        && !text_paint.record_terminal(receipt)
    {
        return Err(crate::Error::InvalidModel {
            message: "C4 text paint receipt did not match the terminal SVG".to_string(),
        });
    }
    Ok(rooted_svg)
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

        fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> crate::Result<()> {
            self.retained.replace_range(range, replacement);
            Ok(())
        }

        fn checkpoint(&mut self) -> crate::Result<()> {
            if self.rejected {
                Err(crate::Error::InvalidModel {
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
                x: 0.0,
                y: 0.0,
                width: 100.0,
                font_family: "sans-serif",
                font_size: 16.0,
                font_weight: "normal",
                attrs: &[("fill", "#444444")],
            },
        )
        .expect_err("the rejecting sink must stop C4 text emission");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
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
            &json!({
                "themeVariables": {
                    "personBorder": "#112233",
                    "personBkg": "#445566",
                    "textColor": "#778899",
                    "nodeBorder": "#aabbcc",
                    "strokeWidth": 2
                }
            }),
            None,
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
            &json!({
                "fontFamily": authored_font_family,
            }),
            None,
        )
        .expect("write C4 CSS");

        assert!(css.contains(authored_font_family));
    }
}
