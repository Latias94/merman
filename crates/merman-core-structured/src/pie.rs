//! Pie source parsing and semantic model construction.

#[cfg(feature = "editor-facts")]
use merman_core_preprocess::scan::langium::LangiumCommonFact;
use merman_core_preprocess::scan::langium::{
    LangiumCommonDbFields, LangiumCommonDiagnostic, LangiumCommonFacts, LangiumCommonParse,
    parse_langium_common, parse_langium_string, strip_langium_inline_comment,
};
use merman_model::families::pie::{PieDiagramModel, PieSection};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PieHeader<'source> {
    Empty,
    ExpectedPie(SourceSpan),
    Body(PieBody<'source>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PieBody<'source> {
    source: &'source str,
    offset: usize,
    show_data_span: Option<SourceSpan>,
}

#[derive(Debug)]
pub enum PieBodyIssue {
    Common(LangiumCommonDiagnostic),
    Unexpected(String),
    NegativeValue { message: String, span: SourceSpan },
}

/// A borrowed observation in parser input coordinates.
#[cfg(feature = "editor-facts")]
pub enum PieObservation<'statement> {
    CommonRecovery(&'statement LangiumCommonDiagnostic),
    Common(&'statement LangiumCommonFact),
    ShowData(SourceSpan),
    Section {
        label: &'statement str,
        statement_span: SourceSpan,
        label_span: SourceSpan,
        value_span: SourceSpan,
    },
    Unexpected(SourceSpan),
    NegativeValue {
        message: &'statement str,
        span: SourceSpan,
    },
}

impl PieBody<'_> {
    /// Parses the body and replays slice declarations in source order.
    pub fn parse_controlled(
        self,
        #[cfg(feature = "editor-facts")] capture_facts: bool,
        #[cfg(feature = "editor-facts")] mut observe: Option<&mut dyn FnMut(PieObservation<'_>)>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<PieDiagramModel, PieBodyIssue>> {
        let mut offset = self.offset;
        let mut common = LangiumCommonFacts::default();
        let mut parsed_sections = Vec::new();
        let mut first_issue = None;
        #[cfg(feature = "editor-facts")]
        if let (Some(observe), Some(span)) = (
            observe.as_deref_mut().filter(|_| capture_facts),
            self.show_data_span,
        ) {
            observe(PieObservation::ShowData(span));
        }

        while offset < self.source.len() {
            control.checkpoint()?;
            match parse_pie_statement(self.source, offset) {
                PieStatement::Common(parsed) => {
                    if let Some(diagnostic) = parsed.diagnostic {
                        #[cfg(feature = "editor-facts")]
                        if let Some(observe) = observe.as_deref_mut() {
                            observe(PieObservation::CommonRecovery(&diagnostic));
                        }
                        first_issue.get_or_insert(PieBodyIssue::Common(diagnostic));
                    }
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                        observe(PieObservation::Common(&parsed.fact));
                    }
                    common.push(parsed.fact);
                    offset += parsed.consumed;
                }
                PieStatement::Section {
                    section,
                    next_offset,
                } => {
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                        observe(PieObservation::Section {
                            label: &section.label,
                            statement_span: section.statement_span,
                            label_span: section.label_span,
                            value_span: section.value_span,
                        });
                    }
                    parsed_sections.push(section);
                    offset = next_offset;
                }
                PieStatement::Empty { next_offset } => offset = next_offset,
                PieStatement::Unexpected {
                    text,
                    #[cfg(feature = "editor-facts")]
                    span,
                    next_offset,
                } => {
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut() {
                        observe(PieObservation::Unexpected(span));
                    }
                    first_issue.get_or_insert(PieBodyIssue::Unexpected(text));
                    offset = next_offset;
                }
            }
        }

        #[cfg(feature = "editor-facts")]
        let observe_recovery = observe.is_some();
        #[cfg(not(feature = "editor-facts"))]
        let observe_recovery = false;
        let mut sections = Vec::new();
        let mut seen = HashSet::new();
        for (index, section) in parsed_sections.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if section.value < 0.0 {
                if observe_recovery || first_issue.is_none() {
                    let message = format!(
                        "\"{}\" has invalid value: {}. Negative values are not allowed in pie charts. All slice values must be >= 0.",
                        section.label,
                        ryu_js::Buffer::new().format(section.value)
                    );
                    let span = section.value_span;
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut() {
                        observe(PieObservation::NegativeValue {
                            message: &message,
                            span,
                        });
                    }
                    first_issue.get_or_insert(PieBodyIssue::NegativeValue { message, span });
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
        if let Some(issue) = first_issue {
            return Ok(Err(issue));
        }
        control.checkpoint()?;
        Ok(Ok(PieDiagramModel {
            show_data: self.show_data_span.is_some(),
            title: common.title,
            acc_title: common.acc_title,
            acc_descr: common.acc_descr,
            sections,
        }))
    }
}

pub fn pie_body_start_controlled<'source>(
    code: &'source str,
    control: &OperationControl,
) -> OperationControlResult<PieHeader<'source>> {
    let mut offset = 0usize;
    while offset < code.len() {
        control.checkpoint()?;
        let (line, next_offset) = physical_line(code, offset);
        let visible = strip_inline_comment(line);
        let trimmed = visible.trim_start_matches(is_horizontal_whitespace);
        if trimmed.is_empty() {
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
        return Ok(PieHeader::Body(PieBody {
            source: code,
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
        #[cfg(feature = "editor-facts")]
        span: SourceSpan,
        next_offset: usize,
    },
}

struct PieParsedSection {
    label: String,
    value: f64,
    #[cfg(feature = "editor-facts")]
    statement_span: SourceSpan,
    #[cfg(feature = "editor-facts")]
    label_span: SourceSpan,
    value_span: SourceSpan,
}

fn parse_pie_statement(code: &str, offset: usize) -> PieStatement {
    if let Some(parsed) = parse_langium_common(code, offset) {
        return PieStatement::Common(parsed);
    }

    let (line, next_offset) = physical_line(code, offset);
    let visible = strip_inline_comment(line);
    let trimmed = visible.trim_matches(is_horizontal_whitespace);
    if trimmed.is_empty() {
        return PieStatement::Empty { next_offset };
    }

    if let Some((section, next_offset)) = parse_pie_section(code, offset) {
        return PieStatement::Section {
            section,
            next_offset,
        };
    }

    PieStatement::Unexpected {
        text: trimmed.to_string(),
        #[cfg(feature = "editor-facts")]
        span: SourceSpan::new(
            offset + visible.find(trimmed).unwrap_or_default(),
            offset + visible.find(trimmed).unwrap_or_default() + trimmed.len(),
        ),
        next_offset,
    }
}

fn parse_pie_section(code: &str, statement_start: usize) -> Option<(PieParsedSection, usize)> {
    let (input, input_start) = trim_start_with_offset(&code[statement_start..], statement_start);
    // STRING owns its physical newlines and escapes; only the value's EOL ends a section.
    let parsed_label = parse_langium_string(input, input_start)?;
    let (line, next_offset) = physical_line(code, parsed_label.raw_span.end);
    let visible = strip_inline_comment(line);
    let (rest, rest_start) = trim_start_with_offset(visible, parsed_label.raw_span.end);
    let rest = rest.strip_prefix(':')?;
    let (number_and_trailing, number_start) = trim_start_with_offset(rest, rest_start + 1);
    let number = number_and_trailing.trim_end_matches(is_horizontal_whitespace);
    let value = parse_number_pie(number)?;
    let value_span = SourceSpan::new(number_start, number_start + number.len());

    Some((
        PieParsedSection {
            label: parsed_label.value,
            value,
            #[cfg(feature = "editor-facts")]
            statement_span: SourceSpan::new(
                statement_start,
                parsed_label.raw_span.end + line.len(),
            ),
            #[cfg(feature = "editor-facts")]
            label_span: parsed_label.value_span,
            value_span,
        },
        next_offset,
    ))
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

fn trim_start_with_offset(input: &str, input_start: usize) -> (&str, usize) {
    let trimmed = input.trim_start_matches(is_horizontal_whitespace);
    (trimmed, input_start + input.len() - trimmed.len())
}

fn is_horizontal_whitespace(ch: char) -> bool {
    matches!(ch, ' ' | '\t')
}
