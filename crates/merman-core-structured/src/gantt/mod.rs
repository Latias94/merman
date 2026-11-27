//! Gantt statement parsing, task resolution, and canonical model construction.

use std::collections::HashMap;

#[cfg(feature = "presentation")]
use merman_model::families::gantt::{GanttDiagramRenderModel, GanttOutputData, GanttRenderTask};
use merman_model::{
    SourceSpan,
    families::gantt::{
        GanttClickEvent, GanttDiagramModel, GanttTask, GanttTaskEnd, GanttTaskEndConstraint,
        GanttTaskRaw, GanttTaskStart, GanttTaskStartConstraint,
    },
    runtime::OperationContext,
    time::{CivilDate, CivilDateTime, OffsetDateTime, UtcOffset},
};

mod date;
mod datetime;
#[cfg(feature = "editor-facts")]
mod facts;
mod model;
mod parse;
#[cfg(test)]
mod tests;

use date::*;
use datetime::*;
use model::*;

#[cfg(feature = "editor-facts")]
pub use facts::{GanttExpectedSyntax, GanttObservation, GanttSymbolKind};
#[cfg(feature = "editor-facts")]
pub use parse::parse_gantt_syntax_observed_controlled;
pub use parse::{GanttSource, GanttSyntax, parse_gantt_syntax_controlled};
#[cfg(any(test, feature = "test-support"))]
pub use parse::{gantt_syntax_construction_count, reset_gantt_syntax_construction_count};

/// A family error in preprocessed source coordinates.
#[derive(Debug)]
pub struct GanttSourceIssue {
    pub message: String,
    pub location: GanttIssueLocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GanttIssueLocation {
    Fallback,
    Exact(SourceSpan),
    InsertionPoint(usize),
}

type Result<T> = std::result::Result<T, GanttSourceIssue>;

impl GanttSourceIssue {
    fn fallback(message: String) -> Self {
        Self {
            message,
            location: GanttIssueLocation::Fallback,
        }
    }

    fn exact(message: String, span: SourceSpan) -> Self {
        Self {
            message,
            location: GanttIssueLocation::Exact(span),
        }
    }

    fn insertion_point(message: impl Into<String>, offset: usize) -> Self {
        Self {
            message: message.into(),
            location: GanttIssueLocation::InsertionPoint(offset),
        }
    }
}

const ALL_WEEKDAYS_MASK: u8 = 0b0111_1111;
const MAX_CONSECUTIVE_EXCLUDED_DAYS: usize = 366;

fn weekday_mask_monday_based(iso_weekday: u32) -> u8 {
    1u8 << (iso_weekday.saturating_sub(1) as u8)
}

fn excluded_weekdays_mask(excludes: &[String], weekend: &str) -> u8 {
    let mut mask = 0u8;

    if excludes.iter().any(|v| v == "weekends") {
        let weekend_start = match weekend {
            "friday" => 5u32,
            _ => 6u32,
        };
        mask |= weekday_mask_monday_based(weekend_start);
        mask |= weekday_mask_monday_based(weekend_start + 1);
    }

    for value in excludes {
        let weekday = match value.as_str() {
            "monday" => Some(1),
            "tuesday" => Some(2),
            "wednesday" => Some(3),
            "thursday" => Some(4),
            "friday" => Some(5),
            "saturday" => Some(6),
            "sunday" => Some(7),
            _ => None,
        };
        if let Some(weekday) = weekday {
            mask |= weekday_mask_monday_based(weekday);
        }
    }

    mask
}

fn validate_excludes_leave_a_working_weekday(db: &GanttDb) -> Result<()> {
    if db.includes.is_empty()
        && excluded_weekdays_mask(&db.excludes, &db.weekend) == ALL_WEEKDAYS_MASK
    {
        return Err(GanttSourceIssue::fallback(
            "invalid excludes: excludes every weekday".to_string(),
        ));
    }
    Ok(())
}

fn is_invalid_date(
    db: &GanttDb,
    date: OffsetDateTime,
    date_format: &str,
    runtime: &OperationContext,
) -> bool {
    let formatted = format_dayjs_like(date, date_format, runtime);
    let date_only = format_dayjs_like(date, "YYYY-MM-DD", runtime);

    if db
        .includes
        .iter()
        .any(|v| v == &formatted || v == &date_only)
    {
        return false;
    }

    if db.excludes.iter().any(|v| v == "weekends") {
        let weekend_start = match db.weekend.as_str() {
            "friday" => 5u32,
            _ => 6u32,
        };
        let local = datetime_to_local_civil(date, runtime);
        let iso = local.weekday().number_from_monday(); // 1..=7
        if iso == weekend_start || iso == weekend_start + 1 {
            return true;
        }
    }

    let weekday = datetime_to_local_civil(date, runtime)
        .weekday()
        .full_name()
        .to_lowercase();
    if db.excludes.iter().any(|v| v == &weekday) {
        return true;
    }

    db.excludes
        .iter()
        .any(|v| v == &formatted || v == &date_only)
}

fn fix_task_dates(
    db: &GanttDb,
    mut start_time: OffsetDateTime,
    mut end_time: OffsetDateTime,
    date_format: &str,
    runtime: &OperationContext,
) -> Result<(OffsetDateTime, Option<OffsetDateTime>)> {
    if start_time > end_time {
        return Ok((end_time, None));
    }
    validate_excludes_leave_a_working_weekday(db)?;

    let mut invalid = false;
    let mut render_end_time: Option<OffsetDateTime> = None;
    let mut consecutive_invalid_days = 0usize;
    while start_time <= end_time {
        if !invalid {
            render_end_time = Some(end_time);
        }
        invalid = is_invalid_date(db, start_time, date_format, runtime);
        if invalid {
            consecutive_invalid_days += 1;
            if consecutive_invalid_days > MAX_CONSECUTIVE_EXCLUDED_DAYS {
                return Err(GanttSourceIssue::fallback(
                    "invalid excludes: no includable date found within one year".to_string(),
                ));
            }
            end_time = add_days_local(end_time, 1, runtime).ok_or_else(|| {
                GanttSourceIssue::fallback(
                    "invalid excludes: adjusted task end date is out of range".to_string(),
                )
            })?;
        } else {
            consecutive_invalid_days = 0;
        }
        start_time = add_days_local(start_time, 1, runtime).ok_or_else(|| {
            GanttSourceIssue::fallback(
                "invalid excludes: adjusted task start date is out of range".to_string(),
            )
        })?;
    }
    Ok((end_time, render_end_time))
}
