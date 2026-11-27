//! Journey statement scanning and semantic model construction.

use merman_core_preprocess::scan::{
    LineCursor, split_statement_suffix_hash_or_semi, starts_with_case_insensitive,
};
use merman_model::families::journey::{JourneyDiagramModel, JourneyTask};
use merman_model::number::parse_js_number;
use merman_model::text::is_ecmascript_whitespace;
use merman_model::{OperationControl, OperationControlResult, SourceSpan};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JourneySourceIssue {
    UnexpectedHeader(SourceSpan),
    ExpectedHeader(SourceSpan),
    MissingAccTitle(usize),
    MissingAccDescr(usize),
    UnterminatedAccDescr(usize),
    TrailingSectionTaskData(SourceSpan),
    MissingTaskName(SourceSpan),
    MissingTaskData(usize),
}

#[cfg(feature = "editor-facts")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JourneyTextKind {
    Title,
    AccTitle,
    AccDescr,
    AccDescrBlock,
    Section,
    Task,
    Score,
    People,
}

/// Source observations borrowed from the active statement in preprocessed coordinates.
#[cfg(feature = "editor-facts")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JourneyObservation<'source> {
    Text {
        kind: JourneyTextKind,
        text: &'source str,
        statement: SourceSpan,
        selection: SourceSpan,
    },
    UnterminatedAccDescr(SourceSpan),
}

#[derive(Debug)]
pub struct JourneyParsedSource {
    pub model: JourneyDiagramModel,
    pub header_seen: bool,
}

#[derive(Debug, Default)]
struct JourneyDb {
    title: String,
    acc_title: String,
    acc_descr: String,

    current_section: String,
    current_section_index: Option<usize>,
    sections: Vec<String>,
    tasks: Vec<JourneyTask>,
}

impl JourneyDb {
    fn add_section(&mut self, txt: &str) {
        self.current_section = txt.to_string();
        self.current_section_index = Some(self.sections.len());
        self.sections.push(txt.to_string());
    }

    fn add_task(&mut self, descr: &str, task_data: &str) {
        let rest = task_data.strip_prefix(':').unwrap_or(task_data);
        let mut fields = rest.split(':');

        let score_str = fields.next().unwrap_or("");
        // Mermaid upstream uses JS `Number(...)` for parsing task scores. This means:
        // - whitespace-only => 0
        // - invalid strings => NaN (and Mermaid happily renders an SVG containing `NaN`)
        //
        // Retain the legacy `score=0` + `scoreIsNaN=true` state for invalid Number input.
        // Signed infinities are valid Number results and must retain their score and mouth kind.
        let (score, score_is_nan) = match parse_js_number(score_str) {
            Some(value) if !value.is_nan() => (value, false),
            _ => (0.0, true),
        };

        let people = fields.next().map_or_else(Vec::new, |people| {
            people
                .split(',')
                .map(|s| s.trim_matches(is_ecmascript_whitespace).to_string())
                .collect()
        });

        self.tasks.push(JourneyTask {
            score,
            score_is_nan,
            people,
            section: self.current_section.clone(),
            section_index: self.current_section_index,
            task_type: self.current_section.clone(),
            task: descr.to_string(),
        });
    }

    fn actors_sorted(&self) -> Vec<String> {
        let mut set = BTreeSet::<String>::new();
        for t in &self.tasks {
            for p in &t.people {
                set.insert(p.clone());
            }
        }
        let mut actors: Vec<_> = set.into_iter().collect();
        actors.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
        actors
    }
}

struct JourneyText<'a> {
    text: &'a str,
    #[cfg(feature = "editor-facts")]
    span: SourceSpan,
    statement: SourceSpan,
}

#[cfg(feature = "editor-facts")]
impl<'source> JourneyText<'source> {
    fn observation(&self, kind: JourneyTextKind) -> JourneyObservation<'source> {
        JourneyObservation::Text {
            kind,
            text: self.text,
            statement: self.statement,
            selection: self.span,
        }
    }
}

fn scan_journey_while(
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

fn parse_keyword_arg_one_ws<'a>(
    source: &'a str,
    start: usize,
    keyword: &str,
    control: &OperationControl,
) -> OperationControlResult<Option<JourneyText<'a>>> {
    let rest = &source[start..];
    if !starts_with_case_insensitive(rest, keyword) {
        return Ok(None);
    }
    let Some(ws) = rest[keyword.len()..].chars().next() else {
        return Ok(None);
    };
    if !is_ecmascript_whitespace(ws) {
        return Ok(None);
    }
    let value_start = start + keyword.len() + ws.len_utf8();
    let value_end = scan_journey_while(
        source,
        value_start,
        |ch| !matches!(ch, '#' | '\n' | ';') && (keyword != "section" || ch != ':'),
        control,
    )?;
    Ok((value_start < value_end).then(|| JourneyText {
        text: &source[value_start..value_end],
        #[cfg(feature = "editor-facts")]
        span: SourceSpan::new(value_start, value_end),
        statement: SourceSpan::new(start, value_end),
    }))
}

fn parse_accessibility_delimiter(
    source: &str,
    start: usize,
    key: &str,
    delimiter: u8,
    control: &OperationControl,
) -> OperationControlResult<Option<usize>> {
    if !starts_with_case_insensitive(&source[start..], key) {
        return Ok(None);
    }
    let end = scan_journey_while(source, start + key.len(), is_ecmascript_whitespace, control)?;
    Ok((source.as_bytes().get(end) == Some(&delimiter)).then_some(end))
}

fn parse_key_colon_value<'a>(
    source: &'a str,
    start: usize,
    key: &str,
    control: &OperationControl,
) -> OperationControlResult<Option<JourneyText<'a>>> {
    let Some(colon) = parse_accessibility_delimiter(source, start, key, b':', control)? else {
        return Ok(None);
    };
    let value_start = scan_journey_while(source, colon + 1, is_ecmascript_whitespace, control)?;
    let value_end = scan_journey_while(source, value_start, |ch| ch != '\n', control)?;
    Ok(Some(JourneyText {
        text: &source[value_start..value_end],
        #[cfg(feature = "editor-facts")]
        span: SourceSpan::new(value_start, value_end),
        statement: SourceSpan::new(start, value_end),
    }))
}

struct JourneyBlockText<'a> {
    text: &'a str,
    span: SourceSpan,
    #[cfg(feature = "editor-facts")]
    opening: SourceSpan,
    closing: Option<SourceSpan>,
    #[cfg(feature = "editor-facts")]
    statement: SourceSpan,
}

fn parse_acc_descr_block_spanned<'a>(
    lines: &mut LineCursor<'a>,
    start: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<JourneyBlockText<'a>>> {
    control.checkpoint()?;
    let source = lines.source();
    let Some(open) = parse_accessibility_delimiter(source, start, "accDescr", b'{', control)?
    else {
        return Ok(None);
    };
    let content_start = open + 1;
    let content_end = scan_journey_while(source, content_start, |ch| ch != '}', control)?;
    let closing =
        (content_end < source.len()).then_some(SourceSpan::new(content_end, content_end + 1));
    let end = closing.map_or(content_end, |span| span.end);
    lines.resume_at(end);
    Ok(Some(JourneyBlockText {
        text: source[content_start..content_end].trim_matches(is_ecmascript_whitespace),
        span: SourceSpan::new(content_start, content_end),
        #[cfg(feature = "editor-facts")]
        opening: SourceSpan::new(content_start - 1, content_start),
        closing,
        #[cfg(feature = "editor-facts")]
        statement: SourceSpan::new(start, end),
    }))
}

fn strip_comment_prefix(line: &str) -> &str {
    let t = line.trim_start_matches(is_ecmascript_whitespace);
    if t.starts_with('#') {
        return "";
    }
    let prefixed_comment = t
        .chars()
        .next()
        .is_some_and(|ch| ch != '}' && ch.len_utf16() == 1 && t[ch.len_utf8()..].starts_with("%%"));
    if (t.starts_with("%%") && !t.starts_with("%%{")) || prefixed_comment {
        return "";
    }
    split_statement_suffix_hash_or_semi(line)
}

/// Parses Journey statements once, retaining the first issue while continuing recovery.
///
/// Rich observations are optional. The unterminated accessibility block observation is
/// still delivered to a recovery-only observer. Every span is relative to `code`.
pub fn parse_journey_source_controlled<'source>(
    code: &'source str,
    #[cfg(feature = "editor-facts")] capture_facts: bool,
    #[cfg(feature = "editor-facts")] mut observe: Option<
        &mut dyn FnMut(JourneyObservation<'source>),
    >,
    control: &OperationControl,
) -> OperationControlResult<Result<JourneyParsedSource, JourneySourceIssue>> {
    control.checkpoint()?;
    let mut db = JourneyDb::default();
    let mut lines = LineCursor::new(code);
    let mut header_seen = false;
    let mut first_error = None;

    while let Some((line, line_start)) = lines.next_line() {
        control.checkpoint()?;
        let stripped = strip_comment_prefix(line);
        let t = stripped.trim_matches(is_ecmascript_whitespace);
        if t.is_empty() {
            continue;
        }

        if !header_seen {
            if starts_with_case_insensitive(t, "journey") {
                header_seen = true;
                let header_start = line_start + line.find(t).unwrap_or(0);
                let after_header = &t["journey".len()..];
                let rest = after_header.trim_start_matches(is_ecmascript_whitespace);
                if !rest.is_empty() {
                    let start = header_start
                        + "journey".len()
                        + after_header.len().saturating_sub(rest.len());
                    first_error.get_or_insert(JourneySourceIssue::UnexpectedHeader(
                        SourceSpan::new(start, start + rest.len()),
                    ));
                    continue;
                }
                continue;
            }
            let start =
                line_start + line.len() - line.trim_start_matches(is_ecmascript_whitespace).len();
            first_error.get_or_insert(JourneySourceIssue::ExpectedHeader(SourceSpan::new(
                start,
                line_start + line.len(),
            )));
            continue;
        }

        let token_start =
            line_start + line.len() - line.trim_start_matches(is_ecmascript_whitespace).len();
        if let Some(v) = parse_keyword_arg_one_ws(lines.source(), token_start, "title", control)? {
            lines.resume_at(v.statement.end);
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                observe(v.observation(JourneyTextKind::Title));
            }
            db.title = v.text.to_string();
            continue;
        }
        if let Some(v) = parse_key_colon_value(lines.source(), token_start, "accTitle", control)? {
            lines.resume_at(v.statement.end);
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                observe(v.observation(JourneyTextKind::AccTitle));
            }
            if v.text.is_empty() {
                first_error.get_or_insert(JourneySourceIssue::MissingAccTitle(v.statement.end));
                continue;
            }
            db.acc_title = v.text.trim_matches(is_ecmascript_whitespace).to_string();
            continue;
        }
        if let Some(v) = parse_key_colon_value(lines.source(), token_start, "accDescr", control)? {
            lines.resume_at(v.statement.end);
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                observe(v.observation(JourneyTextKind::AccDescr));
            }
            if v.text.is_empty() {
                first_error.get_or_insert(JourneySourceIssue::MissingAccDescr(v.statement.end));
                continue;
            }
            db.acc_descr = v.text.trim_matches(is_ecmascript_whitespace).to_string();
            continue;
        }
        if let Some(v) = parse_acc_descr_block_spanned(&mut lines, token_start, control)? {
            if v.text.is_empty() && v.closing.is_some() {
                continue;
            }
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                observe(JourneyObservation::Text {
                    kind: JourneyTextKind::AccDescrBlock,
                    text: v.text,
                    statement: v.statement,
                    selection: v.span,
                });
            }
            if v.closing.is_none() {
                #[cfg(feature = "editor-facts")]
                if let Some(observe) = observe.as_deref_mut() {
                    observe(JourneyObservation::UnterminatedAccDescr(SourceSpan::new(
                        v.opening.start,
                        v.span.end,
                    )));
                }
                first_error.get_or_insert(JourneySourceIssue::UnterminatedAccDescr(v.span.end));
            }
            db.acc_descr = v.text.to_string();
            continue;
        }
        if let Some(v) = parse_keyword_arg_one_ws(lines.source(), token_start, "section", control)?
        {
            let end = v.statement.end;
            lines.resume_at(end);
            if lines.source().as_bytes().get(end) == Some(&b':') {
                let line_end = scan_journey_while(lines.source(), end, |ch| ch != '\n', control)?;
                lines.resume_at(line_end);
                first_error.get_or_insert(JourneySourceIssue::TrailingSectionTaskData(
                    SourceSpan::new(end, line_end),
                ));
                continue;
            }
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                let value = v.text.trim_matches(is_ecmascript_whitespace);
                if !value.is_empty() {
                    let start = v.span.start + v.text.len()
                        - v.text.trim_start_matches(is_ecmascript_whitespace).len();
                    observe(JourneyObservation::Text {
                        kind: JourneyTextKind::Section,
                        text: value,
                        statement: SourceSpan::new(line_start, end),
                        selection: SourceSpan::new(start, start + value.len()),
                    });
                }
            }
            db.add_section(v.text);
            continue;
        }

        let colon = stripped.find(':');
        let task_name_source = colon
            .map_or(stripped, |colon| &stripped[..colon])
            .trim_start_matches(is_ecmascript_whitespace);
        let task_name = task_name_source.trim_matches(is_ecmascript_whitespace);
        if task_name.is_empty() {
            let start = line_start + line.find(t).unwrap_or(0);
            first_error.get_or_insert(JourneySourceIssue::MissingTaskName(SourceSpan::new(
                start,
                start + t.len(),
            )));
            continue;
        }
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
            let task_relative = task_name.as_ptr() as usize - line.as_ptr() as usize;
            let task_start = line_start + task_relative;
            let task_end = task_start + task_name.len();
            observe(JourneyObservation::Text {
                kind: JourneyTextKind::Task,
                text: task_name,
                statement: SourceSpan::new(line_start, line_start + line.len()),
                selection: SourceSpan::new(task_start, task_end),
            });
        }

        let Some(colon) = colon else {
            let insertion = line_start + stripped.trim_end_matches(is_ecmascript_whitespace).len();
            first_error.get_or_insert(JourneySourceIssue::MissingTaskData(insertion));
            continue;
        };
        let task_data = &stripped[colon..];
        if task_data.len() == ':'.len_utf8() {
            first_error.get_or_insert(JourneySourceIssue::MissingTaskData(
                line_start + colon + ':'.len_utf8(),
            ));
            continue;
        }

        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
            let rest_source = &stripped[colon + ':'.len_utf8()..];
            let rest = rest_source.trim_start_matches(is_ecmascript_whitespace);
            let rest_start =
                line_start + colon + ':'.len_utf8() + rest_source.len().saturating_sub(rest.len());
            let score_end = rest.find(':').unwrap_or(rest.len());
            let score_text = rest[..score_end].trim_matches(is_ecmascript_whitespace);
            if !score_text.is_empty() {
                let score_start = rest_start + rest[..score_end].find(score_text).unwrap_or(0);
                let span = SourceSpan::new(score_start, score_start + score_text.len());
                observe(JourneyObservation::Text {
                    kind: JourneyTextKind::Score,
                    text: score_text,
                    statement: span,
                    selection: span,
                });
            }

            if score_end < rest.len() {
                let people_source = rest[score_end + ':'.len_utf8()..]
                    .split(':')
                    .next()
                    .unwrap_or("");
                let people = people_source.trim_matches(is_ecmascript_whitespace);
                if !people.is_empty() {
                    let people_start = rest_start
                        + score_end
                        + ':'.len_utf8()
                        + people_source.find(people).unwrap_or(0);
                    let span = SourceSpan::new(people_start, people_start + people.len());
                    observe(JourneyObservation::Text {
                        kind: JourneyTextKind::People,
                        text: people,
                        statement: span,
                        selection: span,
                    });
                }
            }
        }

        db.add_task(task_name_source, task_data);
    }

    if let Some(error) = first_error {
        return Ok(Err(error));
    }

    control.checkpoint()?;
    let actors = db.actors_sorted();
    let model = JourneyDiagramModel {
        title: (!db.title.is_empty()).then_some(db.title),
        acc_title: (!db.acc_title.is_empty()).then_some(db.acc_title),
        acc_descr: (!db.acc_descr.is_empty()).then_some(db.acc_descr),
        actors,
        sections: db.sections,
        tasks: db.tasks,
    };
    Ok(Ok(JourneyParsedSource { model, header_seen }))
}

#[cfg(test)]
mod tests;
