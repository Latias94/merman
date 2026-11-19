//! Validate the flat hierarchy once, before allocating a grid or expanding any numbers.

use super::unsupported;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use merman_model::OperationPhase;
use merman_model::families::block::{BlockDiagramModel, BlockNode};
use rustc_hash::FxHashMap;

pub(super) struct Hierarchy {
    pub(super) root: Option<usize>,
    pub(super) preorder: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum Columns {
    Auto,
    Fixed(usize),
}

pub(super) fn columns(node: &BlockNode, resources: &ResourceContext) -> Result<Columns> {
    match node.columns {
        None | Some(-1.0) => Ok(Columns::Auto),
        Some(value) => {
            count(value, false, "invalid Block column count", resources).map(Columns::Fixed)
        }
    }
}

pub(super) fn span(node: &BlockNode, resources: &ResourceContext) -> Result<usize> {
    count(
        node.width_in_columns.unwrap_or(1.0),
        false,
        "invalid Block column span",
        resources,
    )
}

fn count(
    value: f64,
    zero: bool,
    feature: &'static str,
    resources: &ResourceContext,
) -> Result<usize> {
    if !value.is_finite() || value.fract() != 0.0 || value < if zero { 0.0 } else { 1.0 } {
        return Err(unsupported(feature));
    }
    // On 64-bit targets MAX rounds up to 2^64 as f64. Reject that boundary before casting,
    // rather than accepting Rust's saturating float conversion as source semantics.
    if value >= usize::MAX as f64 {
        return Err(resources.overflow(AsciiResourceLimitId::MaxGridCells));
    }
    let value = value as usize;
    resources.check(AsciiResourceLimitId::MaxGridCells, value)?;
    Ok(value)
}

pub(super) fn validate(
    model: &BlockDiagramModel,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Hierarchy> {
    let count = model.blocks_flat.len();
    resources.charge_layout_work(resources.checked_work_add(count, model.edges.len())?)?;
    admit_model_text(model, resources)?;
    let Some(root) = model.root else {
        if count == 0 && model.edges.is_empty() {
            return Ok(Hierarchy {
                root: None,
                preorder: Vec::new(),
            });
        }
        return Err(unsupported("missing Block root index"));
    };
    let Some(root_node) = model.blocks_flat.get(root) else {
        return Err(unsupported("invalid Block root index"));
    };
    if root_node.block_type != "composite" || !root_node.label.is_empty() {
        return Err(unsupported("Block root must be an unlabelled composite"));
    }
    let mut parents = filled_vec(count, None)?;
    let mut visited = filled_vec(count, false)?;
    let mut bindings = FxHashMap::default();
    bindings
        .try_reserve(count)
        .map_err(|_| allocation_failed())?;
    for (index, node) in model.blocks_flat.iter().enumerate() {
        execution.checkpoint_loop(OperationPhase::Layout, index)?;
        if node.id.is_empty() {
            return Err(unsupported("empty Block binding identifier"));
        }
        if bindings.insert(node.id.as_str(), index).is_some() {
            return Err(unsupported("ambiguous duplicate Block database bindings"));
        }
        span(node, resources)?;
        if node.block_type == "composite" {
            columns(node, resources)?;
        } else if !node.children.is_empty() || node.columns.is_some() {
            return Err(unsupported("Block children or columns on a non-composite"));
        }
        if let Some(width) = node.width {
            if node.block_type != "space" {
                return Err(unsupported("Block expansion width on a non-spacer"));
            }
            // Parser output already expands space:N into N one-cell records. `width` remains
            // authored metadata on each clone; it must never be expanded a second time here.
            self::count(
                width,
                true,
                "invalid Block spacer expansion count",
                resources,
            )?;
        }
        if !node.directions.is_empty() && node.block_type != "block_arrow" {
            return Err(unsupported("Block directions on a non-arrow shape"));
        }
        for direction in &node.directions {
            if !matches!(
                direction.as_str(),
                "left" | "right" | "up" | "down" | "x" | "y"
            ) {
                return Err(unsupported("unknown Block arrow direction"));
            }
        }
        resources.charge_layout_work(node.children.len())?;
        for &child in &node.children {
            let Some(parent) = parents.get_mut(child) else {
                return Err(unsupported("invalid Block child index"));
            };
            if parent.replace(index).is_some() {
                return Err(unsupported(
                    "Block child has multiple parents or repeated membership",
                ));
            }
        }
    }
    if parents[root].is_some() {
        return Err(unsupported("cyclic Block hierarchy"));
    }
    let mut preorder = Vec::new();
    let mut stack = Vec::new();
    reserve(&mut preorder, count)?;
    reserve(&mut stack, count)?;
    stack.push((root, 0usize));
    while let Some((index, depth)) = stack.pop() {
        resources.charge_layout_work(1)?;
        resources.check_nesting_depth(depth)?;
        if std::mem::replace(&mut visited[index], true) {
            return Err(unsupported("cyclic Block hierarchy"));
        }
        preorder.push(index);
        let children = &model.blocks_flat[index].children;
        if !children.is_empty() {
            let next_depth = depth
                .checked_add(1)
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxNestingDepth))?;
            // Admit depth before enqueuing descendants, even for a very wide hierarchy.
            resources.check_nesting_depth(next_depth)?;
            resources.charge_layout_work(children.len())?;
            stack.extend(children.iter().rev().map(|&child| (child, next_depth)));
        }
    }
    for (index, node) in model.blocks_flat.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if !visited[index] && (node.block_type != "space" || !node.children.is_empty()) {
            // Mermaid keeps unused space expansion templates in its database. Other detached
            // records (including rebinding by class application) have no unambiguous location.
            return Err(unsupported("unreachable or cyclic Block database records"));
        }
    }
    for edge in &model.edges {
        for endpoint in [&edge.start, &edge.end] {
            let Some(&index) = bindings.get(endpoint.as_str()) else {
                return Err(unsupported("unknown Block edge endpoint"));
            };
            if !visited[index] || index == root || model.blocks_flat[index].block_type == "space" {
                return Err(unsupported(
                    "Block edge endpoint has no visible hierarchy geometry",
                ));
            }
        }
    }
    Ok(Hierarchy {
        root: Some(root),
        preorder,
    })
}

/// Admit the complete borrowed text batch before allocating maps or cloning any labels/IDs.
/// Empty metadata entries still cost work, and no iterator's length can hide unmetered loops.
fn admit_model_text(model: &BlockDiagramModel, resources: &ResourceContext) -> Result<()> {
    let mut bytes = 0usize;
    let mut include = |text: &str| -> Result<()> {
        bytes = bytes
            .checked_add(text.len())
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        resources.check(AsciiResourceLimitId::MaxOutputBytes, bytes)?;
        resources.charge_layout_work(text.len().max(1))
    };
    for node in &model.blocks_flat {
        for text in [&node.id, &node.label, &node.block_type]
            .into_iter()
            .chain(&node.classes)
            .chain(&node.styles)
            .chain(&node.directions)
        {
            include(text)?;
        }
    }
    for edge in &model.edges {
        for text in [&edge.id, &edge.start, &edge.end, &edge.label] {
            include(text)?;
        }
        for marker in [&edge.arrow_type_start, &edge.arrow_type_end]
            .into_iter()
            .flatten()
        {
            include(marker)?;
        }
    }
    for (key, definition) in &model.class_defs {
        for text in [key, &definition.id]
            .into_iter()
            .chain(&definition.styles)
            .chain(&definition.text_styles)
        {
            include(text)?;
        }
    }
    for fact in &model.warning_facts {
        include(&fact.rule_id)?;
        include(&fact.message)?;
    }
    Ok(())
}

pub(super) fn filled_vec<T: Clone>(count: usize, value: T) -> Result<Vec<T>> {
    let mut values = Vec::new();
    reserve(&mut values, count)?;
    values.resize(count, value);
    Ok(values)
}

pub(super) fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<()> {
    values
        .try_reserve(additional)
        .map_err(|_| allocation_failed())
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed("ascii_layout_work")
}
