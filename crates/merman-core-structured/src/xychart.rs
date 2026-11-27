//! XY Chart statement scanning, validation, and semantic model construction.

use merman_markup::sanitize::sanitize_text;
use merman_model::families::xy_chart::{
    XyChartAxisModel, XyChartDiagramModel, XyChartPlotModel, XyChartPlotType,
};
use merman_model::{
    MermaidConfig, OperationControl, OperationControlResult, SourceSpan,
    text::is_ecmascript_whitespace,
};

pub struct ParsedXyChart {
    pub model: XyChartDiagramModel,
    pub first_issue: Option<XyChartIssue>,
}

pub enum XyChartIssue {
    Exact { message: String, span: SourceSpan },
    InsertionPoint { message: String, offset: usize },
    MissingHeader(SourceSpan),
    UnexpectedStatement { message: String, span: SourceSpan },
}

type ParseResult<T> = Result<T, XyChartIssue>;

impl XyChartIssue {
    fn exact(message: impl Into<String>, span: SourceSpan) -> Self {
        Self::Exact {
            message: message.into(),
            span,
        }
    }

    fn insertion_point(message: impl Into<String>, offset: usize) -> Self {
        Self::InsertionPoint {
            message: message.into(),
            offset,
        }
    }
}

#[cfg(feature = "editor-facts")]
pub enum XyChartObservation<'a> {
    Header(Option<SourceSpan>),
    Statement(&'a ParsedXyChartStatement),
}

#[cfg(any(test, feature = "test-support"))]
use std::cell::Cell;

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static XYCHART_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(any(test, feature = "test-support"))]
pub fn reset_xychart_syntax_construction_count() {
    XYCHART_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(any(test, feature = "test-support"))]
pub fn xychart_syntax_construction_count() -> usize {
    XYCHART_SYNTAX_CONSTRUCTION_COUNT.get()
}

#[derive(Debug, Clone)]
enum AxisData {
    Band {
        title: String,
        categories: Vec<String>,
    },
    Linear {
        title: String,
        // Absence is distinct from an authored or inferred infinite endpoint.
        range: Option<(f64, f64)>,
    },
}

#[derive(Debug, Clone)]
struct Plot {
    plot_type: XyChartPlotType,
    title: Option<String>,
    values: Vec<f64>,
    data: Vec<(String, Option<f64>)>,
    point_labels: Vec<String>,
}

#[derive(Debug, Clone)]
struct ParsedDataPoint {
    value: f64,
    label: String,
    #[cfg(feature = "editor-facts")]
    label_span: Option<SourceSpan>,
}

enum ParsedAxisData {
    None,
    Band {
        categories: Vec<String>,
        #[cfg(feature = "editor-facts")]
        source: SourceSpan,
    },
    Range {
        min: f64,
        max: f64,
        #[cfg(feature = "editor-facts")]
        source: SourceSpan,
    },
}

pub struct ParsedAxisStatement {
    title: Option<SpannedText>,
    data: ParsedAxisData,
}

pub struct ParsedPlotStatement {
    title: Option<SpannedText>,
    data: Vec<ParsedDataPoint>,
    #[cfg(feature = "editor-facts")]
    data_source: SourceSpan,
}

pub enum ParsedXyChartStatement {
    Title(SpannedText),
    AccessibilityTitle(SpannedText),
    AccessibilityDescription(SpannedText),
    XAxis(ParsedAxisStatement),
    YAxis(ParsedAxisStatement),
    Line(ParsedPlotStatement),
    Bar(ParsedPlotStatement),
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
    fn new(config: &MermaidConfig) -> Self {
        let orientation = config
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
                range: None,
            },
            plots: Vec::new(),
            has_set_x_axis: false,
            has_set_y_axis: false,
        }
    }

    fn set_orientation(&mut self, o: &str) {
        if o == "horizontal" {
            self.orientation = "horizontal".to_string();
        } else {
            self.orientation = "vertical".to_string();
        }
    }

    fn set_x_axis_title(&mut self, title: &str, config: &MermaidConfig) {
        let t = sanitize_text(title.trim_matches(is_ecmascript_whitespace), config);
        match &mut self.x_axis {
            AxisData::Band { title, .. } => *title = t,
            AxisData::Linear { title, .. } => *title = t,
        }
    }

    fn set_y_axis_title(&mut self, title: &str, config: &MermaidConfig) {
        let t = sanitize_text(title.trim_matches(is_ecmascript_whitespace), config);
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
        self.x_axis = AxisData::Linear {
            title,
            range: Some((min, max)),
        };
        self.has_set_x_axis = true;
    }

    fn set_x_axis_band(&mut self, categories: Vec<String>, config: &MermaidConfig) {
        let title = match &self.x_axis {
            AxisData::Band { title, .. } => title.clone(),
            AxisData::Linear { title, .. } => title.clone(),
        };
        let categories = categories
            .into_iter()
            .map(|c| sanitize_text(c.trim_matches(is_ecmascript_whitespace), config))
            .collect::<Vec<_>>();
        self.x_axis = AxisData::Band { title, categories };
        self.has_set_x_axis = true;
    }

    fn set_y_axis_range(&mut self, min: f64, max: f64) {
        let title = match &self.y_axis {
            AxisData::Linear { title, .. } => title.clone(),
            AxisData::Band { title, .. } => title.clone(),
        };
        self.y_axis = AxisData::Linear {
            title,
            range: Some((min, max)),
        };
        self.has_set_y_axis = true;
    }

    fn set_y_axis_range_from_plot_data(&mut self, data: &[f64]) {
        let min_value = data.iter().copied().fold(f64::INFINITY, |a, b| a.min(b));
        let max_value = data
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, |a, b| a.max(b));

        let (prev_min, prev_max, title) = match &self.y_axis {
            AxisData::Linear { title, range } => {
                let (min, max) = range.unwrap_or((f64::INFINITY, f64::NEG_INFINITY));
                (min, max, title.clone())
            }
            AxisData::Band { title, .. } => (f64::INFINITY, f64::NEG_INFINITY, title.clone()),
        };

        self.y_axis = AxisData::Linear {
            title,
            range: Some((prev_min.min(min_value), prev_max.max(max_value))),
        };
    }

    fn transform_data_without_category(
        &mut self,
        mut data: Vec<f64>,
    ) -> (Vec<f64>, Vec<(String, Option<f64>)>) {
        if data.is_empty() {
            return (data, Vec::new());
        }

        if !self.has_set_x_axis {
            let (prev_min, prev_max) = match &self.x_axis {
                AxisData::Linear { range, .. } => {
                    range.unwrap_or((f64::INFINITY, f64::NEG_INFINITY))
                }
                AxisData::Band { .. } => (f64::INFINITY, f64::NEG_INFINITY),
            };
            self.set_x_axis_range(prev_min.min(1.0), prev_max.max(data.len() as f64));
        }

        if let AxisData::Band { categories, .. } = &self.x_axis {
            data.truncate(categories.len());
        }

        if !self.has_set_y_axis {
            self.set_y_axis_range_from_plot_data(&data);
        }

        let plot_data = match &self.x_axis {
            AxisData::Band { categories, .. } => categories
                .iter()
                .enumerate()
                .map(|(i, c)| (c.clone(), data.get(i).copied()))
                .collect(),
            AxisData::Linear { range, .. } => {
                let (min, max) = range.unwrap_or((f64::INFINITY, f64::NEG_INFINITY));
                if data.len() == 1 {
                    vec![(format_js_number(min), data.first().copied())]
                } else {
                    let step = (max - min) / ((data.len() as f64) - 1.0);
                    data.iter()
                        .enumerate()
                        .map(|(index, datum)| {
                            (format_js_number(min + (index as f64) * step), Some(*datum))
                        })
                        .collect()
                }
            }
        };
        (data, plot_data)
    }

    fn add_line_data(
        &mut self,
        title: Option<String>,
        data: Vec<ParsedDataPoint>,
        config: &MermaidConfig,
    ) {
        let values = data.iter().map(|point| point.value).collect::<Vec<_>>();
        let mut point_labels = data
            .iter()
            .map(|point| {
                if point.label.is_empty() {
                    String::new()
                } else {
                    sanitize_text(point.label.trim_matches(is_ecmascript_whitespace), config)
                }
            })
            .collect::<Vec<_>>();
        let (values, pairs) = self.transform_data_without_category(values);
        if point_labels.iter().all(String::is_empty) {
            point_labels.clear();
        }
        self.plots.push(Plot {
            plot_type: XyChartPlotType::Line,
            title,
            values,
            data: pairs,
            point_labels,
        });
    }

    fn add_bar_data(&mut self, title: Option<String>, data: Vec<ParsedDataPoint>) {
        let values = data.iter().map(|point| point.value).collect::<Vec<_>>();
        let (values, pairs) = self.transform_data_without_category(values);
        self.plots.push(Plot {
            plot_type: XyChartPlotType::Bar,
            title,
            values,
            data: pairs,
            point_labels: Vec::new(),
        });
    }

    fn into_semantic_model(
        self,
        title: Option<String>,
        acc_title: Option<String>,
        acc_descr: Option<String>,
    ) -> XyChartDiagramModel {
        XyChartDiagramModel {
            orientation: self.orientation,
            title,
            acc_title,
            acc_descr,
            x_axis: axis_data_to_semantic_model(self.x_axis),
            y_axis: axis_data_to_semantic_model(self.y_axis),
            plots: self
                .plots
                .into_iter()
                .map(|p| XyChartPlotModel {
                    plot_type: p.plot_type,
                    title: p.title,
                    values: p.values,
                    data: p.data,
                    point_labels: p.point_labels,
                })
                .collect(),
        }
    }
}

#[cfg(feature = "editor-facts")]
impl ParsedAxisStatement {
    pub const fn title(&self) -> Option<&SpannedText> {
        self.title.as_ref()
    }

    pub fn data_span(&self) -> Option<SourceSpan> {
        match &self.data {
            ParsedAxisData::None => None,
            ParsedAxisData::Band { source, .. } | ParsedAxisData::Range { source, .. } => {
                Some(*source)
            }
        }
    }
}

#[cfg(feature = "editor-facts")]
impl ParsedPlotStatement {
    pub const fn title(&self) -> Option<&SpannedText> {
        self.title.as_ref()
    }

    pub const fn data_span(&self) -> SourceSpan {
        self.data_source
    }

    pub fn data_labels(&self) -> impl Iterator<Item = (&str, SourceSpan)> {
        self.data
            .iter()
            .filter_map(|point| point.label_span.map(|span| (point.label.as_str(), span)))
    }
}

fn apply_x_axis_statement(
    axis: ParsedAxisStatement,
    state: &mut XyChartState,
    config: &MermaidConfig,
) {
    state.set_x_axis_title(axis.title.as_ref().map_or("", SpannedText::as_str), config);
    match axis.data {
        ParsedAxisData::None => {}
        ParsedAxisData::Band { categories, .. } => state.set_x_axis_band(categories, config),
        ParsedAxisData::Range { min, max, .. } => state.set_x_axis_range(min, max),
    }
}

fn apply_y_axis_statement(
    axis: ParsedAxisStatement,
    state: &mut XyChartState,
    config: &MermaidConfig,
) {
    state.set_y_axis_title(axis.title.as_ref().map_or("", SpannedText::as_str), config);
    if let ParsedAxisData::Range { min, max, .. } = axis.data {
        state.set_y_axis_range(min, max);
    }
}

fn parse_xychart_statement(
    statement_text: &str,
    statement_start: usize,
) -> ParseResult<ParsedXyChartStatement> {
    let statement_span = SourceSpan::new(statement_start, statement_start + statement_text.len());

    let (keyword, keyword_len) = xychart_word(statement_text);
    let rest = &statement_text[keyword_len..];
    let rest_start = statement_start + keyword_len;
    match keyword {
        Some(XyChartKeyword::Title) => {
            return parse_text_source(rest, rest_start).map(ParsedXyChartStatement::Title);
        }
        Some(XyChartKeyword::AccTitle) => {
            return parse_colon_value_source(rest, rest_start, "accTitle")
                .map(ParsedXyChartStatement::AccessibilityTitle);
        }
        Some(XyChartKeyword::AccDescr) => {
            return parse_acc_descr_source(rest, rest_start)
                .map(ParsedXyChartStatement::AccessibilityDescription);
        }
        Some(XyChartKeyword::XAxis) => {
            return parse_x_axis_source(rest, rest_start).map(ParsedXyChartStatement::XAxis);
        }
        Some(XyChartKeyword::YAxis) => {
            return parse_y_axis_source(rest, rest_start).map(ParsedXyChartStatement::YAxis);
        }
        Some(XyChartKeyword::Line) => {
            return parse_plot_stmt_source(rest, rest_start).map(ParsedXyChartStatement::Line);
        }
        Some(XyChartKeyword::Bar) => {
            return parse_plot_stmt_source(rest, rest_start).map(ParsedXyChartStatement::Bar);
        }
        None => {}
    }

    Err(XyChartIssue::UnexpectedStatement {
        message: format!("unexpected xychart statement: {statement_text}"),
        span: statement_span,
    })
}

/// Parses one preprocessed XY Chart source with the supplied configuration.
pub fn parse_xychart_source_controlled(
    code: &str,
    config: &MermaidConfig,
    control: &OperationControl,
) -> OperationControlResult<ParsedXyChart> {
    parse_source(
        code,
        config,
        #[cfg(feature = "editor-facts")]
        None,
        control,
    )
}

/// Reports borrowed source facts from the same parse used to build the model.
#[cfg(feature = "editor-facts")]
pub fn parse_xychart_source_with_observer_controlled(
    code: &str,
    config: &MermaidConfig,
    observe: &mut dyn FnMut(XyChartObservation<'_>),
    control: &OperationControl,
) -> OperationControlResult<ParsedXyChart> {
    parse_source(code, config, Some(observe), control)
}

fn parse_source(
    code: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] mut observe: Option<&mut dyn FnMut(XyChartObservation<'_>)>,
    control: &OperationControl,
) -> OperationControlResult<ParsedXyChart> {
    control.checkpoint()?;
    #[cfg(any(test, feature = "test-support"))]
    XYCHART_SYNTAX_CONSTRUCTION_COUNT.set(XYCHART_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let syntax = split_statements_spanned_controlled(code, control)?;
    let mut statements = syntax.statements.into_iter().filter(|statement| {
        !statement
            .text
            .trim_matches(is_ecmascript_whitespace)
            .is_empty()
    });
    let Some(header) = statements.next() else {
        return Ok(ParsedXyChart {
            model: XyChartState::new(config).into_semantic_model(None, None, None),
            first_issue: None,
        });
    };

    let mut state = XyChartState::new(config);
    let header_start = header.trimmed_start();
    let mut first_issue = parse_header(&header.text, header_start, &mut state).err();
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observe.as_deref_mut() {
        let header_trimmed = header.text.trim_matches(is_ecmascript_whitespace);
        let header_span = header_token_len_and_rest(header_trimmed)
            .map(|(prefix_len, _)| SourceSpan::new(header_start, header_start + prefix_len));
        observe(XyChartObservation::Header(header_span));
    }

    let mut title = None;
    let mut acc_title = None;
    let mut acc_descr = None;

    for (index, statement) in statements.enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        let statement_start = statement.trimmed_start();
        let statement_text = statement.text.trim_matches(is_ecmascript_whitespace);
        if statement_text.is_empty() {
            continue;
        }
        let parsed = match parse_xychart_statement(statement_text, statement_start) {
            Ok(parsed) => parsed,
            Err(issue) => {
                first_issue.get_or_insert(issue);
                continue;
            }
        };
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(XyChartObservation::Statement(&parsed));
        }

        match parsed {
            ParsedXyChartStatement::Title(value) => {
                title = Some(
                    value
                        .text
                        .trim_matches(is_ecmascript_whitespace)
                        .to_string(),
                );
            }
            ParsedXyChartStatement::AccessibilityTitle(value) => acc_title = Some(value.text),
            ParsedXyChartStatement::AccessibilityDescription(value) => acc_descr = Some(value.text),
            ParsedXyChartStatement::XAxis(axis) => apply_x_axis_statement(axis, &mut state, config),
            ParsedXyChartStatement::YAxis(axis) => apply_y_axis_statement(axis, &mut state, config),
            ParsedXyChartStatement::Line(plot) => state.add_line_data(
                plot.title
                    .as_ref()
                    .and_then(|title| plot_title_value(title.as_str(), config)),
                plot.data,
                config,
            ),
            ParsedXyChartStatement::Bar(plot) => state.add_bar_data(
                plot.title
                    .as_ref()
                    .and_then(|title| plot_title_value(title.as_str(), config)),
                plot.data,
            ),
        }
    }

    control.checkpoint()?;
    Ok(ParsedXyChart {
        model: state.into_semantic_model(title, acc_title, acc_descr),
        first_issue,
    })
}

fn format_js_number(value: f64) -> String {
    // Mermaid template strings use Number text, including nonfinite values and unsigned zero.
    ryu_js::Buffer::new().format(value).to_owned()
}

fn plot_title_value(title: &str, config: &MermaidConfig) -> Option<String> {
    let title = sanitize_text(title.trim_matches(is_ecmascript_whitespace), config);
    (!title.is_empty()).then_some(title)
}

fn axis_data_to_semantic_model(axis: AxisData) -> XyChartAxisModel {
    match axis {
        AxisData::Band { title, categories } => XyChartAxisModel::Band { title, categories },
        AxisData::Linear { title, range } => {
            let min = range.map(|(min, _)| min);
            let max = range.map(|(_, max)| max);
            XyChartAxisModel::Linear { title, min, max }
        }
    }
}

fn parse_header(stmt: &str, statement_start: usize, state: &mut XyChartState) -> ParseResult<()> {
    let t = stmt.trim_matches(is_ecmascript_whitespace);
    let trimmed_start = statement_start;
    let lower = t.to_ascii_lowercase();
    let (prefix, rest) = if lower.starts_with("xychart-beta") {
        ("xychart-beta", &t["xychart-beta".len()..])
    } else if lower.starts_with("xychart") {
        ("xychart", &t["xychart".len()..])
    } else {
        return Err(XyChartIssue::MissingHeader(SourceSpan::new(
            trimmed_start,
            trimmed_start + t.len(),
        )));
    };
    let rem = rest.trim_matches(is_ecmascript_whitespace);
    if rem.is_empty() {
        return Ok(());
    }
    if !rest.starts_with(is_ecmascript_whitespace) {
        let rem_start = trimmed_start + prefix.len();
        return Err(XyChartIssue::exact(
            format!("unexpected token after {prefix}: {rem}"),
            SourceSpan::new(rem_start, trimmed_start + t.len()),
        ));
    }

    if rem.eq_ignore_ascii_case("vertical") || rem.eq_ignore_ascii_case("horizontal") {
        state.set_orientation(rem);
        return Ok(());
    }

    let rem_start = trimmed_start + t.len().saturating_sub(rem.len());
    Err(XyChartIssue::exact(
        format!("invalid chart orientation: {rem}"),
        SourceSpan::new(rem_start, rem_start + rem.len()),
    ))
}

#[cfg(feature = "editor-facts")]
fn header_token_len_and_rest(stmt: &str) -> Option<(usize, &str)> {
    let t = stmt.trim_start_matches(is_ecmascript_whitespace);
    let lower = t.to_ascii_lowercase();
    if lower.starts_with("xychart-beta") {
        return Some(("xychart-beta".len(), &t["xychart-beta".len()..]));
    }
    if lower.starts_with("xychart") {
        return Some(("xychart".len(), &t["xychart".len()..]));
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum XyChartKeyword {
    Title,
    AccTitle,
    AccDescr,
    XAxis,
    YAxis,
    Line,
    Bar,
}

/// Jison's literal word rules use an ASCII word boundary. Thus `barTitle` and
/// `bar_1` remain text. Statement dispatch and lexical seams share this owner;
/// quoted and structured payloads are scanned in their own states.
fn xychart_word(input: &str) -> (Option<XyChartKeyword>, usize) {
    let alpha_len = input.bytes().take_while(u8::is_ascii_alphabetic).count();
    for (word, keyword) in [
        ("title", XyChartKeyword::Title),
        ("accTitle", XyChartKeyword::AccTitle),
        ("accDescr", XyChartKeyword::AccDescr),
        ("x-axis", XyChartKeyword::XAxis),
        ("y-axis", XyChartKeyword::YAxis),
        ("line", XyChartKeyword::Line),
        ("bar", XyChartKeyword::Bar),
    ] {
        if !input
            .get(..word.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(word))
        {
            continue;
        }
        if matches!(keyword, XyChartKeyword::AccTitle | XyChartKeyword::AccDescr) {
            let tail = input[word.len()..].trim_start_matches(is_ecmascript_whitespace);
            if !tail.starts_with(':')
                && !(keyword == XyChartKeyword::AccDescr && tail.starts_with('{'))
            {
                continue;
            }
        } else if input
            .as_bytes()
            .get(word.len())
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            continue;
        }
        return (Some(keyword), word.len());
    }
    (None, alpha_len)
}

fn xychart_reserved_text_token_len(input: &str) -> Option<usize> {
    let (keyword, len) = xychart_word(input);
    if keyword.is_some() {
        return Some(len);
    }
    // Header literals have a trailing ASCII word boundary; the orientation regexp does not.
    for word in ["xychart-beta", "xychart"] {
        if input
            .get(..word.len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(word))
            && !input
                .as_bytes()
                .get(word.len())
                .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
        {
            return Some(word.len());
        }
    }
    ["vertical", "horizontal"]
        .into_iter()
        .find(|word| {
            input
                .get(..word.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(word))
        })
        .map(str::len)
}

#[derive(Debug, Clone)]
pub struct SpannedText {
    text: String,
    #[cfg(feature = "editor-facts")]
    start: usize,
    #[cfg(feature = "editor-facts")]
    end: usize,
}

impl SpannedText {
    pub fn as_str(&self) -> &str {
        &self.text
    }

    #[cfg(feature = "editor-facts")]
    pub fn span(&self) -> SourceSpan {
        SourceSpan::new(self.start, self.end)
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
        let leading = self
            .text
            .len()
            .saturating_sub(self.text.trim_start_matches(is_ecmascript_whitespace).len());
        let text = &self.text[leading..];
        let trimmed_len = text.trim_end_matches(is_ecmascript_whitespace).len();
        Self {
            text: &text[..trimmed_len],
            start: self.start + leading,
            end: self.start + leading + trimmed_len,
        }
    }

    fn to_text(self) -> SpannedText {
        SpannedText {
            text: self.text.to_string(),
            #[cfg(feature = "editor-facts")]
            start: self.start,
            #[cfg(feature = "editor-facts")]
            end: self.end,
        }
    }
}

fn parse_text_source(input: &str, input_start: usize) -> ParseResult<SpannedText> {
    let (value, tail) = parse_text_prefix_source(input, input_start)?;
    let trailing = tail.trim();
    if !trailing.text.is_empty() {
        return Err(XyChartIssue::exact(
            "unexpected trailing tokens after text".to_string(),
            SourceSpan::new(trailing.start, trailing.end),
        ));
    }
    Ok(value)
}

fn parse_text_prefix_source<'a>(
    input: &'a str,
    input_start: usize,
) -> ParseResult<(SpannedText, SpannedSlice<'a>)> {
    let text = input.trim_start_matches(is_ecmascript_whitespace);
    let text_start = input_start + input.len() - text.len();
    if text.is_empty() {
        return Err(XyChartIssue::insertion_point(
            "expected text".to_string(),
            text_start,
        ));
    }

    if let Some(body) = text.strip_prefix("\"`") {
        let Some(end) = body.find("`\"") else {
            return Err(XyChartIssue::insertion_point(
                "unterminated markdown string".to_string(),
                input_start + input.len(),
            ));
        };
        #[cfg(feature = "editor-facts")]
        let value_start = text_start + 2;
        let tail_start = 2 + end + 2;
        return Ok((
            SpannedText {
                text: body[..end].to_string(),
                #[cfg(feature = "editor-facts")]
                start: value_start,
                #[cfg(feature = "editor-facts")]
                end: value_start + end,
            },
            SpannedSlice::new(
                &text[tail_start..],
                text_start + tail_start,
                input_start + input.len(),
            ),
        ));
    }

    if let Some(body) = text.strip_prefix('"') {
        let Some(end) = body.find('"') else {
            return Err(XyChartIssue::insertion_point(
                "unterminated string".to_string(),
                input_start + input.len(),
            ));
        };
        #[cfg(feature = "editor-facts")]
        let value_start = text_start + 1;
        let tail_start = 1 + end + 1;
        return Ok((
            SpannedText {
                text: body[..end].to_string(),
                #[cfg(feature = "editor-facts")]
                start: value_start,
                #[cfg(feature = "editor-facts")]
                end: value_start + end,
            },
            SpannedSlice::new(
                &text[tail_start..],
                text_start + tail_start,
                input_start + input.len(),
            ),
        ));
    }

    let bracket_start = text.find('[');
    let range_start = range_suffix_start(text);
    let arrow_start = text.find("-->").filter(|offset| {
        *offset == 0
            || text[..*offset]
                .chars()
                .next_back()
                .is_some_and(is_ecmascript_whitespace)
    });
    let tail_rel = [bracket_start, range_start, arrow_start]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(text.len());
    let raw_value = &text[..tail_rel];
    let value_len = raw_value.trim_end_matches(is_ecmascript_whitespace).len();
    let raw_value = &raw_value[..value_len];
    if raw_value.is_empty() {
        return Err(XyChartIssue::insertion_point(
            "expected text".to_string(),
            text_start,
        ));
    }

    let mut value = String::new();
    let mut offset = 0;
    while offset < raw_value.len() {
        let remaining = &raw_value[offset..];
        let ch = remaining.chars().next().expect("nonempty text suffix");
        if ch.is_ascii_alphabetic() {
            if let Some(len) = xychart_reserved_text_token_len(remaining) {
                let token = &remaining[..len];
                return Err(XyChartIssue::exact(
                    format!("unexpected reserved token in text: {token}"),
                    SourceSpan::new(text_start + offset, text_start + offset + len),
                ));
            }
            // Consume ALPHA as one token: a reserved suffix inside `foobar` is text,
            // but a new token after NUM, punctuation, or whitespace is lexed again.
            let len = remaining
                .bytes()
                .take_while(u8::is_ascii_alphabetic)
                .count();
            value.push_str(&remaining[..len]);
            offset += len;
            continue;
        }
        offset += ch.len_utf8();
        if is_ecmascript_whitespace(ch) {
            continue;
        }
        if !is_text_token_char(ch) {
            return Err(XyChartIssue::exact(
                format!("unexpected token in text: {ch}"),
                SourceSpan::new(text_start + offset - ch.len_utf8(), text_start + offset),
            ));
        }
        value.push(ch);
    }
    if value.is_empty() {
        return Err(XyChartIssue::insertion_point(
            "expected text".to_string(),
            text_start,
        ));
    }
    Ok((
        SpannedText {
            text: value,
            #[cfg(feature = "editor-facts")]
            start: text_start,
            #[cfg(feature = "editor-facts")]
            end: text_start + raw_value.len(),
        },
        SpannedSlice::new(
            &text[tail_rel..],
            text_start + tail_rel,
            input_start + input.len(),
        ),
    ))
}

fn is_text_token_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '&' | '+' | '=' | '*' | '.' | '#' | '-' | '_')
}

fn range_suffix_start(input: &str) -> Option<usize> {
    input.char_indices().find_map(|(offset, _)| {
        if offset == 0
            || !input[..offset]
                .chars()
                .next_back()
                .is_some_and(is_ecmascript_whitespace)
        {
            return None;
        }
        let (_, tail) = take_number_token(&input[offset..])?;
        tail.trim_start_matches(is_ecmascript_whitespace)
            .starts_with("-->")
            .then_some(offset)
    })
}

fn parse_colon_value_source(
    input: &str,
    input_start: usize,
    keyword: &str,
) -> ParseResult<SpannedText> {
    let input = SpannedSlice::new(input, input_start, input_start + input.len()).trim();
    let Some(value) = input.text.strip_prefix(':') else {
        return Err(XyChartIssue::insertion_point(
            format!("expected ':' after {keyword}"),
            input.start,
        ));
    };
    let value = SpannedSlice::new(value, input.start + 1, input.end)
        .trim()
        .to_text();
    Ok(value)
}

fn parse_acc_descr_source(input: &str, input_start: usize) -> ParseResult<SpannedText> {
    let input = SpannedSlice::new(input, input_start, input_start + input.len()).trim();
    if input.text.starts_with(':') {
        let value = SpannedSlice::new(&input.text[1..], input.start + 1, input.end)
            .trim()
            .to_text();
        return Ok(value);
    }

    let Some(body) = input.text.strip_prefix('{') else {
        return Err(XyChartIssue::insertion_point(
            "expected ':' or '{' after accDescr".to_string(),
            input.start,
        ));
    };
    let Some(close) = body.find('}') else {
        return Err(XyChartIssue::insertion_point(
            "unterminated accDescr block".to_string(),
            input.end,
        ));
    };
    let body_source =
        SpannedSlice::new(&body[..close], input.start + 1, input.start + 1 + close).trim();
    let trailing =
        SpannedSlice::new(&body[close + 1..], input.start + 1 + close + 1, input.end).trim();
    if !trailing.text.is_empty() {
        return Err(XyChartIssue::exact(
            "unexpected trailing tokens after accDescr block".to_string(),
            SourceSpan::new(trailing.start, trailing.end),
        ));
    }
    Ok(body_source.to_text())
}

fn parse_number(s: &str) -> Option<f64> {
    let t = s.trim_matches(is_ecmascript_whitespace);
    if t.is_empty() {
        return None;
    }
    let unsigned = t
        .strip_prefix('+')
        .or_else(|| t.strip_prefix('-'))
        .unwrap_or(t);
    let valid = if let Some(fraction) = unsigned.strip_prefix('.') {
        !fraction.is_empty() && fraction.chars().all(|ch| ch.is_ascii_digit())
    } else if let Some((integer, fraction)) = unsigned.split_once('.') {
        !integer.is_empty()
            && !fraction.is_empty()
            && integer.chars().all(|ch| ch.is_ascii_digit())
            && fraction.chars().all(|ch| ch.is_ascii_digit())
    } else {
        !unsigned.is_empty() && unsigned.chars().all(|ch| ch.is_ascii_digit())
    };
    if !valid {
        return None;
    }
    t.parse::<f64>().ok()
}

fn parse_x_axis_source(rest: &str, rest_start: usize) -> ParseResult<ParsedAxisStatement> {
    let input = SpannedSlice::new(rest, rest_start, rest_start + rest.len()).trim();
    if input.text.is_empty() {
        return Err(XyChartIssue::insertion_point(
            "x-axis requires data".to_string(),
            input.start,
        ));
    }

    if input.text.starts_with('[') {
        let (categories, _source) = parse_text_list_source(input.text, input.start)?;
        return Ok(ParsedAxisStatement {
            title: None,
            data: ParsedAxisData::Band {
                categories,
                #[cfg(feature = "editor-facts")]
                source: _source,
            },
        });
    }
    if let Some((min, max, _source)) = try_parse_range_source(input.text, input.start)? {
        return Ok(ParsedAxisStatement {
            title: None,
            data: ParsedAxisData::Range {
                min,
                max,
                #[cfg(feature = "editor-facts")]
                source: _source,
            },
        });
    }

    let (title, tail) = parse_text_prefix_source(input.text, input.start)?;
    let tail = tail.trim();
    if tail.text.is_empty() {
        return Ok(ParsedAxisStatement {
            title: Some(title),
            data: ParsedAxisData::None,
        });
    }
    if tail.text.starts_with('[') {
        let (categories, _source) = parse_text_list_source(tail.text, tail.start)?;
        return Ok(ParsedAxisStatement {
            title: Some(title),
            data: ParsedAxisData::Band {
                categories,
                #[cfg(feature = "editor-facts")]
                source: _source,
            },
        });
    }
    if let Some((min, max, _source)) = try_parse_range_source(tail.text, tail.start)? {
        return Ok(ParsedAxisStatement {
            title: Some(title),
            data: ParsedAxisData::Range {
                min,
                max,
                #[cfg(feature = "editor-facts")]
                source: _source,
            },
        });
    }
    Err(XyChartIssue::exact(
        "invalid x-axis data".to_string(),
        SourceSpan::new(tail.start, tail.end),
    ))
}

fn parse_y_axis_source(rest: &str, rest_start: usize) -> ParseResult<ParsedAxisStatement> {
    let input = SpannedSlice::new(rest, rest_start, rest_start + rest.len()).trim();
    if input.text.is_empty() {
        return Err(XyChartIssue::insertion_point(
            "y-axis requires data".to_string(),
            input.start,
        ));
    }
    if input.text.starts_with('[') {
        return Err(XyChartIssue::exact(
            "y-axis does not support band data".to_string(),
            SourceSpan::new(input.start, input.end),
        ));
    }
    if let Some((min, max, _source)) = try_parse_range_source(input.text, input.start)? {
        return Ok(ParsedAxisStatement {
            title: None,
            data: ParsedAxisData::Range {
                min,
                max,
                #[cfg(feature = "editor-facts")]
                source: _source,
            },
        });
    }

    let (title, tail) = parse_text_prefix_source(input.text, input.start)?;
    let tail = tail.trim();
    if tail.text.is_empty() {
        return Ok(ParsedAxisStatement {
            title: Some(title),
            data: ParsedAxisData::None,
        });
    }
    if tail.text.starts_with('[') {
        return Err(XyChartIssue::exact(
            "y-axis does not support band data".to_string(),
            SourceSpan::new(tail.start, tail.end),
        ));
    }
    if let Some((min, max, _source)) = try_parse_range_source(tail.text, tail.start)? {
        return Ok(ParsedAxisStatement {
            title: Some(title),
            data: ParsedAxisData::Range {
                min,
                max,
                #[cfg(feature = "editor-facts")]
                source: _source,
            },
        });
    }
    Err(XyChartIssue::exact(
        "invalid y-axis data".to_string(),
        SourceSpan::new(tail.start, tail.end),
    ))
}

fn parse_plot_stmt_source(rest: &str, rest_start: usize) -> ParseResult<ParsedPlotStatement> {
    let input = SpannedSlice::new(rest, rest_start, rest_start + rest.len()).trim();
    if input.text.is_empty() {
        return Err(XyChartIssue::insertion_point(
            "plot requires data".to_string(),
            input.start,
        ));
    }

    let (title, data_input) = if input.text.starts_with('[') {
        (None, input)
    } else {
        let (title, tail) = parse_text_prefix_source(input.text, input.start)?;
        let tail = tail.trim();
        if tail.text.is_empty() {
            return Err(XyChartIssue::insertion_point(
                "plot data missing".to_string(),
                input.end,
            ));
        }
        if !tail.text.starts_with('[') {
            return Err(XyChartIssue::exact(
                "plot data missing".to_string(),
                SourceSpan::new(tail.start, tail.end),
            ));
        }
        (Some(title), tail)
    };
    let (data, _data_source) =
        parse_data_point_list_in_brackets_spanned(data_input.text, data_input.start)?;
    Ok(ParsedPlotStatement {
        title,
        data,
        #[cfg(feature = "editor-facts")]
        data_source: _data_source,
    })
}

fn try_parse_range_source(
    input: &str,
    input_start: usize,
) -> ParseResult<Option<(f64, f64, SourceSpan)>> {
    let input = SpannedSlice::new(input, input_start, input_start + input.len()).trim();
    let Some((first, after_first)) = take_number_token(input.text) else {
        return Ok(None);
    };
    let after_first_start = input.start + first.len();
    let after_first = SpannedSlice::new(after_first, after_first_start, input.end).trim();
    if !after_first.text.starts_with("-->") {
        return Ok(None);
    }

    let first_value = parse_number(first).ok_or_else(|| {
        XyChartIssue::exact(
            format!("invalid number: {first}"),
            SourceSpan::new(input.start, input.start + first.len()),
        )
    })?;
    let second_input = SpannedSlice::new(
        &after_first.text[3..],
        after_first.start + 3,
        after_first.end,
    )
    .trim();
    let Some((second, trailing)) = take_number_token(second_input.text) else {
        if second_input.text.is_empty() {
            return Err(XyChartIssue::insertion_point(
                "expected number".to_string(),
                second_input.start,
            ));
        }
        let token_len = second_input
            .text
            .find(is_ecmascript_whitespace)
            .unwrap_or(second_input.text.len());
        return Err(XyChartIssue::exact(
            format!("invalid number: {}", &second_input.text[..token_len]),
            SourceSpan::new(second_input.start, second_input.start + token_len),
        ));
    };
    let trailing_start = second_input.start + second.len();
    let trailing = SpannedSlice::new(trailing, trailing_start, second_input.end).trim();
    if !trailing.text.is_empty() {
        return Err(XyChartIssue::exact(
            "unexpected trailing tokens after range".to_string(),
            SourceSpan::new(trailing.start, trailing.end),
        ));
    }

    let second_value = parse_number(second).ok_or_else(|| {
        XyChartIssue::exact(
            format!("invalid number: {second}"),
            SourceSpan::new(second_input.start, second_input.start + second.len()),
        )
    })?;
    let source_end = second_input.start + second.len();
    Ok(Some((
        first_value,
        second_value,
        SourceSpan::new(input.start, source_end),
    )))
}

fn take_number_token(input: &str) -> Option<(&str, &str)> {
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
    Some((&input[..idx], &input[idx..]))
}

fn parse_text_list_source(
    input: &str,
    input_start: usize,
) -> ParseResult<(Vec<String>, SourceSpan)> {
    let (inner, inner_start) = extract_bracket_inner_spanned(input, input_start)?;
    let source = SpannedSlice::new(inner, inner_start, inner_start + inner.len()).trim();
    if source.text.is_empty() {
        return Err(XyChartIssue::insertion_point(
            "empty category".to_string(),
            source.start,
        ));
    }

    let mut categories = Vec::new();
    let parts = split_top_level_commas_spanned(inner, inner_start);
    for part in parts {
        let part = part.trim();
        if part.text.is_empty() {
            return Err(XyChartIssue::insertion_point(
                "empty category".to_string(),
                part.start,
            ));
        }
        categories.push(parse_text_source(part.text, part.start)?.text);
    }
    Ok((categories, SourceSpan::new(source.start, source.end)))
}

fn parse_data_point_list_in_brackets_spanned(
    input: &str,
    input_start: usize,
) -> ParseResult<(Vec<ParsedDataPoint>, SourceSpan)> {
    let t = input.trim_start_matches(is_ecmascript_whitespace);
    let t_start = input_start + input.len() - t.len();
    let (inner, inner_start) = extract_bracket_inner_spanned(t, t_start)?;
    let source = SpannedSlice::new(inner, inner_start, inner_start + inner.len()).trim();
    if source.text.is_empty() {
        return Err(XyChartIssue::insertion_point(
            "plot data cannot be empty".to_string(),
            source.start,
        ));
    }
    let parts = split_top_level_commas_spanned(inner, inner_start);
    let mut out = Vec::new();
    for part in parts {
        let trimmed = part.trim();
        if trimmed.text.is_empty() {
            return Err(XyChartIssue::insertion_point(
                "empty number".to_string(),
                trimmed.start,
            ));
        }
        out.push(parse_data_point_spanned(trimmed)?);
    }
    Ok((out, SourceSpan::new(source.start, source.end)))
}

fn parse_data_point_spanned(part: SpannedSlice<'_>) -> ParseResult<ParsedDataPoint> {
    if let Some(quote_rel) = part.text.find('"') {
        let number_source =
            SpannedSlice::new(&part.text[..quote_rel], part.start, part.start + quote_rel).trim();
        let number_part = number_source.text;
        if number_part.is_empty() {
            return Err(XyChartIssue::insertion_point(
                "expected number".to_string(),
                part.start,
            ));
        }
        let number_start = number_source.start;
        let value = parse_number(number_part).ok_or_else(|| {
            XyChartIssue::exact(
                format!("invalid number: {number_part}"),
                SourceSpan::new(number_start, number_start + number_part.len()),
            )
        })?;
        let label =
            parse_data_point_label_spanned(&part.text[quote_rel..], part.start + quote_rel)?;
        return Ok(ParsedDataPoint {
            value,
            label: label.text,
            #[cfg(feature = "editor-facts")]
            label_span: Some(SourceSpan::new(label.start, label.end)),
        });
    }

    let value = parse_number(part.text).ok_or_else(|| {
        XyChartIssue::exact(
            format!("invalid number: {}", part.text),
            SourceSpan::new(part.start, part.end),
        )
    })?;
    Ok(ParsedDataPoint {
        value,
        label: String::new(),
        #[cfg(feature = "editor-facts")]
        label_span: None,
    })
}

fn parse_data_point_label_spanned(input: &str, input_start: usize) -> ParseResult<SpannedText> {
    let Some(body) = input.strip_prefix('"') else {
        return Err(XyChartIssue::insertion_point(
            "expected data label".to_string(),
            input_start,
        ));
    };
    let Some(end) = body.find('"') else {
        return Err(XyChartIssue::insertion_point(
            "unterminated data label".to_string(),
            input_start,
        ));
    };
    let rest = &body[end + 1..];
    if !rest.trim_matches(is_ecmascript_whitespace).is_empty() {
        let leading = rest
            .len()
            .saturating_sub(rest.trim_start_matches(is_ecmascript_whitespace).len());
        return Err(XyChartIssue::exact(
            "unexpected trailing tokens after data label".to_string(),
            SourceSpan::new(
                input_start + 1 + end + 1 + leading,
                input_start + input.len(),
            ),
        ));
    }
    Ok(SpannedText {
        text: body[..end].to_string(),
        #[cfg(feature = "editor-facts")]
        start: input_start + 1,
        #[cfg(feature = "editor-facts")]
        end: input_start + 1 + end,
    })
}

fn extract_bracket_inner_spanned(input: &str, input_start: usize) -> ParseResult<(&str, usize)> {
    let t = input.trim_start_matches(is_ecmascript_whitespace);
    let t_start = input_start + input.len().saturating_sub(t.len());
    if !t.starts_with('[') {
        return Err(XyChartIssue::insertion_point(
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
            return Err(XyChartIssue::exact(
                "unbalanced '['".to_string(),
                SourceSpan::new(t_start + idx, t_start + idx + ch.len_utf8()),
            ));
        }
        if ch == ']' {
            let inner = &t[1..idx];
            let rest = &t[idx + 1..];
            if !rest.trim_matches(is_ecmascript_whitespace).is_empty() {
                let trailing_start = rest
                    .len()
                    .saturating_sub(rest.trim_start_matches(is_ecmascript_whitespace).len());
                let trailing = rest.trim_matches(is_ecmascript_whitespace);
                let start = t_start + idx + 1 + trailing_start;
                return Err(XyChartIssue::exact(
                    "unexpected trailing tokens after ']'".to_string(),
                    SourceSpan::new(start, start + trailing.len()),
                ));
            }
            return Ok((inner, t_start + 1));
        }
        idx += ch.len_utf8();
    }

    Err(XyChartIssue::insertion_point(
        "unbalanced ']'".to_string(),
        t_start + t.len(),
    ))
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

struct XyChartSyntaxFragments {
    statements: Vec<SpannedStatement>,
}

impl SpannedStatement {
    fn trimmed_start(&self) -> usize {
        self.start
            + self
                .text
                .len()
                .saturating_sub(self.text.trim_start_matches(is_ecmascript_whitespace).len())
    }
}

fn split_statements_spanned_controlled(
    input: &str,
    control: &OperationControl,
) -> OperationControlResult<XyChartSyntaxFragments> {
    control.checkpoint()?;
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut cur_start = 0usize;
    let mut in_quote = false;
    let mut in_md = false;
    let mut in_acc_descr_block = false;
    let mut in_accessibility_line = false;
    let mut bracket_depth = 0usize;
    let mut word_end = 0usize;
    let mut iter = input.char_indices().peekable();
    let mut next_checkpoint = 0usize;

    while let Some((idx, ch)) = iter.next() {
        if idx >= next_checkpoint {
            control.checkpoint()?;
            next_checkpoint = idx.saturating_add(4096);
        }
        if cur.is_empty() {
            cur_start = idx;
        }

        // Accessibility values are lexer states, not ordinary text. In particular,
        // quotes and command words inside them cannot introduce a new statement.
        if in_acc_descr_block {
            cur.push(ch);
            if ch == '}' {
                in_acc_descr_block = false;
                out.push(SpannedStatement {
                    text: std::mem::take(&mut cur),
                    start: cur_start,
                });
                cur_start = idx + ch.len_utf8();
            }
            continue;
        }
        if in_accessibility_line {
            if ch == '\n' {
                in_accessibility_line = false;
                out.push(SpannedStatement {
                    text: std::mem::take(&mut cur),
                    start: cur_start,
                });
                cur_start = idx + ch.len_utf8();
            } else {
                cur.push(ch);
            }
            continue;
        }

        if in_md {
            cur.push(ch);
            if ch == '`'
                && iter.peek().is_some_and(|(_, next)| *next == '"')
                && let Some((_quote_idx, quote)) = iter.next()
            {
                cur.push(quote);
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

        if !in_acc_descr_block
            && ch == '%'
            && iter.peek().is_some_and(|(_, next)| *next == '%')
            && !input[idx..].starts_with("%%{")
        {
            let mut next_statement_start = input.len();
            for (comment_idx, comment_ch) in iter.by_ref() {
                if comment_idx >= next_checkpoint {
                    control.checkpoint()?;
                    next_checkpoint = comment_idx.saturating_add(4096);
                }
                if comment_ch == '\n' {
                    next_statement_start = comment_idx + comment_ch.len_utf8();
                    break;
                }
            }
            if !cur.trim_matches(is_ecmascript_whitespace).is_empty() {
                out.push(SpannedStatement {
                    text: std::mem::take(&mut cur),
                    start: cur_start,
                });
            } else {
                cur.clear();
            }
            cur_start = next_statement_start;
            bracket_depth = 0;
            continue;
        }

        if idx < word_end {
            cur.push(ch);
            continue;
        }
        if ch.is_ascii_alphabetic() {
            let (keyword, word_len) = xychart_word(&input[idx..]);
            word_end = idx + word_len;
            if bracket_depth == 0
                && let Some(keyword) = keyword
            {
                if !cur.trim_matches(is_ecmascript_whitespace).is_empty() {
                    out.push(SpannedStatement {
                        text: std::mem::take(&mut cur),
                        start: cur_start,
                    });
                } else {
                    cur.clear();
                }
                cur_start = idx;
                in_accessibility_line = keyword == XyChartKeyword::AccTitle
                    || (keyword == XyChartKeyword::AccDescr
                        && input[word_end..]
                            .trim_start_matches(is_ecmascript_whitespace)
                            .starts_with(':'));
            }
            cur.push(ch);
            continue;
        }

        match ch {
            '{' if !in_acc_descr_block && is_xychart_accessibility_block_prefix(&cur) => {
                in_acc_descr_block = true;
            }
            '[' => bracket_depth += 1,
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }

        if (ch == '\n' || ch == ';') && !in_acc_descr_block {
            out.push(SpannedStatement {
                text: std::mem::take(&mut cur),
                start: cur_start,
            });
            cur_start = idx + ch.len_utf8();
            bracket_depth = 0;
            continue;
        }

        cur.push(ch);
    }

    if !in_acc_descr_block && !cur.is_empty() {
        out.push(SpannedStatement {
            text: cur,
            start: cur_start,
        });
    }
    control.checkpoint()?;
    Ok(XyChartSyntaxFragments { statements: out })
}

fn is_xychart_accessibility_block_prefix(statement: &str) -> bool {
    let statement = statement.trim_start_matches(is_ecmascript_whitespace);
    statement
        .get(.."accDescr".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("accDescr"))
        && statement["accDescr".len()..]
            .chars()
            .all(is_ecmascript_whitespace)
}
