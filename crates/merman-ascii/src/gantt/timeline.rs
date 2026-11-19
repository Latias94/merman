//! An elapsed-time axis over resolved model facts; this module never schedules tasks.

use super::{
    GanttSectionKey, GanttTaskIndex, format_resolved_date, invalid_task_section_occurrence,
};
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use crate::safe_text::{
    BudgetedTextDocument, BudgetedTextLine, terminal_single_line_text_requires_normalization,
};
use crate::text::display_width_with_profile;
use crate::{AsciiCharset, AsciiRenderOptions, Result};
use merman_model::OperationPhase;
use merman_model::families::gantt::GanttTerminalView;
use merman_model::time::LocalTimeZone;

const LABEL_WIDTH: usize = 26;
const MIN_WIDTH: usize = 60;
const MAX_WIDTH: usize = 120;

#[derive(Clone, Copy)]
pub(super) struct Domain {
    start: i64,
    end: i64,
}

pub(super) fn domain(
    model: GanttTerminalView<'_>,
    index: Option<&GanttTaskIndex<'_>>,
    resources: &ResourceContext,
) -> Result<Option<Domain>> {
    resources.checkpoint()?;
    // Compact changes row ownership in Mermaid. Until that layout has its own terminal
    // contract, retain its complete records rather than substitute independent task rows.
    if !model.metadata().display_mode.is_empty() {
        return Ok(None);
    }
    if let Some(index) = index {
        // Group frames may not reorder authored task rows. Detached models can assign sections
        // backwards even while their order fields appear canonical; preserve those as records.
        let mut expected = 0;
        for section in (0..model.metadata().sections.len())
            .map(GanttSectionKey::Declared)
            .chain(
                index
                    .orphan_section_order
                    .iter()
                    .copied()
                    .map(GanttSectionKey::Orphan),
            )
        {
            resources.charge_layout_work(1)?;
            if let Some(tasks) = index.tasks_by_section.get(&section) {
                for &task in tasks {
                    resources.charge_layout_work(1)?;
                    if task != expected {
                        return Ok(None);
                    }
                    expected += 1;
                }
            }
        }
    }
    let mut domain: Option<Domain> = None;
    for (index, view) in model.tasks().enumerate() {
        resources.charge_layout_work(1)?;
        let task = view.semantic;
        if !task.processed
            || task.vert
            || task.end_ms < task.start_ms
            || usize::try_from(task.order).ok() != Some(index)
            || view
                .render_end_ms
                .is_some_and(|end| end < task.start_ms || end > task.end_ms)
        {
            return Ok(None);
        }
        domain = Some(match domain {
            Some(domain) => Domain {
                start: domain.start.min(task.start_ms),
                end: domain.end.max(task.end_ms),
            },
            None => Domain {
                start: task.start_ms,
                end: task.end_ms,
            },
        });
    }
    Ok(domain.filter(|domain| domain.start < domain.end))
}

struct Axis {
    domain: Domain,
    cells: usize,
}

impl Axis {
    /// Doubled instants keep odd-millisecond milestone midpoints exact before rounding.
    fn position(&self, twice_ms: i128) -> usize {
        let offset = (twice_ms - i128::from(self.domain.start) * 2) as u128;
        let span = (i128::from(self.domain.end) - i128::from(self.domain.start)) as u128 * 2;
        ((offset * (self.cells - 1) as u128 + span / 2) / span) as usize
    }
}

pub(super) fn write(
    model: GanttTerminalView<'_>,
    index: Option<&GanttTaskIndex<'_>>,
    domain: Domain,
    local_time_zone: &LocalTimeZone,
    options: &AsciiRenderOptions,
    document: &mut BudgetedTextDocument,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    let width = execution
        .max_width()
        .unwrap_or(80)
        .clamp(MIN_WIDTH, MAX_WIDTH);
    let axis = Axis {
        domain,
        cells: width - LABEL_WIDTH - 4,
    };
    let resources = document.resources_mut();
    let grid_rows = resources.checked_grid_add(model.tasks().len(), 1)?;
    resources.grid_extent(width, grid_rows)?;
    resources.charge_layout_work_product(grid_rows, axis.cells)?;
    execution.rebind_resource_context(document.resources_mut(), OperationPhase::Emit);
    wrapped(document, width, "Gantt timeline (elapsed time)")?;
    let start = format_axis_endpoint(domain.start, local_time_zone);
    let end = format_axis_endpoint(domain.end, local_time_zone);
    document.push_wrapped_prefixed_line_with("", "  ", width, |line| {
        line.push_str("From ")?;
        line.push_str(&start)?;
        line.push_str(" to ")?;
        line.push_str(&end)?;
        Ok(())
    })?;
    let span = (i128::from(domain.end) - i128::from(domain.start)) as u128;
    let cell_ms = span.div_ceil((axis.cells - 1) as u128);
    document.push_wrapped_prefixed_line_with("", "  ", width, |line| {
        line.write_fmt(format_args!(
            "Endpoints rounded to cells; <= {cell_ms} ms/cell."
        ))
    })?;
    document.push_line_with(|line| {
        repeat(line, " ", LABEL_WIDTH)?;
        line.push_str(" | ")?;
        line.push_str("start")?;
        repeat(line, " ", axis.cells / 2 - 6)?;
        line.push_str("50%")?;
        repeat(line, " ", axis.cells - axis.cells / 2 - 5)?;
        line.push_str("end|")
    })?;

    let metadata = model.metadata();
    if let Some(index) = index {
        for (section_index, section) in metadata.sections.iter().enumerate() {
            section_heading(document, width, Some(section_index), section)?;
            if let Some(tasks) = index
                .tasks_by_section
                .get(&GanttSectionKey::Declared(section_index))
            {
                for &task in tasks {
                    write_task(model, task, &axis, options, document)?;
                }
            }
        }
        for &section in &index.orphan_section_order {
            section_heading(document, width, None, section)?;
            if let Some(tasks) = index
                .tasks_by_section
                .get(&GanttSectionKey::Orphan(section))
            {
                for &task in tasks {
                    write_task(model, task, &axis, options, document)?;
                }
            }
        }
    } else {
        let mut section = None;
        for (index, view) in model.tasks().enumerate() {
            let task = view.semantic;
            if section != Some(task.section.as_str()) {
                section = Some(task.section.as_str());
                if !task.section.is_empty() {
                    section_heading(document, width, None, &task.section)?;
                }
            }
            write_task(model, index, &axis, options, document)?;
        }
    }
    wrapped(
        document,
        width,
        "D done; A active; ! critical; o milestone; * collapsed interval.",
    )?;
    wrapped(
        document,
        width,
        "Long or unsafe labels use their T key; exact task records follow.",
    )?;
    wrapped(
        document,
        width,
        "Bars end at visible end; dependencies use scheduled end in records.",
    )
}

fn section_heading(
    document: &mut BudgetedTextDocument,
    width: usize,
    index: Option<usize>,
    section: &str,
) -> Result<()> {
    document.push_wrapped_prefixed_line_with("", "  ", width, |line| {
        if let Some(index) = index {
            line.write_fmt(format_args!("Section {}: ", index + 1))?;
        } else {
            line.push_str("Section: ")?;
        }
        line.push_quoted_text(section)
    })
}

fn write_task(
    model: GanttTerminalView<'_>,
    index: usize,
    axis: &Axis,
    options: &AsciiRenderOptions,
    document: &mut BudgetedTextDocument,
) -> Result<()> {
    let view = model
        .task(index)
        .ok_or_else(invalid_task_section_occurrence)?;
    let task = view.semantic;
    let prefix = format!(
        "T{} {}{}{} ",
        index + 1,
        if task.done { "D" } else { " " },
        if task.active { "A" } else { " " },
        if task.crit { "!" } else { " " },
    );
    let resources = document.resources_mut();
    let safe_label = !terminal_single_line_text_requires_normalization(&task.task, resources)?
        && !task.task.contains('|');
    let label = if safe_label
        && display_width_with_profile(&task.task, options.terminal_width_profile)
            <= LABEL_WIDTH.saturating_sub(prefix.len())
    {
        task.task.as_str()
    } else {
        ""
    };
    let label_width = display_width_with_profile(label, options.terminal_width_profile);
    let start = axis.position(i128::from(task.start_ms) * 2);
    let end = axis.position(i128::from(view.render_end_ms.unwrap_or(task.end_ms)) * 2);
    let milestone = axis.position(i128::from(task.start_ms) + i128::from(task.end_ms));
    let horizontal = match options.structural_charset() {
        AsciiCharset::Ascii => "=",
        AsciiCharset::Unicode => "─",
    };
    document.push_line_with(|line| {
        line.push_str(&prefix)?;
        line.push_str(label)?;
        repeat(line, " ", LABEL_WIDTH - prefix.len() - label_width)?;
        line.push_str(" | ")?;
        for column in 0..axis.cells {
            let symbol = if task.milestone {
                if column == milestone { "o" } else { " " }
            } else if start == end && column == start {
                "*"
            } else if column == start {
                "["
            } else if column == end {
                "]"
            } else if column > start && column < end {
                horizontal
            } else {
                " "
            };
            line.push_str(symbol)?;
        }
        line.push_str("|")
    })
}

fn repeat(line: &mut BudgetedTextLine<'_>, symbol: &str, count: usize) -> Result<()> {
    for _ in 0..count {
        line.push_str(symbol)?;
    }
    Ok(())
}

fn wrapped(document: &mut BudgetedTextDocument, width: usize, text: &str) -> Result<()> {
    document.push_wrapped_prefixed_line_with("", "  ", width, |line| line.push_str(text))
}

fn format_axis_endpoint(ms: i64, local_time_zone: &LocalTimeZone) -> String {
    local_time_zone
        .at_instant(ms)
        .map(format_axis_local)
        .unwrap_or_else(|| format!("{ms} ms since Unix epoch"))
}

fn format_axis_local(local: merman_model::time::OffsetDateTime) -> String {
    // The model's generic offset Display uses minute precision. Historical resolver rules
    // can contain seconds, so retain them here without changing other output families.
    let mut text = format_resolved_date(local.timestamp_millis(), local, Some(local));
    let seconds = local.offset().seconds();
    let absolute = seconds.unsigned_abs();
    text.push_str(&format!(
        " {}{:02}:{:02}",
        if seconds < 0 { '-' } else { '+' },
        absolute / 3600,
        (absolute % 3600) / 60,
    ));
    if !absolute.is_multiple_of(60) {
        text.push_str(&format!(":{:02}", absolute % 60));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::time::{CivilDate, CivilDateTime, OffsetDateTime, UtcOffset};

    #[test]
    fn historical_axis_offsets_retain_seconds_and_minute_offsets_keep_their_spelling() {
        let date = CivilDateTime::new(CivilDate::new(1880, 1, 1).unwrap(), 0, 0, 0, 0).unwrap();
        for (seconds, expected) in [
            (-17_762, "1880-01-01 -04:56:02"),
            (17_762, "1880-01-01 +04:56:02"),
            (-2, "1880-01-01 -00:00:02"),
            (28_800, "1880-01-01 +08:00"),
            (0, "1880-01-01 +00:00"),
        ] {
            let offset = UtcOffset::from_seconds(seconds).unwrap();
            let local = OffsetDateTime::from_local(date, offset).unwrap();
            assert_eq!(format_axis_local(local), expected);
            let utc_ms = OffsetDateTime::from_local(date, UtcOffset::UTC)
                .unwrap()
                .timestamp_millis();
            assert_eq!(local.timestamp_millis(), utc_ms - i64::from(seconds) * 1000);
        }
    }
}
