use super::PreprocessedSource;
use crate::{EditorSemanticFacts, OperationControl, OperationControlResult};

impl PreprocessedSource {
    pub(crate) fn remap_editor_facts_controlled(
        &self,
        facts: &mut EditorSemanticFacts,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let original_symbol_count = facts.symbols.len();
        let mut remapped_symbols = Vec::with_capacity(original_symbol_count);
        for (index, mut symbol) in std::mem::take(&mut facts.symbols).into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let Some(span) = self.try_map_span(symbol.span) else {
                continue;
            };
            let Some(selection) = self.try_map_span(symbol.selection) else {
                continue;
            };
            symbol.span = span;
            symbol.selection = selection;
            remapped_symbols.push(symbol);
        }
        facts.symbols = remapped_symbols;

        let mut dropped_diagnostic_spans = 0usize;
        for (index, diagnostic) in facts.diagnostics.iter_mut().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(span) = diagnostic.span {
                diagnostic.span = self.try_map_span(span);
                dropped_diagnostic_spans += usize::from(diagnostic.span.is_none());
            }
        }
        let original_expected_count = facts.expected_syntax.len();
        let mut remapped_expected = Vec::with_capacity(original_expected_count);
        for (index, mut expected) in std::mem::take(&mut facts.expected_syntax)
            .into_iter()
            .enumerate()
        {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            if let Some(span) = self.try_map_span(expected.span) {
                expected.span = span;
                remapped_expected.push(expected);
            }
        }
        facts.expected_syntax = remapped_expected;

        let dropped_symbols = original_symbol_count - facts.symbols.len();
        let dropped_expected = original_expected_count - facts.expected_syntax.len();
        let dropped_spans = dropped_symbols + dropped_expected + dropped_diagnostic_spans;
        if dropped_spans > 0 {
            facts.mark_recovered_with_diagnostic(
                format!(
                    "dropped {dropped_spans} editor fact span(s) that crossed a preprocessing edit"
                ),
                None,
            );
        }
        control.checkpoint()
    }

    #[cfg(feature = "presentation")]
    pub(crate) fn finish_editor_semantic_facts(
        &self,
        facts: EditorSemanticFacts,
        diagram_type: &str,
        control: &OperationControl,
    ) -> OperationControlResult<EditorSemanticFacts> {
        self.finish_editor_semantic_facts_with_demand(
            facts,
            diagram_type,
            crate::family::EditorFactsDemand::Capture,
            control,
        )
    }

    pub(crate) fn finish_editor_semantic_facts_with_demand(
        &self,
        mut facts: EditorSemanticFacts,
        diagram_type: &str,
        demand: crate::family::EditorFactsDemand,
        control: &OperationControl,
    ) -> OperationControlResult<EditorSemanticFacts> {
        let family_directive_prefixes = std::mem::take(&mut facts.directive_prefixes);
        self.remap_editor_facts_controlled(&mut facts, control)?;
        facts.family_semantics = crate::family::diagram_type_editor_semantics(diagram_type)
            .expect("built-in semantic facts have typed editor family semantics");
        if !demand.captures_rich() {
            facts.retain_recovery_facts();
            return Ok(facts);
        }
        for expected in self.global_expected_syntax().chunks(128) {
            control.checkpoint()?;
            facts.expected_syntax.extend_from_slice(expected);
        }
        facts.finalize_expected_syntax_controlled(control)?;
        for (index, prefix) in self.global_directive_prefixes().iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            facts.push_directive_prefix(prefix.clone());
        }
        for (index, prefix) in family_directive_prefixes.into_iter().enumerate() {
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            facts.push_directive_prefix(prefix);
        }
        control.checkpoint()?;
        Ok(facts)
    }
}
