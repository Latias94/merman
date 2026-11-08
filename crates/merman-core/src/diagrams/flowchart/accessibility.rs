#[cfg(test)]
use std::cell::Cell;

use super::{LexError, Lexer, Tok, is_ecmascript_trim_char};
#[cfg(feature = "editor-facts")]
use crate::SourceSpan;
use crate::{OperationControl, OperationControlResult};

#[cfg(test)]
thread_local! {
    static FLOWCHART_ACCESSIBILITY_SCAN_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(super) fn reset_flowchart_accessibility_scan_count() {
    FLOWCHART_ACCESSIBILITY_SCAN_COUNT.set(0);
}

#[cfg(test)]
pub(super) fn flowchart_accessibility_scan_count() -> usize {
    FLOWCHART_ACCESSIBILITY_SCAN_COUNT.get()
}

#[cfg(test)]
pub(super) fn record_flowchart_accessibility_scan() {
    FLOWCHART_ACCESSIBILITY_SCAN_COUNT
        .set(FLOWCHART_ACCESSIBILITY_SCAN_COUNT.get().saturating_add(1));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FlowchartAccessibilityDirective {
    Title,
    Description,
}

#[cfg(feature = "editor-facts")]
impl FlowchartAccessibilityDirective {
    pub(super) const fn prefix(self) -> &'static str {
        match self {
            Self::Title => "accTitle",
            Self::Description => "accDescr",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(feature = "editor-facts")]
pub(super) struct FlowchartAccessibilityStatement {
    pub(super) directive: FlowchartAccessibilityDirective,
    pub(super) complete: bool,
    pub(super) span: SourceSpan,
}

/// Accessibility values and editor facts collected at the authoritative lexer's plain cursor.
///
/// Labels, metadata and presentation arguments are consumed by their existing lexer owners,
/// so their interior text never reaches this collector. The original source stays unchanged.
#[derive(Default)]
pub(super) struct FlowchartAccessibility {
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    #[cfg(feature = "editor-facts")]
    pub(super) statements: Vec<FlowchartAccessibilityStatement>,
    #[cfg(feature = "editor-facts")]
    pub(super) capture_statements: bool,
}

impl Lexer<'_> {
    pub(super) fn lex_accessibility_statement(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        // The grammar skips accessibility through a separator, so only admit it where a
        // statement can begin; it must not turn an invalid adjacent node into a valid one.
        if !self.at_statement_start {
            return None;
        }
        let rest = &self.input[self.pos..];
        let (directive, rest) = if let Some(rest) = rest.strip_prefix("accTitle") {
            (FlowchartAccessibilityDirective::Title, rest)
        } else {
            let rest = rest.strip_prefix("accDescr")?;
            (FlowchartAccessibilityDirective::Description, rest)
        };
        let rest = rest.trim_start_matches(is_ecmascript_trim_char);
        let block = match rest.as_bytes().first()? {
            b':' => false,
            b'{' if directive == FlowchartAccessibilityDirective::Description => true,
            _ => return None,
        };
        // The selected Jison rule consumes whitespace after ':' or '{' before its payload.
        let value = rest[1..].trim_start_matches(is_ecmascript_trim_char);
        let content_start = self.input.len() - value.len();
        Some(
            self.consume_accessibility_statement(directive, content_start, block)
                .map_err(|_| {
                    self.pos = self.input.len();
                    LexError::new("Flowchart accessibility scan cancelled")
                }),
        )
    }

    fn consume_accessibility_statement(
        &mut self,
        directive: FlowchartAccessibilityDirective,
        content_start: usize,
        block: bool,
    ) -> OperationControlResult<(usize, Tok, usize)> {
        let start = self.pos;
        let end = find_byte_controlled(
            self.input,
            content_start,
            if block { b'}' } else { b'\n' },
            self.control.as_ref(),
        )?;
        let content_end = end.unwrap_or(self.input.len());
        let complete = !block || end.is_some();
        self.pos = if block {
            end.map_or(self.input.len(), |position| position + 1)
        } else {
            content_end
        };
        if complete {
            let value = self.input[content_start..content_end]
                .trim_matches(is_ecmascript_trim_char)
                .to_owned();
            match directive {
                FlowchartAccessibilityDirective::Title => self.accessibility.title = Some(value),
                FlowchartAccessibilityDirective::Description => {
                    self.accessibility.description = Some(value);
                }
            }
        }
        #[cfg(feature = "editor-facts")]
        if self.accessibility.capture_statements {
            self.accessibility
                .statements
                .push(FlowchartAccessibilityStatement {
                    directive,
                    complete,
                    span: SourceSpan::new(start, self.pos),
                });
        }
        // A separator lets the existing grammar skip the directive without changing source
        // offsets or introducing accessibility tokens into the generated parser contract.
        Ok((start, Tok::Sep, self.pos))
    }
}

fn find_byte_controlled(
    code: &str,
    start: usize,
    needle: u8,
    control: Option<&OperationControl>,
) -> OperationControlResult<Option<usize>> {
    for (chunk_index, chunk) in code.as_bytes()[start..].chunks(4096).enumerate() {
        if let Some(control) = control {
            control.checkpoint()?;
        }
        if let Some(relative) = chunk.iter().position(|byte| *byte == needle) {
            return Ok(Some(start + chunk_index * 4096 + relative));
        }
    }
    if let Some(control) = control {
        control.checkpoint()?;
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_observes_cancellation_inside_long_accessibility_payloads() {
        let source = format!("flowchart TD\naccDescr {{\n{}", "x".repeat(32 * 1024));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(5);

        let mut lexer = Lexer::new(&source).with_control(&control);
        assert!(lexer.by_ref().any(|item| item.is_err()));
        assert!(control.is_cancelled());
        assert!(lexer.next().is_none());
    }
}
