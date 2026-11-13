//! Class source edits derived from the family's preprocessed syntax.

use super::Tok;
use super::lexer::Lexer;
use super::parse::{
    ClassSyntax, class_editor_events, class_name_base_end, recover_interaction_action,
};
use crate::{OperationControl, SourceSpan};

pub(crate) fn editor_rename_edits(
    source: &str,
    selections: &[SourceSpan],
    new_name: &str,
) -> Option<Vec<(SourceSpan, String)>> {
    let needs_backticks = super::lexer::editor_name_requires_backticks(new_name)?;
    let control = OperationControl::new();
    let preprocessed =
        crate::preprocess::preprocess_builtin_semantic_with_directive_recovery_controlled(
            source,
            Some("classDiagram"),
            crate::preprocess::DirectiveRecoveryMode::RecoverLine,
            &control,
        )
        .ok()?
        .ok()?;
    let code = preprocessed.code();
    let syntax = ClassSyntax::lex(code, &control).ok()?;
    let mut selections = selections.to_vec();
    selections.sort_unstable_by_key(|span| (span.start, span.end));
    selections.dedup();
    if selections
        .iter()
        .any(|span| span.start >= span.end || source.get(span.start..span.end).is_none())
        || selections
            .windows(2)
            .any(|pair| pair[0].end > pair[1].start)
    {
        return None;
    }
    let mut pending = selections.as_slice();
    let mut edits = Vec::with_capacity(selections.len());
    let mut namespace_name = false;
    let mut click_target = false;
    for event in class_editor_events(&syntax.events, code, &control) {
        let event = event.ok()?;
        let recovered_list = matches!(&event, std::borrow::Cow::Owned(_));
        let (start, token, end) = match event.as_ref() {
            Ok((start, token, end)) => (*start, token, *end),
            _ => continue,
        };
        let is_namespace =
            std::mem::replace(&mut namespace_name, matches!(token, Tok::NamespaceKw));
        let is_click_target = std::mem::replace(&mut click_target, matches!(token, Tok::ClickKw));
        if !matches!(token, Tok::Name(_) | Tok::Str(_)) {
            continue;
        }
        let end = if is_click_target && let Tok::Name(name) = token {
            recover_interaction_action(code, name, SourceSpan::new(start, end))
                .map_or(end, |(target, _)| target.span.end)
        } else {
            end
        };
        let Some(container) = preprocessed
            .source
            .try_map_span(SourceSpan::new(start, end))
        else {
            continue;
        };
        let count = pending.partition_point(|span| span.end <= container.end);
        if count == 0 {
            continue;
        }
        let (selected, rest) = pending.split_at(count);
        if selected.iter().any(|span| span.start < container.start) {
            return None;
        }
        pending = rest;
        let authored = source.get(container.start..container.end)?;
        let base_end = if is_namespace {
            authored.len()
        } else {
            class_name_base_end(authored)
        };
        // Strings and authored backtick containers already protect their names. Generic
        // contents are opaque to the lexer, including secondary interaction-list targets.
        let quoted = authored.starts_with('`');
        let base = SourceSpan::new(container.start, container.start + base_end);
        if needs_backticks
            && matches!(token, Tok::Name(_))
            && !quoted
            && (authored[..base_end].contains(',') || recovered_list)
        {
            // A recovered list can continue after a generic terminal. Quote the entire list
            // then; otherwise leave its authored external generic suffix in place.
            let quote_end = if recovered_list && authored[base_end..].contains(',') {
                container.end
            } else {
                base.end
            };
            let quoted_count = selected.partition_point(|span| span.end <= quote_end);
            let mut replacement = source.get(container.start..quote_end)?.to_string();
            for span in selected[..quoted_count].iter().rev() {
                replacement.replace_range(
                    span.start - container.start..span.end - container.start,
                    new_name,
                );
            }
            edits.push((
                SourceSpan::new(container.start, quote_end),
                format!("`{replacement}`"),
            ));
            for span in &selected[quoted_count..] {
                edits.push((*span, new_name.to_string()));
            }
        } else {
            for span in selected {
                let independent = matches!(token, Tok::Name(_)) && !quoted && span.end <= base.end;
                let replacement = if needs_backticks && independent {
                    format!("`{new_name}`")
                } else if independent && name_needs_separator(new_name, &source[span.end..]) {
                    format!("{new_name} ")
                } else {
                    new_name.to_string()
                };
                edits.push((*span, replacement));
            }
        }
    }
    (pending.is_empty()
        && edits
            .windows(2)
            .all(|pair| pair[0].0.end <= pair[1].0.start))
    .then_some(edits)
}

fn name_needs_separator(name: &str, suffix: &str) -> bool {
    if suffix.chars().next().is_some_and(char::is_alphanumeric) {
        return true;
    }
    let operator = if suffix.starts_with("--") {
        "--"
    } else if suffix.starts_with("..") {
        ".."
    } else {
        return false;
    };
    // Only the adjacent line token is needed to expose a trailing `o` becoming aggregation
    // or a trailing `-` joining the line. Keep the complete candidate as the first Name.
    let mut probe = String::with_capacity(name.len() + operator.len());
    probe.push_str(name);
    probe.push_str(operator);
    !matches!(
        Lexer::new(&probe).next(),
        Some(Ok((0, Tok::Name(value), end))) if end == name.len() && value == name
    )
}
