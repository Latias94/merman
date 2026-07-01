use crate::options::AsciiRenderOptions;
use crate::text::{normalize_optional_text, push_wrapped_prefixed_line, trim_trailing_blank_lines};
use merman_core::diagrams::gantt::{GanttDiagramRenderModel, GanttRenderTask};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_gantt_diagram(
    model: &GanttDiagramRenderModel,
    _options: &AsciiRenderOptions,
) -> String {
    let mut lines = Vec::new();

    if let Some(title) = normalize_optional_text(model.title.as_deref()) {
        lines.push(title);
    }
    if let Some(acc_title) = normalize_optional_text(model.acc_title.as_deref()) {
        lines.push(format!("accTitle: {acc_title}"));
    }
    if let Some(acc_descr) = normalize_optional_text(model.acc_descr.as_deref()) {
        lines.push(format!("accDescr: {acc_descr}"));
    }
    if !model.date_format.is_empty() {
        lines.push(format!("dateFormat: {}", model.date_format));
    }
    if !model.axis_format.is_empty() {
        lines.push(format!("axisFormat: {}", model.axis_format));
    }

    let mut current_section: Option<&str> = None;
    for task in &model.tasks {
        if current_section != Some(task.section.as_str()) {
            current_section = Some(task.section.as_str());
            lines.push(format!("section: {}", task.section));
        }
        push_wrapped_prefixed_line(
            &mut lines,
            "  - ",
            "    ",
            &render_task_text(task),
            SUMMARY_WRAP_WIDTH,
        );
    }

    trim_trailing_blank_lines(lines).join("\n")
}

fn render_task_text(task: &GanttRenderTask) -> String {
    let start = format_date(task.start_ms);
    let end = format_date(task.render_end_ms.unwrap_or(task.end_ms));
    let mut flags = Vec::new();
    if task.milestone {
        flags.push("milestone");
    }
    if task.active {
        flags.push("active");
    }
    if task.done {
        flags.push("done");
    }
    if task.crit {
        flags.push("crit");
    }
    if task.vert {
        flags.push("vert");
    }
    let suffix = if flags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", flags.join(", "))
    };
    format!("{} [{} -> {}]{}", task.task, start, end, suffix)
}

fn format_date(ms: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)
        .map(|dt| {
            let local = merman_core::time::datetime_to_local_fixed(
                dt.with_timezone(&merman_core::time::utc_fixed_offset()),
            );
            local.date_naive().format("%Y-%m-%d").to_string()
        })
        .unwrap_or_else(|| ms.to_string())
}
