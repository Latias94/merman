use super::parse::Draft;
use merman_model::{OperationControl, OperationControlResult, SourceSpan};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UsecaseOccurrenceKind {
    Actor,
    Usecase,
    Boundary,
    Object,
    Relationship,
    Class,
    Property,
    Label,
}

#[derive(Clone, Copy)]
pub enum UsecaseOccurrenceRole {
    Declaration { generated: bool },
    Reference { generated: bool },
    ClassDefinition,
    Payload,
}

pub struct UsecaseOccurrence<'a> {
    pub name: &'a str,
    pub detail: Option<&'a str>,
    pub kind: UsecaseOccurrenceKind,
    pub role: UsecaseOccurrenceRole,
    pub span: SourceSpan,
    pub selection: SourceSpan,
}

pub enum UsecaseBinding {
    Resolved(UsecaseOccurrenceKind),
    Ambiguous,
    Unbound,
}

pub struct UsecaseBindings<'a>(HashMap<&'a str, Option<UsecaseOccurrenceKind>>);

impl<'a> UsecaseBindings<'a> {
    pub(super) fn from_draft(
        draft: &'a Draft,
        control: &OperationControl,
    ) -> OperationControlResult<Self> {
        use UsecaseOccurrenceKind::{Actor, Boundary, Object, Relationship, Usecase};

        // Completed declarations own identity even if a later statement is incomplete.
        let declarations = draft
            .entities
            .iter()
            .map(|entity| {
                (
                    entity.id.as_str(),
                    if entity.actor { Actor } else { Usecase },
                )
            })
            .chain(
                draft
                    .boundaries
                    .iter()
                    .map(|boundary| (boundary.entity.id.as_str(), Boundary)),
            )
            .chain(
                draft
                    .json
                    .iter()
                    .map(|json| (json.node.id.as_str(), Object)),
            )
            .chain(draft.relationships.iter().filter_map(|relation| {
                relation
                    .edge
                    .explicit_id
                    .then_some((relation.edge.id.as_str(), Relationship))
            }));
        let mut kinds: HashMap<_, Option<_>> = HashMap::new();
        for (id, kind) in declarations {
            control.checkpoint()?;
            kinds
                .entry(id)
                .and_modify(|previous| {
                    if *previous != Some(kind) || matches!(kind, Object | Relationship) {
                        *previous = None;
                    }
                })
                .or_insert(Some(kind));
        }
        // Only completed relationships can introduce implicit use cases.
        for relation in &draft.relationships {
            control.checkpoint()?;
            for entity in [&relation.source, &relation.target] {
                kinds.entry(entity.id.as_str()).or_insert(Some(Usecase));
            }
        }
        Ok(Self(kinds))
    }

    pub fn get(&self, name: &str) -> UsecaseBinding {
        match self.0.get(name) {
            Some(Some(kind)) => UsecaseBinding::Resolved(*kind),
            Some(None) => UsecaseBinding::Ambiguous,
            None => UsecaseBinding::Unbound,
        }
    }
}

/// Receives borrowed parser occurrences and the completed draft's name bindings.
pub trait UsecaseEditorObserver {
    fn begin(&mut self, directives: &[&str]);

    fn record(&mut self, occurrence: UsecaseOccurrence<'_>);

    fn bind(
        &mut self,
        bindings: &UsecaseBindings<'_>,
        control: &OperationControl,
    ) -> OperationControlResult<()>;
}
