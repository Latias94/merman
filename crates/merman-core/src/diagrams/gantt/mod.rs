use crate::{Error, ParseMetadata, Result, utils};
use chrono::{Datelike, Duration, FixedOffset, NaiveDate, NaiveDateTime, TimeZone, Timelike};
use serde_json::{Value, json};
use std::collections::HashMap;

type DateTimeFixed = chrono::DateTime<FixedOffset>;

mod date;
mod datetime;
mod model;
mod parse;

use date::*;
use datetime::*;
use model::*;

pub use model::{GanttDiagramRenderModel, GanttRenderTask};
pub use parse::{parse_gantt, parse_gantt_editor_facts, parse_gantt_model_for_render};

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
        return Err(Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: "invalid excludes: excludes every weekday".to_string(),
        });
    }
    Ok(())
}

fn is_invalid_date(db: &GanttDb, date: DateTimeFixed, date_format: &str) -> bool {
    let formatted = format_dayjs_like(date, date_format);
    let date_only = format_dayjs_like(date, "YYYY-MM-DD");

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
        let local = crate::runtime::datetime_to_local_fixed(date);
        let iso = local.weekday().number_from_monday(); // 1..=7
        if iso == weekend_start || iso == weekend_start + 1 {
            return true;
        }
    }

    let weekday =
        weekday_full_name(crate::runtime::datetime_to_local_fixed(date).weekday()).to_lowercase();
    if db.excludes.iter().any(|v| v == &weekday) {
        return true;
    }

    db.excludes
        .iter()
        .any(|v| v == &formatted || v == &date_only)
}

fn fix_task_dates(
    db: &GanttDb,
    mut start_time: DateTimeFixed,
    mut end_time: DateTimeFixed,
    date_format: &str,
) -> Result<(DateTimeFixed, Option<DateTimeFixed>)> {
    validate_excludes_leave_a_working_weekday(db)?;

    let mut invalid = false;
    let mut render_end_time: Option<DateTimeFixed> = None;
    let mut consecutive_invalid_days = 0usize;
    while start_time <= end_time {
        if !invalid {
            render_end_time = Some(end_time);
        }
        invalid = is_invalid_date(db, start_time, date_format);
        if invalid {
            consecutive_invalid_days += 1;
            if consecutive_invalid_days > MAX_CONSECUTIVE_EXCLUDED_DAYS {
                return Err(Error::DiagramParse {
                    diagram_type: "gantt".to_string(),
                    message: "invalid excludes: no includable date found within one year"
                        .to_string(),
                });
            }
            end_time = add_days_local(end_time, 1).ok_or_else(|| Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: "invalid excludes: adjusted task end date is out of range".to_string(),
            })?;
        } else {
            consecutive_invalid_days = 0;
        }
        start_time = add_days_local(start_time, 1).ok_or_else(|| Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: "invalid excludes: adjusted task start date is out of range".to_string(),
        })?;
    }
    Ok((end_time, render_end_time))
}

#[cfg(test)]
mod tests;
