use crate::Result;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::safe_text::{
    BudgetedTextDocument, push_document_field, push_line_field, push_optional_document_field,
    push_wrapped_field,
};
use crate::sectioned_text::{SectionedTextPlan, SectionedTextTask, plan_sectioned_text};
use merman_model::families::timeline::{TimelineDiagramModel, TimelineDirection, TimelineTask};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub(super) fn render_timeline_diagram(
    model: &TimelineDiagramModel,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let layout_resources = execution.new_resource_context(merman_model::OperationPhase::Layout);
    let mut document = BudgetedTextDocument::from_resources(layout_resources, options);
    let section_plan = plan_sectioned_text(
        "timeline",
        &model.sections,
        &model.tasks,
        document.resources_mut(),
    )?;
    execution.rebind_resource_context(document.resources_mut(), merman_model::OperationPhase::Emit);

    push_optional_document_field(&mut document, "title", model.title.as_deref())?;
    push_optional_document_field(&mut document, "accTitle", model.acc_title.as_deref())?;
    push_optional_document_field(&mut document, "accDescr", model.acc_descr.as_deref())?;
    document.push_line_with(|line| {
        line.push_str("direction: ")?;
        line.push_str(match model.direction {
            TimelineDirection::LeftToRight => "LR",
            TimelineDirection::TopDown => "TD",
        })
    })?;

    if !model.sections.is_empty() {
        for (section_index, section) in model.sections.iter().enumerate() {
            execution.checkpoint(merman_model::OperationPhase::Emit)?;
            push_document_field(&mut document, "section", section)?;
            for task_index in section_plan.tasks_for_section(section_index) {
                push_task(&mut document, &model.tasks[*task_index], execution)?;
            }
        }
    }
    push_orphan_tasks(&mut document, model, &section_plan, execution)?;

    document.finish_with_execution(execution)
}

impl SectionedTextTask for TimelineTask {
    fn section_label(&self) -> &str {
        &self.section
    }

    fn section_index(&self) -> Option<usize> {
        self.section_index
    }
}

fn push_orphan_tasks(
    document: &mut BudgetedTextDocument,
    model: &TimelineDiagramModel,
    section_plan: &SectionedTextPlan,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    let mut previous_section = None;
    for task_index in section_plan.orphan_task_indices() {
        let task = &model.tasks[*task_index];
        if task.section.is_empty() && model.sections.is_empty() {
            // A section-less Mermaid timeline is a valid flat list; keep its established
            // projection while still disclosing empty ownership once sections are declared.
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

fn push_task(
    document: &mut BudgetedTextDocument,
    task: &TimelineTask,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    execution.checkpoint(merman_model::OperationPhase::Emit)?;
    document.resources_mut().charge_layout_work(1)?;
    document.push_wrapped_prefixed_line_with("  - ", "    ", SUMMARY_WRAP_WIDTH, |line| {
        push_wrapped_field(line, "", "task", &task.task)
    })?;
    for event in &task.events {
        execution.checkpoint(merman_model::OperationPhase::Emit)?;
        document.resources_mut().charge_layout_work(1)?;
        document.push_wrapped_prefixed_line_with(
            "    * ",
            "      ",
            SUMMARY_WRAP_WIDTH,
            |line| push_wrapped_field(line, "", "event", event),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::AsciiResourcePolicy;

    #[test]
    fn canonical_timeline_matches_full_direction_sections_and_events() {
        use merman_core::{
            MermaidEngine, ParseOptions, RenderSemanticModel, SemanticDiagramModel, SemanticParser,
        };

        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default();
        for source in [
            "timeline\ntitle History\naccTitle: Accessible\naccDescr: Complete timeline\nsection Era\n2025 : Start : Build\nsection Era\n2026 : Ship\n",
            "timeline TD\n2026 : Released\n",
            "timeline\nsection Empty\nsection Empty\n",
        ] {
            let canonical = SemanticParser::new().parse_diagram_sync(source).unwrap();
            let SemanticDiagramModel::Timeline(model) = &canonical.model else {
                panic!("Timeline model")
            };
            let full = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let RenderSemanticModel::Timeline(full) = full.model() else {
                panic!("Timeline model")
            };
            assert!(std::ptr::eq(full.tasks.as_ptr(), full.tasks.as_ptr()));
            let actual =
                render_timeline_diagram(model, &options, AsciiExecution::for_test(&policy))
                    .unwrap();
            let expected =
                render_timeline_diagram(full, &options, AsciiExecution::for_test(&policy)).unwrap();
            assert_eq!(actual, expected, "{source}");
        }
    }
}
