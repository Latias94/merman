use crate::{Error, ParseMetadata, Result, utils};
use chrono::{Datelike, Duration, FixedOffset, Local, NaiveDate, NaiveDateTime, TimeZone};
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
        let pos = self.task_index.get(id).copied()?;
        self.raw_tasks.get(pos)
    }

    fn find_task_by_id_mut(&mut self, id: &str) -> Option<&mut RawTask> {
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

        let start_time = start_time + Duration::days(1);
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

fn parse_dayjs_like_strict(date_format: &str, s: &str) -> Option<DateTimeFixed> {
    let fmt = date_format.trim();
    if fmt.is_empty() {
        return None;
    }

    match fmt {
        "YYYY-MM-DD" => {
            let d = NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()?;
            Some(local_from_naive(d.and_hms_opt(0, 0, 0)?))
        }
        "YYYY-MM-DD HH:mm:ss" => {
            let dt = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()?;
            Some(local_from_naive(dt))
        }
        "YYYYMMDD" => {
            let d = NaiveDate::parse_from_str(s, "%Y%m%d").ok()?;
            Some(local_from_naive(d.and_hms_opt(0, 0, 0)?))
        }
        "ss" => {
            let sec: u32 = s.trim().parse().ok()?;
            let sec = sec.min(59);
            let d = NaiveDate::from_ymd_opt(1970, 1, 1)?;
            Some(local_from_naive(d.and_hms_opt(0, 0, sec)?))
        }
        _ => None,
    }
}

fn parse_js_date_fallback(s: &str) -> Result<DateTimeFixed> {
    let s = s.trim();

    if Regex::new(r"^\d{1,4}-\d{2}-\d{2}$").unwrap().is_match(s) {
        let mut it = s.split('-');
        let year: i32 = it
            .next()
            .unwrap()
            .parse()
            .map_err(|_| Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: format!("Invalid date:{s}"),
            })?;
        let month: u32 = it
            .next()
            .unwrap()
            .parse()
            .map_err(|_| Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: format!("Invalid date:{s}"),
            })?;
        let day: u32 = it
            .next()
            .unwrap()
            .parse()
            .map_err(|_| Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: format!("Invalid date:{s}"),
            })?;
        let d = NaiveDate::from_ymd_opt(year, month, day).ok_or_else(|| Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: format!("Invalid date:{s}"),
        })?;
        return Ok(local_from_naive(d.and_hms_opt(0, 0, 0).unwrap()));
    }

    if Regex::new(r"^\d+$").unwrap().is_match(s) {
        let n: i32 = s.parse().map_err(|_| Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: format!("Invalid date:{s}"),
        })?;
        let year = if s.len() <= 2 { 2000 + n } else { n };
        if year < -10000 || year > 10000 {
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
        if year < -10000 || year > 10000 {
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

fn get_start_date(db: &GanttDb, date_format: &str, raw: &str) -> Result<Option<DateTimeFixed>> {
    let s = raw.trim();

    let is_timestamp_format = matches!(date_format.trim(), "x" | "X");
    if is_timestamp_format && Regex::new(r"^\d+$").unwrap().is_match(s) {
        let ms: i64 = s.parse().map_err(|_| Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: format!("Invalid date:{s}"),
        })?;
        let dt = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms).ok_or_else(|| {
            Error::DiagramParse {
                diagram_type: "gantt".to_string(),
                message: format!("Invalid date:{s}"),
            }
        })?;
        return Ok(Some(dt.with_timezone(&FixedOffset::east_opt(0).unwrap())));
    }

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

    if let Some(dt) = parse_dayjs_like_strict(date_format, s) {
        return Ok(Some(dt));
    }

    let dt = parse_js_date_fallback(s)?;
    let year = dt.year();
    if year < -10000 || year > 10000 {
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
    let ms = match unit {
        "ms" => value,
        "s" => value * 1_000.0,
        "m" => value * 60_000.0,
        "h" => value * 3_600_000.0,
        "d" => value * 86_400_000.0,
        "w" => value * 604_800_000.0,
        "M" | "y" => return None,
        _ => return None,
    };
    Some(dt + Duration::milliseconds(ms.trunc() as i64))
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
            dt = dt + Duration::days(1);
        }
        return Ok(Some(dt));
    }

    let (value, unit) = parse_duration(s);
    if value.is_finite() {
        if unit == "M" || unit == "y" {
            return Ok(Some(prev_time));
        }
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

fn format_dayjs_like(dt: DateTimeFixed, fmt: &str) -> String {
    match fmt.trim() {
        "YYYY-MM-DD" => dt.format("%Y-%m-%d").to_string(),
        "YYYYMMDD" => dt.format("%Y%m%d").to_string(),
        "ss" => dt.format("%S").to_string(),
        _ => dt.format("%Y-%m-%d").to_string(),
    }
}

fn is_invalid_date(db: &GanttDb, date: DateTimeFixed, date_format: &str) -> bool {
    let formatted = format_dayjs_like(date, date_format);
    let date_only = date.format("%Y-%m-%d").to_string();

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
        let iso = date.weekday().number_from_monday(); // 1..=7
        if iso == weekend_start || iso == weekend_start + 1 {
            return true;
        }
    }

    let weekday = date.weekday().to_string().to_lowercase();
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
            end_time = end_time + Duration::days(1);
        }
        start_time = start_time + Duration::days(1);
    }
    Ok((end_time, render_end_time))
}

fn strip_inline_comment(line: &str) -> &str {
    let mut pos = 0usize;
    while let Some(rel) = line[pos..].find("%%") {
        let idx = pos + rel;
        if line.get(idx + 2..idx + 3) == Some("{") {
            pos = idx + 2;
            continue;
        }
        return &line[..idx];
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

fn starts_with_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix)
}

fn parse_keyword_arg<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let t = line.trim_start();
    if !starts_with_ci(t, keyword) {
        return None;
    }
    let after = &t[keyword.len()..];
    let Some(ws) = after.chars().next() else {
        return None;
    };
    if !ws.is_whitespace() {
        return None;
    }
    let rest = &after[ws.len_utf8()..];
    Some(split_statement_suffix(rest))
}

fn parse_key_colon_value(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start();
    if !starts_with_ci(t, key) {
        return None;
    }
    let rest = t[key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?;
    Some(split_statement_suffix(rest).trim().to_string())
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

fn parse_click_statement(
    line: &str,
) -> Option<(String, Option<String>, Option<(String, Option<String>)>)> {
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
    if let Some(v) = parse_keyword_arg(stripped, "todayMarker") {
        db.set_today_marker(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "weekday") {
        db.set_weekday(v.trim().to_lowercase().as_str());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "weekend") {
        db.set_weekend(v.trim().to_lowercase().as_str());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "title") {
        db.set_diagram_title(v.trim());
        return Ok(());
    }
    if let Some(v) = parse_keyword_arg(stripped, "section") {
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

    let Some(colon) = stripped.find(':') else {
        return Err(Error::DiagramParse {
            diagram_type: "gantt".to_string(),
            message: format!("unrecognized statement: {t}"),
        });
    };

    let task_txt = stripped[..colon].trim();
    let mut task_data = stripped[colon + 1..].to_string();
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
}
