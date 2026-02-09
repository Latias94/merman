use crate::{Error, ParseMetadata, Result, utils};
use chrono::{
    Datelike, Duration, FixedOffset, Local, NaiveDate, NaiveDateTime, TimeZone, Timelike,
};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashMap;

type DateTimeFixed = chrono::DateTime<FixedOffset>;

#[derive(Debug, Clone)]
enum StartTimeRaw {
    PrevTaskEnd,
    GetStartDate { start_data: String },
}

#[derive(Debug, Clone)]
struct RawTaskRaw {
    data: String,
    start_time: StartTimeRaw,
    end_data: String,
}

#[derive(Debug, Clone)]
struct RawTask {
    section: String,
    type_: String,
    processed: bool,
    manual_end_time: bool,
    render_end_time: Option<DateTimeFixed>,
    raw: RawTaskRaw,
    task: String,
    classes: Vec<String>,
    id: String,
    prev_task_id: Option<String>,
    active: bool,
    done: bool,
    crit: bool,
    milestone: bool,
    vert: bool,
    order: i64,
    start_time: Option<DateTimeFixed>,
    end_time: Option<DateTimeFixed>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ClickEvent {
    function_name: String,
    function_args: Vec<String>,
    raw_function_args: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct GanttDb {
    acc_title: String,
    acc_descr: String,
    diagram_title: String,

    date_format: String,
    axis_format: String,
    tick_interval: Option<String>,
    today_marker: String,
    includes: Vec<String>,
    excludes: Vec<String>,
    links: HashMap<String, String>,
    click_events: HashMap<String, ClickEvent>,

    sections: Vec<String>,
    current_section: String,
    display_mode: String,

    inclusive_end_dates: bool,
    top_axis: bool,
    weekday: String,
    weekend: String,

    raw_tasks: Vec<RawTask>,
    task_index: HashMap<String, usize>,
    task_cnt: i64,
    last_task_id: Option<String>,
    last_order: i64,

    security_level: String,
}

impl GanttDb {
    fn clear(&mut self) {
        *self = Self::default();
        self.weekday = "sunday".to_string();
        self.weekend = "saturday".to_string();
    }

    fn set_security_level(&mut self, level: Option<&str>) {
        self.security_level = level.unwrap_or("strict").to_string();
    }

    fn set_date_format(&mut self, txt: &str) {
        self.date_format = txt.to_string();
    }

    fn enable_inclusive_end_dates(&mut self) {
        self.inclusive_end_dates = true;
    }

    fn enable_top_axis(&mut self) {
        self.top_axis = true;
    }

    fn set_axis_format(&mut self, txt: &str) {
        self.axis_format = txt.to_string();
    }

    fn set_tick_interval(&mut self, txt: &str) {
        self.tick_interval = Some(txt.to_string());
    }

    fn set_today_marker(&mut self, txt: &str) {
        self.today_marker = txt.to_string();
    }

    fn set_includes(&mut self, txt: &str) {
        self.includes = split_list_lower(txt);
    }

    fn set_excludes(&mut self, txt: &str) {
        self.excludes = split_list_lower(txt);
    }

    fn set_weekday(&mut self, txt: &str) {
        self.weekday = txt.to_string();
    }

    fn set_weekend(&mut self, txt: &str) {
        self.weekend = txt.to_string();
    }

    fn set_diagram_title(&mut self, txt: &str) {
        self.diagram_title = txt.to_string();
    }

    fn set_display_mode(&mut self, txt: &str) {
        self.display_mode = txt.to_string();
    }

    fn set_acc_title(&mut self, txt: &str) {
        self.acc_title = txt.to_string();
    }

    fn set_acc_descr(&mut self, txt: &str) {
        self.acc_descr = txt.to_string();
    }

    fn add_section(&mut self, txt: &str) {
        self.current_section = txt.to_string();
        self.sections.push(txt.to_string());
    }

    fn find_task_by_id(&self, id: &str) -> Option<&RawTask> {
        // Mermaid's upstream ganttDb uses a plain JS object (`taskDb`) for id → index mapping,
        // which makes `__proto__` non-addressable via `taskDb[id]` (prototype mutation). Mirror
        // that observable behavior for parity.
        if id == "__proto__" {
            return None;
        }
        let pos = self.task_index.get(id).copied()?;
        self.raw_tasks.get(pos)
    }

    fn find_task_by_id_mut(&mut self, id: &str) -> Option<&mut RawTask> {
        if id == "__proto__" {
            return None;
        }
        let pos = self.task_index.get(id).copied()?;
        self.raw_tasks.get_mut(pos)
    }

    fn set_class(&mut self, ids: &str, class_name: &str) {
        for id in ids.split(',') {
            let id = id.trim();
            let Some(task) = self.find_task_by_id_mut(id) else {
                continue;
            };
            task.classes.push(class_name.to_string());
        }
    }

    fn set_link(&mut self, ids: &str, link_str: &str) {
        let mut link_str = link_str.to_string();
        if self.security_level != "loose" {
            link_str = utils::sanitize_url(&link_str);
        }
        for id in ids.split(',') {
            let id = id.trim();
            if self.find_task_by_id(id).is_some() {
                self.links.insert(id.to_string(), link_str.clone());
            }
        }
        self.set_class(ids, "clickable");
    }

    fn set_click_event(&mut self, ids: &str, function_name: &str, function_args: Option<&str>) {
        if self.security_level == "loose" {
            for id in ids.split(',') {
                let id = id.trim();
                if self.find_task_by_id(id).is_some() {
                    let args = parse_callback_args(function_args).unwrap_or_default();
                    let args = if args.is_empty() {
                        vec![id.to_string()]
                    } else {
                        args
                    };
                    self.click_events.insert(
                        id.to_string(),
                        ClickEvent {
                            function_name: function_name.to_string(),
                            function_args: args,
                            raw_function_args: function_args.map(|s| s.to_string()),
                        },
                    );
                }
            }
        }
        self.set_class(ids, "clickable");
    }

    fn add_task(&mut self, descr: &str, data: &str) {
        let prev_task_id = self.last_task_id.clone();
        let task_info = parse_task_data(&mut self.task_cnt, data);

        let raw_task = RawTask {
            section: self.current_section.clone(),
            type_: self.current_section.clone(),
            processed: false,
            manual_end_time: false,
            render_end_time: None,
            raw: RawTaskRaw {
                data: data.to_string(),
                start_time: task_info.start_time,
                end_data: task_info.end_data,
            },
            task: descr.to_string(),
            classes: Vec::new(),
            id: task_info.id.clone(),
            prev_task_id,
            active: task_info.active,
            done: task_info.done,
            crit: task_info.crit,
            milestone: task_info.milestone,
            vert: task_info.vert,
            order: self.last_order,
            start_time: None,
            end_time: None,
        };

        self.last_order += 1;
        let pos = self.raw_tasks.len();
        self.raw_tasks.push(raw_task);
        self.last_task_id = Some(task_info.id.clone());
        self.task_index.insert(task_info.id, pos);
    }

    fn compile_tasks(&mut self) -> Result<bool> {
        let mut all_processed = true;
        for i in 0..self.raw_tasks.len() {
            let processed = self.compile_task(i)?;
            all_processed = all_processed && processed;
        }
        Ok(all_processed)
    }

    fn compile_task(&mut self, pos: usize) -> Result<bool> {
        let start_spec = self.raw_tasks.get(pos).map(|t| t.raw.start_time.clone());
        let Some(start_spec) = start_spec else {
            return Ok(false);
        };

        match start_spec {
            StartTimeRaw::PrevTaskEnd => {
                let prev_id = self.raw_tasks[pos].prev_task_id.clone();
                if let Some(prev_id) = prev_id {
                    if let Some(prev_task) = self.find_task_by_id(&prev_id) {
                        self.raw_tasks[pos].start_time = prev_task.end_time;
                    }
                }
            }
            StartTimeRaw::GetStartDate { start_data } => {
                let start_time = get_start_date(self, &self.date_format, &start_data)?;
                if let Some(start_time) = start_time {
                    self.raw_tasks[pos].start_time = Some(start_time);
                }
            }
        }

        let Some(start_time) = self.raw_tasks[pos].start_time else {
            return Ok(false);
        };

        let end_data = self.raw_tasks[pos].raw.end_data.clone();
        let end_time = get_end_date(
            self,
            start_time,
            &self.date_format,
            &end_data,
            self.inclusive_end_dates,
        )?;
        self.raw_tasks[pos].end_time = end_time;
        self.raw_tasks[pos].processed = self.raw_tasks[pos].end_time.is_some();

        if self.raw_tasks[pos].processed {
            self.raw_tasks[pos].manual_end_time = is_strict_yyyy_mm_dd(&end_data);
            self.check_task_dates(pos)?;
        }

        Ok(self.raw_tasks[pos].processed)
    }

    fn check_task_dates(&mut self, pos: usize) -> Result<()> {
        if self.excludes.is_empty() || self.raw_tasks[pos].manual_end_time {
            return Ok(());
        }
        let Some(start_time) = self.raw_tasks[pos].start_time else {
            return Ok(());
        };
        let Some(end_time) = self.raw_tasks[pos].end_time else {
            return Ok(());
        };

        let Some(start_time) = add_days_local(start_time, 1) else {
            return Ok(());
        };
        let (fixed_end_time, render_end_time) =
            fix_task_dates(self, start_time, end_time, &self.date_format)?;
        self.raw_tasks[pos].end_time = Some(fixed_end_time);
        self.raw_tasks[pos].render_end_time = render_end_time;

        Ok(())
    }

    fn get_tasks(&mut self) -> Result<Vec<RawTask>> {
        let mut all = self.compile_tasks()?;
        let max_depth = 10;
        let mut iters = 0;
        while !all && iters < max_depth {
            all = self.compile_tasks()?;
            iters += 1;
        }
        Ok(self.raw_tasks.clone())
    }
}

#[derive(Debug, Clone)]
struct TaskInfo {
    id: String,
    start_time: StartTimeRaw,
    end_data: String,
    active: bool,
    done: bool,
    crit: bool,
    milestone: bool,
    vert: bool,
}

fn split_list_lower(txt: &str) -> Vec<String> {
    txt.to_lowercase()
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn parse_task_data(task_cnt: &mut i64, data_str: &str) -> TaskInfo {
    let ds = data_str.strip_prefix(':').unwrap_or(data_str);
    let mut data: Vec<String> = ds.split(',').map(|s| s.to_string()).collect();

    let mut active = false;
    let mut done = false;
    let mut crit = false;
    let mut milestone = false;
    let mut vert = false;

    let tags = ["active", "done", "crit", "milestone", "vert"];
    let mut match_found = true;
    while match_found && !data.is_empty() {
        match_found = false;
        for tag in tags {
            if data.first().is_some_and(|s| s.trim() == tag) {
                match tag {
                    "active" => active = true,
                    "done" => done = true,
                    "crit" => crit = true,
                    "milestone" => milestone = true,
                    "vert" => vert = true,
                    _ => {}
                }
                data.remove(0);
                match_found = true;
                break;
            }
        }
    }

    for d in &mut data {
        *d = d.trim().to_string();
    }

    let mut next_id = |id_str: Option<&str>| -> String {
        match id_str {
            Some(s) => s.to_string(),
            None => {
                *task_cnt += 1;
                format!("task{}", *task_cnt)
            }
        }
    };

    match data.len() {
        1 => TaskInfo {
            id: next_id(None),
            start_time: StartTimeRaw::PrevTaskEnd,
            end_data: data[0].clone(),
            active,
            done,
            crit,
            milestone,
            vert,
        },
        2 => TaskInfo {
            id: next_id(None),
            start_time: StartTimeRaw::GetStartDate {
                start_data: data[0].clone(),
            },
            end_data: data[1].clone(),
            active,
            done,
            crit,
            milestone,
            vert,
        },
        3 => TaskInfo {
            id: next_id(Some(&data[0])),
            start_time: StartTimeRaw::GetStartDate {
                start_data: data[1].clone(),
            },
            end_data: data[2].clone(),
            active,
            done,
            crit,
            milestone,
            vert,
        },
        _ => TaskInfo {
            id: next_id(None),
            start_time: StartTimeRaw::PrevTaskEnd,
            end_data: String::new(),
            active,
            done,
            crit,
            milestone,
            vert,
        },
    }
}

fn today_midnight_local() -> DateTimeFixed {
    let now = Local::now();
    let date = now.date_naive();
    let naive = date.and_hms_opt(0, 0, 0).unwrap_or_else(|| {
        NaiveDate::from_ymd_opt(1970, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
    });
    local_from_naive(naive)
}

fn local_from_naive(naive: NaiveDateTime) -> DateTimeFixed {
    match Local.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => dt.fixed_offset(),
        chrono::LocalResult::Ambiguous(a, _b) => a.fixed_offset(),
        chrono::LocalResult::None => chrono::DateTime::<FixedOffset>::from_naive_utc_and_offset(
            naive,
            FixedOffset::east_opt(0).unwrap(),
        ),
    }
}

fn add_days_local(dt: DateTimeFixed, days: i64) -> Option<DateTimeFixed> {
    let local = dt.with_timezone(&Local);
    let naive = local.naive_local();
    let date = naive.date();
    let time = naive.time();

    let new_date = if days >= 0 {
        date.checked_add_days(chrono::Days::new(days as u64))?
    } else {
        date.checked_sub_days(chrono::Days::new((-days) as u64))?
    };
    Some(local_from_naive(NaiveDateTime::new(new_date, time)))
}

fn add_months_local(dt: DateTimeFixed, months: i64) -> Option<DateTimeFixed> {
    let local = dt.with_timezone(&Local);
    let naive = local.naive_local();
    let mut year = naive.year();
    let mut month0 = naive.month0() as i64; // 0..=11

    month0 += months;
    year += month0.div_euclid(12) as i32;
    month0 = month0.rem_euclid(12);

    let month = (month0 as u32) + 1;
    let day = naive.day().min(last_day_of_month(year, month));
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    Some(local_from_naive(NaiveDateTime::new(date, naive.time())))
}

fn add_years_local(dt: DateTimeFixed, years: i64) -> Option<DateTimeFixed> {
    let local = dt.with_timezone(&Local);
    let naive = local.naive_local();
    let year = naive.year().checked_add(years as i32)?;
    let month = naive.month();
    let day = naive.day().min(last_day_of_month(year, month));
    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    Some(local_from_naive(NaiveDateTime::new(date, naive.time())))
}

fn last_day_of_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_next = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    let last = first_next
        .pred_opt()
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap());
    last.day()
}

#[derive(Debug, Clone)]
enum DayjsFormatItem {
    Literal(String),
    Token(DayjsToken),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DayjsToken {
    Year4,
    Year2,
    Month2,
    Month1,
    MonthNameShort,
    MonthNameLong,
    Day2,
    Day1,
    DayOrdinal,
    Hour24_2,
    Hour24_1,
    Hour12_2,
    Hour12_1,
    Minute2,
    Minute1,
    Second2,
    Second1,
    Millis3,
    Millis2,
    Millis1,
    OffsetColon,
    OffsetNoColon,
    AmPmUpper,
    AmPmLower,
    UnixMs,
    UnixSec,
    WeekdayLong,
    WeekdayShort,
}

fn tokenize_dayjs_format(fmt: &str) -> Vec<DayjsFormatItem> {
    let mut out: Vec<DayjsFormatItem> = Vec::new();

    fn push_lit(out: &mut Vec<DayjsFormatItem>, s: &str) {
        if s.is_empty() {
            return;
        }
        match out.last_mut() {
            Some(DayjsFormatItem::Literal(prev)) => prev.push_str(s),
            _ => out.push(DayjsFormatItem::Literal(s.to_string())),
        }
    }

    let tokens: &[(&str, DayjsToken)] = &[
        ("YYYY", DayjsToken::Year4),
        ("MMMM", DayjsToken::MonthNameLong),
        ("MMM", DayjsToken::MonthNameShort),
        ("MM", DayjsToken::Month2),
        ("M", DayjsToken::Month1),
        ("Do", DayjsToken::DayOrdinal),
        ("DD", DayjsToken::Day2),
        ("D", DayjsToken::Day1),
        ("HH", DayjsToken::Hour24_2),
        ("H", DayjsToken::Hour24_1),
        ("hh", DayjsToken::Hour12_2),
        ("h", DayjsToken::Hour12_1),
        ("mm", DayjsToken::Minute2),
        ("m", DayjsToken::Minute1),
        ("ss", DayjsToken::Second2),
        ("s", DayjsToken::Second1),
        ("SSS", DayjsToken::Millis3),
        ("SS", DayjsToken::Millis2),
        ("S", DayjsToken::Millis1),
        ("ZZ", DayjsToken::OffsetNoColon),
        ("Z", DayjsToken::OffsetColon),
        ("A", DayjsToken::AmPmUpper),
        ("a", DayjsToken::AmPmLower),
        ("x", DayjsToken::UnixMs),
        ("X", DayjsToken::UnixSec),
        ("dddd", DayjsToken::WeekdayLong),
        ("ddd", DayjsToken::WeekdayShort),
        ("YY", DayjsToken::Year2),
    ];

    let bytes = fmt.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            if let Some(end_rel) = fmt[i + 1..].find(']') {
                let inside = &fmt[i + 1..i + 1 + end_rel];
                push_lit(&mut out, inside);
                i = i + 1 + end_rel + 1;
                continue;
            }
        }

        let rest = &fmt[i..];
        let mut matched: Option<(&str, DayjsToken)> = None;
        for (pat, tok) in tokens {
            if rest.starts_with(pat) {
                matched = Some((*pat, *tok));
                break;
            }
        }
        if let Some((pat, tok)) = matched {
            out.push(DayjsFormatItem::Token(tok));
            i += pat.len();
        } else {
            let ch = rest.chars().next().unwrap();
            push_lit(&mut out, &ch.to_string());
            i += ch.len_utf8();
        }
    }

    out
}

#[derive(Debug, Clone, Default)]
struct DayjsParsedParts {
    year: Option<i32>,
    month: Option<u32>,
    day: Option<u32>,
    hour24: Option<u32>,
    hour12: Option<u32>,
    minute: Option<u32>,
    second: Option<u32>,
    millis: Option<u32>,
    ampm_is_pm: Option<bool>,
    offset_minutes: Option<i32>,
    unix_ms: Option<i64>,
}

fn parse_dayjs_like_strict(date_format: &str, s: &str) -> Option<DateTimeFixed> {
    let fmt = date_format.trim();
    if fmt.is_empty() {
        return None;
    }

    let items = tokenize_dayjs_format(fmt);

    fn parse_signed_i64_prefix(input: &str) -> Option<(i64, &str)> {
        let bytes = input.as_bytes();
        if bytes.is_empty() {
            return None;
        }
        let mut i = 0usize;
        let sign: i64 = if bytes[0] == b'-' {
            i = 1;
            -1
        } else {
            1
        };
        let start_digits = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == start_digits {
            return None;
        }
        let v: i64 = input[start_digits..i].parse().ok()?;
        Some((sign.saturating_mul(v), &input[i..]))
    }

    fn parse_int_exact(s: &str, digits: usize) -> Option<(u32, &str)> {
        if s.len() < digits {
            return None;
        }
        let (head, tail) = s.split_at(digits);
        if !head.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let v = head.parse().ok()?;
        Some((v, tail))
    }

    fn parse_int_var(s: &str, min: usize, max: usize) -> Vec<(u32, &str)> {
        let mut out = Vec::new();
        for digits in (min..=max).rev() {
            if let Some((v, tail)) = parse_int_exact(s, digits) {
                out.push((v, tail));
            }
        }
        out
    }

    fn parse_offset(s: &str, with_colon: bool) -> Option<(i32, &str)> {
        let s = s.strip_prefix(|c| c == ' ' || c == '\t').unwrap_or(s);
        if let Some(tail) = s.strip_prefix('Z') {
            return Some((0, tail));
        }
        let (sign, rest) = if let Some(tail) = s.strip_prefix('+') {
            (1i32, tail)
        } else if let Some(tail) = s.strip_prefix('-') {
            (-1i32, tail)
        } else {
            return None;
        };

        let (hh, rest) = parse_int_exact(rest, 2)?;
        let (mm, rest) = if with_colon {
            let rest = rest.strip_prefix(':')?;
            parse_int_exact(rest, 2)?
        } else {
            parse_int_exact(rest, 2)?
        };
        let hh: i32 = hh.try_into().ok()?;
        let mm: i32 = mm.try_into().ok()?;
        if hh > 23 || mm > 59 {
            return None;
        }
        Some((sign * (hh * 60 + mm), rest))
    }

    fn parse_month_name(s: &str) -> Option<(u32, &str)> {
        const MONTHS: [&str; 12] = [
            "january",
            "february",
            "march",
            "april",
            "may",
            "june",
            "july",
            "august",
            "september",
            "october",
            "november",
            "december",
        ];
        const MONTHS_SHORT: [&str; 12] = [
            "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ];

        let lower = s.to_lowercase();
        for (i, name) in MONTHS.iter().enumerate() {
            if lower.starts_with(name) {
                let tail = &s[name.len()..];
                return Some(((i as u32) + 1, tail));
            }
        }
        for (i, name) in MONTHS_SHORT.iter().enumerate() {
            if lower.starts_with(name) {
                let tail = &s[name.len()..];
                return Some(((i as u32) + 1, tail));
            }
        }
        None
    }

    fn parse_weekday_name(s: &str) -> Option<&str> {
        const DAYS: [&str; 7] = [
            "sunday",
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
        ];
        const DAYS_SHORT: [&str; 7] = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

        let lower = s.to_lowercase();
        for name in DAYS {
            if lower.starts_with(name) {
                return Some(&s[name.len()..]);
            }
        }
        for name in DAYS_SHORT {
            if lower.starts_with(name) {
                return Some(&s[name.len()..]);
            }
        }
        None
    }

    fn parse_day_ordinal(s: &str) -> Option<(u32, &str)> {
        let candidates = parse_int_var(s, 1, 2);
        for (day, tail) in candidates {
            let tail_lower = tail.to_lowercase();
            for suffix in ["st", "nd", "rd", "th"] {
                if tail_lower.starts_with(suffix) {
                    return Some((day, &tail[suffix.len()..]));
                }
            }
        }
        None
    }

    fn parse_ampm(s: &str) -> Option<(bool, &str)> {
        let lower = s.to_lowercase();
        if lower.starts_with("am") {
            return Some((false, &s[2..]));
        }
        if lower.starts_with("pm") {
            return Some((true, &s[2..]));
        }
        None
    }

    fn parse_items<'a>(
        items: &[DayjsFormatItem],
        input: &'a str,
        parts: &DayjsParsedParts,
    ) -> Option<(&'a str, DayjsParsedParts)> {
        if items.is_empty() {
            return Some((input, parts.clone()));
        }

        match &items[0] {
            DayjsFormatItem::Literal(lit) => {
                let input = input.strip_prefix(lit.as_str())?;
                parse_items(&items[1..], input, parts)
            }
            DayjsFormatItem::Token(tok) => match tok {
                DayjsToken::Year4 => {
                    let (y, rest) = parse_int_exact(input, 4)?;
                    let mut next = parts.clone();
                    next.year = Some(y as i32);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Year2 => {
                    let (y2, rest) = parse_int_exact(input, 2)?;
                    let y2 = y2 as i32;
                    let year = if y2 <= 68 { 2000 + y2 } else { 1900 + y2 };
                    let mut next = parts.clone();
                    next.year = Some(year);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Month2 => {
                    let (m, rest) = parse_int_exact(input, 2)?;
                    if !(1..=12).contains(&m) {
                        return None;
                    }
                    let mut next = parts.clone();
                    next.month = Some(m);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Month1 => {
                    for (m, rest) in parse_int_var(input, 1, 2) {
                        if !(1..=12).contains(&m) {
                            continue;
                        }
                        let mut next = parts.clone();
                        next.month = Some(m);
                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }
                    None
                }
                DayjsToken::MonthNameShort | DayjsToken::MonthNameLong => {
                    let (m, rest) = parse_month_name(input)?;
                    let mut next = parts.clone();
                    next.month = Some(m);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Day2 => {
                    let (d, rest) = parse_int_exact(input, 2)?;
                    if !(1..=31).contains(&d) {
                        return None;
                    }
                    let mut next = parts.clone();
                    next.day = Some(d);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Day1 => {
                    for (d, rest) in parse_int_var(input, 1, 2) {
                        if !(1..=31).contains(&d) {
                            continue;
                        }
                        let mut next = parts.clone();
                        next.day = Some(d);
                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }
                    None
                }
                DayjsToken::DayOrdinal => {
                    let (d, rest) = parse_day_ordinal(input)?;
                    if !(1..=31).contains(&d) {
                        return None;
                    }
                    let mut next = parts.clone();
                    next.day = Some(d);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Hour24_2 => {
                    let (h, rest) = parse_int_exact(input, 2)?;
                    if h > 23 {
                        return None;
                    }
                    let mut next = parts.clone();
                    next.hour24 = Some(h);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Hour24_1 => {
                    for (h, rest) in parse_int_var(input, 1, 2) {
                        if h > 23 {
                            continue;
                        }
                        let mut next = parts.clone();
                        next.hour24 = Some(h);
                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }
                    None
                }
                DayjsToken::Hour12_2 => {
                    let (h, rest) = parse_int_exact(input, 2)?;
                    if !(1..=12).contains(&h) {
                        return None;
                    }
                    let mut next = parts.clone();
                    next.hour12 = Some(h);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Hour12_1 => {
                    for (h, rest) in parse_int_var(input, 1, 2) {
                        if !(1..=12).contains(&h) {
                            continue;
                        }
                        let mut next = parts.clone();
                        next.hour12 = Some(h);
                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }
                    None
                }
                DayjsToken::Minute2 => {
                    let (m, rest) = parse_int_exact(input, 2)?;
                    if m > 59 {
                        return None;
                    }
                    let mut next = parts.clone();
                    next.minute = Some(m);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Minute1 => {
                    for (m, rest) in parse_int_var(input, 1, 2) {
                        if m > 59 {
                            continue;
                        }
                        let mut next = parts.clone();
                        next.minute = Some(m);
                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }
                    None
                }
                DayjsToken::Second2 => {
                    let (sec, rest) = parse_int_exact(input, 2)?;
                    if sec > 59 {
                        return None;
                    }
                    let mut next = parts.clone();
                    next.second = Some(sec);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Second1 => {
                    for (sec, rest) in parse_int_var(input, 1, 2) {
                        if sec > 59 {
                            continue;
                        }
                        let mut next = parts.clone();
                        next.second = Some(sec);
                        if let Some(r) = parse_items(&items[1..], rest, &next) {
                            return Some(r);
                        }
                    }
                    None
                }
                DayjsToken::Millis3 => {
                    let (ms, rest) = parse_int_exact(input, 3)?;
                    if ms > 999 {
                        return None;
                    }
                    let mut next = parts.clone();
                    next.millis = Some(ms);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Millis2 => {
                    let (ms, rest) = parse_int_exact(input, 2)?;
                    if ms > 99 {
                        return None;
                    }
                    let mut next = parts.clone();
                    next.millis = Some(ms * 10);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::Millis1 => {
                    let (ms, rest) = parse_int_exact(input, 1)?;
                    if ms > 9 {
                        return None;
                    }
                    let mut next = parts.clone();
                    next.millis = Some(ms * 100);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::OffsetColon => {
                    let (mins, rest) = parse_offset(input, true)?;
                    let mut next = parts.clone();
                    next.offset_minutes = Some(mins);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::OffsetNoColon => {
                    let (mins, rest) = parse_offset(input, false)?;
                    let mut next = parts.clone();
                    next.offset_minutes = Some(mins);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::AmPmUpper | DayjsToken::AmPmLower => {
                    let (is_pm, rest) = parse_ampm(input)?;
                    let mut next = parts.clone();
                    next.ampm_is_pm = Some(is_pm);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::UnixMs => {
                    let (ms, rest) = parse_signed_i64_prefix(input)?;
                    let mut next = parts.clone();
                    next.unix_ms = Some(ms);
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::UnixSec => {
                    let (sec, rest) = parse_signed_i64_prefix(input)?;
                    let mut next = parts.clone();
                    next.unix_ms = Some(sec.saturating_mul(1000));
                    parse_items(&items[1..], rest, &next)
                }
                DayjsToken::WeekdayLong | DayjsToken::WeekdayShort => {
                    let rest = parse_weekday_name(input)?;
                    parse_items(&items[1..], rest, parts)
                }
            },
        }
    }

    let parts = DayjsParsedParts::default();
    let (rest, parts) = parse_items(&items, s, &parts)?;
    if !rest.is_empty() {
        return None;
    }

    if let Some(ms) = parts.unix_ms {
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)?;
        return Some(dt.with_timezone(&FixedOffset::east_opt(0).unwrap()));
    }

    let base_date = Local::now().date_naive();

    let year = parts.year.unwrap_or(base_date.year());
    let month = parts.month.unwrap_or(base_date.month());
    let day = parts.day.unwrap_or(base_date.day());

    let mut hour = parts.hour24.unwrap_or(0);
    if parts.hour24.is_none() {
        if let Some(h12) = parts.hour12 {
            let mut h = h12 % 12;
            if parts.ampm_is_pm.unwrap_or(false) {
                h += 12;
            }
            hour = h;
        }
    }

    let minute = parts.minute.unwrap_or(0);
    let second = parts.second.unwrap_or(0);
    let millis = parts.millis.unwrap_or(0);

    let date = NaiveDate::from_ymd_opt(year, month, day)?;
    let naive = date.and_hms_milli_opt(hour, minute, second, millis)?;

    if let Some(mins) = parts.offset_minutes {
        let offset = FixedOffset::east_opt(mins * 60)?;
        offset.from_local_datetime(&naive).single()
    } else {
        Some(local_from_naive(naive))
    }
}

fn parse_js_date_fallback(s: &str) -> Result<DateTimeFixed> {
    let s = s.trim();

    if let Some(dt) = parse_js_like_ymd_datetime(s) {
        let year = dt.year();
        if !(-10000..=10000).contains(&year) {
            return Err(Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: format!("Invalid date:{s}"),
            });
        }
        return Ok(dt);
    }

    if Regex::new(r"^\d+$").unwrap().is_match(s) {
        let n: i32 = s.parse().map_err(|_| Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: format!("Invalid date:{s}"),
        })?;
        let year = if s.len() <= 2 { 2000 + n } else { n };
        if !(-10000..=10000).contains(&year) {
            return Err(Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: format!("Invalid date:{s}"),
            });
        }
        let d = NaiveDate::from_ymd_opt(year, 1, 1).ok_or_else(|| Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: format!("Invalid date:{s}"),
        })?;
        return Ok(local_from_naive(d.and_hms_opt(0, 0, 0).unwrap()));
    }

    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        let year = dt.year();
        if !(-10000..=10000).contains(&year) {
            return Err(Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: format!("Invalid date:{s}"),
            });
        }
        return Ok(dt);
    }

    Err(Error::DiagramParse {
        diagram_type: "gantt".to_string(),
        message: format!("Invalid date:{s}"),
    })
}

fn parse_js_like_ymd_datetime(s: &str) -> Option<DateTimeFixed> {
    fn parse_u32(s: &str) -> Option<u32> {
        if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        s.parse().ok()
    }

    fn split_once(s: &str, ch: char) -> Option<(&str, &str)> {
        let idx = s.find(ch)?;
        Some((&s[..idx], &s[idx + 1..]))
    }

    fn parse_timezone_offset_minutes(s: &str) -> Option<(i32, &str)> {
        let s = s.trim_start();
        if let Some(rest) = s.strip_prefix('Z') {
            return Some((0, rest));
        }
        let (sign, rest) = if let Some(rest) = s.strip_prefix('+') {
            (1i32, rest)
        } else if let Some(rest) = s.strip_prefix('-') {
            (-1i32, rest)
        } else {
            return None;
        };

        let (hh_str, rest) = rest.split_at(2.min(rest.len()));
        let hh = parse_u32(hh_str)? as i32;

        let (mm, rest) = if let Some(rest) = rest.strip_prefix(':') {
            let (mm_str, rest) = rest.split_at(2.min(rest.len()));
            (parse_u32(mm_str)? as i32, rest)
        } else {
            let (mm_str, rest) = rest.split_at(2.min(rest.len()));
            (parse_u32(mm_str)? as i32, rest)
        };

        if hh > 23 || mm > 59 {
            return None;
        }
        Some((sign * (hh * 60 + mm), rest))
    }

    fn js_year_len_is_iso_utc(year_str: &str) -> bool {
        year_str.len() == 4
    }

    let (date_part, mut rest) = {
        let mut end = s.len();
        for (i, c) in s.char_indices() {
            if c == 'T' || c.is_whitespace() {
                end = i;
                break;
            }
        }
        (&s[..end], &s[end..])
    };

    let sep = if date_part.contains('-') {
        '-'
    } else if date_part.contains('/') {
        '/'
    } else {
        return None;
    };

    let (year_str, rest1) = split_once(date_part, sep)?;
    let (month_str, day_str) = split_once(rest1, sep)?;
    if year_str.is_empty() || year_str.len() > 4 {
        return None;
    }
    if !year_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let year: i32 = year_str.parse().ok()?;
    let month = parse_u32(month_str)?;
    let day = parse_u32(day_str)?;
    let date = NaiveDate::from_ymd_opt(year, month, day)?;

    let mut second: u32 = 0;
    let mut millis: u32 = 0;
    let mut tz_minutes: Option<i32> = None;

    rest = rest.trim_start();
    if rest.is_empty() {
        let naive = date.and_hms_milli_opt(0, 0, 0, 0)?;
        if sep == '-' && js_year_len_is_iso_utc(year_str) {
            let dt_utc =
                chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(naive, chrono::Utc);
            return Some(dt_utc.with_timezone(&FixedOffset::east_opt(0)?));
        }
        return Some(local_from_naive(naive));
    }

    if let Some(r) = rest.strip_prefix('T') {
        rest = r;
    }
    rest = rest.trim_start();

    let (hh_str, rest2) = split_once(rest, ':')?;
    let hour = parse_u32(hh_str)?;
    let (mm_str, mut rest3) = {
        let (mm_str, rest) = rest2.split_at(2.min(rest2.len()));
        (mm_str, rest)
    };
    let minute = parse_u32(mm_str)?;

    if let Some(r) = rest3.strip_prefix(':') {
        let (ss_str, mut rest4) = {
            let (ss_str, rest) = r.split_at(2.min(r.len()));
            (ss_str, rest)
        };
        second = parse_u32(ss_str)?;

        if let Some(r) = rest4.strip_prefix('.') {
            let ms_digits: String = r
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .take(3)
                .collect();
            if ms_digits.is_empty() {
                return None;
            }
            millis = match ms_digits.len() {
                1 => parse_u32(&ms_digits)? * 100,
                2 => parse_u32(&ms_digits)? * 10,
                _ => parse_u32(&ms_digits)?,
            };
            rest4 = &r[ms_digits.len()..];
        }

        rest3 = rest4;
    }

    rest3 = rest3.trim_start();
    if !rest3.is_empty() {
        if let Some((mins, tail)) = parse_timezone_offset_minutes(rest3) {
            tz_minutes = Some(mins);
            if !tail.trim().is_empty() {
                return None;
            }
        } else {
            return None;
        }
    }

    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let naive = date.and_hms_milli_opt(hour, minute, second, millis)?;

    if let Some(mins) = tz_minutes {
        let offset = FixedOffset::east_opt(mins * 60)?;
        return offset.from_local_datetime(&naive).single();
    }

    Some(local_from_naive(naive))
}

fn get_start_date(db: &GanttDb, date_format: &str, raw: &str) -> Result<Option<DateTimeFixed>> {
    let s = raw.trim();

    let after_re = Regex::new(r"(?i)^after\s+(?<ids>[\d\w -]+)").unwrap();
    if let Some(caps) = after_re.captures(s) {
        let ids = caps.name("ids").map(|m| m.as_str()).unwrap_or("");
        let mut latest: Option<Option<DateTimeFixed>> = None;
        for id in ids.split(' ') {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            let Some(task) = db.find_task_by_id(id) else {
                continue;
            };
            if latest.is_none() {
                latest = Some(task.end_time);
                continue;
            }
            let Some(current_best) = latest else {
                continue;
            };
            let (Some(task_end), Some(best_end)) = (task.end_time, current_best) else {
                continue;
            };
            if task_end > best_end {
                latest = Some(Some(task_end));
            }
        }
        return Ok(match latest {
            Some(end) => end,
            None => Some(today_midnight_local()),
        });
    }

    // Mermaid's ganttDb special-cases timestamp formats `x` / `X`: for positive integer strings,
    // it uses `new Date(Number(str))` rather than strict dayjs parsing. This treats the numeric
    // payload as *milliseconds* for both `x` and `X`.
    let fmt = date_format.trim();
    if (fmt == "x" || fmt == "X") && !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(ms) = s.parse::<i64>() {
            if let Some(dt) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) {
                return Ok(Some(dt.with_timezone(&FixedOffset::east_opt(0).unwrap())));
            }
        }
    }

    if let Some(dt) = parse_dayjs_like_strict(date_format, s) {
        return Ok(Some(dt));
    }

    let dt = parse_js_date_fallback(s)?;
    let year = dt.year();
    if !(-10000..=10000).contains(&year) {
        return Err(Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: format!("Invalid date:{s}"),
        });
    }
    Ok(Some(dt))
}

fn parse_duration(str_: &str) -> (f64, String) {
    let re = Regex::new(r"^(\d+(?:\.\d+)?)([Mdhmswy]|ms)$").unwrap();
    let Some(caps) = re.captures(str_.trim()) else {
        return (f64::NAN, "ms".to_string());
    };
    let value: f64 = caps.get(1).unwrap().as_str().parse().unwrap_or(f64::NAN);
    let unit = caps.get(2).unwrap().as_str().to_string();
    (value, unit)
}

fn add_duration(dt: DateTimeFixed, value: f64, unit: &str) -> Option<DateTimeFixed> {
    if !value.is_finite() {
        return None;
    }
    match unit {
        "ms" => Some(dt + Duration::milliseconds(value.trunc() as i64)),
        "s" => Some(dt + Duration::milliseconds((value * 1_000.0).trunc() as i64)),
        "m" => Some(dt + Duration::milliseconds((value * 60_000.0).trunc() as i64)),
        "h" => Some(dt + Duration::milliseconds((value * 3_600_000.0).trunc() as i64)),
        "d" => {
            if value.fract() == 0.0 {
                add_days_local(dt, value as i64)
            } else {
                Some(dt + Duration::milliseconds((value * 86_400_000.0).trunc() as i64))
            }
        }
        "w" => {
            if value.fract() == 0.0 {
                add_days_local(dt, (value as i64).saturating_mul(7))
            } else {
                Some(dt + Duration::milliseconds((value * 604_800_000.0).trunc() as i64))
            }
        }
        "M" => {
            if value.fract() == 0.0 {
                add_months_local(dt, value as i64)
            } else {
                None
            }
        }
        "y" => {
            if value.fract() == 0.0 {
                add_years_local(dt, value as i64)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn get_end_date(
    db: &GanttDb,
    prev_time: DateTimeFixed,
    date_format: &str,
    raw: &str,
    inclusive: bool,
) -> Result<Option<DateTimeFixed>> {
    let s = raw.trim();

    let until_re = Regex::new(r"(?i)^until\s+(?<ids>[\d\w -]+)").unwrap();
    if let Some(caps) = until_re.captures(s) {
        let ids = caps.name("ids").map(|m| m.as_str()).unwrap_or("");
        let mut earliest: Option<Option<DateTimeFixed>> = None;
        for id in ids.split(' ') {
            let id = id.trim();
            if id.is_empty() {
                continue;
            }
            let Some(task) = db.find_task_by_id(id) else {
                continue;
            };
            if earliest.is_none() {
                earliest = Some(task.start_time);
                continue;
            }
            let Some(current_best) = earliest else {
                continue;
            };
            let (Some(task_start), Some(best_start)) = (task.start_time, current_best) else {
                continue;
            };
            if task_start < best_start {
                earliest = Some(Some(task_start));
            }
        }
        return Ok(match earliest {
            Some(start) => start,
            None => Some(today_midnight_local()),
        });
    }

    if let Some(mut dt) = parse_dayjs_like_strict(date_format, s) {
        if inclusive {
            dt = add_days_local(dt, 1).unwrap_or(dt);
        }
        return Ok(Some(dt));
    }

    let (value, unit) = parse_duration(s);
    if value.is_finite() {
        if let Some(new_dt) = add_duration(prev_time, value, &unit) {
            return Ok(Some(new_dt));
        }
    }

    Ok(Some(prev_time))
}

fn is_strict_yyyy_mm_dd(s: &str) -> bool {
    let s = s.trim();
    if !Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap().is_match(s) {
        return false;
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
}

fn weekday_full_name(weekday: chrono::Weekday) -> &'static str {
    match weekday {
        chrono::Weekday::Mon => "Monday",
        chrono::Weekday::Tue => "Tuesday",
        chrono::Weekday::Wed => "Wednesday",
        chrono::Weekday::Thu => "Thursday",
        chrono::Weekday::Fri => "Friday",
        chrono::Weekday::Sat => "Saturday",
        chrono::Weekday::Sun => "Sunday",
    }
}

fn weekday_short_name(weekday: chrono::Weekday) -> &'static str {
    match weekday {
        chrono::Weekday::Mon => "Mon",
        chrono::Weekday::Tue => "Tue",
        chrono::Weekday::Wed => "Wed",
        chrono::Weekday::Thu => "Thu",
        chrono::Weekday::Fri => "Fri",
        chrono::Weekday::Sat => "Sat",
        chrono::Weekday::Sun => "Sun",
    }
}

fn month_short_name(month: u32) -> &'static str {
    match month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        12 => "Dec",
        _ => "",
    }
}

fn month_long_name(month: u32) -> &'static str {
    match month {
        1 => "January",
        2 => "February",
        3 => "March",
        4 => "April",
        5 => "May",
        6 => "June",
        7 => "July",
        8 => "August",
        9 => "September",
        10 => "October",
        11 => "November",
        12 => "December",
        _ => "",
    }
}

fn ordinal_suffix(n: u32) -> &'static str {
    let n_mod_100 = n % 100;
    if (11..=13).contains(&n_mod_100) {
        return "th";
    }
    match n % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}

fn format_dayjs_like(dt: DateTimeFixed, fmt: &str) -> String {
    let fmt = fmt.trim();
    if fmt.is_empty() {
        return String::new();
    }

    let items = tokenize_dayjs_format(fmt);
    let local = dt.with_timezone(&Local);
    let naive = local.naive_local();

    let mut out = String::new();
    for item in items {
        match item {
            DayjsFormatItem::Literal(s) => out.push_str(&s),
            DayjsFormatItem::Token(tok) => match tok {
                DayjsToken::Year4 => out.push_str(&format!("{:04}", naive.year())),
                DayjsToken::Year2 => {
                    out.push_str(&format!("{:02}", (naive.year().rem_euclid(100))))
                }
                DayjsToken::Month2 => out.push_str(&format!("{:02}", naive.month())),
                DayjsToken::Month1 => out.push_str(&format!("{}", naive.month())),
                DayjsToken::MonthNameShort => out.push_str(month_short_name(naive.month())),
                DayjsToken::MonthNameLong => out.push_str(month_long_name(naive.month())),
                DayjsToken::Day2 => out.push_str(&format!("{:02}", naive.day())),
                DayjsToken::Day1 => out.push_str(&format!("{}", naive.day())),
                DayjsToken::DayOrdinal => {
                    let d = naive.day();
                    out.push_str(&format!("{d}{}", ordinal_suffix(d)));
                }
                DayjsToken::Hour24_2 => out.push_str(&format!("{:02}", naive.hour())),
                DayjsToken::Hour24_1 => out.push_str(&format!("{}", naive.hour())),
                DayjsToken::Hour12_2 => {
                    let mut h = naive.hour() % 12;
                    if h == 0 {
                        h = 12;
                    }
                    out.push_str(&format!("{:02}", h));
                }
                DayjsToken::Hour12_1 => {
                    let mut h = naive.hour() % 12;
                    if h == 0 {
                        h = 12;
                    }
                    out.push_str(&format!("{}", h));
                }
                DayjsToken::Minute2 => out.push_str(&format!("{:02}", naive.minute())),
                DayjsToken::Minute1 => out.push_str(&format!("{}", naive.minute())),
                DayjsToken::Second2 => out.push_str(&format!("{:02}", naive.second())),
                DayjsToken::Second1 => out.push_str(&format!("{}", naive.second())),
                DayjsToken::Millis3 => {
                    out.push_str(&format!("{:03}", local.timestamp_subsec_millis()))
                }
                DayjsToken::Millis2 => {
                    out.push_str(&format!("{:02}", local.timestamp_subsec_millis() / 10))
                }
                DayjsToken::Millis1 => {
                    out.push_str(&format!("{}", local.timestamp_subsec_millis() / 100))
                }
                DayjsToken::OffsetColon | DayjsToken::OffsetNoColon => {
                    let secs = local.offset().local_minus_utc();
                    let sign = if secs < 0 { '-' } else { '+' };
                    let secs = secs.abs();
                    let hh = secs / 3600;
                    let mm = (secs % 3600) / 60;
                    match tok {
                        DayjsToken::OffsetColon => out.push_str(&format!("{sign}{hh:02}:{mm:02}")),
                        DayjsToken::OffsetNoColon => out.push_str(&format!("{sign}{hh:02}{mm:02}")),
                        _ => {}
                    }
                }
                DayjsToken::AmPmUpper | DayjsToken::AmPmLower => {
                    let is_pm = naive.hour() >= 12;
                    let s = if is_pm { "PM" } else { "AM" };
                    match tok {
                        DayjsToken::AmPmUpper => out.push_str(s),
                        DayjsToken::AmPmLower => out.push_str(&s.to_lowercase()),
                        _ => {}
                    }
                }
                DayjsToken::UnixMs => out.push_str(&dt.timestamp_millis().to_string()),
                DayjsToken::UnixSec => out.push_str(&(dt.timestamp_millis() / 1000).to_string()),
                DayjsToken::WeekdayLong => out.push_str(weekday_full_name(naive.weekday())),
                DayjsToken::WeekdayShort => out.push_str(weekday_short_name(naive.weekday())),
            },
        }
    }
    out
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
        let iso = date.with_timezone(&Local).weekday().number_from_monday(); // 1..=7
        if iso == weekend_start || iso == weekend_start + 1 {
            return true;
        }
    }

    let weekday = weekday_full_name(date.with_timezone(&Local).weekday()).to_lowercase();
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
    let mut invalid = false;
    let mut render_end_time: Option<DateTimeFixed> = None;
    while start_time <= end_time {
        if !invalid {
            render_end_time = Some(end_time);
        }
        invalid = is_invalid_date(db, start_time, date_format);
        if invalid {
            end_time = add_days_local(end_time, 1).unwrap_or(end_time);
        }
        start_time = add_days_local(start_time, 1).unwrap_or(start_time);
    }
    Ok((end_time, render_end_time))
}

fn strip_inline_comment(line: &str) -> &str {
    // Mermaid gantt does not treat `%%` as an inline comment delimiter for statements like `title`
    // or task lines (see `fixtures/gantt/task_inline_percent_comment.mmd`). It does, however,
    // accept full-line `%% ...` comments (and directive lines `%%{...}%%`).
    let t = line.trim_start();
    if t.starts_with("%%{") {
        return line;
    }
    if t.starts_with("%%") {
        return "";
    }
    line
}

fn split_statement_suffix(s: &str) -> &str {
    let mut end = s.len();
    for (i, c) in s.char_indices() {
        if c == '#' || c == ';' {
            end = i;
            break;
        }
    }
    &s[..end]
}

#[allow(dead_code)]
fn split_statement_suffix_semi_only(s: &str) -> &str {
    let mut end = s.len();
    for (i, c) in s.char_indices() {
        if c == ';' {
            end = i;
            break;
        }
    }
    &s[..end]
}

fn starts_with_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn parse_keyword_arg<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let t = line.trim_start();
    if !starts_with_ci(t, keyword) {
        return None;
    }
    let after = &t[keyword.len()..];
    let ws = after.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let rest = &after[ws.len_utf8()..];
    Some(split_statement_suffix(rest))
}

fn parse_keyword_arg_full_line<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let t = line.trim_start();
    if !starts_with_ci(t, keyword) {
        return None;
    }
    let after = &t[keyword.len()..];
    let ws = after.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    Some(&after[ws.len_utf8()..])
}

#[allow(dead_code)]
fn parse_keyword_arg_semi_only<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let t = line.trim_start();
    if !starts_with_ci(t, keyword) {
        return None;
    }
    let after = &t[keyword.len()..];
    let ws = after.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let rest = &after[ws.len_utf8()..];
    Some(split_statement_suffix_semi_only(rest))
}

fn parse_key_colon_value(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start();
    if !starts_with_ci(t, key) {
        return None;
    }
    let rest = t[key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?;
    // Mermaid gantt's `accTitle:` / `accDescr:` values are end-of-line tokens (not `;`/`#`-terminated).
    Some(rest.trim().to_string())
}

fn parse_acc_descr_block(lines: &mut std::str::Lines<'_>, first_line: &str) -> Option<String> {
    let t = first_line.trim_start();
    if !starts_with_ci(t, "accDescr") {
        return None;
    }
    let rest = t["accDescr".len()..].trim_start();
    let rest = rest.strip_prefix('{')?;

    let mut buf = String::new();
    if let Some(end) = rest.find('}') {
        buf.push_str(&rest[..end]);
        return Some(buf.trim().to_string());
    }
    buf.push_str(rest);
    buf.push('\n');

    for line in lines {
        if let Some(end) = line.find('}') {
            buf.push_str(&line[..end]);
            break;
        }
        buf.push_str(line);
        buf.push('\n');
    }
    Some(buf.trim().to_string())
}

fn parse_click_statement(line: &str) -> Option<ClickStatementParts> {
    let t = line.trim_start();
    if !starts_with_ci(t, "click") {
        return None;
    }
    let rest = t["click".len()..].trim_start();
    let mut parts = rest.splitn(2, char::is_whitespace);
    let ids = parts.next()?.trim().to_string();
    let mut tail = parts.next().unwrap_or("").trim_start();

    let mut href: Option<String> = None;
    let mut call: Option<(String, Option<String>)> = None;

    while !tail.is_empty() {
        if starts_with_ci(tail, "href") {
            let mut r = tail["href".len()..].trim_start();
            if !r.starts_with('"') {
                break;
            }
            r = &r[1..];
            let Some(end) = r.find('"') else {
                break;
            };
            href = Some(r[..end].to_string());
            tail = r[end + 1..].trim_start();
            continue;
        }

        if starts_with_ci(tail, "call") {
            let r = tail["call".len()..].trim_start();
            let Some(paren) = r.find('(') else {
                break;
            };
            let name = r[..paren].trim().to_string();
            let after = &r[paren + 1..];
            let Some(end) = after.find(')') else {
                break;
            };
            let args_raw = after[..end].to_string();
            let args = if args_raw.trim().is_empty() {
                None
            } else {
                Some(args_raw)
            };
            call = Some((name, args));
            tail = after[end + 1..].trim_start();
            continue;
        }

        break;
    }

    Some((ids, href, call))
}

type ClickStatementParts = (String, Option<String>, Option<(String, Option<String>)>);

fn parse_callback_args(raw: Option<&str>) -> Option<Vec<String>> {
    let raw = raw?;
    let mut out: Vec<String> = Vec::new();

    let mut cur = String::new();
    let mut in_quotes = false;
    for ch in raw.chars() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
                cur.push(ch);
            }
            ',' if !in_quotes => {
                out.push(cur);
                cur = String::new();
            }
            _ => cur.push(ch),
        }
    }
    out.push(cur);

    let out: Vec<String> = out
        .into_iter()
        .map(|s| {
            let mut item = s.trim().to_string();
            if item.starts_with('"') && item.ends_with('"') && item.len() >= 2 {
                item = item[1..item.len() - 1].to_string();
            }
            item
        })
        .collect();

    Some(out)
}

pub fn parse_gantt(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut db = GanttDb::default();
    db.clear();
    db.set_security_level(meta.effective_config.get_str("securityLevel"));
    if let Some(dm) = meta.effective_config.get_str("gantt.displayMode") {
        db.set_display_mode(dm);
    }

    let mut lines = code.lines();
    let mut header_seen = false;

    while let Some(line) = lines.next() {
        let stripped = strip_inline_comment(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !header_seen {
            if starts_with_ci(trimmed, "gantt") {
                header_seen = true;
                let rest = trimmed["gantt".len()..].trim_start();
                if !rest.is_empty() {
                    parse_gantt_statement(rest, &mut db, &mut lines)?;
                }
                continue;
            }
            return Err(Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: "expected gantt header".to_string(),
            });
        }

        parse_gantt_statement(stripped, &mut db, &mut lines)?;
    }

    if !header_seen {
        return Ok(json!({}));
    }

    let tasks = db.get_tasks()?;
    let tasks_json: Vec<Value> = tasks
        .into_iter()
        .map(|t| {
            let start_ms = t.start_time.map(|d| d.timestamp_millis());
            let end_ms = t.end_time.map(|d| d.timestamp_millis());
            let render_end_ms = t.render_end_time.map(|d| d.timestamp_millis());
            let raw_start = match &t.raw.start_time {
                StartTimeRaw::PrevTaskEnd => {
                    json!({ "type": "prevTaskEnd", "id": t.prev_task_id })
                }
                StartTimeRaw::GetStartDate { start_data } => {
                    json!({ "type": "getStartDate", "startData": start_data })
                }
            };
            json!({
                "section": t.section,
                "type": t.type_,
                "task": t.task,
                "id": t.id,
                "prevTaskId": t.prev_task_id,
                "order": t.order,
                "processed": t.processed,
                "classes": t.classes,
                "active": t.active,
                "done": t.done,
                "crit": t.crit,
                "milestone": t.milestone,
                "vert": t.vert,
                "manualEndTime": t.manual_end_time,
                "renderEndTime": render_end_ms,
                "raw": {
                    "data": t.raw.data,
                    "startTime": raw_start,
                    "endTime": { "data": t.raw.end_data },
                },
                "startTime": start_ms,
                "endTime": end_ms,
            })
        })
        .collect();

    Ok(json!({
        "type": meta.diagram_type,
        "title": if db.diagram_title.is_empty() { None::<String> } else { Some(db.diagram_title) },
        "accTitle": if db.acc_title.is_empty() { None::<String> } else { Some(db.acc_title) },
        "accDescr": if db.acc_descr.is_empty() { None::<String> } else { Some(db.acc_descr) },
        "dateFormat": db.date_format,
        "axisFormat": db.axis_format,
        "tickInterval": db.tick_interval,
        "todayMarker": db.today_marker,
        "includes": db.includes,
        "excludes": db.excludes,
        "inclusiveEndDates": db.inclusive_end_dates,
        "topAxis": db.top_axis,
        "weekday": db.weekday,
        "weekend": db.weekend,
        "displayMode": db.display_mode,
        "sections": db.sections,
        "tasks": tasks_json,
        "links": db.links,
        "clickEvents": db.click_events,
    }))
}

fn parse_gantt_statement(
    line: &str,
    db: &mut GanttDb,
    lines: &mut std::str::Lines<'_>,
) -> Result<()> {
    let stripped = strip_inline_comment(line);
    let t = stripped.trim();
    if t.is_empty() {
        return Ok(());
    }

    if let Some(v) = parse_keyword_arg(stripped, "dateFormat") {
        db.set_date_format(v);
        return Ok(());
    }
    if starts_with_ci(t, "inclusiveEndDates") {
        db.enable_inclusive_end_dates();
        return Ok(());
    }
    if starts_with_ci(t, "topAxis") {
        db.enable_top_axis();
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "axisFormat") {
        db.set_axis_format(v);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "tickInterval") {
        db.set_tick_interval(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "includes") {
        db.set_includes(v);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "excludes") {
        db.set_excludes(v);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "todayMarker") {
        db.set_today_marker(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "weekday") {
        let day = v.trim().to_lowercase();
        if !matches!(
            day.as_str(),
            "monday" | "tuesday" | "wednesday" | "thursday" | "friday" | "saturday" | "sunday"
        ) {
            return Err(Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: format!("invalid weekday: {day}"),
            });
        }
        db.set_weekday(&day);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "weekend") {
        let day = v.trim().to_lowercase();
        if !matches!(day.as_str(), "friday" | "saturday") {
            return Err(Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: format!("invalid weekend: {day}"),
            });
        }
        db.set_weekend(&day);
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "title") {
        db.set_diagram_title(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg_full_line(stripped, "section") {
        db.add_section(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_key_colon_value(stripped, "accTitle") {
        db.set_acc_title(&v);
        return Ok(());
    }
    if let Some(v) = parse_key_colon_value(stripped, "accDescr") {
        db.set_acc_descr(&v);
        return Ok(());
    }
    if let Some(v) = parse_acc_descr_block(lines, stripped) {
        db.set_acc_descr(&v);
        return Ok(());
    }
    if let Some((ids, href, call)) = parse_click_statement(stripped) {
        if let Some((name, args)) = call {
            db.set_click_event(&ids, &name, args.as_deref());
        }
        if let Some(href) = href {
            db.set_link(&ids, &href);
        }
        return Ok(());
    }

    let task_stmt = stripped.trim_start();

    let Some(colon) = task_stmt.find(':') else {
        return Err(Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: format!("unrecognized statement: {t}"),
        });
    };

    // Mermaid passes `taskTxt` through to the DB without trimming. This preserves any trailing
    // whitespace before the `:` delimiter (e.g. `Task1 :id,...` yields `Task1 `).
    let task_txt = &task_stmt[..colon];
    let mut task_data = task_stmt[colon + 1..].to_string();
    task_data = split_statement_suffix(&task_data).to_string();
    if task_txt.is_empty() || task_data.trim().is_empty() {
        return Ok(());
    }
    db.add_task(task_txt, &format!(":{task_data}"));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ParseOptions};
    use chrono::TimeZone;
    use futures::executor::block_on;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn local_ms(y: i32, m0: u32, d: u32, h: u32, min: u32, s: u32) -> i64 {
        let m = m0 + 1;
        let local = Local
            .with_ymd_and_hms(y, m, d, h, min, s)
            .single()
            .or_else(|| Local.with_ymd_and_hms(y, m, d, h, min, s).earliest())
            .unwrap();
        local.fixed_offset().timestamp_millis()
    }

    #[test]
    fn gantt_fixed_dates() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section testa1
test1: id1,2013-01-01,2013-01-12
"#,
        );
        let t0 = &model["tasks"][0];
        assert_eq!(t0["id"].as_str().unwrap(), "id1");
        assert_eq!(t0["task"].as_str().unwrap(), "test1");
        assert_eq!(
            t0["startTime"].as_i64().unwrap(),
            local_ms(2013, 0, 1, 0, 0, 0)
        );
        assert_eq!(
            t0["endTime"].as_i64().unwrap(),
            local_ms(2013, 0, 12, 0, 0, 0)
        );
    }

    #[test]
    fn gantt_duration_units() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section testa1
test1: id1,2013-01-01,2h
"#,
        );
        let t0 = &model["tasks"][0];
        assert_eq!(
            t0["startTime"].as_i64().unwrap(),
            local_ms(2013, 0, 1, 0, 0, 0)
        );
        assert_eq!(
            t0["endTime"].as_i64().unwrap(),
            local_ms(2013, 0, 1, 2, 0, 0)
        );
    }

    #[test]
    fn gantt_relative_after_id() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section sec1
test1: id1,2013-01-01,2w
test2: id2,after id1,1d
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(
            tasks[1]["startTime"].as_i64().unwrap(),
            local_ms(2013, 0, 15, 0, 0, 0)
        );
        assert_eq!(
            tasks[1]["endTime"].as_i64().unwrap(),
            local_ms(2013, 0, 16, 0, 0, 0)
        );
    }

    #[test]
    fn gantt_relative_until_id() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section sec1
task1: id1,2013-01-01,until id3
section sec2
task3: id3,2013-02-01,2d
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(
            tasks[0]["endTime"].as_i64().unwrap(),
            local_ms(2013, 1, 1, 0, 0, 0)
        );
    }

    #[test]
    fn gantt_excludes_weekends_and_specific_days() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
excludes weekends 2019-02-06,friday
section weekends skip test
test1: id1,2019-02-01,1d
test2: id2,after id1,2d
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(
            tasks[0]["startTime"].as_i64().unwrap(),
            local_ms(2019, 1, 1, 0, 0, 0)
        );
        assert_eq!(
            tasks[0]["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 4, 0, 0, 0)
        );
        assert_eq!(
            tasks[0]["renderEndTime"].as_i64().unwrap(),
            local_ms(2019, 1, 2, 0, 0, 0)
        );
        assert_eq!(
            tasks[1]["startTime"].as_i64().unwrap(),
            local_ms(2019, 1, 4, 0, 0, 0)
        );
        assert_eq!(
            tasks[1]["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 7, 0, 0, 0)
        );
        assert_eq!(
            tasks[1]["renderEndTime"].as_i64().unwrap(),
            local_ms(2019, 1, 6, 0, 0, 0)
        );
    }

    #[test]
    fn gantt_inclusive_end_dates_adds_one_day_for_strict_dates() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
inclusiveEndDates
test1: id1,2019-02-01,1d
test2: id2,2019-02-01,2019-02-03
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(
            tasks[0]["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 2, 0, 0, 0)
        );
        assert_eq!(
            tasks[1]["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 4, 0, 0, 0)
        );
        assert!(tasks[1]["renderEndTime"].is_null());
        assert!(tasks[1]["manualEndTime"].as_bool().unwrap());
    }

    #[test]
    fn gantt_rejects_ridiculous_years() {
        let engine = Engine::new();
        let err = block_on(engine.parse_diagram(
            r#"
gantt
dateFormat YYYYMMDD
test1: id1,202304,1d
"#,
            ParseOptions::default(),
        ))
        .unwrap_err();
        assert!(err.to_string().contains("Invalid date:202304"));
    }

    #[test]
    fn gantt_js_date_fallback_year_bounds_match_upstream_guardrail() {
        let dt = parse_js_date_fallback("10000").unwrap();
        assert_eq!(dt.year(), 10000);

        let err = parse_js_date_fallback("10001").unwrap_err();
        assert!(err.to_string().contains("Invalid date:10001"));
    }

    #[test]
    fn gantt_js_date_fallback_rejects_invalid_calendar_dates() {
        let err = parse_js_date_fallback("2019-02-30").unwrap_err();
        assert!(err.to_string().contains("Invalid date:2019-02-30"));
    }

    #[test]
    fn gantt_parse_duration_matches_upstream_examples() {
        assert_eq!(parse_duration("1d"), (1.0, "d".to_string()));
        assert!(parse_duration("1f").0.is_nan());
        assert_eq!(parse_duration("0.1s"), (0.1, "s".to_string()));
        assert_eq!(parse_duration("1ms"), (1.0, "ms".to_string()));
    }

    #[test]
    fn gantt_weekends_can_start_on_friday() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
excludes weekends
weekend friday
section friday-saturday weekends skip test
test1: id1,2024-02-28,3d
"#,
        );
        let t0 = &model["tasks"][0];
        assert_eq!(
            t0["startTime"].as_i64().unwrap(),
            local_ms(2024, 1, 28, 0, 0, 0)
        );
        assert_eq!(
            t0["endTime"].as_i64().unwrap(),
            local_ms(2024, 2, 4, 0, 0, 0)
        );
    }

    #[test]
    fn gantt_seconds_only_date_format_is_accepted() {
        let model = parse(
            r#"
gantt
dateFormat ss
section Network Request
RTT: rtt, 0, 20
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["task"].as_str().unwrap(), "RTT");
        assert_eq!(tasks[0]["id"].as_str().unwrap(), "rtt");
    }

    #[test]
    fn gantt_date_year_typos_fall_back_to_js_date_parsing() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section Vacation
London Trip 1: 2024-12-01, 7d
London Trip 2: 202-12-01, 7d
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);

        let ms0 = tasks[0]["startTime"].as_i64().unwrap();
        let ms1 = tasks[1]["startTime"].as_i64().unwrap();
        let dt0 = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms0).unwrap();
        let dt1 = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms1).unwrap();
        assert_eq!(dt0.year(), 2024);
        assert_eq!(dt1.year(), 202);
    }

    #[test]
    fn gantt_preserves_task_creation_order() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section A
Completed task: done, des1, 2014-01-06,2014-01-08
Active task: active, des2, 2014-01-09, 3d
section B
Future task: des3, after des2, 5d
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks[0]["order"].as_i64().unwrap(), 0);
        assert_eq!(tasks[1]["order"].as_i64().unwrap(), 1);
        assert_eq!(tasks[2]["order"].as_i64().unwrap(), 2);
        assert!(tasks[0]["done"].as_bool().unwrap());
        assert!(tasks[1]["active"].as_bool().unwrap());
        assert_eq!(tasks[0]["id"].as_str().unwrap(), "des1");
        assert_eq!(tasks[1]["id"].as_str().unwrap(), "des2");
        assert_eq!(tasks[2]["id"].as_str().unwrap(), "des3");
    }

    #[test]
    fn gantt_date_format_custom_separators_parse_strict() {
        let model = parse(
            r#"
gantt
dateFormat YYYY/MM/DD
section testa1
test1: id1,2013/01/01,2013/01/12
"#,
        );
        let t0 = &model["tasks"][0];
        assert_eq!(t0["id"].as_str().unwrap(), "id1");
        assert_eq!(
            t0["startTime"].as_i64().unwrap(),
            local_ms(2013, 0, 1, 0, 0, 0)
        );
        assert_eq!(
            t0["endTime"].as_i64().unwrap(),
            local_ms(2013, 0, 12, 0, 0, 0)
        );
    }

    #[test]
    fn dayjs_strict_parses_month_names_and_offsets() {
        let dt = parse_dayjs_like_strict("YYYY-MMM-DD", "2013-Jan-02").unwrap();
        assert_eq!(dt.timestamp_millis(), local_ms(2013, 0, 2, 0, 0, 0));

        let dt =
            parse_dayjs_like_strict("YYYY-MM-DDTHH:mm:ssZ", "2013-01-01T00:00:00+00:00").unwrap();
        assert_eq!(
            dt.timestamp_millis(),
            chrono::Utc
                .with_ymd_and_hms(2013, 1, 1, 0, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis()
        );

        let dt =
            parse_dayjs_like_strict("YYYY-MM-DDTHH:mm:ssZ", "2013-01-01T00:00:00+08:00").unwrap();
        assert_eq!(
            dt.timestamp_millis(),
            chrono::Utc
                .with_ymd_and_hms(2012, 12, 31, 16, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis()
        );

        let dt =
            parse_dayjs_like_strict("YYYY-MM-DDTHH:mm:ssZZ", "2013-01-01T00:00:00+0800").unwrap();
        assert_eq!(
            dt.timestamp_millis(),
            chrono::Utc
                .with_ymd_and_hms(2012, 12, 31, 16, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis()
        );

        let dt = parse_dayjs_like_strict("YYYY-MM-DDTHH:mm:ssZ", "2013-01-01T00:00:00Z").unwrap();
        assert_eq!(
            dt.timestamp_millis(),
            chrono::Utc
                .with_ymd_and_hms(2013, 1, 1, 0, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis()
        );
    }

    #[test]
    fn gantt_js_date_fallback_parses_iso_date_only_as_utc() {
        let model = parse(
            r#"
gantt
dateFormat YYYYMMDD
section A
test1: id1,2013-01-01,1d
"#,
        );
        let t0 = &model["tasks"][0];
        assert_eq!(
            t0["startTime"].as_i64().unwrap(),
            chrono::Utc
                .with_ymd_and_hms(2013, 1, 1, 0, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis()
        );
        assert_eq!(
            t0["endTime"].as_i64().unwrap(),
            chrono::Utc
                .with_ymd_and_hms(2013, 1, 2, 0, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis()
        );
    }

    #[test]
    fn gantt_js_date_fallback_parses_iso_datetime_without_tz_as_local() {
        let model = parse(
            r#"
gantt
dateFormat YYYYMMDD
section A
test1: id1,2013-01-01T00:00:00,1d
"#,
        );
        let t0 = &model["tasks"][0];
        assert_eq!(
            t0["startTime"].as_i64().unwrap(),
            local_ms(2013, 0, 1, 0, 0, 0)
        );
    }

    #[test]
    fn gantt_js_date_fallback_parses_timezone_offsets_with_or_without_colon() {
        let dt = parse_js_date_fallback("2013-01-01T00:00:00+0800").unwrap();
        assert_eq!(
            dt.timestamp_millis(),
            chrono::Utc
                .with_ymd_and_hms(2012, 12, 31, 16, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis()
        );

        let dt = parse_js_date_fallback("2013-01-01T00:00:00+08:00").unwrap();
        assert_eq!(
            dt.timestamp_millis(),
            chrono::Utc
                .with_ymd_and_hms(2012, 12, 31, 16, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis()
        );

        let dt = parse_js_date_fallback("2013-01-01T00:00:00Z").unwrap();
        assert_eq!(
            dt.timestamp_millis(),
            chrono::Utc
                .with_ymd_and_hms(2013, 1, 1, 0, 0, 0)
                .single()
                .unwrap()
                .timestamp_millis()
        );
    }

    #[test]
    fn gantt_js_date_fallback_parses_slash_dates_as_local() {
        let dt = parse_js_date_fallback("2013/01/01").unwrap();
        assert_eq!(dt.timestamp_millis(), local_ms(2013, 0, 1, 0, 0, 0));

        let dt = parse_js_date_fallback("2013/01/01 00:00:00").unwrap();
        assert_eq!(dt.timestamp_millis(), local_ms(2013, 0, 1, 0, 0, 0));
    }

    #[test]
    fn gantt_excludes_weekday_names_use_full_names() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
excludes friday
section A
test1: id1,2019-02-07,2d
"#,
        );
        let t0 = &model["tasks"][0];
        assert_eq!(
            t0["startTime"].as_i64().unwrap(),
            local_ms(2019, 1, 7, 0, 0, 0)
        );
        assert_eq!(
            t0["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 10, 0, 0, 0)
        );
        assert_eq!(
            t0["renderEndTime"].as_i64().unwrap(),
            local_ms(2019, 1, 10, 0, 0, 0)
        );
    }

    #[test]
    fn gantt_click_call_is_ignored_unless_security_level_loose() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section A
task: id1, 2013-01-01, 1d
click id1 call myFn("a,b", c)
"#,
        );
        assert!(model["clickEvents"].as_object().unwrap().is_empty());
    }

    #[test]
    fn gantt_click_call_parses_args_and_defaults_to_id() {
        let model = parse(
            r#"
%%{init: {"securityLevel":"loose"}}%%
gantt
dateFormat YYYY-MM-DD
section A
task: id1, 2013-01-01, 1d
task2: id2, 2013-01-02, 1d
click id2 call myFn("a,b", c)
click id1 call myFn2()
"#,
        );
        let ev1 = &model["clickEvents"]["id1"];
        assert_eq!(ev1["function_name"].as_str().unwrap(), "myFn2");
        assert_eq!(ev1["function_args"][0].as_str().unwrap(), "id1");
        assert!(ev1["raw_function_args"].is_null());

        let ev2 = &model["clickEvents"]["id2"];
        assert_eq!(ev2["function_name"].as_str().unwrap(), "myFn");
        assert_eq!(ev2["function_args"][0].as_str().unwrap(), "a,b");
        assert_eq!(ev2["function_args"][1].as_str().unwrap(), "c");
        assert_eq!(ev2["raw_function_args"].as_str().unwrap(), "\"a,b\", c");
    }

    #[test]
    fn gantt_common_db_sanitizes_title_and_accessibility_fields() {
        let model = parse(
            r#"
gantt
title <script>alert(1)</script><b>ok</b>
accTitle: <script>alert(1)</script><b>AT</b>
accDescr { <script>alert(1)</script>line1
    line2
}
"#,
        );
        assert_eq!(model["title"], json!("<b>ok</b>"));
        assert_eq!(model["accTitle"], json!("<b>AT</b>"));
        assert_eq!(model["accDescr"], json!("line1\nline2"));
    }

    #[test]
    fn gantt_duration_minutes_and_seconds_match_upstream() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section testa1
test1: id1,2013-01-01,2m
test2: id2,2013-01-01,2s
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(
            tasks[0]["endTime"].as_i64().unwrap(),
            local_ms(2013, 0, 1, 0, 2, 0)
        );
        assert_eq!(
            tasks[1]["endTime"].as_i64().unwrap(),
            local_ms(2013, 0, 1, 0, 0, 2)
        );
    }

    #[test]
    fn gantt_fixed_dates_without_id_match_upstream() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section testa1
test1: 2013-01-01,2013-01-12
"#,
        );
        let t0 = &model["tasks"][0];
        assert_eq!(t0["id"].as_str().unwrap(), "task1");
        assert_eq!(
            t0["startTime"].as_i64().unwrap(),
            local_ms(2013, 0, 1, 0, 0, 0)
        );
        assert_eq!(
            t0["endTime"].as_i64().unwrap(),
            local_ms(2013, 0, 12, 0, 0, 0)
        );
    }

    #[test]
    fn gantt_relative_refs_work_across_sections_like_upstream() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section sec1
test1: id1,2013-01-01,2w
test2: id2,after id3,1d
section sec2
test3: id3,after id1,2d
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(
            tasks[1]["startTime"].as_i64().unwrap(),
            local_ms(2013, 0, 17, 0, 0, 0)
        );
        assert_eq!(
            tasks[1]["endTime"].as_i64().unwrap(),
            local_ms(2013, 0, 18, 0, 0, 0)
        );
    }

    #[test]
    fn gantt_relative_after_multiple_ids_uses_latest_end_like_upstream() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section sec1
task1: id1,after id2 id3 id4,1d
task2: id2,2013-01-01,1d
task3: id3,2013-02-01,3d
task4: id4,2013-02-01,2d
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(
            tasks[0]["endTime"].as_i64().unwrap(),
            local_ms(2013, 1, 5, 0, 0, 0)
        );
    }

    #[test]
    fn gantt_relative_until_multiple_ids_uses_earliest_start_like_upstream() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section sec1
task1: id1,2013-01-01,until id2 id3 id4
task2: id2,2013-01-11,1d
task3: id3,2013-02-10,1d
task4: id4,2013-02-12,1d
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(
            tasks[0]["endTime"].as_i64().unwrap(),
            local_ms(2013, 0, 11, 0, 0, 0)
        );
    }

    #[test]
    fn gantt_timestamp_formats_x_and_x_support_signed_and_seconds() {
        let model = parse(
            r#"
gantt
dateFormat x
section T
t1: id1,-1,1ms
"#,
        );
        let t0 = &model["tasks"][0];
        assert_eq!(t0["startTime"].as_i64().unwrap(), -1);
        assert_eq!(t0["endTime"].as_i64().unwrap(), 0);

        let model = parse(
            r#"
gantt
dateFormat X
section T
t1: id1,20,1s
"#,
        );
        let t0 = &model["tasks"][0];
        assert_eq!(t0["startTime"].as_i64().unwrap(), 20);
        assert_eq!(t0["endTime"].as_i64().unwrap(), 1_020);
    }

    #[test]
    fn gantt_ignore_weekends_matches_upstream() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
excludes weekends 2019-02-06,friday
section weekends skip test
test1: id1,2019-02-01,1d
test2: id2,after id1,2d
test3: id3,after id2,7d
test4: id4,2019-02-01,2019-02-20
test5: id5,after id4,1d
section full ending task on last day
test6: id6,2019-02-13,2d
test7: id7,after id6,1d
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 7);

        assert_eq!(
            tasks[0]["startTime"].as_i64().unwrap(),
            local_ms(2019, 1, 1, 0, 0, 0)
        );
        assert_eq!(
            tasks[0]["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 4, 0, 0, 0)
        );
        assert_eq!(
            tasks[0]["renderEndTime"].as_i64().unwrap(),
            local_ms(2019, 1, 2, 0, 0, 0)
        );
        assert_eq!(tasks[0]["id"].as_str().unwrap(), "id1");
        assert_eq!(tasks[0]["task"].as_str().unwrap(), "test1");

        assert_eq!(
            tasks[1]["startTime"].as_i64().unwrap(),
            local_ms(2019, 1, 4, 0, 0, 0)
        );
        assert_eq!(
            tasks[1]["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 7, 0, 0, 0)
        );
        assert_eq!(
            tasks[1]["renderEndTime"].as_i64().unwrap(),
            local_ms(2019, 1, 6, 0, 0, 0)
        );
        assert_eq!(tasks[1]["id"].as_str().unwrap(), "id2");
        assert_eq!(tasks[1]["task"].as_str().unwrap(), "test2");

        assert_eq!(
            tasks[2]["startTime"].as_i64().unwrap(),
            local_ms(2019, 1, 7, 0, 0, 0)
        );
        assert_eq!(
            tasks[2]["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 20, 0, 0, 0)
        );
        assert_eq!(
            tasks[2]["renderEndTime"].as_i64().unwrap(),
            local_ms(2019, 1, 20, 0, 0, 0)
        );
        assert_eq!(tasks[2]["id"].as_str().unwrap(), "id3");
        assert_eq!(tasks[2]["task"].as_str().unwrap(), "test3");

        assert_eq!(
            tasks[3]["startTime"].as_i64().unwrap(),
            local_ms(2019, 1, 1, 0, 0, 0)
        );
        assert_eq!(
            tasks[3]["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 20, 0, 0, 0)
        );
        assert!(tasks[3]["renderEndTime"].is_null());
        assert!(tasks[3]["manualEndTime"].as_bool().unwrap());
        assert_eq!(tasks[3]["id"].as_str().unwrap(), "id4");
        assert_eq!(tasks[3]["task"].as_str().unwrap(), "test4");

        assert_eq!(
            tasks[4]["startTime"].as_i64().unwrap(),
            local_ms(2019, 1, 20, 0, 0, 0)
        );
        assert_eq!(
            tasks[4]["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 21, 0, 0, 0)
        );
        assert_eq!(
            tasks[4]["renderEndTime"].as_i64().unwrap(),
            local_ms(2019, 1, 21, 0, 0, 0)
        );
        assert_eq!(tasks[4]["id"].as_str().unwrap(), "id5");
        assert_eq!(tasks[4]["task"].as_str().unwrap(), "test5");

        assert_eq!(
            tasks[5]["startTime"].as_i64().unwrap(),
            local_ms(2019, 1, 13, 0, 0, 0)
        );
        assert_eq!(
            tasks[5]["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 18, 0, 0, 0)
        );
        assert_eq!(
            tasks[5]["renderEndTime"].as_i64().unwrap(),
            local_ms(2019, 1, 15, 0, 0, 0)
        );
        assert_eq!(tasks[5]["id"].as_str().unwrap(), "id6");
        assert_eq!(tasks[5]["task"].as_str().unwrap(), "test6");

        assert_eq!(
            tasks[6]["startTime"].as_i64().unwrap(),
            local_ms(2019, 1, 18, 0, 0, 0)
        );
        assert_eq!(
            tasks[6]["endTime"].as_i64().unwrap(),
            local_ms(2019, 1, 19, 0, 0, 0)
        );
        assert_eq!(tasks[6]["id"].as_str().unwrap(), "id7");
        assert_eq!(tasks[6]["task"].as_str().unwrap(), "test7");
    }

    #[test]
    fn gantt_maintains_task_creation_order_matches_upstream_sample() {
        let model = parse(
            r#"
gantt
accTitle: Project Execution
dateFormat YYYY-MM-DD
section section A section
Completed task: done,    des1, 2014-01-06,2014-01-08
Active task: active,  des2, 2014-01-09, 3d
Future task: des3, after des2, 5d
Future task2: des4, after des3, 5d

section section Critical tasks
Completed task in the critical line: crit, done, 2014-01-06,24h
Implement parser and jison: crit, done, after des1, 2d
Create tests for parser: crit, active, 3d
Future task in critical line: crit, 5d
Create tests for renderer: 2d
Add to mermaid: 1d

section section Documentation
Describe gantt syntax: active, a1, after des1, 3d
Add gantt diagram to demo page: after a1  , 20h
Add another diagram to demo page: doc1, after a1  , 48h

section section Last section
Describe gantt syntax: after doc1, 3d
Add gantt diagram to demo page: 20h
Add another diagram to demo page: 48h
"#,
        );

        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 16);

        for (i, t) in tasks.iter().enumerate() {
            assert_eq!(t["order"].as_i64().unwrap(), i as i64);
        }

        assert_eq!(tasks[0]["id"].as_str().unwrap(), "des1");
        assert_eq!(tasks[0]["task"].as_str().unwrap(), "Completed task");
        assert_eq!(
            tasks[0]["startTime"].as_i64().unwrap(),
            local_ms(2014, 0, 6, 0, 0, 0)
        );
        assert_eq!(
            tasks[0]["endTime"].as_i64().unwrap(),
            local_ms(2014, 0, 8, 0, 0, 0)
        );

        assert_eq!(tasks[1]["id"].as_str().unwrap(), "des2");
        assert_eq!(tasks[1]["task"].as_str().unwrap(), "Active task");
        assert_eq!(
            tasks[1]["startTime"].as_i64().unwrap(),
            local_ms(2014, 0, 9, 0, 0, 0)
        );
        assert_eq!(
            tasks[1]["endTime"].as_i64().unwrap(),
            local_ms(2014, 0, 12, 0, 0, 0)
        );

        assert_eq!(
            tasks[11]["task"].as_str().unwrap(),
            "Add gantt diagram to demo page"
        );
        assert_eq!(
            tasks[11]["startTime"].as_i64().unwrap(),
            local_ms(2014, 0, 11, 0, 0, 0)
        );
        assert_eq!(
            tasks[11]["endTime"].as_i64().unwrap(),
            local_ms(2014, 0, 11, 20, 0, 0)
        );

        assert_eq!(
            tasks[14]["task"].as_str().unwrap(),
            "Add gantt diagram to demo page"
        );
        assert_eq!(
            tasks[14]["startTime"].as_i64().unwrap(),
            local_ms(2014, 0, 16, 0, 0, 0)
        );
        assert_eq!(
            tasks[14]["endTime"].as_i64().unwrap(),
            local_ms(2014, 0, 16, 20, 0, 0)
        );

        assert_eq!(
            tasks[15]["task"].as_str().unwrap(),
            "Add another diagram to demo page"
        );
        assert_eq!(
            tasks[15]["startTime"].as_i64().unwrap(),
            local_ms(2014, 0, 16, 20, 0, 0)
        );
        assert_eq!(
            tasks[15]["endTime"].as_i64().unwrap(),
            local_ms(2014, 0, 18, 20, 0, 0)
        );
    }

    #[test]
    fn gantt_end_date_on_31st_matches_upstream() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section Task endTime is on the 31st day of the month
test1: id1,2019-09-30,11d
test2: id2,after id1,20d
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);

        assert_eq!(
            tasks[0]["startTime"].as_i64().unwrap(),
            local_ms(2019, 8, 30, 0, 0, 0)
        );
        assert_eq!(
            tasks[0]["endTime"].as_i64().unwrap(),
            local_ms(2019, 9, 11, 0, 0, 0)
        );
        assert_eq!(tasks[0]["id"].as_str().unwrap(), "id1");
        assert_eq!(tasks[0]["task"].as_str().unwrap(), "test1");

        assert_eq!(
            tasks[1]["startTime"].as_i64().unwrap(),
            local_ms(2019, 9, 11, 0, 0, 0)
        );
        assert_eq!(
            tasks[1]["endTime"].as_i64().unwrap(),
            local_ms(2019, 9, 31, 0, 0, 0)
        );
        assert!(tasks[1]["renderEndTime"].is_null());
        assert_eq!(tasks[1]["id"].as_str().unwrap(), "id2");
        assert_eq!(tasks[1]["task"].as_str().unwrap(), "test2");
    }

    #[test]
    fn gantt_today_marker_is_stored() {
        let model = parse(
            r#"
gantt
todayMarker off
"#,
        );
        assert_eq!(model["todayMarker"].as_str().unwrap(), "off");

        let model = parse(
            r#"
gantt
todayMarker stoke:stroke-width:5px,stroke:#00f,opacity:0.5
"#,
        );
        assert_eq!(
            model["todayMarker"].as_str().unwrap(),
            "stoke:stroke-width:5px,stroke:#00f,opacity:0.5"
        );
    }

    #[test]
    fn gantt_section_allows_hash_character() {
        let model = parse(
            r#"
gantt
section A #1
test: id1,2013-01-01,1d
"#,
        );
        assert_eq!(model["sections"][0].as_str().unwrap(), "A #1");
    }

    #[test]
    fn gantt_weekday_rejects_unknown_values() {
        let engine = Engine::new();
        let err = block_on(engine.parse_diagram(
            r#"
gantt
weekday foo
"#,
            ParseOptions::default(),
        ))
        .unwrap_err();
        assert!(err.to_string().contains("invalid weekday"));
    }

    #[test]
    fn gantt_weekend_rejects_unknown_values() {
        let engine = Engine::new();
        let err = block_on(engine.parse_diagram(
            r#"
gantt
weekend monday
"#,
            ParseOptions::default(),
        ))
        .unwrap_err();
        assert!(err.to_string().contains("invalid weekend"));
    }

    #[test]
    fn gantt_after_missing_id_defaults_to_today_midnight_like_upstream() {
        let model = parse(
            r#"
gantt
dateFormat YYYY-MM-DD
section testa1
test1: id1,2013-01-01,2w
test2: id2,after missing,1d
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        let expected = today_midnight_local().timestamp_millis();
        assert_eq!(tasks[1]["startTime"].as_i64().unwrap(), expected);
    }
}
