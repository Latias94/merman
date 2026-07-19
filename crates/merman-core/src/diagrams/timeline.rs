use crate::diagrams::scan::{
    LineCursor, leading_whitespace_len, split_statement_suffix_hash_or_semi,
    starts_with_case_insensitive,
};
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use serde_json::{Value, json};
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static TIMELINE_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
fn reset_timeline_syntax_construction_count() {
    TIMELINE_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
fn timeline_syntax_construction_count() -> usize {
    TIMELINE_SYNTAX_CONSTRUCTION_COUNT.get()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimelineRenderTask {
    pub id: i64,
    pub section: String,
    #[serde(rename = "type")]
    pub task_type: String,
    pub task: String,
    pub score: i64,
    #[serde(default)]
    pub events: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct TimelineDiagramRenderModel {
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub sections: Vec<String>,
    #[serde(default)]
    pub tasks: Vec<TimelineRenderTask>,
    #[serde(skip)]
    compatibility_output: CompatibilityOutputState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CompatibilityOutputState {
    Empty,
    #[default]
    Model,
}

impl TimelineDiagramRenderModel {
    fn empty_compatibility_output() -> Self {
        Self {
            compatibility_output: CompatibilityOutputState::Empty,
            ..Self::default()
        }
    }

    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Default)]
struct TimelineDb {
    title: String,
    acc_title: String,
    acc_descr: String,

    current_section: String,
    sections: Vec<String>,
    tasks: Vec<TimelineRenderTask>,
    next_id: i64,
}

impl TimelineDb {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn add_section(&mut self, txt: &str) {
        self.current_section = txt.to_string();
        self.sections.push(txt.to_string());
    }

    fn add_task(&mut self, period: &str) {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(TimelineRenderTask {
            id,
            section: self.current_section.clone(),
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
    model: Option<TimelineDiagramRenderModel>,
    editor_facts: EditorSemanticFacts,
}

struct TimelineSemanticFailure {
    error: Box<Error>,
    editor_facts: Box<EditorSemanticFacts>,
}

impl TimelineSemanticFailure {
    fn new(error: Error, editor_facts: EditorSemanticFacts) -> Self {
        Self {
            error: Box::new(error),
            editor_facts: Box::new(editor_facts),
        }
    }

    fn into_error(self) -> Error {
        *self.error
    }

    fn into_editor_facts(mut self) -> EditorSemanticFacts {
        let (message, span) = match self.error.as_ref() {
            Error::DiagramParse { diagnostic, .. } => {
                (diagnostic.message().to_string(), diagnostic.span())
            }
            error => (error.to_string(), None),
        };
        self.editor_facts.mark_recovered_from_parse_error(
            format!("timeline parser recovered after parse error: {message}"),
            span,
        );
        *self.editor_facts
    }
}

fn parse_keyword_arg_full_line_after_one_ws<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let t = line.trim_start();
    if !starts_with_case_insensitive(t, keyword) {
        return None;
    }
    let after = &t[keyword.len()..];
    let ws = after.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    Some(&after[ws.len_utf8()..])
}

fn parse_title_value(line: &str) -> Option<String> {
    let rest = parse_keyword_arg_full_line_after_one_ws(line, "title")?;
    Some(rest.to_string())
}

fn parse_section_value(line: &str) -> Option<String> {
    let rest = parse_keyword_arg_full_line_after_one_ws(line, "section")?;
    let end = rest.find(':').unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn parse_key_colon_value_spanned<'a>(
    line: &'a str,
    line_start: usize,
    key: &str,
) -> Option<SpannedText<'a>> {
    let t = line.trim_start();
    if !starts_with_case_insensitive(t, key) {
        return None;
    }
    let rest = t[key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?;
    let value = split_statement_suffix_hash_or_semi(rest).trim();
    if value.is_empty() {
        return None;
    }
    let value_rel = line.find(value)?;
    Some(SpannedText {
        text: value,
        start: line_start + value_rel,
        end: line_start + value_rel + value.len(),
    })
}

fn parse_section_value_spanned<'a>(line: &'a str, line_start: usize) -> Option<SpannedText<'a>> {
    let rest = parse_keyword_arg_full_line_after_one_ws(line, "section")?;
    let end = rest.find(':').unwrap_or(rest.len());
    let value = rest[..end].trim();
    if value.is_empty() {
        return None;
    }
    let value_rel = line.find(value)?;
    Some(SpannedText {
        text: value,
        start: line_start + value_rel,
        end: line_start + value_rel + value.len(),
    })
}

fn push_timeline_payload_fact(
    facts: &mut EditorSemanticFacts,
    text: &str,
    start: usize,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    let end = start + text.len();
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        SourceSpan::new(start, end),
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        text.to_string(),
        Some(detail.to_string()),
        kind,
        SourceSpan::new(start, end),
        SourceSpan::new(start, end),
    ));
}

fn push_timeline_payload_fact_spanned(
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

#[derive(Debug, Clone, Copy)]
struct SpannedText<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

fn parse_key_colon_value_hash_or_semi(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start();
    if !starts_with_case_insensitive(t, key) {
        return None;
    }
    let rest = t[key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?;
    Some(split_statement_suffix_hash_or_semi(rest).trim().to_string())
}

struct TimelineBlockText {
    text: String,
    span: SourceSpan,
}

fn parse_acc_descr_block_spanned(
    cursor: &mut LineCursor<'_>,
    first_line: &str,
    first_line_start: usize,
) -> Option<TimelineBlockText> {
    let t = first_line.trim_start();
    if !starts_with_case_insensitive(t, "accDescr") {
        return None;
    }
    let rest = t["accDescr".len()..].trim_start();
    let rest = rest.strip_prefix('{')?;
    let open = first_line.find('{')?;
    let content_start = first_line_start + open + 1;

    let mut buf = String::new();
    if let Some(end) = rest.find('}') {
        buf.push_str(&rest[..end]);
        return Some(TimelineBlockText {
            text: buf.trim().to_string(),
            span: SourceSpan::new(content_start, content_start + end),
        });
    }
    buf.push_str(rest);
    buf.push('\n');

    let mut content_end = cursor.offset();
    while let Some((line, line_start)) = cursor.next_line() {
        if let Some(end) = line.find('}') {
            buf.push_str(&line[..end]);
            content_end = line_start + end;
            break;
        }
        buf.push_str(line);
        buf.push('\n');
        content_end = line_start + line.len();
    }
    Some(TimelineBlockText {
        text: buf.trim().to_string(),
        span: SourceSpan::new(content_start, content_end.max(content_start)),
    })
}

struct TimelineEventText {
    text: String,
    span: SourceSpan,
}

fn split_events_from_colon_whitespace_spanned(
    input: &str,
    input_start: usize,
) -> Result<Vec<TimelineEventText>> {
    let mut s = input;
    let mut s_start = input_start;
    let mut out = Vec::new();

    while !s.is_empty() {
        let Some(colon) = s.find(':') else {
            return Err(Error::diagram_parse_exact(
                "timeline".to_string(),
                format!("invalid event token: {input}"),
                SourceSpan::new(s_start, s_start + s.len()),
            ));
        };
        if colon != 0 {
            return Err(Error::diagram_parse_exact(
                "timeline".to_string(),
                format!("invalid event token: {input}"),
                SourceSpan::new(s_start, s_start + s.len()),
            ));
        }
        let after_colon = &s[1..];
        let Some(ws) = after_colon.chars().next() else {
            return Err(Error::diagram_parse_insertion_point(
                "timeline".to_string(),
                "invalid event token: missing whitespace after ':'".to_string(),
                s_start + 1,
            ));
        };
        if !ws.is_whitespace() {
            return Err(Error::diagram_parse_insertion_point(
                "timeline".to_string(),
                "invalid event token: missing whitespace after ':'".to_string(),
                s_start + 1,
            ));
        }
        s = &after_colon[ws.len_utf8()..];
        s_start += 1 + ws.len_utf8();

        let mut next_boundary: Option<usize> = None;
        for (i, ch) in s.char_indices() {
            if ch != ':' {
                continue;
            }
            let Some(next) = s[i + 1..].chars().next() else {
                continue;
            };
            if next.is_whitespace() {
                next_boundary = Some(i);
                break;
            }
        }

        let (event, rest) = match next_boundary {
            Some(i) => (&s[..i], &s[i..]),
            None => (s, ""),
        };
        out.push(TimelineEventText {
            text: event.to_string(),
            span: SourceSpan::new(s_start, s_start + event.len()),
        });
        s = rest;
        s_start += event.len();
    }

    Ok(out)
}

pub fn parse_timeline(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let source = construct_timeline_semantic_source(code, meta)
        .map_err(TimelineSemanticFailure::into_error)?;
    match source.model {
        Some(model) => render_model_to_compat_json(&model, meta),
        None => Ok(json!({})),
    }
}

pub(crate) fn parse_timeline_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let TimelineSemanticSource {
        model,
        editor_facts,
    } = construct_timeline_semantic_source(code, meta)
        .map_err(TimelineSemanticFailure::into_error)?;
    let model = match model {
        Some(model) => render_model_to_compat_json(&model, meta)?,
        None => json!({}),
    };
    Ok((model, editor_facts))
}

pub(crate) fn render_model_to_compat_json(
    model: &TimelineDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    if model.compatibility_output == CompatibilityOutputState::Empty {
        return Ok(json!({}));
    }
    Ok(json!({
        "type": meta.diagram_type,
        "title": &model.title,
        "accTitle": &model.acc_title,
        "accDescr": &model.acc_descr,
        "sections": &model.sections,
        "tasks": &model.tasks,
    }))
}

pub fn parse_timeline_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<TimelineDiagramRenderModel> {
    construct_timeline_semantic_source(code, meta)
        .map(|source| {
            source
                .model
                .unwrap_or_else(TimelineDiagramRenderModel::empty_compatibility_output)
        })
        .map_err(TimelineSemanticFailure::into_error)
}

pub fn parse_timeline_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    match construct_timeline_semantic_source(code, meta) {
        Ok(source) => source.editor_facts,
        Err(failure) => failure.into_editor_facts(),
    }
}

fn construct_timeline_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> std::result::Result<TimelineSemanticSource, TimelineSemanticFailure> {
    #[cfg(test)]
    TIMELINE_SYNTAX_CONSTRUCTION_COUNT.set(TIMELINE_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let mut db = TimelineDb::default();
    db.clear();
    let mut editor_facts = EditorSemanticFacts::new();
    let mut lines = LineCursor::new(code);
    let mut header_seen = false;

    while let Some((line, line_start)) = lines.next_line() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("%%") {
            continue;
        }

        if !header_seen {
            if starts_with_case_insensitive(t, "timeline") {
                header_seen = true;
                let rest = t["timeline".len()..].trim_start();
                if !rest.is_empty()
                    && !rest.eq_ignore_ascii_case("LR")
                    && !rest.eq_ignore_ascii_case("TD")
                {
                    let start = line_start + line.find(rest).unwrap_or(line.len());
                    return Err(TimelineSemanticFailure::new(
                        Error::diagram_parse_exact(
                            meta.diagram_type.clone(),
                            "unexpected content after timeline header",
                            SourceSpan::new(start, start + rest.len()),
                        ),
                        editor_facts,
                    ));
                }
                continue;
            }
            let start = line_start + leading_whitespace_len(line);
            return Err(TimelineSemanticFailure::new(
                Error::diagram_parse_exact(
                    meta.diagram_type.clone(),
                    "expected timeline header",
                    SourceSpan::new(start, line_start + line.len()),
                ),
                editor_facts,
            ));
        }

        let stripped = line.trim_start();
        if let Some(v) = parse_title_value(line) {
            editor_facts.push_directive_prefix("title");
            let start = line_start + line.find(&v).unwrap_or(line.len());
            push_timeline_payload_fact(
                &mut editor_facts,
                &v,
                start,
                "timeline title",
                EditorSemanticKind::String,
            );
            db.title = v;
            continue;
        }
        if let Some(v) = parse_key_colon_value_hash_or_semi(line, "accTitle") {
            editor_facts.push_directive_prefix("accTitle");
            if let Some(value) = parse_key_colon_value_spanned(line, line_start, "accTitle") {
                push_timeline_payload_fact(
                    &mut editor_facts,
                    value.text,
                    value.start,
                    "timeline accessibility title",
                    EditorSemanticKind::String,
                );
            }
            db.acc_title = v;
            continue;
        }
        if let Some(v) = parse_key_colon_value_hash_or_semi(line, "accDescr") {
            editor_facts.push_directive_prefix("accDescr");
            if let Some(value) = parse_key_colon_value_spanned(line, line_start, "accDescr") {
                push_timeline_payload_fact(
                    &mut editor_facts,
                    value.text,
                    value.start,
                    "timeline accessibility description",
                    EditorSemanticKind::String,
                );
            }
            db.acc_descr = v;
            continue;
        }
        if let Some(v) = parse_acc_descr_block_spanned(&mut lines, line, line_start) {
            editor_facts.push_directive_prefix("accDescr");
            push_timeline_payload_fact_spanned(
                &mut editor_facts,
                &v.text,
                v.span,
                "timeline accessibility description",
                EditorSemanticKind::String,
            );
            db.acc_descr = v.text;
            continue;
        }
        if let Some(v) = parse_section_value(line) {
            if let Some(value) = parse_section_value_spanned(line, line_start) {
                editor_facts.push_symbol(EditorSemanticSymbol::outline(
                    value.text.to_string(),
                    Some("timeline section".to_string()),
                    EditorSemanticKind::Namespace,
                    SourceSpan::new(line_start, line_start + line.len()),
                    SourceSpan::new(value.start, value.end),
                ));
            }
            db.add_section(&v);
            continue;
        }

        let trimmed = stripped;
        let trimmed_start = line_start + line.len().saturating_sub(trimmed.len());
        if trimmed.starts_with(':') {
            let events = split_events_from_colon_whitespace_spanned(trimmed, trimmed_start)
                .map_err(|error| TimelineSemanticFailure::new(error, editor_facts.clone()))?;
            for e in events {
                db.add_event(&e.text)
                    .map_err(|error| TimelineSemanticFailure::new(error, editor_facts.clone()))?;
                push_timeline_payload_fact_spanned(
                    &mut editor_facts,
                    &e.text,
                    e.span,
                    "timeline event",
                    EditorSemanticKind::String,
                );
            }
            continue;
        }

        let mut end = trimmed.len();
        for (i, ch) in trimmed.char_indices() {
            if ch == ':' || ch == '#' {
                end = i;
                break;
            }
        }
        let period = trimmed[..end].to_string();
        if period.trim().is_empty() {
            continue;
        }
        let task_name = period.trim();
        let task_start = trimmed_start + period.find(task_name).unwrap_or(0);
        let task_span = SourceSpan::new(task_start, task_start + task_name.len());
        editor_facts.push_expected_syntax(EditorExpectedSyntax::new(
            EditorExpectedSyntaxKind::Payload,
            task_span,
        ));
        editor_facts.push_symbol(EditorSemanticSymbol::outline(
            task_name.to_string(),
            Some("timeline task".to_string()),
            EditorSemanticKind::Event,
            SourceSpan::new(trimmed_start, line_start + line.len()),
            task_span,
        ));
        db.add_task(&period);

        let rest = &trimmed[end..];
        if rest.starts_with('#') {
            continue;
        }
        if rest.is_empty() {
            continue;
        }
        if rest.starts_with(':') {
            let rest_start = trimmed_start + end;
            let events = split_events_from_colon_whitespace_spanned(rest, rest_start)
                .map_err(|error| TimelineSemanticFailure::new(error, editor_facts.clone()))?;
            for e in events {
                db.add_event(&e.text)
                    .map_err(|error| TimelineSemanticFailure::new(error, editor_facts.clone()))?;
                push_timeline_payload_fact_spanned(
                    &mut editor_facts,
                    &e.text,
                    e.span,
                    "timeline event",
                    EditorSemanticKind::String,
                );
            }
            continue;
        }
        return Err(TimelineSemanticFailure::new(
            Error::diagram_parse_exact(
                meta.diagram_type.clone(),
                format!("unrecognized statement: {trimmed}"),
                SourceSpan::new(trimmed_start + end, line_start + line.len()),
            ),
            editor_facts,
        ));
    }

    let model = header_seen.then(|| TimelineDiagramRenderModel {
        title: (!db.title.is_empty()).then_some(db.title),
        acc_title: (!db.acc_title.is_empty()).then_some(db.acc_title),
        acc_descr: (!db.acc_descr.is_empty()).then_some(db.acc_descr),
        sections: db.sections,
        tasks: db.tasks,
        compatibility_output: CompatibilityOutputState::Model,
    });
    Ok(TimelineSemanticSource {
        model,
        editor_facts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorExpectedSyntaxKind, EditorSemanticCompleteness, EditorSemanticDiagnosticKind,
        EditorSemanticKind, EditorSemanticRole, Engine, ParseDiagnosticSpanKind, ParseOptions,
        SourceSpan,
    };
    use futures::executor::block_on;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

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

    #[test]
    fn timeline_event_missing_space_reports_insertion_point() {
        let engine = Engine::new();
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

    #[test]
    fn timeline_editor_facts_expose_parser_backed_spans() {
        let engine = Engine::new();
        let text = "timeline\n\
title My timeline\n\
accTitle: My acc title\n\
accDescr: My acc descr\n\
section alpha\n\
task1: event1\n\
task2: event2: event3\n";
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("timeline", text, ParseOptions::strict())
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

    #[test]
    fn timeline_entrypoints_and_combined_projection_construct_once() {
        let engine = Engine::new();
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
        let standalone_editor = parse_timeline_editor_facts(text, &parsed.meta);

        reset_timeline_syntax_construction_count();
        parse_timeline(text, &parsed.meta).expect("Timeline JSON projection succeeds");
        assert_eq!(timeline_syntax_construction_count(), 1);

        reset_timeline_syntax_construction_count();
        let typed = parse_timeline_model_for_render(text, &parsed.meta)
            .expect("Timeline typed projection succeeds");
        assert_eq!(timeline_syntax_construction_count(), 1);

        reset_timeline_syntax_construction_count();
        parse_timeline_editor_facts(text, &parsed.meta);
        assert_eq!(timeline_syntax_construction_count(), 1);

        reset_timeline_syntax_construction_count();
        let (combined_json, combined_editor) =
            parse_timeline_json_and_editor_facts(text, &parsed.meta)
                .expect("Timeline combined projection succeeds");
        assert_eq!(timeline_syntax_construction_count(), 1);
        assert_eq!(combined_json, parsed.model);
        assert_eq!(combined_editor, standalone_editor);

        assert_eq!(
            render_model_to_compat_json(&typed, &parsed.meta).unwrap(),
            combined_json
        );
        assert_eq!(parsed.model["type"], "timeline");
        assert!(parsed.model["accDescr"].is_null());
    }

    #[test]
    fn timeline_typed_projection_preserves_empty_and_header_only_output_states() {
        let meta = ParseMetadata {
            diagram_type: "timeline".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::MermaidConfig::empty_object(),
            title: None,
        };
        for source in ["", "timeline"] {
            let compat = parse_timeline(source, &meta).unwrap();
            let typed = parse_timeline_model_for_render(source, &meta).unwrap();

            assert_eq!(
                render_model_to_compat_json(&typed, &meta).unwrap(),
                compat,
                "projection drift for {source:?}"
            );
        }
    }

    #[test]
    fn timeline_malformed_event_recovers_prior_parser_facts_once() {
        let engine = Engine::new();
        let text = "timeline\nsection Build\nImplement parser:event\n";
        let colon = text.find(':').expect("malformed event colon");

        reset_timeline_syntax_construction_count();
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("timeline", text, ParseOptions::strict())
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

    #[test]
    fn timeline_eof_terminates_multiline_acc_descr_like_pinned_jison() {
        let text = "timeline\naccDescr {\n  partial description\n";
        let meta = ParseMetadata {
            diagram_type: "timeline".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::MermaidConfig::empty_object(),
            title: None,
        };

        reset_timeline_syntax_construction_count();
        let (model, facts) = parse_timeline_json_and_editor_facts(text, &meta)
            .expect("pinned Jison accepts EOF in the multiline accessibility state");
        assert_eq!(timeline_syntax_construction_count(), 1);
        assert_eq!(model["accDescr"], json!("partial description"));
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "partial description" && symbol.role == EditorSemanticRole::Payload
        }));

        reset_timeline_syntax_construction_count();
        let typed = parse_timeline_model_for_render(text, &meta)
            .expect("typed projection accepts the same pinned Jison input");
        assert_eq!(timeline_syntax_construction_count(), 1);
        assert_eq!(typed.acc_descr.as_deref(), Some("partial description"));
    }
}
