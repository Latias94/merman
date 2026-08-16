use super::RelationGraphLine;
use crate::Result;
#[cfg(test)]
use crate::canvas::finish_styled_line_iter_with_deferred_probe;
#[cfg(test)]
use crate::canvas::finish_styled_line_iter_with_deferred_resources;
use crate::canvas::finish_styled_line_iter_with_deferred_resources_with_execution;
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
    if lines.is_empty() {
        return Ok(String::new());
    }

    assert_width_profile(lines, options);
    let mut resources = execution.resource_context(resources, merman_core::OperationPhase::Emit);
    finish_styled_line_iter_with_deferred_resources_with_execution(
        lines.iter().map(RelationGraphLine::styled),
        options,
        true,
        &mut resources,
        deferred,
        execution,
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
    if lines.is_empty() {
        return Ok(String::new());
    }

    assert_width_profile(lines, options);
    finish_styled_line_iter_with_deferred_probe(
        lines.iter().map(RelationGraphLine::styled),
        options,
        true,
        resources,
        deferred,
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
