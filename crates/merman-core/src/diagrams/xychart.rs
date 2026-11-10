#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
use crate::sanitize::sanitize_text;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol,
};
use crate::{Error, MermaidConfig, Result, SourceSpan, family::CombinedSemanticFailure};
use merman_model::text::is_ecmascript_whitespace;
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static XYCHART_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
fn reset_xychart_syntax_construction_count() {
    XYCHART_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
fn xychart_syntax_construction_count() -> usize {
    XYCHART_SYNTAX_CONSTRUCTION_COUNT.get()
}

pub(crate) use merman_model::families::xy_chart::*;

pub(crate) fn sanitize_common_db_fields(model: &mut XyChartDiagramModel, config: &MermaidConfig) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
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

struct XyChartSemanticSource {
    model: XyChartDiagramModel,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

struct XyChartParseOutcome {
    source: XyChartSemanticSource,
    first_error: Option<Error>,
}

impl XyChartParseOutcome {
    fn into_strict_result(
        self,
    ) -> std::result::Result<XyChartSemanticSource, CombinedSemanticFailure> {
        match self.first_error {
            Some(error) => Err(CombinedSemanticFailure::parser_recovery(
                #[cfg(feature = "editor-facts")]
                "xychart",
                error,
                #[cfg(feature = "editor-facts")]
                self.source.editor_facts,
            )),
            None => Ok(self.source),
        }
    }
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

struct ParsedAxisStatement {
    title: Option<SpannedText>,
    data: ParsedAxisData,
}

struct ParsedPlotStatement {
    title: Option<SpannedText>,
    data: Vec<ParsedDataPoint>,
    #[cfg(feature = "editor-facts")]
    data_source: SourceSpan,
}

enum ParsedXyChartStatement {
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
                    vec![(crate::utils::format_js_number(min), data.first().copied())]
                } else {
                    let step = (max - min) / ((data.len() as f64) - 1.0);
                    data.iter()
                        .enumerate()
                        .map(|(index, datum)| {
                            (
                                crate::utils::format_js_number(min + (index as f64) * step),
                                Some(*datum),
                            )
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

fn normalize_xychart_error(error: Error, diagram_type: &str, fallback_span: SourceSpan) -> Error {
    match error {
        Error::DiagramParse {
            diagram_type,
            diagnostic,
        } if diagnostic.span().is_some() => Error::DiagramParse {
            diagram_type,
            diagnostic,
        },
        Error::DiagramParse { diagnostic, .. } => {
            Error::diagram_parse_exact(diagram_type, diagnostic.message(), fallback_span)
        }
        error => Error::diagram_parse_exact(diagram_type, error.to_string(), fallback_span),
    }
}

fn record_xychart_error(first_error: &mut Option<Error>, error: Error) {
    first_error.get_or_insert(error);
}

#[cfg(feature = "editor-facts")]
fn push_xychart_axis_facts(
    facts: &mut EditorSemanticFacts,
    axis: &ParsedAxisStatement,
    detail: &'static str,
) {
    if let Some(title) = axis.title.as_ref() {
        push_xychart_payload_fact(
            facts,
            title.as_str(),
            SourceSpan::new(title.start, title.end),
            detail,
            EditorSemanticKind::String,
        );
    }
    let source = match &axis.data {
        ParsedAxisData::None => None,
        ParsedAxisData::Band { source, .. } | ParsedAxisData::Range { source, .. } => Some(source),
    };
    if let Some(source) = source {
        facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            SourceSpan::new(source.start, source.end),
        ));
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

#[cfg(feature = "editor-facts")]
fn push_xychart_plot_statement_facts(
    facts: &mut EditorSemanticFacts,
    plot: &ParsedPlotStatement,
    detail: &'static str,
    data_label_detail: &'static str,
) {
    if let Some(title) = plot.title.as_ref() {
        push_xychart_payload_fact(
            facts,
            title.as_str(),
            SourceSpan::new(title.start, title.end),
            detail,
            EditorSemanticKind::String,
        );
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        SourceSpan::new(plot.data_source.start, plot.data_source.end),
    ));
    for point in &plot.data {
        let Some(span) = point.label_span else {
            continue;
        };
        push_xychart_payload_fact(
            facts,
            &point.label,
            span,
            data_label_detail,
            EditorSemanticKind::String,
        );
    }
}

fn parse_xychart_statement(
    statement_text: &str,
    statement_start: usize,
    diagram_type: &str,
) -> Result<ParsedXyChartStatement> {
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

    Err(Error::diagram_parse_exact(
        diagram_type.to_string(),
        format!("unexpected xychart statement: {statement_text}"),
        statement_span,
    ))
}

fn construct_xychart_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    config: &MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<XyChartParseOutcome> {
    control.checkpoint()?;
    #[cfg(test)]
    XYCHART_SYNTAX_CONSTRUCTION_COUNT.set(XYCHART_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let syntax = split_statements_spanned_controlled(code, control)?;
    let mut statements = syntax.statements.into_iter().filter(|statement| {
        !statement
            .text
            .trim_matches(is_ecmascript_whitespace)
            .is_empty()
    });
    let Some(header) = statements.next() else {
        return Ok(XyChartParseOutcome {
            source: XyChartSemanticSource {
                model: XyChartState::new(config).into_semantic_model(None, None, None),
                #[cfg(feature = "editor-facts")]
                editor_facts: editor_demand
                    .needs_recovery()
                    .then(EditorSemanticFacts::new),
            },
            first_error: None,
        });
    };

    #[cfg(feature = "editor-facts")]
    let mut editor_facts = match editor_demand {
        EditorFactsDemand::Omit | EditorFactsDemand::Recovery => None,
        EditorFactsDemand::Capture => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            Some(EditorSemanticFacts::new())
        }
    };
    let mut first_error = None;
    let mut state = XyChartState::new(config);
    let header_trimmed = header.text.trim_matches(is_ecmascript_whitespace);
    let header_start = header.trimmed_start();
    let header_span = SourceSpan::new(header_start, header_start + header_trimmed.len());
    if let Err(error) = parse_header(&header.text, header_start, &mut state) {
        let error = normalize_xychart_error(error, diagram_type, header_span);
        first_error = Some(error);
    }
    #[cfg(feature = "editor-facts")]
    if let Some((prefix_len, _)) = header_token_len_and_rest(header_trimmed)
        && let Some(editor_facts) = &mut editor_facts
    {
        editor_facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            SourceSpan::new(header_start, header_start + prefix_len),
        ));
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
        let statement_span =
            SourceSpan::new(statement_start, statement_start + statement_text.len());
        let parsed = match parse_xychart_statement(statement_text, statement_start, diagram_type) {
            Ok(parsed) => parsed,
            Err(error) => {
                record_xychart_error(
                    &mut first_error,
                    normalize_xychart_error(error, diagram_type, statement_span),
                );
                continue;
            }
        };

        match parsed {
            ParsedXyChartStatement::Title(value) => {
                #[cfg(feature = "editor-facts")]
                if let Some(editor_facts) = &mut editor_facts {
                    editor_facts.push_directive_prefix("title");
                }
                #[cfg(feature = "editor-facts")]
                if let Some(facts) = &mut editor_facts {
                    push_xychart_payload_fact(
                        facts,
                        value.as_str(),
                        SourceSpan::new(value.start, value.end),
                        "xychart title",
                        EditorSemanticKind::String,
                    );
                }
                title = Some(
                    value
                        .text
                        .trim_matches(is_ecmascript_whitespace)
                        .to_string(),
                );
            }
            ParsedXyChartStatement::AccessibilityTitle(value) => {
                #[cfg(feature = "editor-facts")]
                if let Some(editor_facts) = &mut editor_facts {
                    editor_facts.push_directive_prefix("accTitle");
                }
                #[cfg(feature = "editor-facts")]
                if !value.text.is_empty()
                    && let Some(facts) = &mut editor_facts
                {
                    push_xychart_payload_fact(
                        facts,
                        value.as_str(),
                        SourceSpan::new(value.start, value.end),
                        "xychart accessibility title",
                        EditorSemanticKind::String,
                    );
                }
                acc_title = Some(value.text);
            }
            ParsedXyChartStatement::AccessibilityDescription(value) => {
                #[cfg(feature = "editor-facts")]
                if let Some(editor_facts) = &mut editor_facts {
                    editor_facts.push_directive_prefix("accDescr");
                }
                #[cfg(feature = "editor-facts")]
                if !value.text.is_empty()
                    && let Some(facts) = &mut editor_facts
                {
                    push_xychart_payload_fact(
                        facts,
                        value.as_str(),
                        SourceSpan::new(value.start, value.end),
                        "xychart accessibility description",
                        EditorSemanticKind::String,
                    );
                }
                acc_descr = Some(value.text);
            }
            ParsedXyChartStatement::XAxis(axis) => {
                #[cfg(feature = "editor-facts")]
                if let Some(facts) = &mut editor_facts {
                    push_xychart_axis_facts(facts, &axis, "xychart x-axis");
                }
                apply_x_axis_statement(axis, &mut state, config);
            }
            ParsedXyChartStatement::YAxis(axis) => {
                #[cfg(feature = "editor-facts")]
                if let Some(facts) = &mut editor_facts {
                    push_xychart_axis_facts(facts, &axis, "xychart y-axis");
                }
                apply_y_axis_statement(axis, &mut state, config);
            }
            ParsedXyChartStatement::Line(plot) => {
                #[cfg(feature = "editor-facts")]
                if let Some(facts) = &mut editor_facts {
                    push_xychart_plot_statement_facts(
                        facts,
                        &plot,
                        "xychart line",
                        "xychart line data label",
                    );
                }
                state.add_line_data(
                    plot.title
                        .as_ref()
                        .and_then(|title| plot_title_value(title.as_str(), config)),
                    plot.data,
                    config,
                );
            }
            ParsedXyChartStatement::Bar(plot) => {
                #[cfg(feature = "editor-facts")]
                if let Some(facts) = &mut editor_facts {
                    push_xychart_plot_statement_facts(
                        facts,
                        &plot,
                        "xychart bar",
                        "xychart bar data label",
                    );
                }
                state.add_bar_data(
                    plot.title
                        .as_ref()
                        .and_then(|title| plot_title_value(title.as_str(), config)),
                    plot.data,
                );
            }
        }
    }

    control.checkpoint()?;
    Ok(XyChartParseOutcome {
        source: XyChartSemanticSource {
            model: state.into_semantic_model(title, acc_title, acc_descr),
            #[cfg(feature = "editor-facts")]
            editor_facts: editor_demand.finish_facts(editor_facts),
        },
        first_error,
    })
}

pub(crate) fn parse_xychart_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<XyChartDiagramModel>> {
    let construction = construct_xychart_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        &metadata.semantic_config,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?
    .into_strict_result();
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = source.editor_facts;
            let mut model = source.model;
            sanitize_common_db_fields(&mut model, &metadata.semantic_config);
            crate::family::SemanticConstruction {
                model: Ok(model),
                #[cfg(feature = "editor-facts")]
                editor_facts,
            }
        }
        Err(failure) => {
            #[cfg(feature = "editor-facts")]
            {
                match editor_demand {
                    EditorFactsDemand::Capture | EditorFactsDemand::Recovery => {
                        let (error, facts) = failure.into_parts();
                        crate::family::SemanticConstruction {
                            model: Err(error),
                            editor_facts: Some(facts),
                        }
                    }
                    EditorFactsDemand::Omit => crate::family::SemanticConstruction {
                        model: Err(failure.into_error()),
                        editor_facts: None,
                    },
                }
            }
            #[cfg(not(feature = "editor-facts"))]
            {
                crate::family::SemanticConstruction {
                    model: Err(failure.into_error()),
                }
            }
        }
    })
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_xychart(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let model =
        parse_xychart_model_for_render_controlled(code, meta, &crate::OperationControl::new())
            .expect("a private parse control cannot be cancelled")?;
    render_model_to_compat_json(&model, meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_xychart_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construct_xychart_semantic_source_controlled(
            code,
            &meta.diagram_type,
            meta.effective_config.as_config(),
            EditorFactsDemand::Capture,
            control,
        )?
        .into_strict_result(),
        |source| {
            let model = source
                .model
                .into_render_model(meta.effective_config.as_config());
            (
                render_model_to_compat_json(&model, meta),
                source.editor_facts.unwrap_or_default(),
            )
        },
        CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_xychart_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<XyChartDiagramRenderModel> {
    parse_xychart_model_for_render_controlled(code, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_xychart_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<XyChartDiagramRenderModel>> {
    let outcome = construct_xychart_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    control.checkpoint()?;
    Ok(outcome
        .into_strict_result()
        .map(|source| {
            source
                .model
                .into_render_model(meta.effective_config.as_config())
        })
        .map_err(CombinedSemanticFailure::into_error))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &XyChartDiagramRenderModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::xy_chart::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
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

fn parse_header(stmt: &str, statement_start: usize, state: &mut XyChartState) -> Result<()> {
    let t = stmt.trim_matches(is_ecmascript_whitespace);
    let trimmed_start = statement_start;
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
    let rem = rest.trim_matches(is_ecmascript_whitespace);
    if rem.is_empty() {
        return Ok(());
    }
    if !rest.starts_with(is_ecmascript_whitespace) {
        let rem_start = trimmed_start + prefix.len();
        return Err(Error::diagram_parse_exact(
            "xychart".to_string(),
            format!("unexpected token after {prefix}: {rem}"),
            SourceSpan::new(rem_start, trimmed_start + t.len()),
        ));
    }

    if rem.eq_ignore_ascii_case("vertical") || rem.eq_ignore_ascii_case("horizontal") {
        state.set_orientation(rem);
        return Ok(());
    }

    let rem_start = trimmed_start + t.len().saturating_sub(rem.len());
    Err(Error::diagram_parse_exact(
        "xychart".to_string(),
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

#[cfg(feature = "editor-facts")]
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

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    #[cfg(feature = "editor-facts")]
    start: usize,
    #[cfg(feature = "editor-facts")]
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

fn parse_text_source(input: &str, input_start: usize) -> Result<SpannedText> {
    let (value, tail) = parse_text_prefix_source(input, input_start)?;
    let trailing = tail.trim();
    if !trailing.text.is_empty() {
        return Err(Error::diagram_parse_exact(
            "xychart".to_string(),
            "unexpected trailing tokens after text".to_string(),
            SourceSpan::new(trailing.start, trailing.end),
        ));
    }
    Ok(value)
}

fn parse_text_prefix_source<'a>(
    input: &'a str,
    input_start: usize,
) -> Result<(SpannedText, SpannedSlice<'a>)> {
    let text = input.trim_start_matches(is_ecmascript_whitespace);
    let text_start = input_start + input.len() - text.len();
    if text.is_empty() {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
            "expected text".to_string(),
            text_start,
        ));
    }

    if let Some(body) = text.strip_prefix("\"`") {
        let Some(end) = body.find("`\"") else {
            return Err(Error::diagram_parse_insertion_point(
                "xychart".to_string(),
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
            return Err(Error::diagram_parse_insertion_point(
                "xychart".to_string(),
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
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
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
                return Err(Error::diagram_parse_exact(
                    "xychart".to_string(),
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
            return Err(Error::diagram_parse_exact(
                "xychart".to_string(),
                format!("unexpected token in text: {ch}"),
                SourceSpan::new(text_start + offset - ch.len_utf8(), text_start + offset),
            ));
        }
        value.push(ch);
    }
    if value.is_empty() {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
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

fn parse_colon_value_source(input: &str, input_start: usize, keyword: &str) -> Result<SpannedText> {
    let input = SpannedSlice::new(input, input_start, input_start + input.len()).trim();
    let Some(value) = input.text.strip_prefix(':') else {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
            format!("expected ':' after {keyword}"),
            input.start,
        ));
    };
    let value = SpannedSlice::new(value, input.start + 1, input.end)
        .trim()
        .to_text();
    Ok(value)
}

fn parse_acc_descr_source(input: &str, input_start: usize) -> Result<SpannedText> {
    let input = SpannedSlice::new(input, input_start, input_start + input.len()).trim();
    if input.text.starts_with(':') {
        let value = SpannedSlice::new(&input.text[1..], input.start + 1, input.end)
            .trim()
            .to_text();
        return Ok(value);
    }

    let Some(body) = input.text.strip_prefix('{') else {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
            "expected ':' or '{' after accDescr".to_string(),
            input.start,
        ));
    };
    let Some(close) = body.find('}') else {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
            "unterminated accDescr block".to_string(),
            input.end,
        ));
    };
    let body_source =
        SpannedSlice::new(&body[..close], input.start + 1, input.start + 1 + close).trim();
    let trailing =
        SpannedSlice::new(&body[close + 1..], input.start + 1 + close + 1, input.end).trim();
    if !trailing.text.is_empty() {
        return Err(Error::diagram_parse_exact(
            "xychart".to_string(),
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

fn parse_x_axis_source(rest: &str, rest_start: usize) -> Result<ParsedAxisStatement> {
    let input = SpannedSlice::new(rest, rest_start, rest_start + rest.len()).trim();
    if input.text.is_empty() {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
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
    Err(Error::diagram_parse_exact(
        "xychart".to_string(),
        "invalid x-axis data".to_string(),
        SourceSpan::new(tail.start, tail.end),
    ))
}

fn parse_y_axis_source(rest: &str, rest_start: usize) -> Result<ParsedAxisStatement> {
    let input = SpannedSlice::new(rest, rest_start, rest_start + rest.len()).trim();
    if input.text.is_empty() {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
            "y-axis requires data".to_string(),
            input.start,
        ));
    }
    if input.text.starts_with('[') {
        return Err(Error::diagram_parse_exact(
            "xychart".to_string(),
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
        return Err(Error::diagram_parse_exact(
            "xychart".to_string(),
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
    Err(Error::diagram_parse_exact(
        "xychart".to_string(),
        "invalid y-axis data".to_string(),
        SourceSpan::new(tail.start, tail.end),
    ))
}

fn parse_plot_stmt_source(rest: &str, rest_start: usize) -> Result<ParsedPlotStatement> {
    let input = SpannedSlice::new(rest, rest_start, rest_start + rest.len()).trim();
    if input.text.is_empty() {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
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
            return Err(Error::diagram_parse_insertion_point(
                "xychart".to_string(),
                "plot data missing".to_string(),
                input.end,
            ));
        }
        if !tail.text.starts_with('[') {
            return Err(Error::diagram_parse_exact(
                "xychart".to_string(),
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
) -> Result<Option<(f64, f64, SourceSpan)>> {
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
        Error::diagram_parse_exact(
            "xychart".to_string(),
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
            return Err(Error::diagram_parse_insertion_point(
                "xychart".to_string(),
                "expected number".to_string(),
                second_input.start,
            ));
        }
        let token_len = second_input
            .text
            .find(is_ecmascript_whitespace)
            .unwrap_or(second_input.text.len());
        return Err(Error::diagram_parse_exact(
            "xychart".to_string(),
            format!("invalid number: {}", &second_input.text[..token_len]),
            SourceSpan::new(second_input.start, second_input.start + token_len),
        ));
    };
    let trailing_start = second_input.start + second.len();
    let trailing = SpannedSlice::new(trailing, trailing_start, second_input.end).trim();
    if !trailing.text.is_empty() {
        return Err(Error::diagram_parse_exact(
            "xychart".to_string(),
            "unexpected trailing tokens after range".to_string(),
            SourceSpan::new(trailing.start, trailing.end),
        ));
    }

    let second_value = parse_number(second).ok_or_else(|| {
        Error::diagram_parse_exact(
            "xychart".to_string(),
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

fn parse_text_list_source(input: &str, input_start: usize) -> Result<(Vec<String>, SourceSpan)> {
    let (inner, inner_start) = extract_bracket_inner_spanned(input, input_start)?;
    let source = SpannedSlice::new(inner, inner_start, inner_start + inner.len()).trim();
    if source.text.is_empty() {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
            "empty category".to_string(),
            source.start,
        ));
    }

    let mut categories = Vec::new();
    let parts = split_top_level_commas_spanned(inner, inner_start);
    for part in parts {
        let part = part.trim();
        if part.text.is_empty() {
            return Err(Error::diagram_parse_insertion_point(
                "xychart".to_string(),
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
) -> Result<(Vec<ParsedDataPoint>, SourceSpan)> {
    let t = input.trim_start_matches(is_ecmascript_whitespace);
    let t_start = input_start + input.len() - t.len();
    let (inner, inner_start) = extract_bracket_inner_spanned(t, t_start)?;
    let source = SpannedSlice::new(inner, inner_start, inner_start + inner.len()).trim();
    if source.text.is_empty() {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
            "plot data cannot be empty".to_string(),
            source.start,
        ));
    }
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
        out.push(parse_data_point_spanned(trimmed)?);
    }
    Ok((out, SourceSpan::new(source.start, source.end)))
}

fn parse_data_point_spanned(part: SpannedSlice<'_>) -> Result<ParsedDataPoint> {
    if let Some(quote_rel) = part.text.find('"') {
        let number_source =
            SpannedSlice::new(&part.text[..quote_rel], part.start, part.start + quote_rel).trim();
        let number_part = number_source.text;
        if number_part.is_empty() {
            return Err(Error::diagram_parse_insertion_point(
                "xychart".to_string(),
                "expected number".to_string(),
                part.start,
            ));
        }
        let number_start = number_source.start;
        let value = parse_number(number_part).ok_or_else(|| {
            Error::diagram_parse_exact(
                "xychart".to_string(),
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
        Error::diagram_parse_exact(
            "xychart".to_string(),
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

fn parse_data_point_label_spanned(input: &str, input_start: usize) -> Result<SpannedText> {
    let Some(body) = input.strip_prefix('"') else {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
            "expected data label".to_string(),
            input_start,
        ));
    };
    let Some(end) = body.find('"') else {
        return Err(Error::diagram_parse_insertion_point(
            "xychart".to_string(),
            "unterminated data label".to_string(),
            input_start,
        ));
    };
    let rest = &body[end + 1..];
    if !rest.trim_matches(is_ecmascript_whitespace).is_empty() {
        let leading = rest
            .len()
            .saturating_sub(rest.trim_start_matches(is_ecmascript_whitespace).len());
        return Err(Error::diagram_parse_exact(
            "xychart".to_string(),
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

fn extract_bracket_inner_spanned(input: &str, input_start: usize) -> Result<(&str, usize)> {
    let t = input.trim_start_matches(is_ecmascript_whitespace);
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
            if !rest.trim_matches(is_ecmascript_whitespace).is_empty() {
                let trailing_start = rest
                    .len()
                    .saturating_sub(rest.trim_start_matches(is_ecmascript_whitespace).len());
                let trailing = rest.trim_matches(is_ecmascript_whitespace);
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
    control: &crate::OperationControl,
) -> crate::OperationControlResult<XyChartSyntaxFragments> {
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

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    #[cfg(feature = "editor-facts")]
    use crate::{EditorSemanticCompleteness, EditorSemanticDiagnosticKind, EditorSemanticRole};
    use crate::{MermaidEngine, ParseDiagnosticSpanKind, ParseOptions};
    use futures::executor::block_on;
    use serde_json::json;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> String {
        let engine = MermaidEngine::new();
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
    fn xychart_empty_sources_share_the_configured_default_model_and_editor_facts() {
        let engine = MermaidEngine::new();
        let configured = concat!(
            "%%{init: {\"xyChart\": {\"chartOrientation\": \"horizontal\", ",
            "\"showTitle\": false, \"xAxis\": {\"showLabel\": false}}}}%%\nxychart",
        );
        let parsed = engine
            .parse_diagram_sync(configured, ParseOptions::strict())
            .unwrap()
            .unwrap();
        for source in ["", " \t\r\n", "%% no statements\n", ";\n"] {
            let typed = parse_xychart_model_for_render(
                source,
                &crate::diagrams::source_metadata_for_test(&parsed.meta),
            )
            .unwrap();
            assert_eq!(typed.orientation, "horizontal");
            assert!(!typed.display.show_title);
            assert!(!typed.display.x_axis.show_label);
            assert!(typed.plots.is_empty());
            let projection = merman_model::families::xy_chart::compat::project(
                &typed,
                &merman_model::presentation::compat::CompatJsonMetadata::from(&parsed.meta),
            )
            .unwrap();
            assert_eq!(projection, parsed.model);
            assert_eq!(
                parse_xychart(
                    source,
                    &crate::diagrams::source_metadata_for_test(&parsed.meta)
                )
                .unwrap(),
                projection
            );
            #[cfg(feature = "editor-facts")]
            {
                let (combined, facts) =
                    crate::family::test_support::into_result(parse_xychart_json_and_editor_facts(
                        source,
                        &crate::diagrams::source_metadata_for_test(&parsed.meta),
                        &crate::OperationControl::new(),
                    ))
                    .unwrap();
                assert_eq!(combined, projection);
                assert!(facts.symbols.is_empty());
                assert!(facts.diagnostics.is_empty());
            }
        }
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
    fn xychart_same_line_commands_preserve_text_and_bar_annotation_controls() {
        let model = parse(concat!(
            "xychart title \"Revenue\nbar [literal]\" ",
            "x-axis \"Fiscal quarter\" [First quarter, \"bar [Q2]\"] ",
            "y-axis Revenue 0 --> 10 ",
            "bar barTitle [1 \"ignored\", 2] line lineTitle [2 \"Low\", 4 \"High\"]",
        ));
        assert_eq!(model["title"], "Revenue\nbar [literal]");
        assert_eq!(model["xAxis"]["title"], "Fiscal quarter");
        assert_eq!(
            model["xAxis"]["categories"],
            json!(["Firstquarter", "bar [Q2]"])
        );
        assert_eq!(model["yAxis"]["max"], 10.0);
        assert_eq!(model["plots"].as_array().unwrap().len(), 2);
        assert_eq!(model["plots"][0]["title"], "barTitle");
        assert_eq!(
            model["plots"][0]["data"],
            json!([["Firstquarter", 1.0], ["bar [Q2]", 2.0]])
        );
        assert!(model["plots"][0].get("pointLabels").is_none());
        assert_eq!(model["plots"][1]["pointLabels"], json!(["Low", "High"]));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_same_line_editor_facts_keep_multiline_and_structured_payload_ranges() {
        let text = concat!(
            "xychart title \"Revenue\nbar [literal]\" ",
            "x-axis \"Fiscal quarter\" [First quarter, \"bar [Q2]\"] ",
            "y-axis Revenue 0 --> 10 ",
            "bar Sales [1 \"ignored\", 2] ",
            "line Forecast [2 \"Low\nline [literal]\", 4 \"High\"] %% bar [99]\n",
            "accTitle: Revenue chart; bar [98]\n",
            "accDescr {A literal \" and bar [97]\nMore %% literal description} ",
            "title \"After block\" line [3, 4]\n",
        );
        let engine = MermaidEngine::new();
        let canonical = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(canonical.model["title"], "After block");
        assert_eq!(canonical.model["plots"].as_array().unwrap().len(), 3);
        assert_eq!(canonical.model["accTitle"], "Revenue chart; bar [98]");
        assert_eq!(
            canonical.model["accDescr"],
            "A literal \" and bar [97]\nMore %% literal description"
        );
        reset_xychart_syntax_construction_count();
        let snapshot = engine
            .parse_diagram_snapshot_with_type_sync("xychart", text)
            .unwrap()
            .unwrap();
        assert_eq!(xychart_syntax_construction_count(), 1);
        assert_eq!(snapshot.outcome().parsed_model().unwrap(), &canonical.model);
        let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("XYChart snapshot must retain the same parser's editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        for payload in [
            "Revenue\nbar [literal]",
            "Fiscal quarter",
            "Sales",
            "Forecast",
            "Low\nline [literal]",
            "High",
            "Revenue chart; bar [98]",
            "A literal \" and bar [97]\nMore %% literal description",
            "After block",
        ] {
            let start = text.find(payload).unwrap();
            let selection = SourceSpan::new(start, start + payload.len());
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| { symbol.name == payload && symbol.selection == selection }),
                "missing source-backed payload {payload:?}"
            );
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload && expected.span == selection
            }));
        }
        for structured in [
            "First quarter, \"bar [Q2]\"",
            "0 --> 10",
            "1 \"ignored\", 2",
            "2 \"Low\nline [literal]\", 4 \"High\"",
            "3, 4",
        ] {
            let start = text.find(structured).unwrap();
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload
                    && expected.span == SourceSpan::new(start, start + structured.len())
            }));
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_same_line_recovery_keeps_exact_error_and_later_facts() {
        let text = "xychart x-axis [A, B] bar [1., 2] line After [3, 4]";
        let engine = MermaidEngine::new();
        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected a numeric diagnostic");
        };
        let start = text.find("1.").unwrap();
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(start, start + 2)));
        assert_eq!(diagnostic.message(), "invalid number: 1.");
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("xychart", text)
            .unwrap()
            .unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        let later_start = text.find("After").unwrap();
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "After"
                && symbol.selection == SourceSpan::new(later_start, later_start + 5)
        }));
    }

    #[test]
    fn xychart_same_line_support_preserves_upstream_rejection_boundaries() {
        for text in [
            "xychart x-axis [A, B] bar [1, 2] stray",
            "xychart x-axis [A, B bar [1, 2]",
            "xychart title \"unterminated x-axis [A, B] bar [1, 2]",
            "xychart title \"A\" \"B\" bar [1, 2]",
            // XY STR ends at every quote; it has no Langium backslash escape rule.
            "xychart title \"A\\\"B\" bar [1, 2]",
        ] {
            assert!(
                MermaidEngine::new()
                    .parse_diagram_sync(text, ParseOptions::strict())
                    .is_err(),
                "unexpectedly accepted {text:?}"
            );
        }
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
    fn xychart_band_text_rejects_reserved_tokens_at_jison_token_boundaries() {
        for (category, token) in [
            ("bar", "bar"),
            ("LINE", "LINE"),
            ("title", "title"),
            ("horizontal", "horizontal"),
            ("horizontalFoo", "horizontal"),
            ("verticalFoo", "vertical"),
            ("vertical42", "vertical"),
            ("HORIZONTAL_name", "HORIZONTAL"),
            ("xychart-beta", "xychart-beta"),
            ("1bar", "bar"),
            ("_line", "line"),
            ("foo bar", "bar"),
            ("A-horizontal", "horizontal"),
        ] {
            let source = format!("xychart\nx-axis [{category}, B]\nline [1, 2]\n");
            let error = MermaidEngine::new()
                .parse_diagram_sync(&source, ParseOptions::strict())
                .expect_err("inclusive band state must retain reserved lexer rules");
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("expected exact XY Chart parser diagnostic");
            };
            let start = "xychart\nx-axis [".len() + category.rfind(token).unwrap();
            assert_eq!(
                diagnostic.message(),
                format!("unexpected reserved token in text: {token}")
            );
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(start, start + token.len()))
            );
        }
    }

    #[test]
    fn xychart_band_text_keeps_alpha_numeric_and_quoted_controls() {
        let model = parse(concat!(
            "xychart\n",
            "x-axis [barTitle, bar_1, bar1, foobar, b ar, 1foo, accTitle, accDescr, 01]\n",
            "line [1, 2, 3, 4, 5, 6, 7, 8, 9]\n",
        ));
        assert_eq!(
            model["xAxis"]["categories"],
            json!([
                "barTitle", "bar_1", "bar1", "foobar", "bar", "1foo", "accTitle", "accDescr", "01"
            ])
        );
        for (quoted, expected) in [
            ("\"bar\"", "bar"),
            ("\"`bar`\"", "bar"),
            ("\"horizontalFoo\"", "horizontalFoo"),
        ] {
            let source = format!("xychart\nx-axis [{quoted}, B]\nline [1, 2]\n");
            assert_eq!(
                parse(&source)["xAxis"]["categories"],
                json!([expected, "B"])
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_reserved_band_recovery_preserves_repeated_raw_label_occurrences() {
        let source = concat!(
            "xychart\n",
            "x-axis [bar, B, bar]\n",
            "line [1 \" bar \", 1 \"bar\", 1 \"bar\"]\n",
        );
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("xychart", source)
            .unwrap()
            .unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        let reserved = source.find("bar").unwrap();
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(reserved, reserved + 3))
        }));
        let occurrences = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.detail.as_deref() == Some("xychart line data label"))
            .map(|symbol| (symbol.name.as_str(), symbol.selection))
            .collect::<Vec<_>>();
        let first = source.find("\" bar \"").unwrap() + 1;
        let second = source.find("\"bar\"").unwrap() + 1;
        let third = source.rfind("\"bar\"").unwrap() + 1;
        assert_eq!(
            occurrences,
            vec![
                (" bar ", SourceSpan::new(first, first + 5)),
                ("bar", SourceSpan::new(second, second + 3)),
                ("bar", SourceSpan::new(third, third + 3)),
            ]
        );
        let valid = source.replace("x-axis [bar, B, bar]", "x-axis [\"bar\", B, \"bar\"]");
        let model = parse(&valid);
        assert_eq!(model["xAxis"]["categories"], json!(["bar", "B", "bar"]));
        assert_eq!(model["plots"][0]["values"], json!([1.0, 1.0, 1.0]));
        assert_eq!(
            model["plots"][0]["pointLabels"],
            json!(["bar", "bar", "bar"])
        );
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
    fn xychart_line_points_accept_optional_labels() {
        let model = parse(
            r#"xychart
x-axis [A, B, C]
y-axis 0 --> 10
line [1 "low", 5, 9 "high"]
bar [2 "ignored", 4, 6]
"#,
        );

        assert_eq!(model["plots"][0]["type"], json!("line"));
        assert_eq!(model["plots"][0]["values"], json!([1.0, 5.0, 9.0]));
        assert_eq!(model["plots"][0]["pointLabels"], json!(["low", "", "high"]));
        assert!(model["plots"][1].get("pointLabels").is_none());
    }

    #[test]
    fn xychart_equal_linear_axis_bounds_map_every_point_by_data_index() {
        let model = parse(
            r#"xychart
x-axis 5 --> 5
line [10, 20, 30]
"#,
        );

        assert_eq!(
            model["plots"][0]["data"],
            json!([["5", 10.0], ["5", 20.0], ["5", 30.0]])
        );
    }

    #[test]
    fn xychart_single_point_on_linear_axis_remains_mapped_to_minimum() {
        let model = parse(
            r#"xychart
x-axis 5 --> 9
line [10]
"#,
        );

        assert_eq!(model["plots"][0]["data"], json!([["5", 10.0]]));
    }

    #[test]
    fn xychart_band_axis_truncates_points_but_preserves_labels_before_auto_y_range() {
        let model = parse(
            r#"xychart
x-axis [Q1, Q2]
line [10 "first", 50 "second", 999 "orphan", 800 "ignored"]
"#,
        );

        assert_eq!(model["yAxis"]["min"], json!(10.0));
        assert_eq!(model["yAxis"]["max"], json!(50.0));
        assert_eq!(model["plots"][0]["values"], json!([10.0, 50.0]));
        assert_eq!(
            model["plots"][0]["data"],
            json!([["Q1", 10.0], ["Q2", 50.0]])
        );
        assert_eq!(
            model["plots"][0]["pointLabels"],
            json!(["first", "second", "orphan", "ignored"])
        );
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
        let engine = MermaidEngine::new();
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

    #[test]
    fn xychart_rejects_trailing_decimal_point_like_pinned_jison() {
        let text = "xychart\nbar [1.]\n";
        let engine = MermaidEngine::new();
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = err else {
            panic!("expected xychart parse error");
        };
        let token_start = text.find("1.").expect("invalid number token");

        assert_eq!(diagnostic.message(), "invalid number: 1.");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(token_start, token_start + "1.".len()))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn xychart_entrypoints_and_combined_projection_construct_once() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "xychart horizontal\n",
            "title \"Revenue; Growth\"; accTitle: Revenue chart\n",
            "accDescr {\n",
            "  Quarterly %% literal\n",
            "  projection\n",
            "}\n",
            "x-axis \"Fiscal quarter\" [Q1, \"Q 2\"]\n",
            "y-axis Revenue 0 --> 100\n",
            "line \"Forecast line\" [23 \"Low\", 45 \"High\"]\n",
            "bar \"Actual bars\" [20, 40]\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("standalone XYChart JSON parse succeeds")
            .expect("standalone XYChart JSON parse returns a diagram");
        reset_xychart_syntax_construction_count();
        let standalone_json = parse_xychart(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("XYChart JSON projection succeeds");
        assert_eq!(xychart_syntax_construction_count(), 1);

        reset_xychart_syntax_construction_count();
        let typed = parse_xychart_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("XYChart typed projection succeeds");
        assert_eq!(xychart_syntax_construction_count(), 1);

        reset_xychart_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        let (combined_json, combined_editor) =
            crate::family::test_support::into_result(parse_xychart_json_and_editor_facts(
                text,
                &crate::diagrams::source_metadata_for_test(&parsed.meta),
                &crate::OperationControl::new(),
            ))
            .expect("XYChart combined projection succeeds");
        #[cfg(not(feature = "editor-facts"))]
        let combined_json = parse_xychart(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .unwrap();
        assert_eq!(xychart_syntax_construction_count(), 1);
        assert_eq!(
            combined_json, standalone_json,
            "XYChart combined projection drift"
        );
        #[cfg(feature = "editor-facts")]
        assert!(!combined_editor.symbols.is_empty());

        let typed_projection = merman_model::families::xy_chart::compat::project(
            &typed,
            &merman_model::presentation::compat::CompatJsonMetadata::from(&parsed.meta),
        )
        .expect("XYChart typed compatibility projection succeeds");
        assert_eq!(
            typed_projection, combined_json,
            "XYChart typed projection drift"
        );
    }

    #[test]
    fn xychart_typed_projection_matches_complete_compat_json() {
        let text = "xychart\nx-axis [Q1, Q2]\nline [1, 2]\n";
        let engine = MermaidEngine::new();
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("XYChart compatibility parse succeeds")
            .expect("XYChart compatibility parse returns a diagram");
        let typed = parse_xychart_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("XYChart typed parse succeeds");

        let projection = merman_model::families::xy_chart::compat::project(
            &typed,
            &merman_model::presentation::compat::CompatJsonMetadata::from(&parsed.meta),
        )
        .expect("XYChart compatibility projection succeeds");

        assert_eq!(projection, parsed.model);
        assert_eq!(projection["type"], json!("xychart"));
        assert!(projection["config"].is_object());
        assert_eq!(projection["title"], Value::Null);
        assert_eq!(projection["accTitle"], Value::Null);
        assert_eq!(projection["accDescr"], Value::Null);
        assert_eq!(projection["plots"][0]["title"], json!(""));
    }

    #[test]
    fn xychart_plot_titles_are_sanitized_and_projected_for_legends() {
        let text = "xychart\nx-axis [Q1, Q2]\nline \" avg \" [1, 2]\nbar \" p95 \" [2, 3]\n";
        let engine = MermaidEngine::new();
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("XYChart compatibility parse succeeds")
            .expect("XYChart compatibility parse returns a diagram");
        let typed = parse_xychart_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("XYChart typed parse succeeds");

        assert_eq!(typed.plots[0].title.as_deref(), Some("avg"));
        assert_eq!(typed.plots[1].title.as_deref(), Some("p95"));
        assert_eq!(parsed.model["plots"][0]["title"], json!("avg"));
        assert_eq!(parsed.model["plots"][1]["title"], json!("p95"));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_editor_projection_preserves_multiline_and_nested_payload_spans() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "xychart\n",
            "title \"Revenue plan\"\n",
            "accTitle: Revenue chart\n",
            "accDescr {\n",
            "  Quarterly %% literal\n",
            "  projection\n",
            "}\n",
            "x-axis \"Fiscal quarter\" [Q1, \"Q 2\"]\n",
            "y-axis Revenue 0 --> 100\n",
            "line \"Forecast line\" [23 \"Low\", 45 \"High\"]\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("XYChart parse succeeds")
            .expect("XYChart model");
        let facts = crate::family::test_support::editor_facts_from_frozen(
            parse_xychart_json_and_editor_facts,
            text,
            &parsed.meta,
        );

        let assert_payload = |name: &str, detail: &str, start: usize| {
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.name == name
                        && symbol.detail.as_deref() == Some(detail)
                        && symbol.selection.start == start
                })
                .unwrap_or_else(|| panic!("missing {detail} payload {name:?} at {start}"));
            assert_eq!(symbol.role, EditorSemanticRole::Payload);
            assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload
                    && expected.span == symbol.selection
            }));
        };

        for (name, detail, start) in [
            (
                "Revenue plan",
                "xychart title",
                text.find("Revenue plan").expect("title source"),
            ),
            (
                "Revenue chart",
                "xychart accessibility title",
                text.find("Revenue chart")
                    .expect("accessibility title source"),
            ),
            (
                "Quarterly %% literal\n  projection",
                "xychart accessibility description",
                text.find("Quarterly %% literal")
                    .expect("accessibility description source"),
            ),
            (
                "Fiscal quarter",
                "xychart x-axis",
                text.find("Fiscal quarter").expect("x-axis title source"),
            ),
            (
                "Revenue",
                "xychart y-axis",
                text.find("y-axis Revenue").expect("y-axis source") + "y-axis ".len(),
            ),
            (
                "Forecast line",
                "xychart line",
                text.find("Forecast line").expect("plot title source"),
            ),
            (
                "Low",
                "xychart line data label",
                text.find("Low").expect("low label source"),
            ),
            (
                "High",
                "xychart line data label",
                text.find("High").expect("high label source"),
            ),
        ] {
            assert_payload(name, detail, start);
        }

        for structured in ["Q1, \"Q 2\"", "0 --> 100", "23 \"Low\", 45 \"High\""] {
            let start = text.find(structured).expect("structured payload source");
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload
                    && expected.span == SourceSpan::new(start, start + structured.len())
            }));
            assert!(!facts.symbols.iter().any(|symbol| {
                symbol.name == structured && symbol.role == EditorSemanticRole::Payload
            }));
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_malformed_plot_recovers_prior_parser_facts() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "xychart\n",
            "title Revenue\n",
            "x-axis [Q1, Q2]\n",
            "bar [23, -4aa5]\n",
        );
        let invalid_start = text.find("-4aa5").expect("invalid number token");
        reset_xychart_syntax_construction_count();
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("xychart", text)
            .expect("XYChart editor recovery succeeds")
            .expect("XYChart editor facts are available");

        assert_eq!(xychart_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Revenue" && symbol.role == EditorSemanticRole::Payload
        }));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span
                    == Some(SourceSpan::new(
                        invalid_start,
                        invalid_start + "-4aa5".len(),
                    ))
        }));
    }

    #[test]
    fn xychart_recovery_keeps_confirmed_prefix_and_safe_later_statements() {
        let text = concat!(
            "xychart\n",
            "title Before\n",
            "bar [1, -4aa5]\n",
            "y-axis [0, 10]\n",
            "line \"After 后来\" [2 \"后来\"]\n",
        );
        let invalid_start = text.find("-4aa5").expect("invalid token");

        let strict = MermaidEngine::new()
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("strict XYChart parse must return its first error");
        let Error::DiagramParse { diagnostic, .. } = strict else {
            panic!("expected diagram parse error");
        };
        assert_eq!(diagnostic.message(), "invalid number: -4aa5");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(
                invalid_start,
                invalid_start + "-4aa5".len()
            ))
        );

        #[cfg(feature = "editor-facts")]
        {
            reset_xychart_syntax_construction_count();
            let facts = MermaidEngine::new()
                .parse_editor_semantic_facts_with_type_sync("xychart", text)
                .expect("XYChart editor recovery")
                .expect("XYChart recovered facts");
            assert_eq!(xychart_syntax_construction_count(), 1);
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "After 后来"
                    && symbol.detail.as_deref() == Some("xychart line")
                    && symbol.role == EditorSemanticRole::Payload
            }));
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_unterminated_eof_string_reports_insertion_point() {
        let text = "xychart\ntitle \"未完成 🤓";
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("xychart", text)
            .expect("XYChart editor recovery")
            .expect("XYChart recovered facts");

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(text.len(), text.len()))
        }));
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn xychart_missing_bracket_recovers_at_the_next_statement_boundary() {
        let text = "xychart\nbar [1, 2\nline [3]\n";
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("xychart", text)
            .expect("XYChart editor recovery")
            .expect("XYChart recovered facts");

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        let later_value_start = text.rfind('3').expect("later line value");
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(later_value_start, later_value_start + 1)
        }));
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    #[test]
    fn canonical_empty_capture_keeps_an_available_empty_journal() {
        reset_xychart_syntax_construction_count();
        let parsed = parse_xychart_semantic_model_controlled(
            "",
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &crate::OperationControl::new(),
        )
        .unwrap();
        assert!(parsed.model.is_ok());
        assert_eq!(xychart_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        {
            let facts = parsed
                .editor_facts
                .expect("empty input still provides requested facts");
            assert!(facts.symbols.is_empty());
            assert!(facts.diagnostics.is_empty());
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Complete
            );
        }
    }

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "xychart".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn semantic_orientation_preserves_upstream_literal_setter_behavior() {
        let mut metadata = metadata();
        metadata.semantic_config = MermaidConfig::from_value(serde_json::json!({
            "xyChart": {"chartOrientation": "horizontal"}
        }));
        for (orientation, expected) in [
            ("horizontal", "horizontal"),
            ("HORIZONTAL", "vertical"),
            ("Horizontal", "vertical"),
            ("vertical", "vertical"),
            ("VERTICAL", "vertical"),
        ] {
            let source = format!("xychart {orientation}\nx-axis [A, B]\nbar [1, 2]");
            let model = parse_xychart_semantic_model_controlled(
                &source,
                &metadata,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(
                model.orientation, expected,
                "orientation token {orientation}"
            );
        }
    }

    #[test]
    fn semantic_same_line_header_axis_and_plot_share_one_construction() {
        for header in ["xychart", "xychart-beta"] {
            reset_xychart_syntax_construction_count();
            let source = format!("{header} x-axis [A, B] bar [1, 2]");
            let model = parse_xychart_semantic_model_controlled(
                &source,
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .model
            .unwrap();
            assert_eq!(xychart_syntax_construction_count(), 1);
            assert_eq!(model.orientation, "vertical");
            assert_eq!(
                model.x_axis,
                XyChartAxisModel::Band {
                    title: String::new(),
                    categories: vec!["A".to_string(), "B".to_string()],
                }
            );
            assert_eq!(model.plots.len(), 1);
            assert_eq!(model.plots[0].plot_type, XyChartPlotType::Bar);
            assert_eq!(model.plots[0].values, vec![1.0, 2.0]);
            assert_eq!(
                model.plots[0].data,
                vec![("A".to_string(), Some(1.0)), ("B".to_string(), Some(2.0)),]
            );
        }
    }

    #[test]
    fn semantic_same_line_lexing_can_cancel_inside_long_strings_and_comments() {
        for source in [
            format!("xychart title \"{}\" bar [1]", "a".repeat(32768)),
            format!("xychart x-axis [A] %%{}\nbar [1]", "a".repeat(32768)),
            format!("xychart x-axis [{}bar, B] line [1, 2]", "A ".repeat(32768)),
        ] {
            let control = crate::OperationControl::new();
            control.cancel_after_checkpoints(5);
            assert!(matches!(
                parse_xychart_semantic_model_controlled(
                    &source,
                    &metadata(),
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Omit,
                    &control,
                ),
                Err(crate::OperationCancelled { .. })
            ));
        }
    }

    #[test]
    fn canonical_terminal_display_policy_reads_all_admitted_visibility_flags() {
        let policy = XyChartDisplayPolicy::from_config(&MermaidConfig::from_value(
            serde_json::json!({"xyChart": {
                "showTitle": false,
                "showDataLabel": true,
                "showDataLabelOutsideBar": true,
                "xAxis": {
                    "showLabel": false, "showTitle": false,
                    "showTick": false, "showAxisLine": false
                },
                "yAxis": {
                    "showLabel": false, "showTitle": false,
                    "showTick": false, "showAxisLine": false
                }
            }}),
        ));
        assert_eq!(
            policy,
            XyChartDisplayPolicy {
                show_title: false,
                show_data_label: true,
                show_data_label_outside_bar: true,
                x_axis: XyChartAxisDisplayPolicy {
                    show_label: false,
                    show_title: false,
                    show_tick: false,
                    show_axis_line: false,
                },
                y_axis: XyChartAxisDisplayPolicy {
                    show_label: false,
                    show_title: false,
                    show_tick: false,
                    show_axis_line: false,
                },
            }
        );
        assert_eq!(
            XyChartDisplayPolicy::from_config(&MermaidConfig::empty_object()),
            XyChartDisplayPolicy::default()
        );
    }

    #[test]
    fn semantic_axes_and_series_construct_once_without_display_or_editor_work() {
        reset_xychart_syntax_construction_count();
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let model = parse_xychart_semantic_model_controlled(
            "xychart horizontal\ntitle Revenue\nx-axis Quarter [Q1, Q2]\ny-axis Revenue 0 --> 100\nline Forecast [23 \"Low\", 45 \"High\"]\n",
            &metadata(), #[cfg(feature = "editor-facts")]
EditorFactsDemand::Omit, &crate::OperationControl::new(),
        ).unwrap().model.unwrap();
        assert_eq!(model.orientation, "horizontal");
        assert_eq!(model.title.as_deref(), Some("Revenue"));
        assert_eq!(
            model.x_axis,
            XyChartAxisModel::Band {
                title: "Quarter".to_string(),
                categories: vec!["Q1".to_string(), "Q2".to_string()]
            }
        );
        assert_eq!(
            model.y_axis,
            XyChartAxisModel::Linear {
                title: "Revenue".to_string(),
                min: Some(0.0),
                max: Some(100.0)
            }
        );
        assert_eq!(model.plots[0].plot_type, XyChartPlotType::Line);
        assert_eq!(model.plots[0].values, vec![23.0, 45.0]);
        assert_eq!(
            model.plots[0].data,
            vec![
                ("Q1".to_string(), Some(23.0)),
                ("Q2".to_string(), Some(45.0))
            ]
        );
        assert_eq!(model.plots[0].point_labels, vec!["Low", "High"]);
        assert_eq!(xychart_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        assert!(
            serde_json::to_value(model)
                .unwrap()
                .get("display")
                .is_none()
        );
    }

    #[test]
    fn semantic_config_sanitizes_text_without_materializing_display_policy() {
        let mut metadata = metadata();
        metadata.semantic_config = MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "strict", "xyChart": {"chartOrientation": "horizontal", "showTitle": false},
            "themeVariables": {"primaryColor": "not-a-color"}
        }));
        let model = parse_xychart_semantic_model_controlled(
            "xychart\ntitle \"<script>bad()</script>Revenue\"\nbar [1, 2]",
            &metadata,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        assert_eq!(model.orientation, "horizontal");
        assert_eq!(model.title.as_deref(), Some("Revenue"));
        assert_eq!(model.plots[0].values, vec![1.0, 2.0]);
    }

    #[test]
    fn semantic_invalid_number_retains_exact_error_and_omits_editor_recovery() {
        let input = "xychart\nbar [1.]\n";
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        let error = parse_xychart_semantic_model_controlled(
            input,
            &metadata(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = &error else {
            panic!("expected exact parser diagnostic")
        };
        let offset = input.find("1.").unwrap();
        assert_eq!(diagnostic.message(), "invalid number: 1.");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(offset, offset + 2)));
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        #[cfg(feature = "presentation")]
        {
            let full = SourcePresentationMetadata {
                diagram_type: "xychart".to_string(),
                config: MermaidConfig::empty_object(),
                effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
                title: None,
            };
            let full_error = parse_xychart_model_for_render_controlled(
                input,
                &full,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .unwrap_err();
            assert_eq!(error.to_string(), full_error.to_string());
        }
    }

    #[test]
    fn semantic_cancellation_stops_before_construction() {
        reset_xychart_syntax_construction_count();
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_xychart_semantic_model_controlled(
                "xychart\nbar [1]",
                &metadata(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &control
            )
            .is_err()
        );
        assert_eq!(xychart_syntax_construction_count(), 0);
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn typed_and_json_paths_skip_editor_projection() {
        let full = SourcePresentationMetadata {
            diagram_type: "xychart".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_xychart_syntax_construction_count();
        parse_xychart_model_for_render_controlled(
            "xychart\nbar [1, 2]",
            &full,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(xychart_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        reset_xychart_syntax_construction_count();
        parse_xychart("xychart\nbar [1, 2]", &full).unwrap();
        assert_eq!(xychart_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }
}
