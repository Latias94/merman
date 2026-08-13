use super::{
    CLASS_LEVEL_HORIZONTAL_GAP, ClassCharset, ClassDirection, ClassNoteIndex, RenderedClassBox,
    RenderedClassBoxIndex, external_namespace_note_summary_rows, grid_overflow,
    layout_allocation_failed, nesting_overflow, note_relation_layouts_for_notes,
    relation_endpoint_id, relation_layout, render_class_box, render_class_component_lines,
    render_class_document_lines, render_horizontal_class_component_lines, render_interface_box,
    render_note_box, work_overflow,
};
use crate::color::AsciiColorRole;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::relation_graph::{self, RelationGraphBox, RelationGraphBoxStyle, RelationGraphLine};
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::{normalize_terminal_text, visit_quoted_terminal_text};
use crate::{AsciiError, Result};
use merman_core::entities::decode_html_entities_to_unicode;
use merman_core::models::class_diagram::{ClassDiagram, ClassNode, Namespace};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

struct NamespaceRenderContext<'a> {
    model: &'a ClassDiagram,
    options: &'a AsciiRenderOptions,
    charset: ClassCharset,
    direction: ClassDirection,
    namespace_facade_aliases: &'a HashMap<String, String>,
    scope_index: &'a NamespaceScopeIndex<'a>,
    note_by_id: ClassNoteIndex<'a>,
}

#[derive(Debug)]
struct NamespaceScopeIndex<'a> {
    namespace_parent: HashMap<&'a str, Option<&'a str>>,
    endpoint_owner: HashMap<&'a str, Option<&'a str>>,
    relation_scope: Vec<Option<&'a str>>,
}

#[derive(Debug, Clone, Copy)]
struct ScopedEndpoint<'a> {
    route_id: &'a str,
    facade_member: Option<&'a str>,
}

impl<'a> NamespaceScopeIndex<'a> {
    fn new(
        model: &'a ClassDiagram,
        namespace_facade_aliases: &'a HashMap<String, String>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        resources.transaction(|resources| {
            Self::new_in_transaction(model, namespace_facade_aliases, resources)
        })
    }

    fn new_in_transaction(
        model: &'a ClassDiagram,
        namespace_facade_aliases: &'a HashMap<String, String>,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let namespace_capacity = model.namespaces.len();
        let endpoint_capacity = model
            .classes
            .len()
            .checked_add(model.interfaces.len())
            .ok_or_else(|| work_overflow(resources))?;
        let scope_walk_bound = namespace_capacity
            .checked_add(1)
            .ok_or_else(|| work_overflow(resources))?;
        // Each relation can walk both ancestry chains once to measure their depths, once to align
        // them, and once more to converge. Charge the bounded five-walk envelope before any index
        // container is allocated; the walk itself rejects a cyclic parent chain at the same bound.
        let scope_work_per_relation = scope_walk_bound
            .checked_mul(5)
            .ok_or_else(|| work_overflow(resources))?;
        let index_work = namespace_capacity
            .checked_add(endpoint_capacity)
            .and_then(|value| value.checked_add(model.relations.len()))
            .and_then(|value| {
                value.checked_add(model.relations.len().checked_mul(scope_work_per_relation)?)
            })
            .ok_or_else(|| work_overflow(resources))?;
        resources.charge_layout_work(index_work)?;

        let mut namespace_parent = HashMap::new();
        namespace_parent
            .try_reserve(namespace_capacity)
            .map_err(|_| layout_allocation_failed())?;
        for namespace in model.namespaces.values() {
            namespace_parent.insert(namespace.id.as_str(), namespace.parent.as_deref());
        }

        let mut endpoint_owner = HashMap::new();
        endpoint_owner
            .try_reserve(endpoint_capacity)
            .map_err(|_| layout_allocation_failed())?;
        for class in model.classes.values() {
            endpoint_owner.insert(class.id.as_str(), class.parent.as_deref());
        }
        for interface in &model.interfaces {
            endpoint_owner.insert(interface.id.as_str(), None);
        }

        let mut relation_scope = Vec::new();
        relation_scope
            .try_reserve_exact(model.relations.len())
            .map_err(|_| layout_allocation_failed())?;
        for relation in &model.relations {
            let left = relation_endpoint_id(namespace_facade_aliases, relation.id1.as_str());
            let right = relation_endpoint_id(namespace_facade_aliases, relation.id2.as_str());
            relation_scope.push(Self::least_common_scope(
                endpoint_owner.get(left).copied().flatten(),
                endpoint_owner.get(right).copied().flatten(),
                &namespace_parent,
                scope_walk_bound,
            )?);
        }

        Ok(Self {
            namespace_parent,
            endpoint_owner,
            relation_scope,
        })
    }

    fn least_common_scope(
        left: Option<&'a str>,
        right: Option<&'a str>,
        parent: &HashMap<&'a str, Option<&'a str>>,
        max_hops: usize,
    ) -> Result<Option<&'a str>> {
        let mut left = left;
        let mut right = right;
        let mut left_depth = Self::scope_depth(left, parent, max_hops)?;
        let mut right_depth = Self::scope_depth(right, parent, max_hops)?;

        while left_depth > right_depth {
            left = Self::parent_scope(left, parent)?;
            left_depth -= 1;
        }
        while right_depth > left_depth {
            right = Self::parent_scope(right, parent)?;
            right_depth -= 1;
        }

        for _ in 0..max_hops {
            if left == right {
                return Ok(left);
            }
            left = Self::parent_scope(left, parent)?;
            right = Self::parent_scope(right, parent)?;
        }
        Err(inconsistent_class_namespace_ownership())
    }

    fn scope_depth(
        mut scope: Option<&'a str>,
        parent: &HashMap<&'a str, Option<&'a str>>,
        max_hops: usize,
    ) -> Result<usize> {
        for depth in 0..max_hops {
            let Some(namespace) = scope else {
                return Ok(depth);
            };
            scope = parent
                .get(namespace)
                .copied()
                .ok_or_else(inconsistent_class_namespace_ownership)?;
        }
        Err(inconsistent_class_namespace_ownership())
    }

    fn parent_scope(
        scope: Option<&'a str>,
        parent: &HashMap<&'a str, Option<&'a str>>,
    ) -> Result<Option<&'a str>> {
        let Some(namespace) = scope else {
            return Ok(None);
        };
        parent
            .get(namespace)
            .copied()
            .ok_or_else(inconsistent_class_namespace_ownership)
    }

    fn scope_for_relation(&self, relation_index: usize) -> Option<&'a str> {
        self.relation_scope.get(relation_index).copied().flatten()
    }

    fn endpoint_for_scope(
        &self,
        endpoint_id: &'a str,
        scope: Option<&'a str>,
        resources: &ResourceContext,
    ) -> Result<ScopedEndpoint<'a>> {
        resources.charge_layout_work(1)?;
        let owner = self.endpoint_owner.get(endpoint_id).copied().flatten();
        let Some(owner) = owner else {
            return Ok(ScopedEndpoint {
                route_id: endpoint_id,
                facade_member: None,
            });
        };
        if Some(owner) == scope {
            return Ok(ScopedEndpoint {
                route_id: endpoint_id,
                facade_member: None,
            });
        }

        let mut child = owner;
        while self.namespace_parent.get(child).copied().flatten() != scope {
            resources.charge_layout_work(1)?;
            child = self
                .namespace_parent
                .get(child)
                .copied()
                .flatten()
                .ok_or_else(|| inconsistent_class_namespace_ownership())?;
        }
        Ok(ScopedEndpoint {
            route_id: child,
            facade_member: Some(endpoint_id),
        })
    }
}

fn route_layout_for_scope<'a>(
    layout: super::RelationLayout<'a>,
    scope: Option<&'a str>,
    scope_index: &NamespaceScopeIndex<'a>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<super::RelationLayout<'a>> {
    resources.transaction(|resources| {
        let top = scope_index.endpoint_for_scope(layout.top_id, scope, resources)?;
        let bottom = scope_index.endpoint_for_scope(layout.bottom_id, scope, resources)?;
        let top_facade_member = top
            .facade_member
            .map(|member| frame_facade_member(member, resources))
            .transpose()?;
        let bottom_facade_member = bottom
            .facade_member
            .map(|member| frame_facade_member(member, resources))
            .transpose()?;
        layout.with_route_endpoints(
            top.route_id,
            bottom.route_id,
            top_facade_member.as_deref(),
            bottom_facade_member.as_deref(),
            width_profile,
            resources,
        )
    })
}

fn frame_facade_member(member: &str, resources: &ResourceContext) -> Result<String> {
    resources.transaction(|resources| {
        let work_before_measure = resources.layout_work_used();
        let mut quoted_bytes = 0usize;
        visit_quoted_terminal_text(member, resources, |fragment| {
            quoted_bytes = quoted_bytes
                .checked_add(fragment.len())
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
            Ok(())
        })?;
        let replay_work = resources
            .layout_work_used()
            .checked_sub(work_before_measure)
            .ok_or_else(|| work_overflow(resources))?;
        let output_bytes = "member(bytes="
            .len()
            .checked_add(decimal_digits(member.len()))
            .and_then(|value| value.checked_add(")=".len()))
            .and_then(|value| value.checked_add(quoted_bytes))
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        let materialization_work = output_bytes.max(1);
        resources.check(AsciiResourceLimitId::MaxOutputBytes, output_bytes)?;
        resources.check_usage(
            replay_work
                .checked_add(materialization_work)
                .ok_or_else(|| work_overflow(resources))?,
            0,
        )?;
        resources.charge_layout_work(materialization_work)?;

        let mut framed = String::new();
        framed
            .try_reserve_exact(output_bytes)
            .map_err(|_| layout_allocation_failed())?;
        write!(&mut framed, "member(bytes={})=", member.len())
            .map_err(|_| layout_allocation_failed())?;
        visit_quoted_terminal_text(member, resources, |fragment| {
            framed.push_str(fragment);
            Ok(())
        })?;
        debug_assert_eq!(framed.len(), output_bytes);
        Ok(framed)
    })
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

pub(super) fn has_renderable_namespaces(model: &ClassDiagram) -> bool {
    model.namespaces.values().any(|namespace| {
        !namespace.class_ids.is_empty()
            || !namespace.note_ids.is_empty()
            || model
                .namespaces
                .values()
                .any(|child| child.parent.as_deref() == Some(namespace.id.as_str()))
    })
}

pub(super) fn validate_class_namespace_ownership(
    model: &ClassDiagram,
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(model.namespaces.len())?;
    let mut class_membership_capacity = 0usize;
    let mut note_membership_capacity = 0usize;
    for namespace in model.namespaces.values() {
        class_membership_capacity = class_membership_capacity
            .checked_add(namespace.class_ids.len())
            .ok_or_else(|| work_overflow(resources))?;
        note_membership_capacity = note_membership_capacity
            .checked_add(namespace.note_ids.len())
            .ok_or_else(|| work_overflow(resources))?;
    }
    let validation_work = class_membership_capacity
        .checked_add(note_membership_capacity)
        .and_then(|value| value.checked_add(model.classes.len()))
        .and_then(|value| value.checked_add(model.notes.len()))
        .ok_or_else(|| work_overflow(resources))?;
    resources.charge_layout_work(validation_work)?;

    let mut class_owners = HashMap::new();
    class_owners
        .try_reserve(class_membership_capacity)
        .map_err(|_| layout_allocation_failed())?;
    let mut note_owners = HashMap::new();
    note_owners
        .try_reserve(note_membership_capacity)
        .map_err(|_| layout_allocation_failed())?;
    let mut notes_by_id = HashMap::new();
    notes_by_id
        .try_reserve(model.notes.len())
        .map_err(|_| layout_allocation_failed())?;
    notes_by_id.extend(model.notes.iter().map(|note| (note.id.as_str(), note)));

    for (namespace_key, namespace) in &model.namespaces {
        if namespace_key != &namespace.id
            || namespace
                .parent
                .as_deref()
                .is_some_and(|parent| !model.namespaces.contains_key(parent))
        {
            return Err(inconsistent_class_namespace_ownership());
        }

        for class_id in &namespace.class_ids {
            let Some(class) = model.classes.get(class_id) else {
                return Err(inconsistent_class_namespace_ownership());
            };
            if class.parent.as_deref() != Some(namespace.id.as_str())
                || class_owners
                    .insert(class_id.as_str(), namespace.id.as_str())
                    .is_some()
            {
                return Err(inconsistent_class_namespace_ownership());
            }
        }

        for note_id in &namespace.note_ids {
            let Some(note) = notes_by_id.get(note_id.as_str()).copied() else {
                return Err(inconsistent_class_namespace_ownership());
            };
            if note.parent.as_deref() != Some(namespace.id.as_str())
                || note_owners
                    .insert(note_id.as_str(), namespace.id.as_str())
                    .is_some()
            {
                return Err(inconsistent_class_namespace_ownership());
            }
        }
    }

    for (class_key, class) in &model.classes {
        if class_key != &class.id
            || class_owners.get(class_key.as_str()).copied() != class.parent.as_deref()
        {
            return Err(inconsistent_class_namespace_ownership());
        }
    }
    for note in &model.notes {
        if note_owners.get(note.id.as_str()).copied() != note.parent.as_deref() {
            return Err(inconsistent_class_namespace_ownership());
        }
    }

    Ok(())
}

fn inconsistent_class_namespace_ownership() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "class",
        feature: "inconsistent class namespace ownership",
    }
}

pub(super) fn render_namespaced_class_diagram(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    direction: ClassDirection,
    namespace_facade_aliases: &HashMap<String, String>,
    resources: &mut ResourceContext,
) -> Result<String> {
    let scope_index = NamespaceScopeIndex::new(model, namespace_facade_aliases, resources)?;
    let boxes = render_namespaced_class_boxes(
        model,
        options,
        charset,
        direction,
        namespace_facade_aliases,
        &scope_index,
        resources,
    )?;
    let mut external_layouts = Vec::new();
    external_layouts
        .try_reserve_exact(model.relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for (relation_index, relation) in model.relations.iter().enumerate() {
        if scope_index.scope_for_relation(relation_index).is_some() {
            continue;
        }
        let layout = relation_layout(
            model,
            relation,
            namespace_facade_aliases,
            options.terminal_width_profile,
            resources,
        )?;
        external_layouts.push(route_layout_for_scope(
            layout,
            None,
            &scope_index,
            options.terminal_width_profile,
            resources,
        )?);
    }
    for layout in &mut external_layouts {
        layout.apply_direction(direction);
    }
    let summary_rows = external_namespace_note_summary_rows(model, namespace_facade_aliases)?;

    if summary_rows.is_empty() && external_layouts.is_empty() {
        if direction.is_horizontal() {
            let lines = relation_graph::render_horizontal_box_strip_lines(
                &boxes,
                direction.horizontal_direction(),
                CLASS_LEVEL_HORIZONTAL_GAP,
                options.terminal_width_profile,
                resources,
            )?;
            return render_class_document_lines(lines, options, resources);
        }
        if direction.is_reversed() {
            let lines = relation_graph::stacked_box_lines_ordered(
                &boxes,
                options.terminal_width_profile,
                true,
                resources,
            )?;
            return render_class_document_lines(lines, options, resources);
        }
        return relation_graph::render_stacked_boxes_with_options(&boxes, options, resources);
    }

    let box_by_id = RenderedClassBoxIndex::new(&boxes, resources)?;
    let routed_lines = if direction.is_horizontal() {
        render_horizontal_class_component_lines(
            &boxes,
            &box_by_id,
            &external_layouts,
            direction,
            options,
            charset,
            resources,
        )?
    } else {
        render_class_component_lines(
            &boxes,
            &box_by_id,
            &external_layouts,
            options,
            charset,
            resources,
        )?
    };

    if summary_rows.is_empty() {
        return render_class_document_lines(routed_lines, options, resources);
    }

    let routed_box = RelationGraphBox::from_rendered_lines(
        "namespace::root-relations".to_string(),
        routed_lines,
        options.terminal_width_profile,
        resources,
    )?;
    if direction.is_horizontal() {
        let gap = CLASS_LEVEL_HORIZONTAL_GAP;
        let base_extent = relation_graph::horizontal_box_strip_extent(
            std::slice::from_ref(&routed_box),
            gap,
            resources,
        )?;
        let lines = relation_graph::render_relation_document_with_summary(
            base_extent,
            &summary_rows,
            None,
            options,
            resources,
            |resources| {
                relation_graph::render_horizontal_box_strip_lines(
                    std::slice::from_ref(&routed_box),
                    direction.horizontal_direction(),
                    gap,
                    options.terminal_width_profile,
                    resources,
                )
            },
        )?;
        return render_class_document_lines(lines, options, resources);
    }
    if direction.is_reversed() {
        let base_extent =
            relation_graph::stacked_box_extent(std::slice::from_ref(&routed_box), resources)?;
        let lines = relation_graph::render_relation_document_with_summary(
            base_extent,
            &summary_rows,
            None,
            options,
            resources,
            |resources| {
                relation_graph::stacked_box_lines_ordered(
                    std::slice::from_ref(&routed_box),
                    options.terminal_width_profile,
                    true,
                    resources,
                )
            },
        )?;
        return render_class_document_lines(lines, options, resources);
    }

    relation_graph::render_stacked_boxes_with_relation_summary(
        std::slice::from_ref(&routed_box),
        &summary_rows,
        None,
        options,
        resources,
    )
}
fn render_namespaced_class_boxes(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    direction: ClassDirection,
    namespace_facade_aliases: &HashMap<String, String>,
    scope_index: &NamespaceScopeIndex<'_>,
    resources: &mut ResourceContext,
) -> Result<Vec<RenderedClassBox>> {
    let note_by_id = ClassNoteIndex::new(&model.notes, resources)?;
    let context = NamespaceRenderContext {
        model,
        options,
        charset,
        direction,
        namespace_facade_aliases,
        scope_index,
        note_by_id,
    };
    let mut rendered_namespace_ids = HashSet::new();
    rendered_namespace_ids
        .try_reserve(model.namespaces.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut visiting_namespace_ids = HashSet::new();
    visiting_namespace_ids
        .try_reserve(model.namespaces.len())
        .map_err(|_| layout_allocation_failed())?;
    let mut boxes = Vec::new();
    let box_capacity = model
        .namespaces
        .len()
        .checked_add(model.classes.len())
        .and_then(|value| value.checked_add(model.interfaces.len()))
        .and_then(|value| value.checked_add(model.notes.len()))
        .ok_or_else(|| work_overflow(resources))?;
    boxes
        .try_reserve_exact(box_capacity)
        .map_err(|_| layout_allocation_failed())?;

    for namespace in model
        .namespaces
        .values()
        .filter(|namespace| namespace.parent.is_none())
    {
        boxes.push(render_namespace_box(
            &context,
            namespace,
            &mut rendered_namespace_ids,
            &mut visiting_namespace_ids,
            1,
            resources,
        )?);
    }

    for namespace in model.namespaces.values() {
        if !rendered_namespace_ids.contains(namespace.id.as_str()) {
            boxes.push(render_namespace_box(
                &context,
                namespace,
                &mut rendered_namespace_ids,
                &mut visiting_namespace_ids,
                1,
                resources,
            )?);
        }
    }

    for class in model.classes.values().filter(|class| {
        !namespace_facade_aliases.contains_key(class.id.as_str())
            && class
                .parent
                .as_ref()
                .is_none_or(|parent| !rendered_namespace_ids.contains(parent))
    }) {
        boxes.push(render_class_box(class, options, charset, resources)?);
    }
    for interface in &model.interfaces {
        boxes.push(render_interface_box(
            interface, options, charset, resources,
        )?);
    }
    for note in model.notes.iter().filter(|note| {
        note.parent
            .as_ref()
            .is_none_or(|parent| !rendered_namespace_ids.contains(parent))
    }) {
        boxes.push(render_note_box(note, options, charset, resources)?);
    }

    Ok(boxes)
}

fn render_namespace_box(
    context: &NamespaceRenderContext<'_>,
    namespace: &Namespace,
    rendered_namespace_ids: &mut HashSet<String>,
    visiting_namespace_ids: &mut HashSet<String>,
    depth: usize,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    resources.check_nesting_depth(depth)?;
    resources.charge_layout_work(context.model.namespaces.len().max(1))?;
    if !visiting_namespace_ids.insert(namespace.id.clone()) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "class",
            feature: "cyclic class namespace nesting",
        });
    }
    rendered_namespace_ids.insert(namespace.id.clone());
    let child_capacity = context
        .model
        .namespaces
        .len()
        .checked_add(namespace.class_ids.len())
        .and_then(|value| value.checked_add(namespace.note_ids.len()))
        .ok_or_else(|| work_overflow(resources))?;
    let mut children = Vec::new();
    children
        .try_reserve_exact(child_capacity)
        .map_err(|_| layout_allocation_failed())?;

    for child in context
        .model
        .namespaces
        .values()
        .filter(|child| child.parent.as_deref() == Some(namespace.id.as_str()))
    {
        children.push(render_namespace_box(
            context,
            child,
            rendered_namespace_ids,
            visiting_namespace_ids,
            depth
                .checked_add(1)
                .ok_or_else(|| nesting_overflow(resources))?,
            resources,
        )?);
    }

    let mut direct_boxes = Vec::new();
    let direct_box_capacity = namespace
        .class_ids
        .len()
        .checked_add(namespace.note_ids.len())
        .ok_or_else(|| work_overflow(resources))?;
    direct_boxes
        .try_reserve_exact(direct_box_capacity)
        .map_err(|_| layout_allocation_failed())?;
    for class_id in &namespace.class_ids {
        if let Some(class) = context.model.classes.get(class_id)
            && !context
                .namespace_facade_aliases
                .contains_key(class.id.as_str())
        {
            direct_boxes.push(render_class_box(
                class,
                context.options,
                context.charset,
                resources,
            )?);
        }
    }

    for note_id in &namespace.note_ids {
        if let Some(note) = context.note_by_id.get(note_id, resources)? {
            direct_boxes.push(render_note_box(
                note,
                context.options,
                context.charset,
                resources,
            )?);
        }
    }
    let direct_layout_capacity = context
        .model
        .relations
        .len()
        .checked_add(context.model.notes.len())
        .ok_or_else(|| work_overflow(resources))?;
    let mut direct_layouts = Vec::new();
    direct_layouts
        .try_reserve_exact(direct_layout_capacity)
        .map_err(|_| layout_allocation_failed())?;
    for (relation_index, relation) in context.model.relations.iter().enumerate() {
        if context.scope_index.scope_for_relation(relation_index) != Some(namespace.id.as_str()) {
            continue;
        }
        let layout = relation_layout(
            context.model,
            relation,
            context.namespace_facade_aliases,
            context.options.terminal_width_profile,
            resources,
        )?;
        direct_layouts.push(route_layout_for_scope(
            layout,
            Some(namespace.id.as_str()),
            context.scope_index,
            context.options.terminal_width_profile,
            resources,
        )?);
    }

    let mut scope_boxes = Vec::new();
    scope_boxes
        .try_reserve_exact(
            children
                .len()
                .checked_add(direct_boxes.len())
                .ok_or_else(|| work_overflow(resources))?,
        )
        .map_err(|_| layout_allocation_failed())?;
    scope_boxes.extend(children.iter().cloned());
    scope_boxes.extend(direct_boxes.iter().cloned());
    let box_by_id = RenderedClassBoxIndex::new(&scope_boxes, resources)?;
    direct_layouts.extend(note_relation_layouts_for_notes(
        context.model.notes.iter().filter(|note| {
            note.parent.as_deref() == Some(namespace.id.as_str()) && note.class_id.is_some()
        }),
        context.namespace_facade_aliases,
        &box_by_id,
        resources,
    )?);
    for layout in &mut direct_layouts {
        layout.apply_direction(context.direction);
    }

    if direct_layouts.is_empty() {
        children.extend(direct_boxes);
    } else {
        let component_lines = if context.direction.is_horizontal() {
            render_horizontal_class_component_lines(
                &scope_boxes,
                &box_by_id,
                &direct_layouts,
                context.direction,
                context.options,
                context.charset,
                resources,
            )?
        } else {
            render_class_component_lines(
                &scope_boxes,
                &box_by_id,
                &direct_layouts,
                context.options,
                context.charset,
                resources,
            )?
        };
        children.push(RelationGraphBox::from_rendered_lines(
            format!("{}::relations", namespace.id),
            component_lines,
            context.options.terminal_width_profile,
            resources,
        )?);
    }

    visiting_namespace_ids.remove(namespace.id.as_str());
    render_namespace_container_box(
        namespace,
        children,
        context.options,
        context.charset,
        context.direction,
        resources,
    )
}

pub(super) fn render_namespace_container_box(
    namespace: &Namespace,
    mut children: Vec<RenderedClassBox>,
    options: &AsciiRenderOptions,
    charset: ClassCharset,
    direction: ClassDirection,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    if direction.is_horizontal() && children.len() > 1 {
        let lines = relation_graph::render_horizontal_box_strip_lines(
            &children,
            direction.horizontal_direction(),
            CLASS_LEVEL_HORIZONTAL_GAP,
            options.terminal_width_profile,
            resources,
        )?;
        let contents = RelationGraphBox::from_rendered_lines(
            format!("{}::contents", namespace.id),
            lines,
            options.terminal_width_profile,
            resources,
        )?;
        children.clear();
        children.push(contents);
    } else if direction == ClassDirection::BottomUp {
        children.reverse();
    }

    let title = namespace_title(namespace);
    let inner_gap = options.box_border_padding;
    let inner_width = children
        .iter()
        .map(RelationGraphBox::width)
        .max()
        .unwrap_or_else(|| {
            crate::text::display_width_with_profile(&title, options.terminal_width_profile)
        })
        .max(crate::text::display_width_with_profile(
            &title,
            options.terminal_width_profile,
        ));
    let content_width = resources.checked_grid_add(
        inner_width,
        resources.checked_grid_mul(options.box_border_padding, 2)?,
    )?;
    let child_rows = children.iter().try_fold(0usize, |height, child| {
        resources.checked_grid_add(height, child.height())
    })?;
    let child_gaps = children.len().saturating_sub(1);
    let body_rows = if children.is_empty() {
        1
    } else {
        resources.checked_grid_add(child_rows, child_gaps)?
    };
    let height = resources.checked_grid_add(body_rows, 4)?;
    let width = resources.checked_grid_add(content_width, 2)?;
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;
    let style = RelationGraphBoxStyle {
        top_left: charset.top_left,
        top_right: charset.top_right,
        bottom_left: charset.bottom_left,
        bottom_right: charset.bottom_right,
        horizontal: charset.horizontal,
        vertical: charset.vertical,
        separator_left: charset.separator_left,
        separator_right: charset.separator_right,
        border_role: AsciiColorRole::GroupBorder,
        text_role: AsciiColorRole::Text,
    };
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(height)
        .map_err(|_| layout_allocation_failed())?;
    lines.push(RelationGraphLine::try_box_border(
        style.top_left,
        style.top_right,
        style.horizontal,
        content_width,
        style.border_role,
        options.terminal_width_profile,
        resources,
    )?);
    lines.push(RelationGraphLine::box_content(
        &title,
        content_width,
        options.box_border_padding,
        style,
        options.terminal_width_profile,
        resources,
    )?);
    lines.push(RelationGraphLine::try_box_border(
        style.separator_left,
        style.separator_right,
        style.horizontal,
        content_width,
        style.border_role,
        options.terminal_width_profile,
        resources,
    )?);

    for (child_index, child) in children.iter().enumerate() {
        if child_index > 0 {
            lines.push(namespace_empty_content_line(
                content_width,
                style.vertical,
                style.border_role,
                options.terminal_width_profile,
                resources,
            )?);
        }
        lines.extend(namespace_child_lines(
            child,
            content_width,
            style.vertical,
            style.border_role,
            inner_gap,
            resources,
        )?);
    }

    if children.is_empty() {
        lines.push(namespace_empty_content_line(
            content_width,
            style.vertical,
            style.border_role,
            options.terminal_width_profile,
            resources,
        )?);
    }

    lines.push(RelationGraphLine::try_box_border(
        style.bottom_left,
        style.bottom_right,
        style.horizontal,
        content_width,
        style.border_role,
        options.terminal_width_profile,
        resources,
    )?);

    Ok(RelationGraphBox::new_with_lines(
        namespace.id.clone(),
        lines,
        width,
        options.terminal_width_profile,
    ))
}

fn namespace_child_lines(
    child: &RenderedClassBox,
    content_width: usize,
    vertical: char,
    border_role: AsciiColorRole,
    inner_gap: usize,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let used_width = resources.checked_grid_add(inner_gap, child.width())?;
    let trailing = content_width
        .checked_sub(used_width)
        .ok_or_else(|| grid_overflow(resources))?;
    let mut lines = Vec::new();
    lines
        .try_reserve_exact(child.height())
        .map_err(|_| layout_allocation_failed())?;
    let vertical_text = vertical.to_string();
    for line in child.lines() {
        let mut parts = Vec::new();
        parts
            .try_reserve_exact(5)
            .map_err(|_| layout_allocation_failed())?;
        parts.push(RelationGraphLine::try_with_role(
            &vertical_text,
            border_role,
            child.width_profile(),
            resources,
        )?);
        parts.push(RelationGraphLine::try_blank(
            inner_gap,
            child.width_profile(),
            resources,
        )?);
        parts.push(line.clone());
        parts.push(RelationGraphLine::try_blank(
            trailing,
            child.width_profile(),
            resources,
        )?);
        parts.push(RelationGraphLine::try_with_role(
            &vertical_text,
            border_role,
            child.width_profile(),
            resources,
        )?);
        lines.push(relation_graph::try_concat_relation_lines(
            parts,
            child.width_profile(),
            resources,
        )?);
    }
    Ok(lines)
}

fn namespace_empty_content_line(
    content_width: usize,
    vertical: char,
    border_role: AsciiColorRole,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<RelationGraphLine> {
    RelationGraphLine::try_box_border(
        vertical,
        vertical,
        ' ',
        content_width,
        border_role,
        width_profile,
        resources,
    )
}

fn namespace_title(namespace: &Namespace) -> String {
    let normalized_label = normalize_terminal_text(&namespace.label);
    let raw = if normalized_label.trim().is_empty() {
        namespace.id.as_str()
    } else {
        normalized_label.as_ref()
    };
    decode_html_entities_to_unicode(raw).into_owned()
}

pub(super) fn namespace_facade_aliases(model: &ClassDiagram) -> Result<HashMap<String, String>> {
    let mut aliases = HashMap::new();
    aliases
        .try_reserve(model.classes.len())
        .map_err(|_| layout_allocation_failed())?;
    for class in model.classes.values() {
        let Some(local_id) = namespace_facade_local_id(model, class) else {
            continue;
        };
        aliases.insert(class.id.clone(), local_id.to_string());
    }
    Ok(aliases)
}

fn namespace_facade_local_id<'a>(model: &'a ClassDiagram, class: &'a ClassNode) -> Option<&'a str> {
    if class
        .parent
        .as_deref()
        .map(str::trim)
        .is_some_and(|parent| !parent.is_empty())
        || !class.annotations.is_empty()
        || !class.members.is_empty()
        || !class.methods.is_empty()
    {
        return None;
    }

    model
        .namespaces
        .values()
        .filter_map(|namespace| {
            let remainder = class
                .id
                .strip_prefix(namespace.id.as_str())?
                .strip_prefix('.')?;
            namespace
                .class_ids
                .iter()
                .any(|id| id == remainder)
                .then_some((namespace.id.len(), remainder))
        })
        .max_by_key(|(namespace_len, _)| *namespace_len)
        .and_then(|(_, local_id)| model.classes.contains_key(local_id).then_some(local_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::{Engine, ParseOptions};

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
    fn namespace_scope_index_admits_exact_work_and_rolls_back_n_minus_one() {
        let model = parsed_class_model(
            "classDiagram\nnamespace Platform {\n  namespace FFI {\n    class Dart\n  }\n  namespace Core {\n    class Renderer\n  }\n}\nDart --> Renderer",
        );
        let aliases = namespace_facade_aliases(&model).expect("aliases should build");
        let unbounded = AsciiResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        let mut measured = ResourceContext::new(unbounded);
        NamespaceScopeIndex::new(&model, &aliases, &mut measured)
            .expect("unbounded namespace index should build");
        let exact_work = measured.layout_work_used();
        assert!(exact_work > 1);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact namespace-index work limit should be valid");
        let mut exact = ResourceContext::new(exact_policy);
        NamespaceScopeIndex::new(&model, &aliases, &mut exact)
            .expect("exact namespace-index work should build");
        assert_eq!(exact.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("max-minus-one namespace-index work limit should be valid");
        let mut below = ResourceContext::new(below_policy);
        below
            .charge_layout_work(1)
            .expect("test checkpoint should be admitted");
        let checkpoint = below.layout_work_used();
        let error = NamespaceScopeIndex::new(&model, &aliases, &mut below)
            .expect_err("max-minus-one namespace-index work must reject");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual > details.max
        ));
        assert_eq!(below.layout_work_used(), checkpoint);
        assert_eq!(below.document_cells_used(), 0);
    }

    #[test]
    fn namespace_scope_index_rejects_cyclic_parent_chains_before_render_descent() {
        let mut model = parsed_class_model(
            "classDiagram\nnamespace Left {\n  class A\n}\nnamespace Right {\n  class B\n}\nA --> B",
        );
        model
            .namespaces
            .get_mut("Left")
            .expect("Left namespace should exist")
            .parent = Some("Right".to_string());
        model
            .namespaces
            .get_mut("Right")
            .expect("Right namespace should exist")
            .parent = Some("Left".to_string());
        let aliases = namespace_facade_aliases(&model).expect("aliases should build");
        let mut resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        ));

        let error = NamespaceScopeIndex::new(&model, &aliases, &mut resources)
            .expect_err("cyclic namespace parents must reject while building the scope index");
        assert!(matches!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "class",
                feature: "inconsistent class namespace ownership",
            }
        ));
        assert_eq!(resources.layout_work_used(), 0);
        assert_eq!(resources.document_cells_used(), 0);
    }
}
