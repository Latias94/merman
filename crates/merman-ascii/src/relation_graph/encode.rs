use super::RelationGraphLine;
use crate::Result;
#[cfg(test)]
use crate::canvas::finish_styled_line_iter_with_deferred_resources;
use crate::canvas::finish_styled_line_iter_with_deferred_resources_with_execution_and_observer;
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::safe_text::DeferredTextRegistry;

#[cfg(test)]
pub(crate) fn render_lines_with_options(
    lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    if lines.is_empty() {
        return Ok(String::new());
    }

    assert_width_profile(lines, options);
    crate::canvas::finish_styled_line_iter_with_resources(
        lines.iter().map(RelationGraphLine::styled),
        options,
        true,
        resources,
    )
}

#[cfg(test)]
pub(crate) fn render_lines_with_deferred_options(
    lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred: &DeferredTextRegistry<'_>,
) -> Result<String> {
    if lines.is_empty() {
        return Ok(String::new());
    }

    assert_width_profile(lines, options);
    finish_styled_line_iter_with_deferred_resources(
        lines.iter().map(RelationGraphLine::styled),
        options,
        true,
        resources,
        deferred,
    )
}

pub(crate) fn render_lines_with_deferred_options_with_execution(
    lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred: &DeferredTextRegistry<'_>,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    render_lines_with_deferred_options_with_execution_and_observer(
        lines,
        options,
        resources,
        deferred,
        execution,
        || {},
    )
}

pub(crate) fn render_lines_with_deferred_options_with_execution_and_observer(
    lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred: &DeferredTextRegistry<'_>,
    execution: AsciiExecution<'_>,
    before_materialize: impl FnOnce(),
) -> Result<String> {
    if lines.is_empty() {
        return Ok(String::new());
    }

    assert_width_profile(lines, options);
    finish_styled_line_iter_with_deferred_resources_with_execution_and_observer(
        lines.iter().map(RelationGraphLine::styled),
        options,
        true,
        resources,
        deferred,
        execution,
        before_materialize,
    )
}

#[cfg(test)]
pub(crate) fn render_lines_with_deferred_probe(
    lines: &[RelationGraphLine],
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred: &DeferredTextRegistry<'_>,
    before_materialize: impl FnOnce(),
) -> Result<String> {
    let policy = resources.policy();
    render_lines_with_deferred_options_with_execution_and_observer(
        lines,
        options,
        resources,
        deferred,
        AsciiExecution::for_test(&policy),
        before_materialize,
    )
}

fn assert_width_profile(lines: &[RelationGraphLine], options: &AsciiRenderOptions) {
    debug_assert!(
        lines
            .iter()
            .all(|line| line.width_profile() == options.terminal_width_profile)
    );
}
