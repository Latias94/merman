use crate::Result;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::safe_text::{BudgetedTextDocument, charge_text_layout};
use merman_core::diagrams::gantt::{
    GanttDiagramRenderModel, GanttRenderTask, GanttRenderTaskStart,
};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_gantt_diagram(
    model: &GanttDiagramRenderModel,
    options: &AsciiRenderOptions,
    local_time_zone: &merman_core::time::LocalTimeZone,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);
    let task_index = admit_then_materialize_gantt_structure(
        model,
        document.resources_mut(),
        |admission, resources| GanttTaskIndex::materialize(model, admission, resources),
    )?;

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

    if let Some(task_index) = task_index {
        let GanttTaskIndex {
            task_section_order,
            tasks_by_section,
            emitted_section_capacity,
        } = task_index;
        let mut emitted_sections = HashSet::new();
        emitted_sections
            .try_reserve(emitted_section_capacity)
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
    } else {
        let mut current_section: Option<&str> = None;
        for task in &model.tasks {
            if current_section != Some(task.section.as_str()) {
                current_section = Some(task.section.as_str());
                push_section(&mut document, &task.section)?;
            }
            push_task(&mut document, task, local_time_zone)?;
        }
    }

    document.finish(options)
}

#[derive(Debug, Clone, Copy)]
struct GanttStructureAdmission {
    task_capacity: usize,
    grouped: bool,
}

impl GanttStructureAdmission {
    fn preflight(model: &GanttDiagramRenderModel, resources: &mut ResourceContext) -> Result<Self> {
        let task_capacity = model.tasks.len();
        let grouped = !model.sections.is_empty();

        // The grouped path retains one slot per task in the id set, section-order vector,
        // section map, and section task vectors. Its emitted-section set additionally admits the
        // authored sections plus the worst case where every task names a distinct orphan section.
        let allocation_work = if grouped {
            let grouped_task_slots = resources.checked_work_mul(task_capacity, 3)?;
            let emitted_section_slots =
                resources.checked_work_add(model.sections.len(), task_capacity)?;
            let indexed_task_slots =
                resources.checked_work_add(task_capacity, grouped_task_slots)?;
            resources.checked_work_add(indexed_task_slots, emitted_section_slots)?
        } else {
            task_capacity
        };
        resources.charge_layout_work(allocation_work)?;

        // Hashing borrowed identifiers is allocation-free, but preflight their terminal-safety
        // work before any input-sized container is allocated.
        for section in &model.sections {
            charge_text_layout(resources, section)?;
        }
        for task in &model.tasks {
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, &task.id)?;
            charge_text_layout(resources, &task.section)?;
            if task.id.is_empty() {
                return Err(crate::error::AsciiError::UnsupportedFeature {
                    diagram_type: "gantt",
                    feature: "empty task ids",
                });
            }
        }

        Ok(Self {
            task_capacity,
            grouped,
        })
    }
}

struct GanttTaskIndex<'model> {
    task_section_order: Vec<&'model str>,
    tasks_by_section: HashMap<&'model str, Vec<&'model GanttRenderTask>>,
    emitted_section_capacity: usize,
}

impl<'model> GanttTaskIndex<'model> {
    fn materialize(
        model: &'model GanttDiagramRenderModel,
        admission: GanttStructureAdmission,
        resources: &mut ResourceContext,
    ) -> Result<Option<Self>> {
        let mut task_ids = HashSet::new();
        task_ids
            .try_reserve(admission.task_capacity)
            .map_err(|_| layout_allocation_error())?;

        let mut grouped_index = if admission.grouped {
            let mut task_section_order = Vec::new();
            let mut tasks_by_section = HashMap::new();
            task_section_order
                .try_reserve(admission.task_capacity)
                .map_err(|_| layout_allocation_error())?;
            tasks_by_section
                .try_reserve(admission.task_capacity)
                .map_err(|_| layout_allocation_error())?;
            Some(Self {
                task_section_order,
                tasks_by_section,
                emitted_section_capacity: 0,
            })
        } else {
            None
        };

        for task in &model.tasks {
            if !task_ids.insert(task.id.as_str()) {
                return Err(crate::error::AsciiError::UnsupportedFeature {
                    diagram_type: "gantt",
                    feature: "duplicate task ids",
                });
            }
            let Some(index) = grouped_index.as_mut() else {
                continue;
            };
            let tasks = match index.tasks_by_section.entry(task.section.as_str()) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    index.task_section_order.push(task.section.as_str());
                    entry.insert(Vec::new())
                }
            };
            tasks
                .try_reserve(1)
                .map_err(|_| layout_allocation_error())?;
            tasks.push(task);
        }

        if let Some(index) = grouped_index.as_mut() {
            index.emitted_section_capacity =
                resources.checked_work_add(model.sections.len(), index.task_section_order.len())?;
        }
        Ok(grouped_index)
    }
}

fn admit_then_materialize_gantt_structure<T>(
    model: &GanttDiagramRenderModel,
    resources: &mut ResourceContext,
    materialize: impl FnOnce(GanttStructureAdmission, &mut ResourceContext) -> Result<T>,
) -> Result<T> {
    let admission = GanttStructureAdmission::preflight(model, resources)?;
    materialize(admission, resources)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AsciiError;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use std::cell::Cell;

    fn direct_model() -> GanttDiagramRenderModel {
        let mut model = GanttDiagramRenderModel::default();
        model.sections = vec!["Build".to_string(), "Ship".to_string()];
        model.tasks = vec![
            GanttRenderTask {
                id: "a".to_string(),
                section: "Build".to_string(),
                ..GanttRenderTask::default()
            },
            GanttRenderTask {
                id: "b".to_string(),
                section: "Orphan".to_string(),
                ..GanttRenderTask::default()
            },
        ];
        model
    }

    fn admission_work(model: &GanttDiagramRenderModel) -> usize {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut resources = ResourceContext::new(policy);
        admit_then_materialize_gantt_structure(model, &mut resources, |_, _| Ok(()))
            .expect("unbounded Gantt admission should succeed");
        resources.layout_work_used()
    }

    #[test]
    fn gantt_admission_accepts_exact_work_before_materializing_index() {
        let model = direct_model();
        let exact_work = admission_work(&model);
        assert_eq!(exact_work, 42);
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact Gantt admission work limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let materialized = Cell::new(false);
        let index = admit_then_materialize_gantt_structure(
            &model,
            &mut resources,
            |admission, resources| {
                materialized.set(true);
                GanttTaskIndex::materialize(&model, admission, resources)
            },
        )
        .expect("exact Gantt admission work should permit materialization");

        assert!(materialized.get());
        let index = index.expect("declared sections require a grouped task index");
        assert_eq!(index.tasks_by_section.len(), 2);
        assert_eq!(resources.layout_work_used(), exact_work);
    }

    #[test]
    fn gantt_admission_rejects_max_minus_one_before_materializing_index() {
        let model = direct_model();
        let exact_work = admission_work(&model);
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one Gantt admission work limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let materialized = Cell::new(false);
        let error = admit_then_materialize_gantt_structure(
            &model,
            &mut resources,
            |admission, resources| {
                materialized.set(true);
                GanttTaskIndex::materialize(&model, admission, resources).map(|_| ())
            },
        )
        .expect_err("max-minus-one Gantt admission work should fail before materialization");

        assert!(!materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == exact_work
                    && details.max == exact_work - 1
        ));
    }
}
