use super::{
    ByteSpan, EditorSymbolKind, FenceExpectedSyntax, FenceExpectedSyntaxKind, FenceReferenceGroup,
    FenceSemanticItem, FenceSemanticRole, FenceTextIndex, FenceTextIndexData, FenceTextIndexSource,
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
        index.expected_syntax.push(FenceExpectedSyntax {
            kind: expected_syntax_kind_from_core(expected.kind),
            span: ByteSpan {
                start: expected.span.start,
                end: expected.span.end,
            },
        });
    }

    for (symbol_index, symbol) in facts.symbols.iter().enumerate() {
        if symbol_index.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        let role = symbol.role;
        let kind = editor_kind_from_core(symbol.kind);
        let is_class_definition = role.is_class_definition();
        if is_class_definition {
            index.class_names.insert(symbol.name.clone());
        }
        if role.contributes_completion() && !is_class_definition {
            index.node_ids.insert(symbol.name.clone());
        }
        let item = FenceSemanticItem::new(
            symbol.name.clone(),
            symbol.detail.clone(),
            kind,
            semantic_role_from_core(role),
            ByteSpan {
                start: symbol.span.start,
                end: symbol.span.end,
            },
            ByteSpan {
                start: symbol.selection.start,
                end: symbol.selection.end,
            },
        )
        .with_rename_policy(symbol.rename_policy);
        if role.contributes_references() {
            index
                .references
                .entry(FenceReferenceGroup::from_semantic_item(&item))
                .or_default()
                .push(item.selection);
        }
        index.semantic_items.push(item);
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

fn editor_kind_from_core(kind: merman_core::EditorSemanticKind) -> EditorSymbolKind {
    match kind {
        merman_core::EditorSemanticKind::Class => EditorSymbolKind::Class,
        merman_core::EditorSemanticKind::Event => EditorSymbolKind::Event,
        merman_core::EditorSemanticKind::Function => EditorSymbolKind::Function,
        merman_core::EditorSemanticKind::Module => EditorSymbolKind::Module,
        merman_core::EditorSemanticKind::Namespace => EditorSymbolKind::Namespace,
        merman_core::EditorSemanticKind::Object => EditorSymbolKind::Object,
        merman_core::EditorSemanticKind::Package => EditorSymbolKind::Package,
        merman_core::EditorSemanticKind::Property => EditorSymbolKind::Property,
        merman_core::EditorSemanticKind::String => EditorSymbolKind::String,
        merman_core::EditorSemanticKind::Struct => EditorSymbolKind::Struct,
        merman_core::EditorSemanticKind::Variable => EditorSymbolKind::Variable,
    }
}

fn semantic_role_from_core(role: merman_core::EditorSemanticRole) -> FenceSemanticRole {
    match role {
        merman_core::EditorSemanticRole::Entity => FenceSemanticRole::Entity,
        // Class definitions retain their typed core role for completion/index ownership, while
        // the existing facts wire contract projects them to the historical outline role.
        merman_core::EditorSemanticRole::ClassDefinition => FenceSemanticRole::Outline,
        merman_core::EditorSemanticRole::Outline => FenceSemanticRole::Outline,
        merman_core::EditorSemanticRole::Payload => FenceSemanticRole::Payload,
    }
}

fn expected_syntax_kind_from_core(
    kind: merman_core::EditorExpectedSyntaxKind,
) -> FenceExpectedSyntaxKind {
    match kind {
        merman_core::EditorExpectedSyntaxKind::IdList => FenceExpectedSyntaxKind::IdList,
        merman_core::EditorExpectedSyntaxKind::NodeIdentifier => {
            FenceExpectedSyntaxKind::NodeIdentifier
        }
        merman_core::EditorExpectedSyntaxKind::Operator => FenceExpectedSyntaxKind::Operator,
        merman_core::EditorExpectedSyntaxKind::ShapeValue => FenceExpectedSyntaxKind::Shape,
        merman_core::EditorExpectedSyntaxKind::ShapeTrigger => {
            FenceExpectedSyntaxKind::ShapeTrigger
        }
        merman_core::EditorExpectedSyntaxKind::DirectionValue => FenceExpectedSyntaxKind::Direction,
        merman_core::EditorExpectedSyntaxKind::Payload => FenceExpectedSyntaxKind::Payload,
    }
}
