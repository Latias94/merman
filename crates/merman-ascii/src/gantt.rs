use crate::Result;
use crate::options::AsciiRenderOptions;
use crate::safe_text::BudgetedTextDocument;
use merman_core::diagrams::gantt::GanttDiagramRenderModel;

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_gantt_diagram(
    model: &GanttDiagramRenderModel,
    options: &AsciiRenderOptions,
    local_time_zone: &merman_core::time::LocalTimeZone,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);

    document.push_optional_line(model.title.as_deref())?;
    document.push_optional_prefixed_line("accTitle: ", model.acc_title.as_deref())?;
    document.push_optional_prefixed_line("accDescr: ", model.acc_descr.as_deref())?;
    if !model.date_format.is_empty() {
        document.push_line_with(|line| {
            line.push_str("dateFormat: ")?;
            line.push_str(&model.date_format)
        })?;
    }
    if !model.axis_format.is_empty() {
        document.push_line_with(|line| {
            line.push_str("axisFormat: ")?;
            line.push_str(&model.axis_format)
        })?;
    }

    let mut current_section: Option<&str> = None;
    for task in &model.tasks {
        if current_section != Some(task.section.as_str()) {
            current_section = Some(task.section.as_str());
            document.push_line_with(|line| {
                line.push_str("section: ")?;
                line.push_str(&task.section)
            })?;
        }
        document.resources_mut().charge_layout_work(1)?;
        let start = format_date(task.start_ms, local_time_zone);
        let end = format_date(task.render_end_ms.unwrap_or(task.end_ms), local_time_zone);
        document.push_wrapped_prefixed_line_with("  - ", "    ", SUMMARY_WRAP_WIDTH, |line| {
            line.push_str(&task.task)?;
            line.push_str(" [")?;
            line.push_str(&start)?;
            line.push_str(" -> ")?;
            line.push_str(&end)?;
            line.push_str("]")?;

            let flags = [
                (task.milestone, "milestone"),
                (task.active, "active"),
                (task.done, "done"),
                (task.crit, "crit"),
                (task.vert, "vert"),
            ];
            let mut emitted_flag = false;
            for flag in flags
                .into_iter()
                .filter_map(|(enabled, flag)| enabled.then_some(flag))
            {
                line.push_str(if emitted_flag { ", " } else { " [" })?;
                line.push_str(flag)?;
                emitted_flag = true;
            }
            if emitted_flag {
                line.push_str("]")?;
            }
            Ok(())
        })?;
    }

    document.finish(options)
}

fn format_date(ms: i64, local_time_zone: &merman_core::time::LocalTimeZone) -> String {
    local_time_zone
        .at_instant(ms)
        .map(|local| local.date().to_string())
        .unwrap_or_else(|| ms.to_string())
}
