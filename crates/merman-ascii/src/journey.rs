use crate::Result;
use crate::error::AsciiError;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::AsciiResourceLimitPhase;
use crate::safe_text::{
    BudgetedTextDocument, push_document_field, push_line_field, push_line_list,
    push_optional_document_field, push_wrapped_field, push_wrapped_list,
};
use crate::sectioned_text::{SectionedTextPlan, SectionedTextTask, plan_sectioned_text};
use merman_core::diagrams::journey::{JourneyDiagramRenderModel, JourneyRenderTask};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub(super) fn render_journey_diagram(
    model: &JourneyDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);
    let section_plan = plan_sectioned_text(
        "journey",
        &model.sections,
        &model.tasks,
        document.resources_mut(),
    )?;

    push_optional_document_field(&mut document, "title", model.title.as_deref())?;
    push_optional_document_field(&mut document, "accTitle", model.acc_title.as_deref())?;
    push_optional_document_field(&mut document, "accDescr", model.acc_descr.as_deref())?;

    let actors = if model.actors.is_empty() {
        collect_actors(&model.tasks, &mut document, execution)?
    } else {
        for _ in &model.actors {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
        }
        Vec::new()
    };
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

    document.finish(options)
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
    document: &mut BudgetedTextDocument,
    execution: AsciiExecution<'_>,
) -> Result<Vec<&'a str>> {
    let mut actors = Vec::new();
    for task in tasks {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        for actor in &task.people {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            if !actor.is_empty() {
                document.preflight_text_work(actor)?;
                actors.try_reserve(1).map_err(|_| {
                    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
                })?;
                actors.push(actor.as_str());
            }
        }
    }
    actors.sort_unstable();
    actors.dedup();
    Ok(actors)
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
