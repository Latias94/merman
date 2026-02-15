#![allow(clippy::too_many_arguments)]

use super::*;

fn journey_css(diagram_id: &str) -> String {
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;

    // Keep `:root` last (matches upstream Mermaid journey SVG baselines).
    let root_rule = format!(r#"#{} :root{{--mermaid-font-family:{};}}"#, id, font);
    let mut out = info_css(diagram_id);
    if let Some(prefix) = out.strip_suffix(&root_rule) {
        out = prefix.to_string();
    }

    // Mermaid's journey diagram reuses the historical "user-journey" stylesheet, post-processed by
    // Mermaid's CSS pipeline (nesting expansion + id scoping + minification).
    let _ = write!(
        &mut out,
        r#"#{} .label{{font-family:{};color:#333;}}"#,
        id, font
    );
    let _ = write!(&mut out, r#"#{} .mouth{{stroke:#666;}}"#, id);
    let _ = write!(&mut out, r#"#{} line{{stroke:#333;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .legend{{fill:#333;font-family:{};}}"#,
        id, font
    );
    let _ = write!(&mut out, r#"#{} .label text{{fill:#333;}}"#, id);
    let _ = write!(&mut out, r#"#{} .label{{color:#333;}}"#, id);
    let _ = write!(&mut out, r#"#{} .face{{fill:#FFF8DC;stroke:#999;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon,#{} .node path{{fill:#ECECFF;stroke:#9370DB;stroke-width:1px;}}"#,
        id, id, id, id, id
    );
    let _ = write!(&mut out, r#"#{} .node .label{{text-align:center;}}"#, id);
    let _ = write!(&mut out, r#"#{} .node.clickable{{cursor:pointer;}}"#, id);
    let _ = write!(&mut out, r#"#{} .arrowheadPath{{fill:#333333;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} .edgePath .path{{stroke:#333333;stroke-width:1.5px;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .flowchart-link{{stroke:#333333;fill:none;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgeLabel{{background-color:rgba(232,232,232, 0.8);text-align:center;}}"#,
        id
    );
    let _ = write!(&mut out, r#"#{} .edgeLabel rect{{opacity:0.5;}}"#, id);
    let _ = write!(&mut out, r#"#{} .cluster text{{fill:#333;}}"#, id);
    let _ = write!(
        &mut out,
        r#"#{} div.mermaidTooltip{{position:absolute;text-align:center;max-width:200px;padding:2px;font-family:{};font-size:12px;background:hsl(80, 100%, 96.2745098039%);border:1px solid #aaaa33;border-radius:2px;pointer-events:none;z-index:100;}}"#,
        id, font
    );
    let _ = write!(
        &mut out,
        r#"#{} .task-type-0,#{} .section-type-0{{fill:#ECECFF;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task-type-1,#{} .section-type-1{{fill:#ffffde;}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task-type-2,#{} .section-type-2{{fill:hsl(304, 100%, 96.2745098039%);}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task-type-3,#{} .section-type-3{{fill:hsl(124, 100%, 93.5294117647%);}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task-type-4,#{} .section-type-4{{fill:hsl(176, 100%, 96.2745098039%);}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task-type-5,#{} .section-type-5{{fill:hsl(-4, 100%, 93.5294117647%);}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task-type-6,#{} .section-type-6{{fill:hsl(8, 100%, 96.2745098039%);}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .task-type-7,#{} .section-type-7{{fill:hsl(188, 100%, 93.5294117647%);}}"#,
        id, id
    );
    let _ = write!(
        &mut out,
        r#"#{} .label-icon{{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}}"#,
        id
    );
    let _ = write!(
        &mut out,
        r#"#{} .node .label-icon path{{fill:currentColor;stroke:revert;stroke-width:revert;}}"#,
        id
    );

    out.push_str(&root_rule);
    out
}

#[derive(Debug, Clone, Deserialize)]
struct JourneySvgModel {
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
}

pub(super) fn render_journey_diagram_svg(
    layout: &crate::model::JourneyDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let model: JourneySvgModel = crate::json::from_value_ref(semantic)?;

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let diagram_title = layout
        .title
        .as_deref()
        .or(diagram_title)
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_from_meta = layout.title.is_none() && diagram_title.is_some();

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: -25.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let mut vb_h = (bounds.max_y - bounds.min_y).max(1.0);
    // Mermaid journey titles can also come from YAML frontmatter (`---\ntitle: ...\n---`).
    // When the title is supplied via frontmatter, our semantic/layout layer currently leaves
    // `layout.title` empty. Upstream still accounts for the title when sizing the root viewBox,
    // so mirror that here to keep `parity-root` stable.
    if title_from_meta {
        vb_h += 70.0;
    }

    let task_font_size = effective_config
        .get("journey")
        .and_then(|j| j.get("taskFontSize"))
        .and_then(|v| v.as_f64())
        .unwrap_or(14.0)
        .max(1.0);
    let task_font_family = effective_config
        .get("journey")
        .and_then(|j| j.get("taskFontFamily"))
        .and_then(|v| v.as_str())
        .unwrap_or("\"Open Sans\", sans-serif");

    let title_font_size = effective_config
        .get("journey")
        .and_then(|j| j.get("titleFontSize"))
        .and_then(|v| v.as_str())
        .unwrap_or("4ex");
    let title_font_family = effective_config
        .get("journey")
        .and_then(|j| j.get("titleFontFamily"))
        .and_then(|v| v.as_str())
        .unwrap_or("\"trebuchet ms\", verdana, arial, sans-serif");
    let title_color = effective_config
        .get("journey")
        .and_then(|j| j.get("titleColor"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    fn split_html_br_lines(text: &str) -> Vec<String> {
        let b = text.as_bytes();
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut i = 0usize;
        while i < b.len() {
            if b[i] != b'<' {
                let ch = text[i..].chars().next().unwrap();
                cur.push(ch);
                i += ch.len_utf8();
                continue;
            }
            if i + 3 >= b.len() {
                cur.push('<');
                i += 1;
                continue;
            }
            if b[i + 1] == b'/' {
                cur.push('<');
                i += 1;
                continue;
            }
            let b1 = b[i + 1];
            let b2 = b[i + 2];
            if !matches!(b1, b'b' | b'B') || !matches!(b2, b'r' | b'R') {
                cur.push('<');
                i += 1;
                continue;
            }
            let mut j = i + 3;
            while j < b.len() && matches!(b[j], b' ' | b'\t' | b'\r' | b'\n') {
                j += 1;
            }
            if j < b.len() && b[j] == b'/' {
                j += 1;
            }
            if j < b.len() && b[j] == b'>' {
                out.push(std::mem::take(&mut cur));
                i = j + 1;
                continue;
            }
            cur.push('<');
            i += 1;
        }
        out.push(cur);
        if out.is_empty() {
            vec!["".to_string()]
        } else {
            out
        }
    }

    fn write_text_candidate(
        out: &mut String,
        content: &str,
        class: &str,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        task_font_size: f64,
        task_font_family: &str,
    ) {
        let content_esc = escape_xml(content);
        let class_esc = escape_attr(class);
        let font_family_esc = escape_attr(task_font_family);
        let cx = x + width / 2.0;
        let cy = y + height / 2.0;

        out.push_str("<switch>");
        let _ = write!(
            out,
            r#"<foreignObject x="{x}" y="{y}" width="{w}" height="{h}">"#,
            x = fmt(x),
            y = fmt(y),
            w = fmt(width),
            h = fmt(height),
        );
        let _ = write!(
            out,
            r#"<div class="{class}" xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><div class="label" style="display: table-cell; text-align: center; vertical-align: middle;">{text}</div></div>"#,
            class = class_esc,
            text = content_esc
        );
        out.push_str("</foreignObject>");

        let lines = split_html_br_lines(content);
        let n = lines.len().max(1) as f64;
        for (i, line) in lines.into_iter().enumerate() {
            let dy = (i as f64) * task_font_size - (task_font_size * (n - 1.0)) / 2.0;
            let _ = write!(
                out,
                r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="{class}" style="text-anchor: middle; font-size: {fs}px; font-family: {ff};"><tspan x="{x}" dy="{dy}">{text}</tspan></text>"#,
                x = fmt(cx),
                y = fmt(cy),
                class = class_esc,
                fs = fmt(task_font_size),
                ff = font_family_esc,
                dy = fmt(dy),
                text = escape_xml(&line)
            );
        }

        out.push_str("</switch>");
    }

    let mut out = String::new();
    let aria = match (model.acc_title.as_deref(), model.acc_descr.as_deref()) {
        (Some(_), Some(_)) => format!(
            r#" aria-describedby="chart-desc-{id}" aria-labelledby="chart-title-{id}""#,
            id = diagram_id_esc
        ),
        (Some(_), None) => format!(
            r#" aria-labelledby="chart-title-{id}""#,
            id = diagram_id_esc
        ),
        (None, Some(_)) => format!(
            r#" aria-describedby="chart-desc-{id}""#,
            id = diagram_id_esc
        ),
        (None, None) => String::new(),
    };
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{min_x} {min_y} {w} {h}" preserveAspectRatio="xMinYMin meet" height="{svg_h}" role="graphics-document document" aria-roledescription="journey"{aria}>"#,
        diagram_id_esc = diagram_id_esc,
        max_w = fmt(layout.width),
        min_x = fmt(vb_min_x),
        min_y = fmt(vb_min_y),
        w = fmt(vb_w),
        h = fmt(vb_h),
        svg_h = fmt(if vb_min_y < 0.0 {
            vb_h - vb_min_y
        } else {
            vb_h
        }),
        aria = aria,
    );

    if let Some(title) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id_esc,
            text = escape_xml(title)
        );
    }
    if let Some(desc) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id_esc,
            text = escape_xml(desc)
        );
    }

    let css = journey_css(diagram_id);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    out.push_str(r#"<g/>"#);
    out.push_str(
        r#"<defs><marker id="arrowhead" refX="5" refY="2" markerWidth="6" markerHeight="4" orient="auto"><path d="M 0,0 V 4 L6,2 Z"/></marker></defs>"#,
    );

    for item in &layout.actor_legend {
        let _ = write!(
            &mut out,
            r##"<circle cx="{cx}" cy="{cy}" class="actor-{pos}" fill="{fill}" stroke="#000" r="{r}"/>"##,
            cx = fmt(item.circle_cx),
            cy = fmt(item.circle_cy),
            pos = item.pos,
            fill = escape_attr(&item.color),
            r = fmt(item.circle_r),
        );
        for line in &item.label_lines {
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" class="legend"><tspan x="{tx}">{text}</tspan></text>"#,
                x = fmt(line.x),
                y = fmt(line.y),
                tx = fmt(line.tspan_x),
                text = escape_xml(&line.text),
            );
        }
    }

    let mut section_iter = layout.sections.iter();
    let mut last_section: Option<&str> = None;
    for task in &layout.tasks {
        if last_section != Some(task.section.as_str()) {
            let Some(section) = section_iter.next() else {
                break;
            };
            let section_class = format!("journey-section section-type-{}", section.num);
            let _ = write!(
                &mut out,
                r##"<g><rect x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" rx="3" ry="3" class="{class}"/>"##,
                x = fmt(section.x),
                y = fmt(section.y),
                fill = escape_attr(&section.fill),
                w = fmt(section.width),
                h = fmt(section.height),
                class = escape_attr(&section_class),
            );
            write_text_candidate(
                &mut out,
                &section.section,
                &section_class,
                section.x,
                section.y,
                section.width,
                section.height,
                task_font_size,
                task_font_family,
            );
            out.push_str("</g>");
        }

        last_section = Some(task.section.as_str());

        let _ = write!(
            &mut out,
            r##"<g><line id="{id}" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" class="task-line" stroke-width="1px" stroke-dasharray="4 2" stroke="#666"/>"##,
            id = escape_attr(&task.line_id),
            x1 = fmt(task.line_x1),
            y1 = fmt(task.line_y1),
            x2 = fmt(task.line_x2),
            y2 = fmt(task.line_y2),
        );

        let _ = write!(
            &mut out,
            r#"<circle cx="{cx}" cy="{cy}" class="face" r="15" stroke-width="2" overflow="visible"/>"#,
            cx = fmt(task.face_cx),
            cy = fmt(task.face_cy),
        );
        out.push_str("<g>");
        let eye_dx = 15.0 / 3.0;
        let eye_r = 1.5;
        let _ = write!(
            &mut out,
            r##"<circle cx="{cx}" cy="{cy}" r="{r}" stroke-width="2" fill="#666" stroke="#666"/>"##,
            cx = fmt(task.face_cx - eye_dx),
            cy = fmt(task.face_cy - eye_dx),
            r = fmt(eye_r),
        );
        let _ = write!(
            &mut out,
            r##"<circle cx="{cx}" cy="{cy}" r="{r}" stroke-width="2" fill="#666" stroke="#666"/>"##,
            cx = fmt(task.face_cx + eye_dx),
            cy = fmt(task.face_cy - eye_dx),
            r = fmt(eye_r),
        );

        match task.mouth {
            crate::model::JourneyMouthKind::Smile => {
                let _ = write!(
                    &mut out,
                    r#"<path class="mouth" d="M7.5,0A7.5,7.5,0,1,1,-7.5,0L-6.818,0A6.818,6.818,0,1,0,6.818,0Z" transform="translate({x},{y})"/>"#,
                    x = fmt(task.face_cx),
                    y = fmt(task.face_cy + 2.0),
                );
            }
            crate::model::JourneyMouthKind::Sad => {
                let _ = write!(
                    &mut out,
                    r#"<path class="mouth" d="M-7.5,0A7.5,7.5,0,1,1,7.5,0L6.818,0A6.818,6.818,0,1,0,-6.818,0Z" transform="translate({x},{y})"/>"#,
                    x = fmt(task.face_cx),
                    y = fmt(task.face_cy + 7.0),
                );
            }
            crate::model::JourneyMouthKind::Ambivalent => {
                let _ = write!(
                    &mut out,
                    r##"<line class="mouth" stroke="#666" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="1px"/>"##,
                    x1 = fmt(task.face_cx - 5.0),
                    y1 = fmt(task.face_cy + 7.0),
                    x2 = fmt(task.face_cx + 5.0),
                    y2 = fmt(task.face_cy + 7.0),
                );
            }
        }

        out.push_str("</g>");

        let _ = write!(
            &mut out,
            r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" rx="3" ry="3" class="task task-type-{num}"/>"##,
            x = fmt(task.x),
            y = fmt(task.y),
            fill = escape_attr(&task.fill),
            w = fmt(task.width),
            h = fmt(task.height),
            num = task.num,
        );

        for c in &task.actor_circles {
            let _ = write!(
                &mut out,
                r##"<circle cx="{cx}" cy="{cy}" class="actor-{pos}" fill="{fill}" stroke="#000" r="{r}"><title>{title}</title></circle>"##,
                cx = fmt(c.cx),
                cy = fmt(c.cy),
                pos = c.pos,
                fill = escape_attr(&c.color),
                r = fmt(c.r),
                title = escape_xml(&c.actor),
            );
        }

        write_text_candidate(
            &mut out,
            &task.task,
            "task",
            task.x,
            task.y,
            task.width,
            task.height,
            task_font_size,
            task_font_family,
        );

        out.push_str("</g>");
    }

    if let Some(title) = diagram_title {
        let _ = write!(
            &mut out,
            r#"<text x="{x}" font-size="{fs}" font-weight="bold" y="{y}" fill="{fill}" font-family="{ff}">{text}</text>"#,
            x = fmt(layout.title_x),
            fs = escape_attr(title_font_size),
            y = fmt(layout.title_y),
            fill = escape_attr(title_color),
            ff = escape_attr(title_font_family),
            text = escape_xml(title),
        );
    }

    let _ = write!(
        &mut out,
        r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4" stroke="black" marker-end="url(#arrowhead)"/>"#,
        x1 = fmt(layout.activity_line.x1),
        y1 = fmt(layout.activity_line.y1),
        x2 = fmt(layout.activity_line.x2),
        y2 = fmt(layout.activity_line.y2),
    );

    out.push_str("</svg>\n");
    Ok(out)
}
