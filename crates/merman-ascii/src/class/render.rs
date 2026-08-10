use crate::AsciiError;
use crate::Result;
use crate::color::AsciiColorRole;
use crate::options::{AsciiCharset, AsciiRenderOptions, TerminalWidthProfile};
use crate::relation_graph;
use crate::relation_graph::RelationGraphBox;
use crate::relation_graph::{
    HorizontalRelationEndpoint, HorizontalRelationMarker, HorizontalRelationStyle,
    LayeredRelationEdge, LayeredRelationError, LayeredRelationRouteStyle, RelationGraphBoxStyle,
    RelationGraphHorizontalDirection, RelationGraphLabel, RelationGraphLine,
    RelationGraphSummaryRow, RelationLineChars, RelationOverlay, RelationParallelPlan,
    RelationPortSide, RelationRegionPlan, RelationSelfLoopMetrics, RelationStackPlan,
    RelationSummaryPaintPlan,
};
#[cfg(test)]
use crate::resource::AsciiResourceLimitId;
use crate::resource::{AsciiResourceLimitPhase, LogicalExtent, ResourceContext};
use crate::safe_text::charge_text_layout;
use crate::text::display_width_with_profile;
use merman_core::entities::decode_html_entities_to_unicode;
use merman_core::models::class_diagram::{
    ClassDiagram, ClassInterface, ClassMember, ClassNode, ClassNote, ClassRelation,
};
use std::collections::{HashMap, HashSet};

mod namespace;

use namespace::{
    has_renderable_namespaces, namespace_facade_aliases, render_namespace_container_box,
    render_namespaced_class_diagram,
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClassDirection {
    TopDown,
    BottomUp,
    LeftRight,
    RightLeft,
}

impl ClassDirection {
    fn try_from_model(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "" | "TB" | "TD" => Ok(Self::TopDown),
            "BT" => Ok(Self::BottomUp),
            "LR" => Ok(Self::LeftRight),
            "RL" => Ok(Self::RightLeft),
            _ => Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "unknown class diagram directions",
            }),
        }
    }

    fn is_horizontal(self) -> bool {
        matches!(self, Self::LeftRight | Self::RightLeft)
    }

    fn is_reversed(self) -> bool {
        matches!(self, Self::BottomUp | Self::RightLeft)
    }

    fn horizontal_direction(self) -> RelationGraphHorizontalDirection {
        match self {
            Self::RightLeft => RelationGraphHorizontalDirection::RightLeft,
            Self::TopDown | Self::BottomUp | Self::LeftRight => {
                RelationGraphHorizontalDirection::LeftRight
            }
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
    top_id: &'a str,
    bottom_id: &'a str,
    top_marker: Option<RelationMarker>,
    bottom_marker: Option<RelationMarker>,
    line: RelationLine,
    label: Option<RelationGraphLabel>,
    top_endpoint_label: Option<RelationGraphLabel>,
    bottom_endpoint_label: Option<RelationGraphLabel>,
    top_endpoint_role: EndpointLabelRole,
    bottom_endpoint_role: EndpointLabelRole,
}

impl RelationLayout<'_> {
    fn apply_direction(&mut self, direction: ClassDirection) {
        if direction != ClassDirection::BottomUp {
            return;
        }
        std::mem::swap(&mut self.top_id, &mut self.bottom_id);
        std::mem::swap(&mut self.top_marker, &mut self.bottom_marker);
        std::mem::swap(
            &mut self.top_endpoint_label,
            &mut self.bottom_endpoint_label,
        );
        std::mem::swap(&mut self.top_endpoint_role, &mut self.bottom_endpoint_role);
    }
}

pub(crate) fn render_class_diagram(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
) -> Result<String> {
    let mut resources = ResourceContext::new(options.resources);
    preflight_class_text(model, &mut resources)?;
    charge_class_model_work(model, &mut resources)?;
    validate_unique_class_render_ids(model, &mut resources)?;
    let charset = ClassCharset::for_options(options);
    let direction = ClassDirection::try_from_model(&model.direction)?;
    let namespace_facade_aliases = namespace_facade_aliases(model)?;
    if has_renderable_namespaces(model) {
        return render_namespaced_class_diagram(
            model,
            options,
            charset,
            direction,
            &namespace_facade_aliases,
            &mut resources,
        );
    }

    let boxes = render_class_boxes(
        model,
        options,
        charset,
        direction,
        &namespace_facade_aliases,
        &mut resources,
    )?;
    if boxes.is_empty() {
        if !model.relations.is_empty() {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "relationships with missing endpoint classes",
            });
        }
        return relation_graph::render_stacked_boxes_with_options(&boxes, options, &mut resources);
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
    for relation in &model.relations {
        layouts.push(relation_layout(
            model,
            relation,
            &namespace_facade_aliases,
            options.terminal_width_profile,
            &resources,
        )?);
    }
    layouts.extend(note_relation_layouts(
        model,
        &namespace_facade_aliases,
        &box_by_id,
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
            return render_class_document_lines(lines, options, &mut resources);
        }
        if direction.is_reversed() {
            let lines = relation_graph::stacked_box_lines_ordered(
                &boxes,
                options.terminal_width_profile,
                true,
                &mut resources,
            )?;
            return render_class_document_lines(lines, options, &mut resources);
        }
        return relation_graph::render_stacked_boxes_with_options(&boxes, options, &mut resources);
    }

    if direction.is_horizontal() {
        let lines = render_horizontal_class_component_lines(
            &boxes,
            &box_by_id,
            &layouts,
            direction,
            options,
            charset,
            &mut resources,
        )?;
        return render_class_document_lines(lines, options, &mut resources);
    }

    render_class_components(
        &boxes,
        &box_by_id,
        &layouts,
        options,
        charset,
        &mut resources,
    )
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
                .map(|namespace| namespace.id.as_str()),
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

fn render_class_boxes(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    direction: ClassDirection,
    namespace_facade_aliases: &HashMap<String, String>,
    resources: &mut ResourceContext,
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
        .filter(|class| !namespace_facade_aliases.contains_key(class.id.as_str()))
    {
        boxes.push(render_class_box(class, options, charset, resources)?);
    }
    for interface in &model.interfaces {
        boxes.push(render_interface_box(
            interface, options, charset, resources,
        )?);
    }
    for note in &model.notes {
        boxes.push(render_note_box(note, options, charset, resources)?);
    }
    for namespace in model.namespaces.values() {
        boxes.push(render_namespace_container_box(
            namespace,
            Vec::new(),
            options,
            charset,
            direction,
            resources,
        )?);
    }
    Ok(boxes)
}

fn render_class_components(
    boxes: &[RenderedClassBox],
    _box_by_id: &RenderedClassBoxIndex<'_>,
    layouts: &[RelationLayout<'_>],
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    resources: &mut ResourceContext,
) -> Result<String> {
    let adapter = ClassRelationComponentAdapter {
        charset,
        width_profile: options.terminal_width_profile,
    };
    relation_graph::render_relation_components(boxes, layouts, options, resources, &adapter)
}

fn render_class_component_lines(
    boxes: &[RenderedClassBox],
    _box_by_id: &RenderedClassBoxIndex<'_>,
    layouts: &[RelationLayout<'_>],
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let adapter = ClassRelationComponentAdapter {
        charset,
        width_profile: options.terminal_width_profile,
    };
    Ok(relation_graph::render_relation_component_lines(
        boxes, layouts, options, resources, &adapter,
    )?
    .unwrap_or_default())
}

fn render_class_document_lines(
    lines: Vec<RelationGraphLine>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    relation_graph::render_lines_with_options(&lines, options, resources)
}

fn render_horizontal_class_component_lines(
    boxes: &[RenderedClassBox],
    _box_by_id: &RenderedClassBoxIndex<'_>,
    layouts: &[RelationLayout<'_>],
    direction: ClassDirection,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let adapter = ClassRelationComponentAdapter {
        charset,
        width_profile: options.terminal_width_profile,
    };
    relation_graph::render_horizontal_relation_components(
        boxes,
        layouts,
        direction.horizontal_direction(),
        options,
        resources,
        &adapter,
    )
}

fn render_class_box(
    class: &ClassNode,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    let sections = class_sections(class, resources)?;
    render_box_sections(class.id.clone(), sections, options, charset, resources)
}

fn render_interface_box(
    interface: &ClassInterface,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    let mut header = Vec::new();
    header
        .try_reserve_exact(2)
        .map_err(|_| layout_allocation_failed())?;
    header.push("<<interface>>".to_string());
    header.push(decode_html_entities_to_unicode(&interface.label).into_owned());
    let mut sections = Vec::new();
    sections
        .try_reserve_exact(1)
        .map_err(|_| layout_allocation_failed())?;
    sections.push(header);
    render_box_sections(interface.id.clone(), sections, options, charset, resources)
}

fn render_note_box(
    note: &ClassNote,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    let label = RelationGraphLabel::try_new(&note.text, options.terminal_width_profile, resources)?;
    let capacity = label
        .as_ref()
        .map(RelationGraphLabel::line_count)
        .unwrap_or(0)
        .checked_add(1)
        .ok_or_else(|| work_overflow(resources))?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(capacity)
        .map_err(|_| layout_allocation_failed())?;
    lines.push("note".to_string());
    if let Some(label) = label {
        lines.extend(label.lines().iter().cloned());
    }

    let mut sections = Vec::new();
    sections
        .try_reserve_exact(1)
        .map_err(|_| layout_allocation_failed())?;
    sections.push(lines);
    render_box_sections(note.id.clone(), sections, options, charset, resources)
}

fn render_box_sections(
    id: String,
    sections: Vec<Vec<String>>,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    let style = RelationGraphBoxStyle {
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
    };
    relation_graph::RelationGraphBox::from_sections(
        id,
        &sections,
        options.box_border_padding,
        style,
        options.terminal_width_profile,
        resources,
    )
}

fn class_sections(class: &ClassNode, resources: &ResourceContext) -> Result<Vec<Vec<String>>> {
    let header_capacity = class
        .annotations
        .len()
        .checked_add(1)
        .ok_or_else(|| work_overflow(resources))?;
    let mut header = Vec::new();
    header
        .try_reserve_exact(header_capacity)
        .map_err(|_| layout_allocation_failed())?;
    header.extend(
        class
            .annotations
            .iter()
            .map(|annotation| format!("<<{annotation}>>")),
    );
    header.push(class_title(class));

    let mut sections = Vec::new();
    sections
        .try_reserve_exact(3)
        .map_err(|_| layout_allocation_failed())?;
    sections.push(header);

    let mut members = Vec::new();
    members
        .try_reserve_exact(class.members.len())
        .map_err(|_| layout_allocation_failed())?;
    members.extend(
        class
            .members
            .iter()
            .map(member_text)
            .filter(|line| !line.is_empty()),
    );
    if !members.is_empty() {
        sections.push(members);
    }

    let mut methods = Vec::new();
    methods
        .try_reserve_exact(class.methods.len())
        .map_err(|_| layout_allocation_failed())?;
    methods.extend(
        class
            .methods
            .iter()
            .map(member_text)
            .filter(|line| !line.is_empty()),
    );
    if !methods.is_empty() {
        sections.push(methods);
    }

    Ok(sections)
}

fn class_title(class: &ClassNode) -> String {
    decode_html_entities_to_unicode(&class.text).into_owned()
}

fn member_text(member: &ClassMember) -> String {
    if !member.display_text.is_empty() {
        return member.display_text.clone();
    }

    let mut text = String::new();
    text.push_str(&member.visibility);
    text.push_str(&member.id);
    if member.member_type == "method"
        || !member.parameters.is_empty()
        || !member.return_type.is_empty()
    {
        text.push('(');
        text.push_str(member.parameters.trim());
        text.push(')');
        if !member.return_type.is_empty() {
            text.push_str(" : ");
            text.push_str(member.return_type.trim());
        }
    }
    text.push_str(&member.classifier);
    text
}

fn relation_layout<'a>(
    model: &'a ClassDiagram,
    relation: &'a ClassRelation,
    namespace_facade_aliases: &'a HashMap<String, String>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<RelationLayout<'a>> {
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

    let label = RelationGraphLabel::try_new(&relation.title, width_profile, resources)?;
    let left_endpoint_label =
        relation_endpoint_label(&relation.relation_title_1, width_profile, resources)?;
    let right_endpoint_label =
        relation_endpoint_label(&relation.relation_title_2, width_profile, resources)?;

    if left_marker == Some(RelationMarker::Extension) && right_marker.is_none() {
        return Ok(RelationLayout {
            top_id: relation_endpoint_id(namespace_facade_aliases, relation.id1.as_str()),
            bottom_id: relation_endpoint_id(namespace_facade_aliases, relation.id2.as_str()),
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
            top_id: relation_endpoint_id(namespace_facade_aliases, relation.id2.as_str()),
            bottom_id: relation_endpoint_id(namespace_facade_aliases, relation.id1.as_str()),
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
        top_id: relation_endpoint_id(namespace_facade_aliases, relation.id1.as_str()),
        bottom_id: relation_endpoint_id(namespace_facade_aliases, relation.id2.as_str()),
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

fn relation_endpoint_id<'a>(
    namespace_facade_aliases: &'a HashMap<String, String>,
    id: &'a str,
) -> &'a str {
    namespace_facade_aliases
        .get(id)
        .map(String::as_str)
        .unwrap_or(id)
}

fn relation_explicit_namespace_id<'a>(
    model: &'a ClassDiagram,
    relation: &'a ClassRelation,
    namespace_facade_aliases: &'a HashMap<String, String>,
) -> Option<&'a str> {
    let left_parent =
        class_explicit_namespace_id(model, relation.id1.as_str(), namespace_facade_aliases)?;
    let right_parent =
        class_explicit_namespace_id(model, relation.id2.as_str(), namespace_facade_aliases)?;
    (left_parent == right_parent).then_some(left_parent)
}

fn class_explicit_namespace_id<'a>(
    model: &'a ClassDiagram,
    id: &'a str,
    namespace_facade_aliases: &'a HashMap<String, String>,
) -> Option<&'a str> {
    let class_id = relation_endpoint_id(namespace_facade_aliases, id);
    let parent = model.classes.get(class_id)?.parent.as_deref()?;
    model
        .namespaces
        .get(parent)
        .filter(|namespace| namespace.explicit)
        .map(|namespace| namespace.id.as_str())
}

fn note_explicit_namespace_id<'a>(
    model: &'a ClassDiagram,
    note: &'a ClassNote,
    namespace_facade_aliases: &'a HashMap<String, String>,
) -> Option<&'a str> {
    let note_parent = note.parent.as_deref()?;
    let note_namespace = model
        .namespaces
        .get(note_parent)
        .filter(|namespace| namespace.explicit)?;
    let target_id = note.class_id.as_deref()?;
    let target_parent = class_explicit_namespace_id(model, target_id, namespace_facade_aliases)?;
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
    namespace_facade_aliases: &'a HashMap<String, String>,
    box_by_id: &RenderedClassBoxIndex<'_>,
    resources: &mut ResourceContext,
) -> Result<Vec<RelationLayout<'a>>> {
    note_relation_layouts_for_notes(
        model.notes.iter(),
        namespace_facade_aliases,
        box_by_id,
        resources,
    )
}

fn note_relation_layouts_for_notes<'a>(
    notes: impl Iterator<Item = &'a ClassNote>,
    namespace_facade_aliases: &'a HashMap<String, String>,
    box_by_id: &RenderedClassBoxIndex<'_>,
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
        let target_id = relation_endpoint_id(namespace_facade_aliases, target_id);
        if box_by_id.get(target_id, resources)?.is_some() {
            layouts.push(RelationLayout {
                top_id: note.id.as_str(),
                bottom_id: target_id,
                top_marker: None,
                bottom_marker: None,
                line: RelationLine::Dotted,
                label: None,
                top_endpoint_label: None,
                bottom_endpoint_label: None,
                top_endpoint_role: EndpointLabelRole::First,
                bottom_endpoint_role: EndpointLabelRole::Second,
            });
        }
    }
    Ok(layouts)
}

fn external_namespace_note_summary_rows(
    model: &ClassDiagram,
    namespace_facade_aliases: &HashMap<String, String>,
) -> Result<Vec<RelationGraphSummaryRow>> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(model.notes.len())
        .map_err(|_| layout_allocation_failed())?;
    for note in model
        .notes
        .iter()
        .filter(|note| note_explicit_namespace_id(model, note, namespace_facade_aliases).is_none())
    {
        let Some(target_id) = note.class_id.as_deref() else {
            continue;
        };
        let target_id = relation_endpoint_id(namespace_facade_aliases, target_id);
        if model.classes.contains_key(target_id) {
            rows.push(RelationGraphSummaryRow::new("note", "..", target_id));
        }
    }
    Ok(rows)
}

fn relation_endpoint_label(
    label: &str,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Option<RelationGraphLabel>> {
    let label = RelationGraphLabel::try_new(label, width_profile, resources)?;
    if label.as_ref().is_some_and(|label| {
        label.lines().len() == 1 && label.lines()[0].eq_ignore_ascii_case("none")
    }) {
        return Ok(None);
    }
    Ok(label)
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

fn plan_parallel_vertical_relations<'plan>(
    boxes: Vec<&'plan RenderedClassBox>,
    layouts: Vec<&'plan RelationLayout<'_>>,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<RelationRegionPlan<'plan>> {
    let first = layouts
        .first()
        .copied()
        .ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "empty parallel class relationship layout",
        })?;
    let top = relation_graph::find_box_ref(&boxes, first.top_id).ok_or(
        AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "relationships with missing endpoint classes",
        },
    )?;
    let bottom = relation_graph::find_box_ref(&boxes, first.bottom_id).ok_or(
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
            rows.push(class_relation_summary_row(layout)?);
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

fn class_relation_summary_row(layout: &RelationLayout<'_>) -> Result<RelationGraphSummaryRow> {
    Ok(RelationGraphSummaryRow::new(
        layout.top_id,
        class_relation_summary_connector(layout),
        layout.bottom_id,
    )
    .with_label(layout.label.as_ref()))
}

fn class_relation_summary_row_for_reason(
    layout: &RelationLayout<'_>,
    reason: relation_graph::LayeredRelationSummaryReason,
) -> Result<RelationGraphSummaryRow> {
    match reason {
        relation_graph::LayeredRelationSummaryReason::Crossing
        | relation_graph::LayeredRelationSummaryReason::RouteCollision
        | relation_graph::LayeredRelationSummaryReason::OverlayCollision => {
            class_relation_summary_row(layout)
        }
    }
}

fn class_relation_summary_connector(layout: &RelationLayout<'_>) -> String {
    let symbol = class_relation_summary_symbol(layout);
    let top_label = layout
        .top_endpoint_label
        .as_ref()
        .map(endpoint_label_summary_text);
    let bottom_label = layout
        .bottom_endpoint_label
        .as_ref()
        .map(endpoint_label_summary_text);

    match (top_label, bottom_label) {
        (Some(top_label), Some(bottom_label)) => {
            format!("\"{top_label}\" {symbol} \"{bottom_label}\"")
        }
        (Some(top_label), None) => format!("\"{top_label}\" {symbol}"),
        (None, Some(bottom_label)) => format!("{symbol} \"{bottom_label}\""),
        (None, None) => symbol.to_string(),
    }
}

fn endpoint_label_summary_text(label: &RelationGraphLabel) -> String {
    label.lines().join("/")
}

fn class_relation_summary_symbol(layout: &RelationLayout<'_>) -> String {
    let line = match layout.line {
        RelationLine::Solid => "--",
        RelationLine::Dotted => "..",
    };
    let top = layout.top_marker.map_or("", |marker| match marker {
        RelationMarker::Extension => "<|",
        RelationMarker::Dependency => "<",
        RelationMarker::Aggregation => "o",
        RelationMarker::Composition => "*",
        RelationMarker::Lollipop => "()",
    });
    let bottom = layout.bottom_marker.map_or("", |marker| match marker {
        RelationMarker::Extension => "|>",
        RelationMarker::Dependency => ">",
        RelationMarker::Aggregation => "o",
        RelationMarker::Composition => "*",
        RelationMarker::Lollipop => "()",
    });
    format!("{top}{line}{bottom}")
}

fn class_layered_edge(layout: &RelationLayout<'_>) -> LayeredRelationEdge {
    let labels = [
        layout.label.as_ref(),
        layout.top_endpoint_label.as_ref(),
        layout.bottom_endpoint_label.as_ref(),
    ];
    LayeredRelationEdge::new(
        layout.top_id,
        layout.bottom_id,
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

impl<'relation> relation_graph::RelationComponentAdapter<RelationLayout<'relation>>
    for ClassRelationComponentAdapter
{
    fn build_edges(&self, layout: &RelationLayout<'relation>) -> LayeredRelationEdge {
        class_layered_edge(layout)
    }

    fn is_self_relation(&self, layout: &RelationLayout<'relation>) -> bool {
        layout.top_id == layout.bottom_id
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
        resources: &mut ResourceContext,
    ) -> Result<Vec<RelationOverlay>> {
        resources.charge_layout_work(3)?;
        let mut overlays = Vec::new();
        overlays
            .try_reserve_exact(4)
            .map_err(|_| layout_allocation_failed())?;
        if let Some(label) = layout.top_endpoint_label.as_ref() {
            overlays.push(RelationOverlay::label(
                geometry.source_x(),
                geometry
                    .source_marker_y()
                    .checked_sub(label.line_count())
                    .ok_or_else(|| grid_overflow(resources))?,
                label.clone(),
                AsciiColorRole::EdgeLabel,
            ));
        }
        if let Some(label) = layout.label.as_ref() {
            let center_x =
                resources.checked_grid_add(geometry.source_x(), geometry.target_x())? / 2;
            overlays.push(RelationOverlay::label(
                center_x,
                geometry.label_y_after_source(),
                label.clone(),
                AsciiColorRole::EdgeLabel,
            ));
        }

        if let Some(marker) = layout.top_marker {
            overlays.push(RelationOverlay::glyph(
                geometry.source_x(),
                geometry.source_marker_y(),
                marker_char(marker, MarkerSide::Top, self.charset),
                AsciiColorRole::EdgeArrow,
            ));
        }
        if let Some(marker) = layout.bottom_marker {
            overlays.push(RelationOverlay::glyph(
                geometry.target_x(),
                geometry.target_marker_y(),
                marker_char(marker, MarkerSide::Bottom, self.charset),
                AsciiColorRole::EdgeArrow,
            ));
        }

        if let Some(label) = layout.bottom_endpoint_label.as_ref() {
            overlays.push(RelationOverlay::label(
                geometry.target_x(),
                resources.checked_grid_add(geometry.target_marker_y(), 1)?,
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
        let top = relation_graph::find_box_ref(boxes, layout.top_id).ok_or(
            AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "relationships with missing endpoint classes",
            },
        )?;
        let bottom = relation_graph::find_box_ref(boxes, layout.bottom_id).ok_or(
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
    ) -> Result<RelationRegionPlan<'plan>> {
        plan_parallel_vertical_relations(
            boxes,
            layouts,
            options,
            self.charset,
            self.width_profile,
            resources,
        )
    }

    fn build_summary_row(
        &self,
        layout: &RelationLayout<'relation>,
        reason: relation_graph::LayeredRelationSummaryReason,
    ) -> Result<RelationGraphSummaryRow> {
        class_relation_summary_row_for_reason(layout, reason)
    }

    fn layered_error(&self, error: LayeredRelationError) -> AsciiError {
        class_layered_error(error)
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
            let capacity = prefix
                .len()
                .checked_add(line.len())
                .ok_or_else(|| work_overflow(resources))?;
            let mut disclosed = String::new();
            disclosed
                .try_reserve_exact(capacity)
                .map_err(|_| layout_allocation_failed())?;
            disclosed.push_str(prefix);
            disclosed.push_str(line);
            label_lines.push(RelationGraphLine::try_with_role(
                &disclosed,
                AsciiColorRole::EdgeLabel,
                width_profile,
                resources,
            )?);
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
    resources.charge_layout_work(item_count.max(1))?;
    charge_work_product(
        resources,
        model.classes.len(),
        model.namespaces.len().max(1),
    )?;
    charge_work_product(
        resources,
        model.relations.len(),
        model.namespaces.len().max(1),
    )?;
    charge_work_product(
        resources,
        model.namespaces.len(),
        model.namespaces.len().max(1),
    )?;
    Ok(())
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
        charge_text_layout(resources, &relation.relation_title_1)?;
        charge_text_layout(resources, &relation.relation_title_2)?;
    }

    for namespace in model.namespaces.values() {
        charge_text_layout(resources, &namespace.id)?;
        charge_text_layout(resources, &namespace.label)?;
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
    if !member.display_text.is_empty() {
        return charge_text_layout(resources, &member.display_text);
    }

    charge_text_layout(resources, &member.id)?;
    for field in [
        &member.visibility,
        &member.parameters,
        &member.return_type,
        &member.classifier,
    ] {
        if !field.is_empty() {
            charge_text_layout(resources, field)?;
        }
    }
    Ok(())
}

fn charge_work_product(resources: &mut ResourceContext, left: usize, right: usize) -> Result<()> {
    resources.charge_layout_work_product(left, right)
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
    use crate::resource::AsciiResourcePolicy;
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::{Engine, ParseOptions};

    fn resources_with_limit(id: AsciiResourceLimitId, max: usize) -> ResourceContext {
        let policy = AsciiResourcePolicy::default()
            .with_limit(id, max)
            .expect("resource test limit should be valid");
        ResourceContext::new(policy)
    }

    fn resources_with_layout_work_limit(max: usize) -> ResourceContext {
        resources_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, max)
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
