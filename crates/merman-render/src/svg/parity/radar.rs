use super::*;

// Radar diagram SVG renderer implementation (split from legacy.rs).

fn radar_css(diagram_id: &str, effective_config: &serde_json::Value) -> String {
    // Keep `:root` last (matches upstream Mermaid radar SVG baselines).
    let id = escape_xml(diagram_id);
    let default_font = r#""trebuchet ms",verdana,arial,sans-serif"#;

    fn theme_var_string(cfg: &serde_json::Value, path: &[&str], fallback: &str) -> String {
        let mut cur = cfg;
        for key in path {
            cur = match cur.get(*key) {
                Some(v) => v,
                None => return fallback.to_string(),
            };
        }
        cur.as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| fallback.to_string())
    }

    fn theme_var_number_as_string(
        cfg: &serde_json::Value,
        path: &[&str],
        fallback: &str,
    ) -> String {
        let mut cur = cfg;
        for key in path {
            cur = match cur.get(*key) {
                Some(v) => v,
                None => return fallback.to_string(),
            };
        }
        if let Some(s) = cur.as_str() {
            return s.to_string();
        }
        if let Some(n) = json_f64(cur) {
            return fmt(n);
        }
        fallback.to_string()
    }

    fn default_c_scale(i: usize) -> &'static str {
        match i {
            0 => "hsl(240, 100%, 76.2745098039%)",
            1 => "hsl(60, 100%, 73.5294117647%)",
            2 => "hsl(80, 100%, 76.2745098039%)",
            3 => "hsl(270, 100%, 76.2745098039%)",
            4 => "hsl(300, 100%, 76.2745098039%)",
            5 => "hsl(330, 100%, 76.2745098039%)",
            6 => "hsl(0, 100%, 76.2745098039%)",
            7 => "hsl(30, 100%, 76.2745098039%)",
            8 => "hsl(90, 100%, 76.2745098039%)",
            9 => "hsl(150, 100%, 76.2745098039%)",
            10 => "hsl(180, 100%, 76.2745098039%)",
            _ => "hsl(210, 100%, 76.2745098039%)",
        }
    }

    let font_family = config_string(effective_config, &["themeVariables", "fontFamily"])
        .map(|s| normalize_css_font_family(&s))
        .unwrap_or_else(|| default_font.to_string());
    let base_font_size =
        theme_var_number_as_string(effective_config, &["themeVariables", "fontSize"], "16px");
    let base_text_color =
        theme_var_string(effective_config, &["themeVariables", "textColor"], "#333");
    let error_bkg_color = theme_var_string(
        effective_config,
        &["themeVariables", "errorBkgColor"],
        "#552222",
    );
    let error_text_color = theme_var_string(
        effective_config,
        &["themeVariables", "errorTextColor"],
        "#552222",
    );
    let line_color = theme_var_string(
        effective_config,
        &["themeVariables", "lineColor"],
        "#333333",
    );

    let title_font_size = base_font_size.clone();
    let title_color = theme_color(effective_config, "titleColor", "#333");

    let axis_color = theme_var_string(
        effective_config,
        &["themeVariables", "radar", "axisColor"],
        "#333333",
    );
    let axis_stroke_width = config_f64(
        effective_config,
        &["themeVariables", "radar", "axisStrokeWidth"],
    )
    .unwrap_or(2.0);
    let axis_label_font_size = config_f64(
        effective_config,
        &["themeVariables", "radar", "axisLabelFontSize"],
    )
    .unwrap_or(12.0);

    let graticule_color = theme_var_string(
        effective_config,
        &["themeVariables", "radar", "graticuleColor"],
        "#DEDEDE",
    );
    let graticule_opacity = config_f64(
        effective_config,
        &["themeVariables", "radar", "graticuleOpacity"],
    )
    .unwrap_or(0.3);
    let graticule_stroke_width = config_f64(
        effective_config,
        &["themeVariables", "radar", "graticuleStrokeWidth"],
    )
    .unwrap_or(1.0);

    let legend_font_size = config_f64(
        effective_config,
        &["themeVariables", "radar", "legendFontSize"],
    )
    .unwrap_or(12.0);

    let curve_opacity = config_f64(
        effective_config,
        &["themeVariables", "radar", "curveOpacity"],
    )
    .unwrap_or(0.5);
    let curve_stroke_width = config_f64(
        effective_config,
        &["themeVariables", "radar", "curveStrokeWidth"],
    )
    .unwrap_or(2.0);

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"#{}{{font-family:{};font-size:{};fill:{};}}"#,
        id, font_family, base_font_size, base_text_color
    );
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .error-icon{{fill:{};}}#{} .error-text{{fill:{};stroke:{};}}"#,
        id, error_bkg_color, id, error_text_color, error_text_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .edge-thickness-normal{{stroke-width:1px;}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, id, id, id, id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .marker{{fill:{};stroke:{};}}#{} .marker.cross{{stroke:{};}}"#,
        id, line_color, line_color, id, line_color
    );
    let _ = write!(
        &mut out,
        r#"#{} svg{{font-family:{};font-size:{};}}#{} p{{margin:0;}}"#,
        id, font_family, base_font_size, id
    );

    let _ = write!(
        &mut out,
        r#"#{} .radarTitle{{font-size:{};color:{};dominant-baseline:hanging;text-anchor:middle;}}"#,
        id, title_font_size, title_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .radarAxisLine{{stroke:{};stroke-width:{};}}"#,
        id,
        axis_color,
        fmt(axis_stroke_width)
    );
    let _ = write!(
        &mut out,
        r#"#{} .radarAxisLabel{{dominant-baseline:middle;text-anchor:middle;font-size:{}px;color:{};}}"#,
        id,
        fmt(axis_label_font_size),
        axis_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .radarGraticule{{fill:{};fill-opacity:{};stroke:{};stroke-width:{};}}"#,
        id,
        graticule_color,
        fmt(graticule_opacity),
        graticule_color,
        fmt(graticule_stroke_width)
    );
    let _ = write!(
        &mut out,
        r#"#{} .radarLegendText{{text-anchor:start;font-size:{}px;dominant-baseline:hanging;}}"#,
        id,
        fmt(legend_font_size)
    );

    for i in 0..12 {
        let key = format!("cScale{i}");
        let c = theme_color(effective_config, &key, default_c_scale(i));
        let _ = write!(
            &mut out,
            r#"#{} .radarCurve-{}{{color:{};fill:{};fill-opacity:{};stroke:{};stroke-width:{};}}#{} .radarLegendBox-{}{{fill:{};fill-opacity:{};stroke:{};}}"#,
            id,
            i,
            c,
            c,
            fmt(curve_opacity),
            c,
            fmt(curve_stroke_width),
            id,
            i,
            c,
            fmt(curve_opacity),
            c
        );
    }

    let _ = write!(
        &mut out,
        r#"#{} :root{{--mermaid-font-family:{};}}"#,
        id, font_family
    );

    out
}

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
