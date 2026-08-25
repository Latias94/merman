use super::super::*;
use crate::c4::{C4_DEFAULT_FONT_FAMILY, C4ConfigView};
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

fn write_c4_css(
    out: &mut impl SvgOutput,
    diagram_id: impl std::fmt::Display + Copy,
    effective_config: &serde_json::Value,
) -> Result<()> {
    let parts = info_css_parts_with_config(diagram_id, effective_config);
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
    out.push_str(&parts.root_rule);
    out.checkpoint()
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

fn c4_write_text_by_tspan(out: &mut impl SvgOutput, text: C4TspanText<'_>) -> Result<()> {
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
    }

    Ok(())
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

pub(crate) fn render_c4_diagram_svg_typed(
    layout: &crate::model::C4DiagramLayout,
    model: &C4DiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    cluster_theme: &crate::c4::C4ClusterThemePlan,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");

    let c4_cfg = C4ConfigView::new(effective_config);
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
    let extra_vert_for_title = if title.is_some() { 60.0 } else { 0.0 };

    let viewbox_x = bounds.min_x - diagram_margin_x;
    let viewbox_y = -(diagram_margin_y + extra_vert_for_title);

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
        viewbox_x,
        viewbox_y,
        width,
        height + extra_vert_for_title,
    );
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, use_max_width)
        .with_max_width(root_svg::RootMaxWidth::SvgNumber(width));
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
    write_c4_css(&mut out, diagram_id, effective_config)?;
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
                let shape_font = c4_cfg.shape_font(&s.type_c4_shape);

                out.push_str(r#"<g class="person-man">"#);
                out.checkpoint()?;

                match s.type_c4_shape.as_str() {
                    "system_db"
                    | "external_system_db"
                    | "container_db"
                    | "external_container_db"
                    | "component_db"
                    | "external_component_db" => {
                        let half = s.width / 2.0;
                        let d1 = format!(
                            "M{},{}c0,-10 {},-10 {},-10c0,0 {},0 {},10l0,{}c0,10 -{},10 -{},10c0,0 -{},0 -{},-10l0,-{}",
                            fmt(s.x),
                            fmt(s.y),
                            fmt(half),
                            fmt(half),
                            fmt(half),
                            fmt(half),
                            fmt(s.height),
                            fmt(half),
                            fmt(half),
                            fmt(half),
                            fmt(half),
                            fmt(s.height)
                        );
                        let d2 = format!(
                            "M{},{}c0,10 {},10 {},10c0,0 {},0 {},-10",
                            fmt(s.x),
                            fmt(s.y),
                            fmt(half),
                            fmt(half),
                            fmt(half),
                            fmt(half)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<path fill="{}" stroke-width="0.5" stroke="{}" d="{}"/>"#,
                            escape_attr(&bg_color),
                            escape_attr(&border_color),
                            escape_attr(&d1)
                        );
                        out.checkpoint()?;
                        let _ = write!(
                            &mut out,
                            r#"<path fill="none" stroke-width="0.5" stroke="{}" d="{}"/>"#,
                            escape_attr(&border_color),
                            escape_attr(&d2)
                        );
                        out.checkpoint()?;
                    }
                    "system_queue"
                    | "external_system_queue"
                    | "container_queue"
                    | "external_container_queue"
                    | "component_queue"
                    | "external_component_queue" => {
                        let half = s.height / 2.0;
                        let d1 = format!(
                            "M{},{}l{},0c5,0 5,{} 5,{}c0,0 0,{} -5,{}l-{},0c-5,0 -5,-{} -5,-{}c0,0 0,-{} 5,-{}",
                            fmt(s.x),
                            fmt(s.y),
                            fmt(s.width),
                            fmt(half),
                            fmt(half),
                            fmt(half),
                            fmt(half),
                            fmt(s.width),
                            fmt(half),
                            fmt(half),
                            fmt(half),
                            fmt(half),
                        );
                        let d2 = format!(
                            "M{},{}c-5,0 -5,{} -5,{}c0,{} 5,{} 5,{}",
                            fmt(s.x + s.width),
                            fmt(s.y),
                            fmt(half),
                            fmt(half),
                            fmt(half),
                            fmt(half),
                            fmt(half)
                        );
                        let _ = write!(
                            &mut out,
                            r#"<path fill="{}" stroke-width="0.5" stroke="{}" d="{}"/>"#,
                            escape_attr(&bg_color),
                            escape_attr(&border_color),
                            escape_attr(&d1)
                        );
                        out.checkpoint()?;
                        let _ = write!(
                            &mut out,
                            r#"<path fill="none" stroke-width="0.5" stroke="{}" d="{}"/>"#,
                            escape_attr(&border_color),
                            escape_attr(&d2)
                        );
                        out.checkpoint()?;
                    }
                    _ => {
                        let _ = write!(
                            &mut out,
                            r#"<rect x="{}" y="{}" fill="{}" stroke="{}" width="{}" height="{}" rx="2.5" ry="2.5" stroke-width="0.5"/>"#,
                            fmt(s.x),
                            fmt(s.y),
                            escape_attr(&bg_color),
                            escape_attr(&border_color),
                            fmt(s.width),
                            fmt(s.height)
                        );
                        out.checkpoint()?;
                    }
                }

                let mut type_font = shape_font.clone();
                type_font.font_size -= 2.0;
                let type_family = type_font
                    .font_family
                    .as_deref()
                    .unwrap_or(C4_DEFAULT_FONT_FAMILY);
                let type_size = type_font.font_size;
                let type_text_length = s.type_block.width.round().max(0.0);
                let _ = write!(
                    &mut out,
                    r#"<text fill="{}" font-family="{}" font-size="{}" font-style="italic" lengthAdjust="spacing" textLength="{}" x="{}" y="{}">{}</text>"#,
                    escape_attr(&font_color),
                    escape_attr(type_family),
                    fmt(type_size.max(1.0)),
                    fmt(type_text_length),
                    fmt(s.x + s.width / 2.0 - type_text_length / 2.0),
                    fmt(s.y + s.type_block.y),
                    escape_xml(&format!("<<{}>>", s.type_c4_shape))
                );
                out.checkpoint()?;

                if matches!(s.type_c4_shape.as_str(), "person" | "external_person") {
                    let href = if s.type_c4_shape == "external_person" {
                        C4_EXTERNAL_PERSON_IMG
                    } else {
                        C4_PERSON_IMG
                    };
                    let _ = write!(
                        &mut out,
                        r#"<image width="48" height="48" x="{}" y="{}" xlink:href="{}"/>"#,
                        fmt(s.x + s.width / 2.0 - 24.0),
                        fmt(s.y + s.image.y),
                        escape_attr(href)
                    );
                    out.checkpoint()?;
                }

                let label_family = shape_font
                    .font_family
                    .as_deref()
                    .unwrap_or(C4_DEFAULT_FONT_FAMILY);
                let label_weight = "bold";
                let label_size = shape_font.font_size + 2.0;
                c4_write_text_by_tspan(
                    &mut out,
                    C4TspanText {
                        content: &s.label.text,
                        x: s.x,
                        y: s.y + s.label.y,
                        width: s.width,
                        font_family: label_family,
                        font_size: label_size,
                        font_weight: label_weight,
                        attrs: &[("fill", &font_color)],
                    },
                )?;

                let body_family = shape_font
                    .font_family
                    .as_deref()
                    .unwrap_or(C4_DEFAULT_FONT_FAMILY);
                let body_weight = shape_font.font_weight.as_deref().unwrap_or("normal");
                let body_size = shape_font.font_size;

                if let Some(techn) = &s.techn {
                    if !techn.text.trim().is_empty() {
                        c4_write_text_by_tspan(
                            &mut out,
                            C4TspanText {
                                content: &techn.text,
                                x: s.x,
                                y: s.y + techn.y,
                                width: s.width,
                                font_family: body_family,
                                font_size: body_size,
                                font_weight: body_weight,
                                attrs: &[("fill", &font_color), ("font-style", "italic")],
                            },
                        )?;
                    }
                } else if let Some(ty) = &s.ty
                    && !ty.text.trim().is_empty()
                {
                    c4_write_text_by_tspan(
                        &mut out,
                        C4TspanText {
                            content: &ty.text,
                            x: s.x,
                            y: s.y + ty.y,
                            width: s.width,
                            font_family: body_family,
                            font_size: body_size,
                            font_weight: body_weight,
                            attrs: &[("fill", &font_color), ("font-style", "italic")],
                        },
                    )?;
                }

                if let Some(descr) = &s.descr
                    && !descr.text.trim().is_empty()
                {
                    let descr_font = c4_cfg.shape_font("person");
                    let descr_family = descr_font
                        .font_family
                        .as_deref()
                        .unwrap_or(C4_DEFAULT_FONT_FAMILY);
                    let descr_weight = descr_font.font_weight.as_deref().unwrap_or("normal");
                    let descr_size = descr_font.font_size;
                    c4_write_text_by_tspan(
                        &mut out,
                        C4TspanText {
                            content: &descr.text,
                            x: s.x,
                            y: s.y + descr.y,
                            width: s.width,
                            font_family: descr_family,
                            font_size: descr_size,
                            font_weight: descr_weight,
                            attrs: &[("fill", &font_color)],
                        },
                    )?;
                }

                out.push_str("</g>");
                out.checkpoint()?;
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
                c4_write_text_by_tspan(
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
                    c4_write_text_by_tspan(
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
                    c4_write_text_by_tspan(
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
        c4_write_text_by_tspan(
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

        if let Some(techn) = &rel.techn
            && !techn.text.trim().is_empty()
        {
            let techn_text = format!("[{}]", techn.text);
            c4_write_text_by_tspan(
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
        }
    }
    out.push_str("</g>");
    out.checkpoint()?;

    if let Some(title) = title {
        let title_x = (width - 2.0 * diagram_margin_x) / 2.0 - 4.0 * diagram_margin_x;
        let title_y = bounds.min_y + diagram_margin_y;
        let _ = write!(
            &mut out,
            r#"<text x="{}" y="{}">{}</text>"#,
            fmt(title_x),
            fmt(title_y),
            escape_xml(&title)
        );
        out.checkpoint()?;
    }

    out.push_str("</svg>");
    options.checkpoint_emit()?;
    let rooted_svg = root_document.complete(out.finish()?)?;
    if cluster_theme_receipt.is_some_and(|receipt| !cluster_theme.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "C4 cluster theme receipt did not match the terminal SVG".to_string(),
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
        write_c4_css(
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
        write_c4_css(
            &mut css,
            "c4",
            &json!({
                "fontFamily": authored_font_family,
            }),
        )
        .expect("write C4 CSS");

        assert!(css.contains(authored_font_family));
    }
}
