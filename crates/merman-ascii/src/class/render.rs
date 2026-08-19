use crate::AsciiError;
use crate::Result;
use crate::color::AsciiColorRole;
use crate::operation::AsciiExecution;
use crate::options::{AsciiCharset, AsciiRenderOptions, TerminalWidthProfile};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
#[cfg(test)]
use crate::relation_graph::RelationGraphHorizontalDirection;
use crate::relation_graph::{
    HorizontalRelationEndpoint, HorizontalRelationMarker, HorizontalRelationStyle,
    LayeredRelationEdge, LayeredRelationError, LayeredRelationRouteStyle, RelationDirection,
    RelationGraphBoxStyle, RelationGraphLabel, RelationGraphLabelBatchPlan, RelationGraphLabelPlan,
    RelationGraphLine, RelationGraphSummaryRow, RelationLineChars, RelationOverlay,
    RelationParallelPlan, RelationPortSide, RelationRegionPlan, RelationSelfLoopMetrics,
    RelationStackPlan, RelationSummaryPaintPlan,
};
#[cfg(test)]
use crate::resource::AsciiResourceLimitId;
use crate::resource::{AsciiResourceLimitPhase, LogicalExtent, ResourceContext};
use crate::safe_text::{
    ComposedTextPlan, DeferredTextLine, DeferredTextPart, DeferredTextRegistry, charge_text_layout,
    grapheme_safe_trim, terminal_single_line_text_requires_normalization,
    terminal_text_requires_normalization,
};
use crate::text::display_width_with_profile;
use merman_core::common::GenericTypesPlan;
use merman_core::models::class_diagram::{
    ClassDiagram, ClassInterface, ClassMember, ClassNode, ClassNote, ClassRelation,
};
use std::collections::{BTreeMap, HashMap, HashSet};

mod namespace;

use namespace::{
    has_renderable_namespaces, render_namespace_container_box, render_namespaced_class_diagram,
    validate_class_namespace_ownership,
};

const CLASS_LEVEL_HORIZONTAL_GAP: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClassCharset {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
    separator_left: char,
    separator_right: char,
    solid_horizontal_relation: char,
    solid_vertical_relation: char,
    dotted_horizontal_relation: char,
    dotted_vertical_relation: char,
    relation_junction: char,
    extension_up: char,
    extension_down: char,
    arrow_up: char,
    arrow_down: char,
    aggregation: char,
    composition: char,
    lollipop: char,
}

impl ClassCharset {
    fn for_options(options: &AsciiRenderOptions) -> Self {
        match options.structural_charset() {
            AsciiCharset::Ascii => Self {
                top_left: '+',
                top_right: '+',
                bottom_left: '+',
                bottom_right: '+',
                horizontal: '-',
                vertical: '|',
                separator_left: '+',
                separator_right: '+',
                solid_horizontal_relation: '-',
                solid_vertical_relation: '|',
                dotted_horizontal_relation: '.',
                dotted_vertical_relation: ':',
                relation_junction: '+',
                extension_up: '^',
                extension_down: 'v',
                arrow_up: '^',
                arrow_down: 'v',
                aggregation: 'o',
                composition: '*',
                lollipop: 'o',
            },
            AsciiCharset::Unicode => Self {
                top_left: '┌',
                top_right: '┐',
                bottom_left: '└',
                bottom_right: '┘',
                horizontal: '─',
                vertical: '│',
                separator_left: '├',
                separator_right: '┤',
                solid_horizontal_relation: '─',
                solid_vertical_relation: '│',
                dotted_horizontal_relation: '╌',
                dotted_vertical_relation: '┆',
                relation_junction: '┼',
                extension_up: '△',
                extension_down: '▽',
                arrow_up: '▲',
                arrow_down: '▼',
                aggregation: '◇',
                composition: '◆',
                lollipop: '○',
            },
        }
    }
}

type RenderedClassBox = RelationGraphBox;

#[derive(Clone, Copy)]
struct ClassRenderSettings<'a> {
    options: &'a AsciiRenderOptions,
    charset: ClassCharset,
    direction: RelationDirection,
}

#[derive(Debug)]
struct ClassNoteIndex<'a> {
    by_id: HashMap<&'a str, &'a ClassNote>,
}

impl<'a> ClassNoteIndex<'a> {
    fn new(notes: &'a [ClassNote], resources: &mut ResourceContext) -> Result<Self> {
        resources.charge_layout_work(notes.len())?;
        let mut by_id = HashMap::new();
        by_id
            .try_reserve(notes.len())
            .map_err(|_| layout_allocation_failed())?;
        for note in notes {
            // Preserve the previous linear lookup's first-match behavior for duplicate ids.
            by_id.entry(note.id.as_str()).or_insert(note);
        }
        Ok(Self { by_id })
    }

    fn get(&self, id: &str, resources: &mut ResourceContext) -> Result<Option<&'a ClassNote>> {
        resources.charge_layout_work(1)?;
        Ok(self.by_id.get(id).copied())
    }
}

#[derive(Debug, Clone, Copy)]
struct ResolvedClassEndpoint<'a> {
    authored_id: &'a str,
    resolved_id: &'a str,
    owner: Option<&'a str>,
}

impl<'a> ResolvedClassEndpoint<'a> {
    const fn into_layout_endpoint(self) -> RelationEndpoint<'a> {
        RelationEndpoint {
            authored_id: self.authored_id,
            resolved_id: self.resolved_id,
            render_id: self.resolved_id,
            owner: self.owner,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClassEndpointMetadata<'a> {
    owner: Option<&'a str>,
}

#[derive(Debug)]
struct ClassEndpointIndex<'a> {
    endpoints: HashMap<&'a str, ClassEndpointMetadata<'a>>,
    facade_aliases: &'a BTreeMap<String, String>,
}

impl<'a> ClassEndpointIndex<'a> {
    fn new(model: &'a ClassDiagram, resources: &mut ResourceContext) -> Result<Self> {
        let capacity = model
            .classes
            .len()
            .checked_add(model.interfaces.len())
            .ok_or_else(|| work_overflow(resources))?;
        resources.charge_layout_work(capacity)?;

        let mut endpoints = HashMap::new();
        endpoints
            .try_reserve(capacity)
            .map_err(|_| layout_allocation_failed())?;
        endpoints.extend(model.classes.values().map(|class| {
            (
                class.id.as_str(),
                ClassEndpointMetadata {
                    owner: class.parent.as_deref(),
                },
            )
        }));
        for interface in &model.interfaces {
            let target = model.classes.get(interface.class_id.as_str()).ok_or(
                AsciiError::UnsupportedFeature {
                    diagram_type: "class",
                    feature: "interfaces with missing target classes",
                },
            )?;
            endpoints.insert(
                interface.id.as_str(),
                ClassEndpointMetadata {
                    owner: target.parent.as_deref(),
                },
            );
        }
        Ok(Self {
            endpoints,
            facade_aliases: &model.namespace_facade_aliases,
        })
    }

    fn resolve(&self, authored_id: &'a str) -> Option<ResolvedClassEndpoint<'a>> {
        let resolved_id = self
            .facade_aliases
            .get(authored_id)
            .map(String::as_str)
            .unwrap_or(authored_id);
        self.endpoints
            .get(resolved_id)
            .map(|metadata| ResolvedClassEndpoint {
                authored_id,
                resolved_id,
                owner: metadata.owner,
            })
    }

    fn is_facade(&self, id: &str) -> bool {
        self.facade_aliases.contains_key(id)
    }
}

#[derive(Debug)]
struct RenderedClassBoxIndex<'a> {
    by_id: HashMap<&'a str, &'a RenderedClassBox>,
}

impl<'a> RenderedClassBoxIndex<'a> {
    fn new(boxes: &'a [RenderedClassBox], resources: &mut ResourceContext) -> Result<Self> {
        resources.charge_layout_work(boxes.len())?;
        let mut by_id = HashMap::new();
        by_id
            .try_reserve(boxes.len())
            .map_err(|_| layout_allocation_failed())?;
        for relation_box in boxes {
            // Preserve the previous linear lookup's first-match behavior for duplicate ids.
            by_id.entry(relation_box.id()).or_insert(relation_box);
        }
        Ok(Self { by_id })
    }

    fn get(
        &self,
        id: &str,
        resources: &mut ResourceContext,
    ) -> Result<Option<&'a RenderedClassBox>> {
        resources.charge_layout_work(1)?;
        Ok(self.by_id.get(id).copied())
    }

    #[cfg(test)]
    fn require(&self, id: &str, resources: &mut ResourceContext) -> Result<&'a RenderedClassBox> {
        self.get(id, resources)?
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "relationships with missing endpoint classes",
            })
    }
}

struct ClassRelationComponentAdapter {
    charset: ClassCharset,
    width_profile: TerminalWidthProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelationMarker {
    Extension,
    Dependency,
    Aggregation,
    Composition,
    Lollipop,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MarkerSide {
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EndpointLabelRole {
    First,
    Second,
}

impl EndpointLabelRole {
    const fn disclosure_prefix(self) -> &'static str {
        match self {
            Self::First => "endpoint 1: ",
            Self::Second => "endpoint 2: ",
        }
    }

    const fn summary_open(self, leading_space: bool) -> &'static str {
        match (self, leading_space) {
            (Self::First, false) => "endpoint1=[",
            (Self::Second, false) => "endpoint2=[",
            (Self::First, true) => " endpoint1=[",
            (Self::Second, true) => " endpoint2=[",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelationLine {
    Solid,
    Dotted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RelationLayout<'a> {
    top: RelationEndpoint<'a>,
    bottom: RelationEndpoint<'a>,
    top_marker: Option<RelationMarker>,
    bottom_marker: Option<RelationMarker>,
    line: RelationLine,
    label: Option<RelationGraphLabel>,
    top_endpoint_label: Option<RelationGraphLabel>,
    bottom_endpoint_label: Option<RelationGraphLabel>,
    top_endpoint_role: EndpointLabelRole,
    bottom_endpoint_role: EndpointLabelRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ClassRelationLabels {
    label: Option<RelationGraphLabel>,
    left_endpoint: Option<RelationGraphLabel>,
    right_endpoint: Option<RelationGraphLabel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RelationEndpoint<'a> {
    authored_id: &'a str,
    resolved_id: &'a str,
    render_id: &'a str,
    owner: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RelationRouteEndpoint<'a> {
    pub(super) render_id: &'a str,
    pub(super) facade_member: Option<&'a str>,
}

impl<'a> RelationEndpoint<'a> {
    fn disclosure_id(self) -> Option<&'a str> {
        if self.authored_id == self.resolved_id {
            None
        } else {
            Some(self.authored_id)
        }
    }
}

impl<'a> RelationLayout<'a> {
    fn apply_direction(&mut self, direction: RelationDirection) {
        if direction != RelationDirection::BottomUp {
            return;
        }
        std::mem::swap(&mut self.top, &mut self.bottom);
        std::mem::swap(&mut self.top_marker, &mut self.bottom_marker);
        std::mem::swap(
            &mut self.top_endpoint_label,
            &mut self.bottom_endpoint_label,
        );
        std::mem::swap(&mut self.top_endpoint_role, &mut self.bottom_endpoint_role);
    }

    pub(super) fn with_render_endpoints(
        mut self,
        top: RelationRouteEndpoint<'a>,
        bottom: RelationRouteEndpoint<'a>,
        width_profile: TerminalWidthProfile,
        deferred_text: &mut DeferredTextRegistry<'a>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        resources.transaction(|resources| {
            self.top.render_id = top.render_id;
            self.bottom.render_id = bottom.render_id;
            self.top_endpoint_label = join_endpoint_label_with_authored_ref(
                self.top.disclosure_id(),
                self.top_endpoint_label.take(),
                width_profile,
                deferred_text,
                resources,
            )?;
            self.bottom_endpoint_label = join_endpoint_label_with_authored_ref(
                self.bottom.disclosure_id(),
                self.bottom_endpoint_label.take(),
                width_profile,
                deferred_text,
                resources,
            )?;
            self.top_endpoint_label = join_endpoint_label_with_facade(
                top.facade_member,
                self.top_endpoint_label.take(),
                width_profile,
                deferred_text,
                resources,
            )?;
            self.bottom_endpoint_label = join_endpoint_label_with_facade(
                bottom.facade_member,
                self.bottom_endpoint_label.take(),
                width_profile,
                deferred_text,
                resources,
            )?;
            Ok(self)
        })
    }
}

fn join_endpoint_label_with_facade<'a>(
    facade_member: Option<&'a str>,
    label: Option<RelationGraphLabel>,
    width_profile: TerminalWidthProfile,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &ResourceContext,
) -> Result<Option<RelationGraphLabel>> {
    let Some(facade_member) = facade_member else {
        return Ok(label);
    };

    let prefix = deferred_text.try_register_parts(width_profile, resources, 3, |push| {
        push(DeferredTextPart::Static("member(bytes="))?;
        push(DeferredTextPart::Decimal(facade_member.len()))?;
        push(DeferredTextPart::Static(")="))
    })?;
    let quoted = deferred_text.try_register_quoted_text(facade_member, width_profile, resources)?;
    let facade = DeferredTextLine::try_concat(&[&prefix, &quoted], resources)?;

    let Some(label) = label else {
        let mut lines = Vec::new();
        lines
            .try_reserve_exact(1)
            .map_err(|_| layout_allocation_failed())?;
        lines.push(facade);
        return RelationGraphLabel::try_from_lines(lines, width_profile, resources).map(Some);
    };
    let separator = deferred_text.try_register_parts(width_profile, resources, 1, |push| {
        push(DeferredTextPart::Static(": "))
    })?;
    let first = label
        .lines()
        .first()
        .map(|line| DeferredTextLine::try_concat(&[&facade, &separator, line], resources))
        .transpose()?
        .unwrap_or(facade);
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(label.line_count().max(1))
        .map_err(|_| layout_allocation_failed())?;
    lines.push(first);
    lines.extend(label.lines().iter().skip(1).cloned());
    RelationGraphLabel::try_from_lines(lines, width_profile, resources).map(Some)
}

fn join_endpoint_label_with_authored_ref<'a>(
    authored_ref: Option<&'a str>,
    label: Option<RelationGraphLabel>,
    width_profile: TerminalWidthProfile,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &ResourceContext,
) -> Result<Option<RelationGraphLabel>> {
    let Some(authored_ref) = authored_ref else {
        return Ok(label);
    };
    let framed = deferred_text.try_register_framed_value(
        "endpointRef(bytes=",
        authored_ref,
        width_profile,
        resources,
    )?;
    let Some(label) = label else {
        let mut lines = Vec::new();
        lines
            .try_reserve_exact(1)
            .map_err(|_| layout_allocation_failed())?;
        lines.push(framed);
        return RelationGraphLabel::try_from_lines(lines, width_profile, resources).map(Some);
    };
    let separator = deferred_text.try_register(
        ComposedTextPlan::try_new(resources, 1, |push| push(": "))?,
        width_profile,
        resources,
    )?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(label.line_count().max(1))
        .map_err(|_| layout_allocation_failed())?;
    for (line_index, line) in label.lines().iter().enumerate() {
        let typed_label =
            deferred_text.try_register_parts(width_profile, resources, 5, |push| {
                push(DeferredTextPart::Static("endpointLabel=[bytes="))?;
                push(DeferredTextPart::Decimal(line.plain_bytes()))?;
                push(DeferredTextPart::Static(" "))?;
                push(DeferredTextPart::QuotedLine(line))?;
                push(DeferredTextPart::Static("]"))
            })?;
        if line_index == 0 {
            lines.push(DeferredTextLine::try_concat(
                &[&framed, &separator, &typed_label],
                resources,
            )?);
        } else {
            lines.push(typed_label);
        }
    }
    RelationGraphLabel::try_from_lines(lines, width_profile, resources).map(Some)
}

pub(crate) fn render_class_diagram_with_execution(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Semantic)?;
    render_class_diagram_impl(model, options, execution)
}

fn render_class_diagram_impl(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let base_resources = ResourceContext::new(*execution.resources());
    let mut resources =
        execution.resource_context(&base_resources, merman_core::OperationPhase::Semantic);
    resources.charge_layout_work(model.direction.len().max(1))?;
    let direction = RelationDirection::try_from_model(
        &model.direction,
        "class",
        "unknown class diagram directions",
    );
    resources.checkpoint()?;
    let direction = direction?;
    preflight_class_text(model, &mut resources)?;
    charge_class_model_work(model, &mut resources)?;
    validate_unique_class_render_ids(model, &mut resources)?;
    let charset = ClassCharset::for_options(options);
    let settings = ClassRenderSettings {
        options,
        charset,
        direction,
    };
    let mut deferred_text = DeferredTextRegistry::new();
    validate_class_namespace_ownership(model, &mut resources)?;
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    resources = execution.resource_context(&resources, merman_core::OperationPhase::Layout);
    let endpoint_index = ClassEndpointIndex::new(model, &mut resources)?;
    validate_class_references(model, &endpoint_index, &mut resources)?;
    let relation_labels =
        prepare_class_relation_labels(model, options, &mut deferred_text, &resources)?;
    if has_renderable_namespaces(model) {
        let rendered = render_namespaced_class_diagram(
            model,
            settings,
            &endpoint_index,
            &relation_labels,
            &mut deferred_text,
            &mut resources,
            execution,
        )?;
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        return Ok(rendered);
    }

    let boxes = render_class_boxes(
        model,
        settings,
        &endpoint_index,
        &mut deferred_text,
        &mut resources,
        execution,
    )?;
    if boxes.is_empty() {
        if !model.relations.is_empty() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "relationships with missing endpoint classes",
            });
        }
        return relation_graph::render_stacked_boxes_with_deferred_options_with_execution(
            &boxes,
            options,
            &mut resources,
            &deferred_text,
            execution,
        );
    }
    let box_by_id = RenderedClassBoxIndex::new(&boxes, &mut resources)?;

    let layout_capacity = model
        .relations
        .len()
        .checked_add(model.notes.len())
        .ok_or_else(|| work_overflow(&resources))?;
    let mut layouts = Vec::new();
    layouts
        .try_reserve_exact(layout_capacity)
        .map_err(|_| layout_allocation_failed())?;
    for (relation_index, relation) in model.relations.iter().enumerate() {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        layouts.push(relation_layout(
            model,
            relation,
            &endpoint_index,
            relation_labels
                .get(relation_index)
                .cloned()
                .ok_or_else(layout_allocation_failed)?,
        )?);
    }
    layouts.extend(note_relation_layouts(
        model,
        &endpoint_index,
        &box_by_id,
        options.terminal_width_profile,
        &mut deferred_text,
        &mut resources,
    )?);
    for layout in &mut layouts {
        layout.apply_direction(direction);
    }

    if layouts.is_empty() {
        if direction.is_horizontal() {
            let lines = relation_graph::render_horizontal_box_strip_lines(
                &boxes,
                direction.horizontal_direction(),
                CLASS_LEVEL_HORIZONTAL_GAP,
                options.terminal_width_profile,
                &resources,
            )?;
            return render_class_document_lines_with_execution(
                lines,
                options,
                &mut resources,
                &deferred_text,
                execution,
            );
        }
        if direction.is_reversed() {
            let lines = relation_graph::stacked_box_lines_ordered(
                &boxes,
                options.terminal_width_profile,
                true,
                &mut resources,
            )?;
            return render_class_document_lines_with_execution(
                lines,
                options,
                &mut resources,
                &deferred_text,
                execution,
            );
        }
        return relation_graph::render_stacked_boxes_with_deferred_options_with_execution(
            &boxes,
            options,
            &mut resources,
            &deferred_text,
            execution,
        );
    }

    if direction.is_horizontal() {
        let lines = render_horizontal_class_component_lines(
            &boxes,
            &layouts,
            settings,
            &mut resources,
            &mut deferred_text,
            execution,
        )?;
        return render_class_document_lines_with_execution(
            lines,
            options,
            &mut resources,
            &deferred_text,
            execution,
        );
    }

    let rendered = render_class_components(
        &boxes,
        &layouts,
        options,
        charset,
        &mut resources,
        &mut deferred_text,
        execution,
    )?;
    execution.checkpoint(merman_core::OperationPhase::Emit)?;
    Ok(rendered)
}

fn validate_unique_class_render_ids(
    model: &ClassDiagram,
    resources: &mut ResourceContext,
) -> Result<()> {
    let capacity = model
        .classes
        .len()
        .checked_add(model.interfaces.len())
        .and_then(|value| value.checked_add(model.notes.len()))
        .and_then(|value| value.checked_add(model.namespaces.len()))
        .ok_or_else(|| work_overflow(resources))?;
    resources.charge_layout_work(capacity)?;

    let mut ids = HashSet::new();
    ids.try_reserve(capacity)
        .map_err(|_| layout_allocation_failed())?;
    let rendered_ids = model
        .classes
        .values()
        .map(|class| class.id.as_str())
        .chain(
            model
                .interfaces
                .iter()
                .map(|interface| interface.id.as_str()),
        )
        .chain(model.notes.iter().map(|note| note.id.as_str()))
        .chain(
            model
                .namespaces
                .values()
                .map(|namespace| namespace.dom_id.as_str()),
        );
    for id in rendered_ids {
        if !ids.insert(id) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "duplicate rendered class ids",
            });
        }
    }
    Ok(())
}

fn validate_class_references(
    model: &ClassDiagram,
    endpoint_index: &ClassEndpointIndex<'_>,
    resources: &mut ResourceContext,
) -> Result<()> {
    for relation in &model.relations {
        resources.charge_layout_work(2)?;
        let left = endpoint_index.resolve(relation.id1.as_str());
        let right = endpoint_index.resolve(relation.id2.as_str());
        if left.is_none() || right.is_none() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "relationships with missing endpoint classes",
            });
        }
    }

    for note in &model.notes {
        let Some(target_id) = note.class_id.as_deref() else {
            continue;
        };
        resources.charge_layout_work(1)?;
        if endpoint_index.resolve(target_id).is_none() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "notes with missing target classes",
            });
        }
    }

    Ok(())
}

fn render_class_boxes<'a>(
    model: &'a ClassDiagram,
    settings: ClassRenderSettings<'_>,
    endpoint_index: &ClassEndpointIndex<'_>,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<RenderedClassBox>> {
    let capacity = model
        .classes
        .len()
        .checked_add(model.interfaces.len())
        .and_then(|value| value.checked_add(model.notes.len()))
        .and_then(|value| value.checked_add(model.namespaces.len()))
        .ok_or_else(|| work_overflow(resources))?;
    let mut boxes = Vec::new();
    boxes
        .try_reserve_exact(capacity)
        .map_err(|_| layout_allocation_failed())?;
    for class in model
        .classes
        .values()
        .filter(|class| !endpoint_index.is_facade(class.id.as_str()))
    {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        boxes.push(render_class_box(
            class,
            settings.options,
            settings.charset,
            deferred_text,
            resources,
        )?);
    }
    for interface in &model.interfaces {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        boxes.push(render_interface_box(
            interface,
            settings.options,
            settings.charset,
            deferred_text,
            resources,
        )?);
    }
    for note in &model.notes {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        boxes.push(render_note_box(
            note,
            settings.options,
            settings.charset,
            deferred_text,
            resources,
        )?);
    }
    for namespace in model.namespaces.values() {
        execution.checkpoint(merman_core::OperationPhase::Layout)?;
        boxes.push(render_namespace_container_box(
            namespace,
            Vec::new(),
            settings,
            deferred_text,
            resources,
            execution,
        )?);
    }
    Ok(boxes)
}

fn render_class_components<'text>(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'text>],
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    resources: &mut ResourceContext,
    deferred_text: &mut DeferredTextRegistry<'text>,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let adapter = ClassRelationComponentAdapter {
        charset,
        width_profile: options.terminal_width_profile,
    };
    relation_graph::render_relation_components_with_deferred_with_execution(
        boxes,
        layouts,
        options,
        resources,
        &adapter,
        deferred_text,
        execution,
    )
}

fn render_class_component_lines<'text>(
    boxes: &[RenderedClassBox],
    _box_by_id: &RenderedClassBoxIndex<'_>,
    layouts: &[RelationLayout<'text>],
    settings: ClassRenderSettings<'_>,
    resources: &mut ResourceContext,
    deferred_text: &mut DeferredTextRegistry<'text>,
    execution: AsciiExecution<'_>,
) -> Result<Vec<RelationGraphLine>> {
    let adapter = ClassRelationComponentAdapter {
        charset: settings.charset,
        width_profile: settings.options.terminal_width_profile,
    };
    relation_graph::render_relation_component_lines_with_execution(
        boxes,
        layouts,
        settings.options,
        resources,
        &adapter,
        deferred_text,
        execution,
    )
}

fn render_class_document_lines_with_execution(
    lines: Vec<RelationGraphLine>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    deferred_text: &DeferredTextRegistry<'_>,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    relation_graph::render_lines_with_deferred_options_with_execution(
        &lines,
        options,
        resources,
        deferred_text,
        execution,
    )
}

fn render_horizontal_class_component_lines<'text>(
    boxes: &[RenderedClassBox],
    layouts: &[RelationLayout<'text>],
    settings: ClassRenderSettings<'_>,
    resources: &mut ResourceContext,
    deferred_text: &mut DeferredTextRegistry<'text>,
    execution: AsciiExecution<'_>,
) -> Result<Vec<RelationGraphLine>> {
    let adapter = ClassRelationComponentAdapter {
        charset: settings.charset,
        width_profile: settings.options.terminal_width_profile,
    };
    relation_graph::render_horizontal_relation_components_with_execution(
        boxes,
        layouts,
        settings.direction.horizontal_direction(),
        settings.options,
        resources,
        &adapter,
        deferred_text,
        execution,
    )
}

fn render_class_box<'a>(
    class: &'a ClassNode,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    let sections = class_sections(
        class,
        options.terminal_width_profile,
        deferred_text,
        resources,
    )?;
    render_box_sections(class.id.clone(), sections, options, charset, resources)
}

fn render_interface_box<'a>(
    interface: &'a ClassInterface,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    let display_plan = ComposedTextPlan::try_new_html_decoded(&interface.label, resources)?;
    let disclose_label =
        authored_display_projection_is_lossy(&display_plan, &interface.label, resources)?;
    let header_capacity = 2usize
        .checked_add(usize::from(disclose_label))
        .ok_or_else(|| work_overflow(resources))?;
    let mut header = Vec::new();
    header
        .try_reserve_exact(header_capacity)
        .map_err(|_| layout_allocation_failed())?;
    header.push(deferred_text.try_register(
        ComposedTextPlan::try_new(resources, 1, |push| push("<<interface>>"))?,
        options.terminal_width_profile,
        resources,
    )?);
    header.push(deferred_text.try_register(
        display_plan,
        options.terminal_width_profile,
        resources,
    )?);
    if disclose_label {
        header.push(deferred_text.try_register_framed_value(
            "interfaceLabel(bytes=",
            &interface.label,
            options.terminal_width_profile,
            resources,
        )?);
    }
    let mut sections = Vec::new();
    sections
        .try_reserve_exact(1)
        .map_err(|_| layout_allocation_failed())?;
    sections.push(header);
    render_box_sections(interface.id.clone(), sections, options, charset, resources)
}

fn render_note_box<'a>(
    note: &'a ClassNote,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    let label = deferred_text.try_register_present_label_lines_with_authored_disclosure(
        &note.text,
        "authored(bytes=",
        options.terminal_width_profile,
        resources,
    )?;
    let capacity = label
        .len()
        .checked_add(1)
        .ok_or_else(|| work_overflow(resources))?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(capacity)
        .map_err(|_| layout_allocation_failed())?;
    lines.push(deferred_text.try_register(
        ComposedTextPlan::try_new(resources, 1, |push| push("note"))?,
        options.terminal_width_profile,
        resources,
    )?);
    lines.extend(label);

    let mut sections = Vec::new();
    sections
        .try_reserve_exact(1)
        .map_err(|_| layout_allocation_failed())?;
    sections.push(lines);
    render_box_sections(note.id.clone(), sections, options, charset, resources)
}

fn render_box_sections(
    id: String,
    sections: Vec<Vec<DeferredTextLine>>,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    relation_graph::RelationGraphBox::from_deferred_sections(
        id,
        &sections,
        options.box_border_padding,
        class_box_style(charset),
        options.terminal_width_profile,
        resources,
    )
}

fn class_box_style(charset: ClassCharset) -> RelationGraphBoxStyle {
    RelationGraphBoxStyle {
        top_left: charset.top_left,
        top_right: charset.top_right,
        bottom_left: charset.bottom_left,
        bottom_right: charset.bottom_right,
        horizontal: charset.horizontal,
        vertical: charset.vertical,
        separator_left: charset.separator_left,
        separator_right: charset.separator_right,
        border_role: AsciiColorRole::NodeBorder,
        text_role: AsciiColorRole::Text,
    }
}

fn class_sections<'a>(
    class: &'a ClassNode,
    width_profile: TerminalWidthProfile,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &ResourceContext,
) -> Result<Vec<Vec<DeferredTextLine>>> {
    let display_plan = ComposedTextPlan::try_new_html_decoded(&class.text, resources)?;
    let disclose_display =
        authored_display_projection_is_lossy(&display_plan, &class.text, resources)?;
    let disclose_identity =
        class.text != class.id || terminal_text_requires_normalization(&class.id, resources)?;
    let header_capacity = class
        .annotations
        .len()
        .checked_add(1)
        .and_then(|capacity| capacity.checked_add(usize::from(disclose_display)))
        .and_then(|capacity| capacity.checked_add(usize::from(disclose_identity)))
        .ok_or_else(|| work_overflow(resources))?;
    let mut header = Vec::new();
    header
        .try_reserve_exact(header_capacity)
        .map_err(|_| layout_allocation_failed())?;
    for annotation in &class.annotations {
        let plan = ComposedTextPlan::try_new(resources, 1, |push| {
            push("<<")?;
            push(annotation)?;
            push(">>")
        })?;
        header.push(deferred_text.try_register(plan, width_profile, resources)?);
    }
    header.push(deferred_text.try_register(display_plan, width_profile, resources)?);
    if disclose_display {
        header.push(deferred_text.try_register_framed_value(
            "text(bytes=",
            &class.text,
            width_profile,
            resources,
        )?);
    }
    if disclose_identity {
        header.push(deferred_text.try_register_framed_value(
            "id(bytes=",
            &class.id,
            width_profile,
            resources,
        )?);
    }

    let mut sections = Vec::new();
    sections
        .try_reserve_exact(3)
        .map_err(|_| layout_allocation_failed())?;
    sections.push(header);

    let mut members = Vec::new();
    members
        .try_reserve_exact(class.members.len())
        .map_err(|_| layout_allocation_failed())?;
    for member in &class.members {
        let line = deferred_text.try_register(
            ClassMemberTextPlan::try_new(member, resources)?.into_text(),
            width_profile,
            resources,
        )?;
        if line.width() > 0 {
            members.push(line);
        }
    }
    if !members.is_empty() {
        sections.push(members);
    }

    let mut methods = Vec::new();
    methods
        .try_reserve_exact(class.methods.len())
        .map_err(|_| layout_allocation_failed())?;
    for method in &class.methods {
        let line = deferred_text.try_register(
            ClassMemberTextPlan::try_new(method, resources)?.into_text(),
            width_profile,
            resources,
        )?;
        if line.width() > 0 {
            methods.push(line);
        }
    }
    if !methods.is_empty() {
        sections.push(methods);
    }

    Ok(sections)
}

fn authored_display_projection_is_lossy(
    display_plan: &ComposedTextPlan<'_>,
    authored: &str,
    resources: &ResourceContext,
) -> Result<bool> {
    Ok(display_plan.source_differs_from(authored, resources)?
        || terminal_single_line_text_requires_normalization(authored, resources)?)
}

#[cfg(test)]
fn member_text_with_probe(
    member: &ClassMember,
    resources: &ResourceContext,
    before_materialize: impl FnOnce(),
) -> Result<String> {
    resources.transaction(|resources| {
        let plan = ClassMemberTextPlan::try_new(member, resources)?;
        plan.materialize(resources, before_materialize)
    })
}

#[derive(Debug)]
struct ClassMemberTextPlan<'a> {
    text: ComposedTextPlan<'a>,
}

impl<'a> ClassMemberTextPlan<'a> {
    fn try_new(member: &'a ClassMember, resources: &ResourceContext) -> Result<Self> {
        if !member.display_text.is_empty() {
            let producer_work_per_pass = 1 + usize::from(!member.classifier.is_empty());
            let text = ComposedTextPlan::try_new(resources, producer_work_per_pass, |push| {
                push(&member.display_text)?;
                if !member.classifier.is_empty() {
                    push(&member.classifier)?;
                }
                Ok(())
            })?;
            return Ok(Self { text });
        }

        let callable = member.member_type == "method"
            || !member.parameters.is_empty()
            || !member.return_type.is_empty();

        let id = plan_canonical_generic_types(&member.id, resources)?;
        let parameters = if callable {
            resources.charge_layout_work(member.parameters.len().max(1))?;
            Some(plan_canonical_generic_types(
                grapheme_safe_trim(&member.parameters),
                resources,
            )?)
        } else {
            None
        };
        let return_type = if callable && !member.return_type.is_empty() {
            resources.charge_layout_work(member.return_type.len().max(1))?;
            Some(plan_canonical_generic_types(
                grapheme_safe_trim(&member.return_type),
                resources,
            )?)
        } else {
            None
        };

        let canonical_scan_work = generic_materialization_scan_work(resources, id)?;
        let canonical_scan_work = parameters.map_or(Ok(canonical_scan_work), |plan| {
            resources.checked_work_add(
                canonical_scan_work,
                generic_materialization_scan_work(resources, plan)?,
            )
        })?;
        let canonical_scan_work = return_type.map_or(Ok(canonical_scan_work), |plan| {
            resources.checked_work_add(
                canonical_scan_work,
                generic_materialization_scan_work(resources, plan)?,
            )
        })?;
        let text = ComposedTextPlan::try_new(resources, canonical_scan_work, |push| {
            push(&member.visibility)?;
            id.try_visit(&mut *push)?;
            if let Some(parameters) = parameters {
                push("(")?;
                parameters.try_visit(&mut *push)?;
                push(")")?;
            }
            if let Some(return_type) = return_type {
                push(" : ")?;
                return_type.try_visit(&mut *push)?;
            }
            push(&member.classifier)
        })?;

        Ok(Self { text })
    }

    #[cfg(test)]
    fn materialize(
        self,
        resources: &ResourceContext,
        before_materialize: impl FnOnce(),
    ) -> Result<String> {
        self.text.materialize(resources, before_materialize)
    }

    fn into_text(self) -> ComposedTextPlan<'a> {
        self.text
    }
}

fn plan_canonical_generic_types<'a>(
    value: &'a str,
    resources: &ResourceContext,
) -> Result<GenericTypesPlan<'a>> {
    // `GenericTypesPlan::new` performs a containment scan, one delimiter/counting pass, and at
    // most one canonical-output pass. Debit the full conservative bound before any of them run.
    let planning_work = resources.checked_work_mul(value.len().max(1), 3)?;
    resources.charge_layout_work(planning_work)?;
    Ok(GenericTypesPlan::new(value))
}

fn generic_materialization_scan_work(
    resources: &ResourceContext,
    plan: GenericTypesPlan<'_>,
) -> Result<usize> {
    plan.materialization_scan_work()
        .ok_or_else(|| resources.work_overflow())
}

fn relation_layout<'a>(
    model: &'a ClassDiagram,
    relation: &'a ClassRelation,
    endpoint_index: &ClassEndpointIndex<'a>,
    labels: ClassRelationLabels,
) -> Result<RelationLayout<'a>> {
    let left_endpoint =
        endpoint_index
            .resolve(relation.id1.as_str())
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "relationships with missing endpoint classes",
            })?;
    let right_endpoint =
        endpoint_index
            .resolve(relation.id2.as_str())
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "relationships with missing endpoint classes",
            })?;
    let line = if relation.relation.line_type == model.constants.line_type.line {
        RelationLine::Solid
    } else if relation.relation.line_type == model.constants.line_type.dotted_line {
        RelationLine::Dotted
    } else {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "unknown class relationship line types",
        });
    };

    let left_marker = marker_for_relation_type(model, relation.relation.type1)?;
    let right_marker = marker_for_relation_type(model, relation.relation.type2)?;
    let none = model.constants.relation_type.none;
    if left_marker.is_none() && relation.relation.type1 != none
        || right_marker.is_none() && relation.relation.type2 != none
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "unknown class relationship marker types",
        });
    }

    let ClassRelationLabels {
        label,
        left_endpoint: left_endpoint_label,
        right_endpoint: right_endpoint_label,
    } = labels;

    if left_marker == Some(RelationMarker::Extension) && right_marker.is_none() {
        return Ok(RelationLayout {
            top: left_endpoint.into_layout_endpoint(),
            bottom: right_endpoint.into_layout_endpoint(),
            top_marker: left_marker,
            bottom_marker: None,
            line,
            label,
            top_endpoint_label: left_endpoint_label,
            bottom_endpoint_label: right_endpoint_label,
            top_endpoint_role: EndpointLabelRole::First,
            bottom_endpoint_role: EndpointLabelRole::Second,
        });
    }

    if right_marker == Some(RelationMarker::Extension) && left_marker.is_none() {
        return Ok(RelationLayout {
            top: right_endpoint.into_layout_endpoint(),
            bottom: left_endpoint.into_layout_endpoint(),
            top_marker: right_marker,
            bottom_marker: None,
            line,
            label,
            top_endpoint_label: right_endpoint_label,
            bottom_endpoint_label: left_endpoint_label,
            top_endpoint_role: EndpointLabelRole::Second,
            bottom_endpoint_role: EndpointLabelRole::First,
        });
    }

    Ok(RelationLayout {
        top: left_endpoint.into_layout_endpoint(),
        bottom: right_endpoint.into_layout_endpoint(),
        top_marker: left_marker,
        bottom_marker: right_marker,
        line,
        label,
        top_endpoint_label: left_endpoint_label,
        bottom_endpoint_label: right_endpoint_label,
        top_endpoint_role: EndpointLabelRole::First,
        bottom_endpoint_role: EndpointLabelRole::Second,
    })
}

fn prepare_class_relation_labels<'a>(
    model: &'a ClassDiagram,
    options: &AsciiRenderOptions,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &ResourceContext,
) -> Result<Vec<ClassRelationLabels>> {
    resources.transaction(|resources| {
        let capacity = model
            .relations
            .len()
            .checked_mul(3)
            .ok_or_else(|| work_overflow(resources))?;
        let mut plans = Vec::new();
        plans
            .try_reserve_exact(capacity)
            .map_err(|_| layout_allocation_failed())?;
        for relation in &model.relations {
            plans.push(RelationGraphLabelPlan::try_new(
                &relation.title,
                options.terminal_width_profile,
                deferred_text,
                resources,
            )?);
            plans.push(
                relation
                    .relation_title_1
                    .as_deref()
                    .map(|label| {
                        RelationGraphLabelPlan::try_new_present(
                            label,
                            options.terminal_width_profile,
                            deferred_text,
                            resources,
                        )
                    })
                    .transpose()?,
            );
            plans.push(
                relation
                    .relation_title_2
                    .as_deref()
                    .map(|label| {
                        RelationGraphLabelPlan::try_new_present(
                            label,
                            options.terminal_width_profile,
                            deferred_text,
                            resources,
                        )
                    })
                    .transpose()?,
            );
        }
        let labels = RelationGraphLabelBatchPlan::try_new(plans, resources)?.materialize(
            options.color_mode == crate::color::AsciiColorMode::Html,
            deferred_text,
            resources,
        )?;
        let mut labels = labels.into_iter();
        let mut prepared = Vec::new();
        prepared
            .try_reserve_exact(model.relations.len())
            .map_err(|_| layout_allocation_failed())?;
        for _ in &model.relations {
            prepared.push(ClassRelationLabels {
                label: labels.next().ok_or_else(layout_allocation_failed)?,
                left_endpoint: labels.next().ok_or_else(layout_allocation_failed)?,
                right_endpoint: labels.next().ok_or_else(layout_allocation_failed)?,
            });
        }
        if labels.next().is_some() {
            return Err(layout_allocation_failed());
        }
        Ok(prepared)
    })
}

fn note_explicit_namespace_id<'a>(
    model: &'a ClassDiagram,
    note: &'a ClassNote,
    endpoint_index: &ClassEndpointIndex<'a>,
) -> Option<&'a str> {
    let note_parent = note.parent.as_deref()?;
    let note_namespace = model
        .namespaces
        .get(note_parent)
        .filter(|namespace| namespace.explicit)?;
    let target_id = note.class_id.as_deref()?;
    let target_parent = endpoint_index.resolve(target_id)?.owner?;
    model
        .namespaces
        .get(target_parent)
        .filter(|namespace| namespace.explicit)?;
    (note_namespace.id == target_parent).then_some(note_namespace.id.as_str())
}

fn marker_for_relation_type(
    model: &ClassDiagram,
    relation_type: i32,
) -> Result<Option<RelationMarker>> {
    let constants = &model.constants.relation_type;
    if relation_type == constants.none {
        return Ok(None);
    }
    if relation_type == constants.extension {
        return Ok(Some(RelationMarker::Extension));
    }
    if relation_type == constants.dependency {
        return Ok(Some(RelationMarker::Dependency));
    }
    if relation_type == constants.aggregation {
        return Ok(Some(RelationMarker::Aggregation));
    }
    if relation_type == constants.composition {
        return Ok(Some(RelationMarker::Composition));
    }
    if relation_type == constants.lollipop {
        return Ok(Some(RelationMarker::Lollipop));
    }

    Err(AsciiError::UnsupportedFeature {
        diagram_type: "class",
        feature: "class relationship types other than extension, dependency, aggregation, composition, or lollipop",
    })
}

fn note_relation_layouts<'a>(
    model: &'a ClassDiagram,
    endpoint_index: &ClassEndpointIndex<'a>,
    box_by_id: &RenderedClassBoxIndex<'_>,
    width_profile: TerminalWidthProfile,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationLayout<'a>>> {
    note_relation_layouts_for_notes(
        model.notes.iter(),
        endpoint_index,
        box_by_id,
        width_profile,
        deferred_text,
        resources,
    )
}

fn note_relation_layouts_for_notes<'a>(
    notes: impl Iterator<Item = &'a ClassNote>,
    endpoint_index: &ClassEndpointIndex<'a>,
    box_by_id: &RenderedClassBoxIndex<'_>,
    width_profile: TerminalWidthProfile,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationLayout<'a>>> {
    let notes = notes;
    let mut layouts = Vec::new();
    layouts
        .try_reserve(notes.size_hint().0)
        .map_err(|_| layout_allocation_failed())?;
    for note in notes {
        let Some(target_id) = note.class_id.as_deref() else {
            continue;
        };
        let Some(target) = endpoint_index.resolve(target_id) else {
            continue;
        };
        if box_by_id.get(target.resolved_id, resources)?.is_some() {
            let bottom_endpoint_label = join_endpoint_label_with_authored_ref(
                target.into_layout_endpoint().disclosure_id(),
                None,
                width_profile,
                deferred_text,
                resources,
            )?;
            layouts.push(RelationLayout {
                top: RelationEndpoint {
                    authored_id: note.id.as_str(),
                    resolved_id: note.id.as_str(),
                    render_id: note.id.as_str(),
                    owner: note.parent.as_deref(),
                },
                bottom: target.into_layout_endpoint(),
                top_marker: None,
                bottom_marker: None,
                line: RelationLine::Dotted,
                label: None,
                top_endpoint_label: None,
                bottom_endpoint_label,
                top_endpoint_role: EndpointLabelRole::First,
                bottom_endpoint_role: EndpointLabelRole::Second,
            });
        }
    }
    Ok(layouts)
}

fn external_namespace_note_summary_rows<'a>(
    model: &'a ClassDiagram,
    endpoint_index: &ClassEndpointIndex<'a>,
    width_profile: TerminalWidthProfile,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphSummaryRow>> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(model.notes.len())
        .map_err(|_| layout_allocation_failed())?;
    for (declaration_index, note) in model
        .notes
        .iter()
        .enumerate()
        .filter(|(_, note)| note_explicit_namespace_id(model, note, endpoint_index).is_none())
    {
        let Some(target_id) = note.class_id.as_deref() else {
            continue;
        };
        let Some(target_endpoint) = endpoint_index.resolve(target_id) else {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "notes with missing target classes",
            });
        };
        let declaration_ordinal = declaration_index
            .checked_add(1)
            .ok_or_else(|| work_overflow(resources))?;
        let source = deferred_text.try_register_parts(width_profile, resources, 7, |push| {
            push(DeferredTextPart::Static("note(index="))?;
            push(DeferredTextPart::Decimal(declaration_ordinal))?;
            push(DeferredTextPart::Static(", text(bytes="))?;
            push(DeferredTextPart::Decimal(note.text.len()))?;
            push(DeferredTextPart::Static(")="))?;
            push(DeferredTextPart::QuotedText(&note.text))?;
            push(DeferredTextPart::Static(")"))
        })?;
        let connector = deferred_text.try_register(
            ComposedTextPlan::try_new(resources, 1, |push| push(".."))?,
            width_profile,
            resources,
        )?;
        let target = if target_endpoint.authored_id == target_endpoint.resolved_id {
            deferred_text.try_register_framed_value(
                "id(bytes=",
                target_endpoint.resolved_id,
                width_profile,
                resources,
            )?
        } else {
            deferred_text.try_register_parts(width_profile, resources, 8, |push| {
                push(DeferredTextPart::Static("id(bytes="))?;
                push(DeferredTextPart::Decimal(target_endpoint.resolved_id.len()))?;
                push(DeferredTextPart::Static(")="))?;
                push(DeferredTextPart::QuotedText(target_endpoint.resolved_id))?;
                push(DeferredTextPart::Static(" targetRef(bytes="))?;
                push(DeferredTextPart::Decimal(target_endpoint.authored_id.len()))?;
                push(DeferredTextPart::Static(")="))?;
                push(DeferredTextPart::QuotedText(target_endpoint.authored_id))
            })?
        };
        rows.push(RelationGraphSummaryRow::new(
            source,
            connector,
            target,
            std::rc::Rc::new(Vec::new()),
        ));
    }
    Ok(rows)
}

fn plan_vertical_relation<'plan>(
    top: &'plan RenderedClassBox,
    bottom: &'plan RenderedClassBox,
    layout: &'plan RelationLayout<'_>,
    charset: ClassCharset,
    resources: &mut ResourceContext,
) -> Result<RelationRegionPlan<'plan>> {
    let label_half_widths = [
        layout
            .label
            .as_ref()
            .map(RelationGraphLabel::half_width)
            .unwrap_or(0),
        layout
            .top_endpoint_label
            .as_ref()
            .map(RelationGraphLabel::half_width)
            .unwrap_or(0),
        layout
            .bottom_endpoint_label
            .as_ref()
            .map(RelationGraphLabel::half_width)
            .unwrap_or(0),
    ];
    let plan = RelationStackPlan::try_new(
        top,
        bottom,
        &label_half_widths,
        resources,
        |center, resources| {
            class_relation_extent(layout, center, charset, top.width_profile(), resources)
        },
    )?;

    Ok(RelationRegionPlan::Vertical {
        plan,
        rows: Box::new(move |center, resources| {
            class_relation_rows(layout, center, charset, top.width_profile(), resources)
        }),
    })
}

fn class_relation_extent(
    layout: &RelationLayout<'_>,
    center: usize,
    charset: ClassCharset,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let top_endpoint = layout
        .top_endpoint_label
        .as_ref()
        .map(|label| (label.width(), label.line_count()))
        .unwrap_or((0, 0));
    let relation_label = layout
        .label
        .as_ref()
        .map(|label| (label.width(), label.line_count()))
        .unwrap_or((0, 0));
    let bottom_endpoint = layout
        .bottom_endpoint_label
        .as_ref()
        .map(|label| (label.width(), label.line_count()))
        .unwrap_or((0, 0));
    let (marker_width, marker_count) = class_relation_marker_block(layout, charset, width_profile);
    let centered = relation_graph::centered_row_blocks_extent(
        center,
        [top_endpoint, relation_label, bottom_endpoint],
        resources,
    )?;
    let width = centered
        .width()
        .max(resources.checked_grid_add(center, marker_width)?);
    let height = resources.checked_grid_add(centered.height(), marker_count)?;
    resources.grid_extent(width, height)
}

fn class_relation_marker_block(
    layout: &RelationLayout<'_>,
    charset: ClassCharset,
    width_profile: TerminalWidthProfile,
) -> (usize, usize) {
    let line_width = relation_char_width(line_char(layout.line, charset), width_profile);
    let top_marker_width = layout
        .top_marker
        .map(|marker| {
            relation_char_width(marker_char(marker, MarkerSide::Top, charset), width_profile)
        })
        .unwrap_or(line_width);
    let bottom_marker_width = layout
        .bottom_marker
        .map(|marker| {
            relation_char_width(
                marker_char(marker, MarkerSide::Bottom, charset),
                width_profile,
            )
        })
        .unwrap_or(line_width);
    if layout.top_marker.is_none() && layout.bottom_marker.is_none() && layout.label.is_none() {
        (line_width, 1)
    } else {
        (top_marker_width.max(bottom_marker_width), 2)
    }
}

fn push_centered_endpoint_label(
    relation_lines: &mut Vec<RelationGraphLine>,
    label: Option<&RelationGraphLabel>,
    center: usize,
    resources: &ResourceContext,
) -> Result<()> {
    if let Some(label) = label {
        relation_lines.extend(relation_graph::centered_label_lines_with_role(
            label,
            center,
            AsciiColorRole::EdgeLabel,
            resources,
        )?);
    }
    Ok(())
}

fn class_relation_rows(
    layout: &RelationLayout<'_>,
    center: usize,
    charset: ClassCharset,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let mut relation_lines = Vec::new();
    push_centered_endpoint_label(
        &mut relation_lines,
        layout.top_endpoint_label.as_ref(),
        center,
        resources,
    )?;
    match (layout.top_marker, layout.bottom_marker) {
        (None, None) => {
            relation_lines.push(relation_graph::marker_line_with_role(
                line_char(layout.line, charset),
                center,
                AsciiColorRole::EdgeLine,
                width_profile,
                resources,
            )?);
            if let Some(label) = layout.label.as_ref() {
                relation_lines.extend(relation_graph::centered_label_lines_with_role(
                    label,
                    center,
                    AsciiColorRole::EdgeLabel,
                    resources,
                )?);
                relation_lines.push(relation_graph::marker_line_with_role(
                    line_char(layout.line, charset),
                    center,
                    AsciiColorRole::EdgeLine,
                    width_profile,
                    resources,
                )?);
            }
        }
        (top_marker, bottom_marker) => {
            relation_lines.push(relation_graph::marker_line_with_role(
                top_marker
                    .map(|marker| marker_char(marker, MarkerSide::Top, charset))
                    .unwrap_or_else(|| line_char(layout.line, charset)),
                center,
                if top_marker.is_some() {
                    AsciiColorRole::EdgeArrow
                } else {
                    AsciiColorRole::EdgeLine
                },
                width_profile,
                resources,
            )?);
            if let Some(label) = layout.label.as_ref() {
                relation_lines.extend(relation_graph::centered_label_lines_with_role(
                    label,
                    center,
                    AsciiColorRole::EdgeLabel,
                    resources,
                )?);
            }
            relation_lines.push(relation_graph::marker_line_with_role(
                bottom_marker
                    .map(|marker| marker_char(marker, MarkerSide::Bottom, charset))
                    .unwrap_or_else(|| line_char(layout.line, charset)),
                center,
                if bottom_marker.is_some() {
                    AsciiColorRole::EdgeArrow
                } else {
                    AsciiColorRole::EdgeLine
                },
                width_profile,
                resources,
            )?);
        }
    }
    push_centered_endpoint_label(
        &mut relation_lines,
        layout.bottom_endpoint_label.as_ref(),
        center,
        resources,
    )?;
    Ok(relation_lines)
}

fn plan_parallel_vertical_relations<'plan, 'text>(
    boxes: Vec<&'plan RenderedClassBox>,
    layouts: Vec<&'plan RelationLayout<'text>>,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
    deferred_text: &mut DeferredTextRegistry<'text>,
) -> Result<RelationRegionPlan<'plan>> {
    let first = layouts
        .first()
        .copied()
        .ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "empty parallel class relationship layout",
        })?;
    let top = relation_graph::find_box_ref(&boxes, first.top.render_id).ok_or(
        AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "relationships with missing endpoint classes",
        },
    )?;
    let bottom = relation_graph::find_box_ref(&boxes, first.bottom.render_id).ok_or(
        AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "relationships with missing endpoint classes",
        },
    )?;
    let reserve_top_endpoint_label = layouts
        .iter()
        .any(|layout| layout.top_endpoint_label.is_some());
    let reserve_bottom_endpoint_label = layouts
        .iter()
        .any(|layout| layout.bottom_endpoint_label.is_some());
    let mut lane_extents = Vec::new();
    lane_extents
        .try_reserve_exact(layouts.len())
        .map_err(|_| layout_allocation_failed())?;
    for layout in &layouts {
        lane_extents.push(parallel_class_lane_extent(
            layout,
            charset,
            reserve_top_endpoint_label,
            reserve_bottom_endpoint_label,
            width_profile,
            resources,
        )?);
    }
    let plan = RelationParallelPlan::new(top, bottom, lane_extents, 2, resources)?;

    if !plan.ports_fit(resources)? {
        let reason = relation_graph::LayeredRelationSummaryReason::RouteCollision;
        let mut rows = Vec::new();
        rows.try_reserve_exact(layouts.len())
            .map_err(|_| layout_allocation_failed())?;
        for layout in layouts {
            rows.push(class_relation_summary_row(
                layout,
                resources,
                width_profile,
                deferred_text,
            )?);
        }
        return Ok(RelationRegionPlan::Summary(
            RelationSummaryPaintPlan::stacked(boxes, rows, Some(reason), options, resources)?,
        ));
    }

    Ok(RelationRegionPlan::Parallel {
        plan,
        lanes: Box::new(move |resources| {
            let mut lanes = Vec::new();
            lanes
                .try_reserve_exact(layouts.len())
                .map_err(|_| layout_allocation_failed())?;
            for layout in layouts {
                lanes.push(parallel_class_lane_rows(
                    layout,
                    charset,
                    reserve_top_endpoint_label,
                    reserve_bottom_endpoint_label,
                    width_profile,
                    resources,
                )?);
            }
            Ok(lanes)
        }),
    })
}

fn parallel_class_lane_extent(
    layout: &RelationLayout<'_>,
    charset: ClassCharset,
    reserve_top_endpoint_label: bool,
    reserve_bottom_endpoint_label: bool,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<LogicalExtent> {
    let top_label_height = layout
        .top_endpoint_label
        .as_ref()
        .map(RelationGraphLabel::line_count)
        .unwrap_or(usize::from(reserve_top_endpoint_label));
    let central_label_height = layout
        .label
        .as_ref()
        .map(RelationGraphLabel::line_count)
        .unwrap_or(1);
    let bottom_label_height = layout
        .bottom_endpoint_label
        .as_ref()
        .map(RelationGraphLabel::line_count)
        .unwrap_or(usize::from(reserve_bottom_endpoint_label));
    let height = resources.checked_grid_add(top_label_height, central_label_height)?;
    let height = resources.checked_grid_add(height, 2)?;
    let height = resources.checked_grid_add(height, bottom_label_height)?;

    let line_width = relation_char_width(line_char(layout.line, charset), width_profile);
    let top_marker_width = layout
        .top_marker
        .map(|marker| {
            relation_char_width(marker_char(marker, MarkerSide::Top, charset), width_profile)
        })
        .unwrap_or(0);
    let bottom_marker_width = layout
        .bottom_marker
        .map(|marker| {
            relation_char_width(
                marker_char(marker, MarkerSide::Bottom, charset),
                width_profile,
            )
        })
        .unwrap_or(0);
    let width = [
        line_width,
        top_marker_width,
        bottom_marker_width,
        layout
            .top_endpoint_label
            .as_ref()
            .map(RelationGraphLabel::width)
            .unwrap_or(0),
        layout
            .label
            .as_ref()
            .map(RelationGraphLabel::width)
            .unwrap_or(0),
        layout
            .bottom_endpoint_label
            .as_ref()
            .map(RelationGraphLabel::width)
            .unwrap_or(0),
    ]
    .into_iter()
    .max()
    .unwrap_or(1)
    .max(1);
    resources.grid_extent(width, height)
}

fn relation_char_width(ch: char, width_profile: TerminalWidthProfile) -> usize {
    let mut encoded = [0; 4];
    display_width_with_profile(ch.encode_utf8(&mut encoded), width_profile)
}

fn endpoint_label_lines_or_empty(
    label: Option<&RelationGraphLabel>,
    reserve_empty: bool,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    match label {
        Some(label) => {
            relation_graph::label_lines_with_role(label, AsciiColorRole::EdgeLabel, resources)
        }
        None if reserve_empty => Ok(vec![RelationGraphLine::try_with_role(
            "",
            AsciiColorRole::EdgeLabel,
            width_profile,
            resources,
        )?]),
        None => Ok(Vec::new()),
    }
}

fn central_label_lines_or_empty(
    label: Option<&RelationGraphLabel>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    match label {
        Some(label) => {
            relation_graph::label_lines_with_role(label, AsciiColorRole::EdgeLabel, resources)
        }
        None => Ok(vec![RelationGraphLine::try_with_role(
            "",
            AsciiColorRole::EdgeLabel,
            width_profile,
            resources,
        )?]),
    }
}

fn parallel_class_lane_rows(
    layout: &RelationLayout<'_>,
    charset: ClassCharset,
    reserve_top_endpoint_label: bool,
    reserve_bottom_endpoint_label: bool,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let line_text = line_char(layout.line, charset).to_string();
    let line = RelationGraphLine::try_with_role(
        &line_text,
        AsciiColorRole::EdgeLine,
        width_profile,
        resources,
    )?;
    let mut rows = endpoint_label_lines_or_empty(
        layout.top_endpoint_label.as_ref(),
        reserve_top_endpoint_label,
        width_profile,
        resources,
    )?;
    let label_lines =
        central_label_lines_or_empty(layout.label.as_ref(), width_profile, resources)?;
    let relation_rows = match (layout.top_marker, layout.bottom_marker) {
        (None, None) => {
            let mut rows = vec![line.clone()];
            rows.extend(label_lines);
            rows.push(line);
            rows
        }
        (top_marker, bottom_marker) => {
            let top = match top_marker {
                Some(marker) => RelationGraphLine::try_with_role(
                    &marker_char(marker, MarkerSide::Top, charset).to_string(),
                    AsciiColorRole::EdgeArrow,
                    width_profile,
                    resources,
                )?,
                None => line.clone(),
            };
            let bottom = match bottom_marker {
                Some(marker) => RelationGraphLine::try_with_role(
                    &marker_char(marker, MarkerSide::Bottom, charset).to_string(),
                    AsciiColorRole::EdgeArrow,
                    width_profile,
                    resources,
                )?,
                None => line,
            };
            let mut rows = vec![top];
            rows.extend(label_lines);
            rows.push(bottom);
            rows
        }
    };
    rows.extend(relation_rows);
    rows.extend(endpoint_label_lines_or_empty(
        layout.bottom_endpoint_label.as_ref(),
        reserve_bottom_endpoint_label,
        width_profile,
        resources,
    )?);
    Ok(rows)
}

fn class_relation_summary_row<'a>(
    layout: &RelationLayout<'a>,
    resources: &ResourceContext,
    width_profile: TerminalWidthProfile,
    deferred_text: &mut DeferredTextRegistry<'a>,
) -> Result<RelationGraphSummaryRow> {
    let source = deferred_text.try_register_framed_value(
        "id(bytes=",
        layout.top.render_id,
        width_profile,
        resources,
    )?;
    let connector =
        class_relation_summary_connector(layout, width_profile, resources, deferred_text)?;
    let target = deferred_text.try_register_framed_value(
        "id(bytes=",
        layout.bottom.render_id,
        width_profile,
        resources,
    )?;
    let label = layout
        .label
        .as_ref()
        .map(RelationGraphLabel::shared_lines)
        .unwrap_or_else(|| std::rc::Rc::new(Vec::new()));
    Ok(RelationGraphSummaryRow::new(
        source, connector, target, label,
    ))
}

fn class_relation_summary_row_for_reason<'a>(
    layout: &RelationLayout<'a>,
    reason: relation_graph::LayeredRelationSummaryReason,
    resources: &ResourceContext,
    width_profile: TerminalWidthProfile,
    deferred_text: &mut DeferredTextRegistry<'a>,
) -> Result<RelationGraphSummaryRow> {
    match reason {
        relation_graph::LayeredRelationSummaryReason::Crossing
        | relation_graph::LayeredRelationSummaryReason::RouteCollision
        | relation_graph::LayeredRelationSummaryReason::OverlayCollision => {
            class_relation_summary_row(layout, resources, width_profile, deferred_text)
        }
    }
}

fn class_relation_summary_connector(
    layout: &RelationLayout<'_>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    deferred_text: &mut DeferredTextRegistry<'_>,
) -> Result<DeferredTextLine> {
    let top_lines = layout
        .top_endpoint_label
        .as_ref()
        .map(RelationGraphLabel::shared_lines);
    let bottom_lines = layout
        .bottom_endpoint_label
        .as_ref()
        .map(RelationGraphLabel::shared_lines);
    let producer_work = top_lines.as_ref().map_or(0, |lines| lines.len())
        + bottom_lines.as_ref().map_or(0, |lines| lines.len())
        + 5;
    deferred_text.try_register_parts(width_profile, resources, producer_work, |push| {
        if let Some(lines) = top_lines.as_ref() {
            push(DeferredTextPart::Static(
                layout.top_endpoint_role.summary_open(false),
            ))?;
            visit_endpoint_label_summary_parts(lines, push)?;
            push(DeferredTextPart::Static("] "))?;
        }

        if let Some(marker) = layout.top_marker {
            push(DeferredTextPart::Static(match marker {
                RelationMarker::Extension => "<|",
                RelationMarker::Dependency => "<",
                RelationMarker::Aggregation => "o",
                RelationMarker::Composition => "*",
                RelationMarker::Lollipop => "()",
            }))?;
        }
        push(DeferredTextPart::Static(match layout.line {
            RelationLine::Solid => "--",
            RelationLine::Dotted => "..",
        }))?;
        if let Some(marker) = layout.bottom_marker {
            push(DeferredTextPart::Static(match marker {
                RelationMarker::Extension => "|>",
                RelationMarker::Dependency => ">",
                RelationMarker::Aggregation => "o",
                RelationMarker::Composition => "*",
                RelationMarker::Lollipop => "()",
            }))?;
        }

        if let Some(lines) = bottom_lines.as_ref() {
            push(DeferredTextPart::Static(
                layout.bottom_endpoint_role.summary_open(true),
            ))?;
            visit_endpoint_label_summary_parts(lines, push)?;
            push(DeferredTextPart::Static("]"))?;
        }
        Ok(())
    })
}

fn visit_endpoint_label_summary_parts<'line, 'text>(
    lines: &'line std::rc::Rc<Vec<DeferredTextLine>>,
    push: &mut dyn FnMut(DeferredTextPart<'line, 'text>) -> Result<()>,
) -> Result<()> {
    for (line_index, line) in lines.iter().enumerate() {
        if line_index > 0 {
            push(DeferredTextPart::Static(", "))?;
        }
        push(DeferredTextPart::Static("bytes="))?;
        push(DeferredTextPart::Decimal(line.plain_bytes()))?;
        push(DeferredTextPart::Static(" "))?;
        push(DeferredTextPart::QuotedLine(line))?;
    }
    Ok(())
}

fn class_layered_edge(layout: &RelationLayout<'_>) -> LayeredRelationEdge {
    let labels = [
        layout.label.as_ref(),
        layout.top_endpoint_label.as_ref(),
        layout.bottom_endpoint_label.as_ref(),
    ];
    LayeredRelationEdge::new(
        layout.top.render_id,
        layout.bottom.render_id,
        labels
            .iter()
            .flatten()
            .map(|label| label.width())
            .max()
            .unwrap_or(0),
        labels
            .iter()
            .flatten()
            .map(|label| label.line_count())
            .max()
            .unwrap_or(0),
    )
}

fn class_layered_error(error: LayeredRelationError) -> AsciiError {
    let feature = match error {
        LayeredRelationError::MissingEndpoint => "relationships with missing endpoint classes",
        LayeredRelationError::UnrelatedBoxes => "class relationship layouts with unrelated classes",
        LayeredRelationError::Crossing => "crossing class relationship layouts",
    };
    AsciiError::UnsupportedFeature {
        diagram_type: "class",
        feature,
    }
}

impl<'relation> relation_graph::RelationComponentAdapter<'relation, RelationLayout<'relation>>
    for ClassRelationComponentAdapter
{
    fn build_edges(&self, layout: &RelationLayout<'relation>) -> LayeredRelationEdge {
        class_layered_edge(layout)
    }

    fn is_self_relation(&self, layout: &RelationLayout<'relation>) -> bool {
        layout.top.render_id == layout.bottom.render_id
    }

    fn self_loop_metrics(
        &self,
        layout: &RelationLayout<'relation>,
        resources: &ResourceContext,
    ) -> Result<RelationSelfLoopMetrics> {
        self_loop_metrics_for_class_layout(layout, self.charset, self.width_profile, resources)
    }

    fn self_loop_rows(
        &self,
        layout: &RelationLayout<'relation>,
        resources: &ResourceContext,
    ) -> Result<relation_graph::RelationSelfLoopRows> {
        self_loop_rows_for_class_layout(layout, self.charset, self.width_profile, resources)
    }

    fn horizontal_relation_style(
        &self,
        layout: &RelationLayout<'relation>,
        source_side: RelationPortSide,
        target_side: RelationPortSide,
        _resources: &ResourceContext,
    ) -> Result<HorizontalRelationStyle> {
        let source_marker = layout.top_marker.map(|marker| {
            HorizontalRelationMarker::new(
                horizontal_class_marker(marker, source_side, self.charset),
                AsciiColorRole::EdgeArrow,
                self.width_profile,
            )
        });
        let target_marker = layout.bottom_marker.map(|marker| {
            HorizontalRelationMarker::new(
                horizontal_class_marker(marker, target_side, self.charset),
                AsciiColorRole::EdgeArrow,
                self.width_profile,
            )
        });
        Ok(HorizontalRelationStyle::new(
            HorizontalRelationEndpoint::new(source_marker, layout.top_endpoint_label.clone()),
            HorizontalRelationEndpoint::new(target_marker, layout.bottom_endpoint_label.clone()),
            layout.label.clone(),
            horizontal_line_char(layout.line, self.charset),
            line_char(layout.line, self.charset),
            relation_line_chars(self.charset),
        ))
    }

    fn layered_horizontal_gap(&self) -> usize {
        CLASS_LEVEL_HORIZONTAL_GAP
    }

    fn layered_route_style(
        &self,
        layout: &RelationLayout<'relation>,
    ) -> Result<LayeredRelationRouteStyle> {
        let vertical = line_char(layout.line, self.charset);
        let horizontal = horizontal_line_char(layout.line, self.charset);
        let relation_chars = relation_line_chars(self.charset);
        Ok(LayeredRelationRouteStyle::new(
            vertical,
            horizontal,
            relation_chars,
            class_route_profile(layout),
        ))
    }

    fn layered_relation_overlays(
        &self,
        layout: &RelationLayout<'relation>,
        geometry: &relation_graph::LayeredRelationRouteGeometry,
        resources: &ResourceContext,
    ) -> Result<Vec<RelationOverlay>> {
        resources.charge_layout_work(3)?;
        let mut overlays = Vec::new();
        overlays
            .try_reserve_exact(4)
            .map_err(|_| layout_allocation_failed())?;
        if let Some(label) = layout.top_endpoint_label.as_ref() {
            let (center_x, y) =
                geometry.endpoint_label_anchor(true, label.line_count(), resources)?;
            overlays.push(RelationOverlay::label(
                center_x,
                y,
                label.clone(),
                AsciiColorRole::EdgeLabel,
            ));
        }
        if let Some(label) = layout.label.as_ref() {
            let (center_x, y) = geometry.relation_label_anchor(label.line_count(), resources)?;
            overlays.push(RelationOverlay::label(
                center_x,
                y,
                label.clone(),
                AsciiColorRole::EdgeLabel,
            ));
        }

        if let Some(marker) = layout.top_marker {
            let port = geometry.source_port();
            overlays.push(RelationOverlay::glyph(
                port.x(),
                port.marker_y(),
                marker_char(
                    marker,
                    marker_side_for_physical_port(port.side())?,
                    self.charset,
                ),
                AsciiColorRole::EdgeArrow,
            ));
        }
        if let Some(marker) = layout.bottom_marker {
            let port = geometry.target_port();
            overlays.push(RelationOverlay::glyph(
                port.x(),
                port.marker_y(),
                marker_char(
                    marker,
                    marker_side_for_physical_port(port.side())?,
                    self.charset,
                ),
                AsciiColorRole::EdgeArrow,
            ));
        }

        if let Some(label) = layout.bottom_endpoint_label.as_ref() {
            let (center_x, y) =
                geometry.endpoint_label_anchor(false, label.line_count(), resources)?;
            overlays.push(RelationOverlay::label(
                center_x,
                y,
                label.clone(),
                AsciiColorRole::EdgeLabel,
            ));
        }

        Ok(overlays)
    }

    fn plan_vertical_region<'plan>(
        &self,
        boxes: &[&'plan RenderedClassBox],
        layout: &'plan RelationLayout<'relation>,
        resources: &mut ResourceContext,
    ) -> Result<RelationRegionPlan<'plan>> {
        let top = relation_graph::find_box_ref(boxes, layout.top.render_id).ok_or(
            AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "relationships with missing endpoint classes",
            },
        )?;
        let bottom = relation_graph::find_box_ref(boxes, layout.bottom.render_id).ok_or(
            AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "relationships with missing endpoint classes",
            },
        )?;
        plan_vertical_relation(top, bottom, layout, self.charset, resources)
    }

    fn plan_parallel_region<'plan>(
        &self,
        boxes: Vec<&'plan RenderedClassBox>,
        layouts: Vec<&'plan RelationLayout<'relation>>,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
        deferred: &mut DeferredTextRegistry<'relation>,
    ) -> Result<RelationRegionPlan<'plan>> {
        plan_parallel_vertical_relations(
            boxes,
            layouts,
            options,
            self.charset,
            self.width_profile,
            resources,
            deferred,
        )
    }

    fn build_summary_row(
        &self,
        layout: &RelationLayout<'relation>,
        reason: relation_graph::LayeredRelationSummaryReason,
        resources: &ResourceContext,
        deferred: &mut DeferredTextRegistry<'relation>,
    ) -> Result<RelationGraphSummaryRow> {
        class_relation_summary_row_for_reason(
            layout,
            reason,
            resources,
            self.width_profile,
            deferred,
        )
    }

    fn layered_error(&self, error: LayeredRelationError) -> AsciiError {
        class_layered_error(error)
    }
}

fn marker_side_for_physical_port(
    side: relation_graph::LayeredRelationPhysicalSide,
) -> Result<MarkerSide> {
    match side {
        relation_graph::LayeredRelationPhysicalSide::Top => Ok(MarkerSide::Bottom),
        relation_graph::LayeredRelationPhysicalSide::Bottom => Ok(MarkerSide::Top),
        relation_graph::LayeredRelationPhysicalSide::Left
        | relation_graph::LayeredRelationPhysicalSide::Right => {
            Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "horizontal ports in layered relationship layouts",
            })
        }
    }
}

fn self_loop_metrics_for_class_layout(
    layout: &RelationLayout<'_>,
    charset: ClassCharset,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<RelationSelfLoopMetrics> {
    let top_marker = layout
        .top_marker
        .map(|marker| marker_char(marker, MarkerSide::Top, charset))
        .unwrap_or('+');
    let bottom_marker = layout
        .bottom_marker
        .map(|marker| marker_char(marker, MarkerSide::Bottom, charset))
        .unwrap_or_else(|| line_char(layout.line, charset));
    let top_marker_width = relation_char_width(top_marker, width_profile);
    let disclose_roles = class_self_loop_discloses_label_roles(layout);
    let mut max_label_width = 0usize;
    for (label, prefix) in [
        (
            layout.top_endpoint_label.as_ref(),
            layout.top_endpoint_role.disclosure_prefix(),
        ),
        (layout.label.as_ref(), "relation: "),
        (
            layout.bottom_endpoint_label.as_ref(),
            layout.bottom_endpoint_role.disclosure_prefix(),
        ),
    ] {
        let Some(label) = label else {
            continue;
        };
        let width = if disclose_roles {
            resources.checked_grid_add(
                display_width_with_profile(prefix, width_profile),
                label.width(),
            )?
        } else {
            label.width()
        };
        max_label_width = max_label_width.max(width);
    }
    Ok(RelationSelfLoopMetrics::new(
        top_marker_width,
        max_label_width,
        class_self_loop_label_line_count(layout, resources)?,
        relation_char_width(bottom_marker, width_profile),
        layout.top_marker.map(|_| top_marker_width),
        horizontal_line_char(layout.line, charset),
        line_char(layout.line, charset),
    ))
}

fn class_self_loop_label_line_count(
    layout: &RelationLayout<'_>,
    resources: &ResourceContext,
) -> Result<usize> {
    layout
        .top_endpoint_label
        .as_ref()
        .map(RelationGraphLabel::line_count)
        .unwrap_or(0)
        .checked_add(
            layout
                .label
                .as_ref()
                .map(RelationGraphLabel::line_count)
                .unwrap_or(0),
        )
        .and_then(|count| {
            count.checked_add(
                layout
                    .bottom_endpoint_label
                    .as_ref()
                    .map(RelationGraphLabel::line_count)
                    .unwrap_or(0),
            )
        })
        .ok_or_else(|| work_overflow(resources))
}

fn self_loop_rows_for_class_layout(
    layout: &RelationLayout<'_>,
    charset: ClassCharset,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<relation_graph::RelationSelfLoopRows> {
    let top_marker_text = layout
        .top_marker
        .map(|marker| marker_char(marker, MarkerSide::Top, charset).to_string())
        .unwrap_or_else(|| "+".to_string());
    let top_marker = RelationGraphLine::try_with_role(
        &top_marker_text,
        if layout.top_marker.is_some() {
            AsciiColorRole::EdgeArrow
        } else {
            AsciiColorRole::EdgeLine
        },
        width_profile,
        resources,
    )?;
    let bottom_marker_text = layout
        .bottom_marker
        .map(|marker| marker_char(marker, MarkerSide::Bottom, charset))
        .unwrap_or_else(|| line_char(layout.line, charset))
        .to_string();
    let bottom_marker = RelationGraphLine::try_with_role(
        &bottom_marker_text,
        if layout.bottom_marker.is_some() {
            AsciiColorRole::EdgeArrow
        } else {
            AsciiColorRole::EdgeLine
        },
        width_profile,
        resources,
    )?;
    let label_line_count = class_self_loop_label_line_count(layout, resources)?;
    let mut label_lines = Vec::new();
    label_lines
        .try_reserve_exact(label_line_count)
        .map_err(|_| layout_allocation_failed())?;
    let disclose_roles = class_self_loop_discloses_label_roles(layout);
    for (label, prefix) in [
        (
            layout.top_endpoint_label.as_ref(),
            layout.top_endpoint_role.disclosure_prefix(),
        ),
        (layout.label.as_ref(), "relation: "),
        (
            layout.bottom_endpoint_label.as_ref(),
            layout.bottom_endpoint_role.disclosure_prefix(),
        ),
    ] {
        let Some(label) = label else {
            continue;
        };
        if !disclose_roles {
            label_lines.extend(relation_graph::label_lines_with_role(
                label,
                AsciiColorRole::EdgeLabel,
                resources,
            )?);
            continue;
        }
        for line in label.lines() {
            let mut disclosed = crate::text::StyledLine::with_resources(width_profile, resources);
            disclosed.try_push_role_text(prefix, AsciiColorRole::EdgeLabel)?;
            disclosed.try_push_deferred_text(line, AsciiColorRole::EdgeLabel)?;
            label_lines.push(RelationGraphLine::from_styled(disclosed));
        }
    }

    let tail_prefix = layout.top_marker.is_some().then(|| top_marker.clone());
    let rows = relation_graph::RelationSelfLoopRows::new(
        top_marker,
        label_lines,
        bottom_marker,
        horizontal_line_char(layout.line, charset),
        line_char(layout.line, charset),
    );
    Ok(match tail_prefix {
        Some(prefix) => rows.with_tail_prefix(prefix),
        None => rows,
    })
}

fn class_self_loop_discloses_label_roles(layout: &RelationLayout<'_>) -> bool {
    layout.top_endpoint_label.is_some() || layout.bottom_endpoint_label.is_some()
}

fn class_route_profile(layout: &RelationLayout<'_>) -> relation_graph::LayeredRelationRouteProfile {
    let endpoint_label_gap = layout
        .top_endpoint_label
        .as_ref()
        .map(RelationGraphLabel::line_count)
        .unwrap_or(0)
        .max(
            layout
                .bottom_endpoint_label
                .as_ref()
                .map(RelationGraphLabel::line_count)
                .unwrap_or(0),
        );

    if endpoint_label_gap > 0 {
        relation_graph::LayeredRelationRouteProfile::class_with_endpoint_labels(endpoint_label_gap)
    } else {
        relation_graph::LayeredRelationRouteProfile::class()
    }
}

fn marker_char(marker: RelationMarker, side: MarkerSide, charset: ClassCharset) -> char {
    match marker {
        RelationMarker::Extension => match side {
            MarkerSide::Top => charset.extension_up,
            MarkerSide::Bottom => charset.extension_down,
        },
        RelationMarker::Dependency => match side {
            MarkerSide::Top => charset.arrow_up,
            MarkerSide::Bottom => charset.arrow_down,
        },
        RelationMarker::Aggregation => charset.aggregation,
        RelationMarker::Composition => charset.composition,
        RelationMarker::Lollipop => charset.lollipop,
    }
}

fn horizontal_class_marker(
    marker: RelationMarker,
    side: RelationPortSide,
    charset: ClassCharset,
) -> String {
    match marker {
        RelationMarker::Extension => match (side, charset.extension_up) {
            (RelationPortSide::Right, '^') => "<|".to_string(),
            (RelationPortSide::Left, '^') => "|>".to_string(),
            (RelationPortSide::Right, _) => "◁".to_string(),
            (RelationPortSide::Left, _) => "▷".to_string(),
        },
        RelationMarker::Dependency => match (side, charset.arrow_up) {
            (RelationPortSide::Right, '^') => "<".to_string(),
            (RelationPortSide::Left, '^') => ">".to_string(),
            (RelationPortSide::Right, _) => "◀".to_string(),
            (RelationPortSide::Left, _) => "▶".to_string(),
        },
        RelationMarker::Aggregation => charset.aggregation.to_string(),
        RelationMarker::Composition => charset.composition.to_string(),
        RelationMarker::Lollipop => charset.lollipop.to_string(),
    }
}

fn horizontal_line_char(line: RelationLine, charset: ClassCharset) -> char {
    match line {
        RelationLine::Solid => charset.solid_horizontal_relation,
        RelationLine::Dotted => charset.dotted_horizontal_relation,
    }
}

fn line_char(line: RelationLine, charset: ClassCharset) -> char {
    match line {
        RelationLine::Solid => charset.solid_vertical_relation,
        RelationLine::Dotted => charset.dotted_vertical_relation,
    }
}

fn relation_line_chars(charset: ClassCharset) -> RelationLineChars {
    RelationLineChars::new(
        [
            charset.solid_horizontal_relation,
            charset.solid_vertical_relation,
            charset.dotted_horizontal_relation,
            charset.dotted_vertical_relation,
        ],
        charset.relation_junction,
    )
}

fn charge_class_model_work(model: &ClassDiagram, resources: &mut ResourceContext) -> Result<()> {
    let item_count = model
        .classes
        .len()
        .checked_add(model.interfaces.len())
        .and_then(|value| value.checked_add(model.notes.len()))
        .and_then(|value| value.checked_add(model.relations.len()))
        .and_then(|value| value.checked_add(model.namespaces.len()))
        .ok_or_else(|| work_overflow(resources))?;
    resources.charge_layout_work(item_count.max(1))
}

fn preflight_class_text(model: &ClassDiagram, resources: &mut ResourceContext) -> Result<()> {
    for class in model.classes.values() {
        charge_text_layout(resources, &class.id)?;
        charge_text_layout(resources, &class.text)?;
        if let Some(parent) = class.parent.as_deref() {
            charge_text_layout(resources, parent)?;
        }
        for annotation in &class.annotations {
            charge_text_layout(resources, annotation)?;
        }
        for member in class.members.iter().chain(&class.methods) {
            preflight_class_member_text(member, resources)?;
        }
    }

    for interface in &model.interfaces {
        charge_text_layout(resources, &interface.id)?;
        charge_text_layout(resources, &interface.label)?;
        charge_text_layout(resources, &interface.class_id)?;
    }

    for note in &model.notes {
        charge_text_layout(resources, &note.id)?;
        charge_text_layout(resources, &note.text)?;
        if let Some(class_id) = note.class_id.as_deref() {
            charge_text_layout(resources, class_id)?;
        }
        if let Some(parent) = note.parent.as_deref() {
            charge_text_layout(resources, parent)?;
        }
    }

    for relation in &model.relations {
        charge_text_layout(resources, &relation.id1)?;
        charge_text_layout(resources, &relation.id2)?;
        charge_text_layout(resources, &relation.title)?;
        if let Some(label) = relation.relation_title_1.as_deref() {
            charge_text_layout(resources, label)?;
        }
        if let Some(label) = relation.relation_title_2.as_deref() {
            charge_text_layout(resources, label)?;
        }
    }

    for namespace in model.namespaces.values() {
        charge_text_layout(resources, &namespace.id)?;
        charge_text_layout(resources, &namespace.label)?;
        charge_text_layout(resources, &namespace.dom_id)?;
        if let Some(parent) = namespace.parent.as_deref() {
            charge_text_layout(resources, parent)?;
        }
        for id in namespace.class_ids.iter().chain(&namespace.note_ids) {
            charge_text_layout(resources, id)?;
        }
    }

    Ok(())
}

fn preflight_class_member_text(
    member: &ClassMember,
    resources: &mut ResourceContext,
) -> Result<()> {
    ClassMemberTextPlan::try_new(member, resources)?;
    Ok(())
}

fn grid_overflow(resources: &ResourceContext) -> AsciiError {
    resources.grid_overflow()
}

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
}

fn nesting_overflow(resources: &ResourceContext) -> AsciiError {
    resources.nesting_overflow()
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::AsciiColorMode;
    use crate::resource::AsciiResourcePolicy;
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::resources::ResourceProfile;
    use merman_core::{Engine, ParseOptions};
    use std::cell::Cell;

    fn resources_with_limit(id: AsciiResourceLimitId, max: usize) -> ResourceContext {
        let policy = AsciiResourcePolicy::default()
            .with_limit(id, max)
            .expect("resource test limit should be valid");
        ResourceContext::new(policy)
    }

    fn resources_with_layout_work_limit(max: usize) -> ResourceContext {
        resources_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, max)
    }

    fn unbounded_policy() -> AsciiResourcePolicy {
        AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
    }

    fn note(id: &str, text: &str) -> ClassNote {
        ClassNote {
            id: id.to_string(),
            class_id: None,
            text: text.to_string(),
            parent: None,
        }
    }

    fn assert_layout_work_limit(error: AsciiError, actual: usize, max: usize) {
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a typed layout work error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.phase(), AsciiResourceLimitPhase::LayoutWork);
        assert_eq!(details.actual, actual);
        assert_eq!(details.max, max);
        assert_eq!(details.profile, AsciiResourcePolicy::default().profile());
    }

    fn parsed_class_model(input: &str) -> ClassDiagram {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
            .expect("class diagram should parse")
            .expect("class diagram should be detected");
        match parsed.into_parts().1 {
            RenderSemanticModel::Class(model) => model,
            other => panic!("expected class render model, got {}", other.kind()),
        }
    }

    fn render_class_section_fixture(
        class: &ClassNode,
        options: &AsciiRenderOptions,
        policy: AsciiResourcePolicy,
        materialized: &Cell<bool>,
    ) -> (Result<String>, (usize, usize), (usize, usize)) {
        let mut resources = ResourceContext::new(policy);
        let mut deferred = DeferredTextRegistry::new();
        let relation_box = render_class_box(
            class,
            options,
            ClassCharset::for_options(options),
            &mut deferred,
            &mut resources,
        )
        .expect("class section fixture should plan before final output admission");
        let lines = relation_graph::stacked_box_lines(
            std::slice::from_ref(&relation_box),
            options.terminal_width_profile,
            &mut resources,
        )
        .expect("class section fixture should build relation rows");
        let before = (
            resources.layout_work_used(),
            resources.document_cells_used(),
        );
        let result = relation_graph::render_lines_with_deferred_probe(
            &lines,
            options,
            &mut resources,
            &deferred,
            || materialized.set(true),
        );
        let after = (
            resources.layout_work_used(),
            resources.document_cells_used(),
        );
        (result, before, after)
    }

    fn render_deferred_class_box(
        relation_box: RenderedClassBox,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
        deferred: &DeferredTextRegistry<'_>,
    ) -> String {
        let lines = relation_graph::stacked_box_lines(
            std::slice::from_ref(&relation_box),
            options.terminal_width_profile,
            resources,
        )
        .expect("class box rows should render");
        relation_graph::render_lines_with_deferred_options(&lines, options, resources, deferred)
            .expect("class box text should materialize")
    }

    fn render_interface_label(label: &str) -> String {
        let interface = ClassInterface {
            id: "interface0".to_string(),
            label: label.to_string(),
            class_id: "Target".to_string(),
        };
        let options = AsciiRenderOptions::unicode();
        let policy = unbounded_policy();
        let mut resources = ResourceContext::new(policy);
        let mut deferred = DeferredTextRegistry::new();
        let relation_box = render_interface_box(
            &interface,
            &options,
            ClassCharset::for_options(&options),
            &mut deferred,
            &mut resources,
        )
        .expect("interface label should plan");
        render_deferred_class_box(relation_box, &options, &mut resources, &deferred)
    }

    fn render_namespace_label(label: &str) -> String {
        render_namespace_identity("Scope", label)
    }

    fn render_namespace_identity(id: &str, label: &str) -> String {
        let namespace = merman_core::models::class_diagram::Namespace {
            id: id.to_string(),
            label: label.to_string(),
            dom_id: "namespace0".to_string(),
            class_ids: Vec::new(),
            note_ids: Vec::new(),
            parent: None,
            explicit: true,
        };
        let options = AsciiRenderOptions::unicode();
        let policy = unbounded_policy();
        let execution = AsciiExecution::for_test(&policy);
        let settings = ClassRenderSettings {
            options: &options,
            charset: ClassCharset::for_options(&options),
            direction: RelationDirection::TopDown,
        };
        let mut resources = ResourceContext::new(policy);
        let mut deferred = DeferredTextRegistry::new();
        let relation_box = render_namespace_container_box(
            &namespace,
            Vec::new(),
            settings,
            &mut deferred,
            &mut resources,
            execution,
        )
        .expect("namespace label should plan");
        render_deferred_class_box(relation_box, &options, &mut resources, &deferred)
    }

    fn render_class_identity_fixture(model: &ClassDiagram) -> String {
        let policy = unbounded_policy();
        render_class_diagram_with_execution(
            model,
            &AsciiRenderOptions::ascii(),
            AsciiExecution::for_test(&policy),
        )
        .expect("class identity fixture should render")
    }

    #[test]
    fn class_interface_and_namespace_labels_disclose_lossy_authored_projection() {
        for (authored, visible, expected_interface, expected_namespace) in [
            (
                "\u{1b}",
                r"\u{1B}",
                r#"interfaceLabel(bytes=1)="\u{1B}""#,
                r#"namespaceLabel(bytes=1)="\u{1B}""#,
            ),
            (
                "&amp;",
                "&",
                r#"interfaceLabel(bytes=5)="&amp;""#,
                r#"namespaceLabel(bytes=5)="&amp;""#,
            ),
        ] {
            let authored_interface = render_interface_label(authored);
            let visible_interface = render_interface_label(visible);
            assert_ne!(authored_interface, visible_interface);
            assert!(authored_interface.contains(expected_interface));
            assert!(!visible_interface.contains("interfaceLabel(bytes="));

            let authored_namespace = render_namespace_label(authored);
            let visible_namespace = render_namespace_label(visible);
            assert_ne!(authored_namespace, visible_namespace);
            assert!(authored_namespace.contains(expected_namespace));
            assert!(!visible_namespace.contains("namespaceLabel(bytes="));
        }

        for absent_label in ["", "   "] {
            let namespace = render_namespace_label(absent_label);
            assert!(!namespace.contains("namespaceLabel(bytes="), "{namespace}");
        }

        for fallback_label in ["", "   "] {
            for (authored_id, visible_id, expected_identity) in [
                ("\u{1b}", r"\u{1B}", r#"namespaceId(bytes=1)="\u{1B}""#),
                ("&amp;", "&", r#"namespaceId(bytes=5)="&amp;""#),
            ] {
                let authored_namespace = render_namespace_identity(authored_id, fallback_label);
                let visible_namespace = render_namespace_identity(visible_id, fallback_label);
                assert_ne!(authored_namespace, visible_namespace);
                assert!(authored_namespace.contains(expected_identity));
                assert!(!authored_namespace.contains("namespaceLabel(bytes="));
                assert!(!visible_namespace.contains("namespaceId(bytes="));
            }
        }
    }

    fn class_summary_model() -> ClassDiagram {
        let mut model =
            parsed_class_model("classDiagram\nclass A\nclass B\nA --> A : self\nA --> B : linked");
        let mut source = model.classes.shift_remove("A").expect("A should exist");
        source.id = "Source<&中".to_string();
        source.dom_id = source.id.clone();
        source.text = source.id.clone();
        let source_id = source.id.clone();
        model.classes.insert(source_id.clone(), source);
        let mut target = model.classes.shift_remove("B").expect("B should exist");
        target.id = "Target<&中".to_string();
        target.dom_id = target.id.clone();
        target.text = target.id.clone();
        let target_id = target.id.clone();
        model.classes.insert(target_id.clone(), target);
        for relation in &mut model.relations {
            if relation.id1 == "A" {
                relation.id1.clone_from(&source_id);
            }
            if relation.id2 == "A" {
                relation.id2.clone_from(&source_id);
            } else if relation.id2 == "B" {
                relation.id2.clone_from(&target_id);
            }
        }
        model.relations[0].title = "self<&中".to_string();
        model.relations[0].relation_title_1 = Some("a<&中<br>b".to_string());
        model.relations[0].relation_title_2 = Some("a/b<&中".to_string());
        model.relations[1].title = "linked<&中".to_string();
        model
    }

    fn render_class_summary_fixture(
        model: &ClassDiagram,
        options: &AsciiRenderOptions,
        policy: AsciiResourcePolicy,
        materialized: &Cell<bool>,
    ) -> (Result<String>, (usize, usize), (usize, usize)) {
        let mut resources = ResourceContext::new(policy);
        let execution = AsciiExecution::for_test(&policy);
        let charset = ClassCharset::for_options(options);
        let direction = RelationDirection::try_from_model(
            &model.direction,
            "class",
            "unknown class diagram directions",
        )
        .expect("class summary direction should be valid");
        let settings = ClassRenderSettings {
            options,
            charset,
            direction,
        };
        let mut deferred = DeferredTextRegistry::new();
        let endpoint_index =
            ClassEndpointIndex::new(model, &mut resources).expect("class endpoints should plan");
        let boxes = render_class_boxes(
            model,
            settings,
            &endpoint_index,
            &mut deferred,
            &mut resources,
            execution,
        )
        .expect("class summary boxes should plan");
        let relation_labels =
            prepare_class_relation_labels(model, options, &mut deferred, &resources)
                .expect("class summary labels should plan as one batch");
        let mut layouts = Vec::new();
        layouts
            .try_reserve_exact(model.relations.len())
            .expect("class summary layout allocation should succeed");
        for (relation_index, relation) in model.relations.iter().enumerate() {
            layouts.push(
                relation_layout(
                    model,
                    relation,
                    &endpoint_index,
                    relation_labels[relation_index].clone(),
                )
                .expect("class summary relation should plan"),
            );
        }
        let adapter = ClassRelationComponentAdapter {
            charset,
            width_profile: options.terminal_width_profile,
        };
        let lines = relation_graph::render_relation_component_lines(
            &boxes,
            &layouts,
            options,
            &mut resources,
            &adapter,
            &mut deferred,
        )
        .expect("class summary should plan");
        let before = (
            resources.layout_work_used(),
            resources.document_cells_used(),
        );
        let result = relation_graph::render_lines_with_deferred_probe(
            &lines,
            options,
            &mut resources,
            &deferred,
            || materialized.set(true),
        );
        let after = (
            resources.layout_work_used(),
            resources.document_cells_used(),
        );
        (result, before, after)
    }

    #[test]
    fn class_relation_text_discloses_lossy_authored_projection_in_routed_and_summary_output() {
        const AUTHORED: &str = "\u{1b}";
        const VISIBLE: &str = r"\u{1B}";
        const DISCLOSURE: &str = r#"authored(bytes=1)="\u{1B}""#;

        let routed = |value: &str| {
            let mut model = parsed_class_model("classDiagram\nclass A\nclass B\nA --> B");
            model.notes.push(note("note0", value));
            let relation = model
                .relations
                .first_mut()
                .expect("routed fixture should contain a relation");
            relation.title = value.to_string();
            relation.relation_title_1 = Some(value.to_string());
            render_class_identity_fixture(&model)
        };
        let authored_routed = routed(AUTHORED);
        let visible_routed = routed(VISIBLE);
        assert_ne!(authored_routed, visible_routed);
        assert_eq!(authored_routed.matches(DISCLOSURE).count(), 3);
        assert!(!visible_routed.contains("authored(bytes="));

        let html_break_routed = routed("left<br>right");
        let mermaid_break_routed = routed(r"left\nright");
        let line_break_routed = routed("left\nright");
        assert_ne!(html_break_routed, mermaid_break_routed);
        assert_ne!(html_break_routed, line_break_routed);
        assert_ne!(mermaid_break_routed, line_break_routed);
        assert_eq!(
            html_break_routed
                .matches(r#"authored(bytes=13)="left<br>right""#)
                .count(),
            3
        );
        assert!(mermaid_break_routed.contains(r#"authored(bytes=11)="left\\nright""#));
        assert!(line_break_routed.contains(r#"authored(bytes=10)="left\nright""#));

        let trimmed_routed = routed(" label ");
        let untrimmed_routed = routed("label");
        assert_ne!(trimmed_routed, untrimmed_routed);
        assert_eq!(
            trimmed_routed
                .matches(r#"authored(bytes=7)=" label ""#)
                .count(),
            3
        );
        assert!(!untrimmed_routed.contains("authored(bytes="));

        let spoofed_routed = routed(r#"\u{1B}<br>authored(bytes=1)="\u{1B}""#);
        assert_ne!(authored_routed, spoofed_routed);
        assert!(
            spoofed_routed.matches("authored(bytes=").count()
                > authored_routed.matches("authored(bytes=").count()
        );

        let controls_first = routed("\u{1b}\\u{7F}");
        let controls_last = routed("\\u{1B}\u{7f}");
        assert_ne!(controls_first, controls_last);

        let summary = |value: &str| {
            let mut model = class_summary_model();
            for relation in &mut model.relations {
                relation.title = value.to_string();
                relation.relation_title_1 = Some(value.to_string());
                relation.relation_title_2 = Some(value.to_string());
            }
            render_class_summary_fixture(
                &model,
                &AsciiRenderOptions::ascii(),
                unbounded_policy(),
                &Cell::new(false),
            )
            .0
            .expect("class summary identity fixture should render")
        };
        let authored_summary = summary(AUTHORED);
        let visible_summary = summary(VISIBLE);
        assert!(authored_summary.contains("relations:"));
        assert_ne!(authored_summary, visible_summary);
        assert!(authored_summary.contains(DISCLOSURE));
        assert!(!visible_summary.contains("authored(bytes="));

        let html_break_summary = summary("left<br>right");
        let mermaid_break_summary = summary(r"left\nright");
        let line_break_summary = summary("left\nright");
        assert_ne!(html_break_summary, mermaid_break_summary);
        assert_ne!(html_break_summary, line_break_summary);
        assert_ne!(mermaid_break_summary, line_break_summary);
        assert!(html_break_summary.contains(r#"authored(bytes=13)="left<br>right""#));
        assert!(mermaid_break_summary.contains(r#"authored(bytes=11)="left\\nright""#));
        assert!(line_break_summary.contains(r#"authored(bytes=10)="left\nright""#));

        let endpoint = |value: Option<&str>| {
            let mut model = parsed_class_model("classDiagram\nclass A\nclass B\nA --> B");
            let relation = model
                .relations
                .first_mut()
                .expect("endpoint identity fixture should contain a relation");
            relation.relation_title_1 = value.map(str::to_string);
            render_class_identity_fixture(&model)
        };
        let absent_endpoint = endpoint(None);
        let empty_endpoint = endpoint(Some(""));
        let blank_endpoint = endpoint(Some(" "));
        assert_ne!(absent_endpoint, empty_endpoint);
        assert_ne!(empty_endpoint, blank_endpoint);
        assert!(empty_endpoint.contains(r#"authored(bytes=0)="""#));
        assert!(blank_endpoint.contains(r#"authored(bytes=1)=" ""#));
    }

    #[test]
    fn authored_endpoint_label_cannot_spoof_generated_endpoint_provenance() {
        const AUTHORED_REF: &str = "Domain.T";
        const SPOOF: &str = r#"endpointRef(bytes=8)="Domain.T""#;

        let mut model = parsed_class_model("classDiagram\nclass A\nclass B\nA --> B");
        model.relations[0].relation_title_1 = Some(SPOOF.to_string());
        let options = AsciiRenderOptions::ascii();
        let policy = unbounded_policy();
        let resources = ResourceContext::new(policy);
        let mut deferred = DeferredTextRegistry::new();
        let mut labels = prepare_class_relation_labels(&model, &options, &mut deferred, &resources)
            .expect("the authored endpoint label should plan");
        let authored = join_endpoint_label_with_authored_ref(
            Some(AUTHORED_REF),
            labels[0].left_endpoint.take(),
            options.terminal_width_profile,
            &mut deferred,
            &resources,
        )
        .expect("authored endpoint label plus provenance should plan")
        .expect("authored endpoint label plus provenance should remain present");
        let generated = join_endpoint_label_with_authored_ref(
            Some(AUTHORED_REF),
            None,
            options.terminal_width_profile,
            &mut deferred,
            &resources,
        )
        .expect("generated endpoint provenance should plan")
        .expect("generated endpoint provenance should be present");

        let authored_lines =
            relation_graph::label_lines_with_role(&authored, AsciiColorRole::EdgeLabel, &resources)
                .expect("authored endpoint label rows should materialize");
        let generated_lines = relation_graph::label_lines_with_role(
            &generated,
            AsciiColorRole::EdgeLabel,
            &resources,
        )
        .expect("generated endpoint provenance rows should materialize");
        let mut authored_resources = ResourceContext::new(policy);
        let authored_output = relation_graph::render_lines_with_deferred_options(
            &authored_lines,
            &options,
            &mut authored_resources,
            &deferred,
        )
        .expect("authored endpoint label should render");
        let mut generated_resources = ResourceContext::new(policy);
        let generated_output = relation_graph::render_lines_with_deferred_options(
            &generated_lines,
            &options,
            &mut generated_resources,
            &deferred,
        )
        .expect("generated endpoint provenance should render");

        assert_ne!(authored_output, generated_output);
        assert!(authored_output.contains(SPOOF));
        assert!(authored_output.contains(r#"endpointLabel=[bytes=31 "endpointRef"#));
        assert_eq!(generated_output, format!("{SPOOF}\n"));
    }

    #[test]
    fn class_summary_frames_swapped_control_and_literal_escape_endpoint_ids_injectively() {
        let summary = |first_id: &str, second_id: &str| {
            let mut model = class_summary_model();
            let mut first = model
                .classes
                .shift_remove("Source<&中")
                .expect("source should exist");
            first.id = first_id.to_string();
            first.dom_id.clone_from(&first.id);
            first.text.clone_from(&first.id);
            let mut second = model
                .classes
                .shift_remove("Target<&中")
                .expect("target should exist");
            second.id = second_id.to_string();
            second.dom_id.clone_from(&second.id);
            second.text.clone_from(&second.id);
            model.classes.insert(first.id.clone(), first);
            model.classes.insert(second.id.clone(), second);
            for relation in &mut model.relations {
                for endpoint in [&mut relation.id1, &mut relation.id2] {
                    if endpoint.as_str() == "Source<&中" {
                        *endpoint = first_id.to_string();
                    } else if endpoint.as_str() == "Target<&中" {
                        *endpoint = second_id.to_string();
                    }
                }
            }
            model.relations[0].title = "self".to_string();
            model.relations[1].title = "ab".to_string();

            let rendered = render_class_summary_fixture(
                &model,
                &AsciiRenderOptions::ascii(),
                unbounded_policy(),
                &Cell::new(false),
            )
            .0
            .expect("class summary identity fixture should render");
            rendered
                .split_once("relations:\n")
                .expect("bidirectional relations should use summary fallback")
                .1
                .to_string()
        };

        let control_first = summary("\u{1b}", r"\u{1B}");
        let literal_escape_first = summary(r"\u{1B}", "\u{1b}");
        let control_first_ab = control_first
            .lines()
            .find(|line| line.ends_with(": ab"))
            .expect("control-first summary should retain the ab relation");
        let literal_escape_first_ab = literal_escape_first
            .lines()
            .find(|line| line.ends_with(": ab"))
            .expect("literal-first summary should retain the ab relation");

        assert!(
            control_first_ab.starts_with(r#"id(bytes=1)="\u{1B}""#)
                && control_first_ab.contains(r#"id(bytes=6)="\\u{1B}""#),
            "control-first endpoint identity must remain injective:\n{control_first}"
        );
        assert!(
            literal_escape_first_ab.starts_with(r#"id(bytes=6)="\\u{1B}""#)
                && literal_escape_first_ab.contains(r#"id(bytes=1)="\u{1B}""#),
            "literal-first endpoint identity must remain injective:\n{literal_escape_first}"
        );
        assert_ne!(control_first, literal_escape_first);
    }

    #[test]
    fn class_summary_admits_exact_encoded_output_before_deferred_materialization() {
        let model = class_summary_model();
        for mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let base = AsciiRenderOptions::unicode().with_color_mode(mode);
            let base_policy = unbounded_policy();
            let measured_probe = Cell::new(false);
            let (measured, _, _) =
                render_class_summary_fixture(&model, &base, base_policy, &measured_probe);
            let expected = measured.expect("unbounded class summary should render");
            assert!(measured_probe.get(), "mode={mode:?}");
            assert!(expected.contains("relations:"), "mode={mode:?}");
            if mode == AsciiColorMode::Html {
                assert!(expected.contains("Source&lt;&amp;中"), "mode={mode:?}");
                assert!(expected.contains("self&lt;&amp;中"), "mode={mode:?}");
            } else {
                assert!(expected.contains("Source<&中"), "mode={mode:?}");
                assert!(expected.contains("self<&中"), "mode={mode:?}");
                assert!(expected.contains(
                    r#"endpoint1=[bytes=6 "a<&中", bytes=1 "b", bytes=32 "authored(bytes=11)=\"a<&中<br>b\""]"#,
                ));
            }

            let exact_policy = base_policy
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len())
                .expect("exact class summary output limit should be valid");
            let exact_probe = Cell::new(false);
            let (rendered, _, _) =
                render_class_summary_fixture(&model, &base, exact_policy, &exact_probe);
            assert_eq!(
                rendered.expect("exact class summary should materialize"),
                expected,
                "mode={mode:?}"
            );
            assert!(exact_probe.get(), "mode={mode:?}");

            let below_policy = base_policy
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len() - 1)
                .expect("max-minus-one class summary limit should be valid");
            let below_probe = Cell::new(false);
            let (error, before, after) =
                render_class_summary_fixture(&model, &base, below_policy, &below_probe);
            assert!(!below_probe.get(), "mode={mode:?}");
            assert_eq!(after, before, "mode={mode:?}");
            assert!(matches!(
                error.expect_err("max-minus-one class summary must reject"),
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == AsciiResourceLimitId::MaxOutputBytes
                        && details.actual == expected.len()
                        && details.max == expected.len() - 1
            ));
        }
    }

    #[test]
    fn class_sections_admit_exact_encoded_output_before_deferred_materialization() {
        let mut model = parsed_class_model("classDiagram\nclass Service");
        let class = model
            .classes
            .get_mut("Service")
            .expect("Service class should exist");
        class.text = "&lt;&amp;中".to_string();
        class.annotations = vec!["authored<&中".to_string()];
        class.members = vec![ClassMember {
            member_type: "attribute".to_string(),
            visibility: "+".to_string(),
            id: "value<&中".to_string(),
            classifier: "*".to_string(),
            parameters: String::new(),
            return_type: String::new(),
            display_text: String::new(),
            css_style: String::new(),
        }];
        let class = model
            .classes
            .get("Service")
            .expect("Service class should remain available");

        for mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let base = AsciiRenderOptions::unicode().with_color_mode(mode);
            let base_policy = unbounded_policy();
            let measured_probe = Cell::new(false);
            let (measured, _, _) =
                render_class_section_fixture(class, &base, base_policy, &measured_probe);
            let expected = measured.expect("unbounded class section should render");
            assert!(measured_probe.get(), "mode={mode:?}");
            if mode == AsciiColorMode::Html {
                assert!(expected.contains("&lt;&amp;中"), "mode={mode:?}");
            } else {
                assert!(expected.contains("<&中"), "mode={mode:?}");
                assert!(expected.contains("+value<&中*"), "mode={mode:?}");
            }

            let exact_policy = base_policy
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len())
                .expect("exact class output limit should be valid");
            let exact_probe = Cell::new(false);
            let (rendered, _, _) =
                render_class_section_fixture(class, &base, exact_policy, &exact_probe);
            assert_eq!(
                rendered.expect("exact class output should materialize"),
                expected,
                "mode={mode:?}"
            );
            assert!(exact_probe.get(), "mode={mode:?}");

            let below_policy = base_policy
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len() - 1)
                .expect("max-minus-one class output limit should be valid");
            let below_probe = Cell::new(false);
            let (error, before, after) =
                render_class_section_fixture(class, &base, below_policy, &below_probe);
            assert!(!below_probe.get(), "mode={mode:?}");
            assert_eq!(after, before, "mode={mode:?}");
            assert!(
                matches!(
                    error.expect_err("max-minus-one class output must reject"),
                    AsciiError::ResourceLimitExceeded(details)
                        if details.limit == AsciiResourceLimitId::MaxOutputBytes
                            && details.actual == expected.len()
                            && details.max == expected.len() - 1
                ),
                "mode={mode:?}"
            );
        }
    }

    #[test]
    fn reconstructed_member_fields_are_preflighted_before_text_materialization() {
        const OVERSIZED_GRAPHEME: &str = "e\u{301}";
        let parsed =
            parsed_class_model("classDiagram\nclass Service {\n  +compute(input) Result\n}");
        for field in [
            "visibility",
            "id",
            "parameters",
            "return_type",
            "classifier",
        ] {
            let mut model = parsed.clone();
            let member = model
                .classes
                .get_mut("Service")
                .and_then(|class| class.methods.first_mut())
                .expect("Service method should exist");
            member.display_text.clear();
            match field {
                "visibility" => member.visibility = OVERSIZED_GRAPHEME.to_string(),
                "id" => member.id = OVERSIZED_GRAPHEME.to_string(),
                "parameters" => member.parameters = OVERSIZED_GRAPHEME.to_string(),
                "return_type" => member.return_type = OVERSIZED_GRAPHEME.to_string(),
                "classifier" => member.classifier = OVERSIZED_GRAPHEME.to_string(),
                _ => unreachable!(),
            }

            let exact_policy = AsciiResourcePolicy::default()
                .with_limit(
                    AsciiResourceLimitId::MaxGraphemeBytes,
                    OVERSIZED_GRAPHEME.len(),
                )
                .expect("grapheme resource limit should be valid");
            preflight_class_text(&model, &mut ResourceContext::new(exact_policy))
                .unwrap_or_else(|error| panic!("{field} must pass at the exact limit: {error:?}"));

            let policy = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 2)
                .expect("grapheme resource limit should be valid");
            let mut resources = ResourceContext::new(policy);
            let error = match preflight_class_text(&model, &mut resources) {
                Ok(()) => panic!("{field} must be preflighted"),
                Err(error) => error,
            };
            let AsciiError::ResourceLimitExceeded(details) = error else {
                panic!("expected a grapheme resource error for {field}, got {error:?}");
            };
            assert_eq!(details.limit, AsciiResourceLimitId::MaxGraphemeBytes);
            assert_eq!(details.actual, OVERSIZED_GRAPHEME.len());
            assert_eq!(details.max, 2);
        }
    }

    #[test]
    fn displayed_member_classifier_is_preflighted_with_the_canonical_text_plan() {
        let mut model = parsed_class_model("classDiagram\nclass Service {\n  +value*\n}");
        let member = model
            .classes
            .get_mut("Service")
            .and_then(|class| class.members.first_mut())
            .expect("Service member should exist");
        member.display_text = "A".to_string();
        member.classifier = "\u{301}".to_string();

        let exact_policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 3)
            .expect("grapheme resource limit should be valid");
        preflight_class_text(&model, &mut ResourceContext::new(exact_policy))
            .expect("display plus classifier should pass at the exact grapheme byte limit");

        let below_policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 2)
            .expect("grapheme resource limit should be valid");
        let error = preflight_class_text(&model, &mut ResourceContext::new(below_policy))
            .expect_err("display plus classifier must be planned as one grapheme stream");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGraphemeBytes
                    && details.actual == 3
                    && details.max == 2
        ));
    }

    #[test]
    fn reconstructed_generic_member_admits_layout_work_before_materializing() {
        let mut model =
            parsed_class_model("classDiagram\nclass Service {\n  #compute(input) Result\n}");
        let member = model
            .classes
            .get_mut("Service")
            .and_then(|class| class.methods.first_mut())
            .expect("Service method should exist");
        member.visibility = "#".to_string();
        member.id = "compute~Array~Array~T~~~".to_string();
        member.parameters = "input: Map~Key,Value~".to_string();
        member.return_type = "Result~List~T~~".to_string();
        member.classifier = "*".to_string();
        member.display_text.clear();

        let member = model
            .classes
            .get("Service")
            .and_then(|class| class.methods.first())
            .expect("Service method should remain available");
        let unbounded = AsciiResourcePolicy::default();
        let measured = ResourceContext::new(unbounded);
        let materialized = Cell::new(false);
        let rendered = member_text_with_probe(member, &measured, || materialized.set(true))
            .expect("unbounded generic member should render");
        let exact_work = measured.layout_work_used();
        assert!(exact_work > 1);
        assert!(materialized.get());
        assert!(
            rendered.contains("compute<Array<Array<T>>>") && rendered.contains("Map<Key,Value>")
        );

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact generic-member work limit should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        let exact_materialized = Cell::new(false);
        member_text_with_probe(member, &exact_resources, || exact_materialized.set(true))
            .expect("exact generic-member work should permit materialization");
        assert!(exact_materialized.get());
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one generic-member work limit should be valid");
        let below_resources = ResourceContext::new(below_policy);
        let below_materialized = Cell::new(false);
        let error =
            member_text_with_probe(member, &below_resources, || below_materialized.set(true))
                .expect_err("max-minus-one generic-member work must reject before materialization");
        assert!(!below_materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual > details.max
        ));
    }

    #[test]
    fn reconstructed_member_rejects_cross_field_grapheme_before_materializing() {
        let mut model =
            parsed_class_model("classDiagram\nclass Service {\n  +compute(input) Result\n}");
        let member = model
            .classes
            .get_mut("Service")
            .and_then(|class| class.methods.first_mut())
            .expect("Service method should exist");
        member.id = "compute".to_string();
        member.parameters = "\u{301}".to_string();
        member.return_type.clear();
        member.display_text.clear();

        let member = model
            .classes
            .get("Service")
            .and_then(|class| class.methods.first())
            .expect("Service method should remain available");
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 2)
            .expect("grapheme resource limit should be valid");
        let resources = ResourceContext::new(policy);
        resources
            .charge_usage(3, 5)
            .expect("test checkpoint should fit");
        let materialized = Cell::new(false);
        let error = member_text_with_probe(member, &resources, || materialized.set(true))
            .expect_err("opening parenthesis plus combining mark exceeds two bytes");
        assert!(!materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGraphemeBytes
                    && details.actual == 3
                    && details.max == 2
        ));
        assert_eq!(resources.layout_work_used(), 3);
        assert_eq!(resources.document_cells_used(), 5);
    }

    #[test]
    fn horizontal_class_strip_checks_grid_and_layout_work_before_allocating_rows() {
        let boxes = vec![
            RelationGraphBox::new("A".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("B".to_string(), vec!["B".to_string()], 1),
        ];

        for limit in [
            AsciiResourceLimitId::MaxGridCells,
            AsciiResourceLimitId::MaxLayoutWorkUnits,
        ] {
            let resources = resources_with_limit(limit, 5);
            let error = relation_graph::render_horizontal_box_strip_lines(
                &boxes,
                RelationGraphHorizontalDirection::LeftRight,
                CLASS_LEVEL_HORIZONTAL_GAP,
                TerminalWidthProfile::Unicode,
                &resources,
            )
            .expect_err("a six-cell horizontal strip must reject a five-cell/work budget");
            let AsciiError::ResourceLimitExceeded(details) = error else {
                panic!("expected a typed horizontal resource error, got {error:?}");
            };
            assert_eq!(details.limit, limit);
            assert_eq!(details.actual, 6);
            assert_eq!(details.max, 5);
        }
    }

    #[test]
    fn class_note_index_charges_linear_build_and_lookup_work() {
        let notes = vec![note("note-a", "first"), note("note-b", "second")];

        let mut exact = resources_with_layout_work_limit(4);
        let index = ClassNoteIndex::new(&notes, &mut exact)
            .expect("two notes should consume two index build work units");
        assert_eq!(
            index
                .get("note-a", &mut exact)
                .expect("first lookup should fit the exact budget")
                .expect("indexed note should exist")
                .text,
            "first"
        );
        assert!(
            index
                .get("missing", &mut exact)
                .expect("missing lookup should still fit the exact budget")
                .is_none()
        );
        assert_eq!(exact.layout_work_used(), 4);

        let mut below = resources_with_layout_work_limit(3);
        let index = ClassNoteIndex::new(&notes, &mut below)
            .expect("index build should fit below the full lookup budget");
        index
            .get("note-a", &mut below)
            .expect("first lookup should reach the configured budget");
        let error = index
            .get("note-b", &mut below)
            .expect_err("the N-1 budget must reject the second constant-time lookup");
        assert_layout_work_limit(error, 4, 3);
    }

    #[test]
    fn class_box_index_charges_linear_build_and_lookup_work() {
        let boxes = vec![
            RelationGraphBox::new("A".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("B".to_string(), vec!["B".to_string()], 1),
        ];

        let mut exact = resources_with_layout_work_limit(4);
        let index = RenderedClassBoxIndex::new(&boxes, &mut exact)
            .expect("two boxes should consume two index build work units");
        assert_eq!(
            index
                .require("A", &mut exact)
                .expect("first endpoint lookup should fit the exact budget")
                .id(),
            "A"
        );
        assert!(
            index
                .get("missing", &mut exact)
                .expect("missing endpoint lookup should fit the exact budget")
                .is_none()
        );
        assert_eq!(exact.layout_work_used(), 4);

        let mut below = resources_with_layout_work_limit(3);
        let index = RenderedClassBoxIndex::new(&boxes, &mut below)
            .expect("index build should fit below the full lookup budget");
        index
            .require("A", &mut below)
            .expect("first endpoint lookup should reach the configured budget");
        let error = index
            .require("B", &mut below)
            .expect_err("the N-1 budget must reject the second constant-time lookup");
        assert_layout_work_limit(error, 4, 3);
    }
}
