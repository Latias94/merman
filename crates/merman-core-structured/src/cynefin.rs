//! Cynefin source parsing and semantic model construction.

#[cfg(feature = "editor-facts")]
use merman_core_preprocess::scan::langium::LangiumCommonFact;
use merman_core_preprocess::scan::langium::{
    LangiumCommonDbFields, LangiumCommonDiagnostic, LangiumCommonFacts, parse_langium_common,
    parse_langium_string, strip_langium_inline_comment,
};
use merman_core_preprocess::scan::strip_line_ending;
use merman_model::families::cynefin::{
    CynefinDiagramModel, CynefinDomainModel, CynefinItemModel, CynefinTransitionModel,
};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

const HEADER: &str = "cynefin-beta";
const DOMAINS: &[&str] = &["complex", "complicated", "clear", "chaotic", "confusion"];

#[derive(Debug, Clone)]
pub struct SpannedText {
    pub text: String,
    pub span: SourceSpan,
    #[cfg(any(test, feature = "editor-facts"))]
    pub selection: SourceSpan,
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

struct CynefinParsedLine {
    parts: Vec<CynefinLinePart>,
    error: Option<&'static str>,
}

#[derive(Debug)]
pub enum CynefinIssue {
    ExpectedHeader(SourceSpan),
    Common(LangiumCommonDiagnostic),
    InvalidBody {
        message: &'static str,
        span: SourceSpan,
    },
    OrphanItem(SourceSpan),
}

/// Observations in source order, expressed in the parser input's byte coordinates.
#[cfg(feature = "editor-facts")]
pub enum CynefinObservation<'statement> {
    Common(&'statement LangiumCommonFact),
    Domain(&'statement SpannedText),
    Item(&'statement SpannedText),
    Transition {
        from: &'statement SpannedText,
        to: &'statement SpannedText,
        label: Option<&'statement SpannedText>,
    },
    Issue(&'statement CynefinIssue),
    SelfLoop(&'statement SpannedText),
}

/// Builds the recovered model, retaining the first issue for strict callers.
pub fn parse_cynefin_controlled(
    code: &str,
    control: &OperationControl,
) -> OperationControlResult<(CynefinDiagramModel, Option<CynefinIssue>)> {
    parse_cynefin(
        code,
        #[cfg(feature = "editor-facts")]
        false,
        #[cfg(feature = "editor-facts")]
        None,
        control,
    )
}

#[cfg(feature = "editor-facts")]
pub fn parse_cynefin_with_observer_controlled(
    code: &str,
    capture_facts: bool,
    observe: Option<&mut dyn FnMut(CynefinObservation<'_>)>,
    control: &OperationControl,
) -> OperationControlResult<(CynefinDiagramModel, Option<CynefinIssue>)> {
    parse_cynefin(code, capture_facts, observe, control)
}

fn parse_cynefin(
    code: &str,
    #[cfg(feature = "editor-facts")] capture_facts: bool,
    #[cfg(feature = "editor-facts")] mut observe: Option<&mut dyn FnMut(CynefinObservation<'_>)>,
    control: &OperationControl,
) -> OperationControlResult<(CynefinDiagramModel, Option<CynefinIssue>)> {
    let mut model = CynefinDiagramModel::default();
    let mut header_decided = false;
    let mut current_domain: Option<usize> = None;
    let mut offset = 0usize;
    let mut common = LangiumCommonFacts::default();
    let mut first_issue = None;

    while offset < code.len() {
        control.checkpoint()?;
        let line_start = offset;
        let (segment, next_offset) = physical_line_at(code, offset);
        offset = next_offset;
        let line = strip_line_ending(segment);
        let stripped = strip_inline_comment_aware(line);
        if stripped.trim().is_empty() {
            continue;
        }

        let (body, body_start) = if !header_decided {
            header_decided = true;
            let Some(header) = split_header(stripped, line_start) else {
                record_issue(
                    &mut first_issue,
                    CynefinIssue::ExpectedHeader(trimmed_source_span(stripped, line_start)),
                    #[cfg(feature = "editor-facts")]
                    &mut observe,
                );
                current_domain = None;
                continue;
            };
            (header.body, header.body_start)
        } else {
            (stripped, line_start)
        };

        if body.trim().is_empty() {
            continue;
        }
        if let Some(parsed) = parse_langium_common(code, body_start) {
            offset = body_start + parsed.consumed;
            current_domain = None;
            #[cfg(feature = "editor-facts")]
            if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                observe(CynefinObservation::Common(&parsed.fact));
            }
            if let Some(diagnostic) = parsed.diagnostic {
                record_issue(
                    &mut first_issue,
                    CynefinIssue::Common(diagnostic),
                    #[cfg(feature = "editor-facts")]
                    &mut observe,
                );
            } else {
                common.push(parsed.fact);
            }
            continue;
        }

        let parsed_line = parse_cynefin_line_parts_controlled(body, body_start, control)?;
        if let Some(message) = parsed_line.error {
            record_issue(
                &mut first_issue,
                CynefinIssue::InvalidBody {
                    message,
                    span: trimmed_source_span(body, body_start),
                },
                #[cfg(feature = "editor-facts")]
                &mut observe,
            );
            current_domain = None;
            continue;
        }

        for (index, part) in parsed_line.parts.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            match part {
                CynefinLinePart::Domain(domain) => {
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                        observe(CynefinObservation::Domain(&domain));
                    }
                    current_domain = Some(start_domain(&mut model.domains, domain.text));
                }
                CynefinLinePart::Item(item) => {
                    let Some(domain_idx) = current_domain else {
                        record_issue(
                            &mut first_issue,
                            CynefinIssue::OrphanItem(item.span),
                            #[cfg(feature = "editor-facts")]
                            &mut observe,
                        );
                        continue;
                    };
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                        observe(CynefinObservation::Item(&item));
                    }
                    model.domains[domain_idx]
                        .items
                        .push(CynefinItemModel { label: item.text });
                }
                CynefinLinePart::Transition(transition) => {
                    current_domain = None;
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                        observe(CynefinObservation::Transition {
                            from: &transition.from,
                            to: &transition.to,
                            label: transition.label.as_ref(),
                        });
                    }
                    if transition.from.text == transition.to.text {
                        #[cfg(feature = "editor-facts")]
                        if let Some(observe) = observe.as_deref_mut() {
                            observe(CynefinObservation::SelfLoop(&transition.from));
                        }
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

    if !header_decided {
        record_issue(
            &mut first_issue,
            CynefinIssue::ExpectedHeader(SourceSpan::new(0, 0)),
            #[cfg(feature = "editor-facts")]
            &mut observe,
        );
    }

    let common = LangiumCommonDbFields::from_facts(&common);
    model.title = common.title;
    model.acc_title = common.acc_title;
    model.acc_descr = common.acc_descr;
    control.checkpoint()?;
    Ok((model, first_issue))
}

fn record_issue(
    first: &mut Option<CynefinIssue>,
    issue: CynefinIssue,
    #[cfg(feature = "editor-facts")] observe: &mut Option<&mut dyn FnMut(CynefinObservation<'_>)>,
) {
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observe.as_deref_mut() {
        observe(CynefinObservation::Issue(&issue));
    }
    first.get_or_insert(issue);
}

fn trimmed_source_span(source: &str, source_start: usize) -> SourceSpan {
    let trimmed = source.trim();
    let start = source_start + source.len().saturating_sub(source.trim_start().len());
    SourceSpan::new(start, start + trimmed.len())
}

struct CynefinHeader<'a> {
    body: &'a str,
    body_start: usize,
}

fn split_header(line: &str, line_start: usize) -> Option<CynefinHeader<'_>> {
    let leading = line.len() - line.trim_start().len();
    let rest = &line[leading..];
    let after_header = rest.strip_prefix(HEADER)?;
    let next = after_header.chars().next();
    if next.is_some_and(|ch| ch != ':' && !ch.is_whitespace() && !after_header.starts_with("%%")) {
        return None;
    }

    let colon_len = after_header.starts_with(':') as usize;
    let body_offset = leading + HEADER.len() + colon_len;
    Some(CynefinHeader {
        body: &line[body_offset..],
        body_start: line_start + body_offset,
    })
}

fn parse_cynefin_line_parts_controlled(
    line: &str,
    line_start: usize,
    control: &OperationControl,
) -> OperationControlResult<CynefinParsedLine> {
    let mut cursor = CynefinCursor::new(line, line_start);
    let mut parts = Vec::new();
    let mut error = None;
    let mut iteration = 0usize;

    loop {
        if iteration.is_multiple_of(128) {
            control.checkpoint()?;
        }
        iteration += 1;
        cursor.skip_ws();
        if cursor.is_eof() {
            break;
        }

        match cursor.take_transition() {
            Ok(Some(transition)) => {
                parts.push(CynefinLinePart::Transition(transition));
                continue;
            }
            Ok(None) => {}
            Err(parse_error) => {
                error = Some(parse_error);
                break;
            }
        }

        if let Some(domain) = cursor.take_domain() {
            parts.push(CynefinLinePart::Domain(domain));
            continue;
        }

        if let Some(item) = cursor.take_quoted_string() {
            parts.push(CynefinLinePart::Item(item));
            continue;
        }

        error = Some("expected cynefin domain, quoted item, transition, or common directive");
        break;
    }

    Ok(CynefinParsedLine { parts, error })
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

struct CynefinCursor<'input> {
    input: &'input str,
    line_start: usize,
    pos: usize,
}

impl<'input> CynefinCursor<'input> {
    fn new(input: &'input str, line_start: usize) -> Self {
        Self {
            input,
            line_start,
            pos: 0,
        }
    }

    fn remaining(&self) -> &'input str {
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

    fn take_transition(&mut self) -> Result<Option<TransitionParts>, &'static str> {
        let start = self.pos;
        self.skip_ws();
        let Some(from) = self.take_domain() else {
            self.pos = start;
            return Ok(None);
        };
        self.skip_ws();
        if !self.take_operator("-->") {
            self.pos = start;
            return Ok(None);
        }

        self.skip_ws();
        let Some(to) = self.take_domain() else {
            return Err("expected cynefin transition target");
        };
        self.skip_ws();

        let label = if self.is_eof() {
            None
        } else {
            if !self.take_delimiter(":") {
                return Err("expected ':' before cynefin transition label");
            }
            self.skip_ws();
            Some(
                self.take_quoted_string()
                    .ok_or("expected quoted cynefin transition label")?,
            )
        };
        self.skip_ws();
        if !self.is_eof() {
            return Err("unexpected trailing cynefin transition tokens");
        }

        Ok(Some(TransitionParts { from, to, label }))
    }

    fn take_operator(&mut self, operator: &str) -> bool {
        if self.remaining().starts_with(operator) {
            self.pos += operator.len();
            true
        } else {
            false
        }
    }

    fn take_delimiter(&mut self, delimiter: &str) -> bool {
        if self.remaining().starts_with(delimiter) {
            self.pos += delimiter.len();
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
                #[cfg(any(test, feature = "editor-facts"))]
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
            #[cfg(any(test, feature = "editor-facts"))]
            selection: parsed.value_span,
        })
    }
}

fn strip_inline_comment_aware(line: &str) -> &str {
    strip_langium_inline_comment(line)
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
}
