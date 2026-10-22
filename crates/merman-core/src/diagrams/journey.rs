#[cfg(feature = "presentation")]
use crate::PresentationMetadata;
use crate::diagrams::scan::{
    LineCursor, leading_whitespace_len, split_statement_suffix_hash_or_semi,
    starts_with_case_insensitive,
};
use crate::family::CombinedSemanticFailure;
#[cfg(feature = "editor-facts")]
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, family::EditorFactsDemand,
};
use crate::{Error, OperationControl, OperationControlResult, Result, SourceSpan};
#[cfg(feature = "compat-json")]
use serde_json::{Value, json};
#[cfg(test)]
use std::cell::Cell;
use std::collections::BTreeSet;

#[cfg(test)]
thread_local! {
    static JOURNEY_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
fn reset_journey_syntax_construction_count() {
    JOURNEY_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
fn journey_syntax_construction_count() -> usize {
    JOURNEY_SYNTAX_CONSTRUCTION_COUNT.get()
}

fn is_false(v: &bool) -> bool {
    !*v
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JourneyTask {
    pub score: i64,
    #[serde(default, rename = "scoreIsNaN", skip_serializing_if = "is_false")]
    pub score_is_nan: bool,
    #[serde(default)]
    pub people: Vec<String>,
    pub section: String,
    /// Index of the authored section occurrence in `JourneyDiagramModel::sections`.
    ///
    /// `None` represents a task without parser-backed occurrence ownership, including tasks
    /// authored before any section and legacy direct models.
    #[serde(
        default,
        rename = "sectionIndex",
        skip_serializing_if = "Option::is_none"
    )]
    pub section_index: Option<usize>,
    #[serde(rename = "type")]
    pub task_type: String,
    pub task: String,
}

/// Backwards-compatible name for a semantic Journey task.
#[cfg(feature = "presentation")]
pub type JourneyRenderTask = JourneyTask;

/// Canonical meaning of a Journey diagram, without compatibility-output state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct JourneyDiagramModel {
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub sections: Vec<String>,
    pub tasks: Vec<JourneyTask>,
    pub actors: Vec<String>,
}

impl JourneyDiagramModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[cfg(feature = "presentation")]
#[derive(Debug, Clone, serde::Serialize, Default)]
#[serde(transparent)]
pub struct JourneyDiagramRenderModel {
    pub semantic: JourneyDiagramModel,
    #[cfg(feature = "compat-json")]
    #[serde(skip)]
    compatibility_output: CompatibilityOutputState,
}

#[cfg(feature = "presentation")]
impl std::ops::Deref for JourneyDiagramRenderModel {
    type Target = JourneyDiagramModel;

    fn deref(&self) -> &Self::Target {
        &self.semantic
    }
}

#[cfg(feature = "presentation")]
impl std::ops::DerefMut for JourneyDiagramRenderModel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.semantic
    }
}

#[cfg(feature = "presentation")]
impl<'de> serde::Deserialize<'de> for JourneyDiagramRenderModel {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Wire {
            title: Option<String>,
            #[serde(rename = "accTitle")]
            acc_title: Option<String>,
            #[serde(rename = "accDescr")]
            acc_descr: Option<String>,
            #[serde(default)]
            sections: Vec<String>,
            #[serde(default)]
            tasks: Vec<JourneyTask>,
            #[serde(default)]
            actors: Vec<String>,
        }
        let wire = <Wire as serde::Deserialize>::deserialize(deserializer)?;
        Ok(Self {
            semantic: JourneyDiagramModel {
                title: wire.title,
                acc_title: wire.acc_title,
                acc_descr: wire.acc_descr,
                sections: wire.sections,
                tasks: wire.tasks,
                actors: wire.actors,
            },
            #[cfg(feature = "compat-json")]
            compatibility_output: CompatibilityOutputState::Model,
        })
    }
}
#[cfg(all(test, feature = "presentation"))]
mod terminal_wire_tests {
    use super::*;

    #[test]
    fn full_wire_stays_flat_and_preserves_collection_defaults() {
        let full: JourneyDiagramRenderModel =
            serde_json::from_value(serde_json::json!({"title": "Retained"})).unwrap();
        let expected = serde_json::json!({"title": "Retained", "accTitle": null, "accDescr": null, "sections": [], "tasks": [], "actors": []});
        assert_eq!(serde_json::to_value(&full).unwrap(), expected);
        assert_eq!(serde_json::to_value(&full.semantic).unwrap(), expected);
        assert!(
            serde_json::from_value::<JourneyDiagramModel>(serde_json::json!({"title": "Retained"}))
                .is_err()
        );
        let roundtrip: JourneyDiagramRenderModel =
            serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(roundtrip).unwrap(), expected);
    }
}

#[cfg(feature = "compat-json")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CompatibilityOutputState {
    Empty,
    #[default]
    Model,
}

#[cfg(feature = "presentation")]
impl JourneyDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        self.semantic.sanitize_common_db_fields(config);
    }
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

    fn add_task(&mut self, descr: &str, task_data: &str) -> Result<()> {
        let rest = task_data.strip_prefix(':').unwrap_or(task_data);
        let pieces: Vec<&str> = rest.split(':').collect();

        let score_str = pieces.first().copied().unwrap_or("");
        // Mermaid upstream uses JS `Number(...)` for parsing task scores. This means:
        // - whitespace-only => 0
        // - invalid strings => NaN (and Mermaid happily renders an SVG containing `NaN`)
        //
        // JSON snapshots cannot represent NaN, so we model it as `score=0` + `scoreIsNaN=true`,
        // and let the SVG renderer re-emit `NaN` for the relevant face/mouth coordinates.
        let score_trim = score_str.trim();
        let (score, score_is_nan) = if score_trim.is_empty() {
            (0_i64, false)
        } else {
            match score_trim.parse::<f64>() {
                Ok(v) if v.is_finite() => (v as i64, false),
                _ => (0_i64, true),
            }
        };

        let people = if pieces.len() == 1 {
            Vec::new()
        } else {
            pieces
                .get(1)
                .copied()
                .unwrap_or("")
                .split(',')
                .map(|s| s.trim().to_string())
                .collect()
        };

        self.tasks.push(JourneyTask {
            score,
            score_is_nan,
            people,
            section: self.current_section.clone(),
            section_index: self.current_section_index,
            task_type: self.current_section.clone(),
            task: descr.to_string(),
        });
        Ok(())
    }

    fn actors_sorted(&self) -> Vec<String> {
        let mut set = BTreeSet::<String>::new();
        for t in &self.tasks {
            for p in &t.people {
                set.insert(p.clone());
            }
        }
        set.into_iter().collect()
    }
}

struct JourneySemanticSource {
    model: JourneyDiagramModel,
    #[cfg(feature = "compat-json")]
    compatibility_output: CompatibilityOutputState,
    #[cfg(feature = "editor-facts")]
    editor_facts: Option<EditorSemanticFacts>,
}

#[cfg(feature = "presentation")]
impl JourneySemanticSource {
    fn into_render_model(self) -> JourneyDiagramRenderModel {
        JourneyDiagramRenderModel {
            semantic: self.model,
            #[cfg(feature = "compat-json")]
            compatibility_output: self.compatibility_output,
        }
    }
}

fn parse_keyword_arg_one_ws(line: &str, keyword: &str) -> Option<String> {
    let t = line.trim_start();
    if !starts_with_case_insensitive(t, keyword) {
        return None;
    }
    let after = &t[keyword.len()..];
    let ws = after.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let rest = &after[ws.len_utf8()..];
    Some(split_statement_suffix_hash_or_semi(rest).to_string())
}

fn parse_key_colon_value(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start();
    if !starts_with_case_insensitive(t, key) {
        return None;
    }
    let rest = t[key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?;
    Some(split_statement_suffix_hash_or_semi(rest).trim().to_string())
}

struct JourneyBlockText {
    text: String,
    span: SourceSpan,
    #[cfg(feature = "editor-facts")]
    opening: SourceSpan,
    closing: Option<SourceSpan>,
}

fn parse_acc_descr_block_spanned(
    lines: &mut LineCursor<'_>,
    first_line: &str,
    first_line_start: usize,
    control: &OperationControl,
) -> OperationControlResult<Option<JourneyBlockText>> {
    control.checkpoint()?;
    let t = first_line.trim_start();
    if !starts_with_case_insensitive(t, "accDescr") {
        return Ok(None);
    }
    let rest = t["accDescr".len()..].trim_start();
    let Some(rest) = rest.strip_prefix('{') else {
        return Ok(None);
    };
    let Some(open) = first_line.find('{') else {
        return Ok(None);
    };
    let content_start = first_line_start + open + 1;

    let mut buf = String::new();
    if let Some(end) = rest.find('}') {
        buf.push_str(&rest[..end]);
        let closing = SourceSpan::new(content_start + end, content_start + end + 1);
        lines.resume_same_line_at(closing.end);
        return Ok(Some(JourneyBlockText {
            text: buf.trim().to_string(),
            span: SourceSpan::new(content_start, content_start + end),
            #[cfg(feature = "editor-facts")]
            opening: SourceSpan::new(content_start - 1, content_start),
            closing: Some(closing),
        }));
    }
    buf.push_str(rest);
    buf.push('\n');

    let mut content_end = lines.offset();
    let mut closing = None;
    while let Some((line, line_start)) = lines.next_line() {
        control.checkpoint()?;
        if let Some(end) = line.find('}') {
            buf.push_str(&line[..end]);
            content_end = line_start + end;
            closing = Some(SourceSpan::new(content_end, content_end + 1));
            lines.resume_same_line_at(content_end + 1);
            break;
        }
        buf.push_str(line);
        buf.push('\n');
        content_end = line_start + line.len();
    }
    Ok(Some(JourneyBlockText {
        text: buf.trim().to_string(),
        span: SourceSpan::new(content_start, content_end.max(content_start)),
        #[cfg(feature = "editor-facts")]
        opening: SourceSpan::new(content_start - 1, content_start),
        closing,
    }))
}

fn strip_comment_prefix(line: &str) -> &str {
    let t = line.trim_start();
    if t.starts_with('#') {
        return "";
    }
    if t.starts_with("%%") && !t.starts_with("%%{") {
        return "";
    }
    split_statement_suffix_hash_or_semi(line)
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_journey(code: &str, meta: &PresentationMetadata) -> Result<Value> {
    let source = construct_journey_semantic_source(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
    )
    .map_err(CombinedSemanticFailure::into_error)?;
    render_model_to_compat_json(&source.into_render_model(), meta)
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub(crate) fn parse_journey_json_and_editor_facts(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<crate::family::CombinedSemanticParse> {
    let construction = construct_journey_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?;
    Ok(crate::family::CombinedSemanticParse::from_construction(
        construction,
        |mut source| {
            let editor_facts = source.editor_facts.take().unwrap_or_default();
            let model = render_model_to_compat_json(&source.into_render_model(), meta);
            (model, editor_facts)
        },
        CombinedSemanticFailure::into_parts,
    ))
}

pub(crate) fn parse_journey_semantic_model_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<crate::family::SemanticConstruction<JourneyDiagramModel>> {
    let construction = construct_journey_semantic_source_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )?;
    Ok(match construction {
        Ok(source) => {
            #[cfg(feature = "editor-facts")]
            let editor_facts = source.editor_facts;
            let mut model = source.model;
            model.sanitize_common_db_fields(config);
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
pub(crate) fn render_model_to_compat_json(
    model: &JourneyDiagramRenderModel,
    meta: &PresentationMetadata,
) -> Result<Value> {
    if model.compatibility_output == CompatibilityOutputState::Empty {
        return Ok(json!({}));
    }
    let mut tasks =
        serde_json::to_value(&model.tasks).expect("Journey tasks must remain JSON-serializable");
    for task in tasks
        .as_array_mut()
        .expect("Journey tasks must serialize to an array")
    {
        if let Some(task) = task.as_object_mut() {
            task.remove("sectionIndex");
        }
    }
    Ok(json!({
        "type": meta.diagram_type,
        "title": &model.title,
        "accTitle": &model.acc_title,
        "accDescr": &model.acc_descr,
        "sections": &model.sections,
        "tasks": tasks,
        "actors": &model.actors,
    }))
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_journey_model_for_render(
    code: &str,
    meta: &PresentationMetadata,
) -> Result<JourneyDiagramRenderModel> {
    construct_journey_semantic_source(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
    )
    .map(JourneySemanticSource::into_render_model)
    .map_err(CombinedSemanticFailure::into_error)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_journey_model_for_render_controlled(
    code: &str,
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<JourneyDiagramRenderModel>> {
    let construction = construct_journey_semantic_source_controlled(
        code,
        &meta.diagram_type,
        #[cfg(feature = "editor-facts")]
        EditorFactsDemand::Omit,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        Err(failure) => return Ok(Err(failure.into_error())),
    };
    control.checkpoint()?;
    Ok(Ok(source.into_render_model()))
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn construct_journey_semantic_source(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
) -> std::result::Result<JourneySemanticSource, CombinedSemanticFailure> {
    construct_journey_semantic_source_controlled(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        &OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
}

fn construct_journey_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<JourneySemanticSource, CombinedSemanticFailure>> {
    control.checkpoint()?;
    #[cfg(test)]
    JOURNEY_SYNTAX_CONSTRUCTION_COUNT.set(JOURNEY_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    parse_journey_semantic_source(
        code,
        diagram_type,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        control,
    )
}

fn parse_journey_semantic_source(
    code: &str,
    diagram_type: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<std::result::Result<JourneySemanticSource, CombinedSemanticFailure>> {
    control.checkpoint()?;
    let mut db = JourneyDb::default();
    #[cfg(feature = "editor-facts")]
    let mut editor_facts = editor_demand
        .needs_recovery()
        .then(EditorSemanticFacts::new);
    let mut lines = LineCursor::new(code);
    let mut header_seen = false;
    let mut first_error = None;

    while let Some((line, line_start)) = lines.next_line() {
        control.checkpoint()?;
        let stripped = strip_comment_prefix(line);
        let t = stripped.trim();
        if t.is_empty() {
            continue;
        }

        if !header_seen {
            if starts_with_case_insensitive(t, "journey") {
                header_seen = true;
                let header_start = line_start + line.find(t).unwrap_or(0);
                let after_header = &t["journey".len()..];
                let rest = after_header.trim_start();
                if !rest.is_empty() {
                    let start = header_start
                        + "journey".len()
                        + after_header.len().saturating_sub(rest.len());
                    first_error.get_or_insert_with(|| {
                        Error::diagram_parse_exact(
                            diagram_type.to_string(),
                            "unexpected content after journey header",
                            SourceSpan::new(start, start + rest.len()),
                        )
                    });
                    continue;
                }
                continue;
            }
            let start = line_start + leading_whitespace_len(line);
            first_error.get_or_insert_with(|| {
                Error::diagram_parse_exact(
                    diagram_type.to_string(),
                    "expected journey header",
                    SourceSpan::new(start, line_start + line.len()),
                )
            });
            continue;
        }

        if let Some(v) = parse_keyword_arg_one_ws(stripped, "title") {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("title");
                if let Some(value) = spanned_keyword_value(line, line_start, "title") {
                    push_journey_payload_fact(
                        facts,
                        value,
                        "journey title",
                        EditorSemanticKind::String,
                    );
                }
            }
            db.title = v;
            continue;
        }
        if let Some(v) = parse_key_colon_value(stripped, "accTitle") {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("accTitle");
                if let Some(value) = spanned_colon_value(line, line_start, "accTitle") {
                    push_journey_payload_fact(
                        facts,
                        value,
                        "journey accessibility title",
                        EditorSemanticKind::String,
                    );
                }
            }
            db.acc_title = v;
            continue;
        }
        if let Some(v) = parse_key_colon_value(stripped, "accDescr") {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("accDescr");
                if let Some(value) = spanned_colon_value(line, line_start, "accDescr") {
                    push_journey_payload_fact(
                        facts,
                        value,
                        "journey accessibility description",
                        EditorSemanticKind::String,
                    );
                }
            }
            db.acc_descr = v;
            continue;
        }
        if let Some(v) = parse_acc_descr_block_spanned(&mut lines, stripped, line_start, control)? {
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
            {
                facts.push_directive_prefix("accDescr");
                push_journey_payload_fact_spanned(
                    facts,
                    &v.text,
                    v.span,
                    "journey accessibility description",
                    EditorSemanticKind::String,
                );
            }
            if v.closing.is_none() {
                #[cfg(feature = "editor-facts")]
                if let Some(facts) = &mut editor_facts {
                    facts.mark_recovered_from_parse_error(
                        "journey parser recovered from unterminated accDescr block",
                        Some(SourceSpan::new(v.opening.start, v.span.end)),
                    );
                }
                first_error.get_or_insert_with(|| {
                    Error::diagram_parse_insertion_point(
                        diagram_type.to_string(),
                        "unterminated accDescr block",
                        v.span.end,
                    )
                });
            }
            db.acc_descr = v.text;
            continue;
        }
        if let Some(v) = parse_keyword_arg_one_ws(stripped, "section") {
            let v = v.split(':').next().unwrap_or("").to_string();
            #[cfg(feature = "editor-facts")]
            if let Some(facts) = editor_facts
                .as_mut()
                .filter(|_| editor_demand.captures_rich())
                && let Some(value) = spanned_keyword_value(line, line_start, "section")
            {
                let section_text = value.text.split(':').next().unwrap_or("").trim();
                if !section_text.is_empty() {
                    let section_start = value.start + value.text.find(section_text).unwrap_or(0);
                    facts.push_symbol(EditorSemanticSymbol::outline(
                        section_text.to_string(),
                        Some("journey section".to_string()),
                        EditorSemanticKind::Namespace,
                        SourceSpan::new(line_start, line_start + line.len()),
                        SourceSpan::new(section_start, section_start + section_text.len()),
                    ));
                }
            }
            db.add_section(&v);
            continue;
        }

        let colon = stripped.find(':');
        let task_name_source = colon.map_or(stripped, |colon| &stripped[..colon]);
        let task_name = task_name_source.trim();
        if task_name.is_empty() {
            let start = line_start + line.find(t).unwrap_or(0);
            first_error.get_or_insert_with(|| {
                Error::diagram_parse_exact(
                    diagram_type.to_string(),
                    "expected journey task name",
                    SourceSpan::new(start, start + t.len()),
                )
            });
            continue;
        }
        #[cfg(feature = "editor-facts")]
        if let Some(facts) = editor_facts
            .as_mut()
            .filter(|_| editor_demand.captures_rich())
        {
            let task_start = line_start + task_name_source.find(task_name).unwrap_or(0);
            let task_end = task_start + task_name.len();
            facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                SourceSpan::new(task_start, task_end),
            ));
            facts.push_symbol(EditorSemanticSymbol::outline(
                task_name.to_string(),
                Some("journey task".to_string()),
                EditorSemanticKind::Event,
                SourceSpan::new(line_start, line_start + line.len()),
                SourceSpan::new(task_start, task_end),
            ));
        }

        let Some(colon) = colon else {
            let insertion = line_start + stripped.trim_end().len();
            first_error.get_or_insert_with(|| {
                Error::diagram_parse_insertion_point(
                    diagram_type.to_string(),
                    "expected journey task data after ':'",
                    insertion,
                )
            });
            continue;
        };
        let task_data = &stripped[colon..];
        if task_data.len() == ':'.len_utf8() {
            first_error.get_or_insert_with(|| {
                Error::diagram_parse_insertion_point(
                    diagram_type.to_string(),
                    "expected journey task data after ':'",
                    line_start + colon + ':'.len_utf8(),
                )
            });
            continue;
        }

        #[cfg(feature = "editor-facts")]
        if let Some(facts) = editor_facts
            .as_mut()
            .filter(|_| editor_demand.captures_rich())
        {
            let rest_source = &stripped[colon + ':'.len_utf8()..];
            let rest = rest_source.trim_start();
            let rest_start =
                line_start + colon + ':'.len_utf8() + rest_source.len().saturating_sub(rest.len());
            let score_end = rest.find(':').unwrap_or(rest.len());
            let score_text = rest[..score_end].trim();
            if !score_text.is_empty() {
                let score_start = rest_start + rest[..score_end].find(score_text).unwrap_or(0);
                facts.push_expected_syntax(EditorExpectedSyntax::new(
                    EditorExpectedSyntaxKind::Payload,
                    SourceSpan::new(score_start, score_start + score_text.len()),
                ));
                facts.push_symbol(EditorSemanticSymbol::payload(
                    score_text.to_string(),
                    Some("journey score".to_string()),
                    EditorSemanticKind::String,
                    SourceSpan::new(score_start, score_start + score_text.len()),
                    SourceSpan::new(score_start, score_start + score_text.len()),
                ));
            }

            if score_end < rest.len() {
                let people_source = &rest[score_end + ':'.len_utf8()..];
                let people = people_source.trim();
                if !people.is_empty() {
                    let people_start = rest_start
                        + score_end
                        + ':'.len_utf8()
                        + people_source.find(people).unwrap_or(0);
                    facts.push_expected_syntax(EditorExpectedSyntax::new(
                        EditorExpectedSyntaxKind::Payload,
                        SourceSpan::new(people_start, people_start + people.len()),
                    ));
                    facts.push_symbol(EditorSemanticSymbol::payload(
                        people.to_string(),
                        Some("journey people".to_string()),
                        EditorSemanticKind::String,
                        SourceSpan::new(people_start, people_start + people.len()),
                        SourceSpan::new(people_start, people_start + people.len()),
                    ));
                }
            }
        }

        if let Err(error) = db.add_task(task_name_source, task_data) {
            first_error.get_or_insert(error);
        }
    }

    if let Some(error) = first_error {
        return Ok(Err(CombinedSemanticFailure::parser_recovery(
            #[cfg(feature = "editor-facts")]
            "journey",
            error,
            #[cfg(feature = "editor-facts")]
            editor_facts,
        )));
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
    Ok(Ok(JourneySemanticSource {
        model,
        #[cfg(feature = "compat-json")]
        compatibility_output: if header_seen {
            CompatibilityOutputState::Model
        } else {
            CompatibilityOutputState::Empty
        },
        #[cfg(feature = "editor-facts")]
        editor_facts,
    }))
}

#[cfg(feature = "editor-facts")]
fn spanned_keyword_value<'a>(
    line: &'a str,
    line_start: usize,
    keyword: &str,
) -> Option<EditorPayloadSpan<'a>> {
    let trimmed = line.trim_start();
    if !starts_with_case_insensitive(trimmed, keyword) {
        return None;
    }
    let after = &trimmed[keyword.len()..];
    let ws = after.chars().next()?;
    if !ws.is_whitespace() {
        return None;
    }
    let value = split_statement_suffix_hash_or_semi(&after[ws.len_utf8()..]).trim();
    if value.is_empty() {
        return None;
    }
    let value_rel = line.find(value)?;
    Some(EditorPayloadSpan {
        text: value,
        start: line_start + value_rel,
        end: line_start + value_rel + value.len(),
    })
}

#[cfg(feature = "editor-facts")]
fn spanned_colon_value<'a>(
    line: &'a str,
    line_start: usize,
    key: &str,
) -> Option<EditorPayloadSpan<'a>> {
    let trimmed = line.trim_start();
    if !starts_with_case_insensitive(trimmed, key) {
        return None;
    }
    let rest = trimmed[key.len()..].trim_start();
    let rest = rest.strip_prefix(':')?;
    let value = split_statement_suffix_hash_or_semi(rest).trim();
    if value.is_empty() {
        return None;
    }
    let value_rel = line.find(value)?;
    Some(EditorPayloadSpan {
        text: value,
        start: line_start + value_rel,
        end: line_start + value_rel + value.len(),
    })
}

#[cfg(feature = "editor-facts")]
fn push_journey_payload_fact(
    facts: &mut EditorSemanticFacts,
    span: EditorPayloadSpan<'_>,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        SourceSpan::new(span.start, span.end),
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        span.text.to_string(),
        Some(detail.to_string()),
        kind,
        SourceSpan::new(span.start, span.end),
        SourceSpan::new(span.start, span.end),
    ));
}

#[cfg(feature = "editor-facts")]
fn push_journey_payload_fact_spanned(
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

#[cfg(feature = "editor-facts")]
struct EditorPayloadSpan<'a> {
    text: &'a str,
    start: usize,
    end: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn journey_title_definition_parses() {
        let model = parse("journey\ntitle Adding journey diagram functionality to mermaid");
        assert_eq!(
            model["title"],
            json!("Adding journey diagram functionality to mermaid")
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_parses_acc_descr_block_and_title_and_acc_title() {
        let model = parse(
            "journey\n\
accDescr {\n\
  A user journey for\n\
  family shopping\n\
}\n\
title Adding journey diagram functionality to mermaid\n\
accTitle: Adding acc journey diagram functionality to mermaid\n\
section Order from website\n",
        );
        assert_eq!(
            model["accDescr"],
            json!("A user journey for\nfamily shopping")
        );
        assert_eq!(
            model["title"],
            json!("Adding journey diagram functionality to mermaid")
        );
        assert_eq!(
            model["accTitle"],
            json!("Adding acc journey diagram functionality to mermaid")
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_parses_acc_title_without_description() {
        let model = parse(
            "journey\n\
accTitle: The title\n\
section Order from website\n",
        );
        assert_eq!(model["accTitle"], json!("The title"));
        assert!(model["accDescr"].is_null());
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_parses_acc_descr_single_line() {
        let model = parse(
            "journey\n\
accDescr: A user journey for family shopping\n\
title Adding journey diagram functionality to mermaid\n\
section Order from website\n",
        );
        assert_eq!(
            model["accDescr"],
            json!("A user journey for family shopping")
        );
        assert_eq!(
            model["title"],
            json!("Adding journey diagram functionality to mermaid")
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_editor_facts_expose_parser_backed_spans() {
        let engine = MermaidEngine::new();
        let text = "journey\n\
title Adding journey diagram functionality to mermaid\n\
accTitle: Adding acc journey diagram functionality to mermaid\n\
accDescr: A user journey for family shopping\n\
section Order from website\n\
A task: 5: Alice, Bob\n";
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("journey", text)
            .unwrap()
            .unwrap();

        assert!(
            facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "accTitle")
        );
        assert!(
            facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "accDescr")
        );
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Order from website"
                && symbol.kind == EditorSemanticKind::Namespace
                && symbol.role == EditorSemanticRole::Outline
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "A task"
                && symbol.kind == EditorSemanticKind::Event
                && symbol.role == EditorSemanticRole::Outline
        }));

        let task_start = text.find("A task").unwrap();
        let score_start = text.find("5: Alice, Bob").unwrap();
        let people_start = text.find("Alice, Bob").unwrap();

        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(task_start, task_start + "A task".len())
        }));
        assert!(!facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
                && expected.span == SourceSpan::new(task_start, task_start + "A task".len())
        }));
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(score_start, score_start + 1)
        }));
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::Payload
                && expected.span == SourceSpan::new(people_start, people_start + "Alice, Bob".len())
        }));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_entrypoints_and_combined_projection_construct_once() {
        let engine = MermaidEngine::new();
        let text = concat!(
            "journey\n",
            "title Checkout journey\n",
            "accTitle: Checkout\n",
            "section Payment\n",
            "Confirm order: 5: Alice, Bob\n",
        );
        let parsed = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect("standalone Journey JSON parse succeeds")
            .expect("standalone Journey JSON parse returns a diagram");
        reset_journey_syntax_construction_count();
        crate::editor::test_work::reset();
        parse_journey(text, &parsed.meta).expect("Journey JSON projection succeeds");
        assert_eq!(journey_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());

        reset_journey_syntax_construction_count();
        let typed = parse_journey_model_for_render(text, &parsed.meta)
            .expect("Journey typed projection succeeds");
        assert_eq!(journey_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());

        reset_journey_syntax_construction_count();
        let (combined_json, combined_editor) = crate::family::test_support::into_result(
            parse_journey_json_and_editor_facts(text, &parsed.meta, &OperationControl::new()),
        )
        .expect("Journey combined projection succeeds");
        assert_eq!(journey_syntax_construction_count(), 1);
        assert_eq!(combined_json, parsed.model);
        assert!(!combined_editor.symbols.is_empty());

        assert_eq!(
            render_model_to_compat_json(&typed, &parsed.meta).unwrap(),
            combined_json
        );
        assert_eq!(parsed.model["type"], "journey");
        assert!(parsed.model["accDescr"].is_null());
        crate::editor::test_work::reset();
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn journey_typed_projection_preserves_empty_and_header_only_output_states() {
        let meta = PresentationMetadata {
            diagram_type: "journey".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        };
        for source in ["", "journey"] {
            let compat = parse_journey(source, &meta).unwrap();
            let typed = parse_journey_model_for_render(source, &meta).unwrap();

            assert_eq!(
                render_model_to_compat_json(&typed, &meta).unwrap(),
                compat,
                "projection drift for {source:?}"
            );
        }
    }

    #[test]
    fn journey_semantic_model_omits_editor_facts_and_compatibility_state() {
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
        reset_journey_syntax_construction_count();
        let source = concat!(
            "journey\n",
            "title Checkout\n",
            "section Repeat\n",
            "First task: 5: Alice\n",
            "section Repeat\n",
            "Second task: invalid: Bob\n",
        );
        let semantic = parse_journey_semantic_model_controlled(
            source,
            "journey",
            &crate::MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(journey_syntax_construction_count(), 1);
        #[cfg(feature = "editor-facts")]
        assert_eq!(crate::editor::test_work::read(), Default::default());
        assert_eq!(semantic.sections, ["Repeat", "Repeat"]);
        assert_eq!(
            semantic
                .tasks
                .iter()
                .map(|task| task.section_index)
                .collect::<Vec<_>>(),
            [Some(0), Some(1)]
        );
        assert!(semantic.tasks[1].score_is_nan);
        assert_eq!(semantic.actors, ["Alice", "Bob"]);

        #[cfg(feature = "presentation")]
        {
            let meta = crate::PresentationMetadata {
                diagram_type: "journey".to_string(),
                config: crate::MermaidConfig::default(),
                effective_config: crate::presentation::PresentationConfig::default(),
                title: None,
            };
            let render = parse_journey_model_for_render(source, &meta).unwrap();
            assert_eq!(
                serde_json::to_value(&semantic).unwrap(),
                serde_json::to_value(render).unwrap()
            );
        }
        #[cfg(feature = "editor-facts")]
        crate::editor::test_work::reset();
    }

    #[test]
    fn journey_semantic_empty_input_has_no_compatibility_sentinel() {
        let semantic = parse_journey_semantic_model_controlled(
            "",
            "journey",
            &crate::MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        #[cfg(feature = "compat-json")]
        let meta = PresentationMetadata {
            diagram_type: "journey".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        };

        assert_eq!(
            serde_json::to_value(semantic).unwrap(),
            json!({
                "title": null,
                "accTitle": null,
                "accDescr": null,
                "sections": [],
                "tasks": [],
                "actors": [],
            })
        );
        #[cfg(feature = "compat-json")]
        assert_eq!(parse_journey("", &meta).unwrap(), json!({}));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_malformed_task_recovers_prior_statement_facts_once() {
        let engine = MermaidEngine::new();
        let text = "journey\nsection Checkout\nBroken task\n";
        let task_start = text.find("Broken task").expect("malformed task");
        let insertion = task_start + "Broken task".len();

        reset_journey_syntax_construction_count();
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("journey", text)
            .expect("Journey editor recovery succeeds")
            .expect("Journey editor facts are available");

        assert_eq!(journey_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Checkout" && symbol.role == EditorSemanticRole::Outline
        }));
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Broken task" && symbol.role == EditorSemanticRole::Outline
        }));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(SourceSpan::new(insertion, insertion))
        }));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_task_without_data_reports_insertion_point() {
        let engine = MermaidEngine::new();
        let text = "journey\nTask:\n";
        let colon = text.find(':').expect("task colon");
        let error = engine
            .parse_diagram_sync(text, ParseOptions::strict())
            .expect_err("a bare task colon is not a Journey taskData token");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected Journey parse error");
        };

        assert_eq!(diagnostic.message(), "expected journey task data after ':'");
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
    fn journey_multiline_acc_descr_fact_preserves_source_span() {
        let text = concat!(
            "journey\n",
            "accDescr {\n",
            "  First line\n",
            "  Second line\n",
            "}\n",
        );
        let facts = crate::family::test_support::editor_facts(
            parse_journey_json_and_editor_facts,
            text,
            &PresentationMetadata {
                diagram_type: "journey".to_string(),
                config: crate::MermaidConfig::empty_object(),
                effective_config: crate::presentation::PresentationConfig::empty_object(),
                title: None,
            },
        );
        let description = facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some("journey accessibility description"))
            .expect("multiline accessibility description fact");
        let content_start = text.find('{').expect("opening brace") + 1;
        let content_end = text.rfind('}').expect("closing brace");

        assert_eq!(description.name, "First line\n  Second line");
        assert_eq!(
            description.selection,
            SourceSpan::new(content_start, content_end)
        );
        assert_eq!(description.role, EditorSemanticRole::Payload);
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_allows_section_titles_with_br_variants() {
        let model = parse(
            "journey\n\
title Adding gantt diagram functionality to mermaid\n\
section Line1<br>Line2<br/>Line3</br />Line4<br\t/>Line5\n",
        );
        let sections = model["sections"].as_array().unwrap();
        assert_eq!(sections.len(), 1);
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_parses_tasks_and_people_like_upstream() {
        let model = parse(
            "journey\n\
title Adding journey diagram functionality to mermaid\n\
section Documentation\n\
A task: 5: Alice, Bob, Charlie\n\
B task: 3:Bob, Charlie\n\
C task: 5\n\
D task: 5: Charlie, Alice\n\
E task: 5:\n\
section Another section\n\
P task: 5:\n\
Q task: 5:\n\
R task: 5:\n",
        );

        let tasks = model["tasks"].as_array().unwrap();
        assert_eq!(tasks.len(), 8);

        assert_eq!(
            tasks[0],
            json!({
                "score": 5,
                "people": ["Alice", "Bob", "Charlie"],
                "section": "Documentation",
                "task": "A task",
                "type": "Documentation",
            })
        );
        assert_eq!(
            tasks[1],
            json!({
                "score": 3,
                "people": ["Bob", "Charlie"],
                "section": "Documentation",
                "task": "B task",
                "type": "Documentation",
            })
        );
        assert_eq!(
            tasks[2],
            json!({
                "score": 5,
                "people": [],
                "section": "Documentation",
                "task": "C task",
                "type": "Documentation",
            })
        );
        assert_eq!(
            tasks[3],
            json!({
                "score": 5,
                "people": ["Charlie", "Alice"],
                "section": "Documentation",
                "task": "D task",
                "type": "Documentation",
            })
        );
        assert_eq!(
            tasks[4],
            json!({
                "score": 5,
                "people": [""],
                "section": "Documentation",
                "task": "E task",
                "type": "Documentation",
            })
        );
        assert_eq!(
            tasks[5],
            json!({
                "score": 5,
                "people": [""],
                "section": "Another section",
                "task": "P task",
                "type": "Another section",
            })
        );
        assert_eq!(
            tasks[6],
            json!({
                "score": 5,
                "people": [""],
                "section": "Another section",
                "task": "Q task",
                "type": "Another section",
            })
        );
        assert_eq!(
            tasks[7],
            json!({
                "score": 5,
                "people": [""],
                "section": "Another section",
                "task": "R task",
                "type": "Another section",
            })
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn journey_typed_tasks_retain_duplicate_section_occurrences() {
        let source = concat!(
            "journey\n",
            "section Repeated\n",
            "First: 5: Alice\n",
            "section Repeated\n",
            "Second: 3: Bob\n",
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("duplicate Journey sections should parse")
            .expect("Journey source should be detected");
        let crate::diagram::RenderSemanticModel::Journey(model) = parsed.model() else {
            panic!("expected a Journey render model");
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
            [Some(0), Some(1)]
        );
        assert_eq!(
            serde_json::to_value(&model.tasks[0]).expect("typed task should serialize")["sectionIndex"],
            json!(0)
        );

        let compat = render_model_to_compat_json(model, parsed.metadata())
            .expect("Journey compatibility projection should serialize");
        assert!(compat["tasks"][0].get("sectionIndex").is_none());
    }

    #[test]
    fn journey_db_tasks_and_actors_should_be_added_matches_upstream_spec() {
        let mut db = JourneyDb {
            acc_title: "Shopping".to_string(),
            acc_descr: "A user journey for family shopping".to_string(),
            ..JourneyDb::default()
        };
        db.add_section("Journey to the shops");
        db.add_task("Get car keys", ":5:Dad").unwrap();
        db.add_task("Go to car", ":3:Dad, Mum, Child#1, Child#2")
            .unwrap();
        db.add_task("Drive to supermarket", ":4:Dad").unwrap();
        db.add_section("Do shopping");
        db.add_task("Go shopping", ":5:Mum").unwrap();

        let actors = db.actors_sorted();
        assert_eq!(
            db.tasks
                .iter()
                .map(|t| {
                    json!({
                        "score": t.score,
                        "people": t.people,
                        "section": t.section,
                        "task": t.task,
                        "type": t.task_type,
                    })
                })
                .collect::<Vec<_>>(),
            vec![
                json!({
                    "score": 5,
                    "people": ["Dad"],
                    "section": "Journey to the shops",
                    "task": "Get car keys",
                    "type": "Journey to the shops",
                }),
                json!({
                    "score": 3,
                    "people": ["Dad", "Mum", "Child#1", "Child#2"],
                    "section": "Journey to the shops",
                    "task": "Go to car",
                    "type": "Journey to the shops",
                }),
                json!({
                    "score": 4,
                    "people": ["Dad"],
                    "section": "Journey to the shops",
                    "task": "Drive to supermarket",
                    "type": "Journey to the shops",
                }),
                json!({
                    "score": 5,
                    "people": ["Mum"],
                    "section": "Do shopping",
                    "task": "Go shopping",
                    "type": "Do shopping",
                }),
            ]
        );

        assert_eq!(
            actors,
            vec![
                "Child#1".to_string(),
                "Child#2".to_string(),
                "Dad".to_string(),
                "Mum".to_string()
            ]
        );
        assert_eq!(
            db.sections,
            vec![
                "Journey to the shops".to_string(),
                "Do shopping".to_string()
            ]
        );
    }
}
