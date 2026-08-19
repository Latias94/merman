use super::{
    CLASS_LEVEL_HORIZONTAL_GAP, ClassDirection, ClassEndpointIndex, ClassNoteIndex,
    ClassRelationLabels, ClassRenderSettings, RenderedClassBox, RenderedClassBoxIndex,
    authored_display_projection_is_lossy, external_namespace_note_summary_rows, grid_overflow,
    layout_allocation_failed, nesting_overflow, note_relation_layouts_for_notes, relation_layout,
    render_class_box, render_class_component_lines, render_class_document_lines_with_execution,
    render_horizontal_class_component_lines, render_interface_box, render_note_box, work_overflow,
};
use crate::color::AsciiColorRole;
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
use crate::relation_graph::{self, RelationGraphBox, RelationGraphBoxStyle, RelationGraphLine};
use crate::resource::ResourceContext;
use crate::safe_text::{ComposedTextPlan, DeferredTextRegistry, terminal_text_is_blank};
use crate::{AsciiError, Result};
use merman_core::OperationPhase;
use merman_core::models::class_diagram::{ClassDiagram, ClassInterface, Namespace};
use std::collections::HashMap;

struct NamespaceRenderContext<'model, 'render> {
    model: &'model ClassDiagram,
    settings: ClassRenderSettings<'render>,
    endpoint_index: &'render ClassEndpointIndex<'model>,
    interface_index: &'render NamespaceInterfaceIndex<'model>,
    render_plan: &'render NamespaceRenderPlan<'model>,
    scope_index: &'render NamespaceScopeIndex<'model>,
    note_by_id: ClassNoteIndex<'model>,
    relation_labels: &'render [ClassRelationLabels],
    execution: AsciiExecution<'render>,
}

#[derive(Debug)]
struct NamespaceInterfaceIndex<'a> {
    by_owner: HashMap<&'a str, Vec<&'a ClassInterface>>,
    root: Vec<&'a ClassInterface>,
}

impl<'a> NamespaceInterfaceIndex<'a> {
    fn new(
        model: &'a ClassDiagram,
        endpoint_index: &ClassEndpointIndex<'a>,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        resources.transaction(|resources| {
            checkpoint_layout(execution)?;
            let index_work = model
                .interfaces
                .len()
                .checked_mul(2)
                .ok_or_else(|| work_overflow(resources))?;
            resources.charge_layout_work(index_work)?;

            let mut owner_counts = HashMap::new();
            owner_counts
                .try_reserve(model.interfaces.len())
                .map_err(|_| layout_allocation_failed())?;
            let mut root_count = 0usize;
            for interface in &model.interfaces {
                checkpoint_layout(execution)?;
                let endpoint = endpoint_index
                    .resolve(interface.id.as_str())
                    .ok_or_else(inconsistent_class_namespace_ownership)?;
                if let Some(owner) = endpoint.owner {
                    let count = owner_counts.entry(owner).or_insert(0usize);
                    *count = count
                        .checked_add(1)
                        .ok_or_else(|| work_overflow(resources))?;
                } else {
                    root_count = root_count
                        .checked_add(1)
                        .ok_or_else(|| work_overflow(resources))?;
                }
            }

            let mut by_owner = HashMap::new();
            by_owner
                .try_reserve(owner_counts.len())
                .map_err(|_| layout_allocation_failed())?;
            for (owner, count) in owner_counts {
                let mut interfaces = Vec::new();
                interfaces
                    .try_reserve_exact(count)
                    .map_err(|_| layout_allocation_failed())?;
                by_owner.insert(owner, interfaces);
            }
            let mut root = Vec::new();
            root.try_reserve_exact(root_count)
                .map_err(|_| layout_allocation_failed())?;

            for interface in &model.interfaces {
                checkpoint_layout(execution)?;
                let endpoint = endpoint_index
                    .resolve(interface.id.as_str())
                    .ok_or_else(inconsistent_class_namespace_ownership)?;
                if let Some(owner) = endpoint.owner {
                    by_owner
                        .get_mut(owner)
                        .ok_or_else(inconsistent_class_namespace_ownership)?
                        .push(interface);
                } else {
                    root.push(interface);
                }
            }

            Ok(Self { by_owner, root })
        })
    }

    fn for_owner(&self, owner: Option<&str>) -> &[&'a ClassInterface] {
        match owner {
            Some(owner) => self
                .by_owner
                .get(owner)
                .map(Vec::as_slice)
                .unwrap_or_default(),
            None => self.root.as_slice(),
        }
    }
}

#[derive(Debug)]
struct NamespaceRenderPlan<'a> {
    children: HashMap<&'a str, Vec<&'a Namespace>>,
    roots: Vec<&'a Namespace>,
    postorder: Vec<&'a Namespace>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NamespaceVisitState {
    Visiting,
    Rendered,
}

impl<'a> NamespaceRenderPlan<'a> {
    fn new(
        model: &'a ClassDiagram,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        resources.transaction(|resources| Self::new_in_transaction(model, resources, execution))
    }

    fn new_in_transaction(
        model: &'a ClassDiagram,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        checkpoint_layout(execution)?;
        let namespace_count = model.namespaces.len();
        let stack_capacity = namespace_count
            .checked_mul(2)
            .ok_or_else(|| work_overflow(resources))?;
        let plan_work = namespace_count
            .checked_mul(5)
            .ok_or_else(|| work_overflow(resources))?;
        resources.charge_layout_work(plan_work)?;

        let mut child_counts = HashMap::new();
        child_counts
            .try_reserve(namespace_count)
            .map_err(|_| layout_allocation_failed())?;
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(namespace_count)
            .map_err(|_| layout_allocation_failed())?;
        for namespace in model.namespaces.values() {
            checkpoint_layout(execution)?;
            if let Some(parent) = namespace.parent.as_deref() {
                let count = child_counts.entry(parent).or_insert(0usize);
                *count = count
                    .checked_add(1)
                    .ok_or_else(|| work_overflow(resources))?;
            } else {
                roots.push(namespace);
            }
        }

        let mut children = HashMap::new();
        children
            .try_reserve(child_counts.len())
            .map_err(|_| layout_allocation_failed())?;
        for (parent, child_count) in child_counts {
            let mut entries = Vec::new();
            entries
                .try_reserve_exact(child_count)
                .map_err(|_| layout_allocation_failed())?;
            children.insert(parent, entries);
        }
        for namespace in model.namespaces.values() {
            checkpoint_layout(execution)?;
            if let Some(parent) = namespace.parent.as_deref() {
                children
                    .get_mut(parent)
                    .ok_or_else(inconsistent_class_namespace_ownership)?
                    .push(namespace);
            }
        }

        let mut states = HashMap::new();
        states
            .try_reserve(namespace_count)
            .map_err(|_| layout_allocation_failed())?;
        let mut postorder = Vec::new();
        postorder
            .try_reserve_exact(namespace_count)
            .map_err(|_| layout_allocation_failed())?;
        let mut stack = Vec::new();
        stack
            .try_reserve_exact(stack_capacity)
            .map_err(|_| layout_allocation_failed())?;

        for root in &roots {
            stack.push((*root, false, 1usize));
            while let Some((namespace, expanded, depth)) = stack.pop() {
                checkpoint_layout(execution)?;
                if expanded {
                    states.insert(namespace.id.as_str(), NamespaceVisitState::Rendered);
                    postorder.push(namespace);
                    continue;
                }
                match states.get(namespace.id.as_str()).copied() {
                    Some(NamespaceVisitState::Visiting) => {
                        return Err(inconsistent_class_namespace_ownership());
                    }
                    Some(NamespaceVisitState::Rendered) => continue,
                    None => {}
                }
                resources.check_nesting_depth(depth)?;
                states.insert(namespace.id.as_str(), NamespaceVisitState::Visiting);
                stack.push((namespace, true, depth));
                if let Some(child_namespaces) = children.get(namespace.id.as_str()) {
                    let child_depth = depth
                        .checked_add(1)
                        .ok_or_else(|| nesting_overflow(resources))?;
                    for child in child_namespaces.iter().rev() {
                        stack.push((*child, false, child_depth));
                    }
                }
            }
        }

        if postorder.len() != namespace_count {
            return Err(inconsistent_class_namespace_ownership());
        }

        Ok(Self {
            children,
            roots,
            postorder,
        })
    }

    fn children(&self, namespace_id: &str) -> &[&'a Namespace] {
        self.children
            .get(namespace_id)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
}

#[derive(Debug)]
struct NamespaceScopeIndex<'a> {
    namespace_parent: HashMap<&'a str, Option<&'a str>>,
    namespace_route_id: HashMap<&'a str, &'a str>,
    relation_scope: Vec<Option<&'a str>>,
}

impl<'a> NamespaceScopeIndex<'a> {
    fn new(
        model: &'a ClassDiagram,
        endpoint_index: &ClassEndpointIndex<'a>,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        resources.transaction(|resources| {
            Self::new_in_transaction(model, endpoint_index, resources, execution)
        })
    }

    fn new_in_transaction(
        model: &'a ClassDiagram,
        endpoint_index: &ClassEndpointIndex<'a>,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        checkpoint_layout(execution)?;
        let namespace_capacity = model.namespaces.len();
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
            .checked_mul(2)
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
            checkpoint_layout(execution)?;
            namespace_parent.insert(namespace.id.as_str(), namespace.parent.as_deref());
        }
        let mut namespace_route_id = HashMap::new();
        namespace_route_id
            .try_reserve(namespace_capacity)
            .map_err(|_| layout_allocation_failed())?;
        for namespace in model.namespaces.values() {
            checkpoint_layout(execution)?;
            namespace_route_id.insert(namespace.id.as_str(), namespace.dom_id.as_str());
        }

        let mut relation_scope = Vec::new();
        relation_scope
            .try_reserve_exact(model.relations.len())
            .map_err(|_| layout_allocation_failed())?;
        for relation in &model.relations {
            checkpoint_layout(execution)?;
            let left = endpoint_index.resolve(relation.id1.as_str()).ok_or(
                AsciiError::UnsupportedFeature {
                    diagram_type: "class",
                    feature: "relationships with missing endpoint classes",
                },
            )?;
            let right = endpoint_index.resolve(relation.id2.as_str()).ok_or(
                AsciiError::UnsupportedFeature {
                    diagram_type: "class",
                    feature: "relationships with missing endpoint classes",
                },
            )?;
            relation_scope.push(Self::least_common_scope(
                left.owner,
                right.owner,
                &namespace_parent,
                scope_walk_bound,
                execution,
            )?);
        }

        Ok(Self {
            namespace_parent,
            namespace_route_id,
            relation_scope,
        })
    }

    fn least_common_scope(
        left: Option<&'a str>,
        right: Option<&'a str>,
        parent: &HashMap<&'a str, Option<&'a str>>,
        max_hops: usize,
        execution: AsciiExecution<'_>,
    ) -> Result<Option<&'a str>> {
        let mut left = left;
        let mut right = right;
        let mut left_depth = Self::scope_depth(left, parent, max_hops, execution)?;
        let mut right_depth = Self::scope_depth(right, parent, max_hops, execution)?;

        while left_depth > right_depth {
            checkpoint_layout(execution)?;
            left = Self::parent_scope(left, parent)?;
            left_depth -= 1;
        }
        while right_depth > left_depth {
            checkpoint_layout(execution)?;
            right = Self::parent_scope(right, parent)?;
            right_depth -= 1;
        }

        for _ in 0..max_hops {
            checkpoint_layout(execution)?;
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
        execution: AsciiExecution<'_>,
    ) -> Result<usize> {
        for depth in 0..max_hops {
            checkpoint_layout(execution)?;
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
        endpoint: super::RelationEndpoint<'a>,
        scope: Option<&'a str>,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<super::RelationRouteEndpoint<'a>> {
        checkpoint_layout(execution)?;
        resources.charge_layout_work(1)?;
        let Some(owner) = endpoint.owner else {
            return Ok(super::RelationRouteEndpoint {
                render_id: endpoint.resolved_id,
                facade_member: None,
            });
        };
        if Some(owner) == scope {
            return Ok(super::RelationRouteEndpoint {
                render_id: endpoint.resolved_id,
                facade_member: None,
            });
        }

        let mut child = owner;
        while self.namespace_parent.get(child).copied().flatten() != scope {
            checkpoint_layout(execution)?;
            resources.charge_layout_work(1)?;
            child = self
                .namespace_parent
                .get(child)
                .copied()
                .flatten()
                .ok_or_else(inconsistent_class_namespace_ownership)?;
        }
        Ok(super::RelationRouteEndpoint {
            render_id: self
                .namespace_route_id
                .get(child)
                .copied()
                .ok_or_else(inconsistent_class_namespace_ownership)?,
            facade_member: Some(endpoint.resolved_id),
        })
    }
}

fn route_layout_for_scope<'a>(
    layout: super::RelationLayout<'a>,
    scope: Option<&'a str>,
    scope_index: &NamespaceScopeIndex<'a>,
    width_profile: TerminalWidthProfile,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<super::RelationLayout<'a>> {
    resources.transaction(|resources| {
        let top = scope_index.endpoint_for_scope(layout.top, scope, resources, execution)?;
        let bottom = scope_index.endpoint_for_scope(layout.bottom, scope, resources, execution)?;
        layout.with_render_endpoints(top, bottom, width_profile, deferred_text, resources)
    })
}

pub(super) fn has_renderable_namespaces(model: &ClassDiagram) -> bool {
    model.namespaces.values().any(|namespace| {
        !namespace.class_ids.is_empty()
            || !namespace.note_ids.is_empty()
            || namespace.parent.is_some()
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
        .and_then(|value| value.checked_add(model.namespace_facade_aliases.len()))
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

    for (facade_id, target_id) in &model.namespace_facade_aliases {
        let Some(facade) = model.classes.get(facade_id) else {
            return Err(inconsistent_class_namespace_ownership());
        };
        let Some(target) = model.classes.get(target_id) else {
            return Err(inconsistent_class_namespace_ownership());
        };
        let Some(parent) = target.parent.as_deref() else {
            return Err(inconsistent_class_namespace_ownership());
        };
        if facade_id == target_id
            || facade.parent.is_some()
            || facade_id
                .strip_prefix(parent)
                .and_then(|remainder| remainder.strip_prefix('.'))
                != Some(target_id.as_str())
            || class_owners.get(target_id.as_str()).copied() != Some(parent)
        {
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

pub(super) fn render_namespaced_class_diagram<'a>(
    model: &'a ClassDiagram,
    settings: ClassRenderSettings<'_>,
    endpoint_index: &ClassEndpointIndex<'a>,
    relation_labels: &[ClassRelationLabels],
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    checkpoint_layout(execution)?;
    let render_plan = NamespaceRenderPlan::new(model, resources, execution)?;
    let scope_index = NamespaceScopeIndex::new(model, endpoint_index, resources, execution)?;
    let interface_index =
        NamespaceInterfaceIndex::new(model, endpoint_index, resources, execution)?;
    let note_by_id = ClassNoteIndex::new(&model.notes, resources)?;
    let context = NamespaceRenderContext {
        model,
        settings,
        endpoint_index,
        interface_index: &interface_index,
        render_plan: &render_plan,
        scope_index: &scope_index,
        note_by_id,
        relation_labels,
        execution,
    };
    let boxes = render_namespaced_class_boxes(&context, deferred_text, resources)?;
    let mut external_layouts = Vec::new();
    external_layouts
        .try_reserve_exact(model.relations.len())
        .map_err(|_| layout_allocation_failed())?;
    for (relation_index, relation) in model.relations.iter().enumerate() {
        checkpoint_layout(execution)?;
        if scope_index.scope_for_relation(relation_index).is_some() {
            continue;
        }
        let layout = relation_layout(
            model,
            relation,
            endpoint_index,
            relation_labels
                .get(relation_index)
                .cloned()
                .ok_or_else(layout_allocation_failed)?,
        )?;
        external_layouts.push(route_layout_for_scope(
            layout,
            None,
            &scope_index,
            settings.options.terminal_width_profile,
            deferred_text,
            resources,
            execution,
        )?);
    }
    for layout in &mut external_layouts {
        checkpoint_layout(execution)?;
        layout.apply_direction(settings.direction);
    }
    let summary_rows = external_namespace_note_summary_rows(
        model,
        endpoint_index,
        settings.options.terminal_width_profile,
        deferred_text,
        resources,
    )?;

    if summary_rows.is_empty() && external_layouts.is_empty() {
        if settings.direction.is_horizontal() {
            let lines = relation_graph::render_horizontal_box_strip_lines(
                &boxes,
                settings.direction.horizontal_direction(),
                CLASS_LEVEL_HORIZONTAL_GAP,
                settings.options.terminal_width_profile,
                resources,
            )?;
            return render_class_document_lines_with_execution(
                lines,
                settings.options,
                resources,
                deferred_text,
                execution,
            );
        }
        if settings.direction.is_reversed() {
            let lines = relation_graph::stacked_box_lines_ordered(
                &boxes,
                settings.options.terminal_width_profile,
                true,
                resources,
            )?;
            return render_class_document_lines_with_execution(
                lines,
                settings.options,
                resources,
                deferred_text,
                execution,
            );
        }
        return relation_graph::render_stacked_boxes_with_deferred_options_with_execution(
            &boxes,
            settings.options,
            resources,
            deferred_text,
            execution,
        );
    }

    let box_by_id = RenderedClassBoxIndex::new(&boxes, resources)?;
    let routed_lines = if settings.direction.is_horizontal() {
        render_horizontal_class_component_lines(
            &boxes,
            &external_layouts,
            settings,
            resources,
            deferred_text,
            execution,
        )?
    } else {
        render_class_component_lines(
            &boxes,
            &box_by_id,
            &external_layouts,
            settings,
            resources,
            deferred_text,
            execution,
        )?
    };

    if summary_rows.is_empty() {
        return render_class_document_lines_with_execution(
            routed_lines,
            settings.options,
            resources,
            deferred_text,
            execution,
        );
    }

    let routed_box = RelationGraphBox::from_rendered_lines(
        "namespace::root-relations".to_string(),
        routed_lines,
        settings.options.terminal_width_profile,
        resources,
    )?;
    if settings.direction.is_horizontal() {
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
            settings.options,
            resources,
            |resources| {
                relation_graph::render_horizontal_box_strip_lines(
                    std::slice::from_ref(&routed_box),
                    settings.direction.horizontal_direction(),
                    gap,
                    settings.options.terminal_width_profile,
                    resources,
                )
            },
        )?;
        return render_class_document_lines_with_execution(
            lines,
            settings.options,
            resources,
            deferred_text,
            execution,
        );
    }
    if settings.direction.is_reversed() {
        let base_extent =
            relation_graph::stacked_box_extent(std::slice::from_ref(&routed_box), resources)?;
        let lines = relation_graph::render_relation_document_with_summary(
            base_extent,
            &summary_rows,
            None,
            settings.options,
            resources,
            |resources| {
                relation_graph::stacked_box_lines_ordered(
                    std::slice::from_ref(&routed_box),
                    settings.options.terminal_width_profile,
                    true,
                    resources,
                )
            },
        )?;
        return render_class_document_lines_with_execution(
            lines,
            settings.options,
            resources,
            deferred_text,
            execution,
        );
    }

    let lines = relation_graph::render_relation_document_with_summary(
        relation_graph::stacked_box_extent(std::slice::from_ref(&routed_box), resources)?,
        &summary_rows,
        None,
        settings.options,
        resources,
        |resources| {
            relation_graph::stacked_box_lines(
                std::slice::from_ref(&routed_box),
                settings.options.terminal_width_profile,
                resources,
            )
        },
    )?;
    render_class_document_lines_with_execution(
        lines,
        settings.options,
        resources,
        deferred_text,
        execution,
    )
}
fn render_namespaced_class_boxes<'a>(
    context: &NamespaceRenderContext<'a, '_>,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &mut ResourceContext,
) -> Result<Vec<RenderedClassBox>> {
    let mut rendered_namespaces = HashMap::new();
    rendered_namespaces
        .try_reserve(context.model.namespaces.len())
        .map_err(|_| layout_allocation_failed())?;
    for namespace in &context.render_plan.postorder {
        checkpoint_layout(context.execution)?;
        let child_namespaces = context.render_plan.children(namespace.id.as_str());
        let mut children = Vec::new();
        children
            .try_reserve_exact(child_namespaces.len())
            .map_err(|_| layout_allocation_failed())?;
        for child in child_namespaces {
            children.push(
                rendered_namespaces
                    .remove(child.id.as_str())
                    .ok_or_else(inconsistent_class_namespace_ownership)?,
            );
        }
        let rendered =
            render_namespace_box(context, namespace, children, deferred_text, resources)?;
        rendered_namespaces.insert(namespace.id.as_str(), rendered);
    }

    let mut boxes = Vec::new();
    let box_capacity = context
        .render_plan
        .roots
        .len()
        .checked_add(context.model.classes.len())
        .and_then(|value| value.checked_add(context.model.interfaces.len()))
        .and_then(|value| value.checked_add(context.model.notes.len()))
        .ok_or_else(|| work_overflow(resources))?;
    boxes
        .try_reserve_exact(box_capacity)
        .map_err(|_| layout_allocation_failed())?;

    for namespace in &context.render_plan.roots {
        checkpoint_layout(context.execution)?;
        boxes.push(
            rendered_namespaces
                .remove(namespace.id.as_str())
                .ok_or_else(inconsistent_class_namespace_ownership)?,
        );
    }
    if !rendered_namespaces.is_empty() {
        return Err(inconsistent_class_namespace_ownership());
    }

    for class in context.model.classes.values().filter(|class| {
        !context.endpoint_index.is_facade(class.id.as_str()) && class.parent.is_none()
    }) {
        checkpoint_layout(context.execution)?;
        boxes.push(render_class_box(
            class,
            context.settings.options,
            context.settings.charset,
            deferred_text,
            resources,
        )?);
    }
    for interface in context.interface_index.for_owner(None) {
        checkpoint_layout(context.execution)?;
        boxes.push(render_interface_box(
            interface,
            context.settings.options,
            context.settings.charset,
            deferred_text,
            resources,
        )?);
    }
    for note in context
        .model
        .notes
        .iter()
        .filter(|note| note.parent.is_none())
    {
        checkpoint_layout(context.execution)?;
        boxes.push(render_note_box(
            note,
            context.settings.options,
            context.settings.charset,
            deferred_text,
            resources,
        )?);
    }

    Ok(boxes)
}

fn render_namespace_box<'model>(
    context: &NamespaceRenderContext<'model, '_>,
    namespace: &'model Namespace,
    mut children: Vec<RenderedClassBox>,
    deferred_text: &mut DeferredTextRegistry<'model>,
    resources: &mut ResourceContext,
) -> Result<RenderedClassBox> {
    checkpoint_layout(context.execution)?;
    let mut direct_boxes = Vec::new();
    let direct_box_capacity = namespace
        .class_ids
        .len()
        .checked_add(namespace.note_ids.len())
        .and_then(|value| {
            value.checked_add(
                context
                    .interface_index
                    .for_owner(Some(namespace.id.as_str()))
                    .len(),
            )
        })
        .ok_or_else(|| work_overflow(resources))?;
    direct_boxes
        .try_reserve_exact(direct_box_capacity)
        .map_err(|_| layout_allocation_failed())?;
    children
        .try_reserve(direct_box_capacity)
        .map_err(|_| layout_allocation_failed())?;
    for class_id in &namespace.class_ids {
        checkpoint_layout(context.execution)?;
        if let Some(class) = context.model.classes.get(class_id)
            && !context.endpoint_index.is_facade(class.id.as_str())
        {
            direct_boxes.push(render_class_box(
                class,
                context.settings.options,
                context.settings.charset,
                deferred_text,
                resources,
            )?);
        }
    }

    for interface in context
        .interface_index
        .for_owner(Some(namespace.id.as_str()))
    {
        checkpoint_layout(context.execution)?;
        direct_boxes.push(render_interface_box(
            interface,
            context.settings.options,
            context.settings.charset,
            deferred_text,
            resources,
        )?);
    }

    for note_id in &namespace.note_ids {
        checkpoint_layout(context.execution)?;
        if let Some(note) = context.note_by_id.get(note_id, resources)? {
            direct_boxes.push(render_note_box(
                note,
                context.settings.options,
                context.settings.charset,
                deferred_text,
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
        checkpoint_layout(context.execution)?;
        if context.scope_index.scope_for_relation(relation_index) != Some(namespace.id.as_str()) {
            continue;
        }
        let layout = relation_layout(
            context.model,
            relation,
            context.endpoint_index,
            context
                .relation_labels
                .get(relation_index)
                .cloned()
                .ok_or_else(layout_allocation_failed)?,
        )?;
        direct_layouts.push(route_layout_for_scope(
            layout,
            Some(namespace.id.as_str()),
            context.scope_index,
            context.settings.options.terminal_width_profile,
            deferred_text,
            resources,
            context.execution,
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
        context.endpoint_index,
        &box_by_id,
        context.settings.options.terminal_width_profile,
        deferred_text,
        resources,
    )?);
    for layout in &mut direct_layouts {
        checkpoint_layout(context.execution)?;
        layout.apply_direction(context.settings.direction);
    }

    if direct_layouts.is_empty() {
        children.extend(direct_boxes);
    } else {
        let component_lines = if context.settings.direction.is_horizontal() {
            render_horizontal_class_component_lines(
                &scope_boxes,
                &direct_layouts,
                context.settings,
                resources,
                deferred_text,
                context.execution,
            )?
        } else {
            render_class_component_lines(
                &scope_boxes,
                &box_by_id,
                &direct_layouts,
                context.settings,
                resources,
                deferred_text,
                context.execution,
            )?
        };
        let relation_component = RelationGraphBox::from_rendered_lines(
            format!("{}::relations", namespace.id),
            component_lines,
            context.settings.options.terminal_width_profile,
            resources,
        )?;
        children.clear();
        children.push(relation_component);
    }

    render_namespace_container_box(
        namespace,
        children,
        context.settings,
        deferred_text,
        resources,
        context.execution,
    )
}

pub(super) fn render_namespace_container_box<'a>(
    namespace: &'a Namespace,
    mut children: Vec<RenderedClassBox>,
    settings: ClassRenderSettings<'_>,
    deferred_text: &mut DeferredTextRegistry<'a>,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<RenderedClassBox> {
    checkpoint_layout(execution)?;
    if settings.direction.is_horizontal() && children.len() > 1 {
        let lines = relation_graph::render_horizontal_box_strip_lines(
            &children,
            settings.direction.horizontal_direction(),
            CLASS_LEVEL_HORIZONTAL_GAP,
            settings.options.terminal_width_profile,
            resources,
        )?;
        let contents = RelationGraphBox::from_rendered_lines(
            format!("{}::contents", namespace.id),
            lines,
            settings.options.terminal_width_profile,
            resources,
        )?;
        children.clear();
        children.push(contents);
    } else if settings.direction == ClassDirection::BottomUp {
        children.reverse();
    }

    let (raw_title, has_authored_title) = namespace_title(namespace, resources)?;
    let title_plan = ComposedTextPlan::try_new_html_decoded(raw_title, resources)?;
    let title_projection_is_lossy =
        authored_display_projection_is_lossy(&title_plan, raw_title, resources)?;
    let disclose_label = has_authored_title && title_projection_is_lossy;
    let title = deferred_text.try_register(
        title_plan,
        settings.options.terminal_width_profile,
        resources,
    )?;
    let authored_label = disclose_label
        .then(|| {
            deferred_text.try_register_framed_value(
                "namespaceLabel(bytes=",
                &namespace.label,
                settings.options.terminal_width_profile,
                resources,
            )
        })
        .transpose()?;
    let identity = namespace_authored_identity(
        namespace,
        raw_title,
        !has_authored_title && title_projection_is_lossy,
    )
    .map(|id| {
        deferred_text.try_register_framed_value(
            "namespaceId(bytes=",
            id,
            settings.options.terminal_width_profile,
            resources,
        )
    })
    .transpose()?;
    let header_width = authored_label
        .as_ref()
        .map_or(title.width(), |label| title.width().max(label.width()));
    let header_width = identity
        .as_ref()
        .map_or(header_width, |identity| header_width.max(identity.width()));
    let inner_gap = settings.options.box_border_padding;
    let inner_width = children
        .iter()
        .map(RelationGraphBox::width)
        .max()
        .unwrap_or(header_width)
        .max(header_width);
    let content_width = resources.checked_grid_add(
        inner_width,
        resources.checked_grid_mul(settings.options.box_border_padding, 2)?,
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
    let frame_rows = resources.checked_grid_add(4, usize::from(authored_label.is_some()))?;
    let frame_rows = resources.checked_grid_add(frame_rows, usize::from(identity.is_some()))?;
    let height = resources.checked_grid_add(body_rows, frame_rows)?;
    let width = resources.checked_grid_add(content_width, 2)?;
    let extent = resources.grid_extent(width, height)?;
    resources.charge_layout_work(extent.cells())?;
    let style = RelationGraphBoxStyle {
        top_left: settings.charset.top_left,
        top_right: settings.charset.top_right,
        bottom_left: settings.charset.bottom_left,
        bottom_right: settings.charset.bottom_right,
        horizontal: settings.charset.horizontal,
        vertical: settings.charset.vertical,
        separator_left: settings.charset.separator_left,
        separator_right: settings.charset.separator_right,
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
        settings.options.terminal_width_profile,
        resources,
    )?);
    lines.push(RelationGraphLine::deferred_box_content(
        &title,
        content_width,
        settings.options.box_border_padding,
        style,
        settings.options.terminal_width_profile,
        resources,
    )?);
    if let Some(authored_label) = authored_label.as_ref() {
        lines.push(RelationGraphLine::deferred_box_content(
            authored_label,
            content_width,
            settings.options.box_border_padding,
            style,
            settings.options.terminal_width_profile,
            resources,
        )?);
    }
    if let Some(identity) = identity.as_ref() {
        lines.push(RelationGraphLine::deferred_box_content(
            identity,
            content_width,
            settings.options.box_border_padding,
            style,
            settings.options.terminal_width_profile,
            resources,
        )?);
    }
    lines.push(RelationGraphLine::try_box_border(
        style.separator_left,
        style.separator_right,
        style.horizontal,
        content_width,
        style.border_role,
        settings.options.terminal_width_profile,
        resources,
    )?);

    for (child_index, child) in children.iter().enumerate() {
        checkpoint_layout(execution)?;
        if child_index > 0 {
            lines.push(namespace_empty_content_line(
                content_width,
                style.vertical,
                style.border_role,
                settings.options.terminal_width_profile,
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
            settings.options.terminal_width_profile,
            resources,
        )?);
    }

    lines.push(RelationGraphLine::try_box_border(
        style.bottom_left,
        style.bottom_right,
        style.horizontal,
        content_width,
        style.border_role,
        settings.options.terminal_width_profile,
        resources,
    )?);

    Ok(RelationGraphBox::new_with_lines(
        namespace.dom_id.clone(),
        lines,
        width,
        settings.options.terminal_width_profile,
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

fn namespace_title<'a>(
    namespace: &'a Namespace,
    resources: &ResourceContext,
) -> Result<(&'a str, bool)> {
    if terminal_text_is_blank(&namespace.label, resources)? {
        Ok((namespace.id.as_str(), false))
    } else {
        Ok((namespace.label.as_str(), true))
    }
}

fn namespace_authored_identity<'a>(
    namespace: &'a Namespace,
    raw_title: &str,
    fallback_projection_is_lossy: bool,
) -> Option<&'a str> {
    let leaf_id = namespace
        .id
        .rsplit('.')
        .next()
        .unwrap_or(namespace.id.as_str());
    let identity_recoverable_from_parent = namespace.parent.as_deref().is_some_and(|parent_id| {
        namespace
            .id
            .strip_prefix(parent_id)
            .and_then(|remainder| remainder.strip_prefix('.'))
            == Some(leaf_id)
    });
    let authored_title_hides_identity = raw_title != namespace.id.as_str()
        && (namespace.label.as_str() != leaf_id || !identity_recoverable_from_parent);
    (fallback_projection_is_lossy || authored_title_hides_identity).then_some(namespace.id.as_str())
}

fn checkpoint_layout(execution: AsciiExecution<'_>) -> Result<()> {
    execution.checkpoint(OperationPhase::Layout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::AsciiRenderOptions;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::diagram::RenderSemanticModel;
    use merman_core::resources::ResourceProfile;
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

    fn deeply_nested_namespace_model(depth: usize) -> ClassDiagram {
        let mut model = parsed_class_model("classDiagram\nnamespace Seed {}");
        model.namespaces.clear();
        for index in 0..depth {
            let id = format!("n{index}");
            model.namespaces.insert(
                id.clone(),
                Namespace {
                    id: id.clone(),
                    label: id.clone(),
                    dom_id: id,
                    class_ids: Vec::new(),
                    note_ids: Vec::new(),
                    parent: index.checked_sub(1).map(|parent| format!("n{parent}")),
                    explicit: true,
                },
            );
        }
        model
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn namespace_render_is_iterative_and_plan_admits_exact_linear_work() {
        const PLAN_DEPTH: usize = 512;
        const RENDER_DEPTH: usize = 128;

        std::thread::Builder::new()
            .name("class-namespace-plan-small-stack".to_string())
            .stack_size(64 * 1024)
            .spawn(|| {
                let model = deeply_nested_namespace_model(PLAN_DEPTH);
                let unbounded =
                    AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
                let measured = ResourceContext::new(unbounded);
                let plan = NamespaceRenderPlan::new(
                    &model,
                    &measured,
                    AsciiExecution::for_test(&unbounded),
                )
                .expect("deep namespace planning should not depend on the thread stack");
                let exact_work = PLAN_DEPTH * 5;
                assert_eq!(measured.layout_work_used(), exact_work);
                assert_eq!(plan.roots.len(), 1);
                assert_eq!(plan.roots[0].id, "n0");
                assert_eq!(
                    plan.postorder
                        .first()
                        .map(|namespace| namespace.id.as_str()),
                    Some("n511")
                );
                assert_eq!(
                    plan.postorder.last().map(|namespace| namespace.id.as_str()),
                    Some("n0")
                );

                let exact_policy = unbounded
                    .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
                    .expect("exact namespace-plan work limit should be valid");
                let exact = ResourceContext::new(exact_policy);
                NamespaceRenderPlan::new(&model, &exact, AsciiExecution::for_test(&exact_policy))
                    .expect("exact namespace-plan work should build");
                assert_eq!(exact.layout_work_used(), exact_work);

                let below_policy = unbounded
                    .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
                    .expect("max-minus-one namespace-plan work limit should be valid");
                let below = ResourceContext::new(below_policy);
                let error = NamespaceRenderPlan::new(
                    &model,
                    &below,
                    AsciiExecution::for_test(&below_policy),
                )
                .expect_err("max-minus-one namespace-plan work must reject");
                assert!(matches!(
                    error,
                    AsciiError::ResourceLimitExceeded(details)
                        if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                            && details.actual == exact_work
                            && details.max == exact_work - 1
                ));
                assert_eq!(below.layout_work_used(), 0);
                assert_eq!(below.document_cells_used(), 0);

                let render_model = deeply_nested_namespace_model(RENDER_DEPTH);
                let rendered = crate::class::render::render_class_diagram_with_execution(
                    &render_model,
                    &AsciiRenderOptions::ascii(),
                    AsciiExecution::for_test(&unbounded),
                )
                .expect("deep namespace rendering should not depend on the thread stack");
                assert!(rendered.contains("n0"));
                assert!(rendered.contains("n127"));
            })
            .expect("the small-stack thread should start")
            .join()
            .expect("the small-stack thread should finish");
    }

    #[test]
    fn namespace_scope_index_admits_exact_work_and_rolls_back_n_minus_one() {
        let model = parsed_class_model(
            "classDiagram\nnamespace Platform {\n  namespace FFI {\n    class Dart\n  }\n  namespace Core {\n    class Renderer\n  }\n}\nDart --> Renderer",
        );
        let unbounded = AsciiResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        let mut endpoint_resources = ResourceContext::new(unbounded);
        let endpoint_index = ClassEndpointIndex::new(&model, &mut endpoint_resources)
            .expect("class endpoint index should build");
        let measured = ResourceContext::new(unbounded);
        NamespaceScopeIndex::new(
            &model,
            &endpoint_index,
            &measured,
            AsciiExecution::for_test(&unbounded),
        )
        .expect("unbounded namespace index should build");
        let exact_work = measured.layout_work_used();
        assert!(exact_work > 1);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact namespace-index work limit should be valid");
        let exact = ResourceContext::new(exact_policy);
        NamespaceScopeIndex::new(
            &model,
            &endpoint_index,
            &exact,
            AsciiExecution::for_test(&exact_policy),
        )
        .expect("exact namespace-index work should build");
        assert_eq!(exact.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("max-minus-one namespace-index work limit should be valid");
        let below = ResourceContext::new(below_policy);
        below
            .charge_layout_work(1)
            .expect("test checkpoint should be admitted");
        let checkpoint = below.layout_work_used();
        let error = NamespaceScopeIndex::new(
            &model,
            &endpoint_index,
            &below,
            AsciiExecution::for_test(&below_policy),
        )
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
    fn parser_lollipop_interface_inherits_target_namespace_for_placement_and_routing() {
        let model = parsed_class_model(concat!(
            "classDiagram\n",
            "namespace Domain {\n  class Service\n}\n",
            "IService ()-- Service",
        ));
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut resources = ResourceContext::new(policy);
        validate_class_namespace_ownership(&model, &mut resources)
            .expect("parser namespace ownership should be internally consistent");
        let endpoint_index = ClassEndpointIndex::new(&model, &mut resources)
            .expect("class endpoint index should inherit interface ownership");

        let interface = model
            .interfaces
            .first()
            .expect("lollipop relation should create an interface endpoint");
        let endpoint = endpoint_index
            .resolve(interface.id.as_str())
            .expect("interface endpoint should be indexed");
        assert_eq!(endpoint.owner, Some("Domain"));

        let interface_index = NamespaceInterfaceIndex::new(
            &model,
            &endpoint_index,
            &resources,
            AsciiExecution::for_test(&policy),
        )
        .expect("namespace interface placement should build");
        assert!(interface_index.for_owner(None).is_empty());
        assert_eq!(
            interface_index
                .for_owner(Some("Domain"))
                .iter()
                .map(|interface| interface.id.as_str())
                .collect::<Vec<_>>(),
            vec!["interface0"]
        );

        let scope_index = NamespaceScopeIndex::new(
            &model,
            &endpoint_index,
            &resources,
            AsciiExecution::for_test(&policy),
        )
        .expect("interface relation scope should build");
        assert_eq!(scope_index.scope_for_relation(0), Some("Domain"));
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
        let policy = AsciiResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        let mut endpoint_resources = ResourceContext::new(policy);
        let endpoint_index = ClassEndpointIndex::new(&model, &mut endpoint_resources)
            .expect("class endpoint index should build");
        let resources = ResourceContext::new(policy);

        let error = NamespaceScopeIndex::new(
            &model,
            &endpoint_index,
            &resources,
            AsciiExecution::for_test(&policy),
        )
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
