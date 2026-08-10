use super::{
    FenceReferenceGroup, FenceTextIndex, FenceTextIndexData, FenceTextIndexSource,
    byte_span_from_source,
};

#[cfg(test)]
pub(super) fn from_core_facts(facts: merman_core::EditorSemanticFacts) -> FenceTextIndex {
    let cancellation = crate::AnalysisCancellationToken::new();
    from_core_facts_cancellable(&facts, &cancellation)
        .expect("a private analysis cancellation token cannot be cancelled")
}

pub(super) fn from_core_facts_cancellable(
    facts: &merman_core::EditorSemanticFacts,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<FenceTextIndex, crate::AnalysisCancelled> {
    cancellation.checkpoint()?;
    let source = match facts.completeness {
        merman_core::EditorSemanticCompleteness::Complete => FenceTextIndexSource::ParserComplete,
        merman_core::EditorSemanticCompleteness::Recovered => FenceTextIndexSource::ParserRecovered,
    };
    let mut index = FenceTextIndexData {
        completion_vocabulary: facts.completion_vocabulary,
        source,
        ..FenceTextIndexData::default()
    };
    index.lexeme_failure = facts.lexeme_failure();
    index.lexemes.reserve(facts.lexemes().len());
    for (lexeme_index, lexeme) in facts.lexemes().iter().enumerate() {
        if lexeme_index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        index.lexemes.push(*lexeme);
    }
    index
        .directive_prefixes
        .extend(facts.directive_prefixes.iter().cloned());
    index.expected_syntax.reserve(facts.expected_syntax.len());
    for (expected_index, expected) in facts.expected_syntax.iter().enumerate() {
        if expected_index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        index.expected_syntax.push(*expected);
    }

    for (symbol_index, symbol) in facts.symbols.iter().enumerate() {
        if symbol_index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        let role = symbol.role;
        if role.contributes_references() {
            index
                .references
                .entry(FenceReferenceGroup::from_semantic_item(symbol))
                .or_default()
                .push(byte_span_from_source(symbol.selection));
        }
        index.semantic_items.push(symbol.clone());
    }

    cancellation.checkpoint()?;
    index.semantic_items.sort_by(|left, right| {
        (
            left.span.start,
            left.span.end,
            left.name.as_str(),
            left.selection.start,
            left.selection.end,
        )
            .cmp(&(
                right.span.start,
                right.span.end,
                right.name.as_str(),
                right.selection.start,
                right.selection.end,
            ))
    });
    index.build_point_indexes(cancellation)?;
    cancellation.checkpoint()?;
    Ok(FenceTextIndex::from_data(index))
}
