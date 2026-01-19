use crate::model::{
    Bounds, GanttAxisTickLayout, GanttDiagramLayout, GanttExcludeRangeLayout, GanttRowLayout,
    GanttSectionTitleLayout, GanttTaskBarLayout, GanttTaskLabelLayout, GanttTaskLayout,
};
use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextStyle};
use crate::{Error, Result};
use chrono::{Datelike, Local, TimeZone, Timelike};
use serde::Deserialize;
use std::collections::HashMap;

const DEFAULT_WIDTH: f64 = 1200.0;
const MS_PER_DAY: i64 = 86_400_000;

#[derive(Debug, Clone, Deserialize)]
struct GanttTaskModel {
    id: String,
    task: String,
    section: String,
    #[serde(rename = "type")]
    task_type: String,
    #[serde(default)]
    classes: Vec<String>,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    crit: bool,
    #[serde(default)]
    milestone: bool,
    #[serde(default)]
    vert: bool,
    #[serde(default)]
    order: i64,
    #[serde(rename = "startTime")]
    start_ms: i64,
    #[serde(rename = "endTime")]
    end_ms: i64,
    #[serde(default, rename = "renderEndTime")]
    render_end_ms: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
struct GanttModel {
    #[serde(default)]
    title: Option<String>,
    #[serde(default, rename = "dateFormat")]
    date_format: String,
    #[serde(default, rename = "axisFormat")]
    axis_format: String,
    #[serde(default, rename = "tickInterval")]
    tick_interval: Option<String>,
    #[serde(default, rename = "todayMarker")]
    today_marker: String,
    #[serde(default)]
    includes: Vec<String>,
    #[serde(default)]
    excludes: Vec<String>,
    #[serde(default, rename = "displayMode")]
    display_mode: String,
    #[serde(default, rename = "topAxis")]
    top_axis: bool,
    #[serde(default)]
    weekday: String,
    #[serde(default)]
    weekend: String,
    #[serde(default)]
    tasks: Vec<GanttTaskModel>,
}

fn cfg_f64(cfg: &serde_json::Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_f64()
}

fn cfg_i64(cfg: &serde_json::Value, path: &[&str]) -> Option<i64> {
    let mut cur = cfg;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_i64()
}

fn cfg_bool(cfg: &serde_json::Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_bool()
}

fn month_name_short(m: u32) -> &'static str {
    match m {
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

fn month_name_long(m: u32) -> &'static str {
    match m {
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

fn weekday_name_short(w: chrono::Weekday) -> &'static str {
    match w {
        chrono::Weekday::Mon => "Mon",
        chrono::Weekday::Tue => "Tue",
        chrono::Weekday::Wed => "Wed",
        chrono::Weekday::Thu => "Thu",
        chrono::Weekday::Fri => "Fri",
        chrono::Weekday::Sat => "Sat",
        chrono::Weekday::Sun => "Sun",
    }
}

fn weekday_name_long(w: chrono::Weekday) -> &'static str {
    match w {
        chrono::Weekday::Mon => "Monday",
        chrono::Weekday::Tue => "Tuesday",
        chrono::Weekday::Wed => "Wednesday",
        chrono::Weekday::Thu => "Thursday",
        chrono::Weekday::Fri => "Friday",
        chrono::Weekday::Sat => "Saturday",
        chrono::Weekday::Sun => "Sunday",
    }
}

fn ordinal_suffix(n: u32) -> &'static str {
    let nn = n % 100;
    if (11..=13).contains(&nn) {
        return "th";
    }
    match n % 10 {
        1 => "st",
        2 => "nd",
        3 => "rd",
        _ => "th",
    }
}

fn format_dayjs_like(ms: i64, fmt: &str) -> Option<String> {
    let dt_utc = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)?;
    let dt = dt_utc.with_timezone(&Local);
    let fmt = fmt.trim();

    let mut out = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '[' {
            i += 1;
            while i < chars.len() && chars[i] != ']' {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() && chars[i] == ']' {
                i += 1;
            }
            continue;
        }

        let rest: String = chars[i..].iter().collect();
        let token = [
            "YYYY", "MMMM", "MMM", "dddd", "ddd", "YY", "MM", "DD", "Do", "HH", "hh", "mm", "ss",
            "SSS", "ZZ", "Z", "A", "a", "x", "X", "M", "D", "H", "h", "m", "s",
        ]
        .into_iter()
        .find(|t| rest.starts_with(t));

        if let Some(t) = token {
            match t {
                "YYYY" => out.push_str(&format!("{:04}", dt.year())),
                "YY" => out.push_str(&format!("{:02}", (dt.year() % 100).abs())),
                "MMMM" => out.push_str(month_name_long(dt.month())),
                "MMM" => out.push_str(month_name_short(dt.month())),
                "MM" => out.push_str(&format!("{:02}", dt.month())),
                "M" => out.push_str(&format!("{}", dt.month())),
                "DD" => out.push_str(&format!("{:02}", dt.day())),
                "D" => out.push_str(&format!("{}", dt.day())),
                "Do" => out.push_str(&format!("{}{}", dt.day(), ordinal_suffix(dt.day()))),
                "dddd" => out.push_str(weekday_name_long(dt.weekday())),
                "ddd" => out.push_str(weekday_name_short(dt.weekday())),
                "HH" => out.push_str(&format!("{:02}", dt.hour())),
                "H" => out.push_str(&format!("{}", dt.hour())),
                "hh" => {
                    let h = dt.hour() % 12;
                    let h = if h == 0 { 12 } else { h };
                    out.push_str(&format!("{:02}", h));
                }
                "h" => {
                    let h = dt.hour() % 12;
                    let h = if h == 0 { 12 } else { h };
                    out.push_str(&format!("{}", h));
                }
                "mm" => out.push_str(&format!("{:02}", dt.minute())),
                "m" => out.push_str(&format!("{}", dt.minute())),
                "ss" => out.push_str(&format!("{:02}", dt.second())),
                "s" => out.push_str(&format!("{}", dt.second())),
                "SSS" => out.push_str(&format!("{:03}", dt.timestamp_subsec_millis())),
                "A" => out.push_str(if dt.hour() < 12 { "AM" } else { "PM" }),
                "a" => out.push_str(if dt.hour() < 12 { "am" } else { "pm" }),
                "Z" => {
                    let off = dt.offset().local_minus_utc();
                    let sign = if off >= 0 { '+' } else { '-' };
                    let off = off.abs();
                    let hh = off / 3600;
                    let mm = (off % 3600) / 60;
                    out.push_str(&format!("{sign}{:02}:{:02}", hh, mm));
                }
                "ZZ" => {
                    let off = dt.offset().local_minus_utc();
                    let sign = if off >= 0 { '+' } else { '-' };
                    let off = off.abs();
                    let hh = off / 3600;
                    let mm = (off % 3600) / 60;
                    out.push_str(&format!("{sign}{:02}{:02}", hh, mm));
                }
                "x" => out.push_str(&format!("{ms}")),
                "X" => out.push_str(&format!("{}", ms / 1000)),
                _ => {}
            }
            i += t.len();
            continue;
        }

        out.push(c);
        i += 1;
    }

    Some(out)
}

fn format_yyyy_mm_dd(ms: i64) -> Option<String> {
    format_dayjs_like(ms, "YYYY-MM-DD")
}

fn weekend_start_day(weekend: &str) -> u32 {
    match weekend {
        "friday" => 5,
        _ => 6,
    }
}

fn is_invalid_date(
    ms: i64,
    date_format: &str,
    excludes: &[String],
    includes: &[String],
    weekend: &str,
) -> bool {
    let Some(formatted_date) = format_dayjs_like(ms, date_format) else {
        return false;
    };
    let Some(date_only) = format_yyyy_mm_dd(ms) else {
        return false;
    };

    if includes
        .iter()
        .any(|t| t == &formatted_date || t == &date_only)
    {
        return false;
    }

    let Some(dt_utc) = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms) else {
        return false;
    };
    let dt = dt_utc.with_timezone(&Local);
    let iso_weekday = dt.weekday().number_from_monday();

    if excludes.iter().any(|t| t == "weekends") {
        let start = weekend_start_day(weekend);
        if iso_weekday == start || iso_weekday == start + 1 {
            return true;
        }
    }

    let weekday_lower = weekday_name_long(dt.weekday()).to_lowercase();
    if excludes.iter().any(|t| t == &weekday_lower) {
        return true;
    }

    excludes
        .iter()
        .any(|t| t == &formatted_date || t == &date_only)
}

fn start_of_day_ms(ms: i64) -> Option<i64> {
    let dt_utc = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)?;
    let dt = dt_utc.with_timezone(&Local);
    let d = dt.date_naive();
    let local = Local
        .from_local_datetime(&d.and_hms_opt(0, 0, 0)?)
        .single()?;
    Some(local.with_timezone(&chrono::Utc).timestamp_millis())
}

fn end_of_day_ms(ms: i64) -> Option<i64> {
    let start = start_of_day_ms(ms)?;
    Some(start + MS_PER_DAY - 1)
}

fn scale_time(ms: i64, min_ms: i64, max_ms: i64, range: f64) -> f64 {
    if max_ms <= min_ms {
        return 0.0;
    }
    let t = (ms - min_ms) as f64 / (max_ms - min_ms) as f64;
    (t * range).round()
}

fn collect_categories(tasks: &[GanttTaskModel]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tasks {
        if !out.iter().any(|x| x == &t.task_type) {
            out.push(t.task_type.clone());
        }
    }
    out
}

fn get_max_intersections(tasks: &mut [GanttTaskModel], order_offset: i64) -> i64 {
    let mut timeline: Vec<i64> = vec![i64::MIN; tasks.len()];
    let mut sorted: Vec<usize> = (0..tasks.len()).collect();
    sorted.sort_by(|&a, &b| {
        let ta = tasks[a].start_ms;
        let tb = tasks[b].start_ms;
        ta.cmp(&tb)
            .then_with(|| tasks[a].order.cmp(&tasks[b].order))
    });

    let mut max_i: i64 = 0;
    for idx in sorted {
        for j in 0..timeline.len() {
            if tasks[idx].start_ms >= timeline[j] {
                timeline[j] = tasks[idx].end_ms;
                tasks[idx].order = j as i64 + order_offset;
                max_i = max_i.max(j as i64);
                break;
            }
        }
    }
    max_i
}

fn auto_tick_interval(min_ms: i64, max_ms: i64) -> (i64, &'static str) {
    let span = (max_ms - min_ms).abs().max(1) as f64;
    let approx = span / 10.0;

    for (unit_ms, name) in [
        (MS_PER_DAY * 30, "month"),
        (MS_PER_DAY * 7, "week"),
        (MS_PER_DAY, "day"),
        (3_600_000, "hour"),
        (60_000, "minute"),
        (1_000, "second"),
        (1, "millisecond"),
    ] {
        if approx >= unit_ms as f64 {
            let every = (approx / unit_ms as f64).round().max(1.0) as i64;
            return (every, name);
        }
    }
    (1, "day")
}

fn parse_tick_interval(s: &str) -> Option<(i64, &str)> {
    let s = s.trim();
    let mut num = String::new();
    let mut idx = 0;
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            num.push(ch);
            idx += 1;
        } else {
            break;
        }
    }
    let every = num.parse::<i64>().ok()?;
    if every <= 0 {
        return None;
    }
    let unit = &s[idx..];
    match unit {
        "millisecond" | "second" | "minute" | "hour" | "day" | "week" | "month" => {
            Some((every, unit))
        }
        _ => None,
    }
}

fn add_interval(ms: i64, every: i64, unit: &str) -> Option<i64> {
    let dt_utc = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(ms)?;
    let dt = dt_utc.with_timezone(&Local);
    let naive = dt.naive_local();

    let next = match unit {
        "millisecond" => naive + chrono::Duration::milliseconds(every),
        "second" => naive + chrono::Duration::seconds(every),
        "minute" => naive + chrono::Duration::minutes(every),
        "hour" => naive + chrono::Duration::hours(every),
        "day" => naive + chrono::Duration::days(every),
        "week" => naive + chrono::Duration::days(every * 7),
        "month" => {
            let mut y = naive.date().year();
            let mut m = naive.date().month() as i32 + every as i32;
            while m > 12 {
                y += 1;
                m -= 12;
            }
            while m < 1 {
                y -= 1;
                m += 12;
            }
            let d = naive.date().day().min(28);
            let date = chrono::NaiveDate::from_ymd_opt(y, m as u32, d)?;
            date.and_hms_opt(
                naive.time().hour(),
                naive.time().minute(),
                naive.time().second(),
            )?
        }
        _ => return None,
    };

    let out = Local.from_local_datetime(&next).single()?;
    Some(out.with_timezone(&chrono::Utc).timestamp_millis())
}

fn axis_format_to_strftime(axis_format: &str, date_format: &str, cfg_axis_format: &str) -> String {
    let user = axis_format.trim();
    if !user.is_empty() {
        return user.to_string();
    }
    if date_format.trim() == "D" {
        return "%d".to_string();
    }
    let conf = cfg_axis_format.trim();
    if !conf.is_empty() {
        return conf.to_string();
    }
    "%Y-%m-%d".to_string()
}

fn build_ticks(
    min_ms: i64,
    max_ms: i64,
    range: f64,
    left_padding: f64,
    axis_format: &str,
    tick_interval: Option<&str>,
) -> Vec<GanttAxisTickLayout> {
    let (every, unit) = tick_interval
        .and_then(parse_tick_interval)
        .unwrap_or_else(|| auto_tick_interval(min_ms, max_ms));

    let mut ticks = Vec::new();
    let mut cur = min_ms;
    let max_ticks = 2000;
    for _ in 0..max_ticks {
        if cur > max_ms {
            break;
        }
        let x = scale_time(cur, min_ms, max_ms, range) + left_padding;
        let label = chrono::DateTime::<chrono::Utc>::from_timestamp_millis(cur)
            .map(|d| d.with_timezone(&Local).format(axis_format).to_string())
            .unwrap_or_default();
        ticks.push(GanttAxisTickLayout {
            time_ms: cur,
            x,
            label,
        });
        let Some(next) = add_interval(cur, every, unit) else {
            break;
        };
        if next <= cur {
            break;
        }
        cur = next;
    }
    ticks
}

pub fn layout_gantt_diagram(
    model: &serde_json::Value,
    config: &serde_json::Value,
    text_measurer: &dyn TextMeasurer,
) -> Result<GanttDiagramLayout> {
    let mut m: GanttModel = serde_json::from_value(model.clone()).map_err(Error::Json)?;

    let gantt_cfg = config.get("gantt").unwrap_or(config);
    let bar_gap = cfg_f64(gantt_cfg, &["barGap"]).unwrap_or(4.0);
    let bar_height = cfg_f64(gantt_cfg, &["barHeight"]).unwrap_or(20.0);
    let top_padding = cfg_f64(gantt_cfg, &["topPadding"]).unwrap_or(50.0);
    let left_padding = cfg_f64(gantt_cfg, &["leftPadding"]).unwrap_or(75.0);
    let right_padding = cfg_f64(gantt_cfg, &["rightPadding"]).unwrap_or(75.0);
    let grid_line_start_padding = cfg_f64(gantt_cfg, &["gridLineStartPadding"]).unwrap_or(35.0);
    let title_top_margin = cfg_f64(gantt_cfg, &["titleTopMargin"]).unwrap_or(25.0);
    let font_size = cfg_f64(gantt_cfg, &["fontSize"]).unwrap_or(11.0);
    let section_font_size = cfg_f64(gantt_cfg, &["sectionFontSize"]).unwrap_or(11.0);
    let number_section_styles = cfg_i64(gantt_cfg, &["numberSectionStyles"]).unwrap_or(4);

    let cfg_display_mode = gantt_cfg
        .get("displayMode")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cfg_top_axis = cfg_bool(gantt_cfg, &["topAxis"]).unwrap_or(false);
    let cfg_axis_format = gantt_cfg
        .get("axisFormat")
        .and_then(|v| v.as_str())
        .unwrap_or("%Y-%m-%d");

    let width = gantt_cfg
        .get("useWidth")
        .and_then(|v| v.as_f64())
        .unwrap_or(DEFAULT_WIDTH);
    let gap = bar_height + bar_gap;

    let categories = collect_categories(&m.tasks);
    let is_compact = m.display_mode == "compact" || cfg_display_mode == "compact";

    let mut category_heights: Vec<(String, i64)> = Vec::new();
    if is_compact {
        let mut section_order: Vec<String> = Vec::new();
        let mut section_map: HashMap<String, Vec<usize>> = HashMap::new();
        for (idx, t) in m.tasks.iter().enumerate() {
            if !section_map.contains_key(&t.section) {
                section_order.push(t.section.clone());
                section_map.insert(t.section.clone(), Vec::new());
            }
            section_map.get_mut(&t.section).unwrap().push(idx);
        }

        let mut order_offset: i64 = 0;
        for sec in section_order {
            let idxs = section_map.get(&sec).cloned().unwrap_or_default();
            let mut subset: Vec<GanttTaskModel> =
                idxs.iter().map(|&i| m.tasks[i].clone()).collect();
            let max_i = get_max_intersections(&mut subset, order_offset);
            for (pos, &orig_idx) in idxs.iter().enumerate() {
                m.tasks[orig_idx].order = subset[pos].order;
            }
            let height = max_i + 1;
            order_offset += height;
            category_heights.push((sec, height));
        }
    } else {
        for c in &categories {
            let count = m.tasks.iter().filter(|t| &t.task_type == c).count() as i64;
            category_heights.push((c.clone(), count));
        }
    }

    let mut height = 2.0 * top_padding;
    if is_compact {
        for (_k, h) in &category_heights {
            height += *h as f64 * gap;
        }
    } else {
        height += m.tasks.len() as f64 * gap;
    }

    let has_tasks = !m.tasks.is_empty();
    let (min_ms, max_ms) = if has_tasks {
        let min_ms = m.tasks.iter().map(|t| t.start_ms).min().unwrap_or(0);
        let max_ms = m.tasks.iter().map(|t| t.end_ms).max().unwrap_or(min_ms);
        (min_ms, max_ms)
    } else {
        (0, 0)
    };
    let range = (width - left_padding - right_padding).max(1.0);

    // Sort by start time for rendering.
    m.tasks.sort_by(|a, b| a.start_ms.cmp(&b.start_ms));

    // Exclude day ranges.
    let mut excludes_layout: Vec<GanttExcludeRangeLayout> = Vec::new();
    if has_tasks && (!m.excludes.is_empty() || !m.includes.is_empty()) {
        let span_days = (max_ms - min_ms).abs() / MS_PER_DAY;
        if span_days <= 365 * 5 {
            let mut cur = start_of_day_ms(min_ms).unwrap_or(min_ms);
            let max_day = start_of_day_ms(max_ms).unwrap_or(max_ms);
            let mut range_start: Option<i64> = None;
            let mut range_end: Option<i64> = None;

            while cur <= max_day {
                let invalid =
                    is_invalid_date(cur, &m.date_format, &m.excludes, &m.includes, &m.weekend);
                if invalid {
                    if range_start.is_none() {
                        range_start = Some(cur);
                        range_end = Some(cur);
                    } else {
                        range_end = Some(cur);
                    }
                } else if let (Some(s), Some(e)) = (range_start.take(), range_end.take()) {
                    let id = format!(
                        "exclude-{}",
                        format_yyyy_mm_dd(s).unwrap_or_else(|| "invalid".to_string())
                    );
                    let x0 = scale_time(s, min_ms, max_ms, range) + left_padding;
                    let eod = end_of_day_ms(e).unwrap_or(e);
                    let x1 = scale_time(eod, min_ms, max_ms, range) + left_padding;
                    excludes_layout.push(GanttExcludeRangeLayout {
                        id,
                        start_ms: s,
                        end_ms: eod,
                        x: x0,
                        y: grid_line_start_padding,
                        width: (x1 - x0).max(0.0),
                        height: (height - top_padding - grid_line_start_padding).max(0.0),
                    });
                }
                cur += MS_PER_DAY;
            }

            if let (Some(s), Some(e)) = (range_start.take(), range_end.take()) {
                let id = format!(
                    "exclude-{}",
                    format_yyyy_mm_dd(s).unwrap_or_else(|| "invalid".to_string())
                );
                let x0 = scale_time(s, min_ms, max_ms, range) + left_padding;
                let eod = end_of_day_ms(e).unwrap_or(e);
                let x1 = scale_time(eod, min_ms, max_ms, range) + left_padding;
                excludes_layout.push(GanttExcludeRangeLayout {
                    id,
                    start_ms: s,
                    end_ms: eod,
                    x: x0,
                    y: grid_line_start_padding,
                    width: (x1 - x0).max(0.0),
                    height: (height - top_padding - grid_line_start_padding).max(0.0),
                });
            }
        }
    }

    // Background rows.
    let mut unique_orders: Vec<i64> = Vec::new();
    for t in &m.tasks {
        if !unique_orders.contains(&t.order) {
            unique_orders.push(t.order);
        }
    }
    unique_orders.sort();

    let mut rows: Vec<GanttRowLayout> = Vec::new();
    for order in &unique_orders {
        let ttype = m
            .tasks
            .iter()
            .find(|t| t.order == *order)
            .map(|t| t.task_type.clone())
            .unwrap_or_default();

        let mut sec_num = 0_i64;
        for (i, c) in categories.iter().enumerate() {
            if &ttype == c {
                sec_num = (i as i64) % number_section_styles;
            }
        }

        let y = *order as f64 * gap + top_padding - 2.0;
        rows.push(GanttRowLayout {
            index: *order,
            x: 0.0,
            y,
            width: width - right_padding / 2.0,
            height: gap,
            class: format!("section section{sec_num}"),
        });
    }

    // Tasks (bars + labels).
    let text_style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size,
        font_weight: None,
    };

    let mut tasks: Vec<GanttTaskLayout> = Vec::new();
    for t in &m.tasks {
        let start_x = scale_time(t.start_ms, min_ms, max_ms, range);
        let end_x = scale_time(t.end_ms, min_ms, max_ms, range);
        let render_end_x = scale_time(t.render_end_ms.unwrap_or(t.end_ms), min_ms, max_ms, range);

        let mut bar_x = start_x + left_padding;
        if t.milestone {
            bar_x = start_x + left_padding + 0.5 * (end_x - start_x) - 0.5 * bar_height;
        }

        let bar_y = if t.vert {
            grid_line_start_padding
        } else {
            t.order as f64 * gap + top_padding
        };
        let bar_width = if t.milestone {
            bar_height
        } else if t.vert {
            0.08 * bar_height
        } else {
            (render_end_x - start_x).max(0.0)
        };
        let bar_height_actual = if t.vert {
            m.tasks.len() as f64 * gap + bar_height * 2.0
        } else {
            bar_height
        };

        let mut sec_num = 0_i64;
        for (i, c) in categories.iter().enumerate() {
            if &t.task_type == c {
                sec_num = (i as i64) % number_section_styles;
            }
        }

        let mut task_class = String::new();
        if t.active {
            if t.crit {
                task_class.push_str(" activeCrit");
            } else {
                task_class.push_str(" active");
            }
        } else if t.done {
            if t.crit {
                task_class.push_str(" doneCrit");
            } else {
                task_class.push_str(" done");
            }
        } else if t.crit {
            task_class.push_str(" crit");
        }
        if task_class.is_empty() {
            task_class.push_str(" task");
        }
        if t.milestone {
            task_class = format!(" milestone{task_class}");
        }
        if t.vert {
            task_class = format!(" vert{task_class}");
        }
        task_class.push_str(&format!("{sec_num}"));
        if !t.classes.is_empty() {
            task_class.push(' ');
            task_class.push_str(&t.classes.join(" "));
        }

        let bar = GanttTaskBarLayout {
            id: t.id.clone(),
            x: bar_x,
            y: bar_y,
            width: bar_width,
            height: bar_height_actual,
            rx: 3.0,
            ry: 3.0,
            class: format!("task{task_class}"),
        };

        let mut label_start_x = start_x;
        let mut label_end_x = render_end_x;
        if t.milestone {
            label_start_x += 0.5 * (end_x - start_x) - 0.5 * bar_height;
            label_end_x = label_start_x + bar_height;
        }

        let metrics = text_measurer.measure(&t.task, &text_style);
        let text_width = metrics.width;

        let label_x = if t.vert {
            start_x + left_padding
        } else if text_width > (label_end_x - label_start_x).abs() {
            if label_end_x + text_width + 1.5 * left_padding > width {
                label_start_x + left_padding - 5.0
            } else {
                label_end_x + left_padding + 5.0
            }
        } else {
            (label_end_x - label_start_x) / 2.0 + label_start_x + left_padding
        };

        let label_y = if t.vert {
            grid_line_start_padding + m.tasks.len() as f64 * gap + 60.0
        } else {
            t.order as f64 * gap + bar_height / 2.0 + (font_size / 2.0 - 2.0) + top_padding
        };

        let base_classes = if t.classes.is_empty() {
            String::new()
        } else {
            format!("{} ", t.classes.join(" "))
        };

        let outside_left = !t.vert
            && text_width > (label_end_x - label_start_x).abs()
            && (label_end_x + text_width + 1.5 * left_padding > width);
        let outside_right =
            !t.vert && text_width > (label_end_x - label_start_x).abs() && !outside_left;

        let label_class = if outside_left {
            format!("{base_classes}taskTextOutsideLeft taskTextOutside{sec_num}")
        } else if outside_right {
            format!(
                "{base_classes}taskTextOutsideRight taskTextOutside{sec_num} width-{text_width}"
            )
        } else {
            format!("{base_classes}taskText taskText{sec_num} width-{text_width}")
        };

        let label = GanttTaskLabelLayout {
            id: format!("{}-text", t.id),
            text: t.task.clone(),
            font_size,
            width: text_width,
            x: label_x,
            y: label_y,
            class: label_class.trim().to_string(),
        };

        tasks.push(GanttTaskLayout {
            id: t.id.clone(),
            task: t.task.clone(),
            section: t.section.clone(),
            task_type: t.task_type.clone(),
            order: t.order,
            start_ms: t.start_ms,
            end_ms: t.end_ms,
            render_end_ms: t.render_end_ms,
            milestone: t.milestone,
            vert: t.vert,
            bar,
            label,
        });
    }

    // Section titles.
    let mut section_titles: Vec<GanttSectionTitleLayout> = Vec::new();
    let mut prev_gap: i64 = 0;
    for (idx, (sec, h)) in category_heights.iter().enumerate() {
        let lines = DeterministicTextMeasurer::normalized_text_lines(sec);
        let dy_em = -((lines.len().saturating_sub(1)) as f64) / 2.0;

        let mut sec_num = 0_i64;
        for (j, c) in categories.iter().enumerate() {
            if sec == c {
                sec_num = (j as i64) % number_section_styles;
            }
        }

        let y = if idx == 0 {
            (*h as f64 * gap) / 2.0 + top_padding
        } else {
            prev_gap += category_heights[idx - 1].1;
            (*h as f64 * gap) / 2.0 + prev_gap as f64 * gap + top_padding
        };

        section_titles.push(GanttSectionTitleLayout {
            section: sec.clone(),
            index: idx as i64,
            x: 10.0,
            y,
            dy_em,
            lines,
            class: format!("sectionTitle sectionTitle{sec_num}"),
        });
    }

    let axis_format = axis_format_to_strftime(&m.axis_format, &m.date_format, cfg_axis_format);
    let tick_interval = m.tick_interval.as_deref();
    let bottom_ticks = if has_tasks {
        build_ticks(
            min_ms,
            max_ms,
            range,
            left_padding,
            &axis_format,
            tick_interval,
        )
    } else {
        Vec::new()
    };
    let top_axis_enabled = m.top_axis || cfg_top_axis;
    let top_ticks = if has_tasks && top_axis_enabled {
        build_ticks(
            min_ms,
            max_ms,
            range,
            left_padding,
            &axis_format,
            tick_interval,
        )
    } else {
        Vec::new()
    };

    let bounds = Some(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: width,
        max_y: height,
    });

    Ok(GanttDiagramLayout {
        bounds,
        width,
        height,
        left_padding,
        right_padding,
        top_padding,
        grid_line_start_padding,
        bar_height,
        bar_gap,
        title_top_margin,
        font_size,
        section_font_size,
        number_section_styles,
        display_mode: if m.display_mode.is_empty() {
            cfg_display_mode
        } else {
            m.display_mode.clone()
        },
        date_format: m.date_format.clone(),
        axis_format: m.axis_format.clone(),
        tick_interval: m.tick_interval.clone(),
        top_axis: top_axis_enabled,
        today_marker: m.today_marker.clone(),
        categories,
        rows,
        section_titles,
        tasks,
        excludes: excludes_layout,
        bottom_ticks,
        top_ticks,
        title: m.title.clone(),
        title_x: width / 2.0,
        title_y: title_top_margin,
    })
}
