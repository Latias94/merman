use merman_core_preprocess::{MappedSource, SourceEdit};

#[cfg(feature = "editor-facts")]
use crate::{EditorExpectedSyntax, EditorExpectedSyntaxKind};
use crate::{OperationControl, OperationControlResult, SourceSpan};

/// Parser text together with an exact, composable map back to the caller's source.
#[derive(Debug, Clone)]
pub struct PreprocessedSource {
    mapped: MappedSource,
    #[cfg(feature = "editor-facts")]
    global_expected_syntax: Vec<EditorExpectedSyntax>,
    #[cfg(feature = "editor-facts")]
    global_directive_prefixes: Vec<String>,
    #[cfg(feature = "editor-facts")]
    recovered_incomplete_directive: bool,
    frontmatter_line_offset: usize,
    first_line_column_offset: usize,
}

impl PreprocessedSource {
    pub fn new(source: &str) -> Self {
        Self::new_controlled(source, false, &OperationControl::new())
            .expect("a private parse control cannot be cancelled")
    }

    pub(super) fn new_controlled(
        source: &str,
        capture_spans: bool,
        control: &OperationControl,
    ) -> OperationControlResult<Self> {
        Ok(Self {
            mapped: MappedSource::new_controlled(source, capture_spans, control)?,
            #[cfg(feature = "editor-facts")]
            global_expected_syntax: Vec::new(),
            #[cfg(feature = "editor-facts")]
            global_directive_prefixes: Vec::new(),
            #[cfg(feature = "editor-facts")]
            recovered_incomplete_directive: false,
            frontmatter_line_offset: 0,
            first_line_column_offset: 0,
        })
    }

    pub fn text(&self) -> &str {
        self.mapped.text()
    }

    #[cfg(feature = "editor-facts")]
    pub(super) fn copy_global_editor_evidence_from(&mut self, source: &Self) {
        self.global_expected_syntax
            .clone_from(&source.global_expected_syntax);
        self.global_directive_prefixes
            .clone_from(&source.global_directive_prefixes);
    }

    #[cfg(any(feature = "presentation", feature = "diagram-agentflow"))]
    pub(crate) fn parser_position_offset(&self) -> (usize, usize) {
        (self.frontmatter_line_offset, self.first_line_column_offset)
    }

    pub(super) fn record_frontmatter_removal(&mut self, end: usize) {
        self.frontmatter_line_offset += self.text()[..end]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count();
        self.first_line_column_offset = 0;
    }

    pub(super) fn set_first_line_column_offset(&mut self, columns: usize) {
        self.first_line_column_offset = columns;
    }

    pub fn try_map_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        self.mapped.try_map_span(span)
    }

    pub(super) fn prepare_span_mapping_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        self.mapped.prepare_span_mapping_controlled(control)
    }

    #[cfg(feature = "editor-facts")]
    pub(crate) fn may_drop_editor_spans_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<bool> {
        self.mapped.may_drop_spans_controlled(control)
    }

    pub(crate) fn remap_parse_error_controlled(
        &self,
        error: crate::Error,
        control: &OperationControl,
    ) -> OperationControlResult<crate::Error> {
        Ok(match error {
            crate::Error::DiagramParse {
                diagram_type,
                diagnostic,
            } => {
                let diagnostic = match diagnostic.span() {
                    Some(span) => {
                        self.prepare_span_mapping_controlled(control)?;
                        match self.try_map_span(span) {
                            Some(remapped) => diagnostic.map_span(|_| remapped),
                            None => diagnostic.without_span(),
                        }
                    }
                    None => diagnostic,
                };
                crate::Error::DiagramParse {
                    diagram_type,
                    diagnostic,
                }
            }
            error => error,
        })
    }

    pub(crate) fn remap_warning_facts_controlled(
        &self,
        facts: &mut [crate::DiagramWarningFact],
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        for chunk in facts.chunks_mut(128) {
            control.checkpoint()?;
            for fact in chunk {
                if fact.span.is_some() || fact.fix_span.is_some() {
                    self.prepare_span_mapping_controlled(control)?;
                    self.remap_warning_fact_spans(fact);
                }
            }
        }
        control.checkpoint()
    }

    pub(super) fn remap_warning_fact_spans(&self, fact: &mut crate::DiagramWarningFact) {
        let source_span = fact.span;
        let remapped_span = source_span.and_then(|span| self.try_map_span(span));
        fact.span = remapped_span;
        fact.fix_span = match (fact.fix_span, source_span, remapped_span) {
            (Some(fix_span), Some(source_span), Some(remapped_span))
                if fix_span.start == fix_span.end && fix_span.start == source_span.end =>
            {
                Some(SourceSpan::new(remapped_span.end, remapped_span.end))
            }
            (Some(fix_span), _, _) => self.try_map_span(fix_span),
            (None, _, _) => None,
        };
    }

    #[cfg(feature = "editor-facts")]
    pub(super) fn try_map_enclosing_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        self.mapped.try_map_enclosing_span(span)
    }

    #[cfg(feature = "editor-facts")]
    pub(crate) fn global_expected_syntax(&self) -> &[EditorExpectedSyntax] {
        &self.global_expected_syntax
    }

    #[cfg(feature = "editor-facts")]
    pub(crate) fn global_directive_prefixes(&self) -> &[String] {
        &self.global_directive_prefixes
    }

    #[cfg(feature = "editor-facts")]
    pub(crate) const fn recovered_incomplete_directive(&self) -> bool {
        self.recovered_incomplete_directive
    }

    #[cfg(feature = "editor-facts")]
    pub(super) fn mark_recovered_incomplete_directive(&mut self) {
        self.recovered_incomplete_directive = true;
    }

    #[cfg(feature = "editor-facts")]
    pub(super) fn record_global_directive_prefix(&mut self, prefix: String) {
        if !self.global_directive_prefixes.contains(&prefix) {
            self.global_directive_prefixes.push(prefix);
        }
    }

    #[cfg(feature = "editor-facts")]
    pub(super) fn record_global_expected_syntax(
        &mut self,
        kind: EditorExpectedSyntaxKind,
        span: SourceSpan,
    ) {
        let Some(span) = self.try_map_span(span) else {
            return;
        };
        self.global_expected_syntax
            .push(EditorExpectedSyntax::new(kind, span));
    }

    pub fn into_text(self) -> String {
        self.mapped.into_text()
    }

    pub(super) fn apply_edits(
        &mut self,
        edits: Vec<SourceEdit>,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        self.mapped.apply_edits(edits, control)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaps_diagnostics_and_warning_fixes_after_cancelled_mapping() {
        let removed = "discard\r\n";
        let original = format!("{removed}前😀TAIL");
        let mut source = PreprocessedSource::new(&original);
        let control = OperationControl::new();
        source
            .apply_edits(vec![SourceEdit::delete(0..removed.len())], &control)
            .unwrap();

        let len = "前😀".len();
        let span = SourceSpan::new(0, len);
        let expected = SourceSpan::new(removed.len(), removed.len() + len);
        let error = || {
            crate::Error::diagram_parse_diagnostic(
                "flowchart-v2",
                crate::ParseDiagnostic::new("unexpected label")
                    .with_span(span, crate::ParseDiagnosticSpanKind::Exact),
            )
        };
        let cancelled = OperationControl::new();
        cancelled.cancel_after_checkpoints(1);
        assert!(
            source
                .remap_parse_error_controlled(error(), &cancelled)
                .is_err()
        );

        let remapped = source
            .remap_parse_error_controlled(error(), &control)
            .unwrap();
        assert!(
            matches!(remapped, crate::Error::DiagramParse { diagnostic, .. }
            if diagnostic.span() == Some(expected))
        );

        let mut facts = [crate::DiagramWarningFact::new("warning", "label")
            .with_span(span)
            .with_fix_span(SourceSpan::new(span.end, span.end))];
        source
            .remap_warning_facts_controlled(&mut facts, &control)
            .unwrap();
        assert_eq!(facts[0].span, Some(expected));
        assert_eq!(
            facts[0].fix_span,
            Some(SourceSpan::new(expected.end, expected.end))
        );
    }
}
