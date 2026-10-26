use crate::Result;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::safe_text::{
    BudgetedTextDocument, charge_text_layout, push_document_field, push_document_list,
    push_optional_document_field, push_wrapped_field, push_wrapped_list,
};
use merman_core::diagrams::gantt::{
    GanttTask, GanttTaskEndConstraint, GanttTaskStartConstraint, GanttTerminalTaskView,
    GanttTerminalView,
};
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub(super) fn render_gantt_diagram(
    model: GanttTerminalView<'_>,
    options: &AsciiRenderOptions,
    local_time_zone: &merman_core::time::LocalTimeZone,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let metadata = model.metadata();
    let layout_resources = execution.new_resource_context(merman_core::OperationPhase::Layout);
    let mut document = BudgetedTextDocument::from_resources(layout_resources, options);
    let task_index = admit_then_materialize_gantt_structure(
        model,
        document.resources_mut(),
        |admission, resources| GanttTaskIndex::materialize(model, admission, resources),
    )?;
    execution.rebind_resource_context(document.resources_mut(), merman_core::OperationPhase::Emit);

    push_optional_document_field(&mut document, "title", metadata.title.as_deref())?;
    push_optional_document_field(&mut document, "accTitle", metadata.acc_title.as_deref())?;
    push_optional_document_field(&mut document, "accDescr", metadata.acc_descr.as_deref())?;
    if !metadata.date_format.is_empty() {
        push_document_field(&mut document, "dateFormat", metadata.date_format)?;
    }
    if !metadata.axis_format.is_empty() {
        push_document_field(&mut document, "axisFormat", metadata.axis_format)?;
    }

    if let Some(tick_interval) = metadata.tick_interval.as_deref() {
        push_document_field(&mut document, "tickInterval", tick_interval)?;
    }
    if !metadata.today_marker.is_empty() {
        push_document_field(&mut document, "todayMarker", metadata.today_marker)?;
    }
    if !metadata.includes.is_empty() {
        push_document_list(
            &mut document,
            "includes",
            metadata.includes.iter().map(String::as_str),
        )?;
    }
    if !metadata.excludes.is_empty() {
        push_document_list(
            &mut document,
            "excludes",
            metadata.excludes.iter().map(String::as_str),
        )?;
    }
    if metadata.inclusive_end_dates {
        document.push_line("inclusiveEndDates: true")?;
    }
    if !metadata.display_mode.is_empty() {
        push_document_field(&mut document, "displayMode", metadata.display_mode)?;
    }
    if metadata.top_axis {
        document.push_line("topAxis: true")?;
    }
    if !metadata.weekday.is_empty() && metadata.weekday != "sunday" {
        push_document_field(&mut document, "weekday", metadata.weekday)?;
    }
    if !metadata.weekend.is_empty() && metadata.weekend != "saturday" {
        push_document_field(&mut document, "weekend", metadata.weekend)?;
    }

    if let Some(task_index) = task_index {
        let GanttTaskIndex {
            orphan_section_order,
            tasks_by_section,
        } = task_index;
        for (section_index, section) in metadata.sections.iter().enumerate() {
            document.resources_mut().checkpoint()?;
            push_section(&mut document, section)?;
            if let Some(tasks) = tasks_by_section.get(&GanttSectionKey::Declared(section_index)) {
                for task in tasks {
                    push_task(
                        &mut document,
                        indexed_task(model, *task)?,
                        local_time_zone,
                        execution,
                    )?;
                }
            }
        }

        // Direct typed models may contain a task whose section is absent from the declaration
        // list. Preserve it under its authored section instead of dropping the task.
        for section in orphan_section_order {
            document.resources_mut().checkpoint()?;
            push_section(&mut document, section)?;
            if let Some(tasks) = tasks_by_section.get(&GanttSectionKey::Orphan(section)) {
                for task in tasks {
                    push_task(
                        &mut document,
                        indexed_task(model, *task)?,
                        local_time_zone,
                        execution,
                    )?;
                }
            }
        }
    } else {
        let mut current_section: Option<&str> = None;
        for task in model.tasks() {
            let semantic = task.semantic;
            if current_section != Some(semantic.section.as_str()) {
                current_section = Some(semantic.section.as_str());
                push_section(&mut document, &semantic.section)?;
            }
            push_task(&mut document, task, local_time_zone, execution)?;
        }
    }

    document.finish_with_execution(execution)
}

#[derive(Debug, Clone, Copy)]
struct GanttStructureAdmission {
    task_capacity: usize,
    section_lookup_capacity: usize,
    grouped: bool,
}

impl GanttStructureAdmission {
    fn preflight(model: GanttTerminalView<'_>, resources: &ResourceContext) -> Result<Self> {
        let metadata = model.metadata();
        let task_capacity = model.tasks().len();
        let grouped = !metadata.sections.is_empty();

        // The declared-section lookup contains exactly one entry per authored occurrence. Orphan
        // sections are owned separately by the per-task grouped index.
        let section_lookup_capacity = if grouped { metadata.sections.len() } else { 0 };
        let allocation_work = if grouped {
            let grouped_task_slots = resources.checked_work_mul(task_capacity, 3)?;
            let indexed_task_slots =
                resources.checked_work_add(task_capacity, grouped_task_slots)?;
            resources.checked_work_add(indexed_task_slots, section_lookup_capacity)?
        } else {
            task_capacity
        };
        resources.charge_layout_work(allocation_work)?;

        // Hashing borrowed identifiers is allocation-free, but preflight their terminal-safety
        // work before any input-sized container is allocated.
        for section in metadata.sections {
            charge_text_layout(resources, section)?;
        }
        for task in model.tasks() {
            let task = task.semantic;
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, &task.id)?;
            charge_text_layout(resources, &task.section)?;
            if task.id.is_empty() {
                return Err(crate::error::AsciiError::UnsupportedFeature {
                    diagram_type: "gantt",
                    feature: "empty task ids",
                });
            }
            if let Some(section_index) = task.section_index
                && metadata.sections.get(section_index).map(String::as_str)
                    != Some(task.section.as_str())
            {
                return Err(invalid_task_section_occurrence());
            }
        }

        Ok(Self {
            task_capacity,
            section_lookup_capacity,
            grouped,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GanttSectionKey<'model> {
    Declared(usize),
    Orphan(&'model str),
}

#[derive(Debug, Clone, Copy)]
enum DeclaredSectionMatch {
    Unique(usize),
    Ambiguous,
}

struct GanttTaskIndex<'model> {
    orphan_section_order: Vec<&'model str>,
    tasks_by_section: HashMap<GanttSectionKey<'model>, Vec<usize>>,
}

impl<'model> GanttTaskIndex<'model> {
    fn materialize(
        model: GanttTerminalView<'model>,
        admission: GanttStructureAdmission,
        resources: &ResourceContext,
    ) -> Result<Option<Self>> {
        let metadata = model.metadata();
        resources.checkpoint()?;
        let mut task_ids = HashSet::new();
        task_ids
            .try_reserve(admission.task_capacity)
            .map_err(|_| layout_allocation_error())?;

        let mut grouped_index = if admission.grouped {
            resources.checkpoint()?;
            let mut orphan_section_order = Vec::new();
            let mut tasks_by_section = HashMap::new();
            orphan_section_order
                .try_reserve(admission.task_capacity)
                .map_err(|_| layout_allocation_error())?;
            tasks_by_section
                .try_reserve(admission.task_capacity)
                .map_err(|_| layout_allocation_error())?;
            Some(Self {
                orphan_section_order,
                tasks_by_section,
            })
        } else {
            None
        };

        let mut declared_sections = HashMap::new();
        if admission.grouped {
            resources.checkpoint()?;
            declared_sections
                .try_reserve(admission.section_lookup_capacity)
                .map_err(|_| layout_allocation_error())?;
            for (section_index, section) in metadata.sections.iter().enumerate() {
                resources.checkpoint()?;
                match declared_sections.entry(section.as_str()) {
                    Entry::Occupied(mut entry) => {
                        *entry.get_mut() = DeclaredSectionMatch::Ambiguous;
                    }
                    Entry::Vacant(entry) => {
                        entry.insert(DeclaredSectionMatch::Unique(section_index));
                    }
                }
            }
        }

        for (task_index, task) in model.tasks().enumerate() {
            let semantic = task.semantic;
            resources.checkpoint()?;
            if !task_ids.insert(semantic.id.as_str()) {
                return Err(crate::error::AsciiError::UnsupportedFeature {
                    diagram_type: "gantt",
                    feature: "duplicate task ids",
                });
            }
            let Some(index) = grouped_index.as_mut() else {
                if semantic.section_index.is_some() {
                    return Err(invalid_task_section_occurrence());
                }
                continue;
            };
            let section_key = match semantic.section_index {
                Some(section_index)
                    if metadata.sections.get(section_index).map(String::as_str)
                        == Some(semantic.section.as_str()) =>
                {
                    GanttSectionKey::Declared(section_index)
                }
                Some(_) => return Err(invalid_task_section_occurrence()),
                None => match declared_sections.get(semantic.section.as_str()) {
                    Some(DeclaredSectionMatch::Unique(section_index)) => {
                        GanttSectionKey::Declared(*section_index)
                    }
                    Some(DeclaredSectionMatch::Ambiguous) => {
                        return Err(ambiguous_task_section_occurrence());
                    }
                    None => GanttSectionKey::Orphan(semantic.section.as_str()),
                },
            };
            let tasks = match index.tasks_by_section.entry(section_key) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => {
                    if let GanttSectionKey::Orphan(section) = section_key {
                        index.orphan_section_order.push(section);
                    }
                    entry.insert(Vec::new())
                }
            };
            resources.checkpoint()?;
            tasks
                .try_reserve(1)
                .map_err(|_| layout_allocation_error())?;
            tasks.push(task_index);
        }

        resources.checkpoint()?;
        Ok(grouped_index)
    }
}

fn invalid_task_section_occurrence() -> crate::error::AsciiError {
    crate::error::AsciiError::UnsupportedFeature {
        diagram_type: "gantt",
        feature: "invalid task section occurrence",
    }
}

fn ambiguous_task_section_occurrence() -> crate::error::AsciiError {
    crate::error::AsciiError::UnsupportedFeature {
        diagram_type: "gantt",
        feature: "ambiguous task section occurrence",
    }
}

fn admit_then_materialize_gantt_structure<T>(
    model: GanttTerminalView<'_>,
    resources: &ResourceContext,
    materialize: impl FnOnce(GanttStructureAdmission, &ResourceContext) -> Result<T>,
) -> Result<T> {
    resources.transaction(|resources| {
        let admission = GanttStructureAdmission::preflight(model, resources)?;
        materialize(admission, resources)
    })
}

fn push_section(document: &mut BudgetedTextDocument, section: &str) -> Result<()> {
    push_document_field(document, "section", section)
}

fn indexed_task<'a>(
    model: GanttTerminalView<'a>,
    index: usize,
) -> Result<GanttTerminalTaskView<'a>> {
    model
        .task(index)
        .ok_or_else(invalid_task_section_occurrence)
}

fn push_task(
    document: &mut BudgetedTextDocument,
    task: GanttTerminalTaskView<'_>,
    local_time_zone: &merman_core::time::LocalTimeZone,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    execution.checkpoint(merman_core::OperationPhase::Emit)?;
    document.resources_mut().charge_layout_work(1)?;
    let render_end = task
        .render_end_ms
        .filter(|render_end| *render_end != task.semantic.end_ms)
        .map(|render_end| format_date(render_end, local_time_zone));
    let task = task.semantic;
    let start = format_date(task.start_ms, local_time_zone);
    let end = format_date(task.end_ms, local_time_zone);
    document.push_wrapped_prefixed_line_with("  - ", "    ", SUMMARY_WRAP_WIDTH, |line| {
        push_wrapped_field(line, "", "task", &task.task)?;
        push_wrapped_field(line, " [", "id", &task.id)?;
        line.write_fmt(format_args!(", order={}", task.order))?;
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
            push_wrapped_list(
                line,
                ", ",
                "classes",
                task.classes.iter().map(String::as_str),
            )?;
        }
        if task.manual_end_time {
            line.push_str(", manualEnd=true")?;
        }
        if task.task_type != task.section && !task.task_type.is_empty() {
            push_wrapped_field(line, ", ", "type", &task.task_type)?;
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
    line: &mut crate::safe_text::BudgetedWrappedText<'_>,
    task: &GanttTask,
) -> Result<()> {
    match &task.start_constraint {
        GanttTaskStartConstraint::PreviousTaskEnd { dependency_id } => {
            if let Some(id) = dependency_id {
                push_constraint_value(line, "after", id)?;
            }
        }
        GanttTaskStartConstraint::Fixed { value } => {
            push_constraint_value(line, "start", value)?;
        }
        GanttTaskStartConstraint::After { dependency_ids } => {
            push_dependency_constraint(line, "after", dependency_ids)?;
        }
    }
    Ok(())
}

fn push_end_constraint(
    line: &mut crate::safe_text::BudgetedWrappedText<'_>,
    task: &GanttTask,
) -> Result<()> {
    match &task.end_constraint {
        GanttTaskEndConstraint::Unspecified => Ok(()),
        GanttTaskEndConstraint::Fixed { value } => push_constraint_value(line, "end", value),
        GanttTaskEndConstraint::Duration { value } => {
            push_constraint_value(line, "duration", value)
        }
        GanttTaskEndConstraint::Until { dependency_ids } => {
            push_dependency_constraint(line, "until", dependency_ids)
        }
    }
}

fn push_constraint_value(
    line: &mut crate::safe_text::BudgetedWrappedText<'_>,
    key: &str,
    value: &str,
) -> Result<()> {
    push_wrapped_field(line, ", ", key, value)
}

fn push_dependency_constraint(
    line: &mut crate::safe_text::BudgetedWrappedText<'_>,
    key: &str,
    dependency_ids: &[String],
) -> Result<()> {
    push_wrapped_list(line, ", ", key, dependency_ids.iter().map(String::as_str))
}

fn format_date(ms: i64, local_time_zone: &merman_core::time::LocalTimeZone) -> String {
    local_time_zone
        .at_instant(ms)
        .map(|local| {
            format_resolved_date(
                ms,
                local,
                local_time_zone.resolve_local(local.local_datetime()),
            )
        })
        .unwrap_or_else(|| ms.to_string())
}

fn format_resolved_date(
    ms: i64,
    local: merman_core::time::OffsetDateTime,
    compatible: Option<merman_core::time::OffsetDateTime>,
) -> String {
    let datetime = local.local_datetime();
    let mut formatted = if datetime.hour() == 0
        && datetime.minute() == 0
        && datetime.second() == 0
        && datetime.millisecond() == 0
    {
        local.date().to_string()
    } else {
        datetime.to_string()
    };
    if compatible.is_some_and(|compatible| compatible.timestamp_millis() != ms) {
        formatted.push_str(&local.offset().to_string());
    }
    formatted
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
    use merman_core::diagrams::gantt::{GanttDiagramRenderModel, GanttRenderTask};
    use merman_core::resources::ResourceProfile;
    use merman_core::time::{OffsetDateTime, UtcOffset};
    use merman_core::{CancelReason, OperationControl, OperationPhase};
    use std::cell::Cell;

    fn direct_model() -> GanttDiagramRenderModel {
        let mut model = GanttDiagramRenderModel::default();
        model.sections = vec!["Build".to_string(), "Ship".to_string()];
        model.tasks = vec![
            GanttRenderTask {
                semantic: GanttTask {
                    id: "a".to_string(),
                    section: "Build".to_string(),
                    ..GanttTask::default()
                },
                ..GanttRenderTask::default()
            },
            GanttRenderTask {
                semantic: GanttTask {
                    id: "b".to_string(),
                    section: "Orphan".to_string(),
                    ..GanttTask::default()
                },
                ..GanttRenderTask::default()
            },
        ];
        model
    }

    #[test]
    fn canonical_gantt_tasks_match_full_terminal_dates_sections_and_constraints() {
        use merman_core::presentation::PresentationConfig;
        use merman_core::{
            MermaidConfig, MermaidEngine, ParseOptions, RenderSemanticModel, SemanticDiagramModel,
            SemanticParser,
        };

        let runtime = merman_core::runtime::RuntimePolicy::deterministic()
            .with_local_time_zone(merman_core::time::LocalTimeZone::utc());
        let config = serde_json::json!({"gantt": {"displayMode": "compact"}});
        let parser = SemanticParser::new()
            .with_runtime_policy(runtime.clone())
            .with_site_config(MermaidConfig::from_value(config.clone()));
        let engine = MermaidEngine::new()
            .with_runtime_policy(runtime)
            .with_site_config(PresentationConfig::from_value(config));
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default();
        for source in [
            "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nsection Delivery\nTask: task,2019-02-01,1d\nsection Delivery\nNext: next,after task,2d\n",
            "gantt\ndateFormat YYYY-MM-DD\nexcludes weekends\nincludes 2019-02-02\nsection Delivery\nTask: task,2019-02-01,1d\nFixed: fixed,2019-02-02,2019-02-04\n",
            "gantt\nsection Empty\nsection Empty\n",
        ] {
            let canonical = parser
                .parse_diagram_sync(source)
                .expect("canonical Gantt parse");
            let SemanticDiagramModel::Gantt(model) = &canonical.model else {
                panic!("expected canonical Gantt");
            };
            let full = engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .expect("full Gantt parse")
                .expect("detected Gantt");
            let RenderSemanticModel::Gantt(full) = full.model() else {
                panic!("expected full Gantt");
            };
            for (canonical, full) in model.tasks.iter().zip(&full.tasks) {
                assert_eq!(canonical.render_end_ms, full.render_end_ms);
                assert_eq!(canonical.render_end_ms, full.semantic.render_end_ms);
            }
            let actual = render_gantt_diagram(
                GanttTerminalView::from_model(model, &canonical.metadata.semantic_config),
                &options,
                &merman_core::time::LocalTimeZone::utc(),
                AsciiExecution::for_test(&policy),
            )
            .expect("canonical Gantt terminal output");
            let expected = render_gantt_diagram(
                GanttTerminalView::from_render(full),
                &options,
                &merman_core::time::LocalTimeZone::utc(),
                AsciiExecution::for_test(&policy),
            )
            .expect("full Gantt terminal output");
            assert_eq!(actual, expected, "{source}");
        }
    }

    #[test]
    fn repeated_local_time_discloses_the_later_absolute_instant() {
        let daylight_offset = UtcOffset::from_minutes(-4 * 60).expect("valid EDT offset");
        let standard_offset = UtcOffset::from_minutes(-5 * 60).expect("valid EST offset");
        let earlier_ms = 1_793_511_000_000;
        let later_ms = 1_793_514_600_000;
        let earlier = OffsetDateTime::from_unix_millis(earlier_ms, daylight_offset);
        let later = OffsetDateTime::from_unix_millis(later_ms, standard_offset);

        assert_eq!(earlier.local_datetime(), later.local_datetime());
        assert_eq!(
            format_resolved_date(earlier_ms, earlier, Some(earlier)),
            "2026-11-01T01:30:00.000"
        );
        assert_eq!(
            format_resolved_date(later_ms, later, Some(earlier)),
            "2026-11-01T01:30:00.000-05:00"
        );
    }

    #[test]
    fn gantt_admission_accepts_exact_work_before_materializing_index() {
        const STRUCTURE_WORK: usize = 40;
        let model = direct_model();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, STRUCTURE_WORK)
            .expect("exact Gantt admission work limit should be valid");
        let resources = ResourceContext::new(policy);
        let materialized = Cell::new(false);
        let index = admit_then_materialize_gantt_structure(
            GanttTerminalView::from_render(&model),
            &resources,
            |admission, resources| {
                materialized.set(true);
                GanttTaskIndex::materialize(
                    GanttTerminalView::from_render(&model),
                    admission,
                    resources,
                )
            },
        )
        .expect("exact Gantt admission work should permit materialization");

        assert!(materialized.get());
        let index = index.expect("declared sections require a grouped task index");
        assert_eq!(index.tasks_by_section.len(), 2);
        assert_eq!(resources.layout_work_used(), STRUCTURE_WORK);
    }

    #[test]
    fn gantt_admission_rejects_max_minus_one_before_materializing_index() {
        const STRUCTURE_WORK: usize = 40;
        let model = direct_model();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, STRUCTURE_WORK - 1)
            .expect("max-minus-one Gantt admission work limit should be valid");
        let resources = ResourceContext::new(policy);
        let materialized = Cell::new(false);
        let error = admit_then_materialize_gantt_structure(
            GanttTerminalView::from_render(&model),
            &resources,
            |admission, resources| {
                materialized.set(true);
                GanttTaskIndex::materialize(
                    GanttTerminalView::from_render(&model),
                    admission,
                    resources,
                )
                .map(|_| ())
            },
        )
        .expect_err("max-minus-one Gantt admission work should fail before materialization");

        assert!(!materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == STRUCTURE_WORK
                    && details.max == STRUCTURE_WORK - 1
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn gantt_index_failure_rolls_back_structure_work() {
        const PRIOR_WORK: usize = 3;
        const PRIOR_CELLS: usize = 2;
        let mut model = direct_model();
        let duplicate_id = model.tasks[0].id.clone();
        model.tasks[1].id = duplicate_id;
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(PRIOR_WORK, PRIOR_CELLS)
            .expect("prior ledger usage should fit");

        let error = admit_then_materialize_gantt_structure(
            GanttTerminalView::from_render(&model),
            &resources,
            |admission, resources| {
                GanttTaskIndex::materialize(
                    GanttTerminalView::from_render(&model),
                    admission,
                    resources,
                )
                .map(|_| ())
            },
        )
        .expect_err("duplicate ids should reject index materialization");

        assert_eq!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "gantt",
                feature: "duplicate task ids",
            }
        );
        assert_eq!(resources.layout_work_used(), PRIOR_WORK);
        assert_eq!(resources.document_cells_used(), PRIOR_CELLS);
    }

    #[test]
    fn gantt_rejects_explicit_section_occurrence_without_declared_sections() {
        let mut model = GanttDiagramRenderModel::default();
        model.tasks.push(GanttRenderTask {
            semantic: GanttTask {
                id: "task".to_string(),
                section: "missing".to_string(),
                section_index: Some(0),
                ..GanttTask::default()
            },
            ..GanttRenderTask::default()
        });
        let resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));
        let materialized = Cell::new(false);

        let error = match admit_then_materialize_gantt_structure(
            GanttTerminalView::from_render(&model),
            &resources,
            |admission, resources| {
                materialized.set(true);
                GanttTaskIndex::materialize(
                    GanttTerminalView::from_render(&model),
                    admission,
                    resources,
                )
            },
        ) {
            Ok(_) => panic!("an explicit occurrence requires a declared section"),
            Err(error) => error,
        };

        assert_eq!(error, invalid_task_section_occurrence());
        assert!(!materialized.get());
    }

    #[test]
    fn gantt_index_cancellation_rolls_back_admitted_work() {
        const PRIOR_WORK: usize = 3;
        const PRIOR_CELLS: usize = 2;
        let model = direct_model();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(PRIOR_WORK, PRIOR_CELLS)
            .expect("prior ledger usage should fit");
        let control = OperationControl::new();
        let controlled = resources.controlled(control.clone(), OperationPhase::Layout);

        let error = admit_then_materialize_gantt_structure(
            GanttTerminalView::from_render(&model),
            &controlled,
            |admission, resources| {
                control.cancel();
                GanttTaskIndex::materialize(
                    GanttTerminalView::from_render(&model),
                    admission,
                    resources,
                )
                .map(|_| ())
            },
        )
        .expect_err("materialization should observe cancellation at its first checkpoint");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), PRIOR_WORK);
        assert_eq!(resources.document_cells_used(), PRIOR_CELLS);
    }
}
