//! Info source parsing and semantic model construction.

#[cfg(feature = "editor-facts")]
use merman_core_preprocess::scan::langium::LangiumCommonFact;
use merman_core_preprocess::scan::langium::{
    LangiumCommonDiagnostic, LangiumCommonParse, parse_langium_common,
};
use merman_model::families::info::InfoDiagramModel;
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

enum InfoStatement {
    Common(LangiumCommonParse),
    ShowInfo {
        #[cfg(feature = "editor-facts")]
        span: SourceSpan,
        consumed: usize,
    },
    Empty {
        next_offset: usize,
    },
    Unexpected {
        ch: char,
        skipped: usize,
        bad_offset: usize,
        #[cfg(feature = "editor-facts")]
        span: SourceSpan,
        next_offset: usize,
    },
}

/// Reads the next statement at a UTF-8 byte boundary before the end of the source.
fn parse_info_statement(code: &str, offset: usize, allow_show_info: bool) -> Option<InfoStatement> {
    let rest = code.get(offset..)?;
    if rest.is_empty() {
        return None;
    }
    if let Some(parsed) = parse_langium_common(code, offset) {
        return Some(InfoStatement::Common(parsed));
    }

    let (line, next_offset) = physical_line(rest, offset);
    let visible = strip_inline_comment(line);
    let trimmed = visible.trim();
    if trimmed.is_empty() {
        return Some(InfoStatement::Empty { next_offset });
    }

    let leading = visible.len() - visible.trim_start().len();
    if allow_show_info && let Some(token_len) = keyword_token_len(visible.trim_start(), "showInfo")
    {
        #[cfg(feature = "editor-facts")]
        let start = offset + leading;
        return Some(InfoStatement::ShowInfo {
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(start, start + token_len),
            consumed: leading + token_len,
        });
    }

    let bad_offset = offset + visible.find(trimmed).unwrap_or_default();
    Some(InfoStatement::Unexpected {
        ch: trimmed.chars().next().unwrap_or('?'),
        skipped: trimmed.chars().count(),
        bad_offset,
        #[cfg(feature = "editor-facts")]
        span: SourceSpan::new(bad_offset, bad_offset + trimmed.len()),
        next_offset,
    })
}

#[derive(Debug)]
pub enum InfoHeader<'source> {
    Empty,
    ExpectedInfo(SourceSpan),
    Body(InfoBody<'source>),
}

pub fn info_body_start_controlled<'source>(
    code: &'source str,
    control: &OperationControl,
) -> OperationControlResult<InfoHeader<'source>> {
    control.checkpoint()?;
    let mut offset = 0usize;
    while offset < code.len() {
        control.checkpoint()?;
        let (line, next_offset) = physical_line(&code[offset..], offset);
        let visible = strip_inline_comment(line);
        let trimmed = visible.trim_start();
        if trimmed.trim().is_empty() {
            offset = next_offset;
            continue;
        }
        let leading = visible.len() - trimmed.len();
        let header_start = offset + leading;
        let Some(header_len) = keyword_token_len(trimmed, "info") else {
            let token_len = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
            return Ok(InfoHeader::ExpectedInfo(SourceSpan::new(
                header_start,
                header_start + token_len,
            )));
        };
        return Ok(InfoHeader::Body(InfoBody {
            source: code,
            offset: header_start + header_len,
        }));
    }
    Ok(InfoHeader::Empty)
}

#[derive(Debug)]
pub struct InfoBody<'source> {
    source: &'source str,
    offset: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InfoBodyIssue {
    Common(LangiumCommonDiagnostic),
    Unexpected {
        ch: char,
        skipped: usize,
        bad_offset: usize,
    },
}

/// Observations from the active statement in the supplied parser input coordinates.
#[cfg(feature = "editor-facts")]
pub enum InfoObservation<'statement> {
    CommonRecovery(&'statement LangiumCommonDiagnostic),
    Common(&'statement LangiumCommonFact),
    ShowInfo(SourceSpan),
    Unexpected(SourceSpan),
}

impl InfoBody<'_> {
    /// Resumes after the Info header and builds the model in the same pass.
    ///
    /// Recovery retains the first issue. It still reports later issues to an observer,
    /// and a rich observer also receives the surviving statement facts in source order.
    pub fn parse_controlled(
        self,
        #[cfg(feature = "editor-facts")] capture_facts: bool,
        #[cfg(feature = "editor-facts")] mut observe: Option<&mut dyn FnMut(InfoObservation<'_>)>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<InfoDiagramModel, InfoBodyIssue>> {
        let mut offset = self.offset;
        let mut show_info = false;
        let mut common_seen = false;
        let mut first_issue = None;

        while offset < self.source.len() {
            control.checkpoint()?;
            let statement = parse_info_statement(self.source, offset, !show_info && !common_seen)
                .expect("the Info parser advances over complete source tokens");
            match statement {
                InfoStatement::Common(parsed) => {
                    if let Some(diagnostic) = parsed.diagnostic {
                        #[cfg(feature = "editor-facts")]
                        if let Some(observe) = observe.as_deref_mut() {
                            observe(InfoObservation::CommonRecovery(&diagnostic));
                        }
                        first_issue.get_or_insert(InfoBodyIssue::Common(diagnostic));
                    }
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                        observe(InfoObservation::Common(&parsed.fact));
                    }
                    common_seen = true;
                    offset += parsed.consumed;
                }
                InfoStatement::ShowInfo {
                    #[cfg(feature = "editor-facts")]
                    span,
                    consumed,
                } => {
                    show_info = true;
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut().filter(|_| capture_facts) {
                        observe(InfoObservation::ShowInfo(span));
                    }
                    offset += consumed;
                }
                InfoStatement::Empty { next_offset } => offset = next_offset,
                InfoStatement::Unexpected {
                    ch,
                    skipped,
                    bad_offset,
                    #[cfg(feature = "editor-facts")]
                    span,
                    next_offset,
                } => {
                    #[cfg(feature = "editor-facts")]
                    if let Some(observe) = observe.as_deref_mut() {
                        observe(InfoObservation::Unexpected(span));
                    }
                    first_issue.get_or_insert(InfoBodyIssue::Unexpected {
                        ch,
                        skipped,
                        bad_offset,
                    });
                    offset = next_offset;
                }
            }
        }

        if let Some(issue) = first_issue {
            return Ok(Err(issue));
        }
        control.checkpoint()?;
        Ok(Ok(InfoDiagramModel { show_info }))
    }
}

fn keyword_token_len(input: &str, keyword: &str) -> Option<usize> {
    let rest = input.strip_prefix(keyword)?;
    (rest.is_empty()
        || rest.starts_with("%%")
        || rest.chars().next().is_some_and(char::is_whitespace))
    .then_some(keyword.len())
}

fn physical_line(rest: &str, offset: usize) -> (&str, usize) {
    if let Some(newline) = rest.find('\n') {
        let line = rest[..newline]
            .strip_suffix('\r')
            .unwrap_or(&rest[..newline]);
        (line, offset + newline + 1)
    } else {
        (rest, offset + rest.len())
    }
}

fn strip_inline_comment(line: &str) -> &str {
    match line.find("%%") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_reject_offsets_outside_utf8_boundaries() {
        assert!(parse_info_statement("info\né", 6, false).is_none());
        assert!(parse_info_statement("info", usize::MAX, false).is_none());
    }

    #[test]
    fn statements_distinguish_empty_lines_from_end_of_input() {
        assert!(parse_info_statement("", 0, false).is_none());
        assert!(parse_info_statement("info", 4, false).is_none());
        assert!(matches!(
            parse_info_statement("info\n\n", 5, false),
            Some(InfoStatement::Empty { next_offset: 6 })
        ));
    }

    #[test]
    fn empty_header_observes_cancellation() {
        let control = OperationControl::new();
        control.cancel();
        assert!(info_body_start_controlled("", &control).is_err());
    }

    #[test]
    fn info_body_checkpoints_success_before_returning_the_model() {
        let parse = |source, checkpoints| {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(checkpoints);
            let InfoHeader::Body(body) = info_body_start_controlled(source, &control)? else {
                panic!("expected an Info body");
            };
            body.parse_controlled(
                #[cfg(feature = "editor-facts")]
                false,
                #[cfg(feature = "editor-facts")]
                None,
                &control,
            )
        };

        assert!(parse("info showInfo", 3).is_err());
        assert!(parse("info showInfo", 4).unwrap().unwrap().show_info);
        assert_eq!(
            parse("info wrong", 3).unwrap().unwrap_err(),
            InfoBodyIssue::Unexpected {
                ch: 'w',
                skipped: 5,
                bad_offset: 5,
            }
        );
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn info_recovery_observes_later_common_field_without_replacing_first_issue() {
        let source = "info showInfo\r\n  λβ  \r\naccDescr { 部分";
        let unknown = source.find("λβ").unwrap();
        let show_info = SourceSpan::new(5, "info showInfo".len());
        let unexpected = SourceSpan::new(unknown, unknown + "λβ".len());
        let missing_close = SourceSpan::new(source.len(), source.len());
        let descr = SourceSpan::new(source.find("部分").unwrap(), source.len());

        for capture in [false, true] {
            let control = OperationControl::new();
            let InfoHeader::Body(body) = info_body_start_controlled(source, &control).unwrap()
            else {
                panic!("expected an Info body");
            };
            let mut observations = Vec::new();
            let mut observe = |fact: InfoObservation<'_>| {
                let entry = match fact {
                    InfoObservation::CommonRecovery(diagnostic) => {
                        ("common error", diagnostic.span)
                    }
                    InfoObservation::Common(fact) => (fact.field.directive(), fact.value_span),
                    InfoObservation::ShowInfo(span) => ("showInfo", span),
                    InfoObservation::Unexpected(span) => ("unexpected", span),
                };
                observations.push(entry);
            };
            assert_eq!(
                body.parse_controlled(capture, Some(&mut observe), &control)
                    .unwrap()
                    .unwrap_err(),
                InfoBodyIssue::Unexpected {
                    ch: 'λ',
                    skipped: 2,
                    bad_offset: unknown,
                }
            );
            if capture {
                assert_eq!(
                    observations,
                    [
                        ("showInfo", show_info),
                        ("unexpected", unexpected),
                        ("common error", missing_close),
                        ("accDescr", descr),
                    ]
                );
            } else {
                assert_eq!(
                    observations,
                    [("unexpected", unexpected), ("common error", missing_close)]
                );
            }
        }
    }
}
