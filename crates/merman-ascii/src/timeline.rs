use crate::Result;
use crate::options::AsciiRenderOptions;
use crate::safe_text::BudgetedTextDocument;
use merman_core::diagrams::timeline::{TimelineDiagramRenderModel, TimelineRenderTask};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_timeline_diagram(
    model: &TimelineDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);

    document.push_optional_line(model.title.as_deref())?;
    document.push_optional_prefixed_line("accTitle: ", model.acc_title.as_deref())?;
    document.push_optional_prefixed_line("accDescr: ", model.acc_descr.as_deref())?;

    if !model.sections.is_empty() {
        for section in &model.sections {
            document.push_line_with(|line| {
                line.push_str("section: ")?;
                line.push_str(section)
            })?;
            for task in model.tasks.iter().filter(|task| task.section == *section) {
                push_task(&mut document, task)?;
            }
        }

        for task in model.tasks.iter().filter(|task| {
            !model
                .sections
                .iter()
                .any(|section| section == &task.section)
        }) {
            push_task(&mut document, task)?;
        }
    } else {
        for task in &model.tasks {
            push_task(&mut document, task)?;
        }
    }

    document.finish(options)
}

fn push_task(document: &mut BudgetedTextDocument, task: &TimelineRenderTask) -> Result<()> {
    document.resources_mut().charge_layout_work(1)?;
    document.push_wrapped_prefixed_line_with("  - ", "    ", SUMMARY_WRAP_WIDTH, |line| {
        line.push_str(&task.task)?;
        if task.score != 0 {
            line.write_fmt(format_args!(" (score {})", task.score))?;
        }
        Ok(())
    })?;
    for event in &task.events {
        document.resources_mut().charge_layout_work(1)?;
        document.push_wrapped_prefixed_line("    * ", "      ", event, SUMMARY_WRAP_WIDTH)?;
    }
    Ok(())
}
