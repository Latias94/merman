use super::SequenceCheckpointCursor;
use super::model::{SequenceControlKind, SequenceEvent};
use crate::color::AsciiRgb;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use merman_core::OperationPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceItemId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceItemParent {
    Root,
    Section {
        control: SequenceItemId,
        section: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct SequenceBody {
    items: Vec<SequenceItem>,
    roots: Vec<SequenceItemId>,
    max_depth: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SequenceItem {
    Event(SequenceEvent),
    Control(SequenceControl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceControl {
    model_index: usize,
    end_model_index: usize,
    pub(super) kind: SequenceControlKind,
    pub(super) label: String,
    pub(super) background: Option<AsciiRgb>,
    pub(super) participant_span: Option<SequenceParticipantSpan>,
    pub(super) sections: Vec<SequenceSection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceSection {
    pub(super) separator: Option<SequenceControlSeparator>,
    children: Vec<SequenceItemId>,
    participant_span: Option<SequenceParticipantSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceControlSeparator {
    model_index: usize,
    pub(super) label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SequenceParticipantSpan {
    pub(super) first: usize,
    pub(super) last: usize,
}

impl SequenceParticipantSpan {
    pub(super) fn from_event(event: &SequenceEvent) -> Self {
        match event {
            SequenceEvent::Message(message) => Self::between(message.from, message.to),
            SequenceEvent::Note(note) => Self::between(note.from, note.to),
            SequenceEvent::ActivationStart { actor, .. }
            | SequenceEvent::ActivationEnd { actor, .. } => Self::single(*actor),
        }
    }

    pub(super) fn include(&mut self, other: Self) {
        self.first = self.first.min(other.first);
        self.last = self.last.max(other.last);
    }

    pub(super) fn all(participant_count: usize) -> Result<Self> {
        let last = participant_count
            .checked_sub(1)
            .ok_or_else(invalid_control_tree)?;
        Ok(Self { first: 0, last })
    }

    pub(super) fn single(actor: usize) -> Self {
        Self {
            first: actor,
            last: actor,
        }
    }

    fn between(first: usize, second: usize) -> Self {
        Self {
            first: first.min(second),
            last: first.max(second),
        }
    }

    #[cfg(test)]
    pub(super) fn contains(self, actor: usize) -> bool {
        (self.first..=self.last).contains(&actor)
    }
}

impl SequenceBody {
    fn item(&self, id: SequenceItemId) -> Result<&SequenceItem> {
        self.items.get(id.0).ok_or_else(invalid_control_tree)
    }

    fn control(&self, id: SequenceItemId) -> Result<&SequenceControl> {
        match self.item(id)? {
            SequenceItem::Control(control) => Ok(control),
            SequenceItem::Event(_) => Err(invalid_control_tree()),
        }
    }

    pub(super) fn try_for_each_event(
        &self,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
        mut visitor: impl FnMut(&SequenceEvent, &mut SequenceCheckpointCursor<'_>) -> Result<()>,
    ) -> Result<()> {
        for item in &self.items {
            checkpoints.tick()?;
            if let SequenceItem::Event(event) = item {
                visitor(event, checkpoints)?;
            }
        }
        Ok(())
    }

    pub(super) fn try_visit<'body>(
        &'body self,
        resources: &mut ResourceContext,
        execution: AsciiExecution<'_>,
        mut visitor: impl FnMut(SequenceVisit<'body>, &mut ResourceContext) -> Result<()>,
    ) -> Result<()> {
        let capacity = self
            .max_depth
            .checked_add(1)
            .ok_or_else(|| resources.work_overflow())?;
        execution.checkpoint(OperationPhase::Layout)?;
        resources.charge_layout_work(capacity)?;
        let mut stack = Vec::new();
        stack
            .try_reserve_exact(capacity)
            .map_err(|_| allocation_failed())?;
        stack.push(SequenceVisitCursor::Root { next: 0 });

        while !stack.is_empty() {
            execution.checkpoint(OperationPhase::Layout)?;
            resources.charge_layout_work(1)?;
            let action = next_visit_action(self, &mut stack)?;
            match action {
                SequenceVisitAction::Visit(visit) => visitor(visit, resources)?,
                SequenceVisitAction::VisitAndPop(visit) => {
                    stack.pop();
                    visitor(visit, resources)?;
                }
                SequenceVisitAction::PushControl { item, depth } => {
                    visitor(
                        SequenceVisit::EnterControl {
                            control: self.control(item)?,
                            depth,
                        },
                        resources,
                    )?;
                    stack.try_reserve(1).map_err(|_| allocation_failed())?;
                    stack.push(SequenceVisitCursor::Control {
                        item,
                        section: 0,
                        child: 0,
                        section_entered: false,
                        depth,
                    });
                }
                SequenceVisitAction::Pop => {
                    stack.pop();
                }
                SequenceVisitAction::Continue => {}
                SequenceVisitAction::Done => break,
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) enum SequenceVisit<'a> {
    Event(&'a SequenceEvent),
    EnterControl {
        control: &'a SequenceControl,
        depth: usize,
    },
    EnterSection {
        control: &'a SequenceControl,
        section_index: usize,
    },
    ExitControl,
}

#[derive(Debug, Clone, Copy)]
enum SequenceVisitCursor {
    Root {
        next: usize,
    },
    Control {
        item: SequenceItemId,
        section: usize,
        child: usize,
        section_entered: bool,
        depth: usize,
    },
}

enum SequenceVisitAction<'a> {
    Visit(SequenceVisit<'a>),
    VisitAndPop(SequenceVisit<'a>),
    PushControl { item: SequenceItemId, depth: usize },
    Pop,
    Continue,
    Done,
}

fn next_visit_action<'a>(
    body: &'a SequenceBody,
    stack: &mut [SequenceVisitCursor],
) -> Result<SequenceVisitAction<'a>> {
    let Some(cursor) = stack.last_mut() else {
        return Ok(SequenceVisitAction::Done);
    };
    match cursor {
        SequenceVisitCursor::Root { next } => {
            let Some(item) = body.roots.get(*next).copied() else {
                return Ok(SequenceVisitAction::Pop);
            };
            *next += 1;
            match body.item(item)? {
                SequenceItem::Event(event) => {
                    Ok(SequenceVisitAction::Visit(SequenceVisit::Event(event)))
                }
                SequenceItem::Control(_) => Ok(SequenceVisitAction::PushControl { item, depth: 1 }),
            }
        }
        SequenceVisitCursor::Control {
            item,
            section,
            child,
            section_entered,
            depth,
        } => {
            let control = body.control(*item)?;
            let Some(section_plan) = control.sections.get(*section) else {
                return Ok(SequenceVisitAction::VisitAndPop(SequenceVisit::ExitControl));
            };
            if !*section_entered {
                *section_entered = true;
                return Ok(SequenceVisitAction::Visit(SequenceVisit::EnterSection {
                    control,
                    section_index: *section,
                }));
            }
            if let Some(child_item) = section_plan.children.get(*child).copied() {
                *child += 1;
                return match body.item(child_item)? {
                    SequenceItem::Event(event) => {
                        Ok(SequenceVisitAction::Visit(SequenceVisit::Event(event)))
                    }
                    SequenceItem::Control(_) => {
                        let child_depth = depth.checked_add(1).ok_or_else(invalid_control_tree)?;
                        Ok(SequenceVisitAction::PushControl {
                            item: child_item,
                            depth: child_depth,
                        })
                    }
                };
            }
            *section += 1;
            *child = 0;
            *section_entered = false;
            Ok(SequenceVisitAction::Continue)
        }
    }
}

pub(super) struct SequenceTreeBuilder {
    body: SequenceBody,
    stack: Vec<OpenControl>,
}

#[derive(Debug, Clone, Copy)]
struct OpenControl {
    item: SequenceItemId,
    current_section: usize,
}

impl SequenceTreeBuilder {
    pub(super) fn new(
        expected_items: usize,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        Self::new_with_probe(expected_items, resources, execution, || {})
    }

    fn new_with_probe(
        expected_items: usize,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
        before_allocate: impl FnOnce(),
    ) -> Result<Self> {
        resources.transaction(|resources| {
            execution.checkpoint(OperationPhase::Semantic)?;
            resources.charge_layout_work_product(expected_items, 2)?;
            before_allocate();
            let mut body = SequenceBody::default();
            body.items
                .try_reserve_exact(expected_items)
                .map_err(|_| allocation_failed())?;
            body.roots
                .try_reserve_exact(expected_items)
                .map_err(|_| allocation_failed())?;
            Ok(Self {
                body,
                stack: Vec::new(),
            })
        })
    }

    pub(super) fn push_event(
        &mut self,
        event: SequenceEvent,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<()> {
        resources.transaction(|resources| {
            execution.checkpoint(OperationPhase::Semantic)?;
            resources.charge_layout_work(1)?;
            let parent = self.current_parent();
            self.reserve_item_attachment(parent)?;
            let span = SequenceParticipantSpan::from_event(&event);
            let item = SequenceItemId(self.body.items.len());
            self.body.items.push(SequenceItem::Event(event));
            self.attach_item(parent, item)?;
            self.include_current_section_span(span)
        })
    }

    pub(super) fn start_control(
        &mut self,
        model_index: usize,
        kind: SequenceControlKind,
        label: String,
        background: Option<AsciiRgb>,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<()> {
        resources.transaction(|resources| {
            let depth = self
                .stack
                .len()
                .checked_add(1)
                .ok_or_else(|| resources.nesting_overflow())?;
            resources.check_nesting_depth(depth)?;
            execution.checkpoint(OperationPhase::Semantic)?;
            resources.charge_layout_work(1)?;
            let parent = self.current_parent();
            self.reserve_item_attachment(parent)?;
            self.stack.try_reserve(1).map_err(|_| allocation_failed())?;
            let mut sections = Vec::new();
            sections
                .try_reserve_exact(1)
                .map_err(|_| allocation_failed())?;
            sections.push(SequenceSection {
                separator: None,
                children: Vec::new(),
                participant_span: None,
            });
            let item = SequenceItemId(self.body.items.len());
            self.body.items.push(SequenceItem::Control(SequenceControl {
                model_index,
                end_model_index: model_index,
                kind,
                label,
                background,
                participant_span: None,
                sections,
            }));
            self.attach_item(parent, item)?;
            self.stack.push(OpenControl {
                item,
                current_section: 0,
            });
            self.body.max_depth = self.body.max_depth.max(depth);
            Ok(())
        })
    }

    pub(super) fn start_section(
        &mut self,
        model_index: usize,
        kind: SequenceControlKind,
        label: String,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<()> {
        resources.transaction(|resources| {
            execution.checkpoint(OperationPhase::Semantic)?;
            resources.charge_layout_work(1)?;
            let open = self
                .stack
                .last()
                .copied()
                .ok_or_else(invalid_control_ordering)?;
            let control = self.body.control(open.item)?;
            if control.kind != kind || kind.separator_keyword().is_none() {
                return Err(invalid_control_ordering());
            }
            let next_section = control.sections.len();
            let control = self.control_mut(open.item)?;
            control
                .sections
                .try_reserve(1)
                .map_err(|_| allocation_failed())?;
            control.sections.push(SequenceSection {
                separator: Some(SequenceControlSeparator { model_index, label }),
                children: Vec::new(),
                participant_span: None,
            });
            self.stack
                .last_mut()
                .ok_or_else(invalid_control_ordering)?
                .current_section = next_section;
            Ok(())
        })
    }

    pub(super) fn end_control(
        &mut self,
        model_index: usize,
        kind: SequenceControlKind,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<()> {
        resources.transaction(|resources| {
            execution.checkpoint(OperationPhase::Semantic)?;
            resources.charge_layout_work(1)?;
            let open = self
                .stack
                .last()
                .copied()
                .ok_or_else(invalid_control_ordering)?;
            let participant_span = {
                let control = self.body.control(open.item)?;
                if !control.kind.accepts_end(kind) {
                    return Err(invalid_control_ordering());
                }
                execution.checkpoint(OperationPhase::Semantic)?;
                resources.charge_layout_work(control.sections.len().max(1))?;
                let mut participant_span: Option<SequenceParticipantSpan> = None;
                for section in &control.sections {
                    execution.checkpoint(OperationPhase::Semantic)?;
                    let Some(span) = section.participant_span else {
                        continue;
                    };
                    if let Some(combined) = participant_span.as_mut() {
                        combined.include(span);
                    } else {
                        participant_span = Some(span);
                    }
                }
                participant_span
            };
            let control = self.control_mut(open.item)?;
            control.end_model_index = model_index;
            control.participant_span = participant_span;
            self.stack.pop();
            if let Some(span) = participant_span {
                self.include_current_section_span(span)?;
            }
            Ok(())
        })
    }

    pub(super) fn finish(self) -> Result<SequenceBody> {
        if !self.stack.is_empty() {
            return Err(invalid_control_ordering());
        }
        Ok(self.body)
    }

    fn current_parent(&self) -> SequenceItemParent {
        self.stack.last().map_or(SequenceItemParent::Root, |open| {
            SequenceItemParent::Section {
                control: open.item,
                section: open.current_section,
            }
        })
    }

    fn reserve_item_attachment(&mut self, parent: SequenceItemParent) -> Result<()> {
        self.body
            .items
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        match parent {
            SequenceItemParent::Root => self
                .body
                .roots
                .try_reserve(1)
                .map_err(|_| allocation_failed()),
            SequenceItemParent::Section { control, section } => self
                .control_mut(control)?
                .sections
                .get_mut(section)
                .ok_or_else(invalid_control_tree)?
                .children
                .try_reserve(1)
                .map_err(|_| allocation_failed()),
        }
    }

    fn attach_item(&mut self, parent: SequenceItemParent, item: SequenceItemId) -> Result<()> {
        match parent {
            SequenceItemParent::Root => self.body.roots.push(item),
            SequenceItemParent::Section { control, section } => self
                .control_mut(control)?
                .sections
                .get_mut(section)
                .ok_or_else(invalid_control_tree)?
                .children
                .push(item),
        }
        Ok(())
    }

    fn include_current_section_span(&mut self, span: SequenceParticipantSpan) -> Result<()> {
        let Some(open) = self.stack.last().copied() else {
            return Ok(());
        };
        let section = self
            .control_mut(open.item)?
            .sections
            .get_mut(open.current_section)
            .ok_or_else(invalid_control_tree)?;
        match &mut section.participant_span {
            Some(current) => current.include(span),
            None => section.participant_span = Some(span),
        }
        Ok(())
    }

    fn control_mut(&mut self, id: SequenceItemId) -> Result<&mut SequenceControl> {
        match self.body.items.get_mut(id.0) {
            Some(SequenceItem::Control(control)) => Ok(control),
            Some(SequenceItem::Event(_)) | None => Err(invalid_control_tree()),
        }
    }
}

fn allocation_failed() -> AsciiError {
    super::projection_allocation_failed()
}

fn invalid_control_tree() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "control tree",
    }
}

fn invalid_control_ordering() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "control block ordering",
    }
}

#[cfg(test)]
mod tests;
