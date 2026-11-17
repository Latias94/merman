use super::adapter::{allocation_failed, checkpoint, unsupported};
use crate::error::Result;
use crate::operation::AsciiExecution;
use crate::resource::ResourceContext;
use crate::safe_text::charge_text_layout;
use merman_model::families::usecase::{
    UsecaseActorType, UsecaseDiagramModel, UsecaseNode, UsecaseNodeKind, UsecaseRelationshipType,
};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy)]
enum Entity {
    Node(usize),
    Boundary(usize),
}

pub(super) struct Index<'a> {
    pub(super) nodes: HashMap<&'a str, usize>,
    pub(super) notes_by_boundary: Vec<Vec<usize>>,
}

/// Bound the structural allocations and identity scans before building any index or label plan.
pub(super) fn preflight(
    model: &UsecaseDiagramModel,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    checkpoint(execution, 0)?;
    let mut items = resources.checked_work_add(model.nodes.len(), model.boundaries.len())?;
    items = resources.checked_work_add(items, model.relationships.len())?;
    items = resources.checked_work_add(items, model.notes.len())?;
    items = resources.checked_work_add(items, model.class_defs.len())?;
    // Index entries, parent/visit arrays, and deferred plans are all linear in these counts.
    resources.charge_layout_work_product(items, 8)?;
    charge_text_layout(resources, &model.direction)?;
    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint(execution, index)?;
        charge_text_layout(resources, &node.id)?;
        if let Some(parent) = &node.parent_id {
            charge_text_layout(resources, parent)?;
        }
    }
    for (index, boundary) in model.boundaries.iter().enumerate() {
        checkpoint(execution, index)?;
        charge_text_layout(resources, &boundary.id)?;
        resources.charge_layout_work(boundary.members.len())?;
        for (index, member) in boundary.members.iter().enumerate() {
            checkpoint(execution, index)?;
            charge_text_layout(resources, member)?;
        }
    }
    for (index, relationship) in model.relationships.iter().enumerate() {
        checkpoint(execution, index)?;
        for value in [&relationship.id, &relationship.source, &relationship.target] {
            charge_text_layout(resources, value)?;
        }
    }
    for (index, note) in model.notes.iter().enumerate() {
        checkpoint(execution, index)?;
        charge_text_layout(resources, &note.id)?;
        charge_text_layout(resources, &note.target)?;
    }
    for (index, class) in model.class_defs.iter().enumerate() {
        checkpoint(execution, index)?;
        charge_text_layout(resources, &class.id)?;
    }
    Ok(())
}

pub(super) fn validate<'a>(
    model: &'a UsecaseDiagramModel,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Index<'a>> {
    if !model.json_nodes.is_empty() {
        return Err(unsupported("JSON table nodes"));
    }
    let entity_count = resources.checked_work_add(model.nodes.len(), model.boundaries.len())?;
    let mut entities = HashMap::new();
    entities
        .try_reserve(entity_count)
        .map_err(|_| allocation_failed())?;
    let mut nodes = HashMap::new();
    nodes
        .try_reserve(model.nodes.len())
        .map_err(|_| allocation_failed())?;
    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint(execution, index)?;
        if node.id.is_empty() {
            return Err(unsupported("empty node ids"));
        }
        if entities
            .insert(node.id.as_str(), Entity::Node(index))
            .is_some()
        {
            return Err(unsupported("duplicate node ids"));
        }
        validate_node(node)?;
        nodes.insert(node.id.as_str(), index);
    }
    for (index, boundary) in model.boundaries.iter().enumerate() {
        checkpoint(execution, index)?;
        if boundary.id.is_empty() {
            return Err(unsupported("empty boundary ids"));
        }
        if entities
            .insert(boundary.id.as_str(), Entity::Boundary(index))
            .is_some()
        {
            return Err(unsupported("duplicate or colliding boundary ids"));
        }
    }

    let mut parent_by_node = reserved_vec(model.nodes.len())?;
    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint(execution, index)?;
        let parent = node
            .parent_id
            .as_deref()
            .map(|id| match entities.get(id) {
                Some(Entity::Boundary(index)) => Ok(*index),
                Some(Entity::Node(_)) => Err(unsupported("node parents that are not boundaries")),
                None => Err(unsupported("unknown node parent ids")),
            })
            .transpose()?;
        if parent.is_some() {
            resources.check_nesting_depth(2)?;
        }
        parent_by_node.push(parent);
    }
    let mut seen_nodes = reserved_vec(model.nodes.len())?;
    seen_nodes.resize(model.nodes.len(), false);
    let mut boundary_parents = reserved_vec(model.boundaries.len())?;
    boundary_parents.resize(model.boundaries.len(), None);
    let mut members = HashSet::new();
    for (index, boundary) in model.boundaries.iter().enumerate() {
        checkpoint(execution, index)?;
        members.clear();
        members
            .try_reserve(boundary.members.len())
            .map_err(|_| allocation_failed())?;
        for (member_index, member) in boundary.members.iter().enumerate() {
            checkpoint(execution, member_index)?;
            if !members.insert(member.as_str()) {
                return Err(unsupported("duplicate boundary members"));
            }
            match entities.get(member.as_str()) {
                Some(Entity::Node(node)) => {
                    if seen_nodes[*node] {
                        return Err(unsupported("nodes in multiple boundaries"));
                    }
                    if parent_by_node[*node] != Some(index) {
                        return Err(unsupported(
                            "boundary membership disagrees with node parent",
                        ));
                    }
                    seen_nodes[*node] = true;
                }
                Some(Entity::Boundary(child)) => {
                    if boundary_parents[*child].replace(index).is_some() {
                        return Err(unsupported("boundaries in multiple containers"));
                    }
                }
                None => return Err(unsupported("boundary members with missing targets")),
            }
        }
    }
    for (index, parent) in parent_by_node.iter().enumerate() {
        checkpoint(execution, index)?;
        if parent.is_some() && !seen_nodes[index] {
            return Err(unsupported(
                "parented nodes missing from boundary membership",
            ));
        }
    }
    validate_boundary_parents(&boundary_parents, resources, execution)?;

    let mut relationship_ids = HashSet::new();
    relationship_ids
        .try_reserve(model.relationships.len())
        .map_err(|_| allocation_failed())?;
    for (index, relationship) in model.relationships.iter().enumerate() {
        checkpoint(execution, index)?;
        if relationship.id.is_empty() || !relationship_ids.insert(relationship.id.as_str()) {
            return Err(unsupported("empty or duplicate relationship ids"));
        }
        if relationship.explicit_id && entities.contains_key(relationship.id.as_str()) {
            return Err(unsupported(
                "explicit relationship ids collide with entities",
            ));
        }
        let source = *nodes
            .get(relationship.source.as_str())
            .ok_or_else(|| unsupported("relationships with missing endpoint nodes"))?;
        let target = *nodes
            .get(relationship.target.as_str())
            .ok_or_else(|| unsupported("relationships with missing endpoint nodes"))?;
        let source_actor = model.nodes[source].kind == UsecaseNodeKind::Actor;
        let target_actor = model.nodes[target].kind == UsecaseNodeKind::Actor;
        match relationship.relationship_type {
            UsecaseRelationshipType::Include | UsecaseRelationshipType::Extend
                if source_actor || target_actor =>
            {
                return Err(unsupported("include and extend require use-case endpoints"));
            }
            UsecaseRelationshipType::Generalization if source_actor != target_actor => {
                return Err(unsupported(
                    "generalization requires endpoints of the same role",
                ));
            }
            _ => {}
        }
        if relationship.minlen == 0 {
            return Err(unsupported("zero relationship minlen"));
        }
    }
    let mut notes_by_boundary = reserved_vec(model.boundaries.len())?;
    notes_by_boundary.resize_with(model.boundaries.len(), Vec::new);
    let mut note_ids = HashSet::new();
    note_ids
        .try_reserve(model.notes.len())
        .map_err(|_| allocation_failed())?;
    for (index, note) in model.notes.iter().enumerate() {
        checkpoint(execution, index)?;
        if note.id.is_empty() || !note_ids.insert(note.id.as_str()) {
            return Err(unsupported("empty or duplicate note ids"));
        }
        let target = *nodes
            .get(note.target.as_str())
            .ok_or_else(|| unsupported("notes with missing target nodes"))?;
        if let Some(parent) = parent_by_node[target] {
            let notes = &mut notes_by_boundary[parent];
            notes.try_reserve(1).map_err(|_| allocation_failed())?;
            notes.push(index);
        }
    }
    let mut class_ids = HashSet::new();
    class_ids
        .try_reserve(model.class_defs.len())
        .map_err(|_| allocation_failed())?;
    for (index, class) in model.class_defs.iter().enumerate() {
        checkpoint(execution, index)?;
        if class.id.is_empty() || !class_ids.insert(class.id.as_str()) {
            return Err(unsupported("empty or duplicate class definition ids"));
        }
    }
    Ok(Index {
        nodes,
        notes_by_boundary,
    })
}

fn validate_node(node: &UsecaseNode) -> Result<()> {
    let has_icon = node.icon.as_ref().is_some_and(|icon| !icon.is_empty());
    if node.kind == UsecaseNodeKind::Actor {
        if has_icon != (node.actor_type == UsecaseActorType::Icon) {
            return Err(unsupported("inconsistent actor icon metadata"));
        }
        if node.business
            && matches!(
                node.actor_type,
                UsecaseActorType::Awesome | UsecaseActorType::Icon
            )
        {
            return Err(unsupported(
                "business actors require normal or hollow geometry",
            ));
        }
    } else {
        if node.actor_type != UsecaseActorType::Normal || node.icon.is_some() {
            return Err(unsupported("actor metadata on use-case nodes"));
        }
        if node.business && node.kind == UsecaseNodeKind::UseCaseRect {
            return Err(unsupported("business rectangular use cases"));
        }
    }
    Ok(())
}

fn validate_boundary_parents(
    parents: &[Option<usize>],
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    let mut visits = reserved_vec(parents.len())?;
    visits.resize(parents.len(), 0u8);
    let mut path = reserved_vec(parents.len())?;
    for start in 0..parents.len() {
        checkpoint(execution, start)?;
        path.clear();
        let mut current = Some(start);
        while let Some(index) = current {
            resources.charge_layout_work(1)?;
            match visits[index] {
                2 => break,
                1 => return Err(unsupported("cyclic boundary membership")),
                _ => {
                    visits[index] = 1;
                    path.push(index);
                    resources.check_nesting_depth(path.len())?;
                    current = parents[index];
                }
            }
        }
        for &index in &path {
            resources.checkpoint()?;
            visits[index] = 2;
        }
    }
    // The canonical Usecase grammar permits only flat system boundaries. Detect malformed
    // cycles above before distinguishing an acyclic but unsupported nested direct model.
    if parents.iter().any(Option::is_some) {
        return Err(unsupported("nested system boundaries"));
    }
    Ok(())
}

pub(super) fn reserved_vec<T>(capacity: usize) -> Result<Vec<T>> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_failed())?;
    Ok(values)
}
