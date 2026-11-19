use super::{allocation_failed, push, reserve};
use crate::Result;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_model::families::zenuml::{
    ZenumlDiagramModel, ZenumlFragmentKind, ZenumlFragmentSection, ZenumlMessageStyle,
    ZenumlStatement, ZenumlStatementKind,
};
use std::collections::{HashMap, HashSet};
use std::slice::Iter;

#[derive(Clone, Copy, Debug)]
pub(super) enum ArrowKind {
    Sync,
    Async,
    Return,
    Create,
}

impl ArrowKind {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Sync => "call",
            Self::Async => "async",
            Self::Return => "return",
            Self::Create => "new",
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum EventKind<'a> {
    Arrow {
        statement: &'a ZenumlStatement,
        from: usize,
        to: usize,
        kind: ArrowKind,
        call: bool,
    },
    Open {
        statement: &'a ZenumlStatement,
        depth: usize,
    },
    Section {
        section: &'a ZenumlFragmentSection,
        index: usize,
        depth: usize,
    },
    Close {
        depth: usize,
    },
    EndCall {
        target: usize,
    },
    AssignmentReturn {
        from: usize,
        to: usize,
        value: &'a str,
    },
    ParallelItem {
        index: usize,
        depth: usize,
    },
    Reference {
        statement: &'a ZenumlStatement,
        left: usize,
        right: usize,
    },
    Divider {
        statement: &'a ZenumlStatement,
    },
}

#[derive(Clone, Copy)]
pub(super) struct Event<'a> {
    pub(super) key: usize,
    pub(super) kind: EventKind<'a>,
}

pub(super) struct Group {
    pub(super) index: usize,
    pub(super) left: usize,
    pub(super) right: usize,
}

pub(super) struct Plan<'a> {
    pub(super) events: Vec<Event<'a>>,
    pub(super) groups: Vec<Group>,
    pub(super) created_at: Vec<Option<usize>>,
    pub(super) frame_depth: usize,
}

enum Task<'a> {
    Statements {
        iter: Iter<'a, ZenumlStatement>,
        depth: usize,
        nesting: usize,
        origin: Option<usize>,
        return_default: Option<usize>,
        parallel: Option<(usize, usize)>,
        index: usize,
    },
    Sections {
        iter: Iter<'a, ZenumlFragmentSection>,
        key: usize,
        index: usize,
        depth: usize,
        nesting: usize,
        origin: Option<usize>,
        return_default: Option<usize>,
        parallel: bool,
    },
    Event(Event<'a>),
}

pub(super) fn prepare<'a>(
    model: &'a ZenumlDiagramModel,
    resources: &ResourceContext,
) -> Result<Option<Plan<'a>>> {
    resources.checkpoint()?;
    let count = model.participants.len();
    resources.charge_layout_work(count)?;
    if count == 0 {
        return Ok(None);
    }
    let mut names = HashMap::new();
    names.try_reserve(count).map_err(|_| allocation_failed())?;
    let mut starter = None;
    for (index, participant) in model.participants.iter().enumerate() {
        resources.charge_layout_work(participant.name.len().max(1))?;
        if participant.name.is_empty() || names.insert(participant.name.as_str(), index).is_some() {
            return Ok(None);
        }
        if participant.is_starter && starter.replace(index).is_some() {
            return Ok(None);
        }
    }
    if let Some(name) = &model.starter {
        let Some(index) = lookup(&names, name, resources)? else {
            return Ok(None);
        };
        if starter.is_some_and(|starter| starter != index) {
            return Ok(None);
        }
        starter = Some(index);
    }
    let Some(groups) = groups(model, &names, resources)? else {
        return Ok(None);
    };
    let mut created_at = Vec::new();
    let mut first_use = Vec::new();
    reserve(&mut created_at, count)?;
    reserve(&mut first_use, count)?;
    created_at.resize(count, None);
    first_use.resize(count, None);
    let mut events = Vec::new();
    let mut pending = Vec::new();
    push(
        &mut pending,
        Task::Statements {
            iter: model.statements.iter(),
            depth: 0,
            nesting: 0,
            origin: model
                .starter
                .as_deref()
                .and_then(|name| names.get(name).copied()),
            return_default: if model.starter.is_none() {
                starter
            } else {
                None
            },
            parallel: None,
            index: 1,
        },
    )?;
    let mut key = 0usize;
    let mut frame_depth = 0usize;
    while let Some(task) = pending.pop() {
        resources.charge_layout_work(1)?;
        match task {
            Task::Event(event) => push(&mut events, event)?,
            Task::Sections {
                mut iter,
                key,
                index,
                depth,
                nesting,
                origin,
                return_default,
                parallel,
            } => {
                if let Some(section) = iter.next() {
                    push(
                        &mut events,
                        Event {
                            key,
                            kind: EventKind::Section {
                                section,
                                index,
                                depth,
                            },
                        },
                    )?;
                    push(
                        &mut pending,
                        Task::Sections {
                            iter,
                            key,
                            index: resources.checked_work_add(index, 1)?,
                            depth,
                            nesting,
                            origin,
                            return_default,
                            parallel,
                        },
                    )?;
                    push(
                        &mut pending,
                        Task::Statements {
                            iter: section.statements.iter(),
                            depth,
                            nesting,
                            origin,
                            return_default,
                            parallel: parallel.then_some((key, depth)),
                            index: 1,
                        },
                    )?;
                }
            }
            Task::Statements {
                mut iter,
                depth,
                nesting,
                origin,
                return_default,
                parallel,
                index,
            } => {
                resources.check_nesting_depth(nesting)?;
                let Some(statement) = iter.next() else {
                    continue;
                };
                if let Some((key, depth)) = parallel {
                    push(
                        &mut events,
                        Event {
                            key,
                            kind: EventKind::ParallelItem { index, depth },
                        },
                    )?;
                }
                key = resources.checked_work_add(key, 1)?;
                push(
                    &mut pending,
                    Task::Statements {
                        iter,
                        depth,
                        nesting,
                        origin,
                        return_default,
                        parallel,
                        index: resources.checked_work_add(index, 1)?,
                    },
                )?;
                let kind = match &statement.kind {
                    ZenumlStatementKind::Message {
                        explicit_from,
                        resolved_from,
                        resolved_to,
                        style,
                        body,
                        assignment,
                        ..
                    } => {
                        let default = if origin.is_none() && explicit_from.is_none() {
                            starter
                        } else {
                            None
                        };
                        let Some(from) = endpoint(&names, resolved_from, default, resources)?
                        else {
                            return Ok(None);
                        };
                        let Some(to) = endpoint(&names, resolved_to, None, resources)? else {
                            return Ok(None);
                        };
                        if !consistent_explicit(explicit_from, from, &names, resources)? {
                            return Ok(None);
                        }
                        first_use[from].get_or_insert(key);
                        first_use[to].get_or_insert(key);
                        if *style == ZenumlMessageStyle::Asynchronous
                            && (!body.is_empty() || assignment.is_some())
                        {
                            return Ok(None);
                        }
                        if !body.is_empty() {
                            push(
                                &mut pending,
                                Task::Event(Event {
                                    key,
                                    kind: EventKind::EndCall { target: to },
                                }),
                            )?;
                        }
                        if let Some(value) = assignment.as_deref().filter(|_| from != to) {
                            push(
                                &mut pending,
                                Task::Event(Event {
                                    key,
                                    kind: EventKind::AssignmentReturn {
                                        from: to,
                                        to: from,
                                        value,
                                    },
                                }),
                            )?;
                        }
                        push(
                            &mut pending,
                            Task::Statements {
                                iter: body.iter(),
                                depth,
                                nesting: resources.checked_work_add(nesting, 1)?,
                                origin: Some(to),
                                return_default: resolved_from.is_none().then_some(from),
                                parallel: None,
                                index: 1,
                            },
                        )?;
                        EventKind::Arrow {
                            statement,
                            from,
                            to,
                            kind: match style {
                                ZenumlMessageStyle::Synchronous => ArrowKind::Sync,
                                ZenumlMessageStyle::Asynchronous => ArrowKind::Async,
                            },
                            call: !body.is_empty(),
                        }
                    }
                    ZenumlStatementKind::Creation {
                        resolved_from,
                        resolved_to,
                        body,
                        assignment,
                        ..
                    } => {
                        let default = if origin.is_none() { starter } else { None };
                        let Some(from) = endpoint(&names, resolved_from, default, resources)?
                        else {
                            return Ok(None);
                        };
                        let Some(to) = lookup(&names, resolved_to, resources)? else {
                            return Ok(None);
                        };
                        if from == to
                            || depth != 0
                            || created_at[to].replace(key).is_some()
                            || first_use[to].is_some()
                        {
                            return Ok(None);
                        }
                        first_use[from].get_or_insert(key);
                        first_use[to] = Some(key);
                        if !body.is_empty() {
                            push(
                                &mut pending,
                                Task::Event(Event {
                                    key,
                                    kind: EventKind::EndCall { target: to },
                                }),
                            )?;
                        }
                        if let Some(value) = assignment.as_deref() {
                            push(
                                &mut pending,
                                Task::Event(Event {
                                    key,
                                    kind: EventKind::AssignmentReturn {
                                        from: to,
                                        to: from,
                                        value,
                                    },
                                }),
                            )?;
                        }
                        push(
                            &mut pending,
                            Task::Statements {
                                iter: body.iter(),
                                depth,
                                nesting: resources.checked_work_add(nesting, 1)?,
                                origin: Some(to),
                                return_default: resolved_from.is_none().then_some(from),
                                parallel: None,
                                index: 1,
                            },
                        )?;
                        EventKind::Arrow {
                            statement,
                            from,
                            to,
                            kind: ArrowKind::Create,
                            call: !body.is_empty(),
                        }
                    }
                    ZenumlStatementKind::Return {
                        explicit_from,
                        resolved_from,
                        explicit_to,
                        resolved_to,
                        ..
                    } => {
                        let default = if origin.is_none() && explicit_from.is_none() {
                            starter
                        } else {
                            None
                        };
                        let Some(from) = endpoint(&names, resolved_from, default, resources)?
                        else {
                            return Ok(None);
                        };
                        let default = if explicit_to.is_none() {
                            return_default
                        } else {
                            None
                        };
                        let Some(to) = endpoint(&names, resolved_to, default, resources)? else {
                            return Ok(None);
                        };
                        if !consistent_explicit(explicit_from, from, &names, resources)?
                            || !consistent_explicit(explicit_to, to, &names, resources)?
                        {
                            return Ok(None);
                        }
                        first_use[from].get_or_insert(key);
                        first_use[to].get_or_insert(key);
                        EventKind::Arrow {
                            statement,
                            from,
                            to,
                            kind: ArrowKind::Return,
                            call: false,
                        }
                    }
                    ZenumlStatementKind::Fragment {
                        sections,
                        fragment_kind,
                        ..
                    } => {
                        let inner = resources.checked_work_add(depth, 1)?;
                        frame_depth = frame_depth.max(inner);
                        push(
                            &mut pending,
                            Task::Event(Event {
                                key,
                                kind: EventKind::Close { depth },
                            }),
                        )?;
                        push(
                            &mut pending,
                            Task::Sections {
                                iter: sections.iter(),
                                key,
                                index: 1,
                                depth: inner,
                                nesting: resources.checked_work_add(nesting, 1)?,
                                origin,
                                return_default,
                                parallel: *fragment_kind == ZenumlFragmentKind::Parallel,
                            },
                        )?;
                        EventKind::Open { statement, depth }
                    }
                    ZenumlStatementKind::Reference { participants, .. } => {
                        let mut left = count - 1;
                        let mut right = 0;
                        if let Some(origin) = origin {
                            left = origin;
                            right = origin;
                            first_use[origin].get_or_insert(key);
                        }
                        for name in participants {
                            let Some(index) = lookup(&names, name, resources)? else {
                                return Ok(None);
                            };
                            left = left.min(index);
                            right = right.max(index);
                            first_use[index].get_or_insert(key);
                        }
                        if participants.is_empty() && origin.is_none() {
                            return Ok(None);
                        }
                        EventKind::Reference {
                            statement,
                            left,
                            right,
                        }
                    }
                    ZenumlStatementKind::Divider { .. } => EventKind::Divider { statement },
                };
                push(&mut events, Event { key, kind })?;
            }
        }
    }
    resources.check(AsciiResourceLimitId::MaxOutputBytes, events.len())?;
    Ok(Some(Plan {
        events,
        groups,
        created_at,
        frame_depth,
    }))
}

fn endpoint(
    names: &HashMap<&str, usize>,
    name: &Option<String>,
    default: Option<usize>,
    resources: &ResourceContext,
) -> Result<Option<usize>> {
    match name {
        Some(name) => lookup(names, name, resources),
        None => Ok(default),
    }
}

fn lookup(
    names: &HashMap<&str, usize>,
    name: &str,
    resources: &ResourceContext,
) -> Result<Option<usize>> {
    resources.charge_layout_work(name.len().max(1))?;
    Ok(names.get(name).copied())
}

fn consistent_explicit(
    name: &Option<String>,
    resolved: usize,
    names: &HashMap<&str, usize>,
    resources: &ResourceContext,
) -> Result<bool> {
    Ok(match name {
        Some(name) => lookup(names, name, resources)? == Some(resolved),
        None => true,
    })
}

fn groups(
    model: &ZenumlDiagramModel,
    names: &HashMap<&str, usize>,
    resources: &ResourceContext,
) -> Result<Option<Vec<Group>>> {
    let mut ids = HashSet::new();
    ids.try_reserve(model.groups.len())
        .map_err(|_| allocation_failed())?;
    let mut membership = Vec::new();
    reserve(&mut membership, model.participants.len())?;
    membership.resize(model.participants.len(), None);
    let mut groups = Vec::new();
    reserve(&mut groups, model.groups.len())?;
    for (index, group) in model.groups.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if let Some(id) = &group.id {
            resources.charge_layout_work(id.len().max(1))?;
            if !ids.insert(id.as_str()) {
                return Ok(None);
            }
        }
        let mut left = model.participants.len();
        let mut right = 0;
        for name in &group.participant_names {
            let Some(participant) = lookup(names, name, resources)? else {
                return Ok(None);
            };
            if membership[participant].replace(index).is_some() {
                return Ok(None);
            }
            left = left.min(participant);
            right = right.max(participant);
        }
        if left > right || right - left + 1 != group.participant_names.len() {
            return Ok(None);
        }
        push(&mut groups, Group { index, left, right })?;
    }
    for (index, participant) in model.participants.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if participant.group_id.as_deref()
            != membership[index].and_then(|group| model.groups[group].id.as_deref())
        {
            return Ok(None);
        }
    }
    Ok(Some(groups))
}
