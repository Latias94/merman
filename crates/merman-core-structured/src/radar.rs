//! Radar grammar, statement recovery, and typed model construction.

#[cfg(feature = "editor-facts")]
use merman_core_preprocess::scan::langium::LangiumCommonFact;
use merman_core_preprocess::scan::langium::{
    LangiumCommonDbFields, LangiumCommonDiagnostic, LangiumCommonFacts, parse_langium_common,
    parse_langium_string, strip_langium_inline_comment,
};
use merman_model::families::radar::{RadarAxis, RadarCurve, RadarDiagramModel, RadarOptions};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

/// The header scan distinguishes empty source from a header-only model.
pub enum RadarHeader<'source> {
    Empty,
    Invalid,
    Body(RadarBody<'source>),
}

pub struct RadarBody<'source> {
    code: &'source str,
    offset: usize,
}

#[derive(Debug)]
pub enum RadarBodyIssue {
    Common(LangiumCommonDiagnostic),
    Statement {
        message: String,
        #[cfg(feature = "editor-facts")]
        span: SourceSpan,
    },
    Curve {
        message: String,
        span: SourceSpan,
    },
}

#[cfg(feature = "editor-facts")]
pub enum RadarFieldKind {
    Axis,
    AxisLabel,
    Curve,
    CurveLabel,
    AxisReference,
    Entry,
    Option,
}

#[cfg(feature = "editor-facts")]
pub enum RadarFieldValue<'a> {
    Text(&'a str),
    Number(f64),
    Bool(bool),
}

/// Borrowed syntax events for the Core editor projection.
#[cfg(feature = "editor-facts")]
pub enum RadarObservation<'a> {
    Common(&'a LangiumCommonFact),
    Issue(&'a RadarBodyIssue),
    Field {
        kind: RadarFieldKind,
        value: RadarFieldValue<'a>,
        span: SourceSpan,
    },
}

const MAX_TICKS: i64 = 32;

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
    Number(f64),
    Graticule(String),
}

#[derive(Debug, Clone)]
struct OptionAst {
    name: String,
    value: OptionValueAst,
    #[cfg(feature = "editor-facts")]
    value_span: SourceSpan,
}

#[derive(Debug, Clone)]
struct SpannedRadarValue {
    value: f64,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct RadarDb {
    title: Option<String>,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    axes: Vec<RadarAxis>,
    curves: Vec<RadarCurve>,
    options: RadarOptions,
}

impl RadarDb {
    fn new() -> Self {
        Self {
            title: None,
            acc_title: None,
            acc_descr: None,
            axes: Vec::new(),
            curves: Vec::new(),
            options: RadarOptions::default(),
        }
    }

    fn set_axes_controlled(
        &mut self,
        axes: Vec<AxisAst>,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let mut rendered = Vec::with_capacity(axes.len());
        for axis in axes {
            control.checkpoint()?;
            rendered.push(RadarAxis {
                label: axis
                    .label
                    .map(|label| label.text)
                    .unwrap_or_else(|| axis.name.text.clone()),
                name: axis.name.text,
            });
        }
        self.axes = rendered;
        Ok(())
    }

    fn set_curves_controlled(
        &mut self,
        curves: Vec<CurveAst>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), RadarBodyIssue>> {
        let mut rendered = Vec::with_capacity(curves.len());
        for curve in curves {
            control.checkpoint()?;
            let label = curve
                .label
                .as_ref()
                .map(|label| label.text.clone())
                .unwrap_or_else(|| curve.name.text.clone());
            let entries =
                match compute_curve_entries_controlled(&self.axes, &curve.entries, control)? {
                    Ok(entries) => entries,
                    Err(message) => {
                        return Ok(Err(RadarBodyIssue::Curve {
                            message,
                            span: curve.span,
                        }));
                    }
                };
            rendered.push(RadarCurve {
                name: curve.name.text,
                label,
                entries,
            });
        }
        self.curves = rendered;
        Ok(Ok(()))
    }

    fn set_options_controlled(
        &mut self,
        options: Vec<OptionAst>,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let mut last: std::collections::HashMap<String, OptionValueAst> =
            std::collections::HashMap::new();
        for opt in options {
            control.checkpoint()?;
            last.insert(opt.name, opt.value);
        }

        if let Some(OptionValueAst::Bool(v)) = last.get("showLegend") {
            self.options.show_legend = *v;
        }
        if let Some(OptionValueAst::Number(v)) = last.get("ticks") {
            // Upstream clamps the last Number, including positive infinity.
            self.options.ticks = if *v > MAX_TICKS as f64 {
                MAX_TICKS as f64
            } else {
                *v
            };
        }
        if let Some(OptionValueAst::Number(v)) = last.get("max") {
            self.options.max = Some(*v);
        }
        if let Some(OptionValueAst::Number(v)) = last.get("min") {
            self.options.min = *v;
        }
        if let Some(OptionValueAst::Graticule(v)) = last.get("graticule") {
            self.options.graticule = v.clone();
        }
        Ok(())
    }

    #[inline]
    fn into_model(self) -> RadarDiagramModel {
        RadarDiagramModel {
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
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

#[cfg(feature = "editor-facts")]
impl SpannedText {
    fn observation(&self, kind: RadarFieldKind) -> RadarObservation<'_> {
        RadarObservation::Field {
            kind,
            value: RadarFieldValue::Text(&self.text),
            span: self.span,
        }
    }
}

enum RadarStatementEvent {
    Axes(Vec<AxisAst>),
    Curves(Vec<CurveAst>),
    Options(Vec<OptionAst>),
}

struct RadarParsedStatement {
    event: RadarStatementEvent,
    consumed: usize,
}

struct RadarParsedStatements {
    events: Vec<RadarStatementEvent>,
    consumed: usize,
}

fn parse_radar_statement(
    stmt: &str,
    stmt_start: usize,
    control: &OperationControl,
) -> OperationControlResult<Result<RadarParsedStatements, String>> {
    let mut events = Vec::new();
    let mut consumed = 0;
    while consumed < stmt.len() {
        control.checkpoint()?;
        if !events.is_empty() && parse_langium_common(stmt, consumed).is_some() {
            break;
        }
        match parse_radar_statement_prefix(&stmt[consumed..], stmt_start + consumed, control)? {
            Ok(parsed) => {
                consumed += parsed.consumed;
                events.push(parsed.event);
            }
            // Retain preceding statements; the main loop can then collect a following
            // multiline curve or recover the next invalid statement at its own offset.
            Err(_) if !events.is_empty() => break,
            Err(error) => return Ok(Err(error)),
        }
    }
    Ok(Ok(RadarParsedStatements { events, consumed }))
}

fn parse_radar_statement_prefix(
    stmt: &str,
    stmt_start: usize,
    control: &OperationControl,
) -> OperationControlResult<Result<RadarParsedStatement, String>> {
    control.checkpoint()?;
    let mut p = TokenParser::new(stmt, stmt_start);
    p.skip_ws();
    if p.try_keyword("axis") {
        p.skip_ws();
        if p.eof() {
            return Ok(Err(
                "axis statement must include at least one axis".to_string()
            ));
        }
        let axes = match parse_axes_list(&mut p, control)? {
            Ok(axes) => axes,
            Err(error) => return Ok(Err(error)),
        };
        return Ok(Ok(RadarParsedStatement {
            event: RadarStatementEvent::Axes(axes),
            consumed: p.pos,
        }));
    }

    if p.try_keyword("curve") {
        let curves = match parse_curves_stmt(&mut p, control)? {
            Ok(curves) => curves,
            Err(error) => return Ok(Err(error)),
        };
        return Ok(Ok(RadarParsedStatement {
            event: RadarStatementEvent::Curves(curves),
            consumed: p.pos,
        }));
    }

    let options = match parse_option_list_stmt(&mut p, control)? {
        Ok(options) => options,
        Err(error) => return Ok(Err(error)),
    };
    if let Some(options) = options {
        return Ok(Ok(RadarParsedStatement {
            event: RadarStatementEvent::Options(options),
            consumed: p.pos,
        }));
    }

    Ok(Err(format!("unexpected radar statement: {}", stmt.trim())))
}

#[cfg(feature = "editor-facts")]
fn observe_radar_statement(
    event: &RadarStatementEvent,
    observe: &mut dyn FnMut(RadarObservation<'_>),
    control: &OperationControl,
) -> OperationControlResult<()> {
    match event {
        RadarStatementEvent::Axes(axes) => {
            for axis in axes {
                control.checkpoint()?;
                observe(axis.name.observation(RadarFieldKind::Axis));
                if let Some(label) = &axis.label {
                    observe(label.observation(RadarFieldKind::AxisLabel));
                }
            }
        }
        RadarStatementEvent::Curves(curves) => {
            for curve in curves {
                control.checkpoint()?;
                observe(curve.name.observation(RadarFieldKind::Curve));
                if let Some(label) = &curve.label {
                    observe(label.observation(RadarFieldKind::CurveLabel));
                }
                for entry in &curve.entries {
                    control.checkpoint()?;
                    if let Some(axis) = &entry.axis {
                        observe(axis.observation(RadarFieldKind::AxisReference));
                    }
                    observe(RadarObservation::Field {
                        kind: RadarFieldKind::Entry,
                        value: RadarFieldValue::Number(entry.value.value),
                        span: entry.value.span,
                    });
                }
            }
        }
        RadarStatementEvent::Options(options) => {
            for option in options {
                control.checkpoint()?;
                let value = match &option.value {
                    OptionValueAst::Bool(value) => RadarFieldValue::Bool(*value),
                    OptionValueAst::Number(value) => RadarFieldValue::Number(*value),
                    OptionValueAst::Graticule(value) => RadarFieldValue::Text(value),
                };
                observe(RadarObservation::Field {
                    kind: RadarFieldKind::Option,
                    value,
                    span: option.value_span,
                });
            }
        }
    }
    Ok(())
}

fn apply_radar_statement_controlled(
    event: RadarStatementEvent,
    axes: &mut Vec<AxisAst>,
    curves: &mut Vec<CurveAst>,
    options: &mut Vec<OptionAst>,
    control: &OperationControl,
) -> OperationControlResult<()> {
    match event {
        RadarStatementEvent::Axes(parsed) => {
            for axis in parsed {
                control.checkpoint()?;
                axes.push(axis);
            }
        }
        RadarStatementEvent::Curves(parsed) => {
            for curve in parsed {
                control.checkpoint()?;
                curves.push(curve);
            }
        }
        RadarStatementEvent::Options(parsed) => {
            for option in parsed {
                control.checkpoint()?;
                options.push(option);
            }
        }
    }
    Ok(())
}

fn compute_curve_entries_controlled(
    axes: &[RadarAxis],
    entries: &[EntryAst],
    control: &OperationControl,
) -> OperationControlResult<Result<Vec<f64>, String>> {
    control.checkpoint()?;
    if entries.is_empty() {
        return Ok(Ok(Vec::new()));
    }

    if entries[0].axis.is_none() {
        let mut values = Vec::with_capacity(entries.len());
        for entry in entries {
            control.checkpoint()?;
            values.push(entry.value.value);
        }
        return Ok(Ok(values));
    }

    if axes.is_empty() {
        return Ok(Err(
            "Axes must be populated before curves for reference entries".to_string(),
        ));
    }

    let mut entries_by_axis = std::collections::HashMap::with_capacity(entries.len());
    for entry in entries {
        control.checkpoint()?;
        if let Some(axis) = &entry.axis {
            entries_by_axis.entry(axis.text.as_str()).or_insert(entry);
        }
    }

    let mut values = Vec::with_capacity(axes.len());
    for axis in axes {
        control.checkpoint()?;
        let Some(found) = entries_by_axis.get(axis.name.as_str()) else {
            return Ok(Err(format!("Missing entry for axis {}", axis.label)));
        };
        values.push(found.value.value);
    }
    Ok(Ok(values))
}

impl RadarBody<'_> {
    /// Builds the model from the parsed header and body.
    pub fn parse_controlled(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<RadarDiagramModel, RadarBodyIssue>> {
        self.parse(
            #[cfg(feature = "editor-facts")]
            false,
            #[cfg(feature = "editor-facts")]
            None,
            control,
        )
    }

    /// Observes issues in source and validation order and fields during rich capture.
    #[cfg(feature = "editor-facts")]
    pub fn parse_with_observer_controlled(
        self,
        capture_fields: bool,
        observe: Option<&mut dyn FnMut(RadarObservation<'_>)>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<RadarDiagramModel, RadarBodyIssue>> {
        self.parse(capture_fields, observe, control)
    }

    fn parse(
        self,
        #[cfg(feature = "editor-facts")] capture_fields: bool,
        #[cfg(feature = "editor-facts")] mut observe: Option<&mut dyn FnMut(RadarObservation<'_>)>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<RadarDiagramModel, RadarBodyIssue>> {
        let Self { code, mut offset } = self;
        let mut common = LangiumCommonFacts::default();
        let mut axes = Vec::new();
        let mut curves = Vec::new();
        let mut options = Vec::new();
        let mut first_issue = None;

        while offset < code.len() {
            control.checkpoint()?;
            if let Some(parsed) = parse_langium_common(code, offset) {
                if let Some(diagnostic) = parsed.diagnostic {
                    let issue = RadarBodyIssue::Common(diagnostic);
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut() {
                        observe(RadarObservation::Issue(&issue));
                    }
                    first_issue.get_or_insert(issue);
                }
                #[cfg(feature = "editor-facts")]
                if let Some(observe) = observe.as_deref_mut().filter(|_| capture_fields) {
                    observe(RadarObservation::Common(&parsed.fact));
                }
                common.push(parsed.fact);
                offset += parsed.consumed;
                continue;
            }

            let (statement, statement_start, next_offset) =
                radar_statement_at(code, offset, control)?;
            offset = next_offset;
            if statement.is_empty() {
                continue;
            }

            match parse_radar_statement(&statement, statement_start, control)? {
                Ok(parsed) => {
                    // The entry grammar repeats statements without requiring a newline.
                    // Continue at the next token, retaining the original source offsets.
                    offset = statement_start + parsed.consumed;
                    for event in parsed.events {
                        #[cfg(feature = "editor-facts")]
                        if let Some(observe) = observe.as_deref_mut().filter(|_| capture_fields) {
                            observe_radar_statement(&event, observe, control)?;
                        }
                        apply_radar_statement_controlled(
                            event,
                            &mut axes,
                            &mut curves,
                            &mut options,
                            control,
                        )?;
                    }
                }
                Err(message) => {
                    let issue = RadarBodyIssue::Statement {
                        message,
                        #[cfg(feature = "editor-facts")]
                        span: SourceSpan::new(
                            statement_start,
                            statement_start + statement.trim_end().len(),
                        ),
                    };
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut() {
                        observe(RadarObservation::Issue(&issue));
                    }
                    first_issue.get_or_insert(issue);
                }
            }
        }

        let common = LangiumCommonDbFields::from_facts(&common);
        let mut db = RadarDb::new();
        db.title = common.title;
        db.acc_title = common.acc_title;
        db.acc_descr = common.acc_descr;
        db.set_axes_controlled(axes, control)?;
        if let Err(issue) = db.set_curves_controlled(curves, control)? {
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe {
                observe(RadarObservation::Issue(&issue));
            }
            first_issue.get_or_insert(issue);
        }
        db.set_options_controlled(options, control)?;
        control.checkpoint()?;

        match first_issue {
            Some(issue) => Ok(Err(issue)),
            None => Ok(Ok(db.into_model())),
        }
    }
}

pub fn radar_body_start_controlled<'source>(
    code: &'source str,
    control: &OperationControl,
) -> OperationControlResult<RadarHeader<'source>> {
    let mut offset = 0usize;
    while offset < code.len() {
        control.checkpoint()?;
        let (line, next_offset) = physical_line(code, offset);
        let visible = strip_langium_inline_comment(line);
        let trimmed = visible.trim_start();
        if trimmed.trim().is_empty() {
            offset = next_offset;
            continue;
        }
        let Some(after_keyword) = trimmed.strip_prefix("radar-beta") else {
            return Ok(RadarHeader::Invalid);
        };
        if after_keyword
            .chars()
            .next()
            .is_some_and(|ch| !ch.is_whitespace() && ch != ':' && !after_keyword.starts_with("%%"))
        {
            return Ok(RadarHeader::Invalid);
        }

        let leading = visible.len() - trimmed.len();
        let header_start = offset + leading;
        let mut body_start = header_start + "radar-beta".len();
        let whitespace = code[body_start..]
            .chars()
            .take_while(|ch| matches!(ch, ' ' | '\t'))
            .map(char::len_utf8)
            .sum::<usize>();
        let colon_start = body_start + whitespace;
        if code.as_bytes().get(colon_start) == Some(&b':') {
            body_start = colon_start + 1;
        }
        return Ok(RadarHeader::Body(RadarBody {
            code,
            offset: body_start,
        }));
    }
    Ok(RadarHeader::Empty)
}

fn radar_statement_at(
    code: &str,
    offset: usize,
    control: &OperationControl,
) -> OperationControlResult<(String, usize, usize)> {
    control.checkpoint()?;
    let (line, mut next_offset) = physical_line(code, offset);
    let visible = strip_langium_inline_comment(line);
    let trimmed = visible.trim();
    let stmt_start = offset + visible.find(trimmed).unwrap_or(0);
    if trimmed.is_empty() {
        return Ok((String::new(), stmt_start, next_offset));
    }
    let mut stmt = mask_radar_inline_comments(&code[stmt_start..next_offset], control)?;
    let mut brace_balance = RadarBraceBalance::default();
    brace_balance.scan(&stmt, control)?;

    if stmt.trim_start().starts_with("curve")
        && brace_balance.saw_opening_brace
        && !brace_balance.is_balanced()
    {
        while next_offset < code.len() {
            control.checkpoint()?;
            let segment_start = next_offset;
            let (_, after_next) = physical_line(code, next_offset);
            next_offset = after_next;
            let segment = mask_radar_inline_comments(&code[segment_start..next_offset], control)?;
            brace_balance.scan(&segment, control)?;
            stmt.push_str(&segment);
            if brace_balance.is_balanced() {
                break;
            }
        }
    }

    Ok((stmt, stmt_start, next_offset))
}

fn mask_radar_inline_comments(
    source: &str,
    control: &OperationControl,
) -> OperationControlResult<String> {
    let mut masked = source.as_bytes().to_vec();
    let mut line_start = 0usize;
    while line_start < source.len() {
        control.checkpoint()?;
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
    Ok(String::from_utf8(masked)
        .expect("comment masking preserves UTF-8 before the comment marker"))
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

#[derive(Default)]
struct RadarBraceBalance {
    in_quote: Option<char>,
    escaped: bool,
    depth: i64,
    saw_opening_brace: bool,
}

impl RadarBraceBalance {
    fn scan(&mut self, source: &str, control: &OperationControl) -> OperationControlResult<()> {
        for (offset, ch) in source.char_indices() {
            if offset % 4096 < ch.len_utf8() {
                control.checkpoint()?;
            }
            if self.escaped {
                self.escaped = false;
                continue;
            }
            if ch == '\\' && self.in_quote.is_some() {
                self.escaped = true;
                continue;
            }
            if let Some(quote) = self.in_quote {
                if ch == quote {
                    self.in_quote = None;
                }
                continue;
            }
            if ch == '"' || ch == '\'' {
                self.in_quote = Some(ch);
                continue;
            }
            if ch == '{' {
                self.saw_opening_brace = true;
                self.depth += 1;
            } else if ch == '}' {
                self.depth -= 1;
            }
        }
        Ok(())
    }

    fn is_balanced(&self) -> bool {
        self.depth == 0
    }
}

fn parse_axes_list(
    p: &mut TokenParser<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<Vec<AxisAst>, String>> {
    let mut out = Vec::new();
    loop {
        control.checkpoint()?;
        p.skip_ws();
        if p.eof() || p.at_statement_start() {
            return Ok(Err("expected axis id".to_string()));
        }
        let Some(name) = p.parse_id() else {
            return Ok(Err("expected axis id".to_string()));
        };
        p.skip_ws();
        let label = if p.try_consume('[') {
            p.skip_ws();
            let Some(s) = p.parse_quoted_string() else {
                return Ok(Err("expected quoted axis label".to_string()));
            };
            p.skip_ws();
            if !p.try_consume(']') {
                return Ok(Err("expected ']'".to_string()));
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
                return Ok(Err("unexpected trailing ',' in axis list".to_string()));
            }
            continue;
        }
        if p.eof() || p.at_statement_start() {
            break;
        }
        return Ok(Err("expected ',' or end of axis list".to_string()));
    }
    Ok(Ok(out))
}

fn parse_curves_stmt(
    p: &mut TokenParser<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<Vec<CurveAst>, String>> {
    control.checkpoint()?;
    p.skip_ws();
    if p.eof() {
        return Ok(Err("expected curve id".to_string()));
    }

    let mut curves = Vec::new();
    loop {
        control.checkpoint()?;
        match parse_curve(p, control)? {
            Ok(curve) => curves.push(curve),
            Err(error) => return Ok(Err(error)),
        }
        p.skip_ws();
        if p.try_consume(',') {
            p.skip_ws();
            if p.eof() {
                return Ok(Err("expected curve after ','".to_string()));
            }
            continue;
        }
        if p.eof() || p.at_statement_start() {
            break;
        }
        return Ok(Err("unexpected trailing tokens after curve".to_string()));
    }
    Ok(Ok(curves))
}

fn parse_curve(
    p: &mut TokenParser<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<CurveAst, String>> {
    control.checkpoint()?;
    p.skip_ws();
    let curve_start = p.pos;
    let (name, label, entries_str, entries_start) = {
        let Some(name) = p.parse_id() else {
            return Ok(Err("expected curve id".to_string()));
        };
        p.skip_ws();
        let label = if p.try_consume('[') {
            p.skip_ws();
            let Some(s) = p.parse_quoted_string() else {
                return Ok(Err("expected quoted curve label".to_string()));
            };
            p.skip_ws();
            if !p.try_consume(']') {
                return Ok(Err("expected ']'".to_string()));
            }
            p.skip_ws();
            Some(s)
        } else {
            None
        };

        if !p.try_consume('{') {
            return Ok(Err("expected '{'".to_string()));
        }

        let (entries_str, entries_start) = match p.take_until_matching_brace(control)? {
            Ok(entries) => entries,
            Err(error) => return Ok(Err(error)),
        };

        (name, label, entries_str, entries_start)
    };
    let entries = match parse_entries(entries_str, entries_start, control)? {
        Ok(entries) => entries,
        Err(error) => return Ok(Err(error)),
    };

    let has_detailed = entries.iter().any(|e| e.axis.is_some());
    let has_numeric = entries.iter().any(|e| e.axis.is_none());
    if has_detailed && has_numeric {
        return Ok(Err(
            "mixed detailed and numeric entries are not supported".to_string()
        ));
    }

    Ok(Ok(CurveAst {
        name,
        label,
        entries,
        span: SourceSpan::new(p.base_offset + curve_start, p.base_offset + p.pos),
    }))
}

fn parse_entries(
    input: &str,
    input_start: usize,
    control: &OperationControl,
) -> OperationControlResult<Result<Vec<EntryAst>, String>> {
    let items = split_top_level(input, ',', control)?;
    let mut out = Vec::new();
    for (item_offset, item) in items {
        control.checkpoint()?;
        let (item, item_start) = trim_start_with_source_offset(item, input_start + item_offset);
        let item = item.trim_end();
        if item.is_empty() {
            return Ok(Err("expected curve entry".to_string()));
        }

        // Try detailed first: <ID> ':'? <NUMBER>
        let mut p = TokenParser::new(item, item_start);
        p.skip_ws();
        let checkpoint = p.checkpoint();
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
        p.rollback(checkpoint);

        // Otherwise numeric: <NUMBER>
        p.skip_ws();
        let Some(num) = p.parse_number_value() else {
            return Ok(Err("expected entry number".to_string()));
        };
        p.skip_ws();
        if !p.eof() {
            return Ok(Err("unexpected trailing tokens in entry".to_string()));
        }
        out.push(EntryAst {
            axis: None,
            value: num,
        });
    }
    Ok(Ok(out))
}

fn parse_option_stmt(p: &mut TokenParser<'_>) -> Result<Option<OptionAst>, String> {
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
        let (value, _value_span) = p
            .parse_bool()
            .ok_or_else(|| "expected boolean".to_string())?;
        return Ok(Some(OptionAst {
            name,
            value: OptionValueAst::Bool(value),
            #[cfg(feature = "editor-facts")]
            value_span: _value_span,
        }));
    }

    if name == "graticule" {
        let value = p
            .parse_id()
            .ok_or_else(|| "expected graticule".to_string())?;
        if value.text != "circle" && value.text != "polygon" {
            return Err("expected graticule".to_string());
        }
        return Ok(Some(OptionAst {
            name,
            value: OptionValueAst::Graticule(value.text),
            #[cfg(feature = "editor-facts")]
            value_span: value.span,
        }));
    }

    let value = p
        .parse_number_value()
        .ok_or_else(|| "expected number".to_string())?;
    Ok(Some(OptionAst {
        name,
        value: OptionValueAst::Number(value.value),
        #[cfg(feature = "editor-facts")]
        value_span: value.span,
    }))
}

fn parse_option_list_stmt(
    p: &mut TokenParser<'_>,
    control: &OperationControl,
) -> OperationControlResult<Result<Option<Vec<OptionAst>>, String>> {
    let mut out = Vec::new();
    loop {
        control.checkpoint()?;
        let opt = match parse_option_stmt(p) {
            Ok(opt) => opt,
            Err(error) => return Ok(Err(error)),
        };
        let Some(opt) = opt else {
            return if out.is_empty() {
                Ok(Ok(None))
            } else {
                Ok(Err("unexpected trailing tokens after option".to_string()))
            };
        };
        out.push(opt);
        p.skip_ws();
        if p.eof() {
            break;
        }
        // Radar repeats Option groups without requiring commas or newlines. NUMBER
        // may end immediately before the next keyword, as in `ticks 1.5min 2`.
        if p.try_consume(',') {
            p.skip_ws();
            if p.eof() {
                return Ok(Err("expected option after ','".to_string()));
            }
        } else if p.at_statement_start() {
            break;
        }
    }
    Ok(Ok(Some(out)))
}

fn split_top_level<'a>(
    input: &'a str,
    delim: char,
    control: &OperationControl,
) -> OperationControlResult<Vec<(usize, &'a str)>> {
    let mut out = Vec::new();
    let mut chunk_start = 0usize;
    let mut in_quote: Option<char> = None;
    let mut escaped = false;
    let mut brace_depth = 0i64;
    let mut bracket_depth = 0i64;
    for (offset, ch) in input.char_indices() {
        if offset % 4096 < ch.len_utf8() {
            control.checkpoint()?;
        }
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
    Ok(out)
}

#[derive(Debug, Clone, Copy)]
struct TokenParserCheckpoint {
    pos: usize,
}

struct TokenParser<'input> {
    input: &'input str,
    pos: usize,
    base_offset: usize,
}

impl<'input> TokenParser<'input> {
    fn new(input: &'input str, base_offset: usize) -> Self {
        Self {
            input,
            pos: 0,
            base_offset,
        }
    }

    fn checkpoint(&self) -> TokenParserCheckpoint {
        TokenParserCheckpoint { pos: self.pos }
    }

    fn rollback(&mut self, checkpoint: TokenParserCheckpoint) {
        self.pos = checkpoint.pos;
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

    fn at_keyword(&self, keyword: &str) -> bool {
        self.input[self.pos..]
            .strip_prefix(keyword)
            .is_some_and(|rest| {
                !rest
                    .chars()
                    .next()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
            })
    }

    fn try_keyword(&mut self, keyword: &str) -> bool {
        if !self.at_keyword(keyword) {
            return false;
        }
        self.pos += keyword.len();
        true
    }

    fn at_statement_start(&self) -> bool {
        [
            "axis",
            "curve",
            "showLegend",
            "ticks",
            "max",
            "min",
            "graticule",
        ]
        .into_iter()
        .any(|keyword| self.at_keyword(keyword))
            || parse_langium_common(self.input, self.pos).is_some()
    }

    fn parse_id(&mut self) -> Option<SpannedText> {
        #[cfg(feature = "editor-facts")]
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
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(self.base_offset + start, self.base_offset + self.pos),
        })
    }

    fn parse_bool(&mut self) -> Option<(bool, SourceSpan)> {
        let start = self.pos;
        if self.input[self.pos..].starts_with("true") {
            self.pos += 4;
            let span = SourceSpan::new(self.base_offset + start, self.base_offset + self.pos);
            return Some((true, span));
        }
        if self.input[self.pos..].starts_with("false") {
            self.pos += 5;
            let span = SourceSpan::new(self.base_offset + start, self.base_offset + self.pos);
            return Some((false, span));
        }
        None
    }

    fn parse_number_value(&mut self) -> Option<SpannedRadarValue> {
        #[cfg(feature = "editor-facts")]
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

        if (saw_dot && token.ends_with('.'))
            || (!saw_dot && token.len() > 1 && token.starts_with('0'))
        {
            return None;
        }
        // common.langium NUMBER uses Number(input) for both FLOAT and INT. Keep the
        // binary64 rounding and overflow until database options have been applied.
        let value: f64 = token.parse().ok()?;
        self.pos += idx;
        #[cfg(feature = "editor-facts")]
        let span = SourceSpan::new(self.base_offset + start, self.base_offset + self.pos);
        Some(SpannedRadarValue {
            value,
            #[cfg(feature = "editor-facts")]
            span,
        })
    }

    fn parse_quoted_string(&mut self) -> Option<SpannedText> {
        let parsed = parse_langium_string(&self.input[self.pos..], self.base_offset + self.pos)?;
        self.pos += parsed.consumed;
        Some(SpannedText {
            text: parsed.value,
            #[cfg(feature = "editor-facts")]
            span: parsed.value_span,
        })
    }

    fn take_until_matching_brace(
        &mut self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(&'input str, usize), String>> {
        let mut depth = 1i64;
        let mut in_quote: Option<char> = None;
        let mut escaped = false;
        let content_start = self.pos;
        while let Some(ch) = self.peek_char() {
            let char_start = self.pos;
            if char_start % 4096 < ch.len_utf8() {
                control.checkpoint()?;
            }
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
                    return Ok(Ok((
                        &self.input[content_start..char_start],
                        self.base_offset + content_start,
                    )));
                }
            }
        }
        Ok(Err("unterminated '{' in curve".to_string()))
    }
}

#[cfg(test)]
mod tests;
