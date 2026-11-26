use super::{LexError, Lexer, Tok, is_ecmascript_trim_char};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowchartAccessibilityDirective {
    Title,
    Description,
}

#[cfg(feature = "editor-facts")]
impl FlowchartAccessibilityDirective {
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Title => "accTitle",
            Self::Description => "accDescr",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(feature = "editor-facts")]
pub struct FlowchartAccessibilityStatement {
    pub directive: FlowchartAccessibilityDirective,
    pub complete: bool,
    pub span: SourceSpan,
}

/// Accessibility values and editor facts collected at the authoritative lexer's plain cursor.
///
/// Labels, metadata and presentation arguments are consumed by their existing lexer owners,
/// so their interior text never reaches this collector. The original source stays unchanged.
#[derive(Default)]
pub struct FlowchartAccessibility {
    title: Option<SourceSpan>,
    description: Option<SourceSpan>,
    #[cfg(feature = "editor-facts")]
    pub(super) statements: Vec<FlowchartAccessibilityStatement>,
    #[cfg(feature = "editor-facts")]
    pub(super) capture_statements: bool,
}

impl FlowchartAccessibility {
    pub const fn value_spans(&self) -> (Option<SourceSpan>, Option<SourceSpan>) {
        (self.title, self.description)
    }

    #[cfg(feature = "editor-facts")]
    pub fn statements(&self) -> &[FlowchartAccessibilityStatement] {
        &self.statements
    }
}

impl Lexer<'_> {
    pub(super) fn lex_accessibility_statement(
        &mut self,
    ) -> Option<std::result::Result<(usize, Tok, usize), LexError>> {
        // Accessibility directives are statements and cannot interrupt a vertex or an edge.
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
        // In flow.jison the closing brace wins over an empty multiline value, so an empty
        // block has no statement value and does not replace an earlier description.
        let token = if complete && (!block || content_start < content_end) {
            let value =
                self.input[content_start..content_end].trim_end_matches(is_ecmascript_trim_char);
            let span = SourceSpan::new(content_start, content_start + value.len());
            match directive {
                FlowchartAccessibilityDirective::Title => self.accessibility.title = Some(span),
                FlowchartAccessibilityDirective::Description => {
                    self.accessibility.description = Some(span);
                }
            }
            Tok::AccessibilityValue(value.to_owned())
        } else {
            Tok::Sep
        };
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
        Ok((start, token, self.pos))
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
