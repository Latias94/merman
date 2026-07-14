use crate::diagrams::scan::strip_line_ending;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticSymbol, Error, ParseMetadata, Result, SourceSpan,
};
use crate::{
    common_db::LangiumCommonDbFields,
    diagrams::langium_common::{
        LangiumCommonFacts, parse_langium_common, parse_langium_string,
        push_langium_common_editor_fact, push_langium_common_recovery,
        strip_langium_inline_comment,
    },
};
use serde_json::{Value, json};

const HEADER: &str = "cynefin-beta";
const DOMAINS: &[&str] = &["complex", "complicated", "clear", "chaotic", "confusion"];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinItemModel {
    pub label: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinDomainModel {
    pub name: String,
    #[serde(default)]
    pub items: Vec<CynefinItemModel>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinTransitionModel {
    pub from: String,
    pub to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct CynefinDiagramModel {
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub domains: Vec<CynefinDomainModel>,
    #[serde(default)]
    pub transitions: Vec<CynefinTransitionModel>,
}

pub type CynefinDiagramRenderModel = CynefinDiagramModel;

impl CynefinDiagramModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_title(&mut self.title, config);
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    span: SourceSpan,
    selection: SourceSpan,
}

#[derive(Debug, Clone)]
struct TransitionParts {
    from: SpannedText,
    to: SpannedText,
    label: Option<SpannedText>,
}

#[derive(Debug, Clone)]
enum CynefinLinePart {
    Domain(SpannedText),
    Item(SpannedText),
    Transition(TransitionParts),
}

struct CynefinSemanticSource {
    model: CynefinDiagramModel,
    editor_facts: EditorSemanticFacts,
}

pub fn parse_cynefin(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut model = parse_cynefin_semantic_source(code, meta)?.model;
    model.sanitize_common_db_fields(&meta.effective_config);

    render_model_to_compat_json(&model, meta)
}

pub(crate) fn parse_cynefin_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let CynefinSemanticSource {
        mut model,
        editor_facts,
    } = parse_cynefin_semantic_source(code, meta)?;
    model.sanitize_common_db_fields(&meta.effective_config);
    Ok((render_model_to_compat_json(&model, meta)?, editor_facts))
}

pub(crate) fn render_model_to_compat_json(
    model: &CynefinDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    Ok(json!({
        "type": meta.diagram_type,
        "title": &model.title,
        "accTitle": &model.acc_title,
        "accDescr": &model.acc_descr,
        "domains": &model.domains,
        "transitions": &model.transitions,
    }))
}

pub fn parse_cynefin_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<CynefinDiagramRenderModel> {
    let mut model = parse_cynefin_semantic_source(code, meta)?.model;
    model.sanitize_common_db_fields(&meta.effective_config);
    Ok(model)
}

pub fn parse_cynefin_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    match parse_cynefin_semantic_source(code, meta) {
        Ok(source) => source.editor_facts,
        Err(_) => scan_cynefin_editor_facts(code),
    }
}

fn scan_cynefin_editor_facts(code: &str) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut offset = 0usize;
    let mut saw_header = false;
    let mut current_domain: Option<String> = None;

    while offset < code.len() {
        let line_start = offset;
        let (segment, next_offset) = physical_line_at(code, offset);
        offset = next_offset;
        let line = strip_line_ending(segment);
        let stripped = strip_inline_comment_aware(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (body, body_start) = if !saw_header {
            let Some((body, body_offset)) = split_header(stripped) else {
                facts.mark_recovered_from_parse_error(
                    "expected cynefin-beta header",
                    Some(trimmed_source_span(stripped, line_start)),
                );
                return facts;
            };
            saw_header = true;
            (body, line_start + body_offset)
        } else {
            let body_start = line_start + line.find(stripped).unwrap_or_default();
            (stripped, body_start)
        };

        if body.trim().is_empty() {
            continue;
        }

        if let Some(parsed) = parse_langium_common(code, body_start) {
            offset = body_start + parsed.consumed;
            current_domain = None;
            push_langium_common_editor_fact(&mut facts, &parsed.fact, "cynefin");
            if let Some(diagnostic) = &parsed.diagnostic {
                push_langium_common_recovery(&mut facts, diagnostic);
            }
            continue;
        }

        let parts = match parse_cynefin_line_parts(body, body_start) {
            Ok(parts) => parts,
            Err(err) => {
                facts.mark_recovered_from_parse_error(
                    format!("cynefin parser recovered after parse error: {err}"),
                    Some(trimmed_source_span(body, body_start)),
                );
                return facts;
            }
        };
        for part in parts {
            match part {
                CynefinLinePart::Domain(domain) => {
                    current_domain = Some(domain.text.clone());
                    push_domain_fact(&mut facts, domain, "cynefin domain");
                }
                CynefinLinePart::Item(item) => {
                    if current_domain.is_some() {
                        push_payload_fact(
                            &mut facts,
                            item,
                            "cynefin domain item",
                            EditorSemanticKind::String,
                        );
                    } else {
                        facts.mark_recovered_from_parse_error(
                            "cynefin item must follow a domain",
                            Some(item.span),
                        );
                        return facts;
                    }
                }
                CynefinLinePart::Transition(transition) => {
                    current_domain = None;
                    push_domain_fact(
                        &mut facts,
                        transition.from.clone(),
                        "cynefin transition source",
                    );
                    push_domain_fact(
                        &mut facts,
                        transition.to.clone(),
                        "cynefin transition target",
                    );
                    if let Some(label) = transition.label {
                        push_payload_fact(
                            &mut facts,
                            label,
                            "cynefin transition label",
                            EditorSemanticKind::String,
                        );
                    }
                    if transition.from.text == transition.to.text {
                        facts.push_diagnostic(
                            format!(
                                "cynefin self-loop transition on domain \"{}\" is skipped",
                                transition.from.text
                            ),
                            Some(transition.from.span),
                        );
                    }
                }
            }
        }
    }

    if !saw_header {
        facts.mark_recovered_from_parse_error(
            "expected cynefin-beta header",
            Some(SourceSpan::new(0, 0)),
        );
    }

    facts
}

fn trimmed_source_span(source: &str, source_start: usize) -> SourceSpan {
    let trimmed = source.trim();
    let start = source_start + source.find(trimmed).unwrap_or_default();
    SourceSpan::new(start, start + trimmed.len())
}

fn parse_cynefin_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> Result<CynefinSemanticSource> {
    #[cfg(test)]
    crate::diagrams::langium_common::record_family_syntax_construction("cynefin");

    let mut model = CynefinDiagramModel::default();
    let mut editor_facts = EditorSemanticFacts::new();
    let mut saw_header = false;
    let mut current_domain: Option<usize> = None;
    let mut offset = 0usize;
    let mut common = LangiumCommonFacts::default();

    while offset < code.len() {
        let line_start = offset;
        let (segment, next_offset) = physical_line_at(code, offset);
        offset = next_offset;
        let line = strip_line_ending(segment);
        let stripped = strip_inline_comment_aware(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (body, body_start) = if !saw_header {
            let Some((body, body_offset)) = split_header(stripped) else {
                return Err(parse_error(meta, "expected cynefin-beta header"));
            };
            saw_header = true;
            (body, line_start + body_offset)
        } else {
            let body_start = line_start + line.find(stripped).unwrap_or_default();
            (stripped, body_start)
        };

        if body.trim().is_empty() {
            continue;
        }

        if let Some(parsed) = parse_langium_common(code, body_start) {
            if let Some(diagnostic) = parsed.diagnostic {
                return Err(Error::diagram_parse_insertion_point(
                    meta.diagram_type.clone(),
                    diagnostic.message,
                    diagnostic.span.start,
                ));
            }
            offset = body_start + parsed.consumed;
            current_domain = None;
            push_langium_common_editor_fact(&mut editor_facts, &parsed.fact, "cynefin");
            common.push(parsed.fact);
            continue;
        }

        for part in parse_cynefin_line_parts(body, body_start)? {
            match part {
                CynefinLinePart::Domain(domain) => {
                    current_domain = Some(start_domain(&mut model.domains, domain.text.clone()));
                    push_domain_fact(&mut editor_facts, domain, "cynefin domain");
                }
                CynefinLinePart::Item(item) => {
                    let Some(domain_idx) = current_domain else {
                        return Err(parse_error(meta, "cynefin item must follow a domain"));
                    };
                    model.domains[domain_idx].items.push(CynefinItemModel {
                        label: item.text.clone(),
                    });
                    push_payload_fact(
                        &mut editor_facts,
                        item,
                        "cynefin domain item",
                        EditorSemanticKind::String,
                    );
                }
                CynefinLinePart::Transition(transition) => {
                    current_domain = None;
                    push_domain_fact(
                        &mut editor_facts,
                        transition.from.clone(),
                        "cynefin transition source",
                    );
                    push_domain_fact(
                        &mut editor_facts,
                        transition.to.clone(),
                        "cynefin transition target",
                    );
                    if let Some(label) = transition.label.clone() {
                        push_payload_fact(
                            &mut editor_facts,
                            label,
                            "cynefin transition label",
                            EditorSemanticKind::String,
                        );
                    }
                    if transition.from.text == transition.to.text {
                        editor_facts.push_diagnostic(
                            format!(
                                "cynefin self-loop transition on domain \"{}\" is skipped",
                                transition.from.text
                            ),
                            Some(transition.from.span),
                        );
                    } else {
                        model.transitions.push(CynefinTransitionModel {
                            from: transition.from.text,
                            to: transition.to.text,
                            label: transition
                                .label
                                .map(|label| label.text)
                                .filter(|label| !label.is_empty()),
                        });
                    }
                }
            }
        }
    }

    if !saw_header {
        return Err(parse_error(meta, "expected cynefin-beta header"));
    }

    let common = LangiumCommonDbFields::from_facts(&common);
    model.title = common.title;
    model.acc_title = common.acc_title;
    model.acc_descr = common.acc_descr;

    Ok(CynefinSemanticSource {
        model,
        editor_facts,
    })
}

fn split_header(line: &str) -> Option<(&str, usize)> {
    let leading = line.len() - line.trim_start().len();
    let rest = &line[leading..];
    let after_header = rest.strip_prefix(HEADER)?;
    let next = after_header.chars().next();
    if next.is_some_and(|ch| ch != ':' && !ch.is_whitespace() && !after_header.starts_with("%%")) {
        return None;
    }

    let colon_len = after_header.starts_with(':') as usize;
    let body_offset = leading + HEADER.len() + colon_len;
    Some((&line[body_offset..], body_offset))
}

fn parse_cynefin_line_parts(line: &str, line_start: usize) -> Result<Vec<CynefinLinePart>> {
    let mut cursor = CynefinCursor::new(line, line_start);
    let mut parts = Vec::new();

    loop {
        cursor.skip_ws();
        if cursor.is_eof() {
            break;
        }

        let part_start = cursor.pos;
        let remaining = cursor.remaining();

        if let Some(transition) = parse_transition_spanned(remaining, line_start + part_start)? {
            cursor.pos = line.len();
            parts.push(CynefinLinePart::Transition(transition));
            continue;
        }

        if let Some(domain) = cursor.take_domain() {
            parts.push(CynefinLinePart::Domain(domain));
            continue;
        }

        if let Some(item) = cursor.take_quoted_string() {
            parts.push(CynefinLinePart::Item(item));
            continue;
        }

        return Err(Error::diagram_parse_fallback(
            "cynefin",
            "expected cynefin domain, quoted item, transition, or common directive",
        ));
    }

    Ok(parts)
}

fn physical_line_at(code: &str, start: usize) -> (&str, usize) {
    let rest = &code[start..];
    let len = rest.find('\n').map_or(rest.len(), |index| index + 1);
    (&rest[..len], start + len)
}

fn start_domain(domains: &mut Vec<CynefinDomainModel>, name: String) -> usize {
    if let Some(idx) = domains.iter().position(|domain| domain.name == name) {
        domains[idx].items.clear();
        idx
    } else {
        domains.push(CynefinDomainModel {
            name,
            items: Vec::new(),
        });
        domains.len() - 1
    }
}

fn parse_transition_spanned(line: &str, line_start: usize) -> Result<Option<TransitionParts>> {
    let mut cursor = CynefinCursor::new(line, line_start);
    cursor.skip_ws();
    let Some(from) = cursor.take_domain() else {
        return Ok(None);
    };
    cursor.skip_ws();
    if !cursor.take_literal("-->") {
        return Ok(None);
    }
    cursor.skip_ws();
    let Some(to) = cursor.take_domain() else {
        return Err(Error::diagram_parse_fallback(
            "cynefin",
            "expected cynefin transition target",
        ));
    };
    cursor.skip_ws();

    let label = if cursor.is_eof() {
        None
    } else {
        if !cursor.take_literal(":") {
            return Err(Error::diagram_parse_fallback(
                "cynefin",
                "expected ':' before cynefin transition label",
            ));
        }
        cursor.skip_ws();
        Some(cursor.take_quoted_string().ok_or_else(|| {
            Error::diagram_parse_fallback("cynefin", "expected quoted cynefin transition label")
        })?)
    };
    cursor.skip_ws();
    if !cursor.is_eof() {
        return Err(Error::diagram_parse_fallback(
            "cynefin",
            "unexpected trailing cynefin transition tokens",
        ));
    }

    Ok(Some(TransitionParts { from, to, label }))
}

fn push_domain_fact(facts: &mut EditorSemanticFacts, domain: SpannedText, detail: &'static str) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::NodeIdentifier,
        domain.selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::outline(
        domain.text,
        Some(detail.to_string()),
        EditorSemanticKind::Namespace,
        domain.span,
        domain.selection,
    ));
}

fn push_payload_fact(
    facts: &mut EditorSemanticFacts,
    value: SpannedText,
    detail: &'static str,
    kind: EditorSemanticKind,
) {
    facts.push_expected_syntax(EditorExpectedSyntax::new(
        EditorExpectedSyntaxKind::Payload,
        value.selection,
    ));
    facts.push_symbol(EditorSemanticSymbol::payload(
        value.text,
        Some(detail.to_string()),
        kind,
        value.span,
        value.selection,
    ));
}

struct CynefinCursor<'a> {
    input: &'a str,
    line_start: usize,
    pos: usize,
}

impl<'a> CynefinCursor<'a> {
    fn new(input: &'a str, line_start: usize) -> Self {
        Self {
            input,
            line_start,
            pos: 0,
        }
    }

    fn remaining(&self) -> &'a str {
        &self.input[self.pos..]
    }

    fn is_eof(&self) -> bool {
        self.pos >= self.input.len()
    }

    fn skip_ws(&mut self) {
        self.pos += self
            .remaining()
            .chars()
            .take_while(|ch| matches!(ch, ' ' | '\t'))
            .map(char::len_utf8)
            .sum::<usize>();
    }

    fn take_literal(&mut self, literal: &str) -> bool {
        if self.remaining().starts_with(literal) {
            self.pos += literal.len();
            true
        } else {
            false
        }
    }

    fn take_domain(&mut self) -> Option<SpannedText> {
        let rest = self.remaining();
        for domain in DOMAINS {
            let Some(after) = rest.strip_prefix(domain) else {
                continue;
            };
            if after
                .chars()
                .next()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                continue;
            }
            let start = self.line_start + self.pos;
            self.pos += domain.len();
            return Some(SpannedText {
                text: (*domain).to_string(),
                span: SourceSpan::new(start, start + domain.len()),
                selection: SourceSpan::new(start, start + domain.len()),
            });
        }
        None
    }

    fn take_quoted_string(&mut self) -> Option<SpannedText> {
        let start = self.line_start + self.pos;
        let parsed = parse_langium_string(self.remaining(), start)?;
        self.pos += parsed.consumed;
        Some(SpannedText {
            text: parsed.value,
            span: parsed.raw_span,
            selection: parsed.value_span,
        })
    }
}

fn strip_inline_comment_aware(line: &str) -> &str {
    strip_langium_inline_comment(line)
}

fn parse_error(meta: &ParseMetadata, message: impl Into<String>) -> Error {
    Error::diagram_parse_fallback(meta.diagram_type.clone(), message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_comment_markers_outside_quoted_strings() {
        assert_eq!(
            strip_inline_comment_aware("  complex %% comment"),
            "  complex "
        );
        assert_eq!(
            strip_inline_comment_aware("  \"100%% visible\" %% comment"),
            "  \"100%% visible\" "
        );
    }

    #[test]
    fn parses_escaped_quoted_string_payload() {
        let mut cursor = CynefinCursor::new("  \"Probe \\\"quoted\\\" value\"", 10);
        cursor.skip_ws();
        let value = cursor.take_quoted_string().unwrap();
        cursor.skip_ws();
        assert!(cursor.is_eof());
        assert_eq!(value.text, "Probe \"quoted\" value");
        assert_eq!(value.selection, SourceSpan::new(13, 35));
    }

    #[test]
    fn cynefin_recovery_uses_exact_trimmed_crlf_spans() {
        for (source, invalid) in [
            ("  invalid header %% hidden\r\n", "invalid header"),
            (
                "cynefin-beta\r\n  invalid body %% hidden\r\n",
                "invalid body",
            ),
            ("cynefin-beta\r\n  \"orphan\" %% hidden\r\n", "\"orphan\""),
        ] {
            let facts = crate::Engine::new()
                .parse_editor_semantic_facts_with_type_sync(
                    "cynefin",
                    source,
                    crate::ParseOptions::strict(),
                )
                .unwrap()
                .unwrap();
            let start = source.find(invalid).unwrap();
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span == Some(SourceSpan::new(start, start + invalid.len()))
            }));
        }
    }

    #[test]
    fn typed_render_model_projects_exact_compatibility_json() {
        let source = concat!(
            "cynefin-beta\n",
            "title Cynefin Map\n",
            "accTitle: Accessible Map\n",
            "complex\n",
            "  \"Probe\"\n",
            "complex --> complicated : \"Sense\"\n",
        );
        let engine = crate::Engine::new();
        let compat = engine
            .parse_diagram_sync(source, crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let typed = engine
            .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let crate::RenderSemanticModel::Cynefin(model) = typed.model else {
            panic!("expected Cynefin render model");
        };

        assert_eq!(
            render_model_to_compat_json(&model, &typed.meta).unwrap(),
            compat.model
        );
    }
}
