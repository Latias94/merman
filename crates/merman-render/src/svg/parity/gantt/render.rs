use super::super::*;
use super::css::gantt_css;
use crate::gantt::{GanttNativeStyle, GanttStyle};
use merman_model::families::gantt::GanttDiagramRenderModel;

// Gantt diagram SVG renderer implementation (split from parity.rs).

fn gantt_scale_time_round(ms: i64, min_ms: i64, max_ms: i64, range: f64) -> f64 {
    if max_ms <= min_ms {
        // D3 scaleTime returns the midpoint of the range for degenerate domains.
        return (range / 2.0).round();
    }
    let t = (ms - min_ms) as f64 / (max_ms - min_ms) as f64;
    (t * range).round()
}

fn fmt_allow_nan(v: f64) -> String {
    if v.is_nan() {
        return "NaN".to_string();
    }
    fmt_string(v)
}

fn gantt_dom_id(diagram_id: SvgDiagramId<'_>, raw_id: &str) -> String {
    format!("{diagram_id}-{raw_id}")
}

fn gantt_insert_before_width(base: &str, insert: &str) -> String {
    let insert = insert.trim();
    if insert.is_empty() {
        return base.to_string();
    }
    let mut parts: Vec<&str> = base.split_whitespace().collect();
    let insert_parts: Vec<&str> = insert.split_whitespace().collect();
    let idx = parts.iter().position(|p| p.starts_with("width-"));
    match idx {
        Some(i) => {
            for (off, p) in insert_parts.iter().enumerate() {
                parts.insert(i + off, p);
            }
        }
        None => parts.extend(insert_parts),
    }
    parts.join(" ")
}

fn render_gantt_axis_ticks(
    out: &mut impl SvgOutput,
    ticks: &[crate::model::GanttAxisTickLayout],
    left_padding: f64,
    tick_size: f64,
    with_dy: bool,
) -> Result<()> {
    for t in ticks {
        let tx = (t.x - left_padding) + 0.5;
        let _ = write!(
            out,
            r#"<g class="tick" opacity="1" transform="translate({},0)">"#,
            fmt(tx)
        );
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"<line stroke="currentColor" y2="{}"/>"#,
            fmt(tick_size)
        );
        out.checkpoint()?;
        if with_dy {
            let _ = write!(
                out,
                r##"<text fill="#000" y="3" dy="1em" stroke="none" font-size="10" style="text-anchor: middle;">{}</text>"##,
                escape_xml(&t.label)
            );
        } else {
            let _ = write!(
                out,
                r##"<text fill="#000" y="-3" dy="0em" stroke="none" font-size="10" style="text-anchor: middle;">{}</text>"##,
                escape_xml(&t.label)
            );
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }

    Ok(())
}

fn render_gantt_axis_group(
    out: &mut impl SvgOutput,
    layout: &crate::model::GanttDiagramLayout,
    ticks: &[crate::model::GanttAxisTickLayout],
    y: f64,
    with_dy: bool,
) -> Result<()> {
    let range = (layout.width - layout.left_padding - layout.right_padding).max(1.0);
    // Mermaid renders two possible axis grids:
    // - bottom axis (ticks extend upward, label baseline uses `dy="1em"` at `y="3"`)
    // - optional top axis (ticks extend downward, labels use `dy="0em"` at `y="-3"`)
    let tick_size = if with_dy {
        -layout.height + layout.top_padding + layout.grid_line_start_padding
    } else {
        layout.height - layout.top_padding - layout.grid_line_start_padding
    };

    let _ = write!(
        out,
        r#"<g class="grid" transform="translate({}, {})" fill="none" font-size="10" font-family="sans-serif" text-anchor="middle">"#,
        fmt(layout.left_padding),
        fmt(y)
    );
    out.checkpoint()?;

    let d = format!(
        "M0.5,{}V0.5H{}V{}",
        fmt(tick_size),
        fmt(range + 0.5),
        fmt(tick_size)
    );
    let _ = write!(
        out,
        r#"<path class="domain" stroke="currentColor" d="{}"/>"#,
        escape_attr(&d)
    );
    out.checkpoint()?;

    render_gantt_axis_ticks(out, ticks, layout.left_padding, tick_size, with_dy)?;

    out.push_str("</g>");
    out.checkpoint()
}

fn gantt_native_css(
    diagram_id: SvgDiagramId<'_>,
    native: &GanttNativeStyle,
    body_family: &str,
) -> String {
    let id = super::super::util::css_selector_diagram_id(diagram_id);
    let grid_stroke = if native.show_grid {
        &native.guide
    } else {
        "none"
    };
    format!(
        "#{id}{{font-family:{body_family};font-weight:{label_weight};font-size:{tick_size}px;}}\
         #{id} .exclude-range{{fill:{excluded_fill};fill-opacity:{excluded_opacity};}}\
         #{id} .grid .tick line{{stroke:{grid_stroke};shape-rendering:crispEdges;}}\
         #{id} .grid .tick text{{font-family:{body_family};font-size:{tick_size}px;fill:{text};}}\
         #{id} .grid path{{stroke-width:0;}}\
         #{id} .today{{fill:none;stroke:{today};stroke-width:2px;}}\
         #{id} .task{{stroke-width:{bar_stroke_width};}}\
         #{id} .taskText,#{id} .vertText{{text-anchor:middle;}}\
         #{id} .sectionTitle,#{id} .taskTextOutsideRight{{text-anchor:start;}}\
         #{id} .taskTextOutsideLeft{{text-anchor:end;}}\
         #{id} .clickable{{cursor:pointer;font-weight:bold;}}\
         #{id} .milestoneText{{font-style:italic;}}\
         #{id} .sectionTitle{{fill:{title_color};}}\
         #{id} .titleText{{text-anchor:middle;font-family:{title_family};\
         font-size:{title_size}px;font-weight:{title_weight};fill:{title_color};}}",
        label_weight = native.label_weight,
        excluded_fill = native.excluded_fill,
        excluded_opacity = native.excluded_opacity,
        tick_size = fmt(native.tick_size),
        text = native.text,
        today = native.today,
        bar_stroke_width = fmt(native.bar_stroke_width),
        title_color = native.title_color,
        title_family = native.title_family,
        title_size = fmt(native.title_size),
        title_weight = native.title_weight,
    )
}

fn render_gantt_rows(
    out: &mut impl SvgOutput,
    layout: &GanttDiagramLayout,
    native: Option<&GanttNativeStyle>,
) -> Result<()> {
    if layout.rows.is_empty() {
        out.push_str("<g/>");
    } else {
        out.push_str("<g>");
        out.checkpoint()?;
        for row in &layout.rows {
            let _ = write!(
                out,
                r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" class="{cls}""#,
                x = fmt(row.x),
                y = fmt(row.y),
                w = fmt(row.width),
                h = fmt(row.height),
                cls = escape_attr(&row.class),
            );
            if let Some(native) = native {
                let _ = write!(
                    out,
                    r#" fill="{}""#,
                    escape_attr(native.row_fill(&row.class))
                );
            }
            out.push_str("/>");
            out.checkpoint()?;
        }
        out.push_str("</g>");
    }
    out.checkpoint()
}

pub(crate) fn render_gantt_diagram_svg_model(
    layout: &crate::model::GanttDiagramLayout,
    model: &GanttDiagramRenderModel,
    style: &GanttStyle,
    task_source_indices: &[usize],
    effective_config: &dyn crate::config::ConfigLookup,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id_or("merman");
    let native = style.native();

    let w = layout.width.max(1.0);
    let h = layout.height.max(1.0);

    let acc_title = model.acc_title.as_deref().filter(|s| !s.is_empty());
    let acc_descr = model.acc_descr.as_deref().filter(|s| !s.is_empty());

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let aria_labelledby = acc_title
        .as_ref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr
        .as_ref()
        .map(|_| format!("chart-desc-{diagram_id}"));
    // The title can extend the viewport beyond the schedule's time scale and rows.
    let root_bounds = layout.bounds.as_ref().map_or_else(
        || root_svg::DiagramBounds::from_view_box(0.0, 0.0, w, h),
        |bounds| {
            root_svg::DiagramBounds::from_extents(
                bounds.min_x,
                bounds.min_y,
                bounds.max_x.max(w),
                bounds.max_y.max(h),
                0.0,
            )
        },
    );
    let use_max_width = config_bool(effective_config, &["gantt", "useMaxWidth"]).unwrap_or(true);
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, use_max_width)
        .with_max_width(root_svg::RootMaxWidth::SvgNumber(root_bounds.width));
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "gantt");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.style_viewbox_order = root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle;
    root_chrome.dom.trailing_newline = false;
    let root_document = options
        .root_viewport(crate::DiagramFamilyId::GANTT, diagram_id)
        .write_open(&mut out, root_spec, root_chrome)?;

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{id}">{text}</title>"#,
            id = diagram_id,
            text = escape_xml(title)
        );
    }
    if let Some(descr) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{id}">{text}</desc>"#,
            id = diagram_id,
            text = escape_xml(descr)
        );
    }
    out.checkpoint()?;

    out.push_str("<style>");
    out.checkpoint()?;
    let css = match native {
        Some(native) => gantt_native_css(diagram_id, native, style.font_family_css()),
        None => gantt_css(diagram_id, style.css_binding()),
    };
    super::super::output::XmlTextOutput::new(&mut out).push_str(&css);
    drop(css);
    out.checkpoint()?;
    out.push_str(r#"</style><g/>"#);
    out.checkpoint()?;

    let (min_ms, max_ms) = match (
        layout.tasks.iter().map(|t| t.start_ms).min(),
        layout.tasks.iter().map(|t| t.end_ms).max(),
    ) {
        (Some(a), Some(b)) => (a, b),
        _ => (0, 0),
    };
    let range = (w - layout.left_padding - layout.right_padding).max(1.0);
    let gap = layout.bar_height + layout.bar_gap;
    // Mermaid's Gantt renderer assigns this custom attribute before the final SVG cleanup.
    // Loose security skips DOMPurify, while every sanitized mode removes the attribute.
    let preserve_task_text_height = effective_config
        .get("securityLevel")
        .and_then(serde_json::Value::as_str)
        == Some("loose");
    let local_time_zone = options.local_time_zone();
    let min_day_start_ms = crate::gantt::start_of_day_ms(min_ms, local_time_zone).unwrap_or(min_ms);
    let min_in_day_offset_ms = (min_ms - min_day_start_ms).max(0);

    if native.is_some() {
        render_gantt_rows(&mut out, layout, native)?;
    }

    // Exclude layer (drawn before the grid in Mermaid).
    if layout.has_excludes_layer {
        if layout.excludes.is_empty() {
            out.push_str("<g/>");
        } else {
            out.push_str("<g>");
            out.checkpoint()?;
            for (i, r) in layout.excludes.iter().enumerate() {
                // Mermaid's gantt exclude rectangles use a slightly unintuitive origin:
                //
                // - `x`/`width` are based on `startOf('day')` / `endOf('day')`
                // - `transform-origin` uses the raw `dayjs(minTime)` time-of-day offset:
                //   `timeScale(d.start) + sidePad + 0.5*(timeScale(d.end)-timeScale(d.start))`
                //
                // This matters when date-only inputs are parsed with a timezone-derived offset
                // (e.g. `YYYY-MM-DD` treated as UTC midnight and shifted when rendered locally).
                let start_day_start_ms = crate::gantt::start_of_day_ms(r.start_ms, local_time_zone)
                    .unwrap_or(r.start_ms);
                let end_day_start_ms =
                    crate::gantt::start_of_day_ms(r.end_ms, local_time_zone).unwrap_or(r.end_ms);
                let start_raw_ms = start_day_start_ms.saturating_add(min_in_day_offset_ms);
                let end_raw_ms = end_day_start_ms.saturating_add(min_in_day_offset_ms);

                let start_x = gantt_scale_time_round(start_raw_ms, min_ms, max_ms, range);
                let end_x = gantt_scale_time_round(end_raw_ms, min_ms, max_ms, range);
                let cx = start_x + layout.left_padding + 0.5 * (end_x - start_x);
                let cy = (i as f64) * gap + 0.5 * h;
                let terminal_id = gantt_dom_id(diagram_id, &r.id);
                let _ = write!(
                    &mut out,
                    r#"<rect id="{id}" x="{x}" y="{y}" width="{w}" height="{h}" transform-origin="{cx}px {cy}px" class="exclude-range"/>"#,
                    id = escape_attr(&terminal_id),
                    x = fmt(r.x),
                    y = fmt(r.y),
                    w = fmt(r.width),
                    h = fmt(r.height),
                    cx = fmt_allow_nan(cx),
                    cy = fmt_allow_nan(cy),
                );
                out.checkpoint()?;
            }
            out.push_str("</g>");
        }
        out.checkpoint()?;
    }

    let bottom_axis_y = h - layout.top_padding;
    render_gantt_axis_group(&mut out, layout, &layout.bottom_ticks, bottom_axis_y, true)?;

    if layout.top_axis {
        render_gantt_axis_group(
            &mut out,
            layout,
            &layout.top_ticks,
            layout.top_padding,
            false,
        )?;
    }

    if native.is_none() {
        render_gantt_rows(&mut out, layout, None)?;
    }

    let mut tasks_in_draw_order: Vec<_> = layout.tasks.iter().enumerate().collect();
    tasks_in_draw_order.sort_by_key(|(_, task)| task.vert);
    let source_task = |layout_index: usize| {
        task_source_indices
            .get(layout_index)
            .and_then(|&index| model.tasks.get(index))
    };

    if layout.tasks.is_empty() {
        out.push_str("<g/>");
    } else {
        out.push_str("<g>");
        out.checkpoint()?;

        for &(task_index, t) in &tasks_in_draw_order {
            let start_x = gantt_scale_time_round(t.start_ms, min_ms, max_ms, range);
            let end_x = gantt_scale_time_round(t.end_ms, min_ms, max_ms, range);
            let center_x = start_x + layout.left_padding + 0.5 * (end_x - start_x);
            let center_y = (t.order as f64) * gap + layout.top_padding + 0.5 * layout.bar_height;
            let origin = format!(
                "{}px {}px",
                fmt_allow_nan(center_x),
                fmt_allow_nan(center_y)
            );

            let _ = write!(&mut out, r#"<rect"#);
            let rx = fmt(t.bar.rx);
            let ry = fmt(t.bar.ry);
            let terminal_id = gantt_dom_id(diagram_id, &t.bar.id);
            let _ = write!(&mut out, r#" id="{}""#, escape_attr(&terminal_id));
            let semantic_task = source_task(task_index);
            if let Some((native, task)) = native.zip(semantic_task) {
                let (fill, stroke) = native.task_paint(task);
                let _ = write!(
                    &mut out,
                    r#" fill="{}" stroke="{}""#,
                    escape_attr(fill),
                    escape_attr(stroke),
                );
                if native.bar_wireframe && !task.milestone && !task.vert {
                    out.push_str(r#" fill-opacity="0.28""#);
                }
                if task.milestone {
                    let _ = write!(
                        &mut out,
                        r#" transform="translate({} {}) rotate(45) scale(0.8) translate({} {})""#,
                        fmt(center_x),
                        fmt(center_y),
                        fmt(-center_x),
                        fmt(-center_y),
                    );
                }
            }
            let _ = write!(
                &mut out,
                r#" rx="{rx}" ry="{ry}" x="{x}" y="{y}" width="{w}" height="{h}" transform-origin="{origin}" class="{cls}"/>"#,
                rx = rx,
                ry = ry,
                x = fmt(t.bar.x),
                y = fmt(t.bar.y),
                w = fmt(t.bar.width),
                h = fmt(t.bar.height),
                origin = escape_attr(&origin),
                cls = escape_attr(&t.bar.class),
            );
            out.checkpoint()?;
        }

        for &(task_index, t) in &tasks_in_draw_order {
            let base_class = &t.label.class;
            let mut task_type_class = String::new();
            let semantic_task = source_task(task_index);
            if let Some(task) = semantic_task {
                let section_suffix = crate::gantt::gantt_section_class_suffix(
                    &task.task_type,
                    &layout.categories,
                    layout.number_section_styles,
                );
                task_type_class = crate::gantt::label_state_classes(task, &section_suffix);
                if task.milestone {
                    if !task_type_class.is_empty() {
                        task_type_class.push(' ');
                    }
                    task_type_class.push_str("milestoneText");
                }
                if task.vert {
                    if !task_type_class.is_empty() {
                        task_type_class.push(' ');
                    }
                    task_type_class.push_str("vertText");
                }
            }

            let class = gantt_insert_before_width(base_class, &task_type_class);
            let _ = write!(
                &mut out,
                r#"<text id="{id}" font-size="{fs}" x="{x}" y="{y}""#,
                id = escape_attr(&gantt_dom_id(diagram_id, &t.label.id)),
                fs = escape_attr(style.css_binding().task_font_size.css()),
                x = fmt(t.label.x),
                y = fmt(t.label.y),
            );
            if let Some((native, task)) = native.zip(semantic_task) {
                let _ = write!(
                    &mut out,
                    r#" fill="{}""#,
                    escape_attr(native.task_ink(task, &class))
                );
            }
            options.checkpoint_emit()?;
            if preserve_task_text_height {
                let _ = write!(&mut out, r#" text-height="{}""#, fmt(layout.bar_height));
            }
            let _ = write!(
                &mut out,
                r#" class="{cls}">{txt}</text>"#,
                cls = escape_attr(&class),
                txt = escape_xml(&t.label.text),
            );
            out.checkpoint()?;
        }

        out.push_str("</g>");
    }
    out.checkpoint()?;

    if layout.section_titles.is_empty() {
        out.push_str("<g/>");
    } else {
        out.push_str("<g>");
        out.checkpoint()?;
        for st in &layout.section_titles {
            let _ = write!(
                &mut out,
                r#"<text dy="{dy}em" x="{x}" y="{y}" font-size="{fs}" class="{cls}">"#,
                dy = fmt(st.dy_em),
                x = fmt(st.x),
                y = fmt(st.y),
                fs = escape_attr(style.css_binding().section_font_size.css()),
                cls = escape_attr(&st.class),
            );
            out.checkpoint()?;
            for (j, line) in st.lines.iter().enumerate() {
                if j == 0 {
                    let _ = write!(
                        &mut out,
                        r#"<tspan alignment-baseline="central" x="{x}">{txt}</tspan>"#,
                        x = fmt(st.x),
                        txt = escape_xml(line)
                    );
                } else {
                    let _ = write!(
                        &mut out,
                        r#"<tspan alignment-baseline="central" x="{x}" dy="1em">{txt}</tspan>"#,
                        x = fmt(st.x),
                        txt = escape_xml(line)
                    );
                }
                out.checkpoint()?;
            }
            out.push_str("</text>");
            out.checkpoint()?;
        }
        out.push_str("</g>");
    }
    out.checkpoint()?;

    if model.today_marker.trim() != "off" {
        let today_x = if layout.tasks.is_empty() {
            f64::NAN
        } else {
            let now_ms = options.unix_ms();
            gantt_scale_time_round(now_ms, min_ms, max_ms, range) + layout.left_padding
        };
        let y1 = layout.title_top_margin;
        let y2 = h - layout.title_top_margin;
        out.push_str(r#"<g class="today">"#);
        let _ = write!(
            &mut out,
            r#"<line x1="{x}" x2="{x}" y1="{y1}" y2="{y2}" class="today""#,
            x = fmt_allow_nan(today_x),
            y1 = fmt(y1),
            y2 = fmt(y2),
        );
        let style_raw = model.today_marker.trim();
        if !style_raw.is_empty() {
            let mut style = style_raw.to_string();
            // Mermaid upstream mmdc output for `todayMarker stroke:#00f;opacity:0.5` ends up as
            // `style="stroke:&00f;opacity:0.5"` (note the `#` → `&`), while comma-separated style
            // strings preserve `#`. Mirror this quirk based on whether the raw marker contains `;`.
            if style.contains(';') {
                style = style.replace('#', "&");
            }
            style = style.replace(',', ";");
            let _ = write!(&mut out, r#" style="{}""#, escape_attr(&style));
        }
        out.push_str("/></g>");
        out.checkpoint()?;
    }

    let title = layout.title.as_deref().unwrap_or_default();
    let _ = write!(
        &mut out,
        r#"<text x="{x}" y="{y}" class="titleText">{txt}</text>"#,
        x = fmt(layout.title_x),
        y = fmt(layout.title_y),
        txt = escape_xml(title),
    );
    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn gantt_axis_stops_after_the_first_svg_sink_failure() {
        let ticks = (0..4)
            .map(|index| crate::model::GanttAxisTickLayout {
                time_ms: index,
                x: 100.0 + (index as f64) * 50.0,
                label: format!("tick-{index}"),
            })
            .collect::<Vec<_>>();
        let mut out = RejectAfterFirstWrite::default();

        let error = render_gantt_axis_ticks(&mut out, &ticks, 75.0, -115.0, true)
            .expect_err("the rejecting sink must stop Gantt axis rendering");

        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            out.write_attempts, 1,
            "Gantt axis rendering must stop after the first failed tick write"
        );
    }
}
