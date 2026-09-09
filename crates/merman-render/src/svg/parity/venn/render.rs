use super::super::roughjs_common::ops_to_svg_path_d;
use super::super::theme::VennTheme;
use super::super::*;
use crate::venn::rough::{circle_geometry, intersection_fill_geometry};
use crate::venn::{
    VennStrokeWidth, venn_area_label, venn_area_presentation, venn_stable_sets_key,
    venn_style_by_key, venn_style_value, venn_title,
};
use merman_core::diagrams::venn::VennDiagramRenderModel;
use merman_core::theme_color::transparentize;
use std::collections::HashMap;
use std::str::FromStr as _;

fn escape_css_attr(value: &str) -> String {
    escape_attr(value)
}

fn data_sets_attr(sets: &[String]) -> String {
    sets.join("_")
}

fn rough_color(value: &str) -> Result<roughr::Srgba> {
    let color = roughr::Color::from_str(value.trim()).map_err(|error| Error::InvalidModel {
        message: format!("invalid Venn RoughJS color `{value}`: {error}"),
    })?;
    Ok(roughr::Srgba::new(
        color.red as f32 / 255.0,
        color.green as f32 / 255.0,
        color.blue as f32 / 255.0,
        color.alpha as f32 / 255.0,
    ))
}

fn write_area_label(
    out: &mut String,
    area: &crate::model::VennAreaLayout,
    font_size: f64,
    text_color: &str,
) {
    let _ = write!(
        out,
        r#"<text class="label" text-anchor="middle" dy=".35em" x="{x}" y="{y}" style="font-size: {font_size}px; fill: {text_fill};"><tspan x="{x}" y="{y}" dy="0.35em">{label}</tspan></text>"#,
        x = fmt(area.text_x),
        y = fmt(area.text_y),
        font_size = fmt(font_size),
        text_fill = escape_css_attr(text_color),
        label = escape_xml(venn_area_label(area)),
    );
}

fn stroke_width_css(stroke_width: &VennStrokeWidth) -> String {
    match stroke_width {
        VennStrokeWidth::Css(value) => value.clone(),
        VennStrokeWidth::LogicalPixels(value) => fmt_string(*value),
    }
}

fn root_open(
    out: &mut String,
    diagram_id: SvgDiagramId<'_>,
    layout: &VennDiagramLayout,
    aria_labelledby: Option<&str>,
    aria_describedby: Option<&str>,
) -> Result<root_svg::RootDocument> {
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "venn");
    root_chrome.aria_labelledby = aria_labelledby;
    root_chrome.aria_describedby = aria_describedby;
    root_chrome.dom = root_svg::RootDomProfile {
        fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
        fixed_style_placement: root_svg::RootStylePlacement::Tail,
        trailing_newline: false,
        ..root_svg::RootDomProfile::default()
    };
    root_svg::RootViewportContext::new(crate::family::RenderFamilyKind::Venn, diagram_id)
        .write_open(
            out,
            root_svg::RootViewportSpec::mermaid(
                root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.width, layout.height),
                layout.use_max_width,
            ),
            root_chrome,
        )
}

fn venn_css<I>(diagram_id: I, theme: &VennTheme) -> String
where
    I: Copy + std::fmt::Display,
{
    format!(
        "#{diagram_id} .venn-title{{font-size:32px;fill:{title_color};font-family:{font_family};}}\
#{diagram_id} .venn-circle text{{font-size:48px;font-family:{font_family};}}\
#{diagram_id} .venn-intersection text{{font-size:48px;fill:{set_text_color};font-family:{font_family};}}\
#{diagram_id} .venn-text-node{{font-family:{font_family};color:{set_text_color};}}",
        title_color = theme.title_color,
        font_family = theme.font_family_css,
        set_text_color = theme.set_text_color,
    )
}

pub(crate) fn render_venn_diagram_svg_model(
    layout: &VennDiagramLayout,
    model: &VennDiagramRenderModel,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("venn");
    let title = venn_title(model, diagram_title);
    let has_acc_title = model
        .acc_title
        .as_deref()
        .is_some_and(|title| !title.trim().is_empty());
    let has_acc_descr = model
        .acc_descr
        .as_deref()
        .is_some_and(|descr| !descr.trim().is_empty());
    let aria_labelledby = has_acc_title.then(|| format!("chart-title-{diagram_id}"));
    let aria_describedby = has_acc_descr.then(|| format!("chart-desc-{diagram_id}"));

    let mut out = String::new();
    let root_document = root_open(
        &mut out,
        diagram_id,
        layout,
        aria_labelledby.as_deref(),
        aria_describedby.as_deref(),
    )?;
    options.checkpoint_emit()?;

    if has_acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id,
            text = escape_xml(model.acc_title.as_deref().unwrap_or_default())
        );
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id,
            text = escape_xml(model.acc_descr.as_deref().unwrap_or_default())
        );
    }

    let theme = PresentationTheme::new(effective_config).venn()?;
    let css = venn_css(diagram_id, &theme);
    let _ = write!(&mut out, r#"<style>{css}</style>"#);
    out.push_str("<g/>");
    options.checkpoint_emit()?;

    if let Some(title) = title {
        let _ = write!(
            &mut out,
            r#"<text class="venn-title" font-size="{font_size}px" text-anchor="middle" dominant-baseline="middle" x="50%" y="{y}" style="fill: {fill};">{text}</text>"#,
            font_size = fmt(32.0 * layout.scale),
            y = fmt(32.0 * layout.scale),
            fill = escape_xml(&theme.title_color),
            text = escape_xml(title)
        );
    }

    let _ = write!(
        &mut out,
        r#"<g transform="translate(0, {title_height})">"#,
        title_height = fmt(layout.title_height)
    );

    let style_by_key = venn_style_by_key(model);
    let is_hand_drawn = config_diagram_look(effective_config).as_str() == "handDrawn";
    let hand_drawn_seed = options.rough_randomness(
        effective_config
            .get("handDrawnSeed")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(options.seed() as f64),
        "render.venn.roughjs",
    );
    let mut circle_index = 0usize;

    for area in &layout.areas {
        let sets_key = venn_stable_sets_key(&area.sets);
        let styles = style_by_key.get(&sets_key);
        let presentation =
            venn_area_presentation(area, circle_index, styles, &theme, layout.scale)?;
        if area.sets.len() == 1 {
            let stroke_color =
                presentation
                    .stroke_color
                    .as_deref()
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("Venn set `{sets_key}` has no stroke color"),
                    })?;
            let stroke_width = presentation
                .stroke_width
                .as_ref()
                .map(stroke_width_css)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("Venn set `{sets_key}` has no stroke width"),
                })?;
            let stroke_opacity =
                presentation
                    .stroke_opacity
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!("Venn set `{sets_key}` has no stroke opacity"),
                    })?;
            let _ = write!(
                &mut out,
                r#"<g class="venn-area venn-circle venn-set-{set_class}" data-venn-sets="{sets}">"#,
                set_class = circle_index % 8,
                sets = escape_attr(&data_sets_attr(&area.sets)),
            );
            if is_hand_drawn {
                let circle = area.circles.first().ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "Venn set `{sets_key}` has no circle geometry for hand-drawn rendering"
                    ),
                })?;
                let stroke_width_value = crate::venn::rough::stroke_width(
                    presentation
                        .stroke_width
                        .as_ref()
                        .ok_or_else(|| Error::InvalidModel {
                            message: format!("Venn set `{sets_key}` has no stroke width"),
                        })?,
                )?;
                let geometry = circle_geometry(
                    circle,
                    rough_color(&presentation.fill_color)?,
                    rough_color(stroke_color)?,
                    stroke_width_value,
                    -41.0 + circle_index as f32 * 60.0,
                    &hand_drawn_seed,
                    options.work_meter(),
                )?;
                let fill_stroke = transparentize(&presentation.fill_color, 0.7)?;
                let _ = write!(
                    &mut out,
                    r#"<g><path d="{fill_path}" stroke="{fill_stroke}" stroke-width="2" fill="none"/><path d="{stroke_path}" stroke="{stroke}" stroke-width="{stroke_width}" fill="none"/></g>"#,
                    fill_path = escape_attr(&ops_to_svg_path_d(&geometry.fill)),
                    fill_stroke = escape_attr(&fill_stroke),
                    stroke_path = escape_attr(&ops_to_svg_path_d(&geometry.outline)),
                    stroke = escape_attr(stroke_color),
                    stroke_width = fmt(stroke_width_value as f64),
                );
            } else {
                let _ = write!(
                    &mut out,
                    r#"<path d="{path}" style="fill: {fill}; fill-opacity: {fill_opacity}; stroke: {stroke}; stroke-width: {stroke_width}; stroke-opacity: {stroke_opacity};"/>"#,
                    path = escape_attr(&area.path),
                    fill = escape_css_attr(&presentation.fill_color),
                    fill_opacity = escape_css_attr(&presentation.fill_opacity),
                    stroke = escape_css_attr(stroke_color),
                    stroke_width = escape_css_attr(&stroke_width),
                    stroke_opacity = fmt(stroke_opacity),
                );
            }
            write_area_label(
                &mut out,
                area,
                48.0 * layout.scale,
                &presentation.text_color,
            );
            out.push_str("</g>");
            circle_index += 1;
        } else {
            let _ = write!(
                &mut out,
                r#"<g class="venn-area venn-intersection" data-venn-sets="{sets}">"#,
                sets = escape_attr(&data_sets_attr(&area.sets)),
            );
            if is_hand_drawn {
                if presentation.has_custom_fill {
                    let fill_geometry = intersection_fill_geometry(
                        &area.path,
                        rough_color(&presentation.fill_color)?,
                        &hand_drawn_seed,
                        options.work_meter(),
                    )?;
                    let fill_stroke = transparentize(&presentation.fill_color, 0.3)?;
                    let _ = write!(
                        &mut out,
                        r#"<g><path d="{fill_path}" stroke="{fill_stroke}" stroke-width="2" fill="none"/></g>"#,
                        fill_path = escape_attr(&ops_to_svg_path_d(&fill_geometry)),
                        fill_stroke = escape_attr(&fill_stroke),
                    );
                } else {
                    let _ = write!(
                        &mut out,
                        r#"<path d="{path}" style="fill-opacity: 0;"/>"#,
                        path = escape_attr(&area.path),
                    );
                }
            } else {
                let _ = write!(
                    &mut out,
                    r#"<path d="{path}" style="fill-opacity: {fill_opacity}; fill: {fill};"/>"#,
                    path = escape_attr(&area.path),
                    fill_opacity = escape_css_attr(&presentation.fill_opacity),
                    fill = escape_css_attr(&presentation.fill_color),
                );
            }
            write_area_label(
                &mut out,
                area,
                48.0 * layout.scale,
                &presentation.text_color,
            );
            out.push_str("</g>");
        }
    }

    if !layout.text_areas.is_empty() {
        let mut nodes_by_key: HashMap<String, Vec<&crate::model::VennTextNodeLayout>> =
            HashMap::new();
        for node in &layout.text_nodes {
            nodes_by_key
                .entry(venn_stable_sets_key(&node.sets))
                .or_default()
                .push(node);
        }

        out.push_str(r#"<g class="venn-text-nodes">"#);
        for text_area in &layout.text_areas {
            let key = venn_stable_sets_key(&text_area.sets);
            let nodes = nodes_by_key.get(&key).map(Vec::as_slice).unwrap_or(&[]);
            let _ = write!(
                &mut out,
                r#"<g class="venn-text-area" font-size="{font_size}px">"#,
                font_size = fmt(text_area.font_size)
            );
            if layout.use_debug_layout {
                let _ = write!(
                    &mut out,
                    r#"<circle class="venn-text-debug-circle" cx="{cx}" cy="{cy}" r="{r}" fill="none" stroke="purple" stroke-width="{stroke_width}" stroke-dasharray="{dash} {gap}"/>"#,
                    cx = fmt(text_area.center_x),
                    cy = fmt(text_area.center_y),
                    r = fmt(text_area.inner_radius),
                    stroke_width = fmt(1.5 * layout.scale),
                    dash = fmt(6.0 * layout.scale),
                    gap = fmt(4.0 * layout.scale)
                );
                for cell in &text_area.debug_cells {
                    let _ = write!(
                        &mut out,
                        r#"<rect class="venn-text-debug-cell" x="{x}" y="{y}" width="{width}" height="{height}" fill="none" stroke="teal" stroke-width="{stroke_width}" stroke-dasharray="{dash} {gap}"/>"#,
                        x = fmt(cell.x),
                        y = fmt(cell.y),
                        width = fmt(cell.width),
                        height = fmt(cell.height),
                        stroke_width = fmt(layout.scale),
                        dash = fmt(4.0 * layout.scale),
                        gap = fmt(3.0 * layout.scale)
                    );
                }
            }

            for node in nodes {
                let text_color = style_by_key
                    .get(&node.id)
                    .and_then(|styles| venn_style_value(Some(styles), "color"));
                let mut span_style = "display: flex; width: 100%; height: 100%; white-space: normal; align-items: center; justify-content: center; text-align: center; overflow-wrap: normal; word-break: normal;".to_string();
                if let Some(text_color) = text_color {
                    span_style.push_str(" color: ");
                    span_style.push_str(text_color);
                    span_style.push(';');
                }
                let label = node.label.as_deref().unwrap_or(node.id.as_str());
                let _ = write!(
                    &mut out,
                    r#"<foreignObject class="venn-text-node-fo" width="{width}" height="{height}" x="{x}" y="{y}" overflow="visible"><span xmlns="http://www.w3.org/1999/xhtml" class="venn-text-node" style="{style}">{label}</span></foreignObject>"#,
                    width = fmt(node.width),
                    height = fmt(node.height),
                    x = fmt(node.x),
                    y = fmt(node.y),
                    style = escape_attr(&span_style),
                    label = escape_xml(label)
                );
            }
            out.push_str("</g>");
        }
        out.push_str("</g>");
    }

    out.push_str("</g></svg>\n");
    options.checkpoint_emit()?;
    root_document.complete(out)
}
