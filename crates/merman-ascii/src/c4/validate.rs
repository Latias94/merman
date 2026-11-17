//! Validate alias ownership and the boundary forest before projecting terminal geometry.

use super::{allocation_failed, reserve, text};
use crate::Result;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_model::families::c4::C4DiagramModel;
use rustc_hash::FxHashMap;

pub(super) struct Hierarchy {
    pub(super) children: Vec<Vec<usize>>,
    pub(super) preorder: Vec<usize>,
    pub(super) hidden: Vec<bool>,
    pub(super) endpoints: Vec<(usize, usize)>,
    pub(super) shape_columns: usize,
    pub(super) boundary_columns: usize,
}

/// Non-spatial or ambiguous detached data keeps complete records instead of invented geometry.
pub(super) fn prepare(
    model: &C4DiagramModel,
    resources: &ResourceContext,
) -> Result<Option<Hierarchy>> {
    resources.checkpoint()?;
    let count = resources.checked_grid_add(model.shapes.len(), model.boundaries.len())?;
    let hierarchy_count = resources.checked_grid_add(count, 1)?;
    resources.charge_layout_work(resources.checked_work_add(count, model.rels.len())?)?;
    resources.check(AsciiResourceLimitId::MaxDocumentCells, count)?;
    resources.check(AsciiResourceLimitId::MaxOutputBytes, count)?;
    if super::view_name(&model.c4_type).is_none() || count == 0 {
        return Ok(None);
    }
    let (Some(shape_columns), Some(boundary_columns)) = (
        columns(model.layout.c4_shape_in_row, count),
        columns(model.layout.c4_boundary_in_row, count),
    ) else {
        return Ok(None);
    };
    let mut shapes = FxHashMap::default();
    let mut boundaries = FxHashMap::default();
    shapes
        .try_reserve(model.shapes.len())
        .map_err(|_| allocation_failed())?;
    boundaries
        .try_reserve(model.boundaries.len())
        .map_err(|_| allocation_failed())?;
    let mut hidden = filled(hierarchy_count, false)?;
    hidden[count] = true;
    for (index, shape) in model.shapes.iter().enumerate() {
        resources.checkpoint()?;
        resources.charge_layout_work(shape.alias.len().max(1))?;
        if shape.alias.is_empty()
            || shapes.insert(shape.alias.as_str(), index).is_some()
            || text(&shape.type_c4_shape).and_then(super::role).is_none()
            || text(&shape.label).is_none()
            || [
                shape.ty.as_ref(),
                shape.techn.as_ref(),
                shape.descr.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|value| text(value).is_none())
        {
            return Ok(None);
        }
    }
    for (index, boundary) in model.boundaries.iter().enumerate() {
        resources.checkpoint()?;
        resources.charge_layout_work(boundary.alias.len().max(1))?;
        if boundary.alias.is_empty()
            || boundaries
                .insert(boundary.alias.as_str(), model.shapes.len() + index)
                .is_some()
            || text(&boundary.label).is_none()
            || [boundary.ty.as_ref(), boundary.descr.as_ref()]
                .into_iter()
                .flatten()
                .any(|value| text(value).is_none())
        {
            return Ok(None);
        }
        // Mermaid suppresses its global boundary frame. Keep it in the record inventory.
        hidden[model.shapes.len() + index] = boundary.alias == "global";
        if boundary.alias == "global" && !boundary.parent_boundary.is_empty() {
            // A hidden intermediate frame would disconnect visible descendant ownership in
            // the route topology. Only the synthetic root is a transparent spatial scope.
            return Ok(None);
        }
    }
    if hidden[..count].iter().all(|&hidden| hidden) {
        return Ok(None);
    }
    let mut children = filled(hierarchy_count, Vec::new())?;
    // C4's renderer packs direct shapes before child boundaries within every scope.
    for (index, parent) in model
        .shapes
        .iter()
        .map(|node| node.parent_boundary.as_str())
        .chain(
            model
                .boundaries
                .iter()
                .map(|node| node.parent_boundary.as_str()),
        )
        .enumerate()
    {
        resources.checkpoint()?;
        resources.charge_layout_work(parent.len().max(1))?;
        let owner = if parent.is_empty() {
            count
        } else {
            let Some(&owner) = boundaries.get(parent) else {
                return Ok(None);
            };
            owner
        };
        reserve(&mut children[owner], 1)?;
        children[owner].push(index);
    }
    let mut preorder = Vec::new();
    let mut stack = Vec::new();
    reserve(&mut preorder, hierarchy_count)?;
    reserve(&mut stack, hierarchy_count)?;
    stack.push((count, 0usize));
    while let Some((index, depth)) = stack.pop() {
        resources.charge_layout_work(1)?;
        resources.check_nesting_depth(depth)?;
        preorder.push(index);
        let depth = resources.checked_grid_add(depth, 1)?;
        resources.charge_layout_work(children[index].len())?;
        stack.extend(children[index].iter().rev().map(|&child| (child, depth)));
    }
    if preorder.len() != hierarchy_count {
        return Ok(None); // A parent cycle is disconnected from the virtual root.
    }
    let mut endpoints = Vec::new();
    reserve(&mut endpoints, model.rels.len())?;
    for relation in &model.rels {
        resources.checkpoint()?;
        resources.charge_layout_work(relation.from_alias.len().max(1))?;
        resources.charge_layout_work(relation.to_alias.len().max(1))?;
        if !matches!(
            relation.rel_type.as_str(),
            "rel" | "rel_b" | "birel" | "rel_u" | "rel_d" | "rel_l" | "rel_r"
        ) || text(&relation.label).is_none()
            || [relation.techn.as_ref(), relation.descr.as_ref()]
                .into_iter()
                .flatten()
                .any(|value| text(value).is_none())
        {
            return Ok(None);
        }
        let endpoint = |alias: &str| shapes.get(alias).or_else(|| boundaries.get(alias)).copied();
        let (Some(from), Some(to)) = (endpoint(&relation.from_alias), endpoint(&relation.to_alias))
        else {
            return Ok(None);
        };
        if hidden[from] || hidden[to] {
            return Ok(None);
        }
        endpoints.push((from, to));
    }
    Ok(Some(Hierarchy {
        children,
        preorder,
        hidden,
        endpoints,
        shape_columns,
        boundary_columns,
    }))
}

fn columns(value: f64, count: usize) -> Option<usize> {
    if !value.is_finite() || value.fract() != 0.0 || value < 1.0 {
        return None;
    }
    // Counts only bound records per row. Huge values never allocate an empty column grid.
    Some(if value >= count as f64 {
        count.max(1)
    } else {
        value as usize
    })
}

pub(super) fn filled<T: Clone>(count: usize, value: T) -> Result<Vec<T>> {
    let mut values = Vec::new();
    reserve(&mut values, count)?;
    values.resize(count, value);
    Ok(values)
}
