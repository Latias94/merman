use crate::Result;
use crate::model::{
    Bounds, TimelineDiagramLayout, TimelineLineLayout, TimelineNodeLayout, TimelineSectionLayout,
    TimelineTaskLayout,
};
use crate::text::{TextMeasurer, TextStyle};
use serde::Deserialize;

const MAX_SECTIONS: i64 = 12;

const BASE_MARGIN: f64 = 50.0;
const NODE_PADDING: f64 = 20.0;
const TASK_STEP_X: f64 = 200.0;
const TASK_CONTENT_WIDTH_DEFAULT: f64 = 150.0;
const EVENT_VERTICAL_OFFSET_FROM_TASK_Y: f64 = 200.0;
const EVENT_GAP_Y: f64 = 10.0;

const TITLE_Y: f64 = 20.0;
const DEFAULT_VIEWBOX_PADDING: f64 = 50.0;

#[derive(Debug, Clone, Deserialize)]
struct TimelineTaskModel {
    id: i64,
    section: String,
    #[serde(rename = "type")]
    task_type: String,
    task: String,
    score: i64,
    #[serde(default)]
    events: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct TimelineModel {
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
    #[serde(default)]
    sections: Vec<String>,
    #[serde(default)]
    tasks: Vec<TimelineTaskModel>,
    title: Option<String>,
    #[serde(rename = "type")]
    diagram_type: String,
}

fn cfg_f64(cfg: &serde_json::Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_f64()
}

fn cfg_bool(cfg: &serde_json::Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_bool()
}

fn section_index(full_section: i64) -> i64 {
    (full_section % MAX_SECTIONS) - 1
}

fn section_class(full_section: i64) -> String {
    format!("section-{}", section_index(full_section))
}

fn wrap_tokens(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut buf = String::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let ch = text[i..].chars().next().unwrap();
        if ch.is_whitespace() {
            if !buf.is_empty() {
                out.push(std::mem::take(&mut buf));
            }
            // Coalesce any whitespace run into a single token.
            while i < bytes.len() {
                let c = text[i..].chars().next().unwrap();
                if !c.is_whitespace() {
                    break;
                }
                i += c.len_utf8();
            }
            out.push(" ".to_string());
            continue;
        }

        let rest = &text[i..];
        if rest.starts_with("<br>") || rest.starts_with("<br/>") || rest.starts_with("<br />") {
            if !buf.is_empty() {
                out.push(std::mem::take(&mut buf));
            }
            if rest.starts_with("<br>") {
                i += "<br>".len();
            } else if rest.starts_with("<br/>") {
                i += "<br/>".len();
            } else {
                i += "<br />".len();
            }
            out.push("<br>".to_string());
            continue;
        }

        buf.push(ch);
        i += ch.len_utf8();
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

fn join_trim(tokens: &[String]) -> String {
    tokens.join(" ").trim().to_string()
}

fn wrap_lines(
    text: &str,
    max_width: f64,
    style: &TextStyle,
    measurer: &dyn TextMeasurer,
) -> Vec<String> {
    let tokens = wrap_tokens(text);
    if tokens.is_empty() {
        return vec![String::new()];
    }

    let mut lines: Vec<String> = Vec::new();
    let mut cur: Vec<String> = Vec::new();

    for tok in tokens {
        cur.push(tok.clone());
        let candidate = join_trim(&cur);
        let candidate_width = measurer.measure(&candidate, style).width;
        if candidate_width > max_width || tok == "<br>" {
            cur.pop();
            lines.push(join_trim(&cur));
            if tok == "<br>" {
                cur = vec![String::new()];
            } else {
                cur = vec![tok];
            }
        }
    }

    lines.push(join_trim(&cur));
    if lines.is_empty() {
        vec![String::new()]
    } else {
        lines
    }
}

fn text_bbox_height(lines: usize, font_size: f64) -> f64 {
    let lh = font_size.max(1.0) * 1.1;
    (lines.max(1) as f64) * lh
}

fn virtual_node_height(
    text: &str,
    content_width: f64,
    font_size: f64,
    padding: f64,
    measurer: &dyn TextMeasurer,
) -> (f64, Vec<String>) {
    let style = TextStyle {
        font_family: None,
        font_size,
        font_weight: None,
    };
    let lines = wrap_lines(text, content_width.max(1.0), &style, measurer);
    let bbox_h = text_bbox_height(lines.len(), font_size);
    let h = bbox_h + font_size.max(1.0) * 1.1 * 0.5 + padding;
    (h, lines)
}

fn compute_node(
    kind: &str,
    label: &str,
    full_section: i64,
    x: f64,
    y: f64,
    content_width: f64,
    max_height: f64,
    font_size: f64,
    measurer: &dyn TextMeasurer,
) -> TimelineNodeLayout {
    let (h0, label_lines) =
        virtual_node_height(label, content_width, font_size, NODE_PADDING, measurer);
    let height = h0.max(max_height).max(1.0);
    let width = (content_width + NODE_PADDING * 2.0).max(1.0);
    TimelineNodeLayout {
        x,
        y,
        width,
        height,
        content_width: content_width.max(1.0),
        padding: NODE_PADDING,
        section_class: section_class(full_section),
        label: label.to_string(),
        label_lines,
        kind: kind.to_string(),
    }
}

fn bounds_from_nodes_and_lines<'a, 'b>(
    nodes: impl IntoIterator<Item = &'a TimelineNodeLayout>,
    lines: impl IntoIterator<Item = &'b TimelineLineLayout>,
) -> Option<(f64, f64, f64, f64)> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    let mut any = false;
    for n in nodes {
        any = true;
        min_x = min_x.min(n.x);
        min_y = min_y.min(n.y);
        max_x = max_x.max(n.x + n.width);
        max_y = max_y.max(n.y + n.height);
    }
    for l in lines {
        any = true;
        min_x = min_x.min(l.x1.min(l.x2));
        min_y = min_y.min(l.y1.min(l.y2));
        max_x = max_x.max(l.x1.max(l.x2));
        max_y = max_y.max(l.y1.max(l.y2));
    }

    if any {
        Some((min_x, min_y, max_x, max_y))
    } else {
        None
    }
}

pub fn layout_timeline_diagram(
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
) -> Result<TimelineDiagramLayout> {
    let model: TimelineModel = serde_json::from_value(semantic.clone())?;
    let _ = (
        model.acc_title.as_deref(),
        model.acc_descr.as_deref(),
        model.diagram_type.as_str(),
    );

    let font_size = effective_config
        .get("fontSize")
        .and_then(|v| v.as_f64())
        .unwrap_or(16.0)
        .max(1.0);

    let left_margin = cfg_f64(effective_config, &["timeline", "leftMargin"])
        .unwrap_or(150.0)
        .max(0.0);
    let disable_multicolor =
        cfg_bool(effective_config, &["timeline", "disableMulticolor"]).unwrap_or(false);
    let task_content_width = cfg_f64(effective_config, &["timeline", "width"])
        .unwrap_or(TASK_CONTENT_WIDTH_DEFAULT)
        .max(1.0);

    let mut max_section_height: f64 = 0.0;
    for section in &model.sections {
        let (h, _lines) = virtual_node_height(
            section,
            task_content_width,
            font_size,
            NODE_PADDING,
            measurer,
        );
        max_section_height = max_section_height.max(h + 20.0);
    }

    let mut max_task_height: f64 = 0.0;
    let mut max_event_line_length: f64 = 0.0;
    for task in &model.tasks {
        let (h, _lines) = virtual_node_height(
            &task.task,
            task_content_width,
            font_size,
            NODE_PADDING,
            measurer,
        );
        max_task_height = max_task_height.max(h + 20.0);

        let mut task_event_len: f64 = 0.0;
        for ev in &task.events {
            let (eh, _lines) =
                virtual_node_height(ev, task_content_width, font_size, NODE_PADDING, measurer);
            task_event_len += eh;
        }
        if !task.events.is_empty() {
            task_event_len += (task.events.len().saturating_sub(1) as f64) * EVENT_GAP_Y;
        }
        max_event_line_length = max_event_line_length.max(task_event_len);
    }

    let base_x = BASE_MARGIN + left_margin;
    let base_y = BASE_MARGIN;

    let mut sections: Vec<TimelineSectionLayout> = Vec::new();
    let mut orphan_tasks: Vec<TimelineTaskLayout> = Vec::new();

    let mut all_nodes_pre_title: Vec<TimelineNodeLayout> = Vec::new();
    let mut all_lines_pre_title: Vec<TimelineLineLayout> = Vec::new();

    let has_sections = !model.sections.is_empty();

    if has_sections {
        let mut master_x = base_x;
        let section_y = base_y;

        for (section_number, section_label) in model.sections.iter().enumerate() {
            let section_number = section_number as i64;
            let tasks_for_section: Vec<&TimelineTaskModel> = model
                .tasks
                .iter()
                .filter(|t| t.section == *section_label)
                .collect();
            let tasks_for_section_count = tasks_for_section.len().max(1);

            let content_width = TASK_STEP_X * (tasks_for_section_count as f64) - 50.0;
            let section_node = compute_node(
                "section",
                section_label,
                section_number,
                master_x,
                section_y,
                content_width,
                max_section_height,
                font_size,
                measurer,
            );
            all_nodes_pre_title.push(section_node.clone());

            let mut tasks: Vec<TimelineTaskLayout> = Vec::new();
            let mut task_x = master_x;
            let task_y = section_y + max_section_height + 50.0;

            for task in &tasks_for_section {
                let full_section = section_number;
                let task_node = compute_node(
                    "task",
                    &task.task,
                    full_section,
                    task_x,
                    task_y,
                    task_content_width,
                    max_task_height,
                    font_size,
                    measurer,
                );
                all_nodes_pre_title.push(task_node.clone());

                let connector = TimelineLineLayout {
                    kind: "task-events".to_string(),
                    x1: task_x + (task_node.width / 2.0),
                    y1: task_y + max_task_height,
                    x2: task_x + (task_node.width / 2.0),
                    y2: task_y + max_task_height + 100.0 + max_event_line_length + 100.0,
                };
                all_lines_pre_title.push(connector.clone());

                let mut events: Vec<TimelineNodeLayout> = Vec::new();
                let mut event_y = task_y + EVENT_VERTICAL_OFFSET_FROM_TASK_Y;
                for ev in &task.events {
                    let event_node = compute_node(
                        "event",
                        ev,
                        full_section,
                        task_x,
                        event_y,
                        task_content_width,
                        50.0,
                        font_size,
                        measurer,
                    );
                    event_y += event_node.height + EVENT_GAP_Y;
                    all_nodes_pre_title.push(event_node.clone());
                    events.push(event_node);
                }

                tasks.push(TimelineTaskLayout {
                    node: task_node,
                    connector,
                    events,
                });

                task_x += TASK_STEP_X;
            }

            sections.push(TimelineSectionLayout {
                node: section_node,
                tasks,
            });

            master_x += TASK_STEP_X * (tasks_for_section_count as f64);
        }
    } else {
        let mut master_x = base_x;
        let master_y = base_y;
        let mut section_color: i64 = 0;

        for task in &model.tasks {
            let task_node = compute_node(
                "task",
                &task.task,
                section_color,
                master_x,
                master_y,
                task_content_width,
                max_task_height,
                font_size,
                measurer,
            );
            all_nodes_pre_title.push(task_node.clone());

            let connector = TimelineLineLayout {
                kind: "task-events".to_string(),
                x1: master_x + (task_node.width / 2.0),
                y1: master_y + max_task_height,
                x2: master_x + (task_node.width / 2.0),
                y2: master_y + max_task_height + 100.0 + max_event_line_length + 100.0,
            };
            all_lines_pre_title.push(connector.clone());

            let mut events: Vec<TimelineNodeLayout> = Vec::new();
            let mut event_y = master_y + EVENT_VERTICAL_OFFSET_FROM_TASK_Y;
            for ev in &task.events {
                let event_node = compute_node(
                    "event",
                    ev,
                    section_color,
                    master_x,
                    event_y,
                    task_content_width,
                    50.0,
                    font_size,
                    measurer,
                );
                event_y += event_node.height + EVENT_GAP_Y;
                all_nodes_pre_title.push(event_node.clone());
                events.push(event_node);
            }

            orphan_tasks.push(TimelineTaskLayout {
                node: task_node,
                connector,
                events,
            });

            master_x += TASK_STEP_X;
            if !disable_multicolor {
                section_color += 1;
            }
        }
    }

    let (pre_min_x, pre_min_y, pre_max_x, pre_max_y) =
        bounds_from_nodes_and_lines(&all_nodes_pre_title, &all_lines_pre_title)
            .unwrap_or((0.0, 0.0, 100.0, 100.0));
    let pre_title_box_width = (pre_max_x - pre_min_x).max(1.0);

    let title = model
        .title
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let title_x = pre_title_box_width / 2.0 - left_margin;

    let depth_y = if has_sections {
        max_section_height + max_task_height + 150.0
    } else {
        max_task_height + 100.0
    };

    let activity_line = TimelineLineLayout {
        kind: "activity".to_string(),
        x1: left_margin,
        y1: depth_y,
        x2: pre_title_box_width + 3.0 * left_margin,
        y2: depth_y,
    };

    let mut all_nodes_full: Vec<TimelineNodeLayout> = all_nodes_pre_title.clone();
    let mut all_lines_full: Vec<TimelineLineLayout> = all_lines_pre_title.clone();
    all_lines_full.push(activity_line.clone());

    if let Some(t) = title.as_deref() {
        // Approximate the title bounds so the viewBox can include it (Mermaid uses a bold 4ex).
        let title_style = TextStyle {
            font_family: None,
            font_size: 32.0,
            font_weight: Some("bold".to_string()),
        };
        let metrics = measurer.measure(t, &title_style);
        all_nodes_full.push(TimelineNodeLayout {
            x: title_x,
            y: 0.0_f64.max(TITLE_Y - title_style.font_size),
            width: metrics.width.max(1.0),
            height: title_style.font_size.max(1.0),
            content_width: metrics.width.max(1.0),
            padding: 0.0,
            section_class: "section-root".to_string(),
            label: t.to_string(),
            label_lines: vec![t.to_string()],
            kind: "title-bounds".to_string(),
        });
    }

    let (full_min_x, full_min_y, full_max_x, full_max_y) =
        bounds_from_nodes_and_lines(&all_nodes_full, &all_lines_full)
            .unwrap_or((pre_min_x, pre_min_y, pre_max_x, pre_max_y));

    let viewbox_padding =
        cfg_f64(effective_config, &["timeline", "padding"]).unwrap_or(DEFAULT_VIEWBOX_PADDING);
    let vb_min_x = full_min_x - viewbox_padding;
    let vb_min_y = full_min_y - viewbox_padding;
    let vb_max_x = full_max_x + viewbox_padding;
    let vb_max_y = full_max_y + viewbox_padding;

    Ok(TimelineDiagramLayout {
        bounds: Some(Bounds {
            min_x: vb_min_x,
            min_y: vb_min_y,
            max_x: vb_max_x,
            max_y: vb_max_y,
        }),
        left_margin,
        base_x,
        base_y,
        pre_title_box_width,
        sections,
        orphan_tasks,
        activity_line,
        title,
        title_x,
        title_y: TITLE_Y,
    })
}
