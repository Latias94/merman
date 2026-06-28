use crate::options::AsciiRenderOptions;
use crate::text::{normalize_optional_text, push_wrapped_prefixed_line, trim_trailing_blank_lines};
use merman_core::diagrams::journey::{JourneyDiagramRenderModel, JourneyRenderTask};
use std::collections::BTreeSet;

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_journey_diagram(
    model: &JourneyDiagramRenderModel,
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

    let actors = if model.actors.is_empty() {
        collect_actors(&model.tasks)
    } else {
        model.actors.clone()
    };
    if !actors.is_empty() {
        lines.push(format!("actors: {}", actors.join(", ")));
    }

    if !model.sections.is_empty() {
        for section in &model.sections {
            lines.push(format!("section: {section}"));
            for task in model.tasks.iter().filter(|task| task.section == *section) {
                push_task(&mut lines, task);
            }
        }
        for task in model.tasks.iter().filter(|task| {
            !model
                .sections
                .iter()
                .any(|section| section == &task.section)
        }) {
            push_task(&mut lines, task);
        }
    } else {
        for task in &model.tasks {
            push_task(&mut lines, task);
        }
    }

    trim_trailing_blank_lines(lines).join("\n")
}

fn collect_actors(tasks: &[JourneyRenderTask]) -> Vec<String> {
    let mut set = BTreeSet::new();
    for task in tasks {
        for actor in &task.people {
            if !actor.is_empty() {
                set.insert(actor.clone());
            }
        }
    }
    set.into_iter().collect()
}

fn push_task(lines: &mut Vec<String>, task: &JourneyRenderTask) {
    let score = if task.score_is_nan {
        "NaN".to_string()
    } else {
        task.score.to_string()
    };
    let people = if task.people.is_empty() {
        String::new()
    } else {
        format!(" ({})", task.people.join(", "))
    };
    push_wrapped_prefixed_line(
        lines,
        "  - ",
        "    ",
        &format!("{} [score {}]{}", task.task, score, people),
        SUMMARY_WRAP_WIDTH,
    );
}
