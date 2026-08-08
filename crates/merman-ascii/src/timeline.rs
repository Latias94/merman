use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::safe_text::encode_text_lines;
use crate::text::{
    normalize_optional_text, push_wrapped_prefixed_line_with_profile, trim_trailing_blank_lines,
};
use merman_core::diagrams::timeline::{TimelineDiagramRenderModel, TimelineRenderTask};

const SUMMARY_WRAP_WIDTH: usize = 80;

pub fn render_timeline_diagram(
    model: &TimelineDiagramRenderModel,
    options: &AsciiRenderOptions,
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

    if !model.sections.is_empty() {
        for section in &model.sections {
            lines.push(format!("section: {section}"));
            for task in model.tasks.iter().filter(|task| task.section == *section) {
                push_task(&mut lines, task, options.terminal_width_profile);
            }
        }

        for task in model.tasks.iter().filter(|task| {
            !model
                .sections
                .iter()
                .any(|section| section == &task.section)
        }) {
            push_task(&mut lines, task, options.terminal_width_profile);
        }
    } else {
        for task in &model.tasks {
            push_task(&mut lines, task, options.terminal_width_profile);
        }
    }

    encode_text_lines(trim_trailing_blank_lines(lines), options)
}

fn push_task(
    lines: &mut Vec<String>,
    task: &TimelineRenderTask,
    width_profile: TerminalWidthProfile,
) {
    let score = if task.score == 0 {
        String::new()
    } else {
        format!(" (score {})", task.score)
    };
    push_wrapped_prefixed_line_with_profile(
        lines,
        "  - ",
        "    ",
        &format!("{}{score}", task.task),
        SUMMARY_WRAP_WIDTH,
        width_profile,
    );
    for event in &task.events {
        push_wrapped_prefixed_line_with_profile(
            lines,
            "    * ",
            "      ",
            event,
            SUMMARY_WRAP_WIDTH,
            width_profile,
        );
    }
}
