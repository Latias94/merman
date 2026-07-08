use super::{
    ByteSpan, EditorSymbolKind, FenceExpectedSyntax, FenceExpectedSyntaxKind, FenceReferenceGroup,
    FenceSemanticItem, FenceSemanticRole, FenceTextIndex, FenceTextIndexSource,
    is_class_definition_detail,
};

pub(super) fn from_core_facts(facts: merman_core::EditorSemanticFacts) -> FenceTextIndex {
    let mut index = FenceTextIndex::default();
    let source_mapped_spans = facts.span_coordinate_space.is_original_source();

    index.source = match (facts.completeness, source_mapped_spans) {
        (merman_core::EditorSemanticCompleteness::Complete, true) => {
            FenceTextIndexSource::ParserComplete
        }
        (merman_core::EditorSemanticCompleteness::Complete, false) => {
            FenceTextIndexSource::ParserCompleteDegradedSpans
        }
        (merman_core::EditorSemanticCompleteness::Recovered, true) => {
            FenceTextIndexSource::ParserRecovered
        }
        (merman_core::EditorSemanticCompleteness::Recovered, false) => {
            FenceTextIndexSource::ParserRecoveredDegradedSpans
        }
    };
    index.directive_prefixes.extend(facts.directive_prefixes);
    if source_mapped_spans {
        index
            .expected_syntax
            .extend(
                facts
                    .expected_syntax
                    .into_iter()
                    .map(|expected| FenceExpectedSyntax {
                        kind: expected_syntax_kind_from_core(expected.kind),
                        span: ByteSpan {
                            start: expected.span.start,
                            end: expected.span.end,
                        },
                    }),
            );
    }

    for symbol in facts.symbols {
        let role = symbol.role;
        let kind = editor_kind_from_core(symbol.kind);
        let is_class_definition = is_class_definition_detail(symbol.detail.as_deref());
        if is_class_definition {
            index.class_names.insert(symbol.name.clone());
        }
        if role.contributes_completion() && !is_class_definition {
            index.node_ids.insert(symbol.name.clone());
        }
        if !source_mapped_spans {
            continue;
        }

        let item = FenceSemanticItem {
            name: symbol.name,
            detail: symbol.detail,
            kind,
            role: semantic_role_from_core(role),
            span: ByteSpan {
                start: symbol.span.start,
                end: symbol.span.end,
            },
            selection: ByteSpan {
                start: symbol.selection.start,
                end: symbol.selection.end,
            },
        };
        if role.contributes_references() {
            index
                .references
                .entry(FenceReferenceGroup::from_semantic_item(&item))
                .or_default()
                .push(item.selection);
        }
        if role.contributes_outline() {
            index.outline_items.push(item.to_line_item());
        }
        index.semantic_items.push(item);
    }

    index.outline_items.sort_by(|left, right| {
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
    index
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
        merman_core::EditorExpectedSyntaxKind::ShapeValue => FenceExpectedSyntaxKind::Shape,
        merman_core::EditorExpectedSyntaxKind::ShapeTrigger => {
            FenceExpectedSyntaxKind::ShapeTrigger
        }
        merman_core::EditorExpectedSyntaxKind::DirectionValue => FenceExpectedSyntaxKind::Direction,
        merman_core::EditorExpectedSyntaxKind::Payload => FenceExpectedSyntaxKind::Payload,
    }
}
