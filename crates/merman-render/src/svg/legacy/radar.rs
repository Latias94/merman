use super::*;

// Radar diagram SVG renderer implementation (split from legacy.rs).

pub(super) fn render_radar_diagram_svg(
    layout: &RadarDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    #[derive(Debug, Clone, serde::Deserialize)]
    struct RadarSvgModel {
        #[serde(rename = "accTitle")]
        acc_title: Option<String>,
        #[serde(rename = "accDescr")]
        acc_descr: Option<String>,
        title: Option<String>,
        #[serde(default)]
        curves: Vec<RadarSvgCurve>,
    }

    #[derive(Debug, Clone, serde::Deserialize)]
    struct RadarSvgCurve {
        label: String,
    }

    let model: RadarSvgModel = serde_json::from_value(semantic.clone())?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("radar");
    let diagram_id_esc = escape_xml(diagram_id);

    let has_acc_title = model
        .acc_title
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_acc_descr = model
        .acc_descr
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty());

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{id}" width="{w}" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 {vbw} {vbh}" height="{h}" role="graphics-document document" aria-roledescription="radar""#,
        id = diagram_id_esc,
        w = fmt(layout.svg_width),
        h = fmt(layout.svg_height),
        vbw = fmt(layout.svg_width),
        vbh = fmt(layout.svg_height),
    );

    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#" aria-describedby="chart-desc-{id}""#,
            id = diagram_id_esc
        );
    }
    if has_acc_title {
        let _ = write!(
            &mut out,
            r#" aria-labelledby="chart-title-{id}""#,
            id = diagram_id_esc
        );
    }

    out.push_str(r#" style="background-color: white;">"#);

    if has_acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml(model.acc_title.as_deref().unwrap_or_default())
        );
    }
    if has_acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml(model.acc_descr.as_deref().unwrap_or_default())
        );
    }

    let css = radar_css(diagram_id, effective_config);
    let _ = write!(&mut out, "<style>{}</style>", css);
    out.push_str("<g/>");

    let _ = write!(
        &mut out,
        r#"<g transform="translate({x}, {y})">"#,
        x = fmt(layout.center_x),
        y = fmt(layout.center_y)
    );

    for g in &layout.graticules {
        if g.kind == "polygon" {
            if g.points.is_empty() {
                out.push_str(r#"<polygon points="" class="radarGraticule"/>"#);
            } else {
                let mut points = String::new();
                for (i, p) in g.points.iter().enumerate() {
                    if i > 0 {
                        points.push(' ');
                    }
                    let _ = write!(&mut points, "{},{}", fmt(p.x), fmt(p.y));
                }
                let _ = write!(
                    &mut out,
                    r#"<polygon points="{points}" class="radarGraticule"/>"#,
                    points = escape_xml(&points)
                );
            }
        } else if let Some(r) = g.r {
            let _ = write!(
                &mut out,
                r#"<circle r="{r}" class="radarGraticule"/>"#,
                r = fmt(r)
            );
        }
    }

    for a in &layout.axes {
        let _ = write!(
            &mut out,
            r#"<line x1="0" y1="0" x2="{x2}" y2="{y2}" class="radarAxisLine"/>"#,
            x2 = fmt(a.line_x2),
            y2 = fmt(a.line_y2)
        );
        let _ = write!(
            &mut out,
            r#"<text x="{x}" y="{y}" class="radarAxisLabel">{label}</text>"#,
            x = fmt(a.label_x),
            y = fmt(a.label_y),
            label = escape_xml(&a.label)
        );
    }

    let polygon_curves = layout
        .graticules
        .first()
        .is_some_and(|g| g.kind.trim() == "polygon");
    for c in &layout.curves {
        if polygon_curves && !c.points.is_empty() {
            let mut points = String::new();
            for (i, p) in c.points.iter().enumerate() {
                if i > 0 {
                    points.push(' ');
                }
                let _ = write!(&mut points, "{},{}", fmt(p.x), fmt(p.y));
            }
            let _ = write!(
                &mut out,
                r#"<polygon points="{points}" class="radarCurve-{idx}"/>"#,
                points = escape_xml(&points),
                idx = c.class_index
            );
        } else {
            let _ = write!(
                &mut out,
                r#"<path d="{d}" class="radarCurve-{idx}"/>"#,
                d = escape_xml(&c.path_d),
                idx = c.class_index
            );
        }
    }

    for item in &layout.legend_items {
        let _ = write!(
            &mut out,
            r#"<g transform="translate({x}, {y})">"#,
            x = fmt(item.x),
            y = fmt(item.y)
        );
        let _ = write!(
            &mut out,
            r#"<rect width="12" height="12" class="radarLegendBox-{idx}"/>"#,
            idx = item.class_index
        );
        let label = model
            .curves
            .get(item.class_index as usize)
            .map(|c| c.label.as_str())
            .unwrap_or("");
        let _ = write!(
            &mut out,
            r#"<text x="16" y="0" class="radarLegendText">{text}</text>"#,
            text = escape_xml(label)
        );
        out.push_str("</g>");
    }

    match model.title.as_deref() {
        Some(t) => {
            let _ = write!(
                &mut out,
                r#"<text class="radarTitle" x="0" y="{y}">{text}</text>"#,
                y = fmt(layout.title_y),
                text = escape_xml(t)
            );
        }
        None => {
            let _ = write!(
                &mut out,
                r#"<text class="radarTitle" x="0" y="{y}"/>"#,
                y = fmt(layout.title_y)
            );
        }
    }

    out.push_str("</g></svg>\n");
    Ok(out)
}
