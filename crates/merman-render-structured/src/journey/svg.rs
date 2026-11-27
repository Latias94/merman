use std::fmt::Write as _;

use merman_model::{OperationPhase, families::journey::JourneyDiagramModel};
use merman_render_core::{
    Bounds, Error, Result,
    number_format::{canonical_number as fmt, canonical_number_with_nonfinite},
    resources::OperationWorkMeter,
    root_svg,
    svg_css::{
        CssSelectorDiagramId, PreparedCommonCss, SvgDiagramIdValue, SvgId, scoped_svg_id,
        scoped_svg_url,
    },
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    text::split_html_br_lines,
    xml::{escape_attr, escape_attr_display, escape_xml},
};

use super::{
    JOURNEY_FACE_RADIUS_PX, JOURNEY_TITLE_EXTRA_HEIGHT_PX, JOURNEY_VIEWBOX_TOP_PAD_PX,
    JourneyCssBinding, JourneyDiagramLayout, JourneyMouthKind, JourneyNativeStyle,
    JourneySectionLayout, JourneyStyle, JourneyTaskLayout, JourneyTextPlacement,
};

fn fmt_task_face_y(v: Option<f64>) -> String {
    match v {
        Some(value) => canonical_number_with_nonfinite(value).to_string(),
        None => "NaN".to_string(),
    }
}

#[derive(Debug, Clone, Copy)]
struct JourneyTextBox {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

#[derive(Debug, Clone, Copy)]
struct JourneyTextStyle<'a> {
    placement: JourneyTextPlacement,
    task_font_size: f64,
    task_font_family: &'a str,
    native_weight: Option<u16>,
    section_colour: Option<&'a str>,
    native_fill: Option<&'a str>,
}

fn write_text_candidate(
    out: &mut impl SvgOutput,
    content: &str,
    class: &str,
    text_box: JourneyTextBox,
    style: JourneyTextStyle<'_>,
    text_fill: &str,
) -> Result<()> {
    let JourneyTextBox {
        x,
        y,
        width,
        height,
    } = text_box;
    let JourneyTextStyle {
        placement,
        task_font_size,
        task_font_family,
        native_weight,
        section_colour,
        native_fill,
    } = style;
    let content_esc = escape_xml(content);
    let class_esc = escape_attr(class);
    let font_family_esc = escape_attr(task_font_family);
    let fill_esc = escape_attr(text_fill);
    let cx = x + width / 2.0;
    let cy = y + height / 2.0;
    let native_typography = native_weight.map(|weight| {
        let mut css = format!(" font-family: {font_family_esc}; font-weight: {weight};");
        if task_font_size >= 0.0 {
            let _ = write!(css, " font-size: {}px;", fmt(task_font_size));
        }
        css
    });
    let native_font_style = native_typography.as_deref().unwrap_or_default();

    if placement == JourneyTextPlacement::Old {
        let fill_style = native_fill
            .map(|fill| {
                let fill = escape_attr(fill);
                format!(" fill: {fill}; color: {fill};")
            })
            .unwrap_or_default();
        let _ = write!(
            out,
            r#"<text x="{x}" y="{y}" class="{class}" style="text-anchor: middle;{native_font_style}{fill_style}">{text}</text>"#,
            x = fmt(cx),
            y = fmt(cy + 5.0),
            class = class_esc,
            text = content_esc,
        );
        return out.checkpoint();
    }

    if placement == JourneyTextPlacement::Fo {
        let fill_style = native_fill
            .map(|fill| {
                let fill = escape_attr(fill);
                format!(" color: {fill}; fill: {fill};")
            })
            .unwrap_or_default();
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
            r#"<div class="{class}" xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><div class="label" style="display: table-cell; text-align: center; vertical-align: middle;{native_font_style}{fill_style}">{text}</div></div>"#,
            class = class_esc,
            text = content_esc
        );
        out.push_str("</foreignObject>");
        out.checkpoint()?;
    }

    let lines = split_html_br_lines(content);
    let n = lines.len().max(1) as f64;
    for (i, line) in lines.into_iter().enumerate() {
        let line_esc = escape_xml(line);
        let dy = (i as f64) * task_font_size - (task_font_size * (n - 1.0)) / 2.0;
        let _ = write!(
            out,
            r#"<text x="{x}" y="{y}" dominant-baseline="central" alignment-baseline="central" class="{class}""#,
            x = fmt(cx),
            y = fmt(cy),
            class = class_esc,
        );
        if placement == JourneyTextPlacement::Tspan
            && let Some(colour) = section_colour
        {
            let _ = write!(out, r#" fill="{}""#, escape_attr(colour));
        }
        out.push_str(r#" style="text-anchor: middle;"#);
        if let Some(css) = native_typography.as_deref() {
            out.push_str(css);
        } else {
            if task_font_size >= 0.0 {
                let _ = write!(out, " font-size: {}px;", fmt(task_font_size));
            }
            let _ = write!(out, " font-family: {font_family_esc};");
        }
        if let Some(fill) = native_fill {
            let _ = write!(
                out,
                " fill: {}; color: {};",
                escape_attr(fill),
                escape_attr(fill)
            );
        } else if placement == JourneyTextPlacement::Fo {
            let _ = write!(out, " fill: {fill_esc};");
        }
        let _ = write!(
            out,
            r#""><tspan x="{x}" dy="{dy}">{text}</tspan></text>"#,
            x = fmt(cx),
            dy = fmt(dy),
            text = line_esc
        );
        out.checkpoint()?;
    }

    if placement == JourneyTextPlacement::Fo {
        out.push_str("</switch>");
    }
    out.checkpoint()
}

fn write_task_rect(
    out: &mut impl SvgOutput,
    task: &JourneyTaskLayout,
    radius: &str,
    native_stroke: Option<&str>,
) {
    let _ = write!(
        out,
        r##"<rect x="{}" y="{}" fill="{}" stroke="#666""##,
        fmt(task.x),
        fmt(task.y),
        escape_attr(&task.fill)
    );
    if let Some(stroke) = native_stroke {
        let _ = write!(out, r#" style="stroke:{};""#, escape_attr(stroke));
    }
    let _ = write!(
        out,
        r#" width="{}" height="{}" rx="{}" ry="{}" class="task task-type-{}"/>"#,
        fmt(task.width),
        fmt(task.height),
        radius,
        radius,
        task.num
    );
}

fn write_section_rect(
    out: &mut impl SvgOutput,
    section: &JourneySectionLayout,
    native: Option<&JourneyNativeStyle>,
) {
    let class = format!("journey-section section-type-{}", section.num);
    out.push_str("<g>");
    if let Some(native) = native {
        let _ = write!(
            out,
            r#"<rect x="{x}" y="{y}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width}" width="{w}" height="{h}" rx="{radius}" ry="{radius}" class="{class}"/>"#,
            x = fmt(section.x),
            y = fmt(section.y),
            fill = escape_attr(&section.fill),
            stroke = escape_attr(&native.border),
            stroke_width = fmt(native.border_width),
            w = fmt(section.width),
            h = fmt(section.height),
            radius = fmt(native.container_radius),
            class = escape_attr(&class),
        );
        return;
    }
    let _ = write!(
        out,
        r##"<rect x="{x}" y="{y}" fill="{fill}" stroke="#666" width="{w}" height="{h}" rx="3" ry="3" class="{class}"/>"##,
        x = fmt(section.x),
        y = fmt(section.y),
        fill = escape_attr(&section.fill),
        w = fmt(section.width),
        h = fmt(section.height),
        class = escape_attr(&class),
    );
}

fn journey_css(
    diagram_id: impl SvgDiagramIdValue,
    theme: &JourneyCssBinding,
    common: &PreparedCommonCss,
) -> String {
    let mut out = String::new();
    common
        .write_prefix(&mut out, diagram_id)
        .expect("String-backed Journey base CSS emission cannot fail");
    let font = common.font_family();
    let text_color = theme.text_color.as_str();
    let line_color = common.line_color();

    // Mermaid's journey diagram reuses the historical "user-journey" stylesheet, post-processed by
    // Mermaid's CSS pipeline (nesting expansion + id scoping + minification).
    let _ = write!(
        &mut out,
        r#"#{} .label{{font-family:{};color:{};}}"#,
        diagram_id, font, text_color
    );
    let _ = write!(&mut out, r#"#{} .mouth{{stroke:#666;}}"#, diagram_id);
    let _ = write!(
        &mut out,
        r#"#{} line{{stroke:{};}}"#,
        diagram_id, text_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .legend{{fill:{};font-family:{};}}"#,
        diagram_id, text_color, font
    );
    let _ = write!(
        &mut out,
        r#"#{} .label text{{fill:{};}}"#,
        diagram_id, text_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .label{{color:{};}}"#,
        diagram_id, text_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .face{{fill:{};stroke:#999;}}"#,
        diagram_id, theme.face_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .node rect,#{} .node circle,#{} .node ellipse,#{} .node polygon,#{} .node path{{fill:{};stroke:{};stroke-width:1px;}}"#,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
        diagram_id,
        theme.main_bkg,
        theme.node_border
    );
    let _ = write!(
        &mut out,
        r#"#{} .node .label{{text-align:center;}}"#,
        diagram_id
    );
    let _ = write!(
        &mut out,
        r#"#{} .node.clickable{{cursor:pointer;}}"#,
        diagram_id
    );
    let _ = write!(
        &mut out,
        r#"#{} .arrowheadPath{{fill:{};}}"#,
        diagram_id, theme.arrowhead_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgePaths .path{{stroke:{};stroke-width:1.5px;}}"#,
        diagram_id, line_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .flowchart-link{{stroke:{};fill:none;}}"#,
        diagram_id, line_color
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgeLabel{{background-color:{};text-align:center;}}"#,
        diagram_id, theme.edge_label_background
    );
    let _ = write!(
        &mut out,
        r#"#{} .edgeLabel rect{{opacity:0.5;}}"#,
        diagram_id
    );
    let _ = write!(
        &mut out,
        r#"#{} .cluster text{{fill:{};}}"#,
        diagram_id, theme.title_color
    );
    let _ = write!(
        &mut out,
        r#"#{} div.mermaidTooltip{{position:absolute;text-align:center;max-width:200px;padding:2px;font-family:{};font-size:12px;background:{};border:1px solid {};border-radius:2px;pointer-events:none;z-index:100;}}"#,
        diagram_id, font, theme.tertiary_color, theme.border2
    );
    for (i, fill) in theme.fill_types.iter().enumerate() {
        let _ = write!(
            &mut out,
            r#"#{} .task-type-{},#{} .section-type-{}{{fill:{};}}"#,
            diagram_id, i, diagram_id, i, fill
        );
    }
    for (i, fill) in theme.actor_colors.iter().enumerate() {
        if let Some(fill) = fill {
            let _ = write!(
                &mut out,
                r#"#{} .actor-{}{{fill:{};}}"#,
                diagram_id, i, fill
            );
        }
    }
    let _ = write!(
        &mut out,
        r#"#{} .label-icon{{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}}"#,
        diagram_id
    );
    let _ = write!(
        &mut out,
        r#"#{} .node .label-icon path{{fill:currentColor;stroke:revert;stroke-width:revert;}}"#,
        diagram_id
    );

    common
        .write_root(&mut out, diagram_id, diagram_id)
        .expect("String-backed Journey root CSS emission cannot fail");
    out
}

/// Emits a complete Journey document for the shared terminal SVG pipeline.
pub fn render_journey_svg(
    layout: &JourneyDiagramLayout,
    model: &JourneyDiagramModel,
    style: &JourneyStyle,
    diagram_title: Option<&str>,
    diagram_id: SvgId<'_>,
    viewport: &root_svg::RootViewportContext<'_>,
    work_meter: &OperationWorkMeter,
) -> Result<root_svg::RootedSvg> {
    let diagram_title = layout
        .title
        .as_deref()
        .or(diagram_title)
        .map(str::trim)
        .filter(|t| !t.is_empty());
    let title_from_meta = layout.title.is_none() && diagram_title.is_some();

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: -JOURNEY_VIEWBOX_TOP_PAD_PX,
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
        vb_h += JOURNEY_TITLE_EXTRA_HEIGHT_PX;
    }

    let native = style.native();
    let render_settings = style.render_settings();
    let task_font_size = render_settings.task_text_style.font_size;
    let task_font_family = render_settings
        .task_text_style
        .font_family
        .as_deref()
        .unwrap_or("\"Open Sans\", sans-serif");
    let title_font_size = render_settings.title_font_size.as_str();
    let title_font_family = render_settings.title_font_family.as_str();
    let title_color = &render_settings.title_color;

    let mut out = BoundedSvgOutput::new(work_meter);
    let aria_labelledby = model
        .acc_title
        .as_deref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = model
        .acc_descr
        .as_deref()
        .map(|_| format!("chart-desc-{diagram_id}"));

    let svg_height = if vb_min_y < 0.0 {
        vb_h - vb_min_y
    } else {
        vb_h
    };
    let preserve_aspect_ratio: [(&str, &str); 1] = [("preserveAspectRatio", "xMinYMin meet")];
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "journey");
    root_chrome.extra_attrs = &preserve_aspect_ratio;
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom = root_svg::RootDomProfile {
        fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
        fixed_style_placement: root_svg::RootStylePlacement::Tail,
        responsive_height_placement: root_svg::RootResponsiveHeightPlacement::AfterExtraAttrs,
        trailing_newline: false,
        ..root_svg::RootDomProfile::default()
    };
    let root_document = viewport.write_open(
        &mut out,
        root_svg::RootViewportSpec::mermaid(
            root_svg::DiagramBounds::from_view_box(vb_min_x, vb_min_y, vb_w, vb_h),
            layout.use_max_width,
        )
        .with_mermaid_responsive_height(layout.use_max_width, svg_height)
        .with_fixed_size(layout.width, svg_height)
        .with_max_width(root_svg::RootMaxWidth::CssSixSignificant(layout.width)),
        root_chrome,
    )?;
    work_meter.checkpoint(OperationPhase::Emit)?;

    if let Some(title) = model.acc_title.as_deref() {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id,
            text = escape_xml(title)
        );
    }
    if let Some(desc) = model.acc_descr.as_deref() {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id,
            text = escape_xml(desc)
        );
    }
    out.checkpoint()?;

    out.push_str("<style>");
    if let Some(native) = native {
        let id = CssSelectorDiagramId(diagram_id);
        let mut css = String::new();
        style.common_css().write_prefix(&mut css, id)?;
        let _ = write!(
            css,
            "#{id}{{font-weight:{weight};}}\
             #{id} .legend{{fill:{text};}}\
             #{id} .task{{stroke-width:{stroke_width};}}\
             #{id} .mouth{{fill:currentColor;stroke:currentColor;}}",
            weight = native.label_weight,
            text = native.text,
            stroke_width = fmt(native.border_width),
        );
        XmlTextOutput::new(&mut out).push_str(&css);
    } else if let Some(mermaid) = style.css_binding() {
        let css = journey_css(diagram_id, mermaid, style.common_css());
        XmlTextOutput::new(&mut out).push_str(&css);
    }
    out.push_str("</style>");
    out.push_str(r#"<g/>"#);
    let arrowhead_id = scoped_svg_id(diagram_id, "arrowhead");
    let arrowhead_url = scoped_svg_url(diagram_id, "arrowhead");
    let arrow_fill = native
        .map(|style| format!(r#" fill="{}""#, escape_attr(&style.arrow)))
        .unwrap_or_default();
    let _ = write!(
        &mut out,
        r#"<defs><marker id="{}" refX="5" refY="2" markerWidth="6" markerHeight="4" orient="auto"><path d="M 0,0 V 4 L6,2 Z"{arrow_fill}/></marker></defs>"#,
        escape_attr_display(arrowhead_id)
    );
    out.checkpoint()?;

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
            let line_esc = escape_xml(&line.text);
            let _ = write!(
                &mut out,
                r#"<text x="{x}" y="{y}" class="legend"><tspan x="{tx}">{text}</tspan></text>"#,
                x = fmt(line.x),
                y = fmt(line.y),
                tx = fmt(line.tspan_x),
                text = line_esc,
            );
        }
        out.checkpoint()?;
    }

    let mut section_iter = layout.sections.iter();
    let mut section_index = 0usize;
    let mut section_colour = Some("black");
    // Match layout's empty initial section: tasks before the first section have no header.
    let mut last_section = "";
    for task in &layout.tasks {
        if last_section != task.section {
            let section = section_iter.next().ok_or_else(|| Error::InvalidModel {
                message: "Journey section layout is missing a task's section header".to_owned(),
            })?;
            let section_class = format!("journey-section section-type-{}", section.num);
            section_colour = render_settings
                .section_colours
                .get(section_index % render_settings.section_colours.len().max(1))
                .map(String::as_str);
            let native_ink = native.map(|style| {
                if style.section_colors_owned
                    && let Some(color) = section_colour
                {
                    return std::borrow::Cow::Borrowed(color);
                }
                style.section_ink(section_index, section.num as usize)
            });
            write_section_rect(&mut out, section, native);
            write_text_candidate(
                &mut out,
                &section.section,
                &section_class,
                JourneyTextBox {
                    x: section.x,
                    y: section.y,
                    width: section.width,
                    height: section.height,
                },
                JourneyTextStyle {
                    placement: render_settings.text_placement,
                    task_font_size,
                    task_font_family,
                    native_weight: native.map(|style| style.label_weight),
                    section_colour,
                    native_fill: native_ink.as_deref(),
                },
                style.text_color(),
            )?;
            out.push_str("</g>");
            out.checkpoint()?;
            section_index += 1;
        }

        last_section = task.section.as_str();

        let line_stroke = native.map_or("#666", |style| style.connector.as_str());
        let _ = write!(
            &mut out,
            r#"<g><line id="{id}" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" class="task-line" stroke-width="1px" stroke-dasharray="4 2" stroke="{line_stroke}"/>"#,
            id = escape_attr_display(scoped_svg_id(diagram_id, &task.line_id)),
            x1 = fmt(task.line_x1),
            y1 = fmt(task.line_y1),
            x2 = fmt(task.line_x2),
            y2 = fmt(task.line_y2),
            line_stroke = escape_attr(line_stroke),
        );
        work_meter.checkpoint(OperationPhase::Emit)?;

        let face_paint = native.map(|style| style.face_paint(&task.mouth));
        let face_attributes = face_paint.as_ref().map_or_else(String::new, |(fill, ink)| {
            format!(r#" fill="{}" stroke="{ink}""#, escape_attr(fill))
        });
        let face_ink = face_paint.as_ref().map_or("#666", |(_, ink)| *ink);
        let _ = write!(
            &mut out,
            r#"<circle cx="{cx}" cy="{cy}" class="face" r="{r}" stroke-width="2" overflow="visible"{face_attributes}/>"#,
            cx = fmt(task.face_cx),
            cy = fmt_task_face_y(task.face_cy),
            r = fmt(JOURNEY_FACE_RADIUS_PX),
        );
        if native.is_some() {
            let _ = write!(&mut out, r#"<g color="{face_ink}">"#);
        } else {
            out.push_str("<g>");
        }
        let eye_dx = JOURNEY_FACE_RADIUS_PX / 3.0;
        let eye_r = 1.5;
        let _ = write!(
            &mut out,
            r#"<circle cx="{cx}" cy="{cy}" r="{r}" stroke-width="2" fill="{face_ink}" stroke="{face_ink}"/>"#,
            cx = fmt(task.face_cx - eye_dx),
            cy = fmt_task_face_y(task.face_cy.map(|v| v - eye_dx)),
            r = fmt(eye_r),
        );
        let _ = write!(
            &mut out,
            r#"<circle cx="{cx}" cy="{cy}" r="{r}" stroke-width="2" fill="{face_ink}" stroke="{face_ink}"/>"#,
            cx = fmt(task.face_cx + eye_dx),
            cy = fmt_task_face_y(task.face_cy.map(|v| v - eye_dx)),
            r = fmt(eye_r),
        );

        match task.mouth {
            JourneyMouthKind::Smile => {
                let _ = write!(
                    &mut out,
                    r#"<path class="mouth" d="M7.5,0A7.5,7.5,0,1,1,-7.5,0L-6.818,0A6.818,6.818,0,1,0,6.818,0Z" transform="translate({x},{y})"/>"#,
                    x = fmt(task.face_cx),
                    y = fmt_task_face_y(task.face_cy.map(|v| v + 2.0)),
                );
            }
            JourneyMouthKind::Sad => {
                let _ = write!(
                    &mut out,
                    r#"<path class="mouth" d="M-7.5,0A7.5,7.5,0,1,1,7.5,0L6.818,0A6.818,6.818,0,1,0,-6.818,0Z" transform="translate({x},{y})"/>"#,
                    x = fmt(task.face_cx),
                    y = fmt_task_face_y(task.face_cy.map(|v| v + 7.0)),
                );
            }
            JourneyMouthKind::Ambivalent => {
                let _ = write!(
                    &mut out,
                    r#"<line class="mouth" stroke="{face_ink}" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="1px"/>"#,
                    x1 = fmt(task.face_cx - 5.0),
                    y1 = fmt_task_face_y(task.face_cy.map(|v| v + 7.0)),
                    x2 = fmt(task.face_cx + 5.0),
                    y2 = fmt_task_face_y(task.face_cy.map(|v| v + 7.0)),
                );
            }
        }

        out.push_str("</g>");

        let native_radius = native.map(|style| fmt(style.radius).to_string());
        write_task_rect(
            &mut out,
            task,
            native_radius.as_deref().unwrap_or("3"),
            native.map(|style| style.border.as_str()),
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
            out.checkpoint()?;
        }

        // Pre-section tasks use the literal black baseline rather than sectionColours.
        let uses_section_colour = section_index != 0 && section_colour.is_some();
        let task_text_fill = native.map(|style| {
            if uses_section_colour && style.section_colors_owned {
                section_colour.unwrap_or(&style.text)
            } else {
                &style.text
            }
        });
        write_text_candidate(
            &mut out,
            &task.task,
            "task",
            JourneyTextBox {
                x: task.x,
                y: task.y,
                width: task.width,
                height: task.height,
            },
            JourneyTextStyle {
                placement: render_settings.text_placement,
                task_font_size,
                task_font_family,
                native_weight: native.map(|style| style.label_weight),
                section_colour,
                native_fill: task_text_fill,
            },
            style.text_color(),
        )?;

        out.push_str("</g>");
        out.checkpoint()?;
    }

    if let Some(title) = diagram_title {
        let title_weight =
            native.map_or_else(|| "bold".to_owned(), |style| style.title_weight.to_string());
        let _ = write!(
            &mut out,
            r#"<text x="{x}" font-size="{fs}" font-weight="{title_weight}" y="{y}" fill="{fill}" font-family="{ff}">{text}</text>"#,
            x = fmt(layout.title_x),
            fs = escape_attr(title_font_size),
            y = fmt(layout.title_y),
            fill = escape_attr(title_color),
            ff = escape_attr(title_font_family),
            text = escape_xml(title),
        );
        out.checkpoint()?;
    }

    let axis_stroke = native.map_or("black", |style| style.connector.as_str());
    let _ = write!(
        &mut out,
        r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4" stroke="{axis_stroke}" marker-end="{marker_end}"/>"#,
        x1 = fmt(layout.activity_line.x1),
        y1 = fmt(layout.activity_line.y1),
        x2 = fmt(layout.activity_line.x2),
        y2 = fmt(layout.activity_line.y2),
        axis_stroke = escape_attr(axis_stroke),
        marker_end = escape_attr_display(arrowhead_url),
    );

    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::super::{JourneyLineLayout, presentation_config};
    use super::*;
    use merman_model::family::DiagramFamilyId;
    use merman_render_core::{
        resources::{
            RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
        },
        svg_css::SvgDiagramIdProjection,
    };

    fn bounded_journey_layout() -> JourneyDiagramLayout {
        JourneyDiagramLayout {
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: -25.0,
                max_x: 500.0,
                max_y: 515.0,
            }),
            left_margin: 150.0,
            max_actor_label_width: 0.0,
            width: 500.0,
            height: 470.0,
            svg_height: 565.0,
            use_max_width: true,
            title: Some("Bounded journey".to_string()),
            title_x: 150.0,
            title_y: 25.0,
            actor_legend: Vec::new(),
            sections: vec![JourneySectionLayout {
                section: "Delivery".to_string(),
                num: 0,
                x: 150.0,
                y: 50.0,
                width: 150.0,
                height: 50.0,
                fill: "#191970".to_string(),
                task_count: 1,
            }],
            tasks: vec![JourneyTaskLayout {
                index: 0,
                section: "Delivery".to_string(),
                task: "Ship safely".to_string(),
                score: 5.0,
                x: 150.0,
                y: 110.0,
                width: 150.0,
                height: 50.0,
                fill: "#191970".to_string(),
                num: 0,
                people: Vec::new(),
                actor_circles: Vec::new(),
                line_id: "task0".to_string(),
                line_x1: 225.0,
                line_y1: 110.0,
                line_x2: 225.0,
                line_y2: 450.0,
                face_cx: 225.0,
                face_cy: Some(300.0),
                mouth: JourneyMouthKind::Smile,
            }],
            activity_line: JourneyLineLayout {
                x1: 150.0,
                y1: 200.0,
                x2: 346.0,
                y2: 200.0,
            },
        }
    }

    fn render_test_journey(
        layout: &JourneyDiagramLayout,
        style: &JourneyStyle,
        id: &str,
        policy: RenderResourcePolicy,
    ) -> Result<root_svg::RootedSvg> {
        let work_meter = OperationWorkMeter::new(policy);
        let projection = SvgDiagramIdProjection::new(&work_meter);
        let diagram_id = SvgId::new(id)
            .expect("valid Journey test ID")
            .with_projection(&projection);
        let viewport = root_svg::RootViewportContext::new(DiagramFamilyId::JOURNEY, diagram_id);
        let svg = render_journey_svg(
            layout,
            &JourneyDiagramModel::default(),
            style,
            None,
            diagram_id,
            &viewport,
            &work_meter,
        );
        projection.finish()?;
        svg
    }

    fn render_bounded_journey_with_policy(
        policy: RenderResourcePolicy,
    ) -> Result<root_svg::RootedSvg> {
        render_test_journey(
            &bounded_journey_layout(),
            &JourneyStyle::resolve(&presentation_config(serde_json::json!({})), None),
            "journey-bounded",
            policy,
        )
    }

    #[test]
    fn task_rect_preserves_geometry_and_encodes_native_stroke() {
        let mut task = bounded_journey_layout().tasks.remove(0);
        task.fill = "A&B".to_string();
        for (stroke, expected) in [(None, None), (Some("#654321"), Some("stroke:#654321;"))] {
            let mut out = String::new();
            write_task_rect(&mut out, &task, "9", stroke);
            let document = roxmltree::Document::parse(&out).unwrap();
            let rect = document.root_element();
            assert_eq!(rect.attribute("style"), expected);
            assert_eq!(rect.attribute("fill"), Some("A&B"));
            assert_eq!(rect.attribute("x"), Some("150"));
            assert_eq!(rect.attribute("y"), Some("110"));
            assert_eq!(rect.attribute("width"), Some("150"));
            assert_eq!(rect.attribute("height"), Some("50"));
            assert_eq!(rect.attribute("rx"), Some("9"));
            assert_eq!(rect.attribute("ry"), Some("9"));
            assert_eq!(rect.attribute("class"), Some("task task-type-0"));
        }
    }

    #[test]
    fn journey_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
        let baseline =
            render_bounded_journey_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
                .expect("render unbounded Journey SVG");
        let exact_bytes = baseline.len();
        assert!(exact_bytes > 1);

        let exact = render_bounded_journey_with_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
                .expect("valid exact Journey SVG ceiling"),
        )
        .expect("exact Journey SVG ceiling must succeed");
        assert_eq!(exact.as_bytes(), baseline.as_bytes());

        let below_exact = exact_bytes - 1;
        let error = render_bounded_journey_with_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
                .expect("valid below-exact Journey SVG ceiling"),
        )
        .expect_err("one byte below the Journey SVG size must fail");
        let Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected Journey MaxSvgBytes rejection, got {error}");
        };
        assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
        assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
        assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
        assert_eq!(limit.max, below_exact);
        assert!(limit.actual > limit.max);
    }

    #[test]
    fn journey_css_honors_mermaid_11_15_theme_options() {
        let cfg = serde_json::json!({
            "themeVariables": {
                "fontFamily": "\"ibm plex sans\", arial, sans-serif",
                "textColor": "#101010",
                "lineColor": "#202020",
                "faceColor": "#303030",
                "mainBkg": "#404040",
                "nodeBorder": "#505050",
                "arrowheadColor": "#606060",
                "edgeLabelBackground": "#707070",
                "titleColor": "#808080",
                "tertiaryColor": "#909090",
                "border2": "#a0a0a0",
                "fillType0": "#b0b0b0",
                "fillType1": "#c0c0c0",
                "actor0": "#d0d0d0",
                "actor1": "#e0e0e0"
            }
        });

        let typography = JourneyStyle::resolve(&presentation_config(cfg), None);
        let css = journey_css(
            "journey",
            typography.css_binding().unwrap(),
            typography.common_css(),
        );

        assert!(css.contains(
            r#"#journey .label{font-family:"ibm plex sans",arial,sans-serif;color:#101010;}"#
        ));
        assert!(css.contains(r#"#journey line{stroke:#101010;}"#));
        assert!(css.contains(r#"#journey .face{fill:#303030;stroke:#999;}"#));
        assert!(css.contains(r#"#journey .node rect,#journey .node circle,#journey .node ellipse,#journey .node polygon,#journey .node path{fill:#404040;stroke:#505050;stroke-width:1px;}"#));
        assert!(css.contains(r#"#journey .arrowheadPath{fill:#606060;}"#));
        assert!(css.contains(r#"#journey .edgePaths .path{stroke:#202020;stroke-width:1.5px;}"#));
        assert!(
            css.contains(r#"#journey .edgeLabel{background-color:#707070;text-align:center;}"#)
        );
        assert!(css.contains(r#"#journey .cluster text{fill:#808080;}"#));
        assert!(css.contains(r#"background:#909090;border:1px solid #a0a0a0;"#));
        assert!(css.contains(r#"#journey .task-type-0,#journey .section-type-0{fill:#b0b0b0;}"#));
        assert!(css.contains(r#"#journey .task-type-1,#journey .section-type-1{fill:#c0c0c0;}"#));
        assert!(css.contains(r#"#journey .actor-0{fill:#d0d0d0;}"#));
        assert!(css.contains(r#"#journey .actor-1{fill:#e0e0e0;}"#));
        assert!(css.contains(r#"#journey .task-type-7,#journey .section-type-7{fill:hsl(188, 100%, 93.5294117647%);}"#));
        assert!(!css.contains("#journey .actor-5{"));
        assert!(css.contains(r#"#journey .flowchart-link{stroke:#202020;fill:none;}"#));
    }

    #[test]
    fn journey_css_uses_default_font_and_edge_color() {
        let cfg = serde_json::json!({});
        let typography = JourneyStyle::resolve(&presentation_config(cfg), None);
        let css = journey_css(
            "journey",
            typography.css_binding().unwrap(),
            typography.common_css(),
        );

        assert!(css.contains(
            r#"#journey .label{font-family:"trebuchet ms",verdana,arial,sans-serif;color:#333;}"#
        ));
        assert!(css.contains(r#"#journey .edgePaths .path{stroke:#333333;stroke-width:1.5px;}"#));
        assert!(css.contains(r#"#journey .flowchart-link{stroke:#333333;fill:none;}"#));
        assert!(css.contains(r#"#journey .face{fill:#FFF8DC;stroke:#999;}"#));
        assert!(css.contains(r#"fill:#ECECFF;stroke:#9370DB;stroke-width:1px;"#));
        assert!(css.contains(r#"#journey .arrowheadPath{fill:#333333;}"#));
        assert!(css.contains(
            r#"#journey .edgeLabel{background-color:rgba(232,232,232, 0.8);text-align:center;}"#
        ));
        assert!(css.contains(r#"#journey .cluster text{fill:#333;}"#));
        assert!(
            css.contains(r#"background:hsl(80, 100%, 96.2745098039%);border:1px solid #aaaa33;"#)
        );
        assert!(css.contains(r#"#journey .task-type-0,#journey .section-type-0{fill:#ECECFF;}"#));
        assert!(css.contains(r#"#journey .task-type-1,#journey .section-type-1{fill:#ffffde;}"#));
        assert!(css.contains(r#"#journey .task-type-2,#journey .section-type-2{fill:hsl(304, 100%, 96.2745098039%);}"#));
        assert!(css.contains(r#"#journey .task-type-7,#journey .section-type-7{fill:hsl(188, 100%, 93.5294117647%);}"#));
        assert_eq!(typography.css_binding().unwrap().fill_types.len(), 8);
        assert!(!css.contains("#journey .actor-"));
    }

    #[test]
    fn journey_rect_fills_use_layout_section_fills_not_theme_fill_types() {
        let cfg = serde_json::json!({
            "themeVariables": {
                "fillType0": "#b0b0b0"
            }
        });
        let layout = JourneyDiagramLayout {
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: -25.0,
                max_x: 500.0,
                max_y: 515.0,
            }),
            left_margin: 150.0,
            max_actor_label_width: 0.0,
            width: 500.0,
            height: 470.0,
            svg_height: 565.0,
            use_max_width: true,
            title: Some("Font size precedence should be deterministic".to_string()),
            title_x: 150.0,
            title_y: 25.0,
            actor_legend: Vec::new(),
            sections: vec![JourneySectionLayout {
                section: "A".to_string(),
                num: 0,
                x: 150.0,
                y: 50.0,
                width: 150.0,
                height: 50.0,
                fill: "#191970".to_string(),
                task_count: 1,
            }],
            tasks: vec![JourneyTaskLayout {
                index: 0,
                section: "A".to_string(),
                task: "Hello".to_string(),
                score: 5.0,
                x: 150.0,
                y: 110.0,
                width: 150.0,
                height: 50.0,
                fill: "#191970".to_string(),
                num: 0,
                people: Vec::new(),
                actor_circles: Vec::new(),
                line_id: "task0".to_string(),
                line_x1: 225.0,
                line_y1: 110.0,
                line_x2: 225.0,
                line_y2: 450.0,
                face_cx: 225.0,
                face_cy: Some(300.0),
                mouth: JourneyMouthKind::Smile,
            }],
            activity_line: JourneyLineLayout {
                x1: 150.0,
                y1: 200.0,
                x2: 346.0,
                y2: 200.0,
            },
        };
        let typography_theme = JourneyStyle::resolve(&presentation_config(cfg.clone()), None);

        let svg = render_test_journey(
            &layout,
            &typography_theme,
            "journey",
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();

        assert!(svg.contains(r#"#journey .task-type-0,#journey .section-type-0{fill:#b0b0b0;}"#));
        assert!(svg.contains(r##"<rect x="150" y="50" fill="#191970" stroke="#666" width="150" height="50" rx="3" ry="3" class="journey-section section-type-0"/>"##));
        assert!(svg.contains(r##"<rect x="150" y="110" fill="#191970" stroke="#666" width="150" height="50" rx="3" ry="3" class="task task-type-0"/>"##));
        assert!(!svg.contains(r##"<rect x="150" y="50" fill="#b0b0b0""##));
        assert!(!svg.contains(r##"<rect x="150" y="110" fill="#b0b0b0""##));
    }

    #[test]
    fn journey_root_honors_disabled_max_width() {
        let layout = JourneyDiagramLayout {
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: -25.0,
                max_x: 320.0,
                max_y: 175.0,
            }),
            left_margin: 150.0,
            max_actor_label_width: 0.0,
            width: 320.0,
            height: 200.0,
            svg_height: 225.0,
            use_max_width: false,
            title: None,
            title_x: 150.0,
            title_y: 25.0,
            actor_legend: Vec::new(),
            sections: Vec::new(),
            tasks: Vec::new(),
            activity_line: JourneyLineLayout {
                x1: 150.0,
                y1: 200.0,
                x2: 166.0,
                y2: 200.0,
            },
        };
        let typography_theme =
            JourneyStyle::resolve(&presentation_config(serde_json::json!({})), None);

        let svg = render_test_journey(
            &layout,
            &typography_theme,
            "journeyFixed",
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let root_open = svg.split_once('>').expect("root svg open tag").0;

        assert!(root_open.contains(r#"width="320""#), "{root_open}");
        assert!(root_open.contains(r#"height="225""#), "{root_open}");
        assert!(
            root_open.contains(r#"viewBox="0 -25 320 200""#),
            "{root_open}"
        );
        assert!(
            root_open.contains(r#"preserveAspectRatio="xMinYMin meet""#),
            "{root_open}"
        );
        assert!(!root_open.contains("style="), "{root_open}");
        assert!(!root_open.contains(r#"width="100%""#), "{root_open}");
        assert!(!root_open.contains("max-width"), "{root_open}");
    }
}
