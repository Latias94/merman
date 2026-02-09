use crate::Result;
use crate::model::{
    Bounds, JourneyActorLegendItemLayout, JourneyActorLegendLineLayout, JourneyDiagramLayout,
    JourneyLineLayout, JourneyMouthKind, JourneySectionLayout, JourneyTaskActorCircleLayout,
    JourneyTaskLayout,
};
use crate::text::{TextMeasurer, TextStyle};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const LEGEND_CIRCLE_CX: f64 = 20.0;
const LEGEND_CIRCLE_R: f64 = 7.0;
const LEGEND_LABEL_X: f64 = 40.0;
const LEGEND_FIRST_Y: f64 = 60.0;
const LEGEND_LINE_STEP_Y: f64 = 20.0;

const SECTION_Y: f64 = 50.0;
const TITLE_Y: f64 = 25.0;
const VIEWBOX_TOP_PAD: f64 = 25.0;

#[allow(dead_code)]
const FACE_RADIUS: f64 = 15.0;
const FACE_BASE_Y: f64 = 300.0;
const FACE_SCORE_STEP_Y: f64 = 30.0;

#[derive(Debug, Clone, Deserialize)]
struct JourneyTaskModel {
    section: String,
    #[serde(rename = "type")]
    #[allow(dead_code)]
    task_type: String,
    task: String,
    score: i64,
    #[serde(default)]
    people: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct JourneyModel {
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
    #[serde(default)]
    actors: Vec<String>,
    #[serde(default)]
    sections: Vec<String>,
    #[serde(default)]
    tasks: Vec<JourneyTaskModel>,
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

fn cfg_str(cfg: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_str().map(|s| s.to_string())
}

fn cfg_string_vec(cfg: &serde_json::Value, path: &[&str]) -> Vec<String> {
    let mut cur = cfg;
    for k in path {
        let Some(next) = cur.get(*k) else {
            return Vec::new();
        };
        cur = next;
    }
    cur.as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn actors_from_tasks(tasks: &[JourneyTaskModel]) -> Vec<String> {
    let mut set = BTreeSet::<String>::new();
    for t in tasks {
        for p in &t.people {
            set.insert(p.to_string());
        }
    }
    set.into_iter().collect()
}

fn wrap_actor_label_lines(
    person: &str,
    max_label_width: f64,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> Vec<String> {
    let full_text_width = measurer.measure(person, style).width;
    if full_text_width <= max_label_width.max(1.0) {
        return vec![person.to_string()];
    }

    let words = person.split(' ').collect::<Vec<_>>();
    let mut lines = Vec::new();
    let mut current_line = String::new();

    for word in words {
        let test_line = if current_line.is_empty() {
            word.to_string()
        } else {
            format!("{current_line} {word}")
        };

        let test_width = measurer.measure(&test_line, style).width;
        if test_width > max_label_width.max(1.0) {
            if !current_line.is_empty() {
                lines.push(std::mem::take(&mut current_line));
            }
            current_line = word.to_string();

            let word_width = measurer.measure(word, style).width;
            if word_width > max_label_width.max(1.0) {
                let mut broken_word = String::new();
                for ch in word.chars() {
                    broken_word.push(ch);
                    let candidate = format!("{broken_word}-");
                    let candidate_width = measurer.measure(&candidate, style).width;
                    if candidate_width > max_label_width.max(1.0) {
                        let mut head = broken_word.clone();
                        head.pop();
                        if !head.is_empty() {
                            lines.push(format!("{head}-"));
                        }
                        broken_word = ch.to_string();
                    }
                }
                current_line = broken_word;
            }
        } else {
            current_line = test_line;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        vec![person.to_string()]
    } else {
        lines
    }
}

pub fn layout_journey_diagram(
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
) -> Result<JourneyDiagramLayout> {
    let model: JourneyModel = serde_json::from_value(semantic.clone())?;
    let _ = (
        model.acc_title.as_deref(),
        model.acc_descr.as_deref(),
        model.sections.as_slice(),
        model.diagram_type.as_str(),
    );

    let left_margin_base = cfg_f64(effective_config, &["journey", "leftMargin"])
        .unwrap_or(150.0)
        .max(0.0);
    let max_label_width = cfg_f64(effective_config, &["journey", "maxLabelWidth"])
        .unwrap_or(360.0)
        .max(1.0);
    let box_text_margin = cfg_f64(effective_config, &["journey", "boxTextMargin"])
        .unwrap_or(5.0)
        .max(0.0);

    let diagram_margin_x = cfg_f64(effective_config, &["journey", "diagramMarginX"])
        .unwrap_or(50.0)
        .max(0.0);
    let diagram_margin_y = cfg_f64(effective_config, &["journey", "diagramMarginY"])
        .unwrap_or(10.0)
        .max(0.0);
    let task_margin = cfg_f64(effective_config, &["journey", "taskMargin"])
        .unwrap_or(50.0)
        .max(0.0);
    let cell_w = cfg_f64(effective_config, &["journey", "width"])
        .unwrap_or(150.0)
        .max(1.0);
    let cell_h = cfg_f64(effective_config, &["journey", "height"])
        .unwrap_or(50.0)
        .max(1.0);

    let actor_colours = cfg_string_vec(effective_config, &["journey", "actorColours"]);
    let section_fills = cfg_string_vec(effective_config, &["journey", "sectionFills"]);

    let actors = if model.actors.is_empty() {
        actors_from_tasks(&model.tasks)
    } else {
        model.actors.clone()
    };

    let mut actor_map: BTreeMap<String, (i64, String)> = BTreeMap::new();
    for (i, actor) in actors.iter().enumerate() {
        let pos = i as i64;
        let color = actor_colours
            .get(i % actor_colours.len().max(1))
            .cloned()
            .unwrap_or_else(|| "#8FBC8F".to_string());
        actor_map.insert(actor.clone(), (pos, color));
    }

    let legend_style = TextStyle::default();
    let mut max_actor_label_width: f64 = 0.0;
    let mut actor_legend: Vec<JourneyActorLegendItemLayout> = Vec::new();

    let mut y_pos = LEGEND_FIRST_Y;
    for actor in actors.iter() {
        let (pos, color) = actor_map
            .get(actor)
            .cloned()
            .unwrap_or((0_i64, "#8FBC8F".to_string()));

        let lines = wrap_actor_label_lines(actor, max_label_width, measurer, &legend_style);
        let mut label_lines: Vec<JourneyActorLegendLineLayout> = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            let x = LEGEND_LABEL_X;
            let y = y_pos + 7.0 + (index as f64) * LEGEND_LINE_STEP_Y;
            let tspan_x = x + box_text_margin * 2.0;
            label_lines.push(JourneyActorLegendLineLayout {
                text: line.to_string(),
                x,
                y,
                tspan_x,
                text_margin: box_text_margin,
            });

            let line_width = measurer.measure(line, &legend_style).width;
            if line_width > max_actor_label_width && line_width > left_margin_base - line_width {
                max_actor_label_width = line_width;
            }
        }

        actor_legend.push(JourneyActorLegendItemLayout {
            actor: actor.to_string(),
            pos,
            color,
            circle_cx: LEGEND_CIRCLE_CX,
            circle_cy: y_pos,
            circle_r: LEGEND_CIRCLE_R,
            label_lines,
        });

        y_pos += LEGEND_LINE_STEP_Y.max(lines.len() as f64 * LEGEND_LINE_STEP_Y);
    }

    let left_margin = left_margin_base + max_actor_label_width;
    let section_v_height = cell_h * 2.0 + diagram_margin_y;
    let task_y = section_v_height;

    let mut sections: Vec<JourneySectionLayout> = Vec::new();
    let mut tasks: Vec<JourneyTaskLayout> = Vec::new();

    let mut last_section = String::new();
    let mut section_number: i64 = 0;
    let mut current_fill = "#CCC".to_string();
    let mut current_num: i64 = 0;

    let mut stopx = left_margin;
    for (i, task) in model.tasks.iter().enumerate() {
        let x = (i as f64) * task_margin + (i as f64) * cell_w + left_margin;
        let is_new_section = last_section != task.section;

        if is_new_section {
            let fills_len = section_fills.len().max(1) as i64;
            current_num = section_number % fills_len;
            current_fill = section_fills
                .get(current_num as usize)
                .cloned()
                .unwrap_or_else(|| "#CCC".to_string());

            let mut count: i64 = 0;
            for t in model.tasks.iter().skip(i) {
                if t.section == task.section {
                    count += 1;
                } else {
                    break;
                }
            }
            let section_width = cell_w * (count as f64) + diagram_margin_x * ((count - 1) as f64);

            sections.push(JourneySectionLayout {
                section: task.section.to_string(),
                num: current_num,
                x,
                y: SECTION_Y,
                width: section_width.max(1.0),
                height: cell_h,
                fill: current_fill.clone(),
                task_count: count,
            });

            last_section = task.section.to_string();
            section_number += 1;
        }

        let center_x = x + cell_w / 2.0;
        let max_height = FACE_BASE_Y + 5.0 * FACE_SCORE_STEP_Y;
        let face_cy = FACE_BASE_Y + (5_i64.saturating_sub(task.score) as f64) * FACE_SCORE_STEP_Y;
        let mouth = if task.score > 3 {
            JourneyMouthKind::Smile
        } else if task.score < 3 {
            JourneyMouthKind::Sad
        } else {
            JourneyMouthKind::Ambivalent
        };

        let mut actor_circles = Vec::new();
        let mut cx = x + 14.0;
        for p in &task.people {
            let Some((pos, color)) = actor_map.get(p).cloned() else {
                continue;
            };
            actor_circles.push(JourneyTaskActorCircleLayout {
                actor: p.to_string(),
                pos,
                color,
                cx,
                cy: task_y,
                r: LEGEND_CIRCLE_R,
            });
            cx += 10.0;
        }

        let line_id = format!("task{i}");
        tasks.push(JourneyTaskLayout {
            index: i as i64,
            section: task.section.to_string(),
            task: task.task.to_string(),
            score: task.score,
            x,
            y: task_y,
            width: cell_w,
            height: cell_h,
            fill: current_fill.clone(),
            num: current_num,
            people: task.people.clone(),
            actor_circles,
            line_id,
            line_x1: center_x,
            line_y1: task_y,
            line_x2: center_x,
            line_y2: max_height,
            face_cx: center_x,
            face_cy,
            mouth,
        });

        stopx = stopx.max(x + diagram_margin_x + task_margin);
    }

    let stopy = (actors.len() as f64 * 50.0).max(if tasks.is_empty() {
        0.0
    } else {
        FACE_BASE_Y + 5.0 * FACE_SCORE_STEP_Y
    });

    let height = (stopy - 0.0 + 2.0 * diagram_margin_y).max(1.0);
    let width = (left_margin + stopx + 2.0 * diagram_margin_x).max(1.0);

    let title = model
        .title
        .as_deref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let extra_vert_for_title = if title.is_some() { 70.0 } else { 0.0 };

    let bounds = Bounds {
        min_x: 0.0,
        min_y: -VIEWBOX_TOP_PAD,
        max_x: width,
        max_y: -VIEWBOX_TOP_PAD + height + extra_vert_for_title,
    };

    let svg_height = height + extra_vert_for_title + VIEWBOX_TOP_PAD;

    let activity_line = JourneyLineLayout {
        x1: left_margin,
        y1: cell_h * 4.0,
        x2: width - left_margin - 4.0,
        y2: cell_h * 4.0,
    };

    let _ = (
        cfg_str(effective_config, &["journey", "taskFontFamily"]),
        cfg_f64(effective_config, &["journey", "taskFontSize"]),
        cfg_str(effective_config, &["journey", "textPlacement"]),
    );

    Ok(JourneyDiagramLayout {
        bounds: Some(bounds),
        left_margin,
        max_actor_label_width,
        width,
        height,
        svg_height,
        title,
        title_x: left_margin,
        title_y: TITLE_Y,
        actor_legend,
        sections,
        tasks,
        activity_line,
    })
}
