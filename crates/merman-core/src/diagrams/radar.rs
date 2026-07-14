use crate::common_db::LangiumCommonDbFields;
use crate::diagrams::langium_common::{
    LangiumCommonFacts, parse_langium_common, parse_langium_string,
    push_langium_common_editor_fact, push_langium_common_recovery, strip_langium_inline_comment,
};
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
    editor::{editor_recovery_fallback_span, ensure_editor_recovery_from_error},
};
use serde_json::{Map, Value, json};

#[derive(Debug, Clone)]
struct AxisAst {
    name: SpannedText,
    label: Option<SpannedText>,
}

#[derive(Debug, Clone)]
struct EntryAst {
    axis: Option<SpannedText>,
    value: SpannedRadarValue,
}

#[derive(Debug, Clone)]
struct CurveAst {
    name: SpannedText,
    label: Option<SpannedText>,
    entries: Vec<EntryAst>,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
enum OptionValueAst {
    Bool(bool),
    Number(Value),
    Graticule(String),
}

#[derive(Debug, Clone)]
struct OptionAst {
    name: String,
    value: OptionValueAst,
    value_span: SourceSpan,
}

#[derive(Debug, Clone)]
struct SpannedRadarValue {
    value: Value,
    span: SourceSpan,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarRenderAxis {
    pub name: String,
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarRenderCurve {
    pub name: String,
    pub label: String,
    #[serde(default)]
    pub entries: Vec<Value>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RadarRenderOptions {
    #[serde(rename = "showLegend")]
    pub show_legend: bool,
    pub ticks: Value,
    pub max: Option<Value>,
    pub min: Value,
    pub graticule: String,
}

impl Default for RadarRenderOptions {
    fn default() -> Self {
        Self {
            show_legend: true,
            ticks: json!(5),
            max: None,
            min: json!(0),
            graticule: "circle".to_string(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct RadarDiagramRenderModel {
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub axes: Vec<RadarRenderAxis>,
    #[serde(default)]
    pub curves: Vec<RadarRenderCurve>,
    #[serde(default)]
    pub options: RadarRenderOptions,
}

impl RadarDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone)]
struct RadarDb {
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    axes: Vec<RadarRenderAxis>,
    curves: Vec<RadarRenderCurve>,
    options: RadarRenderOptions,
}

struct RadarSemanticSource {
    db: Option<RadarDb>,
    editor_facts: EditorSemanticFacts,
}

impl RadarDb {
    fn new() -> Self {
        Self {
            title: None,
            acc_title: None,
            acc_descr: None,
            axes: Vec::new(),
            curves: Vec::new(),
            options: RadarRenderOptions::default(),
        }
    }

    fn set_axes(&mut self, axes: Vec<AxisAst>) {
        self.axes = axes
            .into_iter()
            .map(|axis| RadarRenderAxis {
                label: axis
                    .label
                    .map(|label| label.text)
                    .unwrap_or_else(|| axis.name.text.clone()),
                name: axis.name.text,
            })
            .collect();
    }

    fn set_curves(&mut self, curves: Vec<CurveAst>) -> Result<()> {
        let axes = self.axes.clone();
        self.curves = curves
            .into_iter()
            .map(|curve| {
                let curve_span = curve.span;
                let label = curve
                    .label
                    .as_ref()
                    .map(|label| label.text.clone())
                    .unwrap_or_else(|| curve.name.text.clone());
                let entries = compute_curve_entries(&axes, &curve.entries)
                    .map_err(|error| error.with_exact_span_if_missing(curve_span))?;
                Ok(RadarRenderCurve {
                    name: curve.name.text,
                    label,
                    entries,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(())
    }

    fn set_options(&mut self, options: Vec<OptionAst>) {
        let mut last: std::collections::HashMap<String, OptionValueAst> =
            std::collections::HashMap::new();
        for opt in options {
            last.insert(opt.name, opt.value);
        }

        if let Some(OptionValueAst::Bool(v)) = last.get("showLegend") {
            self.options.show_legend = *v;
        }
        if let Some(OptionValueAst::Number(v)) = last.get("ticks") {
            self.options.ticks = v.clone();
        }
        if let Some(OptionValueAst::Number(v)) = last.get("max") {
            self.options.max = Some(v.clone());
        }
        if let Some(OptionValueAst::Number(v)) = last.get("min") {
            self.options.min = v.clone();
        }
        if let Some(OptionValueAst::Graticule(v)) = last.get("graticule") {
            self.options.graticule = v.clone();
        }
    }

    #[inline]
    fn semantic_value(&self, meta: &ParseMetadata) -> Value {
        let mut out = Map::with_capacity(8);
        out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
        out.insert("title".to_string(), json!(self.title));
        out.insert("accTitle".to_string(), json!(self.acc_title));
        out.insert("accDescr".to_string(), json!(self.acc_descr));
        out.insert(
            "axes".to_string(),
            Value::Array(
                self.axes
                    .iter()
                    .map(|a| json!({"name": a.name, "label": a.label}))
                    .collect(),
            ),
        );
        out.insert(
            "curves".to_string(),
            Value::Array(
                self.curves
                    .iter()
                    .map(|c| json!({"name": c.name, "label": c.label, "entries": c.entries}))
                    .collect(),
            ),
        );
        out.insert(
            "options".to_string(),
            json!({
                "showLegend": self.options.show_legend,
                "ticks": self.options.ticks,
                "max": self.options.max,
                "min": self.options.min,
                "graticule": self.options.graticule,
            }),
        );
        out.insert(
            "config".to_string(),
            crate::config::clone_value_nonrecursive(meta.effective_config.as_value()),
        );
        Value::Object(out)
    }

    #[inline]
    fn into_render_model(self) -> RadarDiagramRenderModel {
        RadarDiagramRenderModel {
            title: self.title,
            acc_title: self.acc_title,
            acc_descr: self.acc_descr,
            axes: self.axes,
            curves: self.curves,
            options: self.options,
        }
    }
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    span: SourceSpan,
}

fn push_radar_entity(
    facts: &mut EditorSemanticFacts,
    text: SpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        text.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::new(
        text.text,
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

fn push_radar_payload(
    facts: &mut EditorSemanticFacts,
    text: SpannedText,
    detail: &str,
    kind: EditorSemanticKind,
) {
    if text.text.is_empty() {
        return;
    }
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        text.span,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.text,
        Some(detail.to_string()),
        kind,
        text.span,
        text.span,
    ));
}

fn value_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}

enum RadarStatementEvent {
    Axes(Vec<AxisAst>),
    Curves(Vec<CurveAst>),
    Options(Vec<OptionAst>),
}

fn parse_radar_statement(
    stmt: &str,
    stmt_start: usize,
) -> std::result::Result<RadarStatementEvent, String> {
    let (trimmed, trimmed_start) = trim_start_with_source_offset(stmt, stmt_start);
    if let Some(rest) = trimmed.strip_prefix("axis") {
        let (rest, rest_start) = trim_start_with_source_offset(rest, trimmed_start + "axis".len());
        if rest.is_empty() {
            return Err("axis statement must include at least one axis".to_string());
        }
        return parse_axes_list(rest, rest_start).map(RadarStatementEvent::Axes);
    }

    if trimmed.starts_with("curve") {
        return parse_curves_stmt(trimmed, trimmed_start).map(RadarStatementEvent::Curves);
    }

    if let Some(options) = parse_option_list_stmt(trimmed, trimmed_start)? {
        return Ok(RadarStatementEvent::Options(options));
    }

    if let Some(option) = parse_option_stmt(trimmed, trimmed_start)? {
        return Ok(RadarStatementEvent::Options(vec![option]));
    }

    Err(format!("unexpected radar statement: {}", stmt.trim()))
}

fn push_radar_statement_facts(facts: &mut EditorSemanticFacts, event: &RadarStatementEvent) {
    match event {
        RadarStatementEvent::Axes(axes) => {
            for axis in axes {
                push_radar_entity(
                    facts,
                    axis.name.clone(),
                    "radar axis",
                    EditorSemanticKind::Variable,
                );
                if let Some(label) = &axis.label {
                    push_radar_payload(
                        facts,
                        label.clone(),
                        "radar axis label",
                        EditorSemanticKind::String,
                    );
                }
            }
        }
        RadarStatementEvent::Curves(curves) => {
            for curve in curves {
                push_radar_entity(
                    facts,
                    curve.name.clone(),
                    "radar curve",
                    EditorSemanticKind::Variable,
                );
                if let Some(label) = &curve.label {
                    push_radar_payload(
                        facts,
                        label.clone(),
                        "radar curve label",
                        EditorSemanticKind::String,
                    );
                }

                for entry in &curve.entries {
                    if let Some(axis) = &entry.axis {
                        push_radar_entity(
                            facts,
                            axis.clone(),
                            "radar curve axis reference",
                            EditorSemanticKind::Variable,
                        );
                    }
                    push_radar_payload(
                        facts,
                        SpannedText {
                            text: value_text(&entry.value.value),
                            span: entry.value.span,
                        },
                        "radar curve entry",
                        EditorSemanticKind::String,
                    );
                }
            }
        }
        RadarStatementEvent::Options(options) => {
            for option in options {
                let token = match &option.value {
                    OptionValueAst::Bool(value) => value.to_string(),
                    OptionValueAst::Number(value) => value_text(value),
                    OptionValueAst::Graticule(value) => value.clone(),
                };
                push_radar_payload(
                    facts,
                    SpannedText {
                        text: token,
                        span: option.value_span,
                    },
                    "radar option",
                    EditorSemanticKind::String,
                );
            }
        }
    }
}

fn apply_radar_statement(
    event: RadarStatementEvent,
    axes: &mut Vec<AxisAst>,
    curves: &mut Vec<CurveAst>,
    options: &mut Vec<OptionAst>,
) {
    match event {
        RadarStatementEvent::Axes(parsed) => axes.extend(parsed),
        RadarStatementEvent::Curves(parsed) => curves.extend(parsed),
        RadarStatementEvent::Options(parsed) => options.extend(parsed),
    }
}

fn scan_radar_editor_facts(code: &str) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let Ok(Some(mut offset)) = radar_body_start(code) else {
        return facts;
    };

    while offset < code.len() {
        if let Some(parsed) = parse_langium_common(code, offset) {
            push_langium_common_editor_fact(&mut facts, &parsed.fact, "radar");
            if let Some(diagnostic) = &parsed.diagnostic {
                push_langium_common_recovery(&mut facts, diagnostic);
            }
            offset += parsed.consumed;
            continue;
        }

        let (stmt, stmt_start, next_offset) = radar_statement_at(code, offset);
        offset = next_offset;
        if stmt.is_empty() {
            continue;
        }
        match parse_radar_statement(&stmt, stmt_start) {
            Ok(event) => push_radar_statement_facts(&mut facts, &event),
            Err(message) => {
                let invalid = stmt.trim_end();
                facts.mark_recovered_from_parse_error(
                    message,
                    Some(SourceSpan::new(stmt_start, stmt_start + invalid.len())),
                );
            }
        }
    }

    facts
}

pub fn parse_radar_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    match parse_radar_semantic_source(code, meta) {
        Ok(source) => source.editor_facts,
        Err(error) => ensure_editor_recovery_from_error(
            scan_radar_editor_facts(code),
            &error,
            editor_recovery_fallback_span(code),
        ),
    }
}

fn compute_curve_entries(axes: &[RadarRenderAxis], entries: &[EntryAst]) -> Result<Vec<Value>> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    if entries[0].axis.is_none() {
        return Ok(entries
            .iter()
            .map(|entry| entry.value.value.clone())
            .collect());
    }

    if axes.is_empty() {
        return Err(Error::diagram_parse_fallback(
            "radar".to_string(),
            "Axes must be populated before curves for reference entries".to_string(),
        ));
    }

    axes.iter()
        .map(|axis| {
            let found = entries.iter().find(|entry| {
                entry
                    .axis
                    .as_ref()
                    .is_some_and(|entry_axis| entry_axis.text == axis.name)
            });
            let Some(found) = found else {
                return Err(Error::diagram_parse_fallback(
                    "radar".to_string(),
                    format!("Missing entry for axis {}", axis.label),
                ));
            };
            Ok(found.value.value.clone())
        })
        .collect()
}

pub fn parse_radar(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let Some(db) = parse_radar_semantic_source(code, meta)?.db else {
        return Ok(json!({}));
    };
    Ok(db.semantic_value(meta))
}

pub(crate) fn parse_radar_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let RadarSemanticSource { db, editor_facts } = parse_radar_semantic_source(code, meta)?;
    let model = db.map_or_else(|| json!({}), |db| db.semantic_value(meta));
    Ok((model, editor_facts))
}

pub fn parse_radar_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<RadarDiagramRenderModel> {
    parse_radar_semantic_source(code, meta).map(|source| {
        source
            .db
            .map(RadarDb::into_render_model)
            .unwrap_or_default()
    })
}

#[inline]
fn parse_radar_semantic_source(code: &str, meta: &ParseMetadata) -> Result<RadarSemanticSource> {
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("radar");

    let Some(mut offset) = radar_body_start(code)? else {
        return Ok(RadarSemanticSource {
            db: None,
            editor_facts: EditorSemanticFacts::new(),
        });
    };

    let mut common = LangiumCommonFacts::default();
    let mut editor_facts = EditorSemanticFacts::new();
    let mut axes: Vec<AxisAst> = Vec::new();
    let mut curves: Vec<CurveAst> = Vec::new();
    let mut options: Vec<OptionAst> = Vec::new();

    while offset < code.len() {
        if let Some(parsed) = parse_langium_common(code, offset) {
            if let Some(diagnostic) = parsed.diagnostic {
                return Err(Error::diagram_parse_insertion_point(
                    meta.diagram_type.clone(),
                    diagnostic.message,
                    diagnostic.span.start,
                ));
            }
            push_langium_common_editor_fact(&mut editor_facts, &parsed.fact, "radar");
            common.push(parsed.fact);
            offset += parsed.consumed;
            continue;
        }

        let (statement, statement_start, next_offset) = radar_statement_at(code, offset);
        offset = next_offset;
        if statement.is_empty() {
            continue;
        }

        let event = parse_radar_statement(&statement, statement_start)
            .map_err(|message| Error::diagram_parse_fallback("radar".to_string(), message))?;
        push_radar_statement_facts(&mut editor_facts, &event);
        apply_radar_statement(event, &mut axes, &mut curves, &mut options);
    }

    let common = LangiumCommonDbFields::from_facts(&common);
    let mut db = RadarDb::new();
    db.title = common.title;
    db.acc_title = common.acc_title;
    db.acc_descr = common.acc_descr;
    db.set_axes(axes);
    db.set_curves(curves)?;
    db.set_options(options);

    Ok(RadarSemanticSource {
        db: Some(db),
        editor_facts,
    })
}

fn strip_inline_comment(line: &str) -> &str {
    strip_langium_inline_comment(line)
}

fn radar_body_start(code: &str) -> Result<Option<usize>> {
    let mut offset = 0usize;
    while offset < code.len() {
        let (line, next_offset) = physical_line(code, offset);
        let visible = strip_inline_comment(line);
        let trimmed = visible.trim_start();
        if trimmed.trim().is_empty() {
            offset = next_offset;
            continue;
        }
        let Some(after_keyword) = trimmed.strip_prefix("radar-beta") else {
            return Err(Error::diagram_parse_fallback(
                "radar".to_string(),
                "expected radar-beta".to_string(),
            ));
        };
        if after_keyword
            .chars()
            .next()
            .is_some_and(|ch| !ch.is_whitespace() && ch != ':' && !after_keyword.starts_with("%%"))
        {
            return Err(Error::diagram_parse_fallback(
                "radar".to_string(),
                "expected radar-beta".to_string(),
            ));
        }

        let leading = visible.len() - trimmed.len();
        let mut body_start = offset + leading + "radar-beta".len();
        let whitespace = code[body_start..]
            .chars()
            .take_while(|ch| matches!(ch, ' ' | '\t'))
            .map(char::len_utf8)
            .sum::<usize>();
        if code.as_bytes().get(body_start + whitespace) == Some(&b':') {
            body_start += whitespace + 1;
        }
        return Ok(Some(body_start));
    }
    Ok(None)
}

fn radar_statement_at(code: &str, offset: usize) -> (String, usize, usize) {
    let (line, mut next_offset) = physical_line(code, offset);
    let visible = strip_inline_comment(line);
    let trimmed = visible.trim();
    let stmt_start = offset + visible.find(trimmed).unwrap_or(0);
    if trimmed.is_empty() {
        return (String::new(), stmt_start, next_offset);
    }
    let mut stmt = mask_radar_inline_comments(&code[stmt_start..next_offset]);

    if stmt.trim_start().starts_with("curve")
        && stmt.contains('{')
        && !braces_balanced_outside_quotes(&stmt)
    {
        while next_offset < code.len() {
            let (_, after_next) = physical_line(code, next_offset);
            next_offset = after_next;
            stmt = mask_radar_inline_comments(&code[stmt_start..next_offset]);
            if braces_balanced_outside_quotes(&stmt) {
                break;
            }
        }
    }

    (stmt, stmt_start, next_offset)
}

fn mask_radar_inline_comments(source: &str) -> String {
    let mut masked = source.as_bytes().to_vec();
    let mut line_start = 0usize;
    while line_start < source.len() {
        let line_end = source[line_start..]
            .find('\n')
            .map_or(source.len(), |newline| line_start + newline);
        let content_end = if line_end > line_start && source.as_bytes()[line_end - 1] == b'\r' {
            line_end - 1
        } else {
            line_end
        };
        let line = &source[line_start..content_end];
        let visible = strip_langium_inline_comment(line);
        if visible.len() < line.len() {
            masked[line_start + visible.len()..content_end].fill(b' ');
        }
        line_start = (line_end + usize::from(line_end < source.len())).min(source.len());
    }
    String::from_utf8(masked).expect("comment masking preserves UTF-8 before the comment marker")
}

fn trim_start_with_source_offset(input: &str, input_start: usize) -> (&str, usize) {
    let trimmed = input.trim_start();
    (trimmed, input_start + input.len() - trimmed.len())
}

fn physical_line(source: &str, offset: usize) -> (&str, usize) {
    let rest = &source[offset..];
    if let Some(newline) = rest.find('\n') {
        let line = rest[..newline]
            .strip_suffix('\r')
            .unwrap_or(&rest[..newline]);
        (line, offset + newline + 1)
    } else {
        (rest, source.len())
    }
}

fn braces_balanced_outside_quotes(s: &str) -> bool {
    let mut in_quote: Option<char> = None;
    let mut escaped = false;
    let mut depth = 0i64;
    for ch in s.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(q) = in_quote {
            if ch == q {
                in_quote = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
            continue;
        }
        if ch == '{' {
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
        }
    }
    depth == 0
}

fn parse_axes_list(input: &str, input_start: usize) -> std::result::Result<Vec<AxisAst>, String> {
    let mut p = TokenParser::new(input, input_start);
    let mut out = Vec::new();
    loop {
        p.skip_ws();
        if p.eof() {
            break;
        }
        let name = p.parse_id().ok_or_else(|| "expected axis id".to_string())?;
        p.skip_ws();
        let label = if p.try_consume('[') {
            p.skip_ws();
            let s = p
                .parse_quoted_string()
                .ok_or_else(|| "expected quoted axis label".to_string())?;
            p.skip_ws();
            if !p.try_consume(']') {
                return Err("expected ']'".to_string());
            }
            Some(s)
        } else {
            None
        };
        out.push(AxisAst { name, label });
        p.skip_ws();
        if p.try_consume(',') {
            // Upstream Mermaid rejects a trailing comma at the end of an `axis` list.
            p.skip_ws();
            if p.eof() {
                return Err("unexpected trailing ',' in axis list".to_string());
            }
            continue;
        }
        if p.eof() {
            break;
        }
        return Err("expected ',' or end of axis list".to_string());
    }
    Ok(out)
}

fn parse_curves_stmt(
    input: &str,
    input_start: usize,
) -> std::result::Result<Vec<CurveAst>, String> {
    let (input, input_start) = trim_start_with_source_offset(input, input_start);
    let rest = input
        .strip_prefix("curve")
        .ok_or_else(|| "expected curve".to_string())?;
    let (rest, rest_start) = trim_start_with_source_offset(rest, input_start + "curve".len());
    if rest.trim().is_empty() {
        return Err("expected curve id".to_string());
    }

    let chunks = split_top_level(rest, ',');
    let mut curves = Vec::new();
    for (chunk_offset, chunk) in chunks {
        let (chunk, chunk_start) = trim_start_with_source_offset(chunk, rest_start + chunk_offset);
        let chunk = chunk.trim_end();
        if chunk.is_empty() {
            return Err("expected curve after ','".to_string());
        }
        curves.push(parse_curve(chunk, chunk_start)?);
    }
    Ok(curves)
}

fn parse_curve(input: &str, input_start: usize) -> std::result::Result<CurveAst, String> {
    let mut p = TokenParser::new(input, input_start);
    p.skip_ws();
    let name = p
        .parse_id()
        .ok_or_else(|| "expected curve id".to_string())?;
    p.skip_ws();
    let label = if p.try_consume('[') {
        p.skip_ws();
        let s = p
            .parse_quoted_string()
            .ok_or_else(|| "expected quoted curve label".to_string())?;
        p.skip_ws();
        if !p.try_consume(']') {
            return Err("expected ']'".to_string());
        }
        p.skip_ws();
        Some(s)
    } else {
        None
    };

    if !p.try_consume('{') {
        return Err("expected '{'".to_string());
    }

    let (entries_str, entries_start) = p.take_until_matching_brace()?;
    let entries = parse_entries(entries_str, entries_start)?;

    p.skip_ws();
    if !p.eof() {
        return Err("unexpected trailing tokens after curve".to_string());
    }

    let has_detailed = entries.iter().any(|e| e.axis.is_some());
    let has_numeric = entries.iter().any(|e| e.axis.is_none());
    if has_detailed && has_numeric {
        return Err("mixed detailed and numeric entries are not supported".to_string());
    }

    Ok(CurveAst {
        name,
        label,
        entries,
        span: SourceSpan::new(input_start, input_start + input.len()),
    })
}

fn parse_entries(input: &str, input_start: usize) -> std::result::Result<Vec<EntryAst>, String> {
    let items = split_top_level(input, ',');
    let mut out = Vec::new();
    for (item_offset, item) in items {
        let (item, item_start) = trim_start_with_source_offset(item, input_start + item_offset);
        let item = item.trim_end();
        if item.is_empty() {
            return Err("expected curve entry".to_string());
        }

        // Try detailed first: <ID> ':'? <NUMBER>
        let mut p = TokenParser::new(item, item_start);
        p.skip_ws();
        let start_pos = p.pos;
        if let Some(axis) = p.parse_id() {
            p.skip_ws();
            p.try_consume(':');
            p.skip_ws();
            if let Some(num) = p.parse_number_value() {
                p.skip_ws();
                if p.eof() {
                    out.push(EntryAst {
                        axis: Some(axis),
                        value: num,
                    });
                    continue;
                }
            }
        }
        p.pos = start_pos;

        // Otherwise numeric: <NUMBER>
        p.skip_ws();
        let num = p
            .parse_number_value()
            .ok_or_else(|| "expected entry number".to_string())?;
        p.skip_ws();
        if !p.eof() {
            return Err("unexpected trailing tokens in entry".to_string());
        }
        out.push(EntryAst {
            axis: None,
            value: num,
        });
    }
    Ok(out)
}

fn parse_option_stmt(
    input: &str,
    input_start: usize,
) -> std::result::Result<Option<OptionAst>, String> {
    let mut p = TokenParser::new(input, input_start);
    p.skip_ws();
    let parsed_name = match p.parse_id() {
        Some(name) => name,
        None => return Ok(None),
    };
    let name = match parsed_name.text.as_str() {
        "showLegend" => "showLegend",
        "ticks" => "ticks",
        "max" => "max",
        "min" => "min",
        "graticule" => "graticule",
        _ => return Ok(None),
    }
    .to_string();
    p.skip_ws();

    if name == "showLegend" {
        let (value, value_span) = p
            .parse_bool()
            .ok_or_else(|| "expected boolean".to_string())?;
        p.skip_ws();
        if !p.eof() {
            return Err("unexpected trailing tokens after option".to_string());
        }
        return Ok(Some(OptionAst {
            name,
            value: OptionValueAst::Bool(value),
            value_span,
        }));
    }

    if name == "graticule" {
        let value = p
            .parse_id()
            .ok_or_else(|| "expected graticule".to_string())?;
        if value.text != "circle" && value.text != "polygon" {
            return Err("expected graticule".to_string());
        }
        p.skip_ws();
        if !p.eof() {
            return Err("unexpected trailing tokens after option".to_string());
        }
        return Ok(Some(OptionAst {
            name,
            value: OptionValueAst::Graticule(value.text),
            value_span: value.span,
        }));
    }

    let value = p
        .parse_number_value()
        .ok_or_else(|| "expected number".to_string())?;
    p.skip_ws();
    if !p.eof() {
        return Err("unexpected trailing tokens after option".to_string());
    }
    Ok(Some(OptionAst {
        name,
        value: OptionValueAst::Number(value.value),
        value_span: value.span,
    }))
}

fn parse_option_list_stmt(
    input: &str,
    input_start: usize,
) -> std::result::Result<Option<Vec<OptionAst>>, String> {
    if !input.contains(',') {
        return Ok(None);
    }
    let chunks = split_top_level(input, ',');
    let mut out = Vec::new();
    for (chunk_offset, chunk) in chunks {
        let (chunk, chunk_start) = trim_start_with_source_offset(chunk, input_start + chunk_offset);
        let chunk = chunk.trim_end();
        if chunk.is_empty() {
            return Err("expected option after ','".to_string());
        }
        let Some(opt) = parse_option_stmt(chunk, chunk_start)? else {
            return Ok(None);
        };
        out.push(opt);
    }
    if out.is_empty() {
        return Ok(None);
    }
    Ok(Some(out))
}

fn split_top_level(input: &str, delim: char) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut chunk_start = 0usize;
    let mut in_quote: Option<char> = None;
    let mut escaped = false;
    let mut brace_depth = 0i64;
    let mut bracket_depth = 0i64;
    for (offset, ch) in input.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(q) = in_quote {
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == q {
                in_quote = None;
            }
            continue;
        }
        if ch == '"' || ch == '\'' {
            in_quote = Some(ch);
            continue;
        }
        match ch {
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            _ => {}
        }
        if ch == delim && brace_depth == 0 && bracket_depth == 0 {
            out.push((chunk_start, &input[chunk_start..offset]));
            chunk_start = offset + ch.len_utf8();
            continue;
        }
    }
    out.push((chunk_start, &input[chunk_start..]));
    out
}

struct TokenParser<'a> {
    input: &'a str,
    pos: usize,
    base_offset: usize,
}

impl<'a> TokenParser<'a> {
    fn new(input: &'a str, base_offset: usize) -> Self {
        Self {
            input,
            pos: 0,
            base_offset,
        }
    }

    fn eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek_char() {
            if ch.is_whitespace() {
                self.pos += ch.len_utf8();
                continue;
            }
            break;
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.pos..].chars().next()
    }

    fn try_consume(&mut self, ch: char) -> bool {
        if self.input[self.pos..].starts_with(ch) {
            self.pos += ch.len_utf8();
            true
        } else {
            false
        }
    }

    fn parse_id(&mut self) -> Option<SpannedText> {
        let start = self.pos;
        let s = &self.input[self.pos..];
        let mut chars = s.chars();
        let first = chars.next()?;
        if !(first.is_ascii_alphanumeric() || first == '_') {
            return None;
        }
        let mut idx = first.len_utf8();
        let mut last = first;
        for ch in chars {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                idx += ch.len_utf8();
                last = ch;
            } else {
                break;
            }
        }
        if last == '-' {
            return None;
        }
        let raw = &s[..idx];
        self.pos += idx;
        Some(SpannedText {
            text: raw.to_string(),
            span: SourceSpan::new(self.base_offset + start, self.base_offset + self.pos),
        })
    }

    fn parse_bool(&mut self) -> Option<(bool, SourceSpan)> {
        let start = self.pos;
        if self.input[self.pos..].starts_with("true") {
            self.pos += 4;
            return Some((
                true,
                SourceSpan::new(self.base_offset + start, self.base_offset + self.pos),
            ));
        }
        if self.input[self.pos..].starts_with("false") {
            self.pos += 5;
            return Some((
                false,
                SourceSpan::new(self.base_offset + start, self.base_offset + self.pos),
            ));
        }
        None
    }

    fn parse_number_value(&mut self) -> Option<SpannedRadarValue> {
        let start = self.pos;
        let s = &self.input[self.pos..];
        if !s.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            return None;
        }
        let mut idx = 0usize;
        let mut saw_dot = false;
        for ch in s.chars() {
            if ch.is_ascii_digit() {
                idx += ch.len_utf8();
                continue;
            }
            if ch == '.' && !saw_dot {
                saw_dot = true;
                idx += 1;
                continue;
            }
            break;
        }
        if idx == 0 {
            return None;
        }
        let token = &s[..idx];

        if saw_dot {
            if token.ends_with('.') {
                return None;
            }
            let v: f64 = token.parse().ok()?;
            self.pos += idx;
            let n = serde_json::Number::from_f64(v)?;
            return Some(SpannedRadarValue {
                value: Value::Number(n),
                span: SourceSpan::new(self.base_offset + start, self.base_offset + self.pos),
            });
        }

        if token.len() > 1 && token.starts_with('0') {
            return None;
        }
        let v: i64 = token.parse().ok()?;
        self.pos += idx;
        Some(SpannedRadarValue {
            value: Value::Number(serde_json::Number::from(v)),
            span: SourceSpan::new(self.base_offset + start, self.base_offset + self.pos),
        })
    }

    fn parse_quoted_string(&mut self) -> Option<SpannedText> {
        let parsed = parse_langium_string(&self.input[self.pos..], self.base_offset + self.pos)?;
        self.pos += parsed.consumed;
        Some(SpannedText {
            text: parsed.value,
            span: parsed.value_span,
        })
    }

    fn take_until_matching_brace(&mut self) -> std::result::Result<(&'a str, usize), String> {
        let mut depth = 1i64;
        let mut in_quote: Option<char> = None;
        let mut escaped = false;
        let content_start = self.pos;
        while let Some(ch) = self.peek_char() {
            let char_start = self.pos;
            self.pos += ch.len_utf8();
            if escaped {
                escaped = false;
                continue;
            }
            if let Some(q) = in_quote {
                if ch == '\\' {
                    escaped = true;
                    continue;
                }
                if ch == q {
                    in_quote = None;
                }
                continue;
            }
            if ch == '"' || ch == '\'' {
                in_quote = Some(ch);
                continue;
            }
            if ch == '{' {
                depth += 1;
                continue;
            }
            if ch == '}' {
                depth -= 1;
                if depth == 0 {
                    return Ok((
                        &self.input[content_start..char_start],
                        self.base_offset + content_start,
                    ));
                }
            }
        }
        Err("unterminated '{' in curve".to_string())
    }
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
            Error::DiagramParse { diagnostic, .. } => diagnostic.message().to_string(),
            other => other.to_string(),
        }
    }

    #[test]
    fn radar_parses_simple_definition() {
        let _ = parse(
            r#"radar-beta
axis A,B,C
curve mycurve{1,2,3}"#,
        );
    }

    #[test]
    fn radar_errors_on_empty_axis_statement() {
        assert_eq!(
            parse_err(
                r#"radar-beta
axis"#,
            ),
            "axis statement must include at least one axis"
        );
    }

    #[test]
    fn radar_parses_title_and_data() {
        let model = parse(
            r#"radar-beta
title Radar diagram
accTitle: Radar accTitle
accDescr: Radar accDescription
axis A["Axis A"], B["Axis B"] ,C["Axis C"]
curve mycurve["My Curve"]{1,2,3}
"#,
        );
        assert_eq!(model["title"], json!("Radar diagram"));
        assert_eq!(model["accTitle"], json!("Radar accTitle"));
        assert_eq!(model["accDescr"], json!("Radar accDescription"));
        assert_eq!(
            model["axes"],
            json!([
                {"name": "A", "label": "Axis A"},
                {"name": "B", "label": "Axis B"},
                {"name": "C", "label": "Axis C"},
            ])
        );
        assert_eq!(
            model["curves"],
            json!([
                {"name": "mycurve", "label": "My Curve", "entries": [1,2,3]},
            ])
        );
        assert_eq!(
            model["options"],
            json!({"showLegend": true, "ticks": 5, "max": Value::Null, "min": 0, "graticule": "circle"})
        );
    }

    #[test]
    fn radar_uses_langium_string_escapes_and_quote_aware_inline_comments() {
        let model = parse(
            r#"radar-beta
axis A["A\n100%% complete"], B['B\tlabel'] %% outside comment
curve sample{1,2}
"#,
        );

        assert_eq!(model["axes"][0]["label"], "An100%% complete");
        assert_eq!(model["axes"][1]["label"], "Btlabel");
    }

    #[test]
    fn radar_parses_options() {
        let model = parse(
            r#"radar-beta
ticks 10
showLegend false
graticule polygon
min 1
max 10
"#,
        );
        assert_eq!(
            model["options"],
            json!({"showLegend": false, "ticks": 10, "max": 10, "min": 1, "graticule": "polygon"})
        );
    }

    #[test]
    fn radar_parses_comma_separated_option_list_like_langium_grammar() {
        let model = parse(
            r#"radar-beta
ticks 5, max 10, min 1, graticule polygon, showLegend false
"#,
        );
        assert_eq!(
            model["options"],
            json!({"showLegend": false, "ticks": 5, "max": 10, "min": 1, "graticule": "polygon"})
        );
    }

    #[test]
    fn radar_editor_facts_preserve_every_repeated_occurrence_span() {
        let text = concat!(
            "radar-beta\r\n",
            "axis A[\"dup\"], A[\"dup\"]\r\n",
            "curve C[\"dup\"]{ A: 1,\r\n",
            "  A: 1 }, C[\"dup\"]{1,1}\r\n",
            "ticks 5\r\n",
            "ticks 5\r\n",
        );
        let facts = Engine::new()
            .parse_editor_semantic_facts_with_type_sync("radar", text, ParseOptions::strict())
            .unwrap()
            .unwrap();

        let occurrences = |needle: &str| {
            text.match_indices(needle)
                .map(|(start, value)| SourceSpan::new(start, start + value.len()))
                .collect::<Vec<_>>()
        };
        let spans_for = |detail: &str| {
            facts
                .symbols
                .iter()
                .filter(|symbol| symbol.detail.as_deref() == Some(detail))
                .map(|symbol| symbol.span)
                .collect::<Vec<_>>()
        };

        let axis_occurrences = occurrences("A");
        assert_eq!(spans_for("radar axis"), axis_occurrences[..2]);
        assert_eq!(
            spans_for("radar curve axis reference"),
            axis_occurrences[2..]
        );

        let label_occurrences = occurrences("dup");
        assert_eq!(spans_for("radar axis label"), label_occurrences[..2]);
        assert_eq!(spans_for("radar curve label"), label_occurrences[2..]);
        assert_eq!(spans_for("radar curve"), occurrences("C"));
        assert_eq!(spans_for("radar curve entry"), occurrences("1"));
        assert_eq!(spans_for("radar option"), occurrences("5"));
    }

    #[test]
    fn radar_errors_on_empty_curve_stmt() {
        let err = parse_err(
            r#"radar-beta
axis my-axis
curve
"#,
        );
        assert_eq!(err, "expected curve id");
    }

    #[test]
    fn radar_rejects_empty_curve_and_entry_list_elements_like_langium_grammar() {
        for (body, expected) in [
            ("curve C{}", "expected curve entry"),
            ("curve C{1,}", "expected curve entry"),
            ("curve C{,1}", "expected curve entry"),
            ("curve C{1},", "expected curve after ','"),
        ] {
            let err = parse_err(&format!("radar-beta\n{body}\n"));
            assert_eq!(err, expected, "unexpected result for {body:?}");
        }
    }

    #[test]
    fn radar_number_and_id_terminals_match_common_langium() {
        for body in ["ticks .5", "curve C{.5}"] {
            let _ = parse_err(&format!("radar-beta\n{body}\n"));
        }

        let model = parse("radar-beta\naxis 1A, 2B\ncurve 3C{1A: 1, 2B: 2}\n");
        assert_eq!(model["axes"][0]["name"], "1A");
        assert_eq!(model["axes"][1]["name"], "2B");
        assert_eq!(model["curves"][0]["name"], "3C");
        assert_eq!(model["curves"][0]["entries"], json!([1, 2]));
    }

    #[test]
    fn radar_recovery_reports_parser_diagnostic_with_exact_crlf_span() {
        let text = concat!(
            "radar-beta\r\n",
            "axis A\r\n",
            "  curve C{.5} %% hidden\r\n",
            "axis B\r\n",
        );
        let facts = Engine::new()
            .parse_editor_semantic_facts_with_type_sync("radar", text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let invalid = "curve C{.5}";
        let start = text.find(invalid).unwrap();

        assert!(facts.symbols.iter().any(|symbol| symbol.name == "B"));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(start, start + invalid.len()))
                && diagnostic.message.contains("expected entry number")
        }));
    }

    #[test]
    fn radar_editor_recovery_reports_curve_validation_errors() {
        let text = concat!(
            "radar-beta\r\n",
            "axis A[\"Axis A\"], B[\"Axis B\"]\r\n",
            "  curve sample{A: 1}  \r\n",
        );
        let invalid = "sample{A: 1}";
        let start = text.find(invalid).unwrap();
        let expected_span = SourceSpan::new(start, start + invalid.len());
        let engine = Engine::new();

        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("a detailed curve missing an axis must fail strict parsing");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected radar parse diagnostic");
        };
        assert_eq!(diagnostic.span(), Some(expected_span));

        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("radar", text, ParseOptions::strict())
            .unwrap()
            .expect("radar editor recovery facts");
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "sample"));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(expected_span)
                && diagnostic.message.contains("Missing entry for axis Axis B")
        }));
    }

    #[test]
    fn radar_errors_on_mixed_numeric_and_detailed_curve_entries() {
        let err = parse_err(
            r#"radar-beta
axis ax1, ax2
curve my-curve { 1, ax1 2 }"#,
        );
        assert_eq!(err, "mixed detailed and numeric entries are not supported");
    }

    #[test]
    fn radar_orders_detailed_curve_entries_by_axes() {
        let model = parse(
            r#"radar-beta
axis A,B,C
curve mycurve{ C: 3, A: 1, B: 2 }"#,
        );
        assert_eq!(
            model["curves"],
            json!([
                {"name": "mycurve", "label": "mycurve", "entries": [1,2,3]},
            ])
        );
    }

    #[test]
    fn radar_accepts_header_with_colon() {
        let _ = parse(
            r#"radar-beta:
axis A,B,C
curve mycurve{1,2,3}"#,
        );
    }

    #[test]
    fn radar_ignores_comment_lines() {
        let _ = parse(
            r#"radar-beta
%% This is a comment
axis A,B,C
%% This is another comment
curve mycurve{1,2,3}
"#,
        );
    }

    #[test]
    fn radar_errors_on_missing_axis_entry() {
        let err = parse_err(
            r#"radar-beta
axis A["Axis A"], B["Axis B"], C["Axis C"]
curve mycurve{ C: 3, A: 1 }"#,
        );
        assert_eq!(err, "Missing entry for axis Axis B");
    }

    #[test]
    fn radar_parses_config_override_directive() {
        let model = parse(
            r#"
%%{init: {'radar': {'marginTop': 80, 'axisLabelFactor': 1.25}}}%%
radar-beta
axis A,B,C
curve mycurve{1,2,3}
"#,
        );
        assert_eq!(model["config"]["radar"]["marginTop"], json!(80));
    }
}
