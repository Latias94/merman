//! Complete ZenUML reading records and a borrowed, nonrecursive semantic projection.
//!
//! `participants` and `groups` retain model order (P1/G1 onward). `statements` is a flat
//! depth-first preorder stream (S1 onward), not the model's recursive wire format. Each
//! statement retains its shallow authored fields, plus a one-based `ordinal`, nullable
//! parent statement ordinal, and nullable one-based `section` within a fragment parent.
//! Message/creation bodies become `bodyCount`, including zero. Fragment `sections` stay
//! shallow: label, optional bodyComment, span, and statementCount, including empty sections.
//! Scalar names, enum spellings, nulls, and optional field omission match canonical serde.
//! The canonical model serializer materializes JSON; serializing a statement kind recurses.
//! Both are avoided so the bounded writer observes each field before more work is admitted.

use super::record::{RecordWriter, details, write_plain_record};
use super::{AsciiDiagram, AsciiError, RawValue, Result, SemanticFallbackWriter};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_model::SourceSpan;
use merman_model::families::zenuml::{
    ZenumlDiagramModel, ZenumlFragmentKind, ZenumlFragmentSection, ZenumlGroup, ZenumlMessageStyle,
    ZenumlParticipant, ZenumlStatement, ZenumlStatementKind,
};
use merman_model::resources::ModelComplexity;
use serde::ser::{SerializeMap, SerializeSeq, SerializeStruct};
use serde::{Deserialize, Serialize, Serializer};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::slice;

type Fields<'a> = BTreeMap<String, &'a RawValue>;

pub(crate) struct Projection<'a>(pub &'a ZenumlDiagramModel);

impl Serialize for Projection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let resources = ResourceContext::new(crate::AsciiResourcePolicy::unbounded());
        ControlledProjection::new(self.0, &resources).serialize(serializer)
    }
}

/// A shallow projection whose traversal errors survive serde's erased error channel.
pub(crate) struct ControlledProjection<'model, 'resources> {
    model: &'model ZenumlDiagramModel,
    control: ProjectionControl<'resources>,
}

impl<'model, 'resources> ControlledProjection<'model, 'resources> {
    pub(crate) fn new(
        model: &'model ZenumlDiagramModel,
        resources: &'resources ResourceContext,
    ) -> Self {
        Self {
            model,
            control: ProjectionControl::new(resources),
        }
    }

    pub(crate) fn take_error(&self) -> Option<AsciiError> {
        self.control.error.borrow_mut().take()
    }
}

pub(crate) fn complexity(
    model: &ZenumlDiagramModel,
    resources: &ResourceContext,
) -> Result<ModelComplexity> {
    let projection = ControlledProjection::new(model, resources);
    // This serializer only counts the shallow projection. Nested statements are visited by
    // Traversal, never by the canonical model serializer or a recursive statement serializer.
    let mut complexity = ModelComplexity::from_serializable(&projection);
    if let Some(error) = projection.take_error() {
        return Err(error);
    }
    complexity.nesting_depth = complexity
        .nesting_depth
        .max(projection.control.nesting_depth.get());
    Ok(complexity)
}

impl Serialize for ControlledProjection<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let control = &self.control;
        control.capture(control.resources.charge_layout_work(1))?;
        control.capture(control.nesting(0))?;
        let mut root = serializer.serialize_struct("ZenumlProjection", 5)?;
        root.serialize_field("title", &control.value(&self.model.title, 1))?;
        root.serialize_field("starter", &control.value(&self.model.starter, 1))?;
        root.serialize_field("participants", &control.value(&self.model.participants, 1))?;
        root.serialize_field("groups", &control.value(&self.model.groups, 1))?;
        root.serialize_field(
            "statements",
            &Statements {
                models: &self.model.statements,
                control,
            },
        )?;
        root.end()
    }
}

struct ProjectionControl<'resources> {
    resources: &'resources ResourceContext,
    error: RefCell<Option<AsciiError>>,
    nesting_depth: Cell<usize>,
    #[cfg(test)]
    before_reserve: Cell<Option<&'resources dyn Fn() -> Result<()>>>,
}

impl<'resources> ProjectionControl<'resources> {
    fn new(resources: &'resources ResourceContext) -> Self {
        Self {
            resources,
            error: RefCell::new(None),
            nesting_depth: Cell::new(0),
            #[cfg(test)]
            before_reserve: Cell::new(None),
        }
    }

    fn capture<T, E: serde::ser::Error>(&self, result: Result<T>) -> std::result::Result<T, E> {
        result.map_err(|error| {
            self.error.borrow_mut().get_or_insert(error);
            E::custom("controlled ZenUML semantic projection rejected")
        })
    }

    fn nesting(&self, depth: usize) -> Result<()> {
        self.resources.check_nesting_depth(depth)?;
        self.nesting_depth.set(self.nesting_depth.get().max(depth));
        Ok(())
    }

    fn child_depth(&self, depth: usize) -> Result<usize> {
        depth
            .checked_add(1)
            .ok_or_else(|| self.resources.nesting_overflow())
    }

    fn value<'a, T: Project + ?Sized>(
        &'a self,
        value: &'a T,
        depth: usize,
    ) -> ControlledValue<'a, 'resources, T> {
        ControlledValue {
            value,
            control: self,
            depth,
        }
    }
}

// Only shallow values implement Project. In particular, ZenumlStatementKind and the
// canonical diagram model deliberately cannot be passed to this wrapper.
trait Project {
    fn serialize_controlled<S: Serializer>(
        &self,
        control: &ProjectionControl<'_>,
        depth: usize,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>;
}

struct ControlledValue<'a, 'resources, T: ?Sized> {
    value: &'a T,
    control: &'a ProjectionControl<'resources>,
    depth: usize,
}

impl<T: Project + ?Sized> Serialize for ControlledValue<'_, '_, T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.control
            .capture(self.control.resources.charge_layout_work(1))?;
        self.value
            .serialize_controlled(self.control, self.depth, serializer)
    }
}

macro_rules! project_scalars {
    ($($ty:ty),* $(,)?) => {$(
        impl Project for $ty {
            fn serialize_controlled<S: Serializer>(
                &self,
                _control: &ProjectionControl<'_>,
                _depth: usize,
                serializer: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                self.serialize(serializer)
            }
        }
    )*};
}

project_scalars!(bool, usize, ZenumlMessageStyle, ZenumlFragmentKind);

impl Project for str {
    fn serialize_controlled<S: Serializer>(
        &self,
        control: &ProjectionControl<'_>,
        _depth: usize,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        control.capture(control.resources.charge_layout_work(self.len()))?;
        serializer.serialize_str(self)
    }
}

impl Project for String {
    fn serialize_controlled<S: Serializer>(
        &self,
        control: &ProjectionControl<'_>,
        depth: usize,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        self.as_str()
            .serialize_controlled(control, depth, serializer)
    }
}

impl<T: Project> Project for Option<T> {
    fn serialize_controlled<S: Serializer>(
        &self,
        control: &ProjectionControl<'_>,
        depth: usize,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Some(value) => serializer.serialize_some(&control.value(value, depth)),
            None => serializer.serialize_none(),
        }
    }
}

impl<T: Project> Project for Vec<T> {
    fn serialize_controlled<S: Serializer>(
        &self,
        control: &ProjectionControl<'_>,
        depth: usize,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        control.capture(control.nesting(depth))?;
        let child_depth = control.capture(control.child_depth(depth))?;
        let mut sequence = serializer.serialize_seq(Some(self.len()))?;
        for value in self {
            sequence.serialize_element(&control.value(value, child_depth))?;
        }
        sequence.end()
    }
}

impl Project for SourceSpan {
    fn serialize_controlled<S: Serializer>(
        &self,
        control: &ProjectionControl<'_>,
        depth: usize,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        control.capture(control.nesting(depth))?;
        // The span has exactly two scalar fields; no input-sized work is hidden here.
        control.capture(control.resources.charge_layout_work(2))?;
        self.serialize(serializer)
    }
}

impl Project for ZenumlParticipant {
    fn serialize_controlled<S: Serializer>(
        &self,
        control: &ProjectionControl<'_>,
        depth: usize,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        control.capture(control.nesting(depth))?;
        let child_depth = control.capture(control.child_depth(depth))?;
        let fields =
            11 + usize::from(self.width_source.is_some()) + usize::from(self.comment.is_some());
        let mut participant = serializer.serialize_struct("ZenumlParticipant", fields)?;
        participant.serialize_field("name", &control.value(&self.name, child_depth))?;
        participant.serialize_field("label", &control.value(&self.label, child_depth))?;
        participant.serialize_field(
            "participantType",
            &control.value(&self.participant_type, child_depth),
        )?;
        participant.serialize_field("stereotype", &control.value(&self.stereotype, child_depth))?;
        participant.serialize_field("emoji", &control.value(&self.emoji, child_depth))?;
        if let Some(width) = &self.width_source {
            participant.serialize_field("widthSource", &control.value(width, child_depth))?;
        }
        participant.serialize_field("color", &control.value(&self.color, child_depth))?;
        if let Some(comment) = &self.comment {
            participant.serialize_field("comment", &control.value(comment, child_depth))?;
        }
        participant.serialize_field("groupId", &control.value(&self.group_id, child_depth))?;
        participant.serialize_field("explicit", &control.value(&self.explicit, child_depth))?;
        participant.serialize_field("isStarter", &control.value(&self.is_starter, child_depth))?;
        participant.serialize_field(
            "declarationSpan",
            &control.value(&self.declaration_span, child_depth),
        )?;
        participant.serialize_field(
            "occurrences",
            &control.value(&self.occurrences, child_depth),
        )?;
        participant.end()
    }
}

impl Project for ZenumlGroup {
    fn serialize_controlled<S: Serializer>(
        &self,
        control: &ProjectionControl<'_>,
        depth: usize,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        control.capture(control.nesting(depth))?;
        let child_depth = control.capture(control.child_depth(depth))?;
        let mut group = serializer.serialize_struct("ZenumlGroup", 3)?;
        group.serialize_field("id", &control.value(&self.id, child_depth))?;
        group.serialize_field(
            "participantNames",
            &control.value(&self.participant_names, child_depth),
        )?;
        group.serialize_field("span", &control.value(&self.span, child_depth))?;
        group.end()
    }
}

impl Project for ZenumlFragmentSection {
    fn serialize_controlled<S: Serializer>(
        &self,
        control: &ProjectionControl<'_>,
        depth: usize,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        control.capture(control.nesting(depth))?;
        let child_depth = control.capture(control.child_depth(depth))?;
        let fields = 3 + usize::from(self.body_comment.is_some());
        let mut section = serializer.serialize_struct("Section", fields)?;
        section.serialize_field("label", &control.value(&self.label, child_depth))?;
        if let Some(comment) = &self.body_comment {
            section.serialize_field("bodyComment", &control.value(comment, child_depth))?;
        }
        section.serialize_field("span", &control.value(&self.span, child_depth))?;
        section.serialize_field(
            "statementCount",
            &control.value(&self.statements.len(), child_depth),
        )?;
        section.end()
    }
}

struct Statements<'a, 'resources> {
    models: &'a [ZenumlStatement],
    control: &'a ProjectionControl<'resources>,
}

impl Serialize for Statements<'_, '_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        self.control
            .capture(self.control.resources.charge_layout_work(1))?;
        self.control.capture(self.control.nesting(1))?;
        let mut stream = serializer.serialize_seq(None)?;
        let mut traversal = self
            .control
            .capture(Traversal::new(self.models, self.control))?;
        while let Some(statement) = self.control.capture(traversal.next())? {
            stream.serialize_element(&self.control.value(&statement, 2))?;
        }
        stream.end()
    }
}

struct Statement<'a> {
    ordinal: usize,
    parent: Option<usize>,
    section: Option<usize>,
    model: &'a ZenumlStatement,
}

impl Project for Statement<'_> {
    fn serialize_controlled<S: Serializer>(
        &self,
        control: &ProjectionControl<'_>,
        depth: usize,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        control.capture(control.nesting(depth))?;
        let child_depth = control.capture(control.child_depth(depth))?;
        let mut record = serializer.serialize_map(None)?;
        record.serialize_entry("ordinal", &control.value(&self.ordinal, child_depth))?;
        record.serialize_entry("parent", &control.value(&self.parent, child_depth))?;
        record.serialize_entry("section", &control.value(&self.section, child_depth))?;
        record.serialize_entry("id", &control.value(&self.model.id, child_depth))?;
        record.serialize_entry("comment", &control.value(&self.model.comment, child_depth))?;
        record.serialize_entry("span", &control.value(&self.model.span, child_depth))?;
        record.serialize_entry(
            "kind",
            &control.value(kind_name(&self.model.kind), child_depth),
        )?;
        match &self.model.kind {
            ZenumlStatementKind::Message {
                explicit_from,
                resolved_from,
                resolved_to,
                label,
                assignment,
                style,
                body,
                body_comment,
            } => {
                record
                    .serialize_entry("explicit_from", &control.value(explicit_from, child_depth))?;
                record
                    .serialize_entry("resolved_from", &control.value(resolved_from, child_depth))?;
                record.serialize_entry("resolved_to", &control.value(resolved_to, child_depth))?;
                record.serialize_entry("label", &control.value(label, child_depth))?;
                record.serialize_entry("assignment", &control.value(assignment, child_depth))?;
                record.serialize_entry("style", &control.value(style, child_depth))?;
                record.serialize_entry("bodyCount", &control.value(&body.len(), child_depth))?;
                if let Some(comment) = body_comment {
                    record.serialize_entry("body_comment", &control.value(comment, child_depth))?;
                }
            }
            ZenumlStatementKind::Creation {
                resolved_from,
                resolved_to,
                constructor,
                parameters,
                assignment,
                label,
                body,
                body_comment,
            } => {
                record
                    .serialize_entry("resolved_from", &control.value(resolved_from, child_depth))?;
                record.serialize_entry("resolved_to", &control.value(resolved_to, child_depth))?;
                record.serialize_entry("constructor", &control.value(constructor, child_depth))?;
                record.serialize_entry("parameters", &control.value(parameters, child_depth))?;
                record.serialize_entry("assignment", &control.value(assignment, child_depth))?;
                record.serialize_entry("label", &control.value(label, child_depth))?;
                record.serialize_entry("bodyCount", &control.value(&body.len(), child_depth))?;
                if let Some(comment) = body_comment {
                    record.serialize_entry("body_comment", &control.value(comment, child_depth))?;
                }
            }
            ZenumlStatementKind::Return {
                explicit_from,
                resolved_from,
                explicit_to,
                resolved_to,
                label,
            } => {
                record
                    .serialize_entry("explicit_from", &control.value(explicit_from, child_depth))?;
                record
                    .serialize_entry("resolved_from", &control.value(resolved_from, child_depth))?;
                record.serialize_entry("explicit_to", &control.value(explicit_to, child_depth))?;
                record.serialize_entry("resolved_to", &control.value(resolved_to, child_depth))?;
                record.serialize_entry("label", &control.value(label, child_depth))?;
            }
            ZenumlStatementKind::Fragment {
                fragment_kind,
                label,
                sections,
            } => {
                record
                    .serialize_entry("fragment_kind", &control.value(fragment_kind, child_depth))?;
                record.serialize_entry("label", &control.value(label, child_depth))?;
                record.serialize_entry("sections", &control.value(sections, child_depth))?;
            }
            ZenumlStatementKind::Reference {
                participants,
                label,
            } => {
                record
                    .serialize_entry("participants", &control.value(participants, child_depth))?;
                record.serialize_entry("label", &control.value(label, child_depth))?;
            }
            ZenumlStatementKind::Divider { label } => {
                record.serialize_entry("label", &control.value(label, child_depth))?;
            }
        }
        record.end()
    }
}

enum Frame<'a> {
    Statements {
        remaining: slice::Iter<'a, ZenumlStatement>,
        parent: Option<usize>,
        section: Option<usize>,
        nesting: usize,
    },
    Sections {
        remaining: std::iter::Enumerate<slice::Iter<'a, ZenumlFragmentSection>>,
        parent: usize,
        nesting: usize,
    },
}

struct Traversal<'a, 'control, 'resources> {
    frames: Vec<Frame<'a>>,
    ordinal: usize,
    control: &'control ProjectionControl<'resources>,
}

impl<'a, 'control, 'resources> Traversal<'a, 'control, 'resources> {
    fn new(
        statements: &'a [ZenumlStatement],
        control: &'control ProjectionControl<'resources>,
    ) -> Result<Self> {
        let mut traversal = Self {
            frames: Vec::new(),
            ordinal: 0,
            control,
        };
        if !statements.is_empty() {
            traversal.push(Frame::Statements {
                remaining: statements.iter(),
                parent: None,
                section: None,
                nesting: 0,
            })?;
        }
        Ok(traversal)
    }

    fn push(&mut self, frame: Frame<'a>) -> Result<()> {
        self.control.resources.charge_layout_work(1)?;
        let nesting = match &frame {
            Frame::Statements { nesting, .. } | Frame::Sections { nesting, .. } => *nesting,
        };
        self.control.nesting(nesting)?;
        // Check admission before retaining another ancestor. Siblings reuse the same capacity.
        #[cfg(test)]
        if let Some(before_reserve) = self.control.before_reserve.get() {
            before_reserve()?;
        }
        self.frames
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        self.frames.push(frame);
        Ok(())
    }

    fn next(&mut self) -> Result<Option<Statement<'a>>> {
        loop {
            // Empty sections and exhausted ancestor frames must also observe the work ledger.
            self.control.resources.charge_layout_work(1)?;
            let Some(frame) = self.frames.last_mut() else {
                return Ok(None);
            };
            let (model, parent, section, nesting) = match frame {
                Frame::Statements {
                    remaining,
                    parent,
                    section,
                    nesting,
                } => {
                    let Some(model) = remaining.next() else {
                        self.frames.pop();
                        continue;
                    };
                    (model, *parent, *section, *nesting)
                }
                Frame::Sections {
                    remaining,
                    parent,
                    nesting,
                } => {
                    let Some((index, section)) = remaining.next() else {
                        self.frames.pop();
                        continue;
                    };
                    let parent = *parent;
                    let nesting = *nesting;
                    if !section.statements.is_empty() {
                        self.push(Frame::Statements {
                            remaining: section.statements.iter(),
                            parent: Some(parent),
                            section: Some(self.control.resources.checked_work_add(index, 1)?),
                            nesting,
                        })?;
                    }
                    continue;
                }
            };
            self.ordinal = self.control.resources.checked_work_add(self.ordinal, 1)?;
            let statement = Statement {
                ordinal: self.ordinal,
                parent,
                section,
                model,
            };
            match &model.kind {
                ZenumlStatementKind::Message { body, .. }
                | ZenumlStatementKind::Creation { body, .. } => {
                    let nesting = self.control.child_depth(nesting)?;
                    self.control.nesting(nesting)?;
                    if !body.is_empty() {
                        self.push(Frame::Statements {
                            remaining: body.iter(),
                            parent: Some(self.ordinal),
                            section: None,
                            nesting,
                        })?;
                    }
                }
                ZenumlStatementKind::Fragment { sections, .. } if !sections.is_empty() => {
                    let nesting = self.control.child_depth(nesting)?;
                    self.push(Frame::Sections {
                        remaining: sections.iter().enumerate(),
                        parent: self.ordinal,
                        nesting,
                    })?;
                }
                _ => {}
            }
            return Ok(Some(statement));
        }
    }
}

pub(super) fn write_if_zenuml(
    model: &AsciiDiagram<'_>,
    bytes: &[u8],
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<bool> {
    let AsciiDiagram::Zenuml(model) = model else {
        return Ok(false);
    };
    lines.admit_node(0)?;
    lines.semantic_resources.charge_layout_work(bytes.len())?;
    lines
        .semantic_resources
        .check(AsciiResourceLimitId::MaxOutputBytes, bytes.len())?;
    let mut root: Fields<'_> = serde_json::from_slice(bytes).map_err(|_| invalid_projection())?;
    let participants = take_array(
        &mut root,
        "participants",
        Some(model.participants.len()),
        lines,
    )?;
    let groups = take_array(&mut root, "groups", Some(model.groups.len()), lines)?;
    let statements = take_array(&mut root, "statements", None, lines)?;
    let identities = Identities::new(model, lines)?;

    let mut record = RecordWriter::new(lines)?;
    record.text("ZenUML title=", false)?;
    optional_text(model.title.as_deref(), &mut record)?;
    record.finish()?;
    root.remove("title");
    let mut record = RecordWriter::new(lines)?;
    record.text("ZenUML starter=", false)?;
    identities.optional_reference(
        model.starter.as_deref(),
        IdentityKind::Participant,
        &mut record,
    )?;
    record.finish()?;
    root.remove("starter");
    write_plain_record(
        &format!(
            "ZenUML participants={} groups={} rootStatements={} statements={}",
            participants.len(),
            groups.len(),
            model.statements.len(),
            statements.len(),
        ),
        lines,
    )?;
    write_plain_record(
        "Keys: P participants; G groups; S statements. P/G keep model order. S uses depth-first preorder, visiting fragment sections in order. parent=null is a root; section=null is a message/creation body or a root; numbered sections are one-based. References retain authored names/IDs and every matching key.",
        lines,
    )?;

    for (index, (participant, raw)) in model.participants.iter().zip(participants).enumerate() {
        let key = format!("P{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [participant] name="), false)?;
        record.text(&participant.name, true)?;
        record.text(" label=", false)?;
        optional_text(participant.label.as_deref(), &mut record)?;
        record.text(" participantType=", false)?;
        optional_text(participant.participant_type.as_deref(), &mut record)?;
        record.text(
            &format!(
                " explicit={} isStarter={} group=",
                participant.explicit, participant.is_starter,
            ),
            false,
        )?;
        identities.optional_reference(
            participant.group_id.as_deref(),
            IdentityKind::Group,
            &mut record,
        )?;
        record.finish()?;
        for name in ["name", "label", "participantType", "explicit", "isStarter"] {
            fields.remove(name);
        }
        details(&key, &fields, lines)?;
        identities.disclose_duplicate(
            &participant.name,
            IdentityKind::Participant,
            index,
            lines,
        )?;
    }
    for (index, (group, raw)) in model.groups.iter().zip(groups).enumerate() {
        let key = format!("G{}", index + 1);
        let mut fields: Fields<'_> = decode(raw, lines)?;
        let mut record = RecordWriter::new(lines)?;
        record.text(&format!("{key} [group] id="), false)?;
        optional_text(group.id.as_deref(), &mut record)?;
        record.text(" participants=", false)?;
        identities.references(&group.participant_names, &mut record)?;
        record.finish()?;
        fields.remove("id");
        details(&key, &fields, lines)?;
        if let Some(id) = &group.id {
            identities.disclose_duplicate(id, IdentityKind::Group, index, lines)?;
        }
    }
    let traversal_resources = lines.semantic_resources.clone();
    let traversal_control = ProjectionControl::new(&traversal_resources);
    let mut traversal = Traversal::new(&model.statements, &traversal_control)?;
    for raw in statements {
        let statement = traversal.next()?.ok_or_else(invalid_projection)?;
        write_statement(&statement, raw, &identities, lines)?;
    }
    if traversal.next()?.is_some() {
        return Err(invalid_projection());
    }
    details("zenuml", &root, lines)?;
    Ok(true)
}

fn write_statement(
    statement: &Statement<'_>,
    raw: &RawValue,
    identities: &Identities<'_>,
    lines: &mut SemanticFallbackWriter<'_>,
) -> Result<()> {
    let key = format!("S{}", statement.ordinal);
    let mut fields: Fields<'_> = decode(raw, lines)?;
    let mut record = RecordWriter::new(lines)?;
    record.text(
        &format!("{key} [{}] id=", kind_name(&statement.model.kind)),
        false,
    )?;
    record.text(&statement.model.id, true)?;
    record.text(" parent=", false)?;
    match statement.parent {
        Some(parent) => record.text(&format!("S{parent}"), false)?,
        None => record.text("null", false)?,
    }
    record.text(" section=", false)?;
    match statement.section {
        Some(section) => record.text(&section.to_string(), false)?,
        None => record.text("null", false)?,
    }
    for name in ["ordinal", "parent", "section", "kind", "id"] {
        fields.remove(name);
    }
    match &statement.model.kind {
        ZenumlStatementKind::Message {
            resolved_from,
            resolved_to,
            label,
            assignment,
            style,
            body,
            ..
        } => {
            write_endpoints(
                resolved_from.as_deref(),
                resolved_to.as_deref(),
                identities,
                &mut record,
            )?;
            record.text(" style=", false)?;
            record.text(
                match style {
                    ZenumlMessageStyle::Synchronous => "synchronous",
                    ZenumlMessageStyle::Asynchronous => "asynchronous",
                },
                false,
            )?;
            record.text(" assignment=", false)?;
            optional_text(assignment.as_deref(), &mut record)?;
            record.text(" label=", false)?;
            record.text(label, true)?;
            record.text(&format!(" bodyCount={}", body.len()), false)?;
            for name in [
                "resolved_from",
                "resolved_to",
                "style",
                "assignment",
                "label",
                "bodyCount",
            ] {
                fields.remove(name);
            }
        }
        ZenumlStatementKind::Creation {
            resolved_from,
            resolved_to,
            assignment,
            label,
            body,
            ..
        } => {
            write_endpoints(
                resolved_from.as_deref(),
                Some(resolved_to),
                identities,
                &mut record,
            )?;
            record.text(" assignment=", false)?;
            optional_text(assignment.as_deref(), &mut record)?;
            record.text(" label=", false)?;
            record.text(label, true)?;
            record.text(&format!(" bodyCount={}", body.len()), false)?;
            for name in [
                "resolved_from",
                "resolved_to",
                "assignment",
                "label",
                "bodyCount",
            ] {
                fields.remove(name);
            }
        }
        ZenumlStatementKind::Return {
            resolved_from,
            resolved_to,
            label,
            ..
        } => {
            write_endpoints(
                resolved_from.as_deref(),
                resolved_to.as_deref(),
                identities,
                &mut record,
            )?;
            record.text(" label=", false)?;
            record.text(label, true)?;
            for name in ["resolved_from", "resolved_to", "label"] {
                fields.remove(name);
            }
        }
        ZenumlStatementKind::Fragment {
            fragment_kind,
            label,
            sections,
        } => {
            record.text(" fragment=", false)?;
            record.text(fragment_kind.as_str(), false)?;
            record.text(" label=", false)?;
            optional_text(label.as_deref(), &mut record)?;
            record.text(&format!(" sections={}", sections.len()), false)?;
            fields.remove("label");
        }
        ZenumlStatementKind::Reference {
            participants,
            label,
        } => {
            record.text(" label=", false)?;
            record.text(label, true)?;
            record.text(" participants=", false)?;
            identities.references(participants, &mut record)?;
            fields.remove("label");
        }
        ZenumlStatementKind::Divider { label } => {
            record.text(" label=", false)?;
            record.text(label, true)?;
            fields.remove("label");
        }
    }
    record.finish()?;
    if let ZenumlStatementKind::Fragment { sections, .. } = &statement.model.kind {
        let raw_sections = take_array(&mut fields, "sections", Some(sections.len()), lines)?;
        for (index, (section, raw)) in sections.iter().zip(raw_sections).enumerate() {
            let scope = format!("{key} section {}", index + 1);
            let mut fields: Fields<'_> = decode(raw, lines)?;
            let mut record = RecordWriter::new(lines)?;
            record.text(&format!("{scope} [fragment section] label="), false)?;
            optional_text(section.label.as_deref(), &mut record)?;
            record.text(
                &format!(" statementCount={}", section.statements.len()),
                false,
            )?;
            record.finish()?;
            fields.remove("label");
            fields.remove("statementCount");
            details(&scope, &fields, lines)?;
        }
    }
    details(&key, &fields, lines)
}

fn kind_name(kind: &ZenumlStatementKind) -> &'static str {
    match kind {
        ZenumlStatementKind::Message { .. } => "message",
        ZenumlStatementKind::Creation { .. } => "creation",
        ZenumlStatementKind::Return { .. } => "return",
        ZenumlStatementKind::Fragment { .. } => "fragment",
        ZenumlStatementKind::Reference { .. } => "reference",
        ZenumlStatementKind::Divider { .. } => "divider",
    }
}

fn optional_text(text: Option<&str>, record: &mut RecordWriter<'_, '_>) -> Result<()> {
    match text {
        Some(text) => record.text(text, true),
        None => record.text("null", false),
    }
}

fn write_endpoints(
    from: Option<&str>,
    to: Option<&str>,
    identities: &Identities<'_>,
    record: &mut RecordWriter<'_, '_>,
) -> Result<()> {
    record.text(" from=", false)?;
    identities.optional_reference(from, IdentityKind::Participant, record)?;
    record.text(" to=", false)?;
    identities.optional_reference(to, IdentityKind::Participant, record)
}

#[derive(Clone, Copy)]
enum IdentityKind {
    Participant,
    Group,
}

impl IdentityKind {
    fn prefix(self) -> char {
        match self {
            Self::Participant => 'P',
            Self::Group => 'G',
        }
    }

    fn field(self) -> &'static str {
        match self {
            Self::Participant => "name",
            Self::Group => "id",
        }
    }
}

struct Identities<'a> {
    participants: HashMap<&'a str, Vec<usize>>,
    groups: HashMap<&'a str, Vec<usize>>,
    resources: ResourceContext,
}

impl<'a> Identities<'a> {
    fn new(model: &'a ZenumlDiagramModel, lines: &SemanticFallbackWriter<'_>) -> Result<Self> {
        let resources = lines.semantic_resources.clone();
        let count = resources.checked_work_add(model.participants.len(), model.groups.len())?;
        resources.charge_layout_work(count)?;
        resources.check(AsciiResourceLimitId::MaxDocumentCells, count)?;
        resources.check(AsciiResourceLimitId::MaxOutputBytes, count)?;
        let mut identities = Self {
            participants: HashMap::new(),
            groups: HashMap::new(),
            resources,
        };
        for (map, declarations) in [
            (&mut identities.participants, model.participants.len()),
            (&mut identities.groups, model.groups.len()),
        ] {
            map.try_reserve(declarations)
                .map_err(|_| allocation_failed())?;
        }
        for (index, participant) in model.participants.iter().enumerate() {
            identities
                .resources
                .charge_layout_work(participant.name.len().max(1))?;
            let keys = identities
                .participants
                .entry(&participant.name)
                .or_default();
            keys.try_reserve(1).map_err(|_| allocation_failed())?;
            keys.push(index + 1);
        }
        for (index, group) in model.groups.iter().enumerate() {
            if let Some(id) = &group.id {
                identities.resources.charge_layout_work(id.len().max(1))?;
                let keys = identities.groups.entry(id).or_default();
                keys.try_reserve(1).map_err(|_| allocation_failed())?;
                keys.push(index + 1);
            }
        }
        Ok(identities)
    }

    fn keys(&self, name: &str, kind: IdentityKind) -> Result<&[usize]> {
        self.resources.charge_layout_work(name.len().max(1))?;
        let map = match kind {
            IdentityKind::Participant => &self.participants,
            IdentityKind::Group => &self.groups,
        };
        Ok(map.get(name).map_or(&[], Vec::as_slice))
    }

    fn write_keys(
        &self,
        keys: &[usize],
        kind: IdentityKind,
        record: &mut RecordWriter<'_, '_>,
    ) -> Result<()> {
        self.resources.charge_layout_work(keys.len())?;
        record.text("[", false)?;
        for (index, key) in keys.iter().enumerate() {
            if index > 0 {
                record.text(",", false)?;
            }
            record.text(&format!("{}{key}", kind.prefix()), false)?;
        }
        record.text("]", false)
    }

    fn optional_reference(
        &self,
        name: Option<&str>,
        kind: IdentityKind,
        record: &mut RecordWriter<'_, '_>,
    ) -> Result<()> {
        let Some(name) = name else {
            return record.text("null", false);
        };
        let keys = self.keys(name, kind)?;
        self.write_keys(keys, kind, record)?;
        match keys {
            [] => record.text(" (unresolved)", false)?,
            [_] => {}
            _ => record.text(" (ambiguous)", false)?,
        }
        record.text(&format!(" {}=", kind.field()), false)?;
        record.text(name, true)
    }

    fn references(&self, names: &[String], record: &mut RecordWriter<'_, '_>) -> Result<()> {
        record.text("[", false)?;
        for (index, name) in names.iter().enumerate() {
            if index > 0 {
                record.text("; ", false)?;
            }
            self.optional_reference(Some(name), IdentityKind::Participant, record)?;
        }
        record.text("]", false)
    }

    fn disclose_duplicate(
        &self,
        name: &str,
        kind: IdentityKind,
        index: usize,
        lines: &mut SemanticFallbackWriter<'_>,
    ) -> Result<()> {
        let keys = self.keys(name, kind)?;
        if keys.len() < 2 || keys.first() != Some(&(index + 1)) {
            return Ok(());
        }
        let mut record = RecordWriter::new(lines)?;
        record.text(
            &format!(
                "{}{} identity diagnostic: duplicate {} ",
                kind.prefix(),
                index + 1,
                kind.field(),
            ),
            false,
        )?;
        self.write_keys(keys, kind, &mut record)?;
        record.text(" (ambiguous) ", false)?;
        record.text(kind.field(), false)?;
        record.text("=", false)?;
        record.text(name, true)?;
        record.finish()
    }
}

fn decode<'a, T: Deserialize<'a>>(
    raw: &'a RawValue,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<T> {
    lines.admit_node(1)?;
    lines
        .semantic_resources
        .charge_layout_work(raw.get().len())?;
    serde_json::from_str(raw.get()).map_err(|_| invalid_projection())
}

fn take_array<'a>(
    fields: &mut Fields<'a>,
    name: &str,
    expected: Option<usize>,
    lines: &SemanticFallbackWriter<'_>,
) -> Result<Vec<&'a RawValue>> {
    let values: Vec<_> = decode(fields.remove(name).ok_or_else(invalid_projection)?, lines)?;
    if expected.is_some_and(|expected| values.len() != expected) {
        return Err(invalid_projection());
    }
    Ok(values)
}

fn invalid_projection() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "zenuml",
        feature: "invalid serialized semantic projection",
    }
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed("ascii_semantic_fallback")
}

#[cfg(test)]
mod tests {
    use super::super::{SemanticFallbackError, serialize_bounded_json};
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::{AsciiResourcePolicy, TerminalWidthProfile};
    use merman_model::{OperationControl, OperationPhase};
    use serde_json::{Value, json};

    const MODEL: &[u8] = br##"{
        "title":"  Zen \"title\"\n  ","starter":"A",
        "participants":[
            {"name":"A","label":"  Actor \"A\"\n  ","participantType":"actor",
             "stereotype":"controller","emoji":"\ud83d\ude80","widthSource":"00120.50",
             "color":"#123456","comment":"  actor comment\n  ","groupId":"dup",
             "explicit":true,"isStarter":true,"declarationSpan":{"start":7,"end":19},
             "occurrences":[{"start":7,"end":19},{"start":7,"end":19}]},
            {"name":"A","label":null,"participantType":null,"stereotype":null,
             "emoji":null,"color":null,"groupId":null,"explicit":false,"isStarter":false,
             "declarationSpan":null,"occurrences":[]},
            {"name":"B","label":"","participantType":"boundary","stereotype":"",
             "emoji":"","widthSource":"","color":"","comment":"","groupId":"absent-group",
             "explicit":true,"isStarter":false,"declarationSpan":{"start":20,"end":21},
             "occurrences":[{"start":24,"end":25}]}
        ],
        "groups":[
            {"id":"dup","participantNames":["A","B","absent","A"],"span":{"start":0,"end":26}},
            {"id":"dup","participantNames":[],"span":{"start":27,"end":28}},
            {"id":null,"participantNames":[],"span":{"start":29,"end":30}}
        ],
        "statements":[
            {"id":"repeated-id","comment":"  call comment\n  ","span":{"start":31,"end":160},
             "kind":"message","explicit_from":"authored-sender","resolved_from":"A",
             "resolved_to":"B","label":"  call(\"x\")\nnext\t\u001b  ","assignment":"",
             "style":"asynchronous","body_comment":"  closing call\n  ","body":[
                {"id":"fragment","comment":null,"span":{"start":40,"end":150},
                 "kind":"fragment","fragment_kind":"alternative","label":null,"sections":[
                    {"label":"","statements":[],"bodyComment":"empty section",
                     "span":{"start":42,"end":43}},
                    {"label":"ok","span":{"start":44,"end":130},"statements":[
                        {"id":"create","comment":"creation comment","span":{"start":50,"end":120},
                         "kind":"creation","resolved_from":"B","resolved_to":"missing",
                         "constructor":"Widget","parameters":"(a, \"b\")","assignment":"instance",
                         "label":"new Widget","body":[
                            {"id":"empty-call","comment":"","span":{"start":60,"end":62},
                             "kind":"message","explicit_from":null,"resolved_from":null,
                             "resolved_to":null,"label":"","assignment":null,
                             "style":"synchronous","body":[],"body_comment":""}
                         ]}
                    ]},
                    {"label":"else","bodyComment":"","span":{"start":131,"end":149},
                     "statements":[
                        {"id":"return","comment":null,"span":{"start":135,"end":148},
                         "kind":"return","explicit_from":"authored-return","resolved_from":null,
                         "explicit_to":"authored-caller","resolved_to":"A","label":" result "}
                    ]}
                 ]}
             ]},
            {"id":"repeated-id","comment":"reference comment","span":{"start":161,"end":170},
             "kind":"reference","participants":["A","missing","A"],"label":"  Ref \"name\"  "},
            {"id":"divider","comment":null,"span":{"start":171,"end":180},
             "kind":"divider","label":"  Phase\nnext  "},
            {"id":"empty-creation","comment":null,"span":{"start":181,"end":190},
             "kind":"creation","resolved_from":null,"resolved_to":"B","constructor":"B",
             "parameters":"","assignment":null,"label":"new B","body":[],
             "body_comment":"closing empty creation"},
            {"id":"empty-fragment","comment":null,"span":{"start":191,"end":199},
             "kind":"fragment","fragment_kind":"optional","label":"","sections":[]}
        ]
    }"##;

    const SOURCE_CONFIG: &str = r#"{"zenuml":{"theme":"dark","empty":[],"flags":[true,false,null]},"opaque":"\ud800","number":1e+09}"#;

    fn model() -> ZenumlDiagramModel {
        serde_json::from_slice(MODEL).unwrap()
    }

    fn projected(model: &ZenumlDiagramModel) -> Value {
        // Only the borrowed projection is serialized; the canonical nested serializer is never used.
        serde_json::from_slice(&serde_json::to_vec(&Projection(model)).unwrap()).unwrap()
    }

    fn envelope(model: &ZenumlDiagramModel) -> Vec<u8> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Envelope<'a> {
            #[serde(flatten)]
            model: Projection<'a>,
            #[serde(rename = "type")]
            diagram_type: &'static str,
            source_config: &'a RawValue,
            source_title: Option<&'a str>,
            future_root: &'a RawValue,
        }
        serde_json::to_vec(&Envelope {
            model: Projection(model),
            diagram_type: "zenuml",
            source_config: serde_json::from_str(SOURCE_CONFIG).unwrap(),
            source_title: Some("  Source \"title\"\n  "),
            future_root: serde_json::from_str(r#"{"opaque":"\ud800","number":1e+09}"#).unwrap(),
        })
        .unwrap()
    }

    fn render(
        model: &ZenumlDiagramModel,
        bytes: &[u8],
        policy: &AsciiResourcePolicy,
        width: usize,
    ) -> Result<String> {
        let execution = AsciiExecution::for_test(policy);
        let mut lines =
            SemanticFallbackWriter::new(execution, width, TerminalWidthProfile::Unicode);
        assert!(write_if_zenuml(
            &AsciiDiagram::Zenuml(model),
            bytes,
            &mut lines,
        )?);
        Ok(lines.finish().into_text())
    }

    fn empty_sections_model(count: usize) -> ZenumlDiagramModel {
        ZenumlDiagramModel {
            title: None,
            starter: None,
            participants: Vec::new(),
            groups: Vec::new(),
            statements: vec![ZenumlStatement {
                id: String::new(),
                comment: None,
                span: SourceSpan::new(0, 0),
                kind: ZenumlStatementKind::Fragment {
                    fragment_kind: ZenumlFragmentKind::Alternative,
                    label: None,
                    sections: vec![
                        ZenumlFragmentSection {
                            label: None,
                            statements: Vec::new(),
                            body_comment: None,
                            span: SourceSpan::new(0, 0),
                        };
                        count
                    ],
                },
            }],
        }
    }

    #[test]
    fn controlled_complexity_preserves_the_complete_shallow_serialized_shape() {
        let model = model();
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let projection = ControlledProjection::new(&model, &resources);
        let actual: Value =
            serde_json::from_slice(&serde_json::to_vec(&projection).unwrap()).unwrap();
        assert_eq!(actual, projected(&model));
        assert_eq!(projection.take_error(), None);
        assert_eq!(
            complexity(&model, &resources).unwrap(),
            ModelComplexity::from_serializable(&Projection(&model)),
        );
    }

    #[test]
    fn one_work_unit_rejects_wide_participantless_models_before_scanning() {
        let mut siblings = empty_sections_model(0);
        siblings.statements = vec![siblings.statements[0].clone(); 16_384];
        for model in [siblings, empty_sections_model(16_384)] {
            let policy = AsciiResourcePolicy::unbounded()
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
                .unwrap();
            let resources = ResourceContext::new(policy);
            assert!(matches!(
                complexity(&model, &resources),
                Err(AsciiError::ResourceLimitExceeded(details))
                    if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                        && details.actual == 2
            ));
            assert_eq!(resources.layout_work_used(), 1);

            let resources = ResourceContext::new(policy);
            let projection = ControlledProjection::new(&model, &resources);
            assert!(serde_json::to_writer(std::io::sink(), &projection).is_err());
            assert!(matches!(
                projection.take_error(),
                Some(AsciiError::ResourceLimitExceeded(details))
                    if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                        && details.actual == 2
            ));
            assert_eq!(resources.layout_work_used(), 1);
        }
    }

    #[test]
    fn traversal_charges_empty_sections_inside_a_single_next_call() {
        let model = empty_sections_model(16_384);
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 4)
            .unwrap();
        let resources = ResourceContext::new(policy);
        let control = ProjectionControl::new(&resources);
        let mut traversal = Traversal::new(&model.statements, &control).unwrap();
        assert_eq!(traversal.next().unwrap().unwrap().ordinal, 1);
        let capacity = traversal.frames.capacity();
        assert!(matches!(
            traversal.next(),
            Err(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == 5
        ));
        let Some(Frame::Sections { remaining, .. }) = traversal.frames.last() else {
            panic!("the section cursor must remain bounded at the rejected scan");
        };
        assert_eq!(remaining.len(), 16_383);
        assert_eq!(traversal.frames.capacity(), capacity);
    }

    #[test]
    fn controlled_projection_preserves_an_injected_traversal_allocation_error() {
        #[derive(Serialize)]
        struct Envelope<'a, 'model, 'resources> {
            #[serde(flatten)]
            model: &'a ControlledProjection<'model, 'resources>,
        }

        let model = empty_sections_model(0);
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let fail_reserve = || Err(allocation_failed());
        let projection = ControlledProjection::new(&model, &resources);
        projection.control.before_reserve.set(Some(&fail_reserve));
        assert!(serde_json::to_writer(std::io::sink(), &Envelope { model: &projection }).is_err());
        let error = projection.take_error().unwrap();
        assert_eq!(error, allocation_failed());
        assert!(matches!(
            super::super::map_semantic_fallback_error(error),
            SemanticFallbackError::Resource(AsciiError::AllocationFailed { .. })
        ));
        assert_eq!(projection.take_error(), None);
    }

    #[test]
    fn complexity_and_controlled_projection_observe_cancellation_during_the_walk() {
        let model = empty_sections_model(4_096);
        for serialize in [false, true] {
            let operation = OperationControl::new();
            operation.cancel_after_checkpoints(128);
            let resources = ResourceContext::new(AsciiResourcePolicy::unbounded())
                .controlled(operation, OperationPhase::Semantic);
            let error = if serialize {
                let projection = ControlledProjection::new(&model, &resources);
                assert!(serde_json::to_writer(std::io::sink(), &projection).is_err());
                projection.take_error().unwrap()
            } else {
                complexity(&model, &resources).unwrap_err()
            };
            assert!(matches!(
                error,
                AsciiError::Cancelled(cancelled) if cancelled.phase == OperationPhase::Semantic
            ));
            assert!(resources.layout_work_used() > 0);
            assert!(resources.layout_work_used() < 4_096);
        }
    }

    #[test]
    fn semantic_nesting_is_rejected_before_retaining_the_next_ancestor() {
        let mut model = empty_sections_model(0);
        for _ in 0..32 {
            let body = std::mem::take(&mut model.statements);
            model = empty_sections_model(1);
            let ZenumlStatementKind::Fragment { sections, .. } = &mut model.statements[0].kind
            else {
                unreachable!();
            };
            sections[0].statements = body;
        }
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxNestingDepth, 8)
            .unwrap();
        let resources = ResourceContext::new(policy);
        let reserves = Cell::new(0);
        let count_reserves = || {
            reserves.set(reserves.get() + 1);
            Ok(())
        };
        let projection = ControlledProjection::new(&model, &resources);
        projection.control.before_reserve.set(Some(&count_reserves));
        assert!(serde_json::to_writer(std::io::sink(), &projection).is_err());
        assert!(matches!(
            projection.take_error(),
            Some(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxNestingDepth
                    && details.actual == 9
        ));
        assert_eq!(projection.control.nesting_depth.get(), 8);
        // One root iterator and two iterators per admitted fragment level. No ninth level
        // reaches the allocation hook, regardless of how many ancestors remain in the model.
        assert_eq!(reserves.get(), 1 + 2 * 8);
    }

    #[test]
    fn flat_preorder_retains_parent_and_section_without_using_authored_ids_as_keys() {
        let model = model();
        let projection = projected(&model);
        let statements = projection["statements"].as_array().unwrap();
        let expected = [
            ("repeated-id", None, None),
            ("fragment", Some(1), None),
            ("create", Some(2), Some(2)),
            ("empty-call", Some(3), None),
            ("return", Some(2), Some(3)),
            ("repeated-id", None, None),
            ("divider", None, None),
            ("empty-creation", None, None),
            ("empty-fragment", None, None),
        ];
        assert_eq!(statements.len(), expected.len());
        for (index, (statement, (id, parent, section))) in
            statements.iter().zip(expected).enumerate()
        {
            assert_eq!(statement["ordinal"], json!(index + 1));
            assert_eq!(statement["id"], id);
            assert_eq!(statement["parent"], json!(parent));
            assert_eq!(statement["section"], json!(section));
            assert!(statement.get("body").is_none());
        }
        let output = render(
            &model,
            &envelope(&model),
            &AsciiResourcePolicy::default(),
            usize::MAX,
        )
        .unwrap();
        let mut previous = 0;
        for (index, kind) in [
            "message",
            "fragment",
            "creation",
            "message",
            "return",
            "reference",
            "divider",
            "creation",
            "fragment",
        ]
        .iter()
        .enumerate()
        {
            let position = output.find(&format!("S{} [{kind}]", index + 1)).unwrap();
            assert!(position >= previous);
            previous = position;
        }
        assert!(output.contains("parent=S2 section=2"));
        assert!(output.contains("parent=S3 section=null"));
        assert!(output.contains("parent=S2 section=3"));
    }

    #[test]
    fn every_original_shallow_scalar_flag_null_and_span_survives() {
        let source: Value = serde_json::from_slice(MODEL).unwrap();
        let projection = projected(&model());
        for field in ["title", "starter", "participants", "groups"] {
            assert_eq!(projection[field], source[field], "changed {field}");
        }
        let source_statements = &source["statements"];
        let fragment = &source_statements[0]["body"][0];
        let creation = &fragment["sections"][1]["statements"][0];
        let authored = [
            &source_statements[0],
            fragment,
            creation,
            &creation["body"][0],
            &fragment["sections"][2]["statements"][0],
            &source_statements[1],
            &source_statements[2],
            &source_statements[3],
            &source_statements[4],
        ];
        for (expected, actual) in authored
            .iter()
            .zip(projection["statements"].as_array().unwrap())
        {
            for (name, value) in expected.as_object().unwrap() {
                if name != "body" && name != "sections" {
                    assert_eq!(actual.get(name), Some(value), "changed {name}: {expected}");
                }
            }
            assert_eq!(
                actual.get("body_comment").is_some(),
                expected.get("body_comment").is_some(),
            );
        }
        for (expected, actual) in fragment["sections"]
            .as_array()
            .unwrap()
            .iter()
            .zip(projection["statements"][1]["sections"].as_array().unwrap())
        {
            for (name, value) in expected.as_object().unwrap() {
                if name != "statements" {
                    assert_eq!(actual.get(name), Some(value), "changed section {name}");
                }
            }
            assert_eq!(
                actual.get("bodyComment").is_some(),
                expected.get("bodyComment").is_some(),
            );
            assert!(actual.get("statements").is_none());
        }
    }

    #[test]
    fn empty_bodies_sections_and_collections_remain_explicit() {
        let model = model();
        let projection = projected(&model);
        let statements = &projection["statements"];
        for (index, count) in [(0, 1), (2, 1), (3, 0), (7, 0)] {
            assert_eq!(statements[index]["bodyCount"], count);
        }
        let sections = statements[1]["sections"].as_array().unwrap();
        assert_eq!(sections.len(), 3);
        for (section, count) in sections.iter().zip([0, 1, 1]) {
            assert_eq!(section["statementCount"], count);
        }
        assert_eq!(statements[8]["sections"], json!([]));
        let empty = ZenumlDiagramModel {
            title: None,
            starter: None,
            participants: Vec::new(),
            groups: Vec::new(),
            statements: Vec::new(),
        };
        assert_eq!(
            projected(&empty),
            json!({"title":null,"starter":null,"participants":[],"groups":[],"statements":[]}),
        );
        let output = render(
            &model,
            &envelope(&model),
            &AsciiResourcePolicy::default(),
            usize::MAX,
        )
        .unwrap();
        for text in [
            "S2 section 1 [fragment section] label=\"\" statementCount=0",
            "S8 [creation]",
            "bodyCount=0",
            "S9 [fragment] id=\"empty-fragment\" parent=null section=null fragment=opt label=\"\" sections=0",
        ] {
            assert!(output.contains(text), "missing {text}:\n{output}");
        }
        let empty_output = render(
            &empty,
            &envelope(&empty),
            &AsciiResourcePolicy::default(),
            usize::MAX,
        )
        .unwrap();
        assert!(empty_output.contains("ZenUML title=null"));
        assert!(empty_output.contains("ZenUML starter=null"));
        assert!(empty_output.contains("participants=0 groups=0 rootStatements=0 statements=0"));
    }

    #[test]
    fn reading_records_preserve_escaping_ambiguous_references_and_complete_raw_metadata() {
        let model = model();
        let bytes = envelope(&model);
        let output = render(&model, &bytes, &AsciiResourcePolicy::default(), usize::MAX).unwrap();
        for text in [
            r#"ZenUML title="  Zen \"title\"\n  ""#,
            r#"ZenUML starter=[P1,P2] (ambiguous) name="A""#,
            r#"P1 [participant] name="A" label="  Actor \"A\"\n  " participantType="actor" explicit=true isStarter=true group=[G1,G2] (ambiguous) id="dup""#,
            r#"group=[] (unresolved) id="absent-group""#,
            r#"G1 [group] id="dup" participants=[[P1,P2] (ambiguous) name="A"; [P3] name="B"; [] (unresolved) name="absent"; [P1,P2] (ambiguous) name="A"]"#,
            "G3 [group] id=null participants=[]",
            r#"from=[P1,P2] (ambiguous) name="A" to=[P3] name="B" style=asynchronous assignment="""#,
            r#"to=[] (unresolved) name="missing" assignment="instance""#,
            "from=null to=null style=synchronous assignment=null",
            r#"participants=[[P1,P2] (ambiguous) name="A"; [] (unresolved) name="missing"; [P1,P2] (ambiguous) name="A"]"#,
            r#"label="  call(\"x\")\nnext\t\u{1B}  ""#,
            r#"S7 [divider] id="divider" parent=null section=null label="  Phase\nnext  ""#,
        ] {
            assert!(output.contains(text), "missing {text}:\n{output}");
        }
        assert_eq!(output.matches("identity diagnostic: duplicate").count(), 2);
        assert!(!output.contains('\u{1b}'));
        let compact = output.split_whitespace().collect::<String>();
        for text in [
            r#"widthSource:"00120.50""#,
            r#"stereotype:"controller""#,
            r#"declarationSpan:{"start":7,"end":19}"#,
            r#"occurrences:[{"start":7,"end":19},{"start":7,"end":19}]"#,
            r#"participantNames:["A","B","absent","A"]"#,
            r#"explicit_from:"authored-sender""#,
            r#"explicit_to:"authored-caller""#,
            r#"body_comment:"closingcall\n""#,
            r#"constructor:"Widget""#,
            r#"parameters:"(a,\"b\")""#,
            r#"fragment_kind:"alternative""#,
            r#"S2section1details:{bodyComment:"emptysection",span:{"start":42,"end":43}}"#,
            r#"sourceConfig:{"zenuml":{"theme":"dark","empty":[],"flags":[true,false,null]},"opaque":"\ud800","number":1e+09}"#,
            r#"sourceTitle:"Source\"title\"\n""#,
            r#"futureRoot:{"opaque":"\ud800","number":1e+09}"#,
        ] {
            assert!(compact.contains(text), "missing {text}:\n{output}");
        }
        let narrow = render(&model, &bytes, &AsciiResourcePolicy::default(), 23).unwrap();
        assert!(narrow.lines().all(|line| {
            crate::text::display_width_with_profile(line, TerminalWidthProfile::Unicode) <= 23
        }));
        assert_eq!(narrow.split_whitespace().collect::<String>(), compact);
    }

    #[test]
    fn traversal_storage_depends_on_depth_instead_of_sibling_count() {
        let mut model = model();
        let leaf = model.statements[2].clone();
        model.statements = vec![leaf; 4_096];
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let control = ProjectionControl::new(&resources);
        let mut traversal = Traversal::new(&model.statements, &control).unwrap();
        for ordinal in 1..=4_096 {
            assert_eq!(traversal.frames.len(), 1);
            assert_eq!(traversal.next().unwrap().unwrap().ordinal, ordinal);
            assert_eq!(traversal.frames.len(), 1);
        }
        assert!(traversal.next().unwrap().is_none());
        assert!(traversal.frames.is_empty());
    }

    #[test]
    fn deep_message_bodies_serialize_as_shallow_records() {
        let mut model = ZenumlDiagramModel {
            title: None,
            starter: None,
            participants: Vec::new(),
            groups: Vec::new(),
            statements: vec![ZenumlStatement {
                id: "leaf".into(),
                comment: None,
                span: SourceSpan::new(0, 0),
                kind: ZenumlStatementKind::Divider { label: "".into() },
            }],
        };
        for index in 0..192 {
            let body = std::mem::take(&mut model.statements);
            model.statements.push(ZenumlStatement {
                id: format!("call-{index}"),
                comment: None,
                span: SourceSpan::new(index, index + 1),
                kind: ZenumlStatementKind::Message {
                    explicit_from: None,
                    resolved_from: None,
                    resolved_to: None,
                    label: String::new(),
                    assignment: None,
                    style: ZenumlMessageStyle::Synchronous,
                    body,
                    body_comment: None,
                },
            });
        }
        let projection = projected(&model);
        assert_eq!(projection["statements"].as_array().unwrap().len(), 193);
        assert_eq!(projection["statements"][192]["id"], "leaf");
        assert_eq!(projection["statements"][192]["parent"], 192);
    }

    #[test]
    fn streaming_projection_observes_output_budget_and_cancellation() {
        let mut model = model();
        model.title = None;
        model.starter = None;
        model.participants.clear();
        model.groups.clear();
        let policy = AsciiResourcePolicy::unbounded()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 128)
            .unwrap();
        let execution = AsciiExecution::for_test(&policy);
        assert!(matches!(
            serialize_bounded_json(&Projection(&model), execution),
            Err(SemanticFallbackError::Resource(AsciiError::ResourceLimitExceeded(details)))
                if details.limit == AsciiResourceLimitId::MaxOutputBytes
        ));
        let control = OperationControl::new();
        control.cancel_after_checkpoints(64);
        let policy = AsciiResourcePolicy::unbounded();
        let execution = AsciiExecution::new(&control, &policy);
        assert!(matches!(
            serialize_bounded_json(&Projection(&model), execution),
            Err(SemanticFallbackError::Cancelled(cancelled))
                if cancelled.phase == OperationPhase::Semantic
        ));
    }
}
