use crate::Result;
use crate::options::AsciiRenderOptions;
use crate::safe_text::BudgetedTextDocument;
use merman_core::diagrams::gantt::{
    GanttDiagramRenderModel, GanttRenderTask, GanttRenderTaskStart,
};
use std::collections::{HashMap, HashSet};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_gantt_diagram(
    model: &GanttDiagramRenderModel,
    options: &AsciiRenderOptions,
    local_time_zone: &merman_core::time::LocalTimeZone,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);
    let mut task_ids = HashSet::new();
    let mut task_sections = HashSet::new();
    let mut task_section_order = Vec::new();
    let mut tasks_by_section: HashMap<&str, Vec<&GanttRenderTask>> = HashMap::new();
    task_ids
        .try_reserve(model.tasks.len())
        .map_err(|_| layout_allocation_error())?;
    task_sections
        .try_reserve(model.tasks.len())
        .map_err(|_| layout_allocation_error())?;
    task_section_order
        .try_reserve(model.tasks.len())
        .map_err(|_| layout_allocation_error())?;
    tasks_by_section
        .try_reserve(model.tasks.len())
        .map_err(|_| layout_allocation_error())?;
    for task in &model.tasks {
        document.resources_mut().charge_layout_work(1)?;
        document.preflight_text_work(&task.id)?;
        if task.id.is_empty() {
            return Err(crate::error::AsciiError::UnsupportedFeature {
                diagram_type: "gantt",
                feature: "empty task ids",
            });
        }
        if !task_ids.insert(task.id.as_str()) {
            return Err(crate::error::AsciiError::UnsupportedFeature {
                diagram_type: "gantt",
                feature: "duplicate task ids",
            });
        }
        if task_sections.insert(task.section.as_str()) {
            task_section_order.push(task.section.as_str());
        }
        let tasks = tasks_by_section.entry(task.section.as_str()).or_default();
        tasks
            .try_reserve(1)
            .map_err(|_| layout_allocation_error())?;
        tasks.push(task);
    }

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

    if let Some(tick_interval) = model.tick_interval.as_deref() {
        document.push_line_with(|line| {
            line.push_str("tickInterval: ")?;
            line.push_str(tick_interval)
        })?;
    }
    if !model.today_marker.is_empty() {
        document.push_line_with(|line| {
            line.push_str("todayMarker: ")?;
            line.push_str(&model.today_marker)
        })?;
    }
    push_optional_list(&mut document, "includes", &model.includes)?;
    push_optional_list(&mut document, "excludes", &model.excludes)?;
    if model.inclusive_end_dates {
        document.push_line("inclusiveEndDates: true")?;
    }
    if !model.display_mode.is_empty() {
        document.push_line_with(|line| {
            line.push_str("displayMode: ")?;
            line.push_str(&model.display_mode)
        })?;
    }
    if model.top_axis {
        document.push_line("topAxis: true")?;
    }
    if !model.weekday.is_empty() && model.weekday != "sunday" {
        document.push_line_with(|line| {
            line.push_str("weekday: ")?;
            line.push_str(&model.weekday)
        })?;
    }
    if !model.weekend.is_empty() && model.weekend != "saturday" {
        document.push_line_with(|line| {
            line.push_str("weekend: ")?;
            line.push_str(&model.weekend)
        })?;
    }

    if model.sections.is_empty() {
        let mut current_section: Option<&str> = None;
        for task in &model.tasks {
            if current_section != Some(task.section.as_str()) {
                current_section = Some(task.section.as_str());
                push_section(&mut document, &task.section)?;
            }
            push_task(&mut document, task, local_time_zone)?;
        }
    } else {
        let mut emitted_sections = HashSet::new();
        emitted_sections
            .try_reserve(model.sections.len())
            .map_err(|_| layout_allocation_error())?;

        for section in &model.sections {
            let first_occurrence = emitted_sections.insert(section.as_str());
            push_section(&mut document, section)?;
            if !first_occurrence {
                continue;
            }
            if let Some(tasks) = tasks_by_section.get(section.as_str()) {
                for task in tasks {
                    push_task(&mut document, task, local_time_zone)?;
                }
            }
        }

        // Direct typed models may contain a task whose section is absent from the declaration
        // list. Preserve it under its authored section instead of dropping the task.
        for section in task_section_order {
            if emitted_sections.insert(section) {
                push_section(&mut document, section)?;
                if let Some(tasks) = tasks_by_section.get(section) {
                    for task in tasks {
                        push_task(&mut document, task, local_time_zone)?;
                    }
                }
            }
        }
    }

    document.finish(options)
}

fn push_section(document: &mut BudgetedTextDocument, section: &str) -> Result<()> {
    document.push_line_with(|line| {
        line.push_str("section: ")?;
        line.push_str(section)
    })
}

fn push_optional_list(
    document: &mut BudgetedTextDocument,
    name: &str,
    values: &[String],
) -> Result<()> {
    if values.is_empty() {
        return Ok(());
    }
    document.push_line_with(|line| {
        line.push_str(name)?;
        line.push_str(": ")?;
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                line.push_str(", ")?;
            }
            line.push_str(value)?;
        }
        Ok(())
    })
}

fn push_task(
    document: &mut BudgetedTextDocument,
    task: &GanttRenderTask,
    local_time_zone: &merman_core::time::LocalTimeZone,
) -> Result<()> {
    document.resources_mut().charge_layout_work(1)?;
    let start = format_date(task.start_ms, local_time_zone);
    let end = format_date(task.end_ms, local_time_zone);
    let render_end = task
        .render_end_ms
        .filter(|render_end| *render_end != task.end_ms)
        .map(|render_end| format_date(render_end, local_time_zone));
    document.push_wrapped_prefixed_line_with("  - ", "    ", SUMMARY_WRAP_WIDTH, |line| {
        line.push_str(&task.task)?;
        line.push_str(" [id=")?;
        line.push_str(&task.id)?;
        line.push_str(", range=")?;
        line.push_str(&start)?;
        line.push_str(" -> ")?;
        line.push_str(&end)?;
        if let Some(render_end) = render_end.as_deref() {
            line.push_str(", renderEnd=")?;
            line.push_str(render_end)?;
        }

        push_start_constraint(line, task)?;
        push_end_constraint(line, task)?;

        if !task.classes.is_empty() {
            line.push_str(", classes=")?;
            for (index, class) in task.classes.iter().enumerate() {
                if index > 0 {
                    line.push_str("|")?;
                }
                line.push_str(class)?;
            }
        }
        if task.manual_end_time {
            line.push_str(", manualEnd=true")?;
        }
        if task.task_type != task.section && !task.task_type.is_empty() {
            line.push_str(", type=")?;
            line.push_str(&task.task_type)?;
        }

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
            line.push_str(if emitted_flag { ", " } else { ", flags=" })?;
            line.push_str(flag)?;
            emitted_flag = true;
        }
        line.push_str("]")?;
        Ok(())
    })
}

fn push_start_constraint(
    line: &mut crate::safe_text::BudgetedWrappedText<'_, '_>,
    task: &GanttRenderTask,
) -> Result<()> {
    match &task.raw.start_time {
        GanttRenderTaskStart::PrevTaskEnd { id } => {
            if let Some(id) = id.as_deref().or(task.prev_task_id.as_deref()) {
                line.push_str(", after=")?;
                line.push_str(id)?;
            }
        }
        GanttRenderTaskStart::GetStartDate { start_data } => {
            let (key, value) = start_data
                .strip_prefix("after ")
                .map(|value| ("after", value))
                .unwrap_or(("start", start_data.as_str()));
            if !value.is_empty() {
                line.push_str(", ")?;
                line.push_str(key)?;
                line.push_str("=")?;
                line.push_str(value)?;
            }
        }
    }
    Ok(())
}

fn push_end_constraint(
    line: &mut crate::safe_text::BudgetedWrappedText<'_, '_>,
    task: &GanttRenderTask,
) -> Result<()> {
    if task.raw.end_time.data.is_empty() {
        return Ok(());
    }
    let (key, value) = task
        .raw
        .end_time
        .data
        .strip_prefix("until ")
        .map(|value| ("until", value))
        .unwrap_or(("duration", task.raw.end_time.data.as_str()));
    line.push_str(", ")?;
    line.push_str(key)?;
    line.push_str("=")?;
    line.push_str(value)
}

fn format_date(ms: i64, local_time_zone: &merman_core::time::LocalTimeZone) -> String {
    local_time_zone
        .at_instant(ms)
        .map(|local| {
            let datetime = local.local_datetime();
            if datetime.hour() == 0
                && datetime.minute() == 0
                && datetime.second() == 0
                && datetime.millisecond() == 0
            {
                local.date().to_string()
            } else {
                datetime.to_string()
            }
        })
        .unwrap_or_else(|| ms.to_string())
}

fn layout_allocation_error() -> crate::error::AsciiError {
    crate::error::AsciiError::AllocationFailed {
        phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}
