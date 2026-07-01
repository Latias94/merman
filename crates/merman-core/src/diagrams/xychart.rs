use crate::diagrams::scan::strip_line_ending;
use crate::sanitize::sanitize_text;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, MermaidConfig, ParseMetadata, Result, SourceSpan,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value, json};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum XyChartAxisRenderModel {
    #[serde(rename = "band")]
    Band {
        #[serde(default)]
        title: String,
        #[serde(default)]
        categories: Vec<String>,
    },
    #[serde(rename = "linear")]
    Linear {
        #[serde(default)]
        title: String,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum XyChartPlotType {
    #[serde(rename = "line")]
    Line,
    #[serde(rename = "bar")]
    Bar,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XyChartPlotRenderModel {
    #[serde(rename = "type")]
    pub plot_type: XyChartPlotType,
    #[serde(default)]
    pub title: Option<String>,
    pub values: Vec<f64>,
    pub data: Vec<(String, Option<f64>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct XyChartAxisDisplayPolicy {
    pub show_label: bool,
    pub show_title: bool,
    pub show_tick: bool,
    pub show_axis_line: bool,
}

impl Default for XyChartAxisDisplayPolicy {
    fn default() -> Self {
        Self {
            show_label: true,
            show_title: true,
            show_tick: true,
            show_axis_line: true,
        }
    }
}

impl XyChartAxisDisplayPolicy {
    fn from_config(config: &MermaidConfig, axis_key: &str) -> Self {
        let default = Self::default();
        Self {
            show_label: config
                .get_bool(&format!("xyChart.{axis_key}.showLabel"))
                .unwrap_or(default.show_label),
            show_title: config
                .get_bool(&format!("xyChart.{axis_key}.showTitle"))
                .unwrap_or(default.show_title),
            show_tick: config
                .get_bool(&format!("xyChart.{axis_key}.showTick"))
                .unwrap_or(default.show_tick),
            show_axis_line: config
                .get_bool(&format!("xyChart.{axis_key}.showAxisLine"))
                .unwrap_or(default.show_axis_line),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct XyChartDisplayPolicy {
    pub show_title: bool,
    pub show_data_label: bool,
    pub show_data_label_outside_bar: bool,
    pub x_axis: XyChartAxisDisplayPolicy,
    pub y_axis: XyChartAxisDisplayPolicy,
}

impl Default for XyChartDisplayPolicy {
    fn default() -> Self {
        Self {
            show_title: true,
            show_data_label: false,
            show_data_label_outside_bar: false,
            x_axis: XyChartAxisDisplayPolicy::default(),
            y_axis: XyChartAxisDisplayPolicy::default(),
        }
    }
}

impl XyChartDisplayPolicy {
    fn from_config(config: &MermaidConfig) -> Self {
        let default = Self::default();
        Self {
            show_title: config
                .get_bool("xyChart.showTitle")
                .unwrap_or(default.show_title),
            show_data_label: config
                .get_bool("xyChart.showDataLabel")
                .unwrap_or(default.show_data_label),
            show_data_label_outside_bar: config
                .get_bool("xyChart.showDataLabelOutsideBar")
                .unwrap_or(default.show_data_label_outside_bar),
            x_axis: XyChartAxisDisplayPolicy::from_config(config, "xAxis"),
            y_axis: XyChartAxisDisplayPolicy::from_config(config, "yAxis"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XyChartDiagramRenderModel {
    #[serde(default)]
    pub orientation: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(rename = "xAxis")]
    pub x_axis: XyChartAxisRenderModel,
    #[serde(rename = "yAxis")]
    pub y_axis: XyChartAxisRenderModel,
    #[serde(default)]
    pub plots: Vec<XyChartPlotRenderModel>,
    #[serde(skip, default)]
    pub display: XyChartDisplayPolicy,
}

impl XyChartDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }

    pub(crate) fn to_compat_json(&self, meta: &ParseMetadata) -> Value {
        let mut out = Map::with_capacity(10);
        out.insert(
            "orientation".to_string(),
            Value::String(self.orientation.clone()),
        );
        out.insert("title".to_string(), option_string_value(&self.title));
        out.insert("accTitle".to_string(), option_string_value(&self.acc_title));
        out.insert("accDescr".to_string(), option_string_value(&self.acc_descr));
        out.insert("xAxis".to_string(), axis_value(&self.x_axis));
        out.insert("yAxis".to_string(), axis_value(&self.y_axis));
        out.insert("plots".to_string(), plots_value(&self.plots));
        out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
        out.insert(
            "config".to_string(),
            crate::config::clone_value_nonrecursive(meta.effective_config.as_value()),
        );
        Value::Object(out)
    }
}

fn option_string_value(value: &Option<String>) -> Value {
    value
        .as_ref()
        .map(|value| Value::String(value.clone()))
        .unwrap_or(Value::Null)
}

fn optional_f64_value(value: Option<f64>) -> Value {
    value
        .and_then(Number::from_f64)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn f64_value(value: f64) -> Value {
    Number::from_f64(value)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn string_array_value(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}

fn axis_value(axis: &XyChartAxisRenderModel) -> Value {
    let mut out = Map::new();
    match axis {
        XyChartAxisRenderModel::Band { title, categories } => {
            out.insert("type".to_string(), Value::String("band".to_string()));
            out.insert("title".to_string(), Value::String(title.clone()));
            out.insert("categories".to_string(), string_array_value(categories));
        }
        XyChartAxisRenderModel::Linear { title, min, max } => {
            out.insert("type".to_string(), Value::String("linear".to_string()));
            out.insert("title".to_string(), Value::String(title.clone()));
            out.insert("min".to_string(), optional_f64_value(*min));
            out.insert("max".to_string(), optional_f64_value(*max));
        }
    }
    Value::Object(out)
}

fn plots_value(plots: &[XyChartPlotRenderModel]) -> Value {
    Value::Array(plots.iter().map(plot_value).collect())
}

fn plot_value(plot: &XyChartPlotRenderModel) -> Value {
    let mut out = Map::new();
    out.insert(
        "type".to_string(),
        Value::String(plot_type_name(plot.plot_type)),
    );
    out.insert(
        "values".to_string(),
        Value::Array(plot.values.iter().copied().map(f64_value).collect()),
    );
    out.insert("data".to_string(), plot_data_value(&plot.data));
    Value::Object(out)
}

fn plot_type_name(plot_type: XyChartPlotType) -> String {
    match plot_type {
        XyChartPlotType::Line => "line".to_string(),
        XyChartPlotType::Bar => "bar".to_string(),
    }
}

fn plot_data_value(data: &[(String, Option<f64>)]) -> Value {
    Value::Array(
        data.iter()
            .map(|(category, value)| {
                Value::Array(vec![
                    Value::String(category.clone()),
                    optional_f64_value(*value),
                ])
            })
            .collect(),
    )
}

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
    plot_type: XyChartPlotType,
    title: Option<String>,
    values: Vec<f64>,
    data: Vec<(String, Option<f64>)>,
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

    fn transform_data_without_category(&mut self, data: &[f64]) -> Vec<(String, Option<f64>)> {
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
                .map(|(i, c)| (c.clone(), data.get(i).copied()))
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
                    .map(|(idx, c)| (c, data.get(idx).copied()))
                    .collect()
            }
        }
    }

    fn add_line_data(&mut self, title: Option<String>, data: Vec<f64>) {
        let pairs = self.transform_data_without_category(&data);
        self.plots.push(Plot {
            plot_type: XyChartPlotType::Line,
            title,
            values: data,
            data: pairs,
        });
    }

    fn add_bar_data(&mut self, title: Option<String>, data: Vec<f64>) {
        let pairs = self.transform_data_without_category(&data);
        self.plots.push(Plot {
            plot_type: XyChartPlotType::Bar,
            title,
            values: data,
            data: pairs,
        });
    }

    fn into_render_model(
        self,
        title: Option<String>,
        acc_title: Option<String>,
        acc_descr: Option<String>,
        meta: &ParseMetadata,
    ) -> XyChartDiagramRenderModel {
        XyChartDiagramRenderModel {
            orientation: self.orientation,
            title,
            acc_title,
            acc_descr,
            x_axis: axis_data_to_render_model(self.x_axis),
            y_axis: axis_data_to_render_model(self.y_axis),
            plots: self
                .plots
                .into_iter()
                .map(|p| XyChartPlotRenderModel {
                    plot_type: p.plot_type,
                    title: p.title,
                    values: p.values,
                    data: p.data,
                })
                .collect(),
            display: XyChartDisplayPolicy::from_config(&meta.effective_config),
        }
    }
}

pub fn parse_xychart(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let Some(model) = parse_xychart_model(code, meta)? else {
        return Ok(json!({}));
    };

    Ok(model.to_compat_json(meta))
}

pub fn parse_xychart_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<XyChartDiagramRenderModel> {
    Ok(parse_xychart_model(code, meta)?.unwrap_or_else(empty_render_model))
}

pub fn parse_xychart_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut lines = code.split_inclusive('\n').peekable();
    let mut offset = 0usize;
    let mut header_seen = false;

    while let Some(segment) = lines.next() {
        let line_start = offset;
        offset += segment.len();
        let line = strip_line_ending(segment);
        let stripped = strip_inline_comment(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !header_seen {
            if let Some((prefix_len, _rest)) = header_token_len_and_rest(trimmed) {
                let header_rel = line.find(trimmed).unwrap_or(0);
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::Payload,
                    SourceSpan::new(
                        line_start + header_rel,
                        line_start + header_rel + prefix_len,
                    ),
                ));
                header_seen = true;
            }
            continue;
        }

        if let Some(rest) = strip_keyword(trimmed, "title") {
            if let Some(value) = parse_text_spanned(rest, line, line_start) {
                facts.push_directive_prefix("title");
                push_xychart_payload_fact(
                    &mut facts,
                    value.as_str(),
                    SourceSpan::new(value.start, value.end),
                    "xychart title",
                    EditorSemanticKind::String,
                );
            }
            continue;
        }
        if let Some(rest) = strip_keyword(trimmed, "accTitle") {
            if let Some(value) = parse_colon_value_spanned(rest, line, line_start) {
                facts.push_directive_prefix("accTitle");
                push_xychart_payload_fact(
                    &mut facts,
                    value.as_str(),
                    SourceSpan::new(value.start, value.end),
                    "xychart accessibility title",
                    EditorSemanticKind::String,
                );
            }
            continue;
        }
        if let Some(rest) = strip_keyword(trimmed, "accDescr") {
            if let Some(value) = parse_acc_descr_spanned(rest, line, line_start) {
                facts.push_directive_prefix("accDescr");
                push_xychart_payload_fact(
                    &mut facts,
                    value.as_str(),
                    SourceSpan::new(value.start, value.end),
                    "xychart accessibility description",
                    EditorSemanticKind::String,
                );
            }
            continue;
        }
        if let Some(rest) = strip_keyword(trimmed, "x-axis") {
            if let Some(value) = parse_axis_title_or_categories_spanned(rest, line, line_start) {
                push_xychart_payload_fact(
                    &mut facts,
                    value.as_str(),
                    SourceSpan::new(value.start, value.end),
                    "xychart x-axis",
                    EditorSemanticKind::String,
                );
            }
            continue;
        }
        if let Some(rest) = strip_keyword(trimmed, "y-axis") {
            if let Some(value) = parse_axis_title_or_categories_spanned(rest, line, line_start) {
                push_xychart_payload_fact(
                    &mut facts,
                    value.as_str(),
                    SourceSpan::new(value.start, value.end),
                    "xychart y-axis",
                    EditorSemanticKind::String,
                );
            }
            continue;
        }
        if let Some(rest) = strip_keyword(trimmed, "line") {
            push_xychart_plot_facts(&mut facts, rest, line, line_start, "xychart line");
            continue;
        }
        if let Some(rest) = strip_keyword(trimmed, "bar") {
            push_xychart_plot_facts(&mut facts, rest, line, line_start, "xychart bar");
            continue;
        }
    }

    facts
}

fn parse_xychart_model(
    code: &str,
    meta: &ParseMetadata,
) -> Result<Option<XyChartDiagramRenderModel>> {
    let statements = split_statements_spanned(code);

    let mut it = statements
        .into_iter()
        .filter(|statement| !statement.text.trim().is_empty());
    let Some(header_stmt) = it.next() else {
        return Ok(None);
    };

    let mut state = XyChartState::new(meta);
    parse_header(&header_stmt.text, &mut state)?;

    let mut title: Option<String> = None;
    let mut acc_title: Option<String> = None;
    let mut acc_descr: Option<String> = None;

    for stmt in it {
        let stmt_start = stmt.trimmed_start();
        let stmt = stmt.text.trim();
        if stmt.is_empty() {
            continue;
        }

        if let Some(rest) = strip_keyword(stmt, "title") {
            let t = parse_text(rest)?;
            title = Some(t.trim().to_string());
            continue;
        }
        if let Some(rest) = strip_keyword(stmt, "accTitle") {
            let rest = rest.trim_start();
            let Some(v) = rest.strip_prefix(':') else {
                return Err(Error::diagram_parse_fallback(
                    "xychart".to_string(),
                    "expected ':' after accTitle".to_string(),
                ));
            };
            acc_title = Some(v.trim().to_string());
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
                    return Err(Error::diagram_parse_fallback(
                        "xychart".to_string(),
                        "unterminated accDescr block".to_string(),
                    ));
                };
                let trailing = &after[end + 1..];
                if !trailing.trim().is_empty() {
                    return Err(Error::diagram_parse_fallback(
                        "xychart".to_string(),
                        "unexpected trailing tokens after accDescr block".to_string(),
                    ));
                }
                acc_descr = Some(after[..end].trim().to_string());
                continue;
            }
            return Err(Error::diagram_parse_fallback(
                "xychart".to_string(),
                "expected ':' or '{' after accDescr".to_string(),
            ));
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
            let rest_start = stmt_start + stmt.len().saturating_sub(rest.len());
            let (plot_title, data) = parse_plot_stmt_spanned(rest, rest_start)?;
            state.add_line_data(plot_title_value(&plot_title, meta), data);
            continue;
        }
        if let Some(rest) = strip_keyword(stmt, "bar") {
            let rest_start = stmt_start + stmt.len().saturating_sub(rest.len());
            let (plot_title, data) = parse_plot_stmt_spanned(rest, rest_start)?;
            state.add_bar_data(plot_title_value(&plot_title, meta), data);
            continue;
        }

        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            format!("unexpected xychart statement: {stmt}"),
        ));
    }

    Ok(Some(
        state.into_render_model(title, acc_title, acc_descr, meta),
    ))
}

fn plot_title_value(title: &str, meta: &ParseMetadata) -> Option<String> {
    let title = sanitize_text(title.trim(), &meta.effective_config);
    (!title.is_empty()).then_some(title)
}

fn empty_render_model() -> XyChartDiagramRenderModel {
    XyChartDiagramRenderModel {
        orientation: "vertical".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        x_axis: XyChartAxisRenderModel::Band {
            title: String::new(),
            categories: Vec::new(),
        },
        y_axis: XyChartAxisRenderModel::Linear {
            title: String::new(),
            min: None,
            max: None,
        },
        plots: Vec::new(),
        display: XyChartDisplayPolicy::default(),
    }
}

fn axis_data_to_render_model(axis: AxisData) -> XyChartAxisRenderModel {
    match axis {
        AxisData::Band { title, categories } => XyChartAxisRenderModel::Band { title, categories },
        AxisData::Linear { title, min, max } => {
            let min = min.is_finite().then_some(min);
            let max = max.is_finite().then_some(max);
            XyChartAxisRenderModel::Linear { title, min, max }
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
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "expected xychart".to_string(),
        ));
    };

    let rem = rest.trim();
    if rem.is_empty() {
        return Ok(());
    }
    if !rest.starts_with(char::is_whitespace) {
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            format!("unexpected token after {prefix}: {rem}"),
        ));
    }

    if rem.eq_ignore_ascii_case("vertical") || rem.eq_ignore_ascii_case("horizontal") {
        state.set_orientation(rem);
        return Ok(());
    }

    Err(Error::diagram_parse_fallback(
        "xychart".to_string(),
        format!("invalid chart orientation: {rem}"),
    ))
}

fn header_token_len_and_rest(stmt: &str) -> Option<(usize, &str)> {
    let t = stmt.trim_start();
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("xychart-beta") {
        return Some(("xychart-beta".len(), &t["xychart-beta".len()..]));
    }
    if lower.starts_with("xychart") {
        return Some(("xychart".len(), &t["xychart".len()..]));
    }
    None
}

fn strip_keyword<'a>(stmt: &'a str, kw: &str) -> Option<&'a str> {
    let s = stmt.trim_start();
    let lower = s.to_ascii_lowercase();
    let kw_lower = kw.to_ascii_lowercase();
    if !lower.starts_with(&kw_lower) {
        return None;
    }
    Some(&s[kw.len()..])
}

fn parse_text_spanned(input: &str, line: &str, line_start: usize) -> Option<SpannedText> {
    let (value, _tail) = parse_text_prefix(input).ok()?;
    let value_rel = line.find(&value)?;
    let start = line_start + value_rel;
    let len = value.len();
    Some(SpannedText {
        text: value,
        start,
        end: start + len,
    })
}

fn parse_colon_value_spanned(input: &str, line: &str, line_start: usize) -> Option<SpannedText> {
    let rest = input.trim_start();
    let rest = rest.strip_prefix(':')?;
    let value = rest.trim();
    if value.is_empty() {
        return None;
    }
    let value_rel = line.find(value)?;
    let start = line_start + value_rel;
    Some(SpannedText {
        text: value.to_string(),
        start,
        end: start + value.len(),
    })
}

fn parse_acc_descr_spanned(input: &str, line: &str, line_start: usize) -> Option<SpannedText> {
    let rest = input.trim_start();
    if let Some(v) = rest.strip_prefix(':') {
        let value = v.trim();
        if value.is_empty() {
            return None;
        }
        let value_rel = line.find(value)?;
        let start = line_start + value_rel;
        return Some(SpannedText {
            text: value.to_string(),
            start,
            end: start + value.len(),
        });
    }
    let after = rest.strip_prefix('{')?;
    let end = after.find('}')?;
    let value = after[..end].trim();
    if value.is_empty() {
        return None;
    }
    let value_rel = line.find(value)?;
    let start = line_start + value_rel;
    Some(SpannedText {
        text: value.to_string(),
        start,
        end: start + value.len(),
    })
}

fn parse_axis_title_or_categories_spanned(
    input: &str,
    line: &str,
    line_start: usize,
) -> Option<SpannedText> {
    let rest = input.trim_start();
    if rest.is_empty() {
        return None;
    }
    if rest.starts_with('[') {
        let start_rel = line.find('[')? + 1;
        let end_rel = line.rfind(']')?;
        if end_rel <= start_rel {
            return None;
        }
        let value = line[start_rel..end_rel].trim();
        if value.is_empty() {
            return None;
        }
        let value_rel = line.find(value)?;
        let start = line_start + value_rel;
        return Some(SpannedText {
            text: value.to_string(),
            start,
            end: start + value.len(),
        });
    }
    let (title, _tail) = parse_text_prefix(rest).ok()?;
    let title_rel = line.find(&title)?;
    let start = line_start + title_rel;
    let len = title.len();
    Some(SpannedText {
        text: title,
        start,
        end: start + len,
    })
}

fn push_xychart_payload_fact(
    facts: &mut EditorSemanticFacts,
    text: &str,
    span: SourceSpan,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.to_string(),
        Some(detail.to_string()),
        kind,
        span,
        span,
    ));
}

fn push_xychart_plot_facts(
    facts: &mut EditorSemanticFacts,
    input: &str,
    line: &str,
    line_start: usize,
    detail: &'static str,
) {
    if let Some(title) = parse_text_prefix_spanned(input, line, line_start) {
        push_xychart_payload_fact(
            facts,
            title.as_str(),
            SourceSpan::new(title.start, title.end),
            detail,
            EditorSemanticKind::String,
        );
    }
    if let Some(open) = line.find('[') {
        let close = line.rfind(']').unwrap_or(line.len());
        if close > open + 1 {
            let value = line[open + 1..close].trim();
            if !value.is_empty() {
                let value_rel = line.find(value).unwrap_or(open + 1);
                let start = line_start + value_rel;
                push_xychart_payload_fact(
                    facts,
                    value,
                    SourceSpan::new(start, start + value.len()),
                    detail,
                    EditorSemanticKind::String,
                );
            }
        }
    }
}

fn parse_text_prefix_spanned(input: &str, line: &str, line_start: usize) -> Option<SpannedText> {
    let (title, _rest) = parse_text_prefix(input).ok()?;
    let title_rel = line.find(&title)?;
    let start = line_start + title_rel;
    let len = title.len();
    Some(SpannedText {
        text: title,
        start,
        end: start + len,
    })
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    start: usize,
    end: usize,
}

impl SpannedText {
    fn as_str(&self) -> &str {
        &self.text
    }
}

#[derive(Debug, Clone, Copy)]
struct SpannedSlice<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

impl<'a> SpannedSlice<'a> {
    fn new(text: &'a str, start: usize, end: usize) -> Self {
        Self { text, start, end }
    }

    fn trim(self) -> Self {
        let leading = self.text.len().saturating_sub(self.text.trim_start().len());
        let text = &self.text[leading..];
        let trimmed_len = text.trim_end().len();
        Self {
            text: &text[..trimmed_len],
            start: self.start + leading,
            end: self.start + leading + trimmed_len,
        }
    }
}

fn strip_inline_comment(line: &str) -> &str {
    match line.find("%%") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn parse_text(input: &str) -> Result<String> {
    let t = input.trim_start();
    if let Some(body) = t.strip_prefix("\"`") {
        let Some(end) = body.find("`\"") else {
            return Err(Error::diagram_parse_fallback(
                "xychart".to_string(),
                "unterminated markdown string".to_string(),
            ));
        };
        let s = &body[..end];
        let rest = &body[end + 2..];
        if !rest.trim().is_empty() {
            return Err(Error::diagram_parse_fallback(
                "xychart".to_string(),
                "unexpected trailing tokens after text".to_string(),
            ));
        }
        return Ok(s.to_string());
    }

    if let Some(body) = t.strip_prefix('"') {
        let Some(end) = body.find('"') else {
            return Err(Error::diagram_parse_fallback(
                "xychart".to_string(),
                "unterminated string".to_string(),
            ));
        };
        let s = &body[..end];
        let rest = &body[end + 1..];
        if !rest.trim().is_empty() {
            return Err(Error::diagram_parse_fallback(
                "xychart".to_string(),
                "unexpected trailing tokens after text".to_string(),
            ));
        }
        return Ok(s.to_string());
    }

    let mut out = String::new();
    for part in t.split_whitespace() {
        out.push_str(part);
    }
    if out.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "expected text".to_string(),
        ));
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
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "x-axis requires data".to_string(),
        ));
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

    Err(Error::diagram_parse_fallback(
        "xychart".to_string(),
        "invalid x-axis data".to_string(),
    ))
}

fn parse_y_axis(rest: &str, state: &mut XyChartState, meta: &ParseMetadata) -> Result<()> {
    let t = rest.trim();
    if t.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "y-axis requires data".to_string(),
        ));
    }

    if let Some((min, max)) = try_parse_range(t)? {
        state.set_y_axis_title("", meta);
        state.set_y_axis_range(min, max);
        return Ok(());
    }

    if t.starts_with('[') {
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "y-axis does not support band data".to_string(),
        ));
    }

    let (title, tail) = parse_text_prefix(t)?;
    state.set_y_axis_title(&title, meta);
    let tail = tail.trim_start();
    if tail.is_empty() {
        return Ok(());
    }

    if tail.starts_with('[') {
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "y-axis does not support band data".to_string(),
        ));
    }

    if let Some((min, max)) = try_parse_range(tail)? {
        state.set_y_axis_range(min, max);
        return Ok(());
    }

    Err(Error::diagram_parse_fallback(
        "xychart".to_string(),
        "invalid y-axis data".to_string(),
    ))
}

fn parse_plot_stmt_spanned(rest: &str, rest_start: usize) -> Result<(String, Vec<f64>)> {
    let leading = rest.len().saturating_sub(rest.trim_start().len());
    let t = rest.trim_start();
    let t_start = rest_start + leading;
    if t.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "plot requires data".to_string(),
        ));
    }

    if t.starts_with('[') {
        let data = parse_number_list_in_brackets_spanned(t, t_start)?;
        if data.is_empty() {
            return Err(Error::diagram_parse_fallback(
                "xychart".to_string(),
                "plot data cannot be empty".to_string(),
            ));
        }
        return Ok((String::new(), data));
    }

    let (title, tail) = parse_text_prefix(t)?;
    let tail_start = t_start + t.len().saturating_sub(tail.len());
    let tail_leading = tail.len().saturating_sub(tail.trim_start().len());
    let tail = tail.trim_start();
    let tail_start = tail_start + tail_leading;
    if !tail.starts_with('[') {
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "plot data missing".to_string(),
        ));
    }
    let data = parse_number_list_in_brackets_spanned(tail, tail_start)?;
    if data.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "plot data cannot be empty".to_string(),
        ));
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
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "expected number".to_string(),
        ));
    };
    if !tail.trim().is_empty() {
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "unexpected trailing tokens after range".to_string(),
        ));
    }
    let a = parse_number(&a_str).ok_or_else(|| {
        Error::diagram_parse_fallback("xychart".to_string(), "invalid number".to_string())
    })?;
    let b = parse_number(&b_str).ok_or_else(|| {
        Error::diagram_parse_fallback("xychart".to_string(), "invalid number".to_string())
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
    if let Some(body) = t.strip_prefix("\"`") {
        let Some(end) = body.find("`\"") else {
            return Err(Error::diagram_parse_fallback(
                "xychart".to_string(),
                "unterminated markdown string".to_string(),
            ));
        };
        let s = &body[..end];
        let rest = &body[end + 2..];
        return Ok((s.to_string(), rest));
    }
    if let Some(body) = t.strip_prefix('"') {
        let Some(end) = body.find('"') else {
            return Err(Error::diagram_parse_fallback(
                "xychart".to_string(),
                "unterminated string".to_string(),
            ));
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
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "expected text".to_string(),
        ));
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
            return Err(Error::diagram_parse_fallback(
                "xychart".to_string(),
                "empty category".to_string(),
            ));
        }
        out.push(parse_text(p)?);
    }
    Ok(out)
}

fn parse_number_list_in_brackets_spanned(input: &str, input_start: usize) -> Result<Vec<f64>> {
    let leading = input.len().saturating_sub(input.trim_start().len());
    let t = input.trim_start();
    let t_start = input_start + leading;
    let (inner, inner_start) = extract_bracket_inner_spanned(t, t_start)?;
    let parts = split_top_level_commas_spanned(inner, inner_start);
    let mut out = Vec::new();
    for part in parts {
        let trimmed = part.trim();
        if trimmed.text.is_empty() {
            return Err(Error::diagram_parse_insertion_point(
                "xychart".to_string(),
                "empty number".to_string(),
                trimmed.start,
            ));
        }
        let n = parse_number(trimmed.text).ok_or_else(|| {
            Error::diagram_parse_exact(
                "xychart".to_string(),
                format!("invalid number: {}", trimmed.text),
                SourceSpan::new(trimmed.start, trimmed.end),
            )
        })?;
        out.push(n);
    }
    Ok(out)
}

fn extract_bracket_inner(input: &str) -> Result<&str> {
    let t = input.trim_start();
    if !t.starts_with('[') {
        return Err(Error::diagram_parse_fallback(
            "xychart".to_string(),
            "expected '['".to_string(),
        ));
    }
    let mut in_quote = false;
    let mut in_md = false;
    let mut idx = 1usize;
    while idx < t.len() {
        let rest = &t[idx..];
        let ch = rest.chars().next().unwrap();
        if in_md {
            if rest.starts_with("`\"") {
                in_md = false;
                idx += 2;
                continue;
            }
            idx += ch.len_utf8();
            continue;
        }
        if in_quote {
            if ch == '"' {
                in_quote = false;
            }
            idx += ch.len_utf8();
            continue;
        }
        if rest.starts_with("\"`") {
            in_md = true;
            idx += 2;
            continue;
        }
        if ch == '"' {
            in_quote = true;
            idx += ch.len_utf8();
            continue;
        }
        if ch == '[' {
            return Err(Error::diagram_parse_fallback(
                "xychart".to_string(),
                "unbalanced '['".to_string(),
            ));
        }
        if ch == ']' {
            let inner = &t[1..idx];
            let rest = &t[idx + 1..];
            if !rest.trim().is_empty() {
                return Err(Error::diagram_parse_fallback(
                    "xychart".to_string(),
                    "unexpected trailing tokens after ']'".to_string(),
                ));
            }
            return Ok(inner);
        }
        idx += ch.len_utf8();
    }

    Err(Error::diagram_parse_fallback(
        "xychart".to_string(),
        "unbalanced ']'".to_string(),
    ))
}

fn extract_bracket_inner_spanned(input: &str, input_start: usize) -> Result<(&str, usize)> {
    let t = input.trim_start();
    let t_start = input_start + input.len().saturating_sub(t.len());
    if !t.starts_with('[') {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
            "expected '['".to_string(),
            t_start,
        ));
    }
    let mut in_quote = false;
    let mut in_md = false;
    let mut idx = 1usize;
    while idx < t.len() {
        let rest = &t[idx..];
        let ch = rest.chars().next().unwrap();
        if in_md {
            if rest.starts_with("`\"") {
                in_md = false;
                idx += 2;
                continue;
            }
            idx += ch.len_utf8();
            continue;
        }
        if in_quote {
            if ch == '"' {
                in_quote = false;
            }
            idx += ch.len_utf8();
            continue;
        }
        if rest.starts_with("\"`") {
            in_md = true;
            idx += 2;
            continue;
        }
        if ch == '"' {
            in_quote = true;
            idx += ch.len_utf8();
            continue;
        }
        if ch == '[' {
            return Err(Error::diagram_parse_exact(
                "xychart".to_string(),
                "unbalanced '['".to_string(),
                SourceSpan::new(t_start + idx, t_start + idx + ch.len_utf8()),
            ));
        }
        if ch == ']' {
            let inner = &t[1..idx];
            let rest = &t[idx + 1..];
            if !rest.trim().is_empty() {
                let trailing_start = rest.len().saturating_sub(rest.trim_start().len());
                let trailing = rest.trim();
                let start = t_start + idx + 1 + trailing_start;
                return Err(Error::diagram_parse_exact(
                    "xychart".to_string(),
                    "unexpected trailing tokens after ']'".to_string(),
                    SourceSpan::new(start, start + trailing.len()),
                ));
            }
            return Ok((inner, t_start + 1));
        }
        idx += ch.len_utf8();
    }

    Err(Error::diagram_parse_insertion_point(
        "xychart".to_string(),
        "unbalanced ']'".to_string(),
        t_start + t.len(),
    ))
}

fn split_top_level_commas(input: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_quote = false;
    let mut in_md = false;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < input.len() {
        let rest = &input[i..];
        let ch = rest.chars().next().unwrap();
        if in_md {
            if rest.starts_with("`\"") {
                in_md = false;
                i += 2;
                continue;
            }
            i += ch.len_utf8();
            continue;
        }
        if in_quote {
            if ch == '"' {
                in_quote = false;
            }
            i += ch.len_utf8();
            continue;
        }
        if rest.starts_with("\"`") {
            in_md = true;
            i += 2;
            continue;
        }
        if ch == '"' {
            in_quote = true;
            i += ch.len_utf8();
            continue;
        }
        if ch == ',' {
            out.push(&input[start..i]);
            i += ch.len_utf8();
            start = i;
            continue;
        }
        i += ch.len_utf8();
    }
    out.push(&input[start..]);
    out
}

fn split_top_level_commas_spanned(input: &str, input_start: usize) -> Vec<SpannedSlice<'_>> {
    let mut out = Vec::new();
    let mut in_quote = false;
    let mut in_md = false;
    let mut start = 0usize;
    let mut i = 0usize;
    while i < input.len() {
        let rest = &input[i..];
        let ch = rest.chars().next().unwrap();
        if in_md {
            if rest.starts_with("`\"") {
                in_md = false;
                i += 2;
                continue;
            }
            i += ch.len_utf8();
            continue;
        }
        if in_quote {
            if ch == '"' {
                in_quote = false;
            }
            i += ch.len_utf8();
            continue;
        }
        if rest.starts_with("\"`") {
            in_md = true;
            i += 2;
            continue;
        }
        if ch == '"' {
            in_quote = true;
            i += ch.len_utf8();
            continue;
        }
        if ch == ',' {
            out.push(SpannedSlice::new(
                &input[start..i],
                input_start + start,
                input_start + i,
            ));
            i += ch.len_utf8();
            start = i;
            continue;
        }
        i += ch.len_utf8();
    }
    out.push(SpannedSlice::new(
        &input[start..],
        input_start + start,
        input_start + input.len(),
    ));
    out
}

#[derive(Debug, Clone)]
struct SpannedStatement {
    text: String,
    start: usize,
}

impl SpannedStatement {
    fn trimmed_start(&self) -> usize {
        self.start + self.text.len().saturating_sub(self.text.trim_start().len())
    }
}

fn split_statements_spanned(input: &str) -> Vec<SpannedStatement> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut cur_start = 0usize;
    let mut in_quote = false;
    let mut in_md = false;
    let mut bracket_depth = 0i64;
    let mut brace_depth = 0i64;
    let mut iter = input.char_indices().peekable();

    while let Some((idx, ch)) = iter.next() {
        if cur.is_empty() {
            cur_start = idx;
        }

        if in_md {
            cur.push(ch);
            if ch == '`' && iter.peek().is_some_and(|(_, next)| *next == '"') {
                if let Some((_quote_idx, quote)) = iter.next() {
                    cur.push(quote);
                    in_md = false;
                }
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

        if ch == '"' && iter.peek().is_some_and(|(_, next)| *next == '`') {
            cur.push(ch);
            if let Some((_tick_idx, tick)) = iter.next() {
                cur.push(tick);
                in_md = true;
            }
            continue;
        }

        if ch == '"' {
            cur.push(ch);
            in_quote = true;
            continue;
        }

        if ch == '%' && iter.peek().is_some_and(|(_, next)| *next == '%') {
            let mut next_statement_start = input.len();
            for (comment_idx, comment_ch) in iter.by_ref() {
                if comment_ch == '\n' {
                    next_statement_start = comment_idx + comment_ch.len_utf8();
                    break;
                }
            }
            if !cur.trim().is_empty() {
                out.push(SpannedStatement {
                    text: std::mem::take(&mut cur),
                    start: cur_start,
                });
            } else {
                cur.clear();
            }
            cur_start = next_statement_start;
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
            out.push(SpannedStatement {
                text: std::mem::take(&mut cur),
                start: cur_start,
            });
            cur_start = idx + ch.len_utf8();
            continue;
        }

        cur.push(ch);
    }

    if !cur.is_empty() {
        out.push(SpannedStatement {
            text: cur,
            start: cur_start,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ParseDiagnosticSpanKind, ParseOptions};
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
            Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
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
    fn xychart_unquoted_multibyte_categories_do_not_panic() {
        let model = parse(
            r#"xychart
x-axis [東京, 大阪]
y-axis "値" 0 --> 10
bar [1, 2]
"#,
        );

        assert_eq!(model["xAxis"]["categories"], json!(["東京", "大阪"]));
    }

    #[test]
    fn xychart_plot_requires_nonempty_data() {
        let err = parse_err("xychart\nline \"t\" [ ]");
        assert!(err.contains("empty"));
        let err = parse_err("xychart\nline \"t\"");
        assert!(err.contains("missing") || err.contains("requires"));
    }

    #[test]
    fn xychart_accepts_line_without_whitespace_after_keyword() {
        let model = parse("xychart\nline[1,2,3]");
        assert_eq!(model["plots"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn xychart_comment_after_plot_does_not_merge_next_statement() {
        let model = parse("xychart\nbar [1] %% keep next line separate\nline [2]\n");
        assert_eq!(model["plots"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn xychart_acc_title_requires_colon() {
        let err = parse_err("xychart\naccTitle hello");
        assert!(err.contains("accTitle"));
    }

    #[test]
    fn xychart_rejects_invalid_x_axis_range_like_upstream() {
        let err = parse_err("xychart\nx-axis xAxisName aaa --> 33\n");
        assert!(err.contains("invalid"));
    }

    #[test]
    fn xychart_rejects_unbalanced_x_axis_brackets_like_upstream() {
        let err = parse_err("xychart\nx-axis xAxisName [ \"cat1\" [ cat2a ]\n");
        assert!(err.contains("unbalanced"));
        let err = parse_err("xychart\nx-axis xAxisName [ \"cat1\" , cat2a ] ]\n");
        assert!(err.contains("unexpected") || err.contains("unbalanced"));
    }

    #[test]
    fn xychart_rejects_invalid_y_axis_range_like_upstream() {
        let err = parse_err("xychart\ny-axis yAxisName 45.5 --> abc\n");
        assert!(err.contains("expected number") || err.contains("invalid"));
    }

    #[test]
    fn xychart_rejects_y_axis_band_data_like_upstream() {
        let err = parse_err("xychart\ny-axis yAxisName [ 45.3, 33 ]\n");
        assert!(err.contains("does not support") || err.contains("band"));
    }

    #[test]
    fn xychart_rejects_unbalanced_plot_brackets_like_upstream() {
        let err = parse_err("xychart\nline \"t\" [  +23 [ -45  , 56.6 ]\n");
        assert!(err.contains("unbalanced") || err.contains("expected"));
        let err = parse_err("xychart\nbar \"t\" [  +23 , -45  ] 56.6 ]\n");
        assert!(err.contains("unexpected") || err.contains("unbalanced"));
    }

    #[test]
    fn xychart_rejects_invalid_plot_commas_and_numbers_like_upstream() {
        let err = parse_err("xychart\nline \"t\" [  +23 ,  , -45  , 56.6 ]\n");
        assert!(err.contains("empty") || err.contains("invalid"));
        let err = parse_err("xychart\nbar \"t\" [  +23 , -4aa5  , 56.6 ]\n");
        assert!(err.contains("invalid number"));
    }

    #[test]
    fn xychart_invalid_plot_number_reports_exact_token_span() {
        let text = "xychart\nbar \"t\" [  +23 , -4aa5  , 56.6 ]\n";
        let engine = Engine::new();
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = err else {
            panic!("expected xychart parse error");
        };

        let token_start = text.find("-4aa5").unwrap();
        assert_eq!(diagnostic.message(), "invalid number: -4aa5");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(token_start, token_start + "-4aa5".len()))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }
}
