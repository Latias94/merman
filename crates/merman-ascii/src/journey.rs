use crate::Result;
use crate::options::AsciiRenderOptions;
use crate::safe_text::BudgetedTextDocument;
use merman_core::diagrams::journey::{JourneyDiagramRenderModel, JourneyRenderTask};
use std::collections::BTreeSet;

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_journey_diagram(
    model: &JourneyDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut document = BudgetedTextDocument::new(options);

    document.push_optional_line(model.title.as_deref())?;
    document.push_optional_prefixed_line("accTitle: ", model.acc_title.as_deref())?;
    document.push_optional_prefixed_line("accDescr: ", model.acc_descr.as_deref())?;

    let actors = if model.actors.is_empty() {
        collect_actors(&model.tasks, &mut document)?
    } else {
        BTreeSet::new()
    };
    if !model.actors.is_empty() || !actors.is_empty() {
        document.push_line_with(|line| {
            line.push_str("actors: ")?;
            if model.actors.is_empty() {
                for (index, actor) in actors.iter().enumerate() {
                    if index > 0 {
                        line.push_str(", ")?;
                    }
                    line.push_str(actor)?;
                }
            } else {
                for (index, actor) in model.actors.iter().enumerate() {
                    if index > 0 {
                        line.push_str(", ")?;
                    }
                    line.push_str(actor)?;
                }
            }
            Ok(())
        })?;
    }

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

fn collect_actors<'a>(
    tasks: &'a [JourneyRenderTask],
    document: &mut BudgetedTextDocument,
) -> Result<BTreeSet<&'a str>> {
    let mut set = BTreeSet::new();
    for task in tasks {
        for actor in &task.people {
            if !actor.is_empty() {
                document.preflight_text_work(actor)?;
                set.insert(actor.as_str());
            }
        }
    }
    Ok(set)
}

fn push_task(document: &mut BudgetedTextDocument, task: &JourneyRenderTask) -> Result<()> {
    document.resources_mut().charge_layout_work(1)?;
    document.push_wrapped_prefixed_line_with("  - ", "    ", SUMMARY_WRAP_WIDTH, |line| {
        line.push_str(&task.task)?;
        line.push_str(" [score ")?;
        if task.score_is_nan {
            line.push_str("NaN")?;
        } else {
            line.write_fmt(format_args!("{}", task.score))?;
        }
        line.push_str("]")?;
        if !task.people.is_empty() {
            line.push_str(" (")?;
            for (index, person) in task.people.iter().enumerate() {
                if index > 0 {
                    line.push_str(", ")?;
                }
                line.push_str(person)?;
            }
            line.push_str(")")?;
        }
        Ok(())
    })
}
