#[cfg(feature = "presentation")]
use crate::SourcePresentationMetadata;
use crate::diagrams::scan::{LineCursor, is_ecmascript_whitespace, starts_with_case_insensitive};
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, family::EditorFactsDemand,
};
use crate::{Error, OperationControl, OperationControlResult, Result, SourceSpan};
#[cfg(feature = "compat-json")]
use serde_json::Value;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static TIMELINE_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_timeline_syntax_construction_count() {
    TIMELINE_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn timeline_syntax_construction_count() -> usize {
    TIMELINE_SYNTAX_CONSTRUCTION_COUNT.get()
}

#[cfg(feature = "presentation")]
use merman_model::families::timeline::TimelineOutputData;
use merman_model::families::timeline::{TimelineDiagramModel, TimelineDirection, TimelineTask};

pub(crate) fn sanitize_common_db_fields(
    model: &mut TimelineDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[derive(Debug, Default)]
struct TimelineDb {
    direction: TimelineDirection,
    title: String,
    acc_title: String,
    acc_descr: String,

    current_section: String,
    current_section_index: Option<usize>,
    sections: Vec<String>,
    tasks: Vec<TimelineTask>,
    next_id: i64,
}

impl TimelineDb {
    fn add_section(&mut self, txt: &str) {
        self.current_section = txt.to_string();
        self.current_section_index = Some(self.sections.len());
        self.sections.push(txt.to_string());
    }

    fn add_task(&mut self, period: &str) {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(TimelineTask {
            id,
            section: self.current_section.clone(),
            section_index: self.current_section_index,
            task_type: self.current_section.clone(),
            task: period.to_string(),
            score: 0,
            events: Vec::new(),
        });
    }

    fn add_event(&mut self, event: &str) -> Result<()> {
        let Some(last) = self.tasks.last_mut() else {
            return Err(Error::diagram_parse_fallback(
                "timeline".to_string(),
                "event without a preceding task".to_string(),
            ));
        };
        last.events.push(event.to_string());
        Ok(())
    }
}

struct TimelineSemanticSource {
    model: TimelineDiagramModel,
    #[cfg(feature = "presentation")]
    output_data: TimelineOutputData,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

#[cfg(feature = "presentation")]
impl TimelineSemanticSource {
    fn into_render_parts(self) -> (TimelineDiagramModel, TimelineOutputData) {
        (self.model, self.output_data)
    }

    #[cfg(feature = "compat-json")]
    fn into_compat_json(self, meta: &SourcePresentationMetadata) -> Result<Value> {
        let (model, output_data) = self.into_render_parts();
        render_model_to_compat_json(&model, output_data, meta)
    }
}

struct TimelineParseIssue {
    error: Error,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
}

struct TimelineParseOutcome {
    source: TimelineSemanticSource,
    issues: Vec<TimelineParseIssue>,
}

impl TimelineParseOutcome {
    fn into_strict_source(self) -> Result<TimelineSemanticSource> {
        if let Some(issue) = self.issues.into_iter().next() {
            return Err(issue.error);
        }
        Ok(self.source)
    }

    fn into_semantic_construction(
        self,
        #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    ) -> crate::family::SemanticConstruction<TimelineSemanticSource> {
        #[cfg(feature = "editor-facts")]
        {
            let mut this = self;

            if editor_demand == EditorFactsDemand::Omit {
                return crate::family::SemanticConstruction {
                    model: this.into_strict_source(),
                    editor_facts: None,
                };
            }
            let mut editor_facts = this.source.editor_facts.take().unwrap_or_default();
            let mut first_error = None;
            for issue in this.issues {
                editor_facts.mark_recovered_from_parse_error(
                    format!(
                        "timeline parser recovered after parse error: {}",
                        issue.error
                    ),
                    Some(issue.span),
                );
                if first_error.is_none() {
                    first_error = Some(issue.error);
                }
            }
            crate::family::SemanticConstruction {
                model: match first_error {
                    Some(error) => Err(error),
                    None => Ok(this.source),
                },
                editor_facts: Some(editor_facts),
            }
        }
        #[cfg(not(feature = "editor-facts"))]
        {
            crate::family::SemanticConstruction {
                model: self.into_strict_source(),
            }
        }
    }

    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn into_combined(
        self,
        meta: &SourcePresentationMetadata,
    ) -> crate::family::CombinedSemanticParse {
        let construction = self.into_semantic_construction(EditorFactsDemand::Capture);
        let editor_facts = construction.editor_facts.unwrap_or_default();
        let construction = match construction.model {
            Ok(source) => Ok((source, editor_facts)),
            Err(error) => Err(crate::family::CombinedSemanticFailure::new(
                error,
                Some(editor_facts),
            )),
        };
        crate::family::CombinedSemanticParse::from_construction(
            construction,
            |(source, editor_facts)| (source.into_compat_json(meta), editor_facts),
            crate::family::CombinedSemanticFailure::into_parts,
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct TimelineSource<'source> {
    text: &'source str,
    start: usize,
}

impl<'source> TimelineSource<'source> {
    fn new(text: &'source str, start: usize) -> Self {
        Self { text, start }
    }

    fn end(self) -> usize {
        self.start + self.text.len()
    }

    fn span(self) -> SourceSpan {
        SourceSpan::new(self.start, self.end())
    }

    fn subslice(self, start: usize, end: usize) -> Self {
        debug_assert!(start <= end);
        debug_assert!(end <= self.text.len());
        Self::new(&self.text[start..end], self.start + start)
    }

    fn trim_start(self) -> Self {
        let leading =
            self.text.len() - self.text.trim_start_matches(is_ecmascript_whitespace).len();
        self.subslice(leading, self.text.len())
    }

    fn trim_end(self) -> Self {
        let text = self.text.trim_end_matches(is_ecmascript_whitespace);
        self.subslice(0, text.len())
    }

    fn trim(self) -> Self {
        self.trim_start().trim_end()
    }
}

#[derive(Debug, Clone, Copy)]
struct ParsedSpaceDirective<'source> {
    value: TimelineSource<'source>,
}

#[derive(Debug, Clone, Copy)]
struct ParsedColonDirective<'source> {
    value: TimelineSource<'source>,
}

struct ParsedAccDescrBlock<'source> {
    keyword: SourceSpan,
    closing: Option<SourceSpan>,
    value: TimelineSource<'source>,
}

#[derive(Debug, Clone, Copy)]
struct ParsedTimelineEvent<'source> {
    value: TimelineSource<'source>,
}

struct ParsedTimelineEvents<'source> {
    events: Vec<ParsedTimelineEvent<'source>>,
    issue: Option<TimelineParseIssue>,
}

#[derive(Debug, Clone, Copy)]
enum TimelinePeriodSuffix<'source> {
    None,
    HashComment,
    Events(TimelineSource<'source>),
}

#[derive(Debug, Clone, Copy)]
struct ParsedTimelinePeriod<'source> {
    period: TimelineSource<'source>,
    task: TimelineSource<'source>,
    suffix: TimelinePeriodSuffix<'source>,
}

fn parse_keyword_prefix<'source>(
    source: TimelineSource<'source>,
    keyword: &str,
) -> Option<(TimelineSource<'source>, TimelineSource<'source>)> {
    if !starts_with_case_insensitive(source.text, keyword) {
        return None;
    }
    let after_keyword = source.subslice(keyword.len(), source.text.len());
    if after_keyword
        .text
        .starts_with(|ch: char| ch.is_ascii_alphanumeric() || ch == '_')
    {
        return None;
    }
    Some((source.subslice(0, keyword.len()), after_keyword))
}

fn starts_timeline_comment(text: &str) -> bool {
    if text.starts_with('%') && !text.starts_with("%{") {
        return true;
    }

    // The selected Jison lexer also skips [^\}]\%\%[^\n]* before other
    // tokens. Its JavaScript regexp consumes one UTF-16 code unit, so an
    // astral character before %% remains part of a period token.
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|ch| ch != '}' && ch.len_utf16() == 1)
        && chars.as_str().starts_with("%%")
}

fn parse_timeline_header(
    source: TimelineSource<'_>,
) -> Option<(TimelineDirection, TimelineSource<'_>)> {
    let (_, after_keyword) = parse_keyword_prefix(source, "timeline")?;
    let direction_text = after_keyword.text.trim_start_matches([' ', '\t']);
    if direction_text.len() < after_keyword.text.len() {
        let direction_source = after_keyword.subslice(
            after_keyword.text.len() - direction_text.len(),
            after_keyword.text.len(),
        );
        for (keyword, direction) in [
            ("LR", TimelineDirection::LeftToRight),
            ("TD", TimelineDirection::TopDown),
        ] {
            if let Some((_, remainder)) = parse_keyword_prefix(direction_source, keyword) {
                return Some((direction, remainder));
            }
        }
    }
    Some((TimelineDirection::LeftToRight, after_keyword))
}

fn scan_timeline_while(
    source: &str,
    start: usize,
    mut accepts: impl FnMut(char) -> bool,
    control: &OperationControl,
) -> OperationControlResult<usize> {
    for (count, (relative, ch)) in source[start..].char_indices().enumerate() {
        if count.is_multiple_of(128) {
            control.checkpoint()?;
        }
        if !accepts(ch) {
            return Ok(start + relative);
        }
    }
    control.checkpoint()?;
    Ok(source.len())
}

fn parse_space_directive<'source>(
    source: TimelineSource<'source>,
    keyword: &str,
    control: &OperationControl,
) -> OperationControlResult<Option<ParsedSpaceDirective<'source>>> {
    let Some((_, after_keyword)) = parse_keyword_prefix(source, keyword) else {
        return Ok(None);
    };
    let Some(whitespace) = after_keyword.text.chars().next() else {
        return Ok(None);
    };
    if !is_ecmascript_whitespace(whitespace) {
        return Ok(None);
    }
    let value_start = whitespace.len_utf8();
    let value_end = scan_timeline_while(
        after_keyword.text,
        value_start,
        |ch| ch != '\n' && (keyword != "section" || ch != ':'),
        control,
    )?;
    let mut value = after_keyword.subslice(value_start, value_end);
    if (value_end == after_keyword.text.len()
        || after_keyword.text.as_bytes().get(value_end) == Some(&b'\n'))
        && let Some(text) = value.text.strip_suffix('\r')
    {
        value = TimelineSource::new(text, value.start);
    }
    Ok((!value.text.is_empty()).then_some(ParsedSpaceDirective { value }))
}

fn parse_accessibility_delimiter(
    source: TimelineSource<'_>,
    keyword: &str,
    delimiter: u8,
    control: &OperationControl,
) -> OperationControlResult<Option<usize>> {
    if parse_keyword_prefix(source, keyword).is_none() {
        return Ok(None);
    }
    let end = scan_timeline_while(
        source.text,
        keyword.len(),
        is_ecmascript_whitespace,
        control,
    )?;
    Ok((source.text.as_bytes().get(end) == Some(&delimiter)).then_some(end))
}

fn parse_colon_directive<'source>(
    source: TimelineSource<'source>,
    keyword: &str,
    control: &OperationControl,
) -> OperationControlResult<Option<ParsedColonDirective<'source>>> {
    let Some(colon) = parse_accessibility_delimiter(source, keyword, b':', control)? else {
        return Ok(None);
    };
    let value_start =
        scan_timeline_while(source.text, colon + 1, is_ecmascript_whitespace, control)?;
    let value_end = scan_timeline_while(source.text, value_start, |ch| ch != '\n', control)?;
    Ok(Some(ParsedColonDirective {
        value: source.subslice(value_start, value_end).trim(),
    }))
}

fn parse_acc_descr_block<'source>(
    cursor: &mut LineCursor<'source>,
    start: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<ParsedAccDescrBlock<'source>>> {
    control.checkpoint()?;
    let source = TimelineSource::new(&cursor.source()[start..], start);
    let Some(open) = parse_accessibility_delimiter(source, "accDescr", b'{', control)? else {
        return Ok(None);
    };
    let content_start = open + 1;
    let content_end = scan_timeline_while(source.text, content_start, |ch| ch != '}', control)?;
    let closing = (content_end < source.text.len()).then_some(SourceSpan::new(
        start + content_end,
        start + content_end + 1,
    ));
    cursor.resume_at(closing.map_or(source.end(), |span| span.end));
    Ok(Some(ParsedAccDescrBlock {
        keyword: SourceSpan::new(start, start + "accDescr".len()),
        closing,
        value: source.subslice(content_start, content_end).trim(),
    }))
}

fn discard_timeline_event_line(
    cursor: &mut LineCursor<'_>,
    source: TimelineSource<'_>,
    control: &OperationControl,
) -> OperationControlResult<()> {
    let end = scan_timeline_while(source.text, 0, |ch| ch != '\n', control)?;
    cursor.resume_at(source.start + end);
    Ok(())
}

fn parse_timeline_events<'source>(
    input: TimelineSource<'source>,
    code: &'source str,
    cursor: &mut LineCursor<'source>,
    diagram_type: &str,
    control: &OperationControl,
) -> OperationControlResult<ParsedTimelineEvents<'source>> {
    let original = input;
    let mut source = TimelineSource::new(&code[input.start..], input.start);
    let mut events = Vec::new();

    while !source.text.is_empty() {
        control.checkpoint()?;
        if starts_timeline_comment(source.text) {
            break;
        }
        if !source.text.starts_with(':') {
            discard_timeline_event_line(cursor, source, control)?;
            return Ok(ParsedTimelineEvents {
                events,
                issue: Some(timeline_parse_issue(
                    Error::diagram_parse_exact(
                        diagram_type.to_string(),
                        format!("invalid event token: {}", original.text),
                        source.span(),
                    ),
                    source.span(),
                )),
            });
        }
        let after_colon = source.subslice(1, source.text.len());
        let Some(whitespace) = after_colon
            .text
            .chars()
            .next()
            .filter(|ch| is_ecmascript_whitespace(*ch))
        else {
            discard_timeline_event_line(cursor, source, control)?;
            let insertion = SourceSpan::new(after_colon.start, after_colon.start);
            return Ok(ParsedTimelineEvents {
                events,
                issue: Some(timeline_parse_issue(
                    Error::diagram_parse_insertion_point(
                        diagram_type.to_string(),
                        "invalid event token: missing whitespace after ':'",
                        after_colon.start,
                    ),
                    insertion,
                )),
            });
        };
        let separator_len = if after_colon.text.starts_with("\r\n") {
            2
        } else {
            whitespace.len_utf8()
        };
        let body = after_colon.subslice(separator_len, after_colon.text.len());
        let mut boundary = body.text.len();
        let mut continues = false;
        for (count, (offset, ch)) in body.text.char_indices().enumerate() {
            if count.is_multiple_of(128) {
                control.checkpoint()?;
            }
            if ch == '\n' {
                boundary = offset;
                break;
            }
            if ch == ':'
                && body.text[offset + 1..]
                    .chars()
                    .next()
                    .is_some_and(is_ecmascript_whitespace)
            {
                boundary = offset;
                continues = true;
                break;
            }
        }
        let mut value = body.subslice(0, boundary);
        if !continues && let Some(text) = value.text.strip_suffix('\r') {
            value = TimelineSource::new(text, value.start);
        }
        if value.text.is_empty() {
            discard_timeline_event_line(cursor, source, control)?;
            let (insertion, message) = if matches!(whitespace, '\r' | '\n') {
                (
                    after_colon.start,
                    "invalid event token: missing whitespace after ':'",
                )
            } else {
                (value.start, "invalid event token: expected event text")
            };
            return Ok(ParsedTimelineEvents {
                events,
                issue: Some(timeline_parse_issue(
                    Error::diagram_parse_insertion_point(
                        diagram_type.to_string(),
                        message,
                        insertion,
                    ),
                    SourceSpan::new(insertion, insertion),
                )),
            });
        }
        cursor.resume_at(value.end());
        events.push(ParsedTimelineEvent { value });
        if !continues {
            break;
        }
        source = body.subslice(boundary, body.text.len());
    }

    Ok(ParsedTimelineEvents {
        events,
        issue: None,
    })
}

fn parse_timeline_period(line: TimelineSource<'_>) -> ParsedTimelinePeriod<'_> {
    let boundary = line
        .text
        .char_indices()
        .find(|(_, ch)| matches!(ch, ':' | '#'));
    let (period, suffix) = match boundary {
        Some((offset, ':')) => (
            line.subslice(0, offset),
            TimelinePeriodSuffix::Events(line.subslice(offset, line.text.len())),
        ),
        Some((offset, '#')) => (line.subslice(0, offset), TimelinePeriodSuffix::HashComment),
        Some(_) => unreachable!("Timeline period boundary is closed"),
        None => (line, TimelinePeriodSuffix::None),
    };
    ParsedTimelinePeriod {
        period,
        task: period.trim(),
        suffix,
    }
}

#[cfg(feature = "editor-facts")]
fn push_timeline_payload_fact(
    demand: EditorFactsDemand,
    facts: &mut Option<EditorSemanticFacts>,
    text: &str,
    span: SourceSpan,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    if !demand.captures_rich() {
        return;
    }
    let Some(facts) = facts else {
        return;
    };
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        span,
    ));
    if text.is_empty() {
        return;
    }
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.to_string(),
        Some(detail.to_string()),
        kind,
        span,
        span,
    ));
}

#[cfg(feature = "editor-facts")]
fn push_timeline_outline_fact(
    demand: EditorFactsDemand,
    facts: &mut Option<EditorSemanticFacts>,
    text: &str,
    statement_span: SourceSpan,
    selection: SourceSpan,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    if !demand.captures_rich() {
        return;
    }
    let Some(facts) = facts else {
        return;
    };
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        selection,
    ));
    if text.is_empty() {
        return;
    }
    facts.push_symbol(EditorSemanticSymbol::outline(
        text.to_string(),
        Some(detail.to_string()),
        kind,
        statement_span,
        selection,
    ));
}

fn timeline_parse_issue(error: Error, fallback: SourceSpan) -> TimelineParseIssue {
    let _span = match &error {
        Error::DiagramParse { diagnostic, .. } => diagnostic.span().unwrap_or(fallback),
        _ => fallback,
    };
    TimelineParseIssue {
        error,
        #[cfg(feature = "editor-facts")]
        span: _span,
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_timeline(code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    let source =
        construct_timeline_semantic_source(code, &meta.diagram_type).into_strict_source()?;
    source.into_compat_json(meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_timeline_json_and_editor_facts(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    Ok(construct_timeline_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?
    .into_combined(meta))
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &TimelineDiagramModel,
    output_data: TimelineOutputData,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    Ok(merman_model::families::timeline::compat::project(
        model,
        output_data,
        &meta.diagram_type,
    ))
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_timeline_model_for_render(
    code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<(TimelineDiagramModel, TimelineOutputData)> {
    construct_timeline_semantic_source(code, &meta.diagram_type)
        .into_strict_source()
        .map(TimelineSemanticSource::into_render_parts)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_timeline_model_for_render_controlled(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<(TimelineDiagramModel, TimelineOutputData)>> {
    let source = construct_timeline_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?
    .into_strict_source();
    let source = match source {
        Ok(source) => source,
        Err(error) => return Ok(Err(error)),
    };
    control.checkpoint()?;
    Ok(Ok(source.into_render_parts()))
}

pub(crate) fn parse_timeline_semantic_model_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<TimelineDiagramModel>> {
    let construction = construct_timeline_semantic_source_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?
    .into_semantic_construction(
        #[cfg(feature = "editor-facts")]
        editor_demand,
    );
    Ok(construction.map_model(|mut source| {
        sanitize_common_db_fields(&mut source.model, config);
        source.model
    }))
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn construct_timeline_semantic_source(code: &str, diagram_type: &str) -> TimelineParseOutcome {
    construct_timeline_semantic_source_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

fn construct_timeline_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<TimelineParseOutcome> {
    control.checkpoint()?;
    #[cfg(test)]
    TIMELINE_SYNTAX_CONSTRUCTION_COUNT.set(TIMELINE_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    parse_timeline_semantic_source(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )
}

fn parse_timeline_semantic_source(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<TimelineParseOutcome> {
    control.checkpoint()?;
    let mut db = TimelineDb::default();
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = editor_demand
        .needs_recovery()
        .then(EditorSemanticFacts::new);
    let mut issues = Vec::new();
    let mut lines = LineCursor::new(code);
    let mut header_seen = false;

    while let Some((line, line_start)) = lines.next_line() {
        control.checkpoint()?;
        let line = TimelineSource::new(line, line_start);
        let significant = line.trim();
        if significant.text.is_empty() {
            continue;
        }

        if starts_timeline_comment(significant.text) {
            continue;
        }
        if significant.text.starts_with('#') {
            continue;
        }

        if !header_seen {
            let Some((direction, remainder)) = parse_timeline_header(significant) else {
                issues.push(timeline_parse_issue(
                    Error::diagram_parse_exact(
                        diagram_type.to_string(),
                        "expected timeline header",
                        significant.span(),
                    ),
                    significant.span(),
                ));
                continue;
            };

            header_seen = true;
            db.direction = direction;
            if !remainder.text.is_empty() {
                lines.resume_same_line_at(remainder.start);
            }
            continue;
        }

        let statement = line.trim_start();
        // Header tokens retain their priority over periods, but are only valid once.
        if let Some((_, remainder)) = parse_timeline_header(statement) {
            let span = SourceSpan::new(statement.start, remainder.start);
            issues.push(timeline_parse_issue(
                Error::diagram_parse_exact(
                    diagram_type.to_string(),
                    "unexpected timeline header",
                    span,
                ),
                span,
            ));
            continue;
        }
        let token_source = TimelineSource::new(&code[statement.start..], statement.start);
        if let Some(parsed) = parse_space_directive(token_source, "title", control)? {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("title");
            }
            #[cfg(feature = "editor-facts")]
            push_timeline_payload_fact(
                editor_demand,
                &mut editor_facts,
                parsed.value.text,
                parsed.value.span(),
                "timeline title",
                EditorSemanticKind::String,
            );
            db.title = parsed.value.text.to_string();
            lines.resume_at(parsed.value.end());
            continue;
        }

        if let Some(parsed) = parse_colon_directive(token_source, "accTitle", control)? {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("accTitle");
            }
            #[cfg(feature = "editor-facts")]
            push_timeline_payload_fact(
                editor_demand,
                &mut editor_facts,
                parsed.value.text,
                parsed.value.span(),
                "timeline accessibility title",
                EditorSemanticKind::String,
            );
            if parsed.value.text.is_empty() {
                issues.push(timeline_parse_issue(
                    Error::diagram_parse_insertion_point(
                        diagram_type.to_string(),
                        "expected accTitle value",
                        parsed.value.start,
                    ),
                    parsed.value.span(),
                ));
            } else {
                db.acc_title = parsed.value.text.to_string();
            }
            lines.resume_at(parsed.value.end());
            continue;
        }

        if let Some(parsed) = parse_colon_directive(token_source, "accDescr", control)? {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("accDescr");
            }
            #[cfg(feature = "editor-facts")]
            push_timeline_payload_fact(
                editor_demand,
                &mut editor_facts,
                parsed.value.text,
                parsed.value.span(),
                "timeline accessibility description",
                EditorSemanticKind::String,
            );
            if parsed.value.text.is_empty() {
                issues.push(timeline_parse_issue(
                    Error::diagram_parse_insertion_point(
                        diagram_type.to_string(),
                        "expected accDescr value",
                        parsed.value.start,
                    ),
                    parsed.value.span(),
                ));
            } else {
                db.acc_descr = parsed.value.text.to_string();
            }
            lines.resume_at(parsed.value.end());
            continue;
        }

        if let Some(parsed) = parse_acc_descr_block(&mut lines, statement.start, control)? {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("accDescr");
            }
            #[cfg(feature = "editor-facts")]
            push_timeline_payload_fact(
                editor_demand,
                &mut editor_facts,
                parsed.value.text,
                parsed.value.span(),
                "timeline accessibility description",
                EditorSemanticKind::String,
            );
            if parsed.closing.is_none() {
                let span = SourceSpan::new(parsed.keyword.start, code.len());
                issues.push(timeline_parse_issue(
                    Error::diagram_parse_insertion_point(
                        diagram_type.to_string(),
                        "unterminated accDescr block",
                        code.len(),
                    ),
                    span,
                ));
            }
            if !parsed.value.text.is_empty() {
                db.acc_descr = parsed.value.text.to_string();
            }
            continue;
        }

        if let Some(parsed) = parse_space_directive(token_source, "section", control)? {
            #[cfg(feature = "editor-facts")]
            let value = parsed.value.trim();
            #[cfg(feature = "editor-facts")]
            push_timeline_outline_fact(
                editor_demand,
                &mut editor_facts,
                value.text,
                SourceSpan::new(statement.start, parsed.value.end()),
                value.span(),
                "timeline section",
                EditorSemanticKind::Namespace,
            );
            db.add_section(parsed.value.text);
            lines.resume_at(parsed.value.end());
            continue;
        }

        if statement.text.starts_with(':') {
            let parsed = parse_timeline_events(statement, code, &mut lines, diagram_type, control)?;
            for event in parsed.events {
                match db.add_event(event.value.text) {
                    Ok(()) => {
                        #[cfg(feature = "editor-facts")]
                        push_timeline_payload_fact(
                            editor_demand,
                            &mut editor_facts,
                            event.value.text,
                            event.value.span(),
                            "timeline event",
                            EditorSemanticKind::String,
                        );
                    }
                    Err(error) => issues.push(timeline_parse_issue(error, event.value.span())),
                }
            }
            if let Some(issue) = parsed.issue {
                issues.push(issue);
            }
            continue;
        }

        let parsed = parse_timeline_period(statement);
        if parsed.task.text.is_empty() {
            continue;
        }

        #[cfg(feature = "editor-facts")]
        push_timeline_outline_fact(
            editor_demand,
            &mut editor_facts,
            parsed.task.text,
            statement.span(),
            parsed.task.span(),
            "timeline task",
            EditorSemanticKind::Event,
        );
        db.add_task(parsed.period.text);

        match parsed.suffix {
            TimelinePeriodSuffix::None => {}
            TimelinePeriodSuffix::HashComment => {}
            TimelinePeriodSuffix::Events(events_source) => {
                let events =
                    parse_timeline_events(events_source, code, &mut lines, diagram_type, control)?;
                for event in events.events {
                    match db.add_event(event.value.text) {
                        Ok(()) => {
                            #[cfg(feature = "editor-facts")]
                            push_timeline_payload_fact(
                                editor_demand,
                                &mut editor_facts,
                                event.value.text,
                                event.value.span(),
                                "timeline event",
                                EditorSemanticKind::String,
                            );
                        }
                        Err(error) => issues.push(timeline_parse_issue(error, event.value.span())),
                    }
                }
                if let Some(issue) = events.issue {
                    issues.push(issue);
                }
            }
        }
    }

    control.checkpoint()?;
    let model = TimelineDiagramModel {
        direction: db.direction,
        title: (!db.title.is_empty()).then_some(db.title),
        acc_title: (!db.acc_title.is_empty()).then_some(db.acc_title),
        acc_descr: (!db.acc_descr.is_empty()).then_some(db.acc_descr),
        sections: db.sections,
        tasks: db.tasks,
    };
    Ok(TimelineParseOutcome {
        source: TimelineSemanticSource {
            model,
            #[cfg(feature = "presentation")]
            output_data: if header_seen {
                TimelineOutputData::Model
            } else {
                TimelineOutputData::Empty
            },
            #[cfg(feature = "editor-facts")]
            editor_facts,
        },
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_multiline_title_and_section_consume_one_separator_and_keep_payloads() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n", "\r"] {
            for payload in [
                "Value;#literal",
                "  Value ",
                "\u{feff}Value\u{feff} ",
                "\u{85}Value",
            ] {
                let source = format!(
                    "timeline{ending}title{ending}{payload}: literal{ending}section{ending}{payload}{ending}P : Event{ending}"
                );
                let parsed = parser.parse_diagram_sync(&source).unwrap();
                let merman_model::semantic::SemanticDiagramModel::Timeline(model) = parsed.model
                else {
                    panic!("expected Timeline model");
                };
                let title = format!("{payload}: literal");
                assert_eq!(model.title.as_deref(), Some(title.as_str()));
                assert_eq!(model.sections, [payload]);
                assert_eq!(model.tasks.len(), 1);
                assert_eq!(model.tasks[0].section, payload);
                assert_eq!(model.tasks[0].events, ["Event"]);
            }
        }
    }

    #[test]
    fn timeline_nonmatching_multiline_keyword_tokens_remain_periods() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for keyword in ["title", "section"] {
                for (separator, period_tail) in [
                    (format!("{ending}{ending}"), ""),
                    (format!(" {ending}"), " "),
                    (format!("\t{ending}"), "\t"),
                    (format!("\u{feff}{ending}"), "\u{feff}"),
                ] {
                    let source = format!(
                        "timeline{ending}{keyword}{separator}Value{ending}P : Event{ending}"
                    );
                    let parsed = parser.parse_diagram_sync(&source).unwrap();
                    let merman_model::semantic::SemanticDiagramModel::Timeline(model) =
                        parsed.model
                    else {
                        panic!("expected Timeline model");
                    };
                    assert!(model.title.is_none(), "{source:?}");
                    assert!(model.sections.is_empty(), "{source:?}");
                    assert_eq!(model.tasks.len(), 3, "{source:?}");
                    assert_eq!(model.tasks[0].task, format!("{keyword}{period_tail}"));
                    assert_eq!(model.tasks[1].task, "Value");
                    assert_eq!(model.tasks[2].events, ["Event"]);
                }
                for tail in [
                    "".to_string(),
                    ending.to_string(),
                    format!("{ending}{ending}"),
                ] {
                    let source = format!("timeline{ending}{keyword}{tail}");
                    let parsed = parser.parse_diagram_sync(&source).unwrap();
                    let merman_model::semantic::SemanticDiagramModel::Timeline(model) =
                        parsed.model
                    else {
                        panic!("expected Timeline model");
                    };
                    assert!(model.title.is_none());
                    assert!(model.sections.is_empty());
                    assert_eq!(model.tasks.len(), 1);
                    assert_eq!(model.tasks[0].task, keyword);
                }
            }
        }
    }

    #[test]
    fn timeline_raw_keyword_scan_preserves_the_one_character_boundary() {
        let control = OperationControl::new();
        for source in ["title\r\nValue", "title \nValue", "title\n\nValue", "title"] {
            assert!(
                parse_space_directive(TimelineSource::new(source, 0), "title", &control)
                    .unwrap()
                    .is_none(),
                "{source:?}"
            );
        }
        let title_source = "title\nValue:literal;#retained";
        let title = parse_space_directive(TimelineSource::new(title_source, 7), "title", &control)
            .unwrap()
            .unwrap();
        assert_eq!(title.value.text, "Value:literal;#retained");
        assert_eq!(
            title.value.span(),
            SourceSpan::new(13, 7 + title_source.len())
        );

        let section_source = "section\nValue: Event";
        let section =
            parse_space_directive(TimelineSource::new(section_source, 0), "section", &control)
                .unwrap()
                .unwrap();
        assert_eq!(section.value.text, "Value");
        assert_eq!(section.value.end(), "section\nValue".len());
    }

    #[test]
    fn timeline_multiline_accessibility_owns_whitespace_and_complete_scalar_lines() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "timeline{ending}",
                    "accTitle{ending}\u{feff}\t:{ending}{ending}\u{feff}\u{85}Value;#x%%retained\u{85}\u{feff}{ending}",
                    "accDescr\u{feff}{ending}{ending}:{ending}Consumed : Ghost{ending}",
                    "P : Event{ending}",
                ),
                ending = ending,
            );
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Timeline(model) = parsed.model else {
                panic!("expected Timeline model");
            };
            assert_eq!(
                model.acc_title.as_deref(),
                Some("\u{85}Value;#x%%retained\u{85}")
            );
            assert_eq!(model.acc_descr.as_deref(), Some("Consumed : Ghost"));
            assert_eq!(model.tasks.len(), 1);
            assert_eq!(model.tasks[0].task, "P ");
            assert_eq!(model.tasks[0].events, ["Event"]);
        }
    }

    #[test]
    fn timeline_empty_description_blocks_preserve_values_and_resume_source_suffixes() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "timeline{ending}accDescr: Old{ending}",
                    "accDescr{{}}{ending}",
                    "accDescr{ending}\u{feff}{{ \t{ending}\u{feff} }}accTitle{ending}:Value{ending}",
                    "accDescr{{Help;#%%literal}}title{ending}Plan{ending}",
                    "P : Event{ending}",
                ),
                ending = ending,
            );
            let parsed = parser.parse_diagram_sync(&source).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Timeline(model) = parsed.model else {
                panic!("expected Timeline model");
            };
            assert_eq!(model.acc_title.as_deref(), Some("Value"));
            assert_eq!(model.acc_descr.as_deref(), Some("Help;#%%literal"));
            assert_eq!(model.title.as_deref(), Some("Plan"));
            assert_eq!(model.tasks.len(), 1);
            #[cfg(feature = "editor-facts")]
            {
                let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                    parser.capture_diagram_snapshot_sync(&source)
                else {
                    panic!("expected Timeline snapshot");
                };
                let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("expected Timeline editor facts");
                };
                let descriptions = facts
                    .symbols
                    .iter()
                    .filter(|symbol| {
                        symbol.detail.as_deref() == Some("timeline accessibility description")
                    })
                    .map(|symbol| symbol.name.as_str())
                    .collect::<Vec<_>>();
                assert_eq!(descriptions, ["Old", "Help;#%%literal"]);
                assert!(facts.diagnostics.is_empty());
            }
            let retained = source.replace("accDescr{Help;#%%literal}", "accDescr{}");
            let parsed = parser.parse_diagram_sync(&retained).unwrap();
            let merman_model::semantic::SemanticDiagramModel::Timeline(model) = parsed.model else {
                panic!("expected Timeline model");
            };
            assert_eq!(model.acc_descr.as_deref(), Some("Old"));
            for (body, expected) in [("\u{feff}", "Old"), ("\u{85}", "\u{85}")] {
                let source = format!(
                    "timeline{ending}accDescr: Old{ending}accDescr{{{body}}}{ending}P : Event{ending}"
                );
                let parsed = parser.parse_diagram_sync(&source).unwrap();
                let merman_model::semantic::SemanticDiagramModel::Timeline(model) = parsed.model
                else {
                    panic!("expected Timeline model");
                };
                assert_eq!(model.acc_descr.as_deref(), Some(expected));
            }
        }
    }

    #[test]
    fn timeline_missing_accessibility_values_reject_at_eof_and_keep_recovery_facts() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for suffix in [
                "accTitle:".to_string(),
                format!("accTitle:{ending}"),
                format!("accTitle{ending}:\u{feff}{ending}"),
                "accDescr:".to_string(),
                format!("accDescr:{ending}"),
                "accDescr{".to_string(),
                format!("accDescr{ending}{{Help"),
                format!("accDescr{ending}{{ \u{feff}{ending}"),
            ] {
                let source = format!("timeline{ending}title Plan{ending}{suffix}");
                assert!(parser.parse_diagram_sync(&source).is_err(), "{source:?}");
                #[cfg(feature = "editor-facts")]
                {
                    let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                        parser.capture_diagram_snapshot_sync(&source)
                    else {
                        panic!("expected Timeline snapshot");
                    };
                    let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                        panic!("expected Timeline editor facts");
                    };
                    assert_eq!(
                        facts.completeness,
                        crate::EditorSemanticCompleteness::Recovered
                    );
                    assert_eq!(facts.diagnostics.len(), 1, "{source:?}");
                    assert_eq!(
                        facts.diagnostics[0].span,
                        Some(SourceSpan::new(source.len(), source.len()))
                    );
                    let title = facts
                        .symbols
                        .iter()
                        .find(|symbol| symbol.detail.as_deref() == Some("timeline title"))
                        .unwrap();
                    assert_eq!(title.name, "Plan");
                    assert_eq!(&source[title.selection.start..title.selection.end], "Plan");
                    if suffix.starts_with("accTitle") || suffix.contains(':') {
                        assert!(facts.expected_syntax.iter().any(|expected| {
                            expected.kind == EditorExpectedSyntaxKind::Payload
                                && expected.span == SourceSpan::new(source.len(), source.len())
                        }));
                    }
                }
            }
        }
    }

    #[test]
    fn timeline_event_one_separator_preserves_values_and_rejects_extra_newlines() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for payload in [
                "Event",
                "  Event ",
                "\u{feff}Event\u{feff}",
                "\u{85}Event",
                "x%%keep",
                "title Value",
                "# Value",
                "section Value",
                "A:B ",
            ] {
                let source =
                    format!("timeline{ending}P :{ending}{payload}: Follow{ending}Q : Next{ending}");
                let parsed = parser.parse_diagram_sync(&source).unwrap();
                let merman_model::semantic::SemanticDiagramModel::Timeline(model) = parsed.model
                else {
                    panic!("expected Timeline model");
                };
                assert_eq!(model.tasks.len(), 2);
                assert_eq!(model.tasks[0].events, [payload, "Follow"]);
                assert_eq!(model.tasks[1].events, ["Next"]);
            }
            for separator in [
                format!("{ending}{ending}"),
                format!(" {ending}"),
                format!("\u{feff}{ending}"),
            ] {
                let source = format!("timeline{ending}P :{separator}Event{ending}Q : Next{ending}");
                assert!(parser.parse_diagram_sync(&source).is_err(), "{source:?}");
                #[cfg(feature = "editor-facts")]
                {
                    let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                        parser.capture_diagram_snapshot_sync(&source)
                    else {
                        panic!("expected Timeline snapshot");
                    };
                    let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                        panic!("expected Timeline editor facts");
                    };
                    assert_eq!(facts.diagnostics.len(), 1, "{source:?}");
                    assert!(facts.symbols.iter().any(|symbol| symbol.name == "Q"));
                    assert!(facts.symbols.iter().any(|symbol| symbol.name == "Next"));
                }
            }
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn timeline_multiline_source_tokens_keep_original_unicode_editor_ranges() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "timeline{ending}",
                    "accDescr{ending}\u{feff}{{{ending}Help \u{1f913}{ending}Next{ending}}}title{ending}Plan;#literal{ending}",
                    "P : Seed{ending}",
                    "  section{ending}\u{feff}Delivery \u{1f913}\u{feff} :{ending}Extra \u{1f913}: Follow{ending}",
                    "accTitle{ending}%% Removed{ending}\u{feff}:{ending}\u{85}Accessible\u{85}{ending}",
                    "Q : Ready{ending}",
                ),
                ending = ending,
            );
            let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                parser.capture_diagram_snapshot_sync(&source)
            else {
                panic!("expected Timeline snapshot");
            };
            let (_, outcome, editor_facts) = snapshot.into_parts();
            let crate::SemanticParseOutcome::Parsed(
                merman_model::semantic::SemanticDiagramModel::Timeline(model),
            ) = outcome
            else {
                panic!("expected Timeline model");
            };
            assert_eq!(model.acc_descr.as_deref(), Some("Help \u{1f913}\nNext"));
            assert_eq!(model.sections, ["\u{feff}Delivery \u{1f913}\u{feff} "]);
            assert_eq!(model.tasks.len(), 2);
            assert_eq!(model.tasks[0].events, ["Seed", "Extra \u{1f913}", "Follow"]);
            assert_eq!(model.tasks[1].section_index, Some(0));
            let crate::ParsedEditorFacts::Available(facts) = editor_facts else {
                panic!("expected Timeline editor facts");
            };
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Complete
            );
            for (detail, name) in [
                ("timeline title", "Plan;#literal"),
                ("timeline section", "Delivery \u{1f913}"),
                ("timeline accessibility title", "\u{85}Accessible\u{85}"),
                ("timeline event", "Extra \u{1f913}"),
            ] {
                let symbol = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.detail.as_deref() == Some(detail) && symbol.name == name)
                    .unwrap();
                let start = source.find(name).unwrap();
                assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
                assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
                assert!(facts.expected_syntax.iter().any(|expected| {
                    expected.kind == EditorExpectedSyntaxKind::Payload
                        && expected.span == symbol.selection
                }));
                if detail == "timeline section" {
                    assert_eq!(symbol.span.start, source.find("section").unwrap());
                    assert_eq!(
                        &source[symbol.span.start..symbol.selection.start],
                        format!("section{ending}\u{feff}")
                    );
                }
            }
            let block = facts
                .symbols
                .iter()
                .find(|symbol| {
                    symbol.detail.as_deref() == Some("timeline accessibility description")
                })
                .unwrap();
            assert_eq!(block.name, "Help \u{1f913}\nNext");
            assert_eq!(
                &source[block.selection.start..block.selection.end],
                format!("Help \u{1f913}{ending}Next")
            );
        }
    }

    #[test]
    fn timeline_multiline_token_scans_observe_cancellation() {
        for (source, keyword) in [
            (format!("title\n{}", "\u{754c}".repeat(4096)), "title"),
            (format!("section\n{}", "\u{754c}".repeat(4096)), "section"),
        ] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(2);
            assert!(
                parse_space_directive(TimelineSource::new(&source, 0), keyword, &control).is_err()
            );
        }
        let scalar = format!("accTitle{}:Value", "\u{feff}\n".repeat(2048));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(
            parse_colon_directive(TimelineSource::new(&scalar, 0), "accTitle", &control).is_err()
        );

        let block = format!("accDescr{{{}}}", "\u{85}".repeat(4096));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        assert!(parse_acc_descr_block(&mut LineCursor::new(&block), 0, &control).is_err());

        let event = format!(":\n{}", "\u{754c}".repeat(4096));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(
            parse_timeline_events(
                TimelineSource::new(&event, 0),
                &event,
                &mut LineCursor::new(&event),
                "timeline",
                &control
            )
            .is_err()
        );
    }

    #[test]
    fn timeline_malformed_tokens_keep_recovery_and_checkpoint_work_linear() {
        for count in [128, 2048] {
            for closed in [false, true] {
                let source = format!(
                    "timeline\nP : Seed\naccDescr{{{}{}Q : Ready\n",
                    "accDescr{\n".repeat(count),
                    if closed { "}accTitle\n:Value\n" } else { "" },
                );
                for capture in [false, true] {
                    let control = OperationControl::new();
                    control.cancel_after_checkpoints(source.chars().count().div_ceil(128) + 64);
                    let parsed = parse_timeline_semantic_model_controlled(
                        &source,
                        "timeline",
                        &crate::MermaidConfig::default(),
                        #[cfg(feature = "editor-facts")]
                        if capture {
                            EditorFactsDemand::Capture
                        } else {
                            EditorFactsDemand::Omit
                        },
                        &control,
                    )
                    .expect("description state must consume its input once");
                    assert_eq!(parsed.model.is_ok(), closed);
                    #[cfg(feature = "editor-facts")]
                    if capture {
                        let facts = parsed.editor_facts.unwrap();
                        assert_eq!(
                            facts.symbols.iter().any(|symbol| symbol.name == "Q"),
                            closed
                        );
                    }
                    #[cfg(not(feature = "editor-facts"))]
                    let _ = capture;
                }
            }
            for (malformed, event_count) in [(":bad\n", 1), (":\nKept: \n", count + 1)] {
                let source = format!("timeline\nP : Seed\n{}Q : Ready\n", malformed.repeat(count));
                let control = OperationControl::new();
                control.cancel_after_checkpoints(16 * count + 64);
                let outcome = construct_timeline_semantic_source_controlled(
                    &source,
                    "timeline",
                    #[cfg(feature = "editor-facts")]
                    EditorFactsDemand::Capture,
                    &control,
                )
                .expect("malformed event lines must not replay suffixes");
                assert_eq!(outcome.issues.len(), count);
                assert_eq!(outcome.source.model.tasks.len(), 2);
                assert_eq!(outcome.source.model.tasks[0].events.len(), event_count);
                assert_eq!(outcome.source.model.tasks[1].events, ["Ready"]);
            }
        }
    }

    #[test]
    fn timeline_boundary_comments_preserve_the_previous_task_and_section() {
        for comment in [
            "%% hidden",
            "x%% hidden",
            "  x%% hidden",
            "中%% hidden",
            ":%% hidden",
        ] {
            let source = format!(
                "timeline\r\nsection A\r\nP : Seed\r\n{comment}\r\nsection B:%% hidden\r\n: Follow\r\nNext : Ready\r\n"
            );
            let model = parse_timeline_semantic_model_controlled(
                &source,
                "timeline",
                &crate::MermaidConfig::default(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .expect("boundary comments do not create a task or an invalid event");

            assert_eq!(model.sections, ["A", "B"]);
            assert_eq!(model.tasks.len(), 2, "{comment:?}");
            assert_eq!(model.tasks[0].task, "P ");
            assert_eq!(model.tasks[0].section, "A");
            assert_eq!(model.tasks[0].events, ["Seed", "Follow"]);
            assert_eq!(model.tasks[1].task, "Next ");
            assert_eq!(model.tasks[1].section, "B");
            assert_eq!(model.tasks[1].events, ["Ready"]);
        }
    }

    #[test]
    fn timeline_boundary_comments_follow_headers_and_directions() {
        for (header, direction) in [
            ("timeline x%% hidden", TimelineDirection::LeftToRight),
            ("timeline:%% hidden", TimelineDirection::LeftToRight),
            ("timeline LR x%% hidden", TimelineDirection::LeftToRight),
            ("timeline TD x%% hidden", TimelineDirection::TopDown),
            ("timeline TD\u{0085}%% hidden", TimelineDirection::TopDown),
        ] {
            let source = format!("{header}\nP : Seed\n");
            let model = parse_timeline_semantic_model_controlled(
                &source,
                "timeline",
                &crate::MermaidConfig::default(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .expect("the normal comment lexer resumes after the header token");

            assert_eq!(model.direction, direction);
            assert_eq!(model.tasks.len(), 1);
            assert_eq!(model.tasks[0].events, ["Seed"]);
        }
    }

    #[test]
    fn timeline_plain_header_resumes_normal_period_tokens() {
        for (header, first_task, first_event) in [
            ("timeline\u{feff}TD", "TD", None),
            ("timeline LRfoo", "LRfoo", None),
            ("timeline P : Seed", "P ", Some("Seed")),
        ] {
            let source = format!("{header}\nNext : Ready\n");
            let model = parse_timeline_semantic_model_controlled(
                &source,
                "timeline",
                &crate::MermaidConfig::default(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .expect("the plain header leaves its remaining tokens in the normal state");

            assert_eq!(model.direction, TimelineDirection::LeftToRight);
            assert_eq!(model.tasks.len(), 2);
            assert_eq!(model.tasks[0].task, first_task);
            assert_eq!(
                model.tasks[0].events,
                first_event.into_iter().collect::<Vec<_>>()
            );
            assert_eq!(model.tasks[1].events, ["Ready"]);
        }
    }

    #[test]
    fn timeline_boundary_comments_require_a_complete_header_keyword() {
        for header in [
            "timelinex%% hidden",
            "timeline_%% hidden",
            "timeline1%% hidden",
        ] {
            let source = format!("{header}\nP : Seed\n");
            let parsed = parse_timeline_semantic_model_controlled(
                &source,
                "timeline",
                &crate::MermaidConfig::default(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            let Error::DiagramParse { diagnostic, .. } = parsed.model.unwrap_err() else {
                panic!("the header keyword must end at a JavaScript word boundary");
            };
            assert_eq!(diagnostic.message(), "expected timeline header");
            assert_eq!(diagnostic.span(), Some(SourceSpan::new(0, header.len())));
            #[cfg(feature = "editor-facts")]
            {
                let facts = parsed.editor_facts.unwrap();
                assert_eq!(
                    facts.completeness,
                    crate::EditorSemanticCompleteness::Recovered
                );
                assert_eq!(facts.diagnostics[0].span, diagnostic.span());
            }
        }
    }

    #[test]
    fn timeline_repeated_headers_remain_invalid_after_resuming_tokens() {
        for (source, header) in [
            ("timeline timeline\nP : Seed\n", "timeline"),
            ("timeline TD timeline LR\nP : Seed\n", "timeline LR"),
            ("timeline\n  timeline\nP : Seed\n", "timeline"),
        ] {
            let parsed = parse_timeline_semantic_model_controlled(
                source,
                "timeline",
                &crate::MermaidConfig::default(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            let Error::DiagramParse { diagnostic, .. } = parsed.model.unwrap_err() else {
                panic!("a second header token is not a period statement");
            };
            let start = source.rfind(header).unwrap();
            assert_eq!(diagnostic.message(), "unexpected timeline header");
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(start, start + header.len()))
            );
            #[cfg(feature = "editor-facts")]
            {
                let facts = parsed.editor_facts.unwrap();
                assert_eq!(
                    facts.completeness,
                    crate::EditorSemanticCompleteness::Recovered
                );
                assert_eq!(facts.diagnostics.len(), 1);
                assert_eq!(facts.diagnostics[0].span, diagnostic.span());
                assert!(facts.symbols.iter().any(|symbol| symbol.name == "P"));
            }
        }
    }

    #[test]
    fn timeline_boundary_comments_use_ecmascript_whitespace() {
        for (prefix, skipped) in [("\u{0085}", false), ("\u{feff}", true), ("\u{00a0}", true)] {
            let source = format!("timeline\nP : Seed\n{prefix}x%% retained\n: Follow\n");
            let parsed = parse_timeline_semantic_model_controlled(
                &source,
                "timeline",
                &crate::MermaidConfig::default(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            let model = parsed.model.expect("whitespace follows the selected lexer");
            if skipped {
                assert_eq!(model.tasks.len(), 1);
                assert_eq!(model.tasks[0].events, ["Seed", "Follow"]);
            } else {
                assert_eq!(model.tasks.len(), 2);
                assert_eq!(model.tasks[0].events, ["Seed"]);
                assert_eq!(model.tasks[1].task, "\u{0085}x%% retained");
                assert_eq!(model.tasks[1].events, ["Follow"]);
            }
            #[cfg(feature = "editor-facts")]
            {
                let facts = parsed.editor_facts.unwrap();
                let task = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name.contains("retained"));
                assert_eq!(task.is_none(), skipped);
                if let Some(task) = task {
                    let start = source.find(prefix).unwrap();
                    assert_eq!(task.name, "\u{0085}x%% retained");
                    assert_eq!(
                        task.selection,
                        SourceSpan::new(start, start + task.name.len())
                    );
                }
            }
        }
    }

    #[test]
    fn timeline_boundary_comment_precedes_an_event_token_after_a_period() {
        let model = parse_timeline_semantic_model_controlled(
            "timeline\nP:%% hidden\n: Follow\nNext : Ready\n",
            "timeline",
            &crate::MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .expect("the comment rule resumes after a period token");

        assert_eq!(model.tasks.len(), 2);
        assert_eq!(model.tasks[0].task, "P");
        assert_eq!(model.tasks[0].events, ["Follow"]);
        assert_eq!(model.tasks[1].events, ["Ready"]);
    }

    #[test]
    fn timeline_single_percent_comments_preserve_event_ownership_at_source_boundaries() {
        let parser = crate::SemanticParser::new();
        for ending in ["\n", "\r\n"] {
            for (suffix, skipped) in [
                ("% hidden", true),
                ("%% hidden", true),
                ("%{literal", false),
                ("X%literal", false),
                ("X%%hidden", true),
                ("\u{85}%%hidden", true),
                ("\u{1f680}%%literal", false),
            ] {
                for prefix in ["", "accDescr\n{Help}"] {
                    let source =
                        format!("timeline\nP : Seed\n{prefix}{suffix}\n: Follow\nQ : Next\n")
                            .replace('\n', ending);
                    let parsed = parser.parse_diagram_sync(&source).unwrap();
                    let merman_model::semantic::SemanticDiagramModel::Timeline(model) =
                        parsed.model
                    else {
                        panic!("expected Timeline model");
                    };
                    assert_eq!(model.tasks.len(), if skipped { 2 } else { 3 });
                    let owner = if skipped { 0 } else { 1 };
                    assert_eq!(model.tasks[owner].events.last().unwrap(), "Follow");
                    if !skipped {
                        assert_eq!(model.tasks[1].task, suffix);
                    }
                    #[cfg(feature = "editor-facts")]
                    {
                        let crate::SemanticSnapshotCapture::Snapshot(snapshot) =
                            parser.capture_diagram_snapshot_sync(&source)
                        else {
                            panic!("expected Timeline snapshot");
                        };
                        let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts()
                        else {
                            panic!("expected Timeline editor facts");
                        };
                        assert_eq!(
                            facts.symbols.iter().any(|symbol| symbol.name == suffix),
                            !skipped
                        );
                        let event = facts
                            .symbols
                            .iter()
                            .find(|symbol| symbol.name == "Follow")
                            .unwrap();
                        assert_eq!(
                            &source[event.selection.start..event.selection.end],
                            "Follow"
                        );
                    }
                }
            }
        }
        let parsed = parser
            .parse_diagram_sync("timeline\nP : %literal\n: Follow\n")
            .unwrap();
        let merman_model::semantic::SemanticDiagramModel::Timeline(model) = parsed.model else {
            panic!("expected Timeline model");
        };
        assert_eq!(model.tasks[0].events, ["%literal", "Follow"]);
    }

    #[test]
    fn timeline_percent_pairs_inside_payload_tokens_remain_literal() {
        let source = concat!(
            "timeline\n",
            "title Release %% ready\n",
            "accTitle: x%% retained\n",
            "accDescr {\nx%% retained\n}\n",
            "section Build %% retained\n",
            "P : literal %% kept\n",
            ": x%% also kept\n",
            ": %% kept\n",
            ": Seed :%% kept\n",
            "xx%% retained\n",
            "}%% retained\n",
            "🤓%% retained\n",
        );
        let model = parse_timeline_semantic_model_controlled(
            source,
            "timeline",
            &crate::MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .expect("comment recognition respects lexer token boundaries");

        assert_eq!(model.title.as_deref(), Some("Release %% ready"));
        assert_eq!(model.acc_title.as_deref(), Some("x%% retained"));
        assert_eq!(model.acc_descr.as_deref(), Some("x%% retained"));
        assert_eq!(model.sections, ["Build %% retained"]);
        assert_eq!(model.tasks.len(), 4);
        assert_eq!(
            model.tasks[0].events,
            [
                "literal %% kept",
                "x%% also kept",
                "%% kept",
                "Seed :%% kept"
            ]
        );
        assert_eq!(model.tasks[1].task, "xx%% retained");
        assert_eq!(model.tasks[2].task, "}%% retained");
        assert_eq!(model.tasks[3].task, "🤓%% retained");
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn timeline_boundary_comments_emit_no_payload_facts() {
        let source = concat!(
            "timeline\r\n",
            "section 发布\r\n",
            "P : Seed\r\n",
            "  中%% hidden\r\n",
            "section 后续:%% hidden\r\n",
            ": literal %% kept\r\n",
            "Next : Ready\r\n",
        );
        let parsed = parse_timeline_semantic_model_controlled(
            source,
            "timeline",
            &crate::MermaidConfig::default(),
            EditorFactsDemand::Capture,
            &OperationControl::new(),
        )
        .unwrap();
        assert!(parsed.model.is_ok());
        let facts = parsed.editor_facts.expect("capture requested");
        assert!(
            facts
                .symbols
                .iter()
                .all(|symbol| !symbol.name.contains("hidden"))
        );
        let literal = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "literal %% kept")
            .expect("event payload remains available to editor services");
        let start = source.find("literal %% kept").unwrap();
        assert_eq!(
            literal.selection,
            SourceSpan::new(start, start + "literal %% kept".len())
        );
        for expected in &facts.expected_syntax {
            assert!(!source[expected.span.start..expected.span.end].contains("hidden"));
        }
    }

    #[test]
    fn timeline_accessibility_preserves_complete_single_line_payloads() {
        for (title, description) in [
            ("Release ready", "Ship stable"),
            ("Release; ready", "Ship #stable"),
            ("发布; ready #stable: 🤓", ";Ship; fully #accessible: 稳定"),
        ] {
            let source = format!(
                "timeline\r\naccTitle:  {title}  \r\naccDescr:  {description}  \r\nP : Seed\r\n"
            );
            let model = parse_timeline_semantic_model_controlled(
                &source,
                "timeline",
                &crate::MermaidConfig::default(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .expect("complete single-line accessibility values parse");

            assert_eq!(model.acc_title.as_deref(), Some(title));
            assert_eq!(model.acc_descr.as_deref(), Some(description));
            assert_eq!(model.tasks.len(), 1);
            assert_eq!(model.tasks[0].events, ["Seed"]);
        }
    }

    #[test]
    fn timeline_section_suffix_dispatches_events_like_following_lines() {
        for (section_separator, event_separator) in [
            ("", " "),
            ("\n", " "),
            ("\r\n", " "),
            ("", "\n"),
            ("", "\r\n"),
            ("\n", "\n"),
        ] {
            let source = format!(
                "timeline\nsection A\nP : Seed\nsection B{section_separator}:{event_separator}Extra: https://example.com:8080/a#part;literal %% kept\nNext : Ready\n"
            );
            let model = parse_timeline_semantic_model_controlled(
                &source,
                "timeline",
                &crate::MermaidConfig::default(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .expect("a section can be followed immediately by event tokens");

            assert_eq!(model.sections, ["A", "B"]);
            assert_eq!(model.tasks.len(), 2);
            assert_eq!(model.tasks[0].task, "P ");
            assert_eq!(model.tasks[0].section, "A");
            assert_eq!(model.tasks[0].section_index, Some(0));
            assert_eq!(
                model.tasks[0].events,
                [
                    "Seed",
                    "Extra",
                    "https://example.com:8080/a#part;literal %% kept"
                ]
            );
            assert_eq!(model.tasks[1].section, "B");
            assert_eq!(model.tasks[1].section_index, Some(1));
            assert_eq!(model.tasks[1].events, ["Ready"]);
        }
    }

    #[test]
    fn timeline_task_suffix_and_later_events_share_the_lf_separator() {
        for separator in [" ", "\n"] {
            let source = format!("timeline\nP:{separator}Seed:{separator}Extra\nNext : Ready\n");
            let model = parse_timeline_semantic_model_controlled(
                &source,
                "timeline",
                &crate::MermaidConfig::default(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap()
            .model
            .expect("task suffixes use the same event rule");

            assert_eq!(model.tasks.len(), 2);
            assert_eq!(model.tasks[0].task, "P");
            assert_eq!(model.tasks[0].events, ["Seed", "Extra"]);
            assert_eq!(model.tasks[1].task, "Next ");
            assert_eq!(model.tasks[1].events, ["Ready"]);
        }
    }

    #[test]
    fn timeline_section_token_requires_text_before_the_colon() {
        let model = parse_timeline_semantic_model_controlled(
            "timeline\nsection : Extra\n",
            "timeline",
            &crate::MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .expect("without section text the period rule owns the token");

        assert!(model.sections.is_empty());
        assert_eq!(model.tasks.len(), 1);
        assert_eq!(model.tasks[0].task, "section ");
        assert_eq!(model.tasks[0].section_index, None);
        assert_eq!(model.tasks[0].events, ["Extra"]);
    }

    #[test]
    fn timeline_section_suffix_rejects_invalid_event_tokens() {
        for (suffix, insertion_offset, message) in [
            (
                "section B:bad\n",
                1,
                "invalid event token: missing whitespace after ':'",
            ),
            (
                "section B:",
                1,
                "invalid event token: missing whitespace after ':'",
            ),
            (
                "section B:\n",
                1,
                "invalid event token: missing whitespace after ':'",
            ),
            ("section B: ", 2, "invalid event token: expected event text"),
            (":", 1, "invalid event token: missing whitespace after ':'"),
        ] {
            let source = format!("timeline\nsection A\nP : Seed\n{suffix}");
            let result = parse_timeline_semantic_model_controlled(
                &source,
                "timeline",
                &crate::MermaidConfig::default(),
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Omit,
                &OperationControl::new(),
            )
            .unwrap();
            let Error::DiagramParse { diagnostic, .. } = result.model.unwrap_err() else {
                panic!("expected a structured Timeline suffix error for {suffix:?}");
            };
            let insertion = source.rfind(':').unwrap() + insertion_offset;
            assert_eq!(diagnostic.message(), message, "{suffix:?}");
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(insertion, insertion))
            );
            assert_eq!(
                diagnostic.span_kind(),
                crate::ParseDiagnosticSpanKind::InsertionPoint
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn timeline_lf_section_suffix_keeps_event_ranges_and_task_sections() {
        use crate::{EditorSemanticCompleteness, EditorSemanticRole};

        for separator in [" ", "\n", "\r\n"] {
            let source = format!(
                "timeline\r\nsection 前段\r\n发布 : Seed\r\n  section 后段:{separator}额外 🤓: Follow\r\nNext : Ready\r\n"
            );
            reset_timeline_syntax_construction_count();
            let parsed = parse_timeline_semantic_model_controlled(
                &source,
                "timeline",
                &crate::MermaidConfig::default(),
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(timeline_syntax_construction_count(), 1);
            let model = parsed.model.expect("the event separator can be LF");
            assert_eq!(model.sections, ["前段", "后段"]);
            assert_eq!(model.tasks.len(), 2);
            assert_eq!(model.tasks[0].section, "前段");
            assert_eq!(model.tasks[0].section_index, Some(0));
            assert_eq!(model.tasks[0].events, ["Seed", "额外 🤓", "Follow"]);
            assert_eq!(model.tasks[1].task, "Next ");
            assert_eq!(model.tasks[1].section, "后段");
            assert_eq!(model.tasks[1].section_index, Some(1));
            assert_eq!(model.tasks[1].events, ["Ready"]);

            let facts = parsed.editor_facts.unwrap();
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            for name in ["额外 🤓", "Follow", "Ready"] {
                let start = source.find(name).unwrap();
                let span = SourceSpan::new(start, start + name.len());
                let symbols = facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.name == name)
                    .collect::<Vec<_>>();
                assert_eq!(symbols.len(), 1);
                assert_eq!(symbols[0].role, EditorSemanticRole::Payload);
                assert_eq!(symbols[0].detail.as_deref(), Some("timeline event"));
                assert_eq!(symbols[0].span, span);
                assert_eq!(symbols[0].selection, span);
                assert!(facts.expected_syntax.iter().any(|expected| {
                    expected.kind == EditorExpectedSyntaxKind::Payload && expected.span == span
                }));
            }
            let section = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == "后段")
                .unwrap();
            let start = source.find("section 后段").unwrap();
            assert_eq!(section.role, EditorSemanticRole::Outline);
            assert_eq!(
                section.span,
                SourceSpan::new(start, start + "section 后段".len())
            );
            assert_eq!(
                section.selection,
                SourceSpan::new(start + "section ".len(), start + "section 后段".len())
            );
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn timeline_lf_event_recovery_keeps_the_next_task() {
        use crate::{EditorSemanticCompleteness, EditorSemanticRole};

        for suffix in [":bad\n", ":\n\n", ": \n"] {
            let source = format!("timeline\nsection A\nP : Seed\nsection B{suffix}Next : Ready\n");
            let outcome = construct_timeline_semantic_source_controlled(
                &source,
                "timeline",
                EditorFactsDemand::Capture,
                &OperationControl::new(),
            )
            .unwrap();
            assert_eq!(outcome.issues.len(), 1, "{source}");
            assert_eq!(outcome.source.model.sections, ["A", "B"]);
            let tasks = &outcome.source.model.tasks;
            assert_eq!(tasks.len(), 2);
            assert_eq!(tasks[0].section_index, Some(0));
            assert_eq!(tasks[0].events, ["Seed"]);
            assert_eq!(tasks[1].task, "Next ");
            assert_eq!(tasks[1].section, "B");
            assert_eq!(tasks[1].section_index, Some(1));
            assert_eq!(tasks[1].events, ["Ready"]);

            let recovered = outcome.into_semantic_construction(EditorFactsDemand::Capture);
            assert!(recovered.model.is_err());
            let facts = recovered.editor_facts.unwrap();
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert_eq!(facts.diagnostics.len(), 1);
            for (name, role) in [
                ("Next", EditorSemanticRole::Outline),
                ("Ready", EditorSemanticRole::Payload),
            ] {
                let start = source.find(name).unwrap();
                let span = SourceSpan::new(start, start + name.len());
                assert!(facts.symbols.iter().any(|symbol| {
                    symbol.name == name && symbol.role == role && symbol.selection == span
                }));
            }
        }
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn timeline_accessibility_and_section_suffix_facts_keep_exact_source_ranges() {
        use crate::{EditorSemanticCompleteness, EditorSemanticRole};

        let source = concat!(
            "---\r\nconfig:\r\n  timeline:\r\n    disableMulticolor: false\r\n---\r\n",
            "%% full-line comment\r\n",
            "timeline\r\n",
            "accTitle:  发布; ready #stable 🤓  \r\n",
            "accDescr:  Ship; fully #accessible 稳定  \r\n",
            "section 前段\r\n",
            "发布 : Seed\r\n",
            "  section 后段: 发布; ready #stable %% literal\r\n",
        );
        let facts = crate::MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("timeline", source)
            .unwrap()
            .expect("Timeline facts");

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        for (name, detail) in [
            ("发布; ready #stable 🤓", "timeline accessibility title"),
            (
                "Ship; fully #accessible 稳定",
                "timeline accessibility description",
            ),
            ("发布; ready #stable %% literal", "timeline event"),
        ] {
            let start = source.find(name).unwrap();
            let span = SourceSpan::new(start, start + name.len());
            let symbols = facts
                .symbols
                .iter()
                .filter(|symbol| symbol.detail.as_deref() == Some(detail) && symbol.name == name)
                .collect::<Vec<_>>();
            assert_eq!(symbols.len(), 1, "{detail}");
            assert_eq!(symbols[0].role, EditorSemanticRole::Payload);
            assert_eq!(symbols[0].span, span);
            assert_eq!(symbols[0].selection, span);
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload && expected.span == span
            }));
        }

        let section = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "后段" && symbol.role == EditorSemanticRole::Outline)
            .expect("section suffix retains the section outline");
        let start = source.find("section 后段").unwrap();
        assert_eq!(section.kind, EditorSemanticKind::Namespace);
        assert_eq!(
            section.span,
            SourceSpan::new(start, start + "section 后段".len())
        );
        assert_eq!(
            section.selection,
            SourceSpan::new(start + "section ".len(), start + "section 后段".len())
        );
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn timeline_section_suffix_recovery_preserves_sections_and_later_events() {
        use crate::{EditorSemanticCompleteness, EditorSemanticRole};

        let source = concat!(
            "timeline TD\r\nsection A\r\nP : Seed\r\n",
            "  section 后段:bad\r\n",
            ": Recovered 🤓\r\n",
            "section Later: Still literal %% kept\r\n",
            "Next : Ready\r\n",
            "section Broken: \r\n",
            "After : Done\r\n",
        );
        let capture = |demand| {
            parse_timeline_semantic_model_controlled(
                source,
                "timeline",
                &crate::MermaidConfig::default(),
                demand,
                &OperationControl::new(),
            )
            .unwrap()
        };
        reset_timeline_syntax_construction_count();
        let parsed = capture(EditorFactsDemand::Capture);
        assert_eq!(timeline_syntax_construction_count(), 1);
        let Error::DiagramParse { diagnostic, .. } = parsed.model.unwrap_err() else {
            panic!("invalid section suffix must fail strict parsing");
        };
        let first = source.find(":bad").unwrap() + 1;
        let second = source.find("Broken: ").unwrap() + "Broken: ".len();
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(first, first)));
        let facts = parsed.editor_facts.unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert_eq!(
            facts
                .diagnostics
                .iter()
                .map(|issue| issue.span)
                .collect::<Vec<_>>(),
            [
                Some(SourceSpan::new(first, first)),
                Some(SourceSpan::new(second, second))
            ]
        );
        for (name, role) in [
            ("后段", EditorSemanticRole::Outline),
            ("Recovered 🤓", EditorSemanticRole::Payload),
            ("Still literal %% kept", EditorSemanticRole::Payload),
            ("Broken", EditorSemanticRole::Outline),
            ("After", EditorSemanticRole::Outline),
            ("Done", EditorSemanticRole::Payload),
        ] {
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name == name && symbol.role == role)
            );
        }
        assert!(!facts.symbols.iter().any(|symbol| symbol.name == "bad"));

        let recovery = capture(EditorFactsDemand::Recovery);
        let recovery_facts = recovery.editor_facts.unwrap();
        assert!(recovery.model.is_err());
        assert_eq!(recovery_facts.diagnostics, facts.diagnostics);
        assert!(recovery_facts.symbols.is_empty());
        assert!(recovery_facts.expected_syntax.is_empty());
    }

    #[test]
    #[cfg(feature = "editor-facts")]
    fn recovery_capture_keeps_ordered_issues_without_timeline_payloads() {
        let source = "timeline\r\ntitle Example\r\nBefore:bad\r\nMiddle: Ready\r\nAfter:bad\r\n";
        let capture = |demand| {
            parse_timeline_semantic_model_controlled(
                source,
                "timeline",
                &crate::MermaidConfig::default(),
                demand,
                &OperationControl::new(),
            )
            .unwrap()
        };
        let full = capture(EditorFactsDemand::Capture);
        let full_facts = full.editor_facts.unwrap();
        assert!(!full_facts.symbols.is_empty());
        crate::editor::test_work::reset();
        reset_timeline_syntax_construction_count();
        let recovery = capture(EditorFactsDemand::Recovery);
        assert_eq!(
            recovery.model.unwrap_err().to_string(),
            full.model.unwrap_err().to_string()
        );
        let facts = recovery.editor_facts.unwrap();
        assert_eq!(facts.diagnostics, full_facts.diagnostics);
        assert_eq!(facts.diagnostics.len(), 2);
        assert!(facts.symbols.is_empty());
        assert!(facts.directive_prefixes.is_empty());
        assert!(facts.expected_syntax.is_empty());
        let work = crate::editor::test_work::read();
        assert_eq!(
            (work.symbols, work.directives, work.expected_syntax),
            (0, 0, 0)
        );
        assert_eq!(timeline_syntax_construction_count(), 1);
    }

    #[test]
    fn canonical_capture_retains_every_recovery_issue_in_source_order() {
        let source = "timeline\r\nBefore:bad\r\nMiddle: Ready\r\nAfter:bad\r\n";
        reset_timeline_syntax_construction_count();
        let parsed = parse_timeline_semantic_model_controlled(
            source,
            "timeline",
            &crate::MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
            &OperationControl::new(),
        )
        .unwrap();
        assert_eq!(timeline_syntax_construction_count(), 1);
        let Error::DiagramParse { diagnostic, .. } = parsed.model.unwrap_err() else {
            panic!("expected first Timeline parse error");
        };
        let first = source.find(':').unwrap() + 1;
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(first, first)));
        #[cfg(feature = "editor-facts")]
        {
            let facts = parsed.editor_facts.expect("requested facts");
            assert_eq!(
                facts.completeness,
                crate::EditorSemanticCompleteness::Recovered
            );
            assert_eq!(facts.diagnostics.len(), 2);
            assert_eq!(
                facts.diagnostics[0].span,
                Some(SourceSpan::new(first, first))
            );
            assert!(
                facts.diagnostics[0].span.unwrap().start < facts.diagnostics[1].span.unwrap().start
            );
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Middle"));
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Ready"));
        }
    }
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[cfg(feature = "editor-facts")]
    use crate::{
        EditorExpectedSyntaxKind, EditorSemanticCompleteness, EditorSemanticDiagnosticKind,
        EditorSemanticKind, EditorSemanticRole,
    };
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    use crate::{MermaidEngine, ParseDiagnosticSpanKind, ParseOptions, SourceSpan};
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    use futures::executor::block_on;
    use serde_json::json;

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    fn parse(text: &str) -> crate::ManagedSemanticJson {
        let engine = MermaidEngine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_simple_section_definition() {
        let model = parse(
            r#"
timeline
section abc-123
"#,
        );
        assert_eq!(model["sections"][0].as_str().unwrap(), "abc-123");
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_section_with_two_tasks() {
        let model = parse(
            r#"
timeline
section abc-123
task1
task2
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);
        for task in tasks {
            assert_eq!(task["section"].as_str().unwrap(), "abc-123");
            assert!(matches!(task["task"].as_str().unwrap(), "task1" | "task2"));
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_two_sections_and_two_tasks_each() {
        let model = parse(
            r#"
timeline
section abc-123
task1
task2
section abc-456
task3
task4
"#,
        );
        assert_eq!(
            model["sections"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_str().unwrap().to_string())
                .collect::<Vec<_>>(),
            vec!["abc-123".to_string(), "abc-456".to_string()]
        );

        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 4);
        for t in tasks {
            let section = t["section"].as_str().unwrap();
            let task = t["task"].as_str().unwrap().trim();
            assert!(matches!(section, "abc-123" | "abc-456"));
            assert!(matches!(task, "task1" | "task2" | "task3" | "task4"));
            if section == "abc-123" {
                assert!(matches!(task, "task1" | "task2"));
            } else {
                assert!(matches!(task, "task3" | "task4"));
            }
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_typed_tasks_retain_duplicate_section_occurrences() {
        let source = concat!(
            "timeline\n",
            "section Repeated\n",
            "2000: First\n",
            "section Repeated\n",
            "2001: Second\n",
            "2002: Third\n",
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("duplicate Timeline sections should parse")
            .expect("Timeline source should be detected");
        let merman_model::presentation::RenderSemanticModel::Timeline(model) = parsed.model()
        else {
            panic!("expected a Timeline render model");
        };

        assert_eq!(
            model.sections,
            vec!["Repeated".to_string(), "Repeated".to_string()]
        );
        assert_eq!(
            model
                .tasks
                .iter()
                .map(|task| task.section_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1), Some(1)]
        );
        assert_eq!(
            serde_json::to_value(&model.tasks[0]).expect("typed task should serialize")["sectionIndex"],
            json!(0)
        );

        let compat = parsed
            .compatibility_json()
            .expect("Timeline compatibility projection should serialize");
        assert!(compat["tasks"][0].get("sectionIndex").is_none());
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_tasks_and_events() {
        let model = parse(
            r#"
timeline
section abc-123
task1: event1
task2: event2: event3
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);
        for t in tasks {
            let task = t["task"].as_str().unwrap().trim();
            match task {
                "task1" => {
                    assert_eq!(
                        t["events"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_str().unwrap().to_string())
                            .collect::<Vec<_>>(),
                        vec!["event1".to_string()]
                    );
                }
                "task2" => {
                    assert_eq!(
                        t["events"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_str().unwrap().to_string())
                            .collect::<Vec<_>>(),
                        vec!["event2".to_string(), "event3".to_string()]
                    );
                }
                _ => panic!("unexpected task: {task}"),
            }
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_events_support_markdown_link() {
        let model = parse(
            r#"
timeline
section abc-123
task1: [event1](http://example.com)
task2: event2: event3
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);
        for t in tasks {
            let task = t["task"].as_str().unwrap().trim();
            match task {
                "task1" => {
                    assert_eq!(
                        t["events"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_str().unwrap().to_string())
                            .collect::<Vec<_>>(),
                        vec!["[event1](http://example.com)".to_string()]
                    );
                }
                "task2" => {}
                _ => panic!("unexpected task: {task}"),
            }
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_multiline_events_are_attached_to_previous_task() {
        let model = parse(
            r#"
timeline
section abc-123
task1: event1
task2: event2: event3
     : event4: event5
"#,
        );
        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 2);
        for t in tasks {
            let task = t["task"].as_str().unwrap().trim();
            match task {
                "task1" => {
                    assert_eq!(t["events"].as_array().unwrap().len(), 1);
                }
                "task2" => {
                    assert_eq!(
                        t["events"]
                            .as_array()
                            .unwrap()
                            .iter()
                            .map(|v| v.as_str().unwrap().to_string())
                            .collect::<Vec<_>>(),
                        vec![
                            "event2".to_string(),
                            "event3".to_string(),
                            "event4".to_string(),
                            "event5".to_string()
                        ]
                    );
                }
                _ => panic!("unexpected task: {task}"),
            }
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_allows_semicolons_in_title_section_and_events() {
        let model = parse(
            r#"
timeline
title ;my;title;
section ;a;bc-123;
;ta;sk1;: ;ev;ent1; : ;ev;ent2; : ;ev;ent3;
"#,
        );
        assert_eq!(model["title"].as_str().unwrap(), ";my;title;");
        assert_eq!(model["sections"][0].as_str().unwrap(), ";a;bc-123;");

        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        let events = tasks[0]["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            vec![
                ";ev;ent1; ".to_string(),
                ";ev;ent2; ".to_string(),
                ";ev;ent3;".to_string()
            ]
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_allows_hashtags_in_title_section_and_events() {
        let model = parse(
            r#"
timeline
title #my#title#
section #a#bc-123#
task1: #ev#ent1# : #ev#ent2# : #ev#ent3#
"#,
        );
        assert_eq!(model["title"].as_str().unwrap(), "#my#title#");
        assert_eq!(model["sections"][0].as_str().unwrap(), "#a#bc-123#");

        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["task"].as_str().unwrap(), "task1");
        let events = tasks[0]["events"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            events,
            vec![
                "#ev#ent1# ".to_string(),
                "#ev#ent2# ".to_string(),
                "#ev#ent3#".to_string()
            ]
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_event_missing_space_reports_insertion_point() {
        let engine = MermaidEngine::new();
        let text = "timeline\ntask1:event1\n";
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = err else {
            panic!("expected timeline parse error");
        };

        let colon = text.find(':').unwrap();
        assert_eq!(
            diagnostic.message(),
            "invalid event token: missing whitespace after ':'"
        );
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(colon + 1, colon + 1))
        );
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let text = "timeline\n\
title My timeline\n\
accTitle: My acc title\n\
accDescr: My acc descr\n\
section alpha\n\
task1: event1\n\
task2: event2: event3\n";
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("timeline", text)
            .unwrap()
            .unwrap();

        assert!(facts.directive_prefixes.iter().any(|p| p == "title"));
        assert!(facts.directive_prefixes.iter().any(|p| p == "accTitle"));
        assert!(facts.directive_prefixes.iter().any(|p| p == "accDescr"));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "alpha"
                && symbol.kind == EditorSemanticKind::Namespace
                && symbol.role == EditorSemanticRole::Outline
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "task1"
                && symbol.kind == EditorSemanticKind::Event
                && symbol.role == EditorSemanticRole::Outline
        }));

        let task_start = text.find("task1").unwrap();
        let event_start = text.find("event1").unwrap();

        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(task_start, task_start + "task1".len())
        }));
        assert!(!facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
                && expected.span == SourceSpan::new(task_start, task_start + "task1".len())
        }));
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(event_start, event_start + "event1".len())
        }));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_entrypoints_and_combined_projection_construct_once() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "timeline\n",
            "title Delivery\n",
            "accTitle: Delivery timeline\n",
            "section Build\n",
            "Implement parser: API ready: Tests green\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("standalone Timeline JSON parse succeeds")
            .expect("standalone Timeline JSON parse returns a diagram");
        reset_timeline_syntax_construction_count();
        crate::editor::test_work::reset();
        parse_timeline(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("Timeline JSON projection succeeds");
        assert_eq!(timeline_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());

        reset_timeline_syntax_construction_count();
        let (typed, _output_data) = parse_timeline_model_for_render(
            text,
            &crate::diagrams::source_metadata_for_test(&parsed.meta),
        )
        .expect("Timeline typed projection succeeds");
        assert_eq!(timeline_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());

        reset_timeline_syntax_construction_count();
        let (combined_json, combined_editor) =
            crate::family::test_support::into_result(parse_timeline_json_and_editor_facts(
                text,
                &crate::diagrams::source_metadata_for_test(&parsed.meta),
                &OperationControl::new(),
            ))
            .expect("Timeline combined projection succeeds");
        assert_eq!(timeline_syntax_construction_count(), 1);
        assert_eq!(combined_json, parsed.model);
        assert!(!combined_editor.symbols.is_empty());

        assert_eq!(
            merman_model::families::timeline::compat::project(
                &typed,
                _output_data,
                &parsed.meta.diagram_type
            ),
            combined_json
        );
        assert_eq!(parsed.model["type"], "timeline");
        assert!(parsed.model["accDescr"].is_null());
        crate::editor::test_work::reset();
    }

    #[test]
    fn timeline_semantic_model_omits_editor_facts_and_preserves_occurrence_identity() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_timeline_syntax_construction_count();
        let source = concat!(
            "timeline TD\n",
            "title Delivery\n",
            "Before: First event\n",
            "section Repeat\n",
            "Build: API ready: Tests green\n",
            "section Repeat\n",
            "Ship\n",
            ": Release ready\n",
        );
        let semantic = parse_timeline_semantic_model_controlled(
            source,
            "timeline",
            &crate::MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(timeline_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        assert_eq!(semantic.direction, TimelineDirection::TopDown);
        assert_eq!(semantic.sections, ["Repeat", "Repeat"]);
        assert_eq!(
            semantic
                .tasks
                .iter()
                .map(|task| (task.id, task.section_index))
                .collect::<Vec<_>>(),
            [(0, None), (1, Some(0)), (2, Some(1))]
        );
        assert_eq!(semantic.tasks[0].events, ["First event"]);
        assert_eq!(semantic.tasks[1].events, ["API ready", "Tests green"]);
        assert_eq!(semantic.tasks[2].events, ["Release ready"]);

        #[cfg(feature = "presentation")]
        {
            let meta = crate::SourcePresentationMetadata {
                diagram_type: "timeline".to_string(),
                config: crate::MermaidConfig::default(),
                effective_config: crate::presentation::SourcePresentationConfig::default(),
                title: None,
            };
            let (render, _output_data) = parse_timeline_model_for_render(source, &meta).unwrap();
            assert_eq!(
                serde_json::to_value(semantic).unwrap(),
                serde_json::to_value(render).unwrap()
            );
        }
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
    }

    #[test]
    fn timeline_semantic_model_applies_common_text_policy_without_theme_resolution() {
        let config = crate::MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "themeVariables": {"primaryColor": "not-a-color"},
        }));
        let source = "timeline\ntitle <script>bad()</script>Delivery\naccTitle: <script>bad()</script>Accessible\naccDescr: <script>bad()</script>Description\n2026: Ready\n";
        let semantic = parse_timeline_semantic_model_controlled(
            source,
            "timeline",
            &config,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .expect("semantic parse does not evaluate theme colors");
        assert_eq!(
            (
                semantic.title.as_deref(),
                semantic.acc_title.as_deref(),
                semantic.acc_descr.as_deref()
            ),
            (Some("Delivery"), Some("Accessible"), Some("Description"))
        );
        #[cfg(feature = "presentation")]
        {
            let meta = SourcePresentationMetadata {
                diagram_type: "timeline".to_string(),
                config: crate::MermaidConfig::empty_object(),
                effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
                title: None,
            };
            let (mut expected, _output_data) =
                parse_timeline_model_for_render(source, &meta).unwrap();
            sanitize_common_db_fields(&mut expected, &config);
            assert_eq!(
                serde_json::to_value(semantic).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
        }
    }

    #[test]
    fn timeline_semantic_empty_input_has_no_compatibility_sentinel() {
        let semantic = parse_timeline_semantic_model_controlled(
            "",
            "timeline",
            &crate::MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        #[cfg(feature = "compat-json")]
        let meta = SourcePresentationMetadata {
            diagram_type: "timeline".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };

        assert_eq!(
            serde_json::to_value(semantic).unwrap(),
            json!({
                "direction": "LR",
                "title": null,
                "accTitle": null,
                "accDescr": null,
                "sections": [],
                "tasks": [],
            })
        );
        #[cfg(feature = "compat-json")]
        assert_eq!(parse_timeline("", &meta).unwrap(), json!({}));
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn timeline_typed_projection_preserves_empty_and_header_only_output_states() {
        let meta = SourcePresentationMetadata {
            diagram_type: "timeline".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        for source in ["", "timeline"] {
            let compat = parse_timeline(source, &meta).unwrap();
            let (typed, _output_data) = parse_timeline_model_for_render(source, &meta).unwrap();

            assert_eq!(
                render_model_to_compat_json(&typed, _output_data, &meta).unwrap(),
                compat,
                "projection drift for {source:?}"
            );
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_malformed_event_recovers_prior_parser_facts_once() {
        let engine = MermaidEngine::new();
        let text = "timeline\nsection Build\nImplement parser:event\n";
        let colon = text.find(':').expect("malformed event colon");

        reset_timeline_syntax_construction_count();
        crate::editor::test_work::reset();
        let error = parse_timeline_semantic_model_controlled(
            text,
            "timeline",
            &crate::MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .expect_err("semantic parse must reject a malformed event");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected Timeline parse error");
        };
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(colon + 1, colon + 1))
        );
        assert_eq!(timeline_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());

        reset_timeline_syntax_construction_count();
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("timeline", text)
            .expect("Timeline editor recovery succeeds")
            .expect("Timeline editor facts are available");

        assert_eq!(timeline_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Build" && symbol.role == EditorSemanticRole::Outline
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Implement parser" && symbol.role == EditorSemanticRole::Outline
        }));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(colon + 1, colon + 1))
        }));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_direction_is_preserved_in_semantic_and_compatibility_models() {
        let engine = MermaidEngine::new();
        for (source, expected) in [
            ("timeline\n2026", TimelineDirection::LeftToRight),
            ("timeline LR\n2026", TimelineDirection::LeftToRight),
            ("timeline TD\n2026", TimelineDirection::TopDown),
        ] {
            let parsed = engine
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .expect("Timeline parse succeeds")
                .expect("Timeline is detected");
            let crate::RenderSemanticModel::Timeline(model) = parsed.model() else {
                panic!("expected Timeline render model");
            };

            assert_eq!(model.direction, expected, "direction for {source:?}");
            assert_eq!(
                parsed.compatibility_json().expect("compatibility JSON")["direction"],
                serde_json::to_value(expected).expect("direction serializes")
            );
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_semantic_facts_are_exact_for_crlf_unicode_and_repeated_text() {
        let source = concat!(
            "---\r\n",
            "config:\r\n",
            "  timeline:\r\n",
            "    disableMulticolor: false\r\n",
            "---\r\n",
            "%%{init: { 'theme': 'base' }}%%\r\n",
            "%% global comment\r\n",
            "timeline LR # header comment\r\n",
            "# family comment 🤓\r\n",
            "title title\r\n",
            "accTitle: accTitle # accessibility comment\r\n",
            "accDescr: accDescr; trailing\r\n",
            "accDescr {\r\n",
            "  多行说明 🤓\r\n",
            "}\r\n",
            "section section\r\n",
            "section 重复\r\n",
            "重复: 重复\r\n",
            ": 重复\r\n",
        );
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("timeline", source)
            .expect("Timeline editor parse")
            .expect("Timeline editor facts");

        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        let repeated = source
            .match_indices("重复")
            .map(|(start, text)| SourceSpan::new(start, start + text.len()))
            .collect::<Vec<_>>();
        assert_eq!(repeated.len(), 4);
        for (span, kind, role) in [
            (
                repeated[0],
                EditorSemanticKind::Namespace,
                EditorSemanticRole::Outline,
            ),
            (
                repeated[1],
                EditorSemanticKind::Event,
                EditorSemanticRole::Outline,
            ),
            (
                repeated[2],
                EditorSemanticKind::String,
                EditorSemanticRole::Payload,
            ),
            (
                repeated[3],
                EditorSemanticKind::String,
                EditorSemanticRole::Payload,
            ),
        ] {
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "重复"
                    && symbol.kind == kind
                    && symbol.role == role
                    && symbol.selection == span
            }));
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_recovery_keeps_error_prefix_and_later_lines_while_strict_returns_first_error() {
        let source = concat!(
            "timeline TD\r\n",
            "section Before\r\n",
            "Before task:event\r\n",
            "After task: 🤓 后续\r\n",
            "Later task:broken\r\n",
        );
        let first_colon = source.find(':').expect("first malformed event colon");
        let meta = SourcePresentationMetadata {
            diagram_type: "timeline".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let error =
            parse_timeline(source, &meta).expect_err("strict parse must return first error");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected structured Timeline parse error");
        };
        assert_eq!(
            diagnostic.message(),
            "invalid event token: missing whitespace after ':'"
        );
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(first_colon + 1, first_colon + 1))
        );

        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("timeline", source)
            .expect("Timeline editor recovery")
            .expect("Timeline recovery facts");
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Before task"
                && symbol.kind == EditorSemanticKind::Event
                && symbol.role == EditorSemanticRole::Outline
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "After task"
                && symbol.kind == EditorSemanticKind::Event
                && symbol.role == EditorSemanticRole::Outline
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "🤓 后续"
                && symbol.kind == EditorSemanticKind::String
                && symbol.role == EditorSemanticRole::Payload
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Later task"
                && symbol.kind == EditorSemanticKind::Event
                && symbol.role == EditorSemanticRole::Outline
        }));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn timeline_rejects_unterminated_multiline_acc_descr_like_pinned_jison() {
        let text = "timeline\naccDescr {\n  partial description\n";

        reset_timeline_syntax_construction_count();
        let snapshot = MermaidEngine::new()
            .parse_diagram_snapshot_with_type_sync("timeline", text)
            .expect("Timeline snapshot operation")
            .expect("Timeline snapshot");
        assert_eq!(timeline_syntax_construction_count(), 1);
        assert!(matches!(
            snapshot.outcome(),
            crate::DiagramParseOutcome::Failed(_)
        ));
        let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Timeline recovery facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(!facts.diagnostics.is_empty());
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "partial description" && symbol.role == EditorSemanticRole::Payload
        }));
    }
}

#[cfg(all(test, feature = "presentation"))]
mod output_data_tests {
    use super::*;

    #[test]
    fn full_producer_preserves_empty_output_independently_of_json_projection() {
        let meta = crate::SourcePresentationMetadata {
            diagram_type: "timeline".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::SourcePresentationConfig::empty_object(),
            title: None,
        };
        let parse = |source| {
            parse_timeline_model_for_render_controlled(
                source,
                &meta,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .unwrap()
        };
        let (empty, empty_output) = parse("");
        let (header, header_output) = parse("timeline");
        assert_eq!(empty_output, TimelineOutputData::Empty);
        assert_eq!(header_output, TimelineOutputData::Model);
        assert_eq!(TimelineOutputData::default(), TimelineOutputData::Model);
        // Identical canonical values must not erase the source producer's distinct output shape.
        assert_eq!(
            serde_json::to_value(&empty).unwrap(),
            serde_json::to_value(&header).unwrap()
        );
        #[cfg(feature = "compat-json")]
        {
            assert_eq!(
                render_model_to_compat_json(&empty, empty_output, &meta).unwrap(),
                serde_json::json!({})
            );
            assert_eq!(
                render_model_to_compat_json(
                    &TimelineDiagramModel::default(),
                    TimelineOutputData::default(),
                    &meta
                )
                .unwrap()["type"],
                "timeline"
            );
        }
    }
}
