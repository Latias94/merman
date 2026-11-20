use super::{Lexer, Tok, flowchart_subgraph_symbol_id};
use crate::preprocess::{
    DirectiveRecoveryMode, preprocess_builtin_semantic_with_directive_recovery_controlled,
};
use crate::{OperationControl, SourceSpan};

/// Check the actual replacement ranges against Mermaid's preprocessor and target-list grammar.
/// Every replacement must survive preprocessing and still occupy one whole target.
pub(crate) fn rename_preserves_targets(
    source: &str,
    old_name: &str,
    new_name: &str,
    targets: &[SourceSpan],
) -> bool {
    let Ok(Ok(preprocessed)) = preprocess_builtin_semantic_with_directive_recovery_controlled(
        source,
        Some("flowchart-v2"),
        DirectiveRecoveryMode::RecoverLine,
        &OperationControl::new(),
    ) else {
        return false;
    };
    let code = preprocessed.code();
    let renaming_list = old_name.contains(',') || new_name.contains(',');
    let mut remaining = targets.iter().peekable();
    let mut consume = |span: SourceSpan| {
        code.get(span.start..span.end) == Some(new_name)
            && remaining
                .next_if(|target| preprocessed.source.try_map_span(span) == Some(**target))
                .is_some()
    };
    let mut changes_inline_class = false;
    for token in Lexer::recovering(code) {
        match token {
            Ok((start, Tok::Id(id), end)) => {
                changes_inline_class = if consume(SourceSpan::new(start, end)) {
                    renaming_list
                } else {
                    // `a,b:::hot` defines one literal vertex and styles the vertices `a` and `b`.
                    id.contains(',')
                        && id
                            .split(',')
                            .any(|target| target == old_name || target == new_name)
                };
                continue;
            }
            Ok((_, Tok::NodeLabel(_) | Tok::ShapeData(_), _)) => continue,
            // FlowDB calls setClass on the vertex itself for an inline class.
            Ok((_, Tok::StyleSep, _)) if changes_inline_class => return false,
            Ok((_, Tok::ClassAssignStmt(stmt), _)) => {
                for span in stmt.target_spans {
                    consume(span);
                }
            }
            Ok((_, Tok::ClickStmt(stmt), _)) => {
                for span in stmt.id_spans {
                    consume(span);
                }
            }
            Ok((_, Tok::StyleStmt(stmt), _)) => {
                if let Some(span) = stmt.target_span {
                    consume(span);
                }
            }
            Ok((_, Tok::SubgraphHeader(header), _)) => {
                if let Some((_, span)) = flowchart_subgraph_symbol_id(&header) {
                    consume(span);
                }
            }
            _ => {}
        }
        changes_inline_class = false;
    }
    remaining.peek().is_none()
}
