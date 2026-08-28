use super::{
    SequenceControlFrame, SequenceControlFrameForest, SequenceControlFramePlan,
    SequenceControlFrameSeparator, SequenceControlOutputAdmission, allocation_failed,
    invalid_control_frame, valid_frame_end_row, work_overflow,
};
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::error::Result;
use crate::resource::ResourceContext;
use crate::sequence::SequenceCheckpointCursor;
use crate::sequence::chars::SequenceChars;
use crate::sequence::layout::SequenceLayout;
use crate::sequence::text::{SequenceLine, padded_line_with_checkpoints};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceControlBodyRow {
    Content,
    Separator(usize),
}

#[derive(Debug, PartialEq, Eq)]
struct SequenceControlBody {
    rows: Vec<SequenceControlBodyRow>,
    content: Vec<SequenceLine>,
}

struct SequenceControlPaintContext<'a, 'diagram> {
    forest: &'a SequenceControlFrameForest,
    frames: &'a [SequenceControlFrame<'diagram>],
    frame_plans: &'a [SequenceControlFramePlan<'diagram>],
    layout: &'a SequenceLayout,
    chars: &'a SequenceChars,
    resources: &'a ResourceContext,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn materialize_control_frames(
    lines: Vec<SequenceLine>,
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame<'_>],
    frame_plans: &[SequenceControlFramePlan<'_>],
    output_admission: SequenceControlOutputAdmission,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<SequenceLine>> {
    let context = SequenceControlPaintContext {
        forest,
        frames,
        frame_plans,
        layout,
        chars,
        resources,
    };
    let line_count = lines.len();
    let mut remaining_lines = Vec::new();
    remaining_lines
        .try_reserve_exact(line_count)
        .map_err(|_| allocation_failed())?;
    for line in lines {
        checkpoints.tick()?;
        remaining_lines.push(Some(line));
    }

    let mut rendered_nodes = Vec::new();
    rendered_nodes
        .try_reserve_exact(context.forest.nodes.len())
        .map_err(|_| allocation_failed())?;
    rendered_nodes.resize_with(context.forest.nodes.len(), || None);

    let mut traversal = Vec::new();
    traversal
        .try_reserve_exact(context.forest.nodes.len())
        .map_err(|_| allocation_failed())?;
    traversal.extend(context.forest.roots.iter().rev().map(|root| (*root, false)));

    while let Some((node_index, expanded)) = traversal.pop() {
        checkpoints.tick()?;
        if expanded {
            let rendered = render_frame_node_iterative(
                node_index,
                &context,
                &mut remaining_lines,
                &mut rendered_nodes,
                checkpoints,
            )?;
            let slot = rendered_nodes
                .get_mut(node_index)
                .ok_or_else(invalid_control_frame)?;
            if slot.replace(rendered).is_some() {
                return Err(invalid_control_frame());
            }
            continue;
        }

        let node = context
            .forest
            .nodes
            .get(node_index)
            .ok_or_else(invalid_control_frame)?;
        let additional = node
            .children
            .len()
            .checked_add(1)
            .ok_or_else(|| work_overflow(resources))?;
        traversal
            .try_reserve(additional)
            .map_err(|_| allocation_failed())?;
        traversal.push((node_index, true));
        for child in node.children.iter().rev() {
            checkpoints.tick()?;
            traversal.push((*child, false));
        }
    }

    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(output_admission.height)
        .map_err(|_| allocation_failed())?;
    let mut row = 0;

    for root in &context.forest.roots {
        checkpoints.tick()?;
        let node = context
            .forest
            .nodes
            .get(*root)
            .ok_or_else(invalid_control_frame)?;
        let frame = context
            .frames
            .get(node.frame_index)
            .ok_or_else(invalid_control_frame)?;
        let Some(node_end) = valid_frame_end_row(frame, line_count) else {
            continue;
        };

        if row < frame.start_row {
            extend_taken_lines(
                &mut rendered,
                &mut remaining_lines,
                row..frame.start_row,
                checkpoints,
            )?;
        }
        let frame_lines = rendered_nodes
            .get_mut(*root)
            .and_then(Option::take)
            .ok_or_else(invalid_control_frame)?;
        extend_owned_lines(&mut rendered, frame_lines, checkpoints)?;
        row = context.resources.checked_grid_add(node_end, 1)?;
    }

    if row < line_count {
        extend_taken_lines(
            &mut rendered,
            &mut remaining_lines,
            row..line_count,
            checkpoints,
        )?;
    }
    for line in &remaining_lines {
        checkpoints.tick()?;
        if line.is_some() {
            return Err(invalid_control_frame());
        }
    }
    for node in &rendered_nodes {
        checkpoints.tick()?;
        if node.is_some() {
            return Err(invalid_control_frame());
        }
    }
    output_admission.validate(&rendered, context.resources, checkpoints)?;
    Ok(rendered)
}

fn render_frame_node_iterative(
    node_index: usize,
    context: &SequenceControlPaintContext<'_, '_>,
    lines: &mut [Option<SequenceLine>],
    rendered_nodes: &mut [Option<Vec<SequenceLine>>],
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<SequenceLine>> {
    let node = context
        .forest
        .nodes
        .get(node_index)
        .ok_or_else(invalid_control_frame)?;
    let frame = context
        .frames
        .get(node.frame_index)
        .ok_or_else(invalid_control_frame)?;
    let plan = context
        .frame_plans
        .get(node_index)
        .ok_or_else(invalid_control_frame)?;
    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(plan.row_count)
        .map_err(|_| allocation_failed())?;
    let body = render_frame_body_iterative(
        node_index,
        context,
        lines,
        rendered_nodes,
        plan.body_rows,
        checkpoints,
    )?;
    let mut content = body.content.into_iter();
    rendered.push(render_top_border(frame, plan, context, checkpoints)?);

    for row in body.rows {
        checkpoints.tick()?;
        match row {
            SequenceControlBodyRow::Content => {
                let line = content.next().ok_or_else(invalid_control_frame)?;
                rendered.push(render_content_row(
                    line,
                    plan,
                    context.chars,
                    frame.background,
                    context.resources,
                    checkpoints,
                )?);
            }
            SequenceControlBodyRow::Separator(separator_index) => {
                let separator = frame
                    .separators
                    .get(separator_index)
                    .ok_or_else(invalid_control_frame)?;
                rendered.push(render_separator_border(
                    frame,
                    separator,
                    separator_index,
                    plan,
                    context,
                    checkpoints,
                )?);
            }
        }
    }
    if content.next().is_some() {
        return Err(invalid_control_frame());
    }

    rendered.push(render_bottom_border(frame, plan, context, checkpoints)?);
    Ok(rendered)
}

fn render_frame_body_iterative(
    node_index: usize,
    context: &SequenceControlPaintContext<'_, '_>,
    lines: &mut [Option<SequenceLine>],
    rendered_nodes: &mut [Option<Vec<SequenceLine>>],
    planned_rows: usize,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceControlBody> {
    let node = context
        .forest
        .nodes
        .get(node_index)
        .ok_or_else(invalid_control_frame)?;
    let frame = context
        .frames
        .get(node.frame_index)
        .ok_or_else(invalid_control_frame)?;
    let end_row = frame.end_row.ok_or_else(invalid_control_frame)?;
    let mut body_rows = Vec::new();
    body_rows
        .try_reserve_exact(planned_rows)
        .map_err(|_| allocation_failed())?;
    let mut content = Vec::new();
    content
        .try_reserve_exact(planned_rows)
        .map_err(|_| allocation_failed())?;
    let mut row = frame.start_row;
    let mut child_index = 0;
    let mut separator_index = 0;
    while row <= end_row {
        checkpoints.tick()?;
        while frame
            .separators
            .get(separator_index)
            .is_some_and(|separator| separator.row == row)
        {
            checkpoints.tick()?;
            body_rows.push(SequenceControlBodyRow::Separator(separator_index));
            separator_index = context.resources.checked_grid_add(separator_index, 1)?;
        }

        if let Some(child_node_index) = node.children.get(child_index).copied() {
            let child = context
                .forest
                .nodes
                .get(child_node_index)
                .ok_or_else(invalid_control_frame)?;
            let child_frame = context
                .frames
                .get(child.frame_index)
                .ok_or_else(invalid_control_frame)?;
            if child_frame.start_row == row {
                let child_lines = rendered_nodes
                    .get_mut(child_node_index)
                    .and_then(Option::take)
                    .ok_or_else(invalid_control_frame)?;
                for child_line in child_lines {
                    checkpoints.tick()?;
                    body_rows.push(SequenceControlBodyRow::Content);
                    content.push(child_line);
                }
                row = context
                    .resources
                    .checked_grid_add(child_frame.end_row.ok_or_else(invalid_control_frame)?, 1)?;
                child_index = context.resources.checked_grid_add(child_index, 1)?;
                continue;
            }
        }

        let line = lines
            .get_mut(row)
            .and_then(Option::take)
            .ok_or_else(invalid_control_frame)?;
        body_rows.push(SequenceControlBodyRow::Content);
        content.push(line);
        row = context.resources.checked_grid_add(row, 1)?;
    }

    if body_rows.len() != planned_rows {
        return Err(invalid_control_frame());
    }
    Ok(SequenceControlBody {
        rows: body_rows,
        content,
    })
}

fn render_top_border(
    frame: &SequenceControlFrame<'_>,
    plan: &SequenceControlFramePlan<'_>,
    context: &SequenceControlPaintContext<'_, '_>,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    checkpoints.tick()?;
    let title = plan.title.materialize_after_admission(checkpoints)?;
    let base = frame.start_boundary.render_lifeline(
        context.layout,
        context.chars,
        context.resources,
        checkpoints,
    )?;
    render_border_row(
        base,
        context.chars.top_left,
        context.chars.top_right,
        context.chars.horizontal,
        plan,
        Some(&title),
        frame.background,
        context.resources,
        checkpoints,
    )
}

fn render_bottom_border(
    frame: &SequenceControlFrame<'_>,
    plan: &SequenceControlFramePlan<'_>,
    context: &SequenceControlPaintContext<'_, '_>,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    let base = frame
        .end_boundary
        .as_ref()
        .ok_or_else(invalid_control_frame)?
        .render_lifeline(
            context.layout,
            context.chars,
            context.resources,
            checkpoints,
        )?;
    render_border_row(
        base,
        context.chars.bottom_left,
        context.chars.bottom_right,
        context.chars.horizontal,
        plan,
        None,
        frame.background,
        context.resources,
        checkpoints,
    )
}

fn render_separator_border(
    frame: &SequenceControlFrame<'_>,
    separator: &SequenceControlFrameSeparator<'_>,
    separator_index: usize,
    plan: &SequenceControlFramePlan<'_>,
    context: &SequenceControlPaintContext<'_, '_>,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    checkpoints.tick()?;
    let title = plan
        .separator_titles
        .get(separator_index)
        .copied()
        .ok_or_else(invalid_control_frame)?
        .materialize_after_admission(checkpoints)?;
    let base = separator.boundary.render_lifeline(
        context.layout,
        context.chars,
        context.resources,
        checkpoints,
    )?;
    render_border_row(
        base,
        context.chars.tee_right,
        context.chars.tee_left,
        context.chars.horizontal,
        plan,
        Some(&title),
        frame.background,
        context.resources,
        checkpoints,
    )
}

// The arguments map one-to-one to the terminal border primitive: geometry, optional label,
// background, and terminal glyphs are intentionally kept explicit at call sites.
#[allow(clippy::too_many_arguments)]
fn render_border_row(
    base: SequenceLine,
    left: char,
    right: char,
    horizontal: char,
    plan: &SequenceControlFramePlan<'_>,
    label: Option<&str>,
    background: Option<AsciiRgb>,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    let mut row = padded_line_with_checkpoints(base, plan.total_width, checkpoints)?;
    let left_index = plan.bounds.left();
    let right_index = plan.bounds.right();
    let frame_end = plan.bounds.right_exclusive(resources)?;
    paint_row_background(&mut row, left_index..frame_end, background, checkpoints)?;
    for x in left_index..frame_end {
        checkpoints.tick()?;
        row.try_set_role(x, horizontal, AsciiColorRole::SequenceFrame)?;
    }
    row.try_set_role(left_index, left, AsciiColorRole::SequenceFrame)?;
    row.try_set_role(right_index, right, AsciiColorRole::SequenceFrame)?;
    if let Some(label) = label {
        row.try_write_text_role_with_checkpoint(
            resources.checked_grid_add(left_index, 1)?,
            label,
            AsciiColorRole::Section,
            resources,
            || checkpoints.tick(),
        )?;
    }
    Ok(row)
}

fn render_content_row(
    row: SequenceLine,
    plan: &SequenceControlFramePlan<'_>,
    chars: &SequenceChars,
    background: Option<AsciiRgb>,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    let mut row = padded_line_with_checkpoints(row, plan.total_width, checkpoints)?;
    let frame_end = plan.bounds.right_exclusive(resources)?;
    paint_row_background_if_unset(
        &mut row,
        plan.bounds.left()..frame_end,
        background,
        checkpoints,
    )?;
    paint_frame_vertical_if_unset(&mut row, plan.bounds.left(), chars.vertical)?;
    paint_frame_vertical_if_unset(&mut row, plan.bounds.right(), chars.vertical)?;
    Ok(row)
}

fn paint_frame_vertical_if_unset(
    row: &mut SequenceLine,
    index: usize,
    vertical: char,
) -> Result<()> {
    if row.get(index) == Some(' ') {
        row.try_set_role(index, vertical, AsciiColorRole::SequenceFrame)?;
    }
    Ok(())
}

fn paint_row_background(
    row: &mut SequenceLine,
    range: impl Iterator<Item = usize>,
    background: Option<AsciiRgb>,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let Some(background) = background else {
        return Ok(());
    };
    for x in range {
        checkpoints.tick()?;
        row.set_background_color(x, background);
    }
    Ok(())
}

fn paint_row_background_if_unset(
    row: &mut SequenceLine,
    range: impl Iterator<Item = usize>,
    background: Option<AsciiRgb>,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let Some(background) = background else {
        return Ok(());
    };
    for x in range {
        checkpoints.tick()?;
        row.set_background_color_if_unset(x, background);
    }
    Ok(())
}

fn extend_taken_lines(
    target: &mut Vec<SequenceLine>,
    source: &mut [Option<SequenceLine>],
    range: std::ops::Range<usize>,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let source = source.get_mut(range).ok_or_else(invalid_control_frame)?;
    target
        .try_reserve(source.len())
        .map_err(|_| allocation_failed())?;
    for line in source {
        checkpoints.tick()?;
        target.push(line.take().ok_or_else(invalid_control_frame)?);
    }
    Ok(())
}

fn extend_owned_lines(
    target: &mut Vec<SequenceLine>,
    source: Vec<SequenceLine>,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    target
        .try_reserve(source.len())
        .map_err(|_| allocation_failed())?;
    for line in source {
        checkpoints.tick()?;
        target.push(line);
    }
    Ok(())
}
