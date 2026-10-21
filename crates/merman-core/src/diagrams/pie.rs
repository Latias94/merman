use crate::common_db::LangiumCommonDbFields;
use crate::diagrams::langium_common::{
    LangiumCommonFacts, LangiumCommonParse, parse_langium_common, parse_langium_string,
    push_langium_common_editor_fact, push_langium_common_recovery, strip_langium_inline_comment,
};
use crate::family::EditorFactsDemand;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, SourceSpan, family,
};
#[cfg(feature = "presentation")]
use crate::{ParseMetadata, Result};
#[cfg(feature = "compat-json")]
use serde_json::{Value, json};
use std::collections::HashSet;

/// Canonical meaning of a Pie diagram, without compatibility-output state.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct PieDiagramModel {
    #[serde(rename = "showData")]
    pub show_data: bool,
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub sections: Vec<PieSection>,
}

impl PieDiagramModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[cfg(feature = "presentation")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct PieDiagramRenderModel {
    #[serde(rename = "showData")]
    pub show_data: bool,
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub sections: Vec<PieRenderSection>,
    #[cfg(feature = "compat-json")]
    #[serde(skip)]
    compatibility_output: CompatibilityOutputState,
}

#[cfg(feature = "compat-json")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CompatibilityOutputState {
    Empty,
    #[default]
    Model,
}

#[cfg(feature = "presentation")]
impl PieDiagramRenderModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PieSection {
    pub label: String,
    pub value: f64,
}

/// Backwards-compatible name for a semantic Pie section.
#[cfg(feature = "presentation")]
pub type PieRenderSection = PieSection;

struct PieSemanticSource {
    model: PieDiagramModel,
    #[cfg(feature = "compat-json")]
    compatibility_output: CompatibilityOutputState,
    editor_facts: Option<EditorSemanticFacts>,
}

#[cfg(feature = "presentation")]
impl PieSemanticSource {
    fn into_render_model(self) -> PieDiagramRenderModel {
        PieDiagramRenderModel {
            show_data: self.model.show_data,
            title: self.model.title,
            acc_title: self.model.acc_title,
            acc_descr: self.model.acc_descr,
            sections: self.model.sections,
            #[cfg(feature = "compat-json")]
            compatibility_output: self.compatibility_output,
        }
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_pie(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let source = parse_pie_semantic_source(code, meta)?;
    render_model_to_compat_json(&source.into_render_model(), meta)
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_pie_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<family::CombinedSemanticParse> {
    let construction = construct_pie_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Capture,
        control,
    )?;
    let parsed = family::CombinedSemanticParse::from_construction(
        construction,
        |mut source| {
            let editor_facts = source.editor_facts.take().unwrap_or_default();
            (
                render_model_to_compat_json(&source.into_render_model(), meta),
                editor_facts,
            )
        },
        family::CombinedSemanticFailure::into_parts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &PieDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    if model.compatibility_output == CompatibilityOutputState::Empty {
        return Ok(json!({}));
    }
    Ok(json!({
            "type": meta.diagram_type,
            "showData": model.show_data,
            "title": &model.title,
            "accTitle": &model.acc_title,
            "accDescr": &model.acc_descr,
            "sections": &model.sections,
    }))
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_pie_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<PieDiagramRenderModel> {
    Ok(parse_pie_semantic_source(code, meta)?.into_render_model())
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_pie_model_for_render_controlled(
    code: &str,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<PieDiagramRenderModel>> {
    let construction = construct_pie_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Omit,
        control,
    )?;
    let source = match construction {
        Ok(source) => source,
        Err(failure) => return Ok(Err(failure.into_error())),
    };
    Ok(Ok(source.into_render_model()))
}

pub(crate) fn parse_pie_semantic_model_controlled(
    code: &str,
    diagram_type: &str,
    config: &crate::MermaidConfig,
    editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<PieDiagramModel>> {
    let construction =
        construct_pie_semantic_source_controlled(code, diagram_type, editor_demand, control)?;
    Ok(match construction {
        Ok(source) => {
            let editor_facts = source.editor_facts;
            let mut model = source.model;
            model.sanitize_common_db_fields(config);
            crate::family::SemanticConstruction {
                model: Ok(model),
                editor_facts,
            }
        }
        Err(failure) => match editor_demand {
            EditorFactsDemand::Capture => {
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
        },
    })
}

#[cfg(any(feature = "compat-json", all(test, feature = "presentation")))]
fn parse_pie_semantic_source(code: &str, meta: &ParseMetadata) -> Result<PieSemanticSource> {
    construct_pie_semantic_source_controlled(
        code,
        &meta.diagram_type,
        EditorFactsDemand::Omit,
        &crate::OperationControl::new(),
    )
    .expect("a private parse control cannot be cancelled")
    .map_err(family::CombinedSemanticFailure::into_error)
}

fn construct_pie_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    editor_demand: EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<
    std::result::Result<PieSemanticSource, family::CombinedSemanticFailure>,
> {
    control.checkpoint()?;
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("pie");

    let body = match pie_body_start_controlled(code, control)? {
        PieHeader::Empty => {
            return Ok(Ok(PieSemanticSource {
                model: PieDiagramModel::default(),
                #[cfg(feature = "compat-json")]
                compatibility_output: CompatibilityOutputState::Empty,
                editor_facts: (editor_demand == EditorFactsDemand::Capture)
                    .then(EditorSemanticFacts::new),
            }));
        }
        PieHeader::ExpectedPie(span) => {
            return Ok(Err(family::CombinedSemanticFailure::parser_recovery(
                "pie",
                Error::diagram_parse_exact(diagram_type.to_string(), "expected pie", span),
                EditorSemanticFacts::new(),
            )));
        }
        PieHeader::Body(body) => body,
    };
    let mut offset = body.offset;
    let mut common = LangiumCommonFacts::default();
    let mut parsed_sections = Vec::new();
    let mut editor_facts = match editor_demand {
        EditorFactsDemand::Omit => None,
        EditorFactsDemand::Capture => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            Some(EditorSemanticFacts::new())
        }
    };
    if let (Some(facts), Some(span)) = (&mut editor_facts, body.show_data_span) {
        push_show_data_editor_fact(facts, span);
    }
    let mut first_error = None;

    while offset < code.len() {
        control.checkpoint()?;
        match parse_pie_statement(code, offset) {
            PieStatement::Common(parsed) => {
                if let Some(diagnostic) = &parsed.diagnostic {
                    if let Some(facts) = &mut editor_facts {
                        push_langium_common_recovery(facts, diagnostic);
                    }
                    first_error.get_or_insert_with(|| {
                        Error::diagram_parse_insertion_point(
                            diagram_type.to_string(),
                            diagnostic.message.clone(),
                            diagnostic.span.start,
                        )
                    });
                }
                if let Some(facts) = &mut editor_facts {
                    push_langium_common_editor_fact(facts, &parsed.fact, "pie");
                }
                common.push(parsed.fact);
                offset += parsed.consumed;
            }
            PieStatement::Section {
                section,
                next_offset,
            } => {
                if let Some(facts) = &mut editor_facts {
                    push_pie_section_editor_fact(facts, &section);
                }
                parsed_sections.push(section);
                offset = next_offset;
            }
            PieStatement::Empty { next_offset } => offset = next_offset,
            PieStatement::Unexpected {
                text,
                span,
                next_offset,
            } => {
                if let Some(facts) = &mut editor_facts {
                    facts.mark_recovered_from_parse_error("unexpected pie statement", Some(span));
                }
                first_error.get_or_insert_with(|| {
                    Error::diagram_parse_fallback(
                        diagram_type.to_string(),
                        format!("unexpected pie statement: {text}"),
                    )
                });
                offset = next_offset;
            }
        }
    }

    let mut sections = Vec::new();
    let mut seen = HashSet::new();
    for (index, section) in parsed_sections.into_iter().enumerate() {
        if index % 128 == 0 {
            control.checkpoint()?;
        }
        if section.value < 0.0 {
            if editor_facts.is_some() || first_error.is_none() {
                let message = format!(
                    "\"{}\" has invalid value: {}. Negative values are not allowed in pie charts. All slice values must be >= 0.",
                    section.label, section.value
                );
                if let Some(facts) = &mut editor_facts {
                    facts
                        .mark_recovered_from_parse_error(message.clone(), Some(section.value_span));
                }
                first_error.get_or_insert_with(|| {
                    Error::diagram_parse_exact(
                        diagram_type.to_string(),
                        message,
                        section.value_span,
                    )
                });
            }
            continue;
        }
        if seen.insert(section.label.clone()) {
            sections.push(PieSection {
                label: section.label,
                value: section.value,
            });
        }
    }

    let common = LangiumCommonDbFields::from_facts(&common);
    if let Some(error) = first_error {
        return Ok(Err(family::CombinedSemanticFailure::new(
            error,
            editor_facts.unwrap_or_default(),
        )));
    }

    control.checkpoint()?;
    Ok(Ok(PieSemanticSource {
        model: PieDiagramModel {
            show_data: body.show_data_span.is_some(),
            title: common.title,
            acc_title: common.acc_title,
            acc_descr: common.acc_descr,
            sections,
        },
        #[cfg(feature = "compat-json")]
        compatibility_output: CompatibilityOutputState::Model,
        editor_facts,
    }))
}

enum PieStatement {
    Common(LangiumCommonParse),
    Section {
        section: PieParsedSection,
        next_offset: usize,
    },
    Empty {
        next_offset: usize,
    },
    Unexpected {
        text: String,
        span: SourceSpan,
        next_offset: usize,
    },
}

struct PieParsedSection {
    label: String,
    value: f64,
    statement_span: SourceSpan,
    label_span: SourceSpan,
    value_span: SourceSpan,
}

fn parse_pie_statement(code: &str, offset: usize) -> PieStatement {
    if let Some(parsed) = parse_langium_common(code, offset) {
        return PieStatement::Common(parsed);
    }

    let (line, next_offset) = physical_line(code, offset);
    let visible = strip_inline_comment(line);
    let trimmed = visible.trim();
    if trimmed.is_empty() {
        return PieStatement::Empty { next_offset };
    }

    if let Some(section) = parse_pie_section(line, offset) {
        return PieStatement::Section {
            section,
            next_offset,
        };
    }

    PieStatement::Unexpected {
        text: trimmed.to_string(),
        span: SourceSpan::new(
            offset + visible.find(trimmed).unwrap_or_default(),
            offset + visible.find(trimmed).unwrap_or_default() + trimmed.len(),
        ),
        next_offset,
    }
}

fn parse_pie_section(line: &str, line_start: usize) -> Option<PieParsedSection> {
    let visible = strip_inline_comment(line);
    let leading = visible.len() - visible.trim_start().len();
    let input = &visible[leading..];
    let parsed_label = parse_quoted_string(input, line_start + leading)?;

    let (rest, rest_start) = trim_start_with_offset(parsed_label.rest, parsed_label.rest_start);
    let rest = rest.strip_prefix(':')?;
    let (number_and_trailing, number_start) = trim_start_with_offset(rest, rest_start + 1);
    let number = number_and_trailing.trim_end();
    let value = parse_number_pie(number)?;
    let value_span = SourceSpan::new(number_start, number_start + number.len());

    Some(PieParsedSection {
        label: parsed_label.value,
        value,
        statement_span: SourceSpan::new(line_start, line_start + line.len()),
        label_span: parsed_label.value_span,
        value_span,
    })
}

fn parse_number_pie(input: &str) -> Option<f64> {
    let unsigned = input.strip_prefix('-').unwrap_or(input);
    let (integer, fraction) = match unsigned.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (unsigned, None),
    };
    if integer.is_empty() || !integer.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    match fraction {
        Some(fraction)
            if fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()) =>
        {
            return None;
        }
        None if integer.len() > 1 && integer.starts_with('0') => return None,
        _ => {}
    }
    input.parse().ok()
}

struct ParsedQuotedString<'a> {
    value: String,
    value_span: SourceSpan,
    rest: &'a str,
    rest_start: usize,
}

fn parse_quoted_string(input: &str, input_start: usize) -> Option<ParsedQuotedString<'_>> {
    let parsed = parse_langium_string(input, input_start)?;
    Some(ParsedQuotedString {
        value: parsed.value,
        value_span: parsed.value_span,
        rest: &input[parsed.consumed..],
        rest_start: input_start + parsed.consumed,
    })
}

fn trim_start_with_offset(input: &str, input_start: usize) -> (&str, usize) {
    let trimmed = input.trim_start();
    (trimmed, input_start + input.len() - trimmed.len())
}

fn push_show_data_editor_fact(facts: &mut EditorSemanticFacts, span: SourceSpan) {
    facts.push_directive_prefix("showData");
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        span,
    ));
}

fn push_pie_section_editor_fact(facts: &mut EditorSemanticFacts, section: &PieParsedSection) {
    facts.push_symbol(EditorSemanticSymbol::outline(
        section.label.clone(),
        Some("pie section".to_string()),
        EditorSemanticKind::String,
        section.statement_span,
        section.label_span,
    ));
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        section.value_span,
    ));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PieHeader {
    Empty,
    ExpectedPie(SourceSpan),
    Body(PieBodyStart),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PieBodyStart {
    offset: usize,
    show_data_span: Option<SourceSpan>,
}

fn pie_body_start_controlled(
    code: &str,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<PieHeader> {
    let mut offset = 0usize;
    while offset < code.len() {
        control.checkpoint()?;
        let (line, next_offset) = physical_line(code, offset);
        let visible = strip_inline_comment(line);
        let trimmed = visible.trim_start();
        if trimmed.trim().is_empty() {
            offset = next_offset;
            continue;
        }
        let Some(header_len) = keyword_token_len(trimmed, "pie") else {
            let start = offset + visible.len() - trimmed.len();
            let token_length = trimmed
                .char_indices()
                .find_map(|(index, ch)| ch.is_whitespace().then_some(index))
                .unwrap_or(trimmed.len());
            return Ok(PieHeader::ExpectedPie(SourceSpan::new(
                start,
                start + token_length,
            )));
        };
        let leading = visible.len() - trimmed.len();
        let header_start = offset + leading;
        let after_header = header_start + header_len;
        let horizontal = code[after_header..]
            .chars()
            .take_while(|ch| matches!(ch, ' ' | '\t'))
            .map(char::len_utf8)
            .sum::<usize>();
        let show_data_start = after_header + horizontal;
        let show_data_span = keyword_token_len(&code[show_data_start..], "showData").map(|len| {
            debug_assert_eq!(len, "showData".len());
            SourceSpan::new(show_data_start, show_data_start + len)
        });
        let body_start = show_data_span.map_or(after_header, |span| span.end);
        return Ok(PieHeader::Body(PieBodyStart {
            offset: body_start,
            show_data_span,
        }));
    }
    Ok(PieHeader::Empty)
}

fn keyword_token_len(input: &str, keyword: &str) -> Option<usize> {
    let rest = input.strip_prefix(keyword)?;
    (rest.is_empty()
        || rest.starts_with("%%")
        || rest.chars().next().is_some_and(char::is_whitespace))
    .then_some(keyword.len())
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

fn strip_inline_comment(line: &str) -> &str {
    strip_langium_inline_comment(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_empty_capture_keeps_an_available_empty_journal() {
        crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
        let parsed = parse_pie_semantic_model_controlled(
            "",
            "pie",
            &MermaidConfig::default(),
            EditorFactsDemand::Capture,
            &crate::OperationControl::new(),
        )
        .unwrap();
        assert!(parsed.model.is_ok());
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("pie"),
            1
        );
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
    use crate::MermaidConfig;
    #[cfg(feature = "presentation")]
    use crate::ParseMetadata;
    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    use crate::{EditorSemanticCompleteness, Engine, Error, ParseOptions, SourceSpan};
    use serde_json::json;

    #[cfg(feature = "presentation")]
    fn metadata() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "pie".to_string(),
            config: MermaidConfig::default(),
            effective_config: crate::presentation::PresentationConfig::default(),
            title: None,
        }
    }

    #[test]
    fn controlled_parse_can_cancel_after_the_pie_header() {
        let control = crate::OperationControl::new();
        control.cancel_after_checkpoints(2);

        assert!(matches!(
            construct_pie_semantic_source_controlled(
                "pie\n\"A\": 1\n\"B\": 2\n",
                "pie",
                EditorFactsDemand::Capture,
                &control,
            ),
            Err(crate::OperationCancelled { .. })
        ));
    }

    #[test]
    fn pie_semantic_model_reuses_one_construction_without_editor_or_compat_state() {
        crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
        crate::editor::test_work::reset();
        let model = parse_pie_semantic_model_controlled(
            "pie showData\ntitle Revenue\n\"A\": 1\n\"A\": 2\n\"B\": 3\n",
            "pie",
            &MermaidConfig::default(),
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(
            serde_json::to_value(model).unwrap(),
            json!({
                "showData": true,
                "title": "Revenue",
                "accTitle": null,
                "accDescr": null,
                "sections": [{"label": "A", "value": 1.0}, {"label": "B", "value": 3.0}],
            })
        );
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("pie"),
            1
        );
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    fn pie_semantic_model_applies_common_text_policy_without_theme_resolution() {
        let config = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "themeVariables": {"primaryColor": "not-a-color"},
        }));
        let source = "pie\ntitle <script>bad()</script>Revenue\naccTitle: <script>bad()</script>Accessible\naccDescr: <script>bad()</script>Description\n\"A\": 1\n";
        let model = parse_pie_semantic_model_controlled(
            source,
            "pie",
            &config,
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .expect("semantic parse does not evaluate theme colors");
        assert_eq!(
            (
                model.title.as_deref(),
                model.acc_title.as_deref(),
                model.acc_descr.as_deref()
            ),
            (Some("Revenue"), Some("Accessible"), Some("Description"))
        );
        #[cfg(feature = "presentation")]
        {
            let mut expected = parse_pie_model_for_render(source, &metadata()).unwrap();
            expected.sanitize_common_db_fields(&config);
            assert_eq!(
                serde_json::to_value(model).unwrap(),
                serde_json::to_value(expected).unwrap()
            );
        }
    }

    #[test]
    fn pie_semantic_empty_input_has_no_compatibility_sentinel() {
        let semantic = parse_pie_semantic_model_controlled(
            "",
            "pie",
            &MermaidConfig::default(),
            EditorFactsDemand::Omit,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();

        assert_eq!(
            serde_json::to_value(semantic).unwrap(),
            json!({
                "showData": false,
                "title": null,
                "accTitle": null,
                "accDescr": null,
                "sections": [],
            })
        );
        #[cfg(feature = "compat-json")]
        assert_eq!(parse_pie("", &metadata()).unwrap(), json!({}));
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn pie_typed_projection_matches_complete_compatibility_json() {
        let text = "pie showData\n\"A\": 1\n\"B\": 2\n";
        let meta = metadata();
        let compat = parse_pie(text, &meta).unwrap();
        let typed = parse_pie_model_for_render(text, &meta).unwrap();

        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), compat);
        assert_eq!(compat["type"], "pie");
        assert!(compat["title"].is_null());
    }

    #[cfg(feature = "compat-json")]
    #[test]
    fn pie_typed_projection_preserves_empty_and_header_only_output_states() {
        let meta = metadata();
        for source in ["", "pie"] {
            let compat = parse_pie(source, &meta).unwrap();
            let typed = parse_pie_model_for_render(source, &meta).unwrap();

            assert_eq!(
                render_model_to_compat_json(&typed, &meta).unwrap(),
                compat,
                "projection drift for {source:?}"
            );
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_invalid_header_is_a_failure_with_recovered_editor_facts() {
        let meta = metadata();
        for source in ["piep", "  piep\n\"A\": 1\n", "packet"] {
            let start = source.find(|ch: char| !ch.is_whitespace()).unwrap();
            let token = source[start..].split_whitespace().next().unwrap();
            let span = SourceSpan::new(start, start + token.len());
            let semantic = parse_pie(source, &meta).expect_err("invalid pie header");
            let typed = parse_pie_model_for_render(source, &meta).expect_err("invalid pie header");
            for error in [semantic, typed] {
                let Error::DiagramParse { diagnostic, .. } = error else {
                    panic!("expected diagram parse failure");
                };
                assert_eq!(diagnostic.message(), "expected pie");
                assert_eq!(diagnostic.span(), Some(span));
            }
            let (model, facts, _) =
                parse_pie_json_and_editor_facts(source, &meta, &crate::OperationControl::new())
                    .unwrap()
                    .into_parts();
            assert!(model.is_err());
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span == Some(span)
            }));
        }
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_supports_title_statement_after_header() {
        let engine = Engine::new();
        let input = r#"
pie showData
  title Market Share
  "A" : 1
  "B" : 2
"#;

        let parsed = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .expect("diagram detected");

        assert_eq!(parsed.meta.diagram_type, "pie");
        assert_eq!(
            parsed.model.get("title").and_then(|v| v.as_str()),
            Some("Market Share")
        );
        assert_eq!(
            parsed.model.get("showData").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_uses_langium_string_escapes_and_quote_aware_inline_comments() {
        let engine = Engine::new();
        let parsed = engine
            .parse_diagram_sync(
                r#"pie
"A\n100%% complete": 1 %% outside comment
'B\tlabel': 2
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .expect("pie parses Langium strings");

        assert_eq!(parsed.model["sections"][0]["label"], "An100%% complete");
        assert_eq!(parsed.model["sections"][1]["label"], "Btlabel");
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_number_terminal_matches_number_pie_exactly() {
        let engine = Engine::new();
        for value in [".5", "01", "1.", "1junk", "1..2"] {
            let source = format!("pie\n\"A\": {value}\n");
            assert!(
                engine
                    .parse_diagram_sync(&source, ParseOptions::strict())
                    .is_err(),
                "NUMBER_PIE must reject {value:?}"
            );
        }

        let parsed = engine
            .parse_diagram_sync(
                "pie\n\"zero\": 0\n\"integer\": -1\n\"decimal\": 01.5\n",
                ParseOptions::strict(),
            )
            .expect_err("negative values are rejected only after syntax succeeds");
        assert!(parsed.to_string().contains("invalid value: -1"));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_editor_recovery_reports_negative_value_validation_errors() {
        let source = "pie\r\n  \"negative\": -1  \r\n\"valid\": 2\r\n";
        let invalid = "-1";
        let start = source.find(invalid).unwrap();
        let expected_span = SourceSpan::new(start, start + invalid.len());
        let engine = Engine::new();

        crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
        crate::editor::test_work::reset();
        let error = engine
            .parse_diagram_sync(source, ParseOptions::strict())
            .expect_err("a negative pie value must fail strict parsing");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("expected pie parse diagnostic");
        };
        assert_eq!(diagnostic.span(), Some(expected_span));
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("pie"),
            1
        );
        assert_eq!(
            crate::editor::test_work::read(),
            crate::editor::test_work::Counts::default()
        );

        for (known_type, typed) in [(true, false), (false, true), (true, true)] {
            crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
            crate::editor::test_work::reset();
            let options = ParseOptions::strict();
            let error = match (known_type, typed) {
                (true, false) => engine
                    .parse_diagram_with_type_sync("pie", source, options)
                    .map(|_| ()),
                (false, true) => engine
                    .parse_diagram_for_render_model_sync(source, options)
                    .map(|_| ()),
                (true, true) => engine
                    .parse_diagram_for_render_model_with_type_sync("pie", source, options)
                    .map(|_| ()),
                (false, false) => unreachable!(),
            }
            .expect_err("semantic-only parsing must still validate negative pie values");
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("expected pie parse diagnostic");
            };
            assert_eq!(diagnostic.span(), Some(expected_span));
            assert_eq!(
                crate::diagrams::langium_common::family_syntax_construction_count("pie"),
                1
            );
            assert_eq!(
                crate::editor::test_work::read(),
                crate::editor::test_work::Counts::default(),
                "negative pie: known={known_type}, typed={typed} discarded editor facts"
            );
        }

        crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
        crate::editor::test_work::reset();
        let facts = crate::family::test_support::editor_facts(
            parse_pie_json_and_editor_facts,
            source,
            &metadata(),
        );
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("pie"),
            1
        );
        let work = crate::editor::test_work::read();
        assert_eq!(work.collections, 1);
        assert_eq!(work.symbols, facts.symbols.len());
        assert_eq!(work.diagnostics, facts.diagnostics.len());
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "negative"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "valid"));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(expected_span)
                && diagnostic
                    .message
                    .contains("Negative values are not allowed")
        }));
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_reports_later_syntax_errors_before_populate_errors() {
        let engine = Engine::new();
        let error = engine
            .parse_diagram_sync(
                "pie\n\"negative\": -1\n\"malformed\": 1junk\n",
                ParseOptions::strict(),
            )
            .expect_err("the malformed NUMBER_PIE must fail parsing");
        let message = error.to_string();

        assert!(message.contains("unexpected pie statement"), "{message}");
        assert!(!message.contains("invalid value: -1"), "{message}");
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_rejects_show_data_after_the_header_line() {
        let source = "pie\nshowData\n\"A\": 1\n";
        let engine = Engine::new();

        let error = engine
            .parse_diagram_sync(source, ParseOptions::strict())
            .expect_err("showData is only valid in the pie header");
        assert!(
            error
                .to_string()
                .contains("unexpected pie statement: showData")
        );

        let facts = crate::family::test_support::editor_facts(
            parse_pie_json_and_editor_facts,
            source,
            &metadata(),
        );
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(
            !facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "showData")
        );
        assert!(
            facts
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("unexpected pie statement"))
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_supports_header_acc_title_inline() {
        let engine = Engine::new();
        let input = r#"
pie accTitle: sample wow
  "A" : 1
"#;

        let parsed = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .expect("diagram detected");

        assert_eq!(parsed.meta.diagram_type, "pie");
        assert_eq!(
            parsed.model.get("accTitle").and_then(|v| v.as_str()),
            Some("sample wow")
        );
    }

    #[cfg(all(feature = "compat-json", feature = "editor-facts"))]
    #[test]
    fn pie_supports_header_acc_descr_block() {
        let engine = Engine::new();
        let input = r#"
pie accDescr {
  first line
  second line
}
  "A" : 1
"#;

        let parsed = engine
            .parse_diagram_sync(input, ParseOptions::strict())
            .unwrap()
            .expect("diagram detected");

        assert_eq!(parsed.meta.diagram_type, "pie");
        assert_eq!(
            parsed.model.get("accDescr").and_then(|v| v.as_str()),
            Some("first line\nsecond line")
        );
    }
}
