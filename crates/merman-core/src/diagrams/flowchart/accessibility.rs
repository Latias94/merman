#[cfg(test)]
use std::cell::Cell;

#[cfg(feature = "editor-facts")]
use crate::family::EditorFactsDemand;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "editor-facts")]
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
}

/// One source-backed interpretation of Flowchart accessibility statements.
///
/// The masked parser input preserves byte offsets and newlines. Semantic values and editor
/// directive facts are projections of the same recognized statements, so they cannot drift.
pub(super) struct FlowchartAccessibilityScan {
    pub(super) parser_input: String,
    pub(super) title: Option<String>,
    pub(super) description: Option<String>,
    #[cfg(feature = "editor-facts")]
    pub(super) statements: Vec<FlowchartAccessibilityStatement>,
}

#[cfg(any(test, feature = "compat-json"))]
pub(super) fn scan_flowchart_accessibility(
    code: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
) -> FlowchartAccessibilityScan {
    let control = OperationControl::new();
    scan_flowchart_accessibility_controlled(
        code,
        #[cfg(feature = "editor-facts")]
        editor_demand,
        &control,
    )
    .expect("a private parse control cannot be cancelled")
}

pub(super) fn scan_flowchart_accessibility_controlled(
    code: &str,
    #[cfg(feature = "editor-facts")] editor_demand: EditorFactsDemand,
    control: &OperationControl,
) -> OperationControlResult<FlowchartAccessibilityScan> {
    #[cfg(test)]
    FLOWCHART_ACCESSIBILITY_SCAN_COUNT
        .set(FLOWCHART_ACCESSIBILITY_SCAN_COUNT.get().saturating_add(1));

    control.checkpoint()?;
    let mut masked = code.as_bytes().to_vec();
    control.checkpoint()?;
    let mut title = None;
    let mut description = None;
    #[cfg(feature = "editor-facts")]
    let mut statements = Vec::new();
    let mut start = 0usize;

    while start < code.len() {
        control.checkpoint()?;
        let line_end = next_line_end_controlled(code, start, control)?;
        let line = &code[start..line_end];
        let trimmed = line.trim_start();
        let prefix_start = start + line.len().saturating_sub(trimmed.len());

        if let Some(after_prefix) = trimmed.strip_prefix("accTitle") {
            let rest = after_prefix.trim_start();
            if let Some(value) = rest.strip_prefix(':') {
                title = Some(value.trim().to_string());
                #[cfg(feature = "editor-facts")]
                if editor_demand == EditorFactsDemand::Capture {
                    statements.push(inline_statement(FlowchartAccessibilityDirective::Title));
                }
                mask_range_preserving_newlines(&mut masked, start, line_end, control)?;
                start = line_end;
                continue;
            }
        }

        let Some(after_prefix) = trimmed.strip_prefix("accDescr") else {
            start = line_end;
            continue;
        };
        let rest = after_prefix.trim_start();
        let whitespace = after_prefix.len().saturating_sub(rest.len());
        let delimiter_start = prefix_start + "accDescr".len() + whitespace;

        if let Some(value) = rest.strip_prefix(':') {
            description = Some(value.trim().to_string());
            #[cfg(feature = "editor-facts")]
            if editor_demand == EditorFactsDemand::Capture {
                statements.push(inline_statement(
                    FlowchartAccessibilityDirective::Description,
                ));
            }
            mask_range_preserving_newlines(&mut masked, start, line_end, control)?;
            start = line_end;
            continue;
        }

        let Some(after_open) = rest.strip_prefix('{') else {
            start = line_end;
            continue;
        };
        let content_start = delimiter_start + 1;
        debug_assert_eq!(content_start, line_end - after_open.len());
        let closing_brace = find_byte_controlled(code, content_start, b'}', control)?;
        let content_end = closing_brace.unwrap_or(code.len());
        if closing_brace.is_some() {
            description = Some(code[content_start..content_end].trim().to_string());
        }

        let statement_end = closing_brace.map_or(code.len(), |position| position + 1);
        #[cfg(feature = "editor-facts")]
        if editor_demand == EditorFactsDemand::Capture {
            statements.push(FlowchartAccessibilityStatement {
                directive: FlowchartAccessibilityDirective::Description,
                complete: closing_brace.is_some(),
            });
        }
        mask_range_preserving_newlines(&mut masked, start, statement_end, control)?;
        start = statement_end;
    }

    control.checkpoint()?;
    let parser_input = String::from_utf8(masked)
        .expect("replacing non-newline source bytes with ASCII spaces preserves UTF-8");
    Ok(FlowchartAccessibilityScan {
        parser_input,
        title,
        description,
        #[cfg(feature = "editor-facts")]
        statements,
    })
}

#[cfg(feature = "editor-facts")]
fn inline_statement(directive: FlowchartAccessibilityDirective) -> FlowchartAccessibilityStatement {
    FlowchartAccessibilityStatement {
        directive,
        complete: true,
    }
}

fn next_line_end_controlled(
    code: &str,
    start: usize,
    control: &OperationControl,
) -> OperationControlResult<usize> {
    Ok(find_byte_controlled(code, start, b'\n', control)?
        .map_or(code.len(), |position| position + 1))
}

fn find_byte_controlled(
    code: &str,
    start: usize,
    needle: u8,
    control: &OperationControl,
) -> OperationControlResult<Option<usize>> {
    for (chunk_index, chunk) in code.as_bytes()[start..].chunks(4096).enumerate() {
        control.checkpoint()?;
        if let Some(relative) = chunk.iter().position(|byte| *byte == needle) {
            return Ok(Some(start + chunk_index * 4096 + relative));
        }
    }
    control.checkpoint()?;
    Ok(None)
}

fn mask_range_preserving_newlines(
    bytes: &mut [u8],
    start: usize,
    end: usize,
    control: &OperationControl,
) -> OperationControlResult<()> {
    for chunk in bytes[start..end].chunks_mut(4096) {
        control.checkpoint()?;
        for byte in chunk {
            if *byte != b'\n' && *byte != b'\r' {
                *byte = b' ';
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "editor-facts")]
    fn omit_demand_preserves_accessibility_semantics_without_a_statement_journal() {
        use crate::family::EditorFactsDemand;

        let source = "flowchart TD\naccTitle: Checkout\naccDescr {\nFirst line\n}\nA --> B\n";
        let control = OperationControl::new();
        let omitted =
            scan_flowchart_accessibility_controlled(source, EditorFactsDemand::Omit, &control)
                .unwrap();
        let captured =
            scan_flowchart_accessibility_controlled(source, EditorFactsDemand::Capture, &control)
                .unwrap();

        assert!(omitted.statements.is_empty());
        assert_eq!(captured.statements.len(), 2);
        assert_eq!(omitted.title, captured.title);
        assert_eq!(omitted.description, captured.description);
        assert_eq!(omitted.parser_input, captured.parser_input);
    }

    #[test]
    fn scan_projects_values_directives_and_offset_preserving_input_once() {
        reset_flowchart_accessibility_scan_count();
        let source = concat!(
            "flowchart TD\n",
            "accTitle: Checkout\n",
            "accDescr {\n",
            "  First line\n",
            "  second line\n",
            "} A --> B\n",
            "A --> B\n",
        );

        let scan = scan_flowchart_accessibility(
            source,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
        );

        assert_eq!(flowchart_accessibility_scan_count(), 1);
        assert_eq!(scan.title.as_deref(), Some("Checkout"));
        assert_eq!(
            scan.description.as_deref(),
            Some("First line\n  second line")
        );
        #[cfg(feature = "editor-facts")]
        assert_eq!(
            scan.statements
                .iter()
                .map(|statement| statement.directive)
                .collect::<Vec<_>>(),
            vec![
                FlowchartAccessibilityDirective::Title,
                FlowchartAccessibilityDirective::Description,
            ]
        );
        assert_eq!(scan.parser_input.len(), source.len());
        assert_eq!(
            scan.parser_input.match_indices('\n').collect::<Vec<_>>(),
            source.match_indices('\n').collect::<Vec<_>>()
        );
        assert!(scan.parser_input.contains("flowchart TD"));
        assert!(scan.parser_input.contains("A --> B"));
        assert!(!scan.parser_input.contains("Checkout"));
        assert!(!scan.parser_input.contains("First line"));

        assert!(scan.parser_input.contains("A --> B"));
    }

    #[test]
    fn scan_observes_cancellation_inside_long_accessibility_payloads() {
        let source = format!("flowchart TD\naccDescr {{\n{}", "x".repeat(32 * 1024));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(5);

        assert!(matches!(
            scan_flowchart_accessibility_controlled(
                &source,
                #[cfg(feature = "editor-facts")]
                EditorFactsDemand::Capture,
                &control,
            ),
            Err(crate::OperationCancelled { .. })
        ));
        assert!(control.is_cancelled());
    }

    #[test]
    fn scan_uses_utf8_byte_spans_and_ignores_unterminated_block_semantics() {
        let source = concat!(
            "flowchart TD\n",
            "  accTitle : 结账流程\n",
            "accDescr {\n  第一行\n  第二行",
        );

        let scan = scan_flowchart_accessibility(
            source,
            #[cfg(feature = "editor-facts")]
            EditorFactsDemand::Capture,
        );

        assert_eq!(scan.title.as_deref(), Some("结账流程"));
        assert_eq!(scan.description, None);
        #[cfg(feature = "editor-facts")]
        assert!(!scan.statements.last().unwrap().complete);
    }
}
