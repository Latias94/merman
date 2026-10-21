use std::collections::HashMap;

use indexmap::IndexMap;
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Serialize, Serializer};

#[cfg(feature = "presentation")]
use super::StateDocumentId;
use super::{
    StateDiagramModel, StateDocument, StateEdge, StateLinks, StateNode, StateNote, StateRecord,
    StateRelation, StateStatement, StateStatementState, StateStyleClass,
};
use crate::resources::ModelComplexity;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StateRecordFields<'a> {
    id: &'a str,
    #[serde(rename = "type")]
    state_type: &'a str,
    descriptions: &'a [String],
    doc: Option<usize>,
    note: &'a Option<StateNote>,
    classes: &'a [String],
    styles: &'a [String],
    text_styles: &'a [String],
    start: Option<bool>,
}

impl StateRecord {
    #[cfg(feature = "compat-json")]
    pub(super) fn fields(&self) -> StateRecordFields<'_> {
        self.fields_with_document(false)
    }

    fn fields_with_document(&self, include_document: bool) -> StateRecordFields<'_> {
        StateRecordFields {
            id: &self.id,
            state_type: &self.state_type,
            descriptions: &self.descriptions,
            doc: self.doc.filter(|_| include_document).map(|id| id.0),
            note: &self.note,
            classes: &self.classes,
            styles: &self.styles,
            text_styles: &self.text_styles,
            start: self.start,
        }
    }
}

pub(super) struct StateRecords<'a> {
    pub states: &'a HashMap<String, StateRecord>,
    pub include_documents: bool,
}

impl Serialize for StateRecords<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.states.len()))?;
        for (id, state) in self.states {
            map.serialize_entry(id, &state.fields_with_document(self.include_documents))?;
        }
        map.end()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StateModelFields<'a> {
    document: FlatDocuments<'a>,
    direction: &'a str,
    acc_title: &'a Option<String>,
    acc_descr: &'a Option<String>,
    nodes: &'a [StateNode],
    edges: &'a [StateEdge],
    relations: &'a [StateRelation],
    links: &'a HashMap<String, StateLinks>,
    states: StateRecords<'a>,
    style_classes: &'a IndexMap<String, StateStyleClass>,
}

impl StateDiagramModel {
    pub(crate) fn model_complexity(&self) -> ModelComplexity {
        let mut complexity = ModelComplexity::from_serializable(&StateModelFields {
            document: FlatDocuments(&self.document),
            direction: &self.direction,
            acc_title: &self.acc_title,
            acc_descr: &self.acc_descr,
            nodes: &self.nodes,
            edges: &self.edges,
            relations: &self.relations,
            links: &self.links,
            states: StateRecords {
                states: &self.states,
                include_documents: true,
            },
            style_classes: &self.style_classes,
        });
        complexity.nesting_depth = complexity
            .nesting_depth
            .max(self.document.semantic_nesting_depth());
        complexity
    }
}

impl StateDocument {
    fn semantic_nesting_depth(&self) -> usize {
        let Some(root) = self.root else {
            return 0;
        };
        let mut maximum = 0usize;
        // Grammar documents form a tree; state records may share references to that tree.
        let mut pending = vec![(root, 0usize)];
        while let Some((id, depth)) = pending.pop() {
            maximum = maximum.max(depth);
            for statement in &self.documents[id.0] {
                if let StateStatement::State(state) = statement
                    && let Some(child) = state.doc
                {
                    pending.push((child, depth.saturating_add(1)));
                }
            }
        }
        maximum
    }
}

struct FlatDocuments<'a>(&'a StateDocument);

impl Serialize for FlatDocuments<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("root", &self.0.root.map(|id| id.0))?;
        map.serialize_entry("documents", &Documents(&self.0.documents))?;
        map.end()
    }
}

struct Documents<'a>(&'a [Vec<StateStatement>]);

impl Serialize for Documents<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for document in self.0 {
            sequence.serialize_element(&Statements(document))?;
        }
        sequence.end()
    }
}

struct Statements<'a>(&'a [StateStatement]);

impl Serialize for Statements<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.len()))?;
        for statement in self.0 {
            sequence.serialize_element(&StatementFields {
                statement,
                canonical: true,
            })?;
        }
        sequence.end()
    }
}

struct StateReferenceFields<'a> {
    state: &'a StateStatementState,
    canonical: bool,
}

impl Serialize for StateReferenceFields<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let state = self.state;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", &state.id)?;
        map.serialize_entry("type", &state.ty)?;
        map.serialize_entry("classes", &state.classes)?;
        if self.canonical {
            serialize_state_fields(&mut map, state)?;
        }
        map.end()
    }
}

fn serialize_state_fields<M: SerializeMap>(
    map: &mut M,
    state: &StateStatementState,
) -> Result<(), M::Error> {
    map.serialize_entry("idSpan", &state.id_span)?;
    map.serialize_entry("description", &state.description)?;
    map.serialize_entry("descriptions", &state.descriptions)?;
    map.serialize_entry("doc", &state.doc.map(|id| id.0))?;
    map.serialize_entry(
        "note",
        &state.note.as_ref().map(|note| StateNoteFields {
            position: &note.position,
            text: &note.text,
        }),
    )?;
    map.serialize_entry("styles", &state.styles)?;
    map.serialize_entry("textStyles", &state.text_styles)?;
    map.serialize_entry("start", &state.start)
}

#[derive(Serialize)]
struct StateNoteFields<'a> {
    position: &'a Option<String>,
    text: &'a str,
}

struct StatementFields<'a> {
    statement: &'a StateStatement,
    canonical: bool,
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    use crate::{MermaidConfig, OperationControl};

    #[test]
    fn state_document_counter_preserves_the_legacy_shallow_shape() {
        let source = concat!(
            "stateDiagram-v2\ndirection LR\naccTitle: title\naccDescr: description\n",
            "state Outer {\nstate \"Label\" as A\nA --> B: transition\n}\n",
            "classDef active fill:red\nclass A active\nstyle A color:blue\n",
            "click A href \"https://example.com\"\n",
        );
        let model = super::super::parse::parse_state_semantic_model_controlled(
            source,
            "stateDiagram-v2",
            &MermaidConfig::default(),
            #[cfg(feature = "editor-facts")]
            crate::family::EditorFactsDemand::Omit,
            &OperationControl::new(),
        )
        .unwrap()
        .model
        .unwrap();
        let scalars = [
            StateStatement::Noop,
            StateStatement::AccTitle("title".to_string()),
            StateStatement::AccDescr("description".to_string()),
        ];
        for statement in model.document.documents.iter().flatten().chain(&scalars) {
            let expected = super::super::db::stmt_to_json_shallow(statement, None);
            let fields = StatementFields {
                statement,
                canonical: false,
            };
            assert_eq!(serde_json::to_value(&fields).unwrap(), *expected.as_value());
            assert_eq!(
                ModelComplexity::from_serializable(&fields),
                ModelComplexity::from_json(expected.as_value()),
            );
        }
    }
}

impl Serialize for StatementFields<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = match self.statement {
            StateStatement::Noop => return serializer.serialize_none(),
            StateStatement::AccTitle(text) | StateStatement::AccDescr(text) => {
                return text.serialize(serializer);
            }
            _ => serializer.serialize_map(None)?,
        };
        match self.statement {
            StateStatement::State(state) => {
                map.serialize_entry("stmt", "state")?;
                map.serialize_entry("id", &state.id)?;
                map.serialize_entry("type", &state.ty)?;
                map.serialize_entry("classes", &state.classes)?;
                if self.canonical {
                    serialize_state_fields(&mut map, state)?;
                } else {
                    map.serialize_entry("description", &state.description)?;
                    map.serialize_entry("doc", &None::<()>)?;
                }
            }
            StateStatement::Relation(relation) => {
                map.serialize_entry("stmt", "relation")?;
                map.serialize_entry(
                    "state1",
                    &StateReferenceFields {
                        state: &relation.state1,
                        canonical: self.canonical,
                    },
                )?;
                map.serialize_entry(
                    "state2",
                    &StateReferenceFields {
                        state: &relation.state2,
                        canonical: self.canonical,
                    },
                )?;
                map.serialize_entry("description", &relation.description)?;
            }
            StateStatement::ClassDef { id, classes } => {
                map.serialize_entry("stmt", "classDef")?;
                map.serialize_entry("id", id)?;
                map.serialize_entry("classes", classes)?;
            }
            StateStatement::ApplyClass { ids, class_name } => {
                map.serialize_entry("stmt", "applyClass")?;
                map.serialize_entry("id", ids)?;
                map.serialize_entry("styleClass", class_name)?;
            }
            StateStatement::Style { ids, styles } => {
                map.serialize_entry("stmt", "style")?;
                map.serialize_entry("id", ids)?;
                map.serialize_entry("styleClass", styles)?;
            }
            StateStatement::Direction(direction) => {
                map.serialize_entry("stmt", "dir")?;
                map.serialize_entry("value", direction)?;
            }
            StateStatement::Click(click) => {
                map.serialize_entry("stmt", "click")?;
                map.serialize_entry("id", &click.id)?;
                map.serialize_entry("url", &click.url)?;
                map.serialize_entry("tooltip", &click.tooltip)?;
            }
            StateStatement::Noop | StateStatement::AccTitle(_) | StateStatement::AccDescr(_) => {
                unreachable!("scalar statements return before map creation");
            }
        }
        map.end()
    }
}

#[cfg(feature = "presentation")]
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct DocumentComplexity {
    pub items: usize,
    pub text_bytes: usize,
    pub depth: usize,
}

#[cfg(feature = "presentation")]
impl StateDocument {
    pub(super) fn complexities(&self) -> Vec<DocumentComplexity> {
        let mut completed = vec![None; self.documents.len()];
        for root in 0..self.documents.len() {
            if completed[root].is_some() {
                continue;
            }
            let mut stack = vec![(StateDocumentId(root), false)];
            while let Some((id, visited)) = stack.pop() {
                if completed[id.0].is_some() {
                    continue;
                }
                if !visited {
                    stack.push((id, true));
                    for statement in &self.documents[id.0] {
                        if let StateStatement::State(state) = statement
                            && let Some(child) = state.doc
                        {
                            stack.push((child, false));
                        }
                    }
                    continue;
                }
                let mut result = DocumentComplexity {
                    items: self.documents[id.0].len(),
                    text_bytes: 0,
                    depth: 1,
                };
                for statement in &self.documents[id.0] {
                    let own = ModelComplexity::from_serializable(&StatementFields {
                        statement,
                        canonical: false,
                    });
                    // The serializer reports depth relative to its root container.
                    let depth = usize::from(matches!(
                        statement,
                        StateStatement::State(_)
                            | StateStatement::Relation(_)
                            | StateStatement::ClassDef { .. }
                            | StateStatement::ApplyClass { .. }
                            | StateStatement::Style { .. }
                            | StateStatement::Direction(_)
                            | StateStatement::Click(_)
                    ));
                    result.items = result
                        .items
                        .saturating_add(own.items.saturating_sub(usize::from(depth == 0)));
                    result.text_bytes = result.text_bytes.saturating_add(own.text_bytes);
                    result.depth = result.depth.max(
                        1usize
                            .saturating_add(own.nesting_depth)
                            .saturating_add(depth),
                    );
                    if let StateStatement::State(state) = statement
                        && let Some(child) = state.doc
                    {
                        let child: DocumentComplexity =
                            completed[child.0].expect("child document completed first");
                        result.items = result.items.saturating_add(child.items);
                        result.text_bytes = result.text_bytes.saturating_add(child.text_bytes);
                        result.depth = result.depth.max(2usize.saturating_add(child.depth));
                    }
                }
                completed[id.0] = Some(result);
            }
        }
        completed
            .into_iter()
            .map(|value| value.expect("all documents completed"))
            .collect()
    }
}
