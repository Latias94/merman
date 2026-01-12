use crate::sanitize::sanitize_text;
use crate::{Error, ParseMetadata, Result};
use serde_json::{Value, json};

#[derive(Debug, Clone)]
enum AxisData {
    Band {
        title: String,
        categories: Vec<String>,
    },
    Linear {
        title: String,
        min: f64,
        max: f64,
    },
}

#[derive(Debug, Clone)]
struct Plot {
    plot_type: &'static str,
    values: Vec<f64>,
    data: Vec<(String, f64)>,
}

#[derive(Debug, Clone)]
struct XyChartState {
    orientation: String,
    x_axis: AxisData,
    y_axis: AxisData,
    plots: Vec<Plot>,
    has_set_x_axis: bool,
    has_set_y_axis: bool,
}

impl XyChartState {
    fn new(meta: &ParseMetadata) -> Self {
        let orientation = meta
            .effective_config
            .get_str("xyChart.chartOrientation")
            .unwrap_or("vertical")
            .to_string();
        Self {
            orientation,
            x_axis: AxisData::Band {
                title: String::new(),
                categories: Vec::new(),
            },
            y_axis: AxisData::Linear {
                title: String::new(),
                min: f64::INFINITY,
                max: f64::NEG_INFINITY,
            },
            plots: Vec::new(),
            has_set_x_axis: false,
            has_set_y_axis: false,
        }
    }

    fn set_orientation(&mut self, o: &str) {
        if o.eq_ignore_ascii_case("horizontal") {
            self.orientation = "horizontal".to_string();
        } else {
            self.orientation = "vertical".to_string();
        }
    }

    fn set_x_axis_title(&mut self, title: &str, meta: &ParseMetadata) {
        let t = sanitize_text(title.trim(), &meta.effective_config);
        match &mut self.x_axis {
            AxisData::Band { title, .. } => *title = t,
            AxisData::Linear { title, .. } => *title = t,
        }
    }

    fn set_y_axis_title(&mut self, title: &str, meta: &ParseMetadata) {
        let t = sanitize_text(title.trim(), &meta.effective_config);
        match &mut self.y_axis {
            AxisData::Linear { title, .. } => *title = t,
            AxisData::Band { title, .. } => *title = t,
        }
    }

    fn set_x_axis_range(&mut self, min: f64, max: f64) {
        let title = match &self.x_axis {
            AxisData::Band { title, .. } => title.clone(),
            AxisData::Linear { title, .. } => title.clone(),
        };
        self.x_axis = AxisData::Linear { title, min, max };
        self.has_set_x_axis = true;
    }

    fn set_x_axis_band(&mut self, categories: Vec<String>, meta: &ParseMetadata) {
        let title = match &self.x_axis {
            AxisData::Band { title, .. } => title.clone(),
            AxisData::Linear { title, .. } => title.clone(),
        };
        let categories = categories
            .into_iter()
            .map(|c| sanitize_text(c.trim(), &meta.effective_config))
            .collect::<Vec<_>>();
        self.x_axis = AxisData::Band { title, categories };
        self.has_set_x_axis = true;
    }

    fn set_y_axis_range(&mut self, min: f64, max: f64) {
        let title = match &self.y_axis {
            AxisData::Linear { title, .. } => title.clone(),
            AxisData::Band { title, .. } => title.clone(),
        };
        self.y_axis = AxisData::Linear { title, min, max };
        self.has_set_y_axis = true;
    }

    fn set_y_axis_range_from_plot_data(&mut self, data: &[f64]) {
        let min_value = data.iter().copied().fold(f64::INFINITY, |a, b| a.min(b));
        let max_value = data
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, |a, b| a.max(b));

        let (prev_min, prev_max, title) = match &self.y_axis {
            AxisData::Linear { min, max, title } => (*min, *max, title.clone()),
            AxisData::Band { title, .. } => (f64::INFINITY, f64::NEG_INFINITY, title.clone()),
        };

        self.y_axis = AxisData::Linear {
            title,
            min: prev_min.min(min_value),
            max: prev_max.max(max_value),
        };
    }

    fn transform_data_without_category(&mut self, data: &[f64]) -> Vec<(String, f64)> {
        if data.is_empty() {
            return Vec::new();
        }

        if !self.has_set_x_axis {
            let (prev_min, prev_max) = match &self.x_axis {
                AxisData::Linear { min, max, .. } => (*min, *max),
                AxisData::Band { .. } => (f64::INFINITY, f64::NEG_INFINITY),
            };
            self.set_x_axis_range(prev_min.min(1.0), prev_max.max(data.len() as f64));
        }

        if !self.has_set_y_axis {
            self.set_y_axis_range_from_plot_data(data);
        }

        match &self.x_axis {
            AxisData::Band { categories, .. } => categories
                .iter()
                .enumerate()
                .filter_map(|(i, c)| data.get(i).copied().map(|v| (c.clone(), v)))
                .collect(),
            AxisData::Linear { min, max, .. } => {
                let denom = (data.len() as f64) - 1.0;
                let step = (*max - *min) / denom;
                let mut cats = Vec::new();
                let mut i = *min;
                while i <= *max {
                    cats.push(format!("{i}"));
                    i += step;
                    if denom == 0.0 {
                        break;
                    }
                }
                cats.into_iter()
                    .enumerate()
                    .filter_map(|(idx, c)| data.get(idx).copied().map(|v| (c, v)))
                    .collect()
            }
        }
    }

    fn add_line_data(&mut self, data: Vec<f64>) {
        let pairs = self.transform_data_without_category(&data);
        self.plots.push(Plot {
            plot_type: "line",
            values: data,
            data: pairs,
        });
    }

    fn add_bar_data(&mut self, data: Vec<f64>) {
        let pairs = self.transform_data_without_category(&data);
        self.plots.push(Plot {
            plot_type: "bar",
            values: data,
            data: pairs,
        });
    }
}

pub fn parse_xychart(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let cleaned = strip_comments(code);
    let statements = split_statements(&cleaned);

    let mut it = statements.into_iter().filter(|s| !s.trim().is_empty());
    let Some(header_stmt) = it.next() else {
        return Ok(json!({}));
    };

    let mut state = XyChartState::new(meta);
    parse_header(&header_stmt, &mut state)?;

    let mut title: Option<String> = None;
    let mut acc_title: Option<String> = None;
    let mut acc_descr: Option<String> = None;

    for stmt in it {
        let stmt = stmt.trim();
        if stmt.is_empty() {
            continue;
        }

        if let Some(rest) = strip_keyword(stmt, "title") {
            let t = parse_text(rest)?;
            title = Some(t.trim().to_string());
            continue;
        }
        if let Some(rest) = strip_keyword(stmt, "accTitle") {
            let v = rest.trim_start();
            let v = v.strip_prefix(':').unwrap_or(v).trim();
            acc_title = Some(v.to_string());
            continue;
        }
        if let Some(rest) = strip_keyword(stmt, "accDescr") {
            let rest = rest.trim_start();
            if let Some(v) = rest.strip_prefix(':') {
                acc_descr = Some(v.trim().to_string());
                continue;
            }
            if let Some(after) = rest.strip_prefix('{') {
                let Some(end) = after.find('}') else {
                    return Err(Error::DiagramParse {
                        diagram_type: "xychart".to_string(),
                        message: "unterminated accDescr block".to_string(),
                    });
                };
                acc_descr = Some(after[..end].trim().to_string());
                continue;
            }
        }

        if let Some(rest) = strip_keyword(stmt, "x-axis") {
            parse_x_axis(rest, &mut state, meta)?;
            continue;
        }
        if let Some(rest) = strip_keyword(stmt, "y-axis") {
            parse_y_axis(rest, &mut state, meta)?;
            continue;
        }
        if let Some(rest) = strip_keyword(stmt, "line") {
            let (_plot_title, data) = parse_plot_stmt(rest)?;
            state.add_line_data(data);
            continue;
        }
        if let Some(rest) = strip_keyword(stmt, "bar") {
            let (_plot_title, data) = parse_plot_stmt(rest)?;
            state.add_bar_data(data);
            continue;
        }

        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: format!("unexpected xychart statement: {stmt}"),
        });
    }

    Ok(json!({
        "type": meta.diagram_type,
        "title": title,
        "accTitle": acc_title,
        "accDescr": acc_descr,
        "orientation": state.orientation,
        "xAxis": axis_to_value(&state.x_axis),
        "yAxis": axis_to_value(&state.y_axis),
        "plots": state.plots.iter().map(|p| {
            json!({
                "type": p.plot_type,
                "values": p.values,
                "data": p.data.iter().map(|(x,y)| json!([x, y])).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "config": meta.effective_config.as_value().clone(),
    }))
}

fn axis_to_value(axis: &AxisData) -> Value {
    match axis {
        AxisData::Band { title, categories } => json!({
            "type": "band",
            "title": title,
            "categories": categories,
        }),
        AxisData::Linear { title, min, max } => {
            let min = if min.is_finite() {
                json!(min)
            } else {
                Value::Null
            };
            let max = if max.is_finite() {
                json!(max)
            } else {
                Value::Null
            };
            json!({
                "type": "linear",
                "title": title,
                "min": min,
                "max": max,
            })
        }
    }
}

fn parse_header(stmt: &str, state: &mut XyChartState) -> Result<()> {
    let t = stmt.trim();
    let lower = t.to_ascii_lowercase();
    let (prefix, rest) = if lower.starts_with("xychart-beta") {
        ("xychart-beta", &t["xychart-beta".len()..])
    } else if lower.starts_with("xychart") {
        ("xychart", &t["xychart".len()..])
    } else {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "expected xychart".to_string(),
        });
    };

    let rem = rest.trim();
    if rem.is_empty() {
        return Ok(());
    }
    if !rest.starts_with(char::is_whitespace) {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: format!("unexpected token after {prefix}: {rem}"),
        });
    }

    if rem.eq_ignore_ascii_case("vertical") || rem.eq_ignore_ascii_case("horizontal") {
        state.set_orientation(rem);
        return Ok(());
    }

    Err(Error::DiagramParse {
        diagram_type: "xychart".to_string(),
        message: format!("invalid chart orientation: {rem}"),
    })
}

fn strip_keyword<'a>(stmt: &'a str, kw: &str) -> Option<&'a str> {
    let s = stmt.trim_start();
    let lower = s.to_ascii_lowercase();
    let kw_lower = kw.to_ascii_lowercase();
    if !lower.starts_with(&kw_lower) {
        return None;
    }
    let rest = &s[kw.len()..];
    if !rest.is_empty()
        && !rest.starts_with(char::is_whitespace)
        && kw != "accTitle"
        && kw != "accDescr"
    {
        return None;
    }
    Some(rest)
}

fn parse_text(input: &str) -> Result<String> {
    let t = input.trim_start();
    if t.starts_with("\"`") {
        let body = &t[2..];
        let Some(end) = body.find("`\"") else {
            return Err(Error::DiagramParse {
                diagram_type: "xychart".to_string(),
                message: "unterminated markdown string".to_string(),
            });
        };
        let s = &body[..end];
        let rest = &body[end + 2..];
        if !rest.trim().is_empty() {
            return Err(Error::DiagramParse {
                diagram_type: "xychart".to_string(),
                message: "unexpected trailing tokens after text".to_string(),
            });
        }
        return Ok(s.to_string());
    }

    if t.starts_with('"') {
        let body = &t[1..];
        let Some(end) = body.find('"') else {
            return Err(Error::DiagramParse {
                diagram_type: "xychart".to_string(),
                message: "unterminated string".to_string(),
            });
        };
        let s = &body[..end];
        let rest = &body[end + 1..];
        if !rest.trim().is_empty() {
            return Err(Error::DiagramParse {
                diagram_type: "xychart".to_string(),
                message: "unexpected trailing tokens after text".to_string(),
            });
        }
        return Ok(s.to_string());
    }

    let mut out = String::new();
    for part in t.split_whitespace() {
        out.push_str(part);
    }
    if out.is_empty() {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "expected text".to_string(),
        });
    }
    Ok(out)
}

fn parse_number(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    // Accept +, -, integers, decimals, and leading dot decimals.
    let ok = t
        .chars()
        .all(|c| c.is_ascii_digit() || c == '+' || c == '-' || c == '.');
    if !ok {
        return None;
    }
    t.parse::<f64>().ok()
}

fn parse_x_axis(rest: &str, state: &mut XyChartState, meta: &ParseMetadata) -> Result<()> {
    let t = rest.trim();
    if t.is_empty() {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "x-axis requires data".to_string(),
        });
    }

    if t.starts_with('[') {
        state.set_x_axis_title("", meta);
        let cats = parse_text_list_in_brackets(t)?;
        state.set_x_axis_band(cats, meta);
        return Ok(());
    }

    if let Some((min, max)) = try_parse_range(t)? {
        state.set_x_axis_title("", meta);
        state.set_x_axis_range(min, max);
        return Ok(());
    }

    let (title, tail) = parse_text_prefix(t)?;
    state.set_x_axis_title(&title, meta);
    let tail = tail.trim_start();
    if tail.is_empty() {
        return Ok(());
    }
    if tail.starts_with('[') {
        let cats = parse_text_list_in_brackets(tail)?;
        state.set_x_axis_band(cats, meta);
        return Ok(());
    }
    if let Some((min, max)) = try_parse_range(tail)? {
        state.set_x_axis_range(min, max);
        return Ok(());
    }

    Err(Error::DiagramParse {
        diagram_type: "xychart".to_string(),
        message: "invalid x-axis data".to_string(),
    })
}

fn parse_y_axis(rest: &str, state: &mut XyChartState, meta: &ParseMetadata) -> Result<()> {
    let t = rest.trim();
    if t.is_empty() {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "y-axis requires data".to_string(),
        });
    }

    if let Some((min, max)) = try_parse_range(t)? {
        state.set_y_axis_title("", meta);
        state.set_y_axis_range(min, max);
        return Ok(());
    }

    if t.starts_with('[') {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "y-axis does not support band data".to_string(),
        });
    }

    let (title, tail) = parse_text_prefix(t)?;
    state.set_y_axis_title(&title, meta);
    let tail = tail.trim_start();
    if tail.is_empty() {
        return Ok(());
    }

    if let Some((min, max)) = try_parse_range(tail)? {
        state.set_y_axis_range(min, max);
        return Ok(());
    }

    Err(Error::DiagramParse {
        diagram_type: "xychart".to_string(),
        message: "invalid y-axis data".to_string(),
    })
}

fn parse_plot_stmt(rest: &str) -> Result<(String, Vec<f64>)> {
    let t = rest.trim();
    if t.is_empty() {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "plot requires data".to_string(),
        });
    }

    if t.starts_with('[') {
        let data = parse_number_list_in_brackets(t)?;
        if data.is_empty() {
            return Err(Error::DiagramParse {
                diagram_type: "xychart".to_string(),
                message: "plot data cannot be empty".to_string(),
            });
        }
        return Ok((String::new(), data));
    }

    let (title, tail) = parse_text_prefix(t)?;
    let tail = tail.trim_start();
    if !tail.starts_with('[') {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "plot data missing".to_string(),
        });
    }
    let data = parse_number_list_in_brackets(tail)?;
    if data.is_empty() {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "plot data cannot be empty".to_string(),
        });
    }
    Ok((title, data))
}

fn try_parse_range(input: &str) -> Result<Option<(f64, f64)>> {
    let mut s = input.trim_start();
    let Some((a_str, tail)) = take_number_token(s) else {
        return Ok(None);
    };
    s = tail.trim_start();
    if !s.starts_with("-->") {
        return Ok(None);
    }
    s = &s[3..];
    let Some((b_str, tail)) = take_number_token(s.trim_start()) else {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "expected number".to_string(),
        });
    };
    if !tail.trim().is_empty() {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "unexpected trailing tokens after range".to_string(),
        });
    }
    let a = parse_number(&a_str).ok_or_else(|| Error::DiagramParse {
        diagram_type: "xychart".to_string(),
        message: "invalid number".to_string(),
    })?;
    let b = parse_number(&b_str).ok_or_else(|| Error::DiagramParse {
        diagram_type: "xychart".to_string(),
        message: "invalid number".to_string(),
    })?;
    Ok(Some((a, b)))
}

fn take_number_token(input: &str) -> Option<(String, &str)> {
    let mut idx = 0usize;
    for (i, ch) in input.char_indices() {
        if i == 0 && (ch == '+' || ch == '-') {
            idx = i + ch.len_utf8();
            continue;
        }
        if ch.is_ascii_digit() || ch == '.' {
            idx = i + ch.len_utf8();
            continue;
        }
        break;
    }
    if idx == 0 {
        return None;
    }
    Some((input[..idx].to_string(), &input[idx..]))
}

fn parse_text_prefix(input: &str) -> Result<(String, &str)> {
    let t = input.trim_start();
    if t.starts_with("\"`") {
        let body = &t[2..];
        let Some(end) = body.find("`\"") else {
            return Err(Error::DiagramParse {
                diagram_type: "xychart".to_string(),
                message: "unterminated markdown string".to_string(),
            });
        };
        let s = &body[..end];
        let rest = &body[end + 2..];
        return Ok((s.to_string(), rest));
    }
    if t.starts_with('"') {
        let body = &t[1..];
        let Some(end) = body.find('"') else {
            return Err(Error::DiagramParse {
                diagram_type: "xychart".to_string(),
                message: "unterminated string".to_string(),
            });
        };
        let s = &body[..end];
        let rest = &body[end + 1..];
        return Ok((s.to_string(), rest));
    }
    let mut end = t.len();
    for (i, ch) in t.char_indices() {
        if ch.is_whitespace() || ch == '[' {
            end = i;
            break;
        }
    }
    let head = &t[..end];
    if head.is_empty() {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "expected text".to_string(),
        });
    }
    Ok((head.to_string(), &t[end..]))
}

fn parse_text_list_in_brackets(input: &str) -> Result<Vec<String>> {
    let t = input.trim_start();
    let inner = extract_bracket_inner(t)?;
    let parts = split_top_level_commas(inner);
    let mut out = Vec::new();
    for p in parts {
        let p = p.trim();
        if p.is_empty() {
            return Err(Error::DiagramParse {
                diagram_type: "xychart".to_string(),
                message: "empty category".to_string(),
            });
        }
        out.push(parse_text(p)?);
    }
    Ok(out)
}

fn parse_number_list_in_brackets(input: &str) -> Result<Vec<f64>> {
    let t = input.trim_start();
    let inner = extract_bracket_inner(t)?;
    let parts = split_top_level_commas(inner);
    let mut out = Vec::new();
    for p in parts {
        let p = p.trim();
        if p.is_empty() {
            return Err(Error::DiagramParse {
                diagram_type: "xychart".to_string(),
                message: "empty number".to_string(),
            });
        }
        let n = parse_number(p).ok_or_else(|| Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: format!("invalid number: {p}"),
        })?;
        out.push(n);
    }
    Ok(out)
}

fn extract_bracket_inner(input: &str) -> Result<&str> {
    let t = input.trim_start();
    if !t.starts_with('[') {
        return Err(Error::DiagramParse {
            diagram_type: "xychart".to_string(),
            message: "expected '['".to_string(),
        });
    }
    let mut in_quote = false;
    let mut in_md = false;
    let mut idx = 1usize;
    while idx < t.len() {
        let ch = t.as_bytes()[idx] as char;
        if in_md {
            if t[idx..].starts_with("`\"") {
                in_md = false;
                idx += 2;
                continue;
            }
            idx += 1;
            continue;
        }
        if in_quote {
            if ch == '"' {
                in_quote = false;
            }
            idx += 1;
            continue;
        }
        if t[idx..].starts_with("\"`") {
            in_md = true;
            idx += 2;
            continue;
        }
        if ch == '"' {
            in_quote = true;
            idx += 1;
            continue;
        }
        if ch == '[' {
            return Err(Error::DiagramParse {
                diagram_type: "xychart".to_string(),
                message: "unbalanced '['".to_string(),
            });
        }
        if ch == ']' {
            let inner = &t[1..idx];
            let rest = &t[idx + 1..];
            if !rest.trim().is_empty() {
                return Err(Error::DiagramParse {
                    diagram_type: "xychart".to_string(),
                    message: "unexpected trailing tokens after ']'".to_string(),
                });
            }
            return Ok(inner);
        }
        idx += 1;
    }

    Err(Error::DiagramParse {
        diagram_type: "xychart".to_string(),
        message: "unbalanced ']'".to_string(),
    })
}

fn split_top_level_commas(input: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_quote = false;
    let mut in_md = false;
    let mut start = 0usize;
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if in_md {
            if input[i..].starts_with("`\"") {
                in_md = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if in_quote {
            if ch == '"' {
                in_quote = false;
            }
            i += 1;
            continue;
        }
        if input[i..].starts_with("\"`") {
            in_md = true;
            i += 2;
            continue;
        }
        if ch == '"' {
            in_quote = true;
            i += 1;
            continue;
        }
        if ch == ',' {
            out.push(&input[start..i]);
            start = i + 1;
            i += 1;
            continue;
        }
        i += 1;
    }
    out.push(&input[start..]);
    out
}

fn strip_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.split_inclusive('\n') {
        let mut in_quote = false;
        let mut chars = line.char_indices().peekable();
        let mut cut = line.len();
        while let Some((idx, ch)) = chars.next() {
            if in_quote {
                if ch == '"' {
                    in_quote = false;
                }
                continue;
            }
            if ch == '"' {
                in_quote = true;
                continue;
            }
            if ch == '%' && chars.peek().is_some_and(|(_, n)| *n == '%') {
                cut = idx;
                break;
            }
        }
        let kept = &line[..cut];
        if kept.trim_start().starts_with("%%{") {
            continue;
        }
        if kept.trim_start().starts_with("%%") {
            continue;
        }
        out.push_str(kept);
    }
    out
}

fn split_statements(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    let mut in_md = false;
    let mut bracket_depth = 0i64;
    let mut brace_depth = 0i64;

    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_md {
            cur.push(ch);
            if ch == '`' && chars.peek() == Some(&'"') {
                cur.push('"');
                chars.next();
                in_md = false;
            }
            continue;
        }
        if in_quote {
            cur.push(ch);
            if ch == '"' {
                in_quote = false;
            }
            continue;
        }

        if ch == '"' && chars.peek() == Some(&'`') {
            cur.push('"');
            cur.push('`');
            chars.next();
            in_md = true;
            continue;
        }

        if ch == '"' {
            cur.push(ch);
            in_quote = true;
            continue;
        }

        match ch {
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            _ => {}
        }

        if (ch == '\n' || ch == ';') && bracket_depth == 0 && brace_depth == 0 {
            out.push(std::mem::take(&mut cur));
            continue;
        }

        cur.push(ch);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ParseOptions};
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> String {
        let engine = Engine::new();
        match block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err() {
            Error::DiagramParse { message, .. } => message,
            other => other.to_string(),
        }
    }

    #[test]
    fn xychart_header_only_is_accepted() {
        let model = parse("xychart");
        assert_eq!(model["plots"], json!([]));
    }

    #[test]
    fn xychart_invalid_header_throws() {
        let err = parse_err("xychart-1");
        assert!(err.contains("unexpected"));
    }

    #[test]
    fn xychart_orientation_is_parsed() {
        let model = parse("xychart horizontal");
        assert_eq!(model["orientation"], json!("horizontal"));
    }

    #[test]
    fn xychart_orientation_invalid_throws() {
        let err = parse_err("xychart abc");
        assert!(err.contains("invalid chart orientation"));
    }

    #[test]
    fn xychart_title_parses_quoted_and_unquoted() {
        let model = parse("xychart\ntitle \"This is a title\"");
        assert_eq!(model["title"], json!("This is a title"));

        let model = parse("xychart\ntitle oneLinertitle");
        assert_eq!(model["title"], json!("oneLinertitle"));
    }

    #[test]
    fn xychart_parses_axis_band_and_range_and_plots() {
        let model = parse(
            r#"xychart horizontal
title "Basic xychart"
x-axis "this is x axis" [category1, "category 2", category3]
y-axis yaxisText 10 --> 150
bar barTitle1 [23, 45, 56.6]
line lineTitle1 [11, 45.5, 67, 23]
"#,
        );
        assert_eq!(model["orientation"], json!("horizontal"));
        assert_eq!(model["xAxis"]["type"], json!("band"));
        assert_eq!(
            model["xAxis"]["categories"],
            json!(["category1", "category 2", "category3"])
        );
        assert_eq!(model["yAxis"]["min"], json!(10.0));
        assert_eq!(model["yAxis"]["max"], json!(150.0));
        assert_eq!(model["plots"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn xychart_plot_requires_nonempty_data() {
        let err = parse_err("xychart\nline \"t\" [ ]");
        assert!(err.contains("empty"));
        let err = parse_err("xychart\nline \"t\"");
        assert!(err.contains("missing") || err.contains("requires"));
    }
}
