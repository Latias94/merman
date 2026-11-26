//! Entity-relationship syntax and model construction.

mod lexer;
mod parser;
mod style_replay;

use crate::scan::collect_lexical_events;
use indexmap::IndexMap;
use lexer::Lexer;
use merman_model::families::er::{
    ErAttribute, ErClassDef, ErDiagramModel, ErEntity, ErRelSpec, ErRelationship, ErSubgraph,
};
use merman_model::text::is_ecmascript_whitespace;
use merman_model::{OperationCancelled, OperationControl, OperationControlResult, SourceSpan};
use std::collections::HashSet;
use style_replay::{STYLE_REPLAY_LIMIT, StyleReplay, StyleReplayCost};

pub use lexer::{ErDirection, ErIdList, ErLexicalError, ErLexicalEvent, ErToken};
pub use parser::{ErExpectedTokens, ErSyntaxDiagnostic};

type ErParsedActions = Result<(ErActionArena, ErActionList), ErSyntaxDiagnostic>;

#[derive(Debug)]
enum ErFailure {
    Syntax(ErSyntaxDiagnostic),
    Cancelled(OperationCancelled),
}

impl From<OperationCancelled> for ErFailure {
    fn from(cancelled: OperationCancelled) -> Self {
        Self::Cancelled(cancelled)
    }
}

/// Lexed ER source. Token and diagnostic offsets refer to the supplied input.
pub struct ErSyntax {
    events: Vec<ErLexicalEvent>,
}

impl ErSyntax {
    /// Lexes the input once, keeping the events after any recoverable lexical error.
    pub fn lex(code: &str, control: &OperationControl) -> OperationControlResult<Self> {
        let mut lexer = Lexer::new(code, Some(control));
        let events = collect_lexical_events(&mut lexer, control)?;
        Ok(Self { events })
    }

    /// Borrows the original event stream for editor projection before model construction.
    #[cfg(feature = "editor-facts")]
    pub fn events(&self) -> &[ErLexicalEvent] {
        &self.events
    }

    /// Parses the events and replays the actions to construct the raw ER model.
    pub fn into_model(
        self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ErDiagramModel, ErSyntaxDiagnostic>> {
        let (arena, actions) = match self.into_actions(control)? {
            Ok(parsed) => parsed,
            Err(diagnostic) => return Ok(Err(diagnostic)),
        };
        let mut db = ErDb::new();
        match db.replay(arena, actions, control, STYLE_REPLAY_LIMIT) {
            Ok(()) => Ok(Ok(db.into_semantic_model())),
            Err(ErFailure::Syntax(diagnostic)) => Ok(Err(diagnostic)),
            Err(ErFailure::Cancelled(cancelled)) => Err(cancelled),
        }
    }

    fn into_actions(self, control: &OperationControl) -> OperationControlResult<ErParsedActions> {
        control.checkpoint()?;
        parser::parse(self.events.into_iter(), control)
    }
}

#[derive(Debug, Clone)]
enum Action {
    AddEntity {
        name: String,
        alias: Option<String>,
    },
    AddAttributes {
        entity: String,
        attributes: Vec<ErAttribute>,
    },
    AddRelationship {
        a: String,
        role: String,
        b: String,
        spec: ErRelSpec,
    },
    SetClass {
        entities: Vec<String>,
        classes: Vec<String>,
        span: SourceSpan,
    },
    AddClassDef {
        classes: Vec<String>,
        raw: String,
        span: SourceSpan,
    },
    AddCssStyles {
        entities: Vec<String>,
        raw: String,
        span: SourceSpan,
    },
    AddSubgraph {
        id: String,
        title: String,
        label_type: String,
        body: ErActionList,
    },
    SetDirection(String),
    SetAccTitle(String),
    SetAccDescr(String),
}

// Grammar symbols carry scalar links; the arena owns every action exactly once.
#[derive(Debug, Clone, Copy, Default)]
struct ErActionList {
    first: Option<usize>,
    last: Option<usize>,
}

#[derive(Debug)]
struct ErActionRecord {
    action: Option<Action>,
    next: Option<usize>,
}

#[derive(Debug, Default)]
struct ErActionArena {
    records: Vec<ErActionRecord>,
}

impl ErActionArena {
    fn push(&mut self, action: Action) -> ErActionList {
        let index = self.records.len();
        self.records.push(ErActionRecord {
            action: Some(action),
            next: None,
        });
        ErActionList {
            first: Some(index),
            last: Some(index),
        }
    }

    fn append_actions(
        &mut self,
        mut list: ErActionList,
        actions: impl IntoIterator<Item = Action>,
    ) -> ErActionList {
        for action in actions {
            let item = self.push(action);
            list = self.append(list, item);
        }
        list
    }

    fn append(&mut self, left: ErActionList, right: ErActionList) -> ErActionList {
        let (Some(last), Some(first)) = (left.last, right.first) else {
            return if left.first.is_some() { left } else { right };
        };
        self.records[last].next = Some(first);
        ErActionList {
            first: left.first,
            last: right.last,
        }
    }
}

struct ErSubgraphCompletion {
    id: String,
    title: String,
    label_type: String,
    members: Vec<String>,
    direction: Option<String>,
}

struct ErReplayFrame {
    next: Option<usize>,
    subgraph: Option<ErSubgraphCompletion>,
}

#[derive(Debug, Default)]
struct ErDb {
    entities: IndexMap<String, ErEntity>,
    relationships: Vec<ErRelationship>,
    classes: IndexMap<String, ErClassDef>,
    direction: String,
    entity_counter: usize,
    acc_title: Option<String>,
    acc_descr: Option<String>,
    subgraphs: Vec<ErSubgraph>,
    subgraph_lookup: IndexMap<String, usize>,
    completed_members: HashSet<String>,
    #[cfg(test)]
    membership_candidates: usize,
}

impl ErDb {
    fn new() -> Self {
        Self {
            direction: "TB".to_string(),
            ..Default::default()
        }
    }

    fn add_entity(&mut self, name: &str, alias: Option<&str>) {
        let Some(existing) = self.entities.get_mut(name) else {
            let id = format!("entity-{name}-{}", self.entity_counter);
            self.entity_counter += 1;
            self.entities.insert(
                name.to_string(),
                ErEntity {
                    id,
                    label: name.to_string(),
                    attributes: Vec::new(),
                    alias: alias.unwrap_or("").to_string(),
                    shape: "erBox".to_string(),
                    css_classes: "default".to_string(),
                    css_styles: Vec::new(),
                },
            );
            return;
        };

        if existing.alias.is_empty()
            && let Some(a) = alias
            && !a.is_empty()
        {
            existing.alias = a.to_string();
        }
    }

    fn add_attributes(&mut self, entity: &str, attributes: Vec<ErAttribute>) {
        self.add_entity(entity, None);
        let Some(e) = self.entities.get_mut(entity) else {
            return;
        };
        for a in attributes {
            e.attributes.push(a);
        }
    }

    fn add_relationship(&mut self, a: &str, role: &str, b: &str, spec: ErRelSpec) {
        let entity_a = if self.subgraph_lookup.contains_key(a) {
            a.to_string()
        } else {
            self.add_entity(a, None);
            let Some(entity) = self.entities.get(a) else {
                return;
            };
            entity.id.clone()
        };
        let entity_b = if self.subgraph_lookup.contains_key(b) {
            b.to_string()
        } else {
            self.add_entity(b, None);
            let Some(entity) = self.entities.get(b) else {
                return;
            };
            entity.id.clone()
        };
        self.relationships.push(ErRelationship {
            entity_a,
            role_a: role.to_string(),
            entity_b,
            rel_spec: spec,
        });
    }

    fn complete_subgraph(
        &mut self,
        completion: ErSubgraphCompletion,
        control: &OperationControl,
    ) -> OperationControlResult<String> {
        let ErSubgraphCompletion {
            id: raw_id,
            title,
            label_type,
            members,
            direction,
        } = completion;
        let mut seen = HashSet::new();
        let mut retained = Vec::new();
        for (index, member) in members.into_iter().enumerate() {
            #[cfg(test)]
            {
                self.membership_candidates += 1;
            }
            if index % 128 == 0 {
                control.checkpoint()?;
            }
            let trimmed = member.trim_matches(is_ecmascript_whitespace);
            if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
                continue;
            }
            // ER appends repeated IDs and keeps the union of all completed memberships.
            if self.completed_members.insert(member.clone()) {
                retained.push(member);
            }
        }

        let id = raw_id.trim_matches(is_ecmascript_whitespace).to_string();
        let subgraph = ErSubgraph {
            classes: Vec::new(),
            css_styles: Vec::new(),
            dir: direction,
            id: id.clone(),
            label_type: match label_type.as_str() {
                "markdown" | "string" | "text" => label_type,
                _ => "markdown".to_string(),
            },
            nodes: retained,
            title: title.trim_matches(is_ecmascript_whitespace).to_string(),
        };
        let index = self.subgraphs.len();
        self.subgraphs.push(subgraph);
        self.subgraph_lookup.insert(id.clone(), index);
        Ok(id)
    }

    fn replay(
        &mut self,
        mut arena: ErActionArena,
        actions: ErActionList,
        control: &OperationControl,
        style_limit: StyleReplayCost,
    ) -> Result<(), ErFailure> {
        let mut styles = StyleReplay::new(control, style_limit);
        let mut frames = vec![ErReplayFrame {
            next: actions.first,
            subgraph: None,
        }];
        let mut replayed = 0usize;
        while let Some(frame) = frames.last_mut() {
            if replayed.is_multiple_of(128) {
                control.checkpoint()?;
            }
            replayed = replayed.saturating_add(1);
            let Some(index) = frame.next else {
                let frame = frames.pop().expect("the current replay frame exists");
                if let Some(completion) = frame.subgraph {
                    let id = self.complete_subgraph(completion, control)?;
                    if let Some(parent) =
                        frames.last_mut().and_then(|frame| frame.subgraph.as_mut())
                    {
                        parent.members.push(id);
                    }
                }
                continue;
            };
            let record = &mut arena.records[index];
            frame.next = record.next;
            let action = record.action.take().expect("an action is replayed once");
            match action {
                Action::AddEntity { name, alias } => {
                    if let Some(subgraph) = &mut frame.subgraph {
                        subgraph.members.push(name.clone());
                    }
                    self.add_entity(&name, alias.as_deref());
                }
                Action::AddAttributes { entity, attributes } => {
                    if let Some(subgraph) = &mut frame.subgraph {
                        subgraph.members.push(entity.clone());
                    }
                    self.add_attributes(&entity, attributes);
                }
                Action::AddRelationship { a, role, b, spec } => {
                    if let Some(subgraph) = &mut frame.subgraph {
                        subgraph.members.push(a.clone());
                        subgraph.members.push(b.clone());
                    }
                    // Jison creates both entities before routing completed subgraph endpoints.
                    self.add_entity(&a, None);
                    self.add_entity(&b, None);
                    self.add_relationship(&a, &role, &b, spec);
                }
                Action::SetClass {
                    entities,
                    classes,
                    span,
                } => styles.set_class(self, &entities, &classes, span)?,
                Action::AddClassDef { classes, raw, span } => {
                    styles.add_class_def(self, classes, &raw, span)?
                }
                Action::AddCssStyles {
                    entities,
                    raw,
                    span,
                } => styles.add_css_styles(self, &entities, &raw, span)?,
                Action::AddSubgraph {
                    id,
                    title,
                    label_type,
                    body,
                } => frames.push(ErReplayFrame {
                    next: body.first,
                    subgraph: Some(ErSubgraphCompletion {
                        id,
                        title,
                        label_type,
                        members: Vec::new(),
                        direction: None,
                    }),
                }),
                Action::SetDirection(dir) => {
                    if let Some(subgraph) = &mut frame.subgraph {
                        subgraph.direction = Some(dir);
                    } else {
                        self.direction = dir;
                    }
                }
                Action::SetAccTitle(t) => {
                    self.acc_title = Some(t);
                }
                Action::SetAccDescr(t) => {
                    self.acc_descr = Some(t);
                }
            }
        }
        control.checkpoint()?;
        Ok(())
    }

    fn into_semantic_model(self) -> ErDiagramModel {
        ErDiagramModel {
            acc_title: self.acc_title,
            acc_descr: self.acc_descr,
            direction: self.direction,
            classes: self.classes,
            entities: self.entities,
            relationships: self.relationships,
            subgraphs: self.subgraphs,
        }
    }
}

#[cfg(test)]
mod tests;
