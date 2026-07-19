use super::ast::*;
use super::model::*;
use crate::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorSemanticFacts, EditorSemanticKind,
    EditorSemanticRole, EditorSemanticSymbol, SourceSpan,
};
use indexmap::IndexMap;

pub(super) struct SemanticBuild {
    pub(super) model: ZenumlDiagramRenderModel,
    pub(super) editor_facts: EditorSemanticFacts,
    pub(super) diagnostics: Vec<SyntaxDiagnostic>,
}

pub(super) fn build(parsed: ParsedSyntax) -> SemanticBuild {
    SemanticBuilder::new(parsed).build()
}

struct ParticipantAccumulator {
    participant: ZenumlParticipant,
}

struct SemanticBuilder {
    syntax: SyntaxDocument,
    tokens: Vec<super::lexer::Token>,
    diagnostics: Vec<SyntaxDiagnostic>,
    participants: IndexMap<String, ParticipantAccumulator>,
    groups: Vec<ZenumlGroup>,
    facts: EditorSemanticFacts,
    generated_statement_id: usize,
}

#[derive(Clone)]
struct ResolveContext {
    origin: String,
    return_to: String,
}

impl SemanticBuilder {
    fn new(parsed: ParsedSyntax) -> Self {
        Self {
            syntax: parsed.document,
            tokens: parsed.tokens,
            diagnostics: parsed.diagnostics,
            participants: IndexMap::new(),
            groups: Vec::new(),
            facts: EditorSemanticFacts::new(),
            generated_statement_id: 0,
        }
    }

    fn build(mut self) -> SemanticBuild {
        self.facts.push_directive_prefix("title");
        if let Some(title) = self.syntax.title.clone() {
            self.push_payload(&title, "zenuml title", EditorSemanticKind::String);
        }
        self.facts.push_directive_prefix("accTitle");
        self.facts.push_directive_prefix("accDescr");
        if let Some(title) = self.syntax.acc_title.clone() {
            self.push_payload(
                &title,
                "zenuml accessibility title",
                EditorSemanticKind::String,
            );
        }
        if let Some(description) = self.syntax.acc_descr.clone() {
            self.push_payload(
                &description,
                "zenuml accessibility description",
                EditorSemanticKind::String,
            );
        }

        for participant in self.syntax.participants.clone() {
            self.declare_participant(&participant, None);
        }
        for (group_index, group) in self.syntax.groups.clone().into_iter().enumerate() {
            let group_id = group
                .name
                .as_ref()
                .map_or_else(|| format!("group-{group_index}"), |name| name.value.clone());
            if let Some(name) = &group.name {
                self.push_outline(
                    name,
                    "zenuml participant group",
                    EditorSemanticKind::Namespace,
                );
            }
            let mut participant_names = Vec::new();
            for participant in &group.participants {
                participant_names.push(participant.name.value.clone());
                self.declare_participant(participant, Some(group_id.clone()));
            }
            self.groups.push(ZenumlGroup {
                id: group_id,
                participant_names,
                span: group.span,
            });
        }

        let starter = self
            .syntax
            .starter
            .clone()
            .unwrap_or_else(|| SpannedText::new("_STARTER_", SourceSpan::new(0, 0)));
        let has_explicit_starter = self.syntax.starter.is_some();
        if has_explicit_starter {
            self.reference_participant(&starter, true, "zenuml starter");
        }

        let context = ResolveContext {
            origin: starter.value.clone(),
            return_to: starter.value.clone(),
        };
        let statements = self.resolve_statements(&self.syntax.statements.clone(), &context, "");

        if !has_explicit_starter && self.participants.contains_key("_STARTER_") {
            let starter = self
                .participants
                .shift_remove("_STARTER_")
                .expect("starter was present");
            self.participants
                .shift_insert(0, "_STARTER_".to_string(), starter);
        }

        for diagnostic in &self.diagnostics {
            self.facts.mark_recovered_from_parse_error(
                format!(
                    "zenuml parser recovered after parse error: {}",
                    diagnostic.message
                ),
                Some(diagnostic.span),
            );
            self.facts.push_expected_syntax(EditorExpectedSyntax::new(
                EditorExpectedSyntaxKind::Payload,
                diagnostic.span,
            ));
        }

        let model = ZenumlDiagramRenderModel {
            title: self.syntax.title.as_ref().map(|title| title.value.clone()),
            acc_title: self
                .syntax
                .acc_title
                .as_ref()
                .map(|title| title.value.clone()),
            acc_descr: self
                .syntax
                .acc_descr
                .as_ref()
                .map(|description| description.value.clone()),
            starter: has_explicit_starter.then_some(starter.value),
            participants: self
                .participants
                .into_values()
                .map(|entry| entry.participant)
                .collect(),
            groups: self.groups,
            statements,
        };
        let _ = self.tokens;
        SemanticBuild {
            model,
            editor_facts: self.facts,
            diagnostics: self.diagnostics,
        }
    }

    fn declare_participant(&mut self, syntax: &ParticipantSyntax, group_id: Option<String>) {
        let name = syntax.name.value.clone();
        let entry =
            self.participants
                .entry(name.clone())
                .or_insert_with(|| ParticipantAccumulator {
                    participant: ZenumlParticipant {
                        name: name.clone(),
                        label: None,
                        participant_type: None,
                        stereotype: None,
                        emoji: None,
                        width: None,
                        color: None,
                        group_id: None,
                        explicit: false,
                        is_starter: false,
                        declaration_span: None,
                        occurrences: Vec::new(),
                    },
                });
        let participant = &mut entry.participant;
        participant.explicit = true;
        participant.declaration_span = Some(syntax.span);
        participant.occurrences.push(syntax.name.span);
        participant.label = participant
            .label
            .take()
            .or_else(|| syntax.label.as_ref().map(|label| label.value.clone()));
        participant.participant_type = participant.participant_type.take().or_else(|| {
            syntax
                .participant_type
                .as_ref()
                .map(|value| value.value.clone())
        });
        participant.stereotype = participant
            .stereotype
            .take()
            .or_else(|| syntax.stereotype.as_ref().map(|value| value.value.clone()));
        participant.emoji = participant
            .emoji
            .take()
            .or_else(|| syntax.emoji.as_ref().map(|value| value.value.clone()));
        participant.width = participant.width.or(syntax.width);
        participant.color = participant
            .color
            .take()
            .or_else(|| syntax.color.as_ref().map(|value| value.value.clone()));
        participant.group_id = participant.group_id.take().or(group_id);

        self.push_entity(&syntax.name, "zenuml participant");
        if let Some(label) = &syntax.label {
            self.push_payload(
                label,
                "zenuml participant label",
                EditorSemanticKind::String,
            );
        }
        if let Some(stereotype) = &syntax.stereotype {
            self.push_payload(
                stereotype,
                "zenuml participant stereotype",
                EditorSemanticKind::Class,
            );
        }
        if let Some(participant_type) = &syntax.participant_type {
            self.push_payload(
                participant_type,
                "zenuml participant type",
                EditorSemanticKind::Class,
            );
        }
        if let Some(emoji) = &syntax.emoji {
            self.push_payload(
                emoji,
                "zenuml participant emoji",
                EditorSemanticKind::String,
            );
        }
        if let Some(color) = &syntax.color {
            self.push_payload(
                color,
                "zenuml participant color",
                EditorSemanticKind::String,
            );
        }
    }

    fn resolve_statements(
        &mut self,
        statements: &[StatementSyntax],
        context: &ResolveContext,
        parent_number: &str,
    ) -> Vec<ZenumlStatement> {
        statements
            .iter()
            .enumerate()
            .map(|(index, statement)| {
                let number = if parent_number.is_empty() {
                    (index + 1).to_string()
                } else {
                    format!("{parent_number}.{}", index + 1)
                };
                self.resolve_statement(statement, context, number)
            })
            .collect()
    }

    fn resolve_statement(
        &mut self,
        statement: &StatementSyntax,
        context: &ResolveContext,
        number: String,
    ) -> ZenumlStatement {
        let id = format!("zenuml-statement-{}", self.generated_statement_id);
        self.generated_statement_id += 1;
        let kind = match &statement.kind {
            StatementKindSyntax::Message(message) => {
                let from = message
                    .from
                    .as_ref()
                    .map_or_else(|| context.origin.clone(), |value| value.value.clone());
                let to = message
                    .to
                    .as_ref()
                    .map_or_else(|| context.origin.clone(), |value| value.value.clone());
                self.reference_named_participant(
                    &from,
                    message.from.as_ref().map(|v| v.span),
                    false,
                    message.from_emoji.as_ref(),
                );
                self.reference_named_participant(
                    &to,
                    message.to.as_ref().map(|v| v.span),
                    false,
                    message.to_emoji.as_ref(),
                );
                self.push_message_signature(&message.signature);
                if let Some(assignment) = &message.assignment {
                    self.push_payload(
                        assignment,
                        "zenuml assignment target",
                        EditorSemanticKind::Variable,
                    );
                }
                let nested_context = ResolveContext {
                    origin: to.clone(),
                    return_to: from.clone(),
                };
                let body = self.resolve_statements(&message.body, &nested_context, &number);
                ZenumlStatementKind::Message {
                    from,
                    to,
                    label: message.signature.value.clone(),
                    assignment: message.assignment.as_ref().map(|value| value.value.clone()),
                    style: match message.style {
                        MessageStyleSyntax::Synchronous => ZenumlMessageStyle::Synchronous,
                        MessageStyleSyntax::Asynchronous => ZenumlMessageStyle::Asynchronous,
                    },
                    body,
                }
            }
            StatementKindSyntax::Creation(creation) => {
                let from = context.origin.clone();
                let to = creation.assignment.as_ref().map_or_else(
                    || creation.constructor.value.clone(),
                    |assignment| format!("{}:{}", assignment.value, creation.constructor.value),
                );
                self.reference_named_participant(&from, None, false, None);
                self.reference_named_participant(&to, Some(creation.constructor.span), false, None);
                self.push_payload(
                    &creation.constructor,
                    "zenuml constructor",
                    EditorSemanticKind::Class,
                );
                if let Some(assignment) = &creation.assignment {
                    self.push_payload(
                        assignment,
                        "zenuml assignment target",
                        EditorSemanticKind::Variable,
                    );
                }
                let nested_context = ResolveContext {
                    origin: to.clone(),
                    return_to: from.clone(),
                };
                let body = self.resolve_statements(&creation.body, &nested_context, &number);
                ZenumlStatementKind::Creation {
                    from,
                    to,
                    constructor: creation.constructor.value.clone(),
                    assignment: creation
                        .assignment
                        .as_ref()
                        .map(|value| value.value.clone()),
                    label: format!("«create» {}", creation.signature.value),
                    body,
                }
            }
            StatementKindSyntax::Return(ret) => {
                let from = ret
                    .from
                    .as_ref()
                    .map_or_else(|| context.origin.clone(), |value| value.value.clone());
                let to = ret
                    .to
                    .as_ref()
                    .map_or_else(|| context.return_to.clone(), |value| value.value.clone());
                self.reference_named_participant(
                    &from,
                    ret.from.as_ref().map(|value| value.span),
                    false,
                    ret.from_emoji.as_ref(),
                );
                self.reference_named_participant(
                    &to,
                    ret.to.as_ref().map(|value| value.span),
                    false,
                    ret.to_emoji.as_ref(),
                );
                if let Some(value) = &ret.value {
                    self.push_payload(value, "zenuml return value", EditorSemanticKind::String);
                }
                ZenumlStatementKind::Return {
                    from,
                    to,
                    label: ret
                        .value
                        .as_ref()
                        .map_or_else(String::new, |value| value.value.clone()),
                }
            }
            StatementKindSyntax::Fragment(fragment) => {
                let fragment_kind = match fragment.kind {
                    FragmentKindSyntax::Loop => ZenumlFragmentKind::Loop,
                    FragmentKindSyntax::Alternative => ZenumlFragmentKind::Alternative,
                    FragmentKindSyntax::Parallel => ZenumlFragmentKind::Parallel,
                    FragmentKindSyntax::Optional => ZenumlFragmentKind::Optional,
                    FragmentKindSyntax::Critical => ZenumlFragmentKind::Critical,
                    FragmentKindSyntax::Section => ZenumlFragmentKind::Section,
                    FragmentKindSyntax::TryCatchFinally => ZenumlFragmentKind::TryCatchFinally,
                };
                if let Some(label) = &fragment.label {
                    self.push_payload(
                        label,
                        "zenuml fragment condition",
                        EditorSemanticKind::String,
                    );
                }
                let sections = fragment
                    .sections
                    .iter()
                    .enumerate()
                    .map(|(section_index, section)| {
                        if let Some(label) = &section.label {
                            self.push_payload(
                                label,
                                "zenuml fragment section",
                                EditorSemanticKind::String,
                            );
                        }
                        ZenumlFragmentSection {
                            label: section.label.as_ref().map(|label| label.value.clone()),
                            statements: self.resolve_statements(
                                &section.statements,
                                context,
                                &format!("{number}.{}", section_index + 1),
                            ),
                            span: section.span,
                        }
                    })
                    .collect();
                ZenumlStatementKind::Fragment {
                    fragment_kind,
                    label: fragment.label.as_ref().map(|label| label.value.clone()),
                    sections,
                }
            }
            StatementKindSyntax::Reference(reference) => {
                for participant in &reference.participants {
                    self.reference_participant(participant, false, "zenuml participant reference");
                }
                let label = reference
                    .participants
                    .iter()
                    .map(|participant| participant.value.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                ZenumlStatementKind::Reference {
                    participants: reference
                        .participants
                        .iter()
                        .map(|participant| participant.value.clone())
                        .collect(),
                    label,
                }
            }
            StatementKindSyntax::Divider(label) => {
                self.push_payload(label, "zenuml divider", EditorSemanticKind::String);
                ZenumlStatementKind::Divider {
                    label: label.value.clone(),
                }
            }
        };
        ZenumlStatement {
            id,
            number,
            comment: statement
                .comment
                .as_ref()
                .map(|comment| comment.value.clone()),
            span: statement.span,
            kind,
        }
    }

    fn reference_participant(&mut self, value: &SpannedText, starter: bool, detail: &str) {
        self.reference_named_participant(&value.value, Some(value.span), starter, None);
        if let Some(symbol) = self.facts.symbols.last_mut() {
            symbol.detail = Some(detail.to_string());
        }
    }

    fn reference_named_participant(
        &mut self,
        name: &str,
        span: Option<SourceSpan>,
        starter: bool,
        emoji: Option<&SpannedText>,
    ) {
        let entry = self
            .participants
            .entry(name.to_string())
            .or_insert_with(|| ParticipantAccumulator {
                participant: ZenumlParticipant {
                    name: name.to_string(),
                    label: None,
                    participant_type: None,
                    stereotype: None,
                    emoji: None,
                    width: None,
                    color: None,
                    group_id: None,
                    explicit: false,
                    is_starter: false,
                    declaration_span: None,
                    occurrences: Vec::new(),
                },
            });
        entry.participant.is_starter |= starter;
        if entry.participant.emoji.is_none() {
            entry.participant.emoji = emoji.map(|emoji| emoji.value.clone());
        }
        if let Some(span) = span {
            entry.participant.occurrences.push(span);
            self.facts.push_symbol(EditorSemanticSymbol::new(
                name,
                Some("zenuml participant reference".to_string()),
                EditorSemanticKind::Event,
                span,
                span,
            ));
        }
        if let Some(emoji) = emoji {
            self.push_payload(
                emoji,
                "zenuml participant emoji",
                EditorSemanticKind::String,
            );
        }
    }

    fn push_message_signature(&mut self, signature: &SpannedText) {
        let (method, span) = signature_method(signature);
        self.facts.push_symbol(EditorSemanticSymbol::with_role(
            method,
            Some("zenuml message".to_string()),
            EditorSemanticKind::Function,
            EditorSemanticRole::Payload,
            span,
            span,
        ));
    }

    fn push_entity(&mut self, value: &SpannedText, detail: &str) {
        self.facts.push_symbol(EditorSemanticSymbol::new(
            value.value.clone(),
            Some(detail.to_string()),
            EditorSemanticKind::Event,
            value.span,
            value.span,
        ));
    }

    fn push_outline(&mut self, value: &SpannedText, detail: &str, kind: EditorSemanticKind) {
        self.facts.push_symbol(EditorSemanticSymbol::outline(
            value.value.clone(),
            Some(detail.to_string()),
            kind,
            value.span,
            value.span,
        ));
    }

    fn push_payload(&mut self, value: &SpannedText, detail: &str, kind: EditorSemanticKind) {
        self.facts.push_symbol(EditorSemanticSymbol::payload(
            value.value.clone(),
            Some(detail.to_string()),
            kind,
            value.span,
            value.span,
        ));
    }
}

fn signature_method(signature: &SpannedText) -> (String, SourceSpan) {
    let text = signature.value.as_str();
    let end = text
        .char_indices()
        .find_map(|(index, ch)| matches!(ch, '(' | '.').then_some(index))
        .unwrap_or(text.len());
    let method = text[..end].trim();
    let leading = text[..end]
        .len()
        .saturating_sub(text[..end].trim_start().len());
    (
        method.to_string(),
        SourceSpan::new(
            signature.span.start + leading,
            signature.span.start + leading + method.len(),
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_message_ownership_is_not_lowered_through_sequence() {
        let source = "zenuml\n@Starter(Client)\nA.one() {\n  B.two()\n}\n";
        let parsed = super::super::parser::parse(source, super::super::lexer::lex(source));
        let built = build(parsed);
        let ZenumlStatementKind::Message { from, to, body, .. } = &built.model.statements[0].kind
        else {
            panic!("expected message");
        };
        assert_eq!((from.as_str(), to.as_str()), ("Client", "A"));
        let ZenumlStatementKind::Message { from, to, .. } = &body[0].kind else {
            panic!("expected nested message");
        };
        assert_eq!((from.as_str(), to.as_str()), ("A", "B"));
    }
}
