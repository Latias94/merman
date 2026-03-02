use super::{PieDiagramLayout, Result, SvgRenderOptions, apply_root_viewport_override, root_svg};
use std::fmt::Write as _;

fn pie_legend_rect_style(fill: &str) -> String {
    // Mermaid emits legend colors via inline `style` in rgb() form for default themes.
    // The compare tooling ignores `style`, but we keep this for human inspection parity.
    let rgb = match fill {
        "#ECECFF" => "rgb(236, 236, 255)",
        "#ffffde" => "rgb(255, 255, 222)",
        "hsl(80, 100%, 56.2745098039%)" => "rgb(181, 255, 32)",
        "hsl(240, 100%, 86.2745098039%)" => "rgb(185, 185, 255)",
        other => other,
    };
    format!("fill: {rgb}; stroke: {rgb};")
}

fn pie_polar_xy(radius: f64, angle: f64) -> (f64, f64) {
    let x = radius * angle.sin();
    let y = -radius * angle.cos();
    (x, y)
}

pub(super) fn render_pie_diagram_svg(
    layout: &PieDiagramLayout,
    semantic: &serde_json::Value,
    _effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: super::PieSvgModel = crate::json::from_value_ref(semantic)?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = super::escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(super::Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 450.0,
        max_y: 450.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    const NO_MAX_WIDTH_SENTINEL: &str = "__NO_MAX_WIDTH__";

    let mut max_w_attr = super::fmt_max_width_px(vb_w);
    let mut viewbox_attr = format!(
        "{} {} {} {}",
        super::fmt(vb_min_x),
        super::fmt(vb_min_y),
        super::fmt(vb_w),
        super::fmt(vb_h)
    );
    let mut w_attr = super::fmt(vb_w).to_string();
    let mut h_attr = super::fmt(vb_h).to_string();
    apply_root_viewport_override(
        diagram_id,
        &mut viewbox_attr,
        &mut w_attr,
        &mut h_attr,
        &mut max_w_attr,
        crate::generated::pie_root_overrides_11_12_2::lookup_pie_root_viewport_override,
    );

    let style_attr = if max_w_attr == NO_MAX_WIDTH_SENTINEL {
        "background-color: white;".to_string()
    } else {
        format!("max-width: {max_w_attr}px; background-color: white;")
    };

    let mut out = String::new();
    let aria_labelledby = model
        .acc_title
        .as_deref()
        .map(|_| format!("chart-title-{diagram_id_esc}"));
    let aria_describedby = model
        .acc_descr
        .as_deref()
        .map(|_| format!("chart-desc-{diagram_id_esc}"));
    root_svg::push_svg_root_open_ex(
        &mut out,
        diagram_id,
        None,
        root_svg::SvgRootWidth::Percent100,
        None,
        Some(style_attr.as_str()),
        viewbox_attr.as_str(),
        root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle,
        &[],
        "pie",
        aria_labelledby.as_deref(),
        aria_describedby.as_deref(),
        false,
    );

    if let Some(t) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = super::escape_xml(t)
        );
    }
    if let Some(d) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = super::escape_xml(d)
        );
    }

    let css = super::pie_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    out.push_str(r#"<g/>"#);

    let _ = write!(
        &mut out,
        r#"<g transform="translate({x},{y})">"#,
        x = super::fmt(layout.center_x),
        y = super::fmt(layout.center_y)
    );
    let _ = write!(
        &mut out,
        r#"<circle cx="0" cy="0" r="{r}" class="pieOuterCircle"/>"#,
        r = super::fmt(layout.outer_radius)
    );

    for slice in &layout.slices {
        let r = layout.radius;
        if slice.is_full_circle {
            let d = format!(
                "M0,-{r}A{r},{r},0,1,1,0,{r}A{r},{r},0,1,1,0,-{r}Z",
                r = super::fmt(r)
            );
            let _ = write!(
                &mut out,
                r#"<path d="{d}" fill="{fill}" class="pieCircle"/>"#,
                d = d,
                fill = super::escape_xml(&slice.fill)
            );
        } else {
            let (x0, y0) = pie_polar_xy(r, slice.start_angle);
            let (x1, y1) = pie_polar_xy(r, slice.end_angle);
            let large = if (slice.end_angle - slice.start_angle) > std::f64::consts::PI {
                1
            } else {
                0
            };
            let d = format!(
                "M{x0},{y0}A{r},{r},0,{large},1,{x1},{y1}L0,0Z",
                x0 = super::fmt(x0),
                y0 = super::fmt(y0),
                r = super::fmt(r),
                large = large,
                x1 = super::fmt(x1),
                y1 = super::fmt(y1)
            );
            let _ = write!(
                &mut out,
                r#"<path d="{d}" fill="{fill}" class="pieCircle"/>"#,
                d = d,
                fill = super::escape_xml(&slice.fill)
            );
        }
    }

    for slice in &layout.slices {
        let _ = write!(
            &mut out,
            r#"<text transform="translate({x},{y})" class="slice" style="text-anchor: middle;">{text}</text>"#,
            x = super::fmt(slice.text_x),
            y = super::fmt(slice.text_y),
            text = super::escape_xml(&format!("{}%", slice.percent))
        );
    }

    match model.title.as_deref() {
        Some(t) => {
            let _ = write!(
                &mut out,
                r#"<text x="0" y="-200" class="pieTitleText">{text}</text>"#,
                text = super::escape_xml(t)
            );
        }
        None => {
            out.push_str(r#"<text x="0" y="-200" class="pieTitleText"/>"#);
        }
    }

    for item in &layout.legend_items {
        let _ = write!(
            &mut out,
            r#"<g class="legend" transform="translate({x},{y})">"#,
            x = super::fmt(layout.legend_x),
            y = super::fmt(item.y)
        );
        let style = pie_legend_rect_style(&item.fill);
        let _ = write!(
            &mut out,
            r#"<rect width="18" height="18" style="{style}"/>"#,
            style = super::escape_xml(&style)
        );
        let text = if model.show_data {
            format!("{} [{}]", item.label, super::fmt(item.value))
        } else {
            item.label.clone()
        };
        let _ = write!(
            &mut out,
            r#"<text x="22" y="14">{text}</text>"#,
            text = super::escape_xml(&text)
        );
        out.push_str("</g>");
    }

    out.push_str("</g></svg>\n");
    Ok(out)
}
