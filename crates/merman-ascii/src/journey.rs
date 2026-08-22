use crate::Result;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{
    BudgetedTextDocument, charge_text_layout, push_document_field, push_line_field, push_line_list,
    push_optional_document_field, push_wrapped_field, push_wrapped_list,
};
use crate::sectioned_text::{SectionedTextPlan, SectionedTextTask, plan_sectioned_text};
use merman_core::diagrams::journey::{JourneyDiagramRenderModel, JourneyRenderTask};
use std::cmp::Ordering;

const SUMMARY_WRAP_WIDTH: usize = 80;

pub(super) fn render_journey_diagram(
    model: &JourneyDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let layout_resources = execution.new_resource_context(merman_core::OperationPhase::Layout);
    let mut document = BudgetedTextDocument::from_resources(layout_resources, options);
    let section_plan = plan_sectioned_text(
        "journey",
        &model.sections,
        &model.tasks,
        document.resources_mut(),
    )?;
    let actors = if model.actors.is_empty() {
        collect_actors(&model.tasks, document.resources_mut())?
    } else {
        for _ in &model.actors {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
        }
        Vec::new()
    };
    execution.rebind_resource_context(document.resources_mut(), merman_core::OperationPhase::Emit);

    push_optional_document_field(&mut document, "title", model.title.as_deref())?;
    push_optional_document_field(&mut document, "accTitle", model.acc_title.as_deref())?;
    push_optional_document_field(&mut document, "accDescr", model.acc_descr.as_deref())?;

    if !model.actors.is_empty() || !actors.is_empty() {
        document.push_line_with(|line| {
            if model.actors.is_empty() {
                push_line_list(line, "", "actors", actors.iter().copied())?;
            } else {
                push_line_list(line, "", "actors", model.actors.iter().map(String::as_str))?;
            }
            Ok(())
        })?;
    }

    if !model.sections.is_empty() {
        for (section_index, section) in model.sections.iter().enumerate() {
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            push_document_field(&mut document, "section", section)?;
            for task_index in section_plan.tasks_for_section(section_index) {
                push_task(&mut document, &model.tasks[*task_index], execution)?;
            }
        }
    }
    push_orphan_tasks(&mut document, model, &section_plan, execution)?;

    document.finish()
}

impl SectionedTextTask for JourneyRenderTask {
    fn section_label(&self) -> &str {
        &self.section
    }

    fn section_index(&self) -> Option<usize> {
        self.section_index
    }
}

fn push_orphan_tasks(
    document: &mut BudgetedTextDocument,
    model: &JourneyDiagramRenderModel,
    section_plan: &SectionedTextPlan,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    let mut previous_section = None;
    for task_index in section_plan.orphan_task_indices() {
        let task = &model.tasks[*task_index];
        // A section-less Mermaid journey is a valid flat list; keep its established projection
        // while still disclosing empty ownership once sections are declared.
        if task.section.is_empty() && model.sections.is_empty() {
            push_task(document, task, execution)?;
            continue;
        }
        if previous_section != Some(task.section.as_str()) {
            push_orphan_section(document, &task.section)?;
            previous_section = Some(task.section.as_str());
        }
        push_task(document, task, execution)?;
    }
    Ok(())
}

fn push_orphan_section(document: &mut BudgetedTextDocument, section: &str) -> Result<()> {
    document.push_line_with(|line| {
        push_line_field(line, "", "section", section)?;
        line.push_str(if section.is_empty() {
            " status=unsectioned"
        } else {
            " status=undeclared"
        })
    })
}

fn collect_actors<'a>(
    tasks: &'a [JourneyRenderTask],
    resources: &ResourceContext,
) -> Result<Vec<&'a str>> {
    admit_then_materialize_journey_actors(tasks, resources, |admission, resources| {
        admission.materialize(tasks, resources)
    })
}

#[derive(Debug, Clone, Copy)]
struct JourneyActorAdmission {
    actor_capacity: usize,
}

impl JourneyActorAdmission {
    fn preflight(tasks: &[JourneyRenderTask], resources: &ResourceContext) -> Result<Self> {
        let mut actor_capacity = 0usize;
        let mut max_actor_bytes = 0usize;
        let mut replay_work = 0usize;

        for task in tasks {
            replay_work = resources.checked_work_add(replay_work, 1)?;
            resources.charge_layout_work(1)?;
            for actor in &task.people {
                replay_work = resources.checked_work_add(replay_work, 1)?;
                resources.charge_layout_work(1)?;
                if actor.is_empty() {
                    continue;
                }
                actor_capacity = resources.checked_work_add(actor_capacity, 1)?;
                max_actor_bytes = max_actor_bytes.max(actor.len());
                charge_text_layout(resources, actor)?;
            }
        }

        let actor_work = actor_materialization_work(actor_capacity, max_actor_bytes, resources)?;
        let materialization_work = resources.checked_work_add(replay_work, actor_work)?;
        resources.charge_layout_work(materialization_work)?;

        Ok(Self { actor_capacity })
    }

    fn materialize<'a>(
        self,
        tasks: &'a [JourneyRenderTask],
        resources: &ResourceContext,
    ) -> Result<Vec<&'a str>> {
        resources.checkpoint()?;
        let mut actors = Vec::new();
        actors
            .try_reserve_exact(self.actor_capacity)
            .map_err(|_| layout_allocation_error())?;

        for (task_index, task) in tasks.iter().enumerate() {
            checkpoint_loop(resources, task_index)?;
            for (actor_index, actor) in task.people.iter().enumerate() {
                checkpoint_loop(resources, actor_index)?;
                if !actor.is_empty() {
                    actors.push(actor.as_str());
                }
            }
        }
        debug_assert_eq!(actors.len(), self.actor_capacity);

        sort_and_dedup_actors(&mut actors, resources)?;
        Ok(actors)
    }
}

fn admit_then_materialize_journey_actors<T>(
    tasks: &[JourneyRenderTask],
    resources: &ResourceContext,
    materialize: impl FnOnce(JourneyActorAdmission, &ResourceContext) -> Result<T>,
) -> Result<T> {
    resources.transaction(|resources| {
        let admission = JourneyActorAdmission::preflight(tasks, resources)?;
        materialize(admission, resources)
    })
}

fn actor_materialization_work(
    actor_count: usize,
    max_actor_bytes: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    if actor_count == 0 {
        return Ok(0);
    }
    if actor_count == 1 {
        return Ok(1);
    }

    // Admit a deterministic upper bound before retaining actor references. Each merge level
    // moves every reference into and back out of scratch, and performs at most N-1 comparisons.
    // A comparison may inspect every byte of the longest actor. Deduplication has the same N-1
    // comparison bound plus at most one in-place reference move per comparison.
    let actor_bytes = max_actor_bytes.max(1);
    let allocation_and_collection = resources.checked_work_mul(actor_count, 2)?;
    let merge_levels = merge_sort_levels(actor_count);
    let comparison_count = actor_count - 1;
    let comparison_work = resources.checked_work_mul(comparison_count, actor_bytes)?;
    let move_work = resources.checked_work_mul(actor_count, 2)?;
    let merge_level_work = resources.checked_work_add(comparison_work, move_work)?;
    let merge_work = resources.checked_work_mul(merge_levels, merge_level_work)?;
    let dedup_item_work = resources.checked_work_add(actor_bytes, 1)?;
    let dedup_work = resources.checked_work_mul(comparison_count, dedup_item_work)?;
    let materialize_and_merge =
        resources.checked_work_add(allocation_and_collection, merge_work)?;
    resources.checked_work_add(materialize_and_merge, dedup_work)
}

fn merge_sort_levels(len: usize) -> usize {
    if len <= 1 {
        0
    } else {
        usize::BITS as usize - (len - 1).leading_zeros() as usize
    }
}

fn sort_and_dedup_actors(actors: &mut Vec<&str>, resources: &ResourceContext) -> Result<()> {
    if actors.len() <= 1 {
        return Ok(());
    }

    resources.checkpoint()?;
    let mut scratch = Vec::new();
    scratch
        .try_reserve_exact(actors.len())
        .map_err(|_| layout_allocation_error())?;

    let mut width = 1usize;
    while width < actors.len() {
        resources.checkpoint()?;
        scratch.clear();
        let mut start = 0usize;
        while start < actors.len() {
            resources.checkpoint()?;
            let middle = resources.checked_work_add(start, width)?.min(actors.len());
            let end = resources.checked_work_add(middle, width)?.min(actors.len());
            merge_actor_runs(actors, start, middle, end, &mut scratch, resources)?;
            start = end;
        }

        for (index, actor) in scratch.iter().copied().enumerate() {
            checkpoint_loop(resources, index)?;
            actors[index] = actor;
        }
        width = if width > actors.len() / 2 {
            actors.len()
        } else {
            width * 2
        };
    }

    let mut unique_len = 1usize;
    for read_index in 1..actors.len() {
        let actor = actors[read_index];
        let previous = actors[unique_len - 1];
        if compare_actor_text(previous, actor, resources)? != Ordering::Equal {
            actors[unique_len] = actor;
            unique_len += 1;
        }
    }
    actors.truncate(unique_len);
    Ok(())
}

fn merge_actor_runs<'a>(
    actors: &[&'a str],
    start: usize,
    middle: usize,
    end: usize,
    output: &mut Vec<&'a str>,
    resources: &ResourceContext,
) -> Result<()> {
    let mut left = start;
    let mut right = middle;
    while left < middle && right < end {
        resources.checkpoint()?;
        if compare_actor_text(actors[left], actors[right], resources)? != Ordering::Greater {
            output.push(actors[left]);
            left += 1;
        } else {
            output.push(actors[right]);
            right += 1;
        }
    }
    while left < middle {
        checkpoint_loop(resources, left - start)?;
        output.push(actors[left]);
        left += 1;
    }
    while right < end {
        checkpoint_loop(resources, right - middle)?;
        output.push(actors[right]);
        right += 1;
    }
    Ok(())
}

fn compare_actor_text(left: &str, right: &str, resources: &ResourceContext) -> Result<Ordering> {
    for (index, (left_byte, right_byte)) in left.bytes().zip(right.bytes()).enumerate() {
        checkpoint_loop(resources, index)?;
        match left_byte.cmp(&right_byte) {
            Ordering::Equal => {}
            ordering => return Ok(ordering),
        }
    }
    Ok(left.len().cmp(&right.len()))
}

fn checkpoint_loop(resources: &ResourceContext, iteration: usize) -> Result<()> {
    if iteration.is_multiple_of(64) {
        resources.checkpoint()?;
    }
    Ok(())
}

fn layout_allocation_error() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

fn push_task(
    document: &mut BudgetedTextDocument,
    task: &JourneyRenderTask,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    execution.checkpoint(merman_core::OperationPhase::Emit)?;
    document.resources_mut().charge_layout_work(1)?;
    document.push_wrapped_prefixed_line_with("  - ", "    ", SUMMARY_WRAP_WIDTH, |line| {
        push_wrapped_field(line, "", "task", &task.task)?;
        line.push_str(" score=")?;
        if task.score_is_nan {
            line.push_str("NaN")?;
        } else {
            line.write_fmt(format_args!("{}", task.score))?;
        }
        if !task.people.is_empty() {
            push_wrapped_list(line, " ", "people", task.people.iter().map(String::as_str))?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use merman_core::{CancelReason, OperationControl, OperationPhase};
    use std::cell::Cell;

    fn actor_tasks() -> Vec<JourneyRenderTask> {
        vec![JourneyRenderTask {
            score: 0,
            score_is_nan: false,
            people: vec!["Bob".to_string(), "Alice".to_string(), "Bob".to_string()],
            section: String::new(),
            section_index: None,
            task_type: String::new(),
            task: String::new(),
        }]
    }

    #[test]
    fn actor_collection_accepts_fixed_exact_work_before_materialization() {
        const ACTOR_PLAN_WORK: usize = 72;
        let tasks = actor_tasks();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, ACTOR_PLAN_WORK)
            .expect("exact actor-planning work limit should be valid");
        let resources = ResourceContext::new(policy);
        let materialized = Cell::new(false);

        let actors =
            admit_then_materialize_journey_actors(&tasks, &resources, |admission, resources| {
                materialized.set(true);
                admission.materialize(&tasks, resources)
            })
            .expect("exact actor-planning work should permit materialization");

        assert!(materialized.get());
        assert_eq!(actors, ["Alice", "Bob"]);
        assert_eq!(resources.layout_work_used(), ACTOR_PLAN_WORK);
    }

    #[test]
    fn actor_collection_rejects_fixed_max_minus_one_before_materialization() {
        const ACTOR_PLAN_WORK: usize = 72;
        let tasks = actor_tasks();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                ACTOR_PLAN_WORK - 1,
            )
            .expect("max-minus-one actor-planning work limit should be valid");
        let resources = ResourceContext::new(policy);
        let materialized = Cell::new(false);

        let error =
            admit_then_materialize_journey_actors(&tasks, &resources, |admission, resources| {
                materialized.set(true);
                admission.materialize(&tasks, resources)
            })
            .expect_err("max-minus-one actor work should reject before materialization");

        assert!(!materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == ACTOR_PLAN_WORK
                    && details.max == ACTOR_PLAN_WORK - 1
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn actor_materialization_cancellation_rolls_back_admitted_work() {
        const PRIOR_WORK: usize = 3;
        const PRIOR_CELLS: usize = 2;
        let tasks = actor_tasks();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(PRIOR_WORK, PRIOR_CELLS)
            .expect("prior ledger usage should fit");
        let control = OperationControl::new();
        let controlled = resources.controlled(control.clone(), OperationPhase::Layout);

        let error =
            admit_then_materialize_journey_actors(&tasks, &controlled, |admission, resources| {
                control.cancel();
                admission.materialize(&tasks, resources)
            })
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

    #[test]
    fn actor_planning_prioritizes_cancellation_over_next_work_ceiling() {
        const PRIOR_WORK: usize = 1;
        let tasks = actor_tasks();
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, PRIOR_WORK)
            .expect("positive actor-planning work limit should be valid");
        let resources = ResourceContext::new(policy);
        resources
            .charge_layout_work(PRIOR_WORK)
            .expect("the exact prior work should fit");
        let control = OperationControl::new();
        control.cancel();
        let controlled = resources.controlled(control, OperationPhase::Layout);

        let error = collect_actors(&tasks, &controlled)
            .expect_err("cancellation should win before the next failing work charge");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), PRIOR_WORK);
    }
}
