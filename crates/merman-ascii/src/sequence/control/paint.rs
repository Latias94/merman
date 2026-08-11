use super::{
    SequenceControlFrame, SequenceControlFrameForest, SequenceControlFramePlan,
    SequenceControlFrameSeparator, SequenceControlOutputAdmission, allocation_failed,
    frame_title_plan, invalid_control_frame, separator_title_plan, valid_frame_end_row,
    work_overflow,
};
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::error::Result;
use crate::options::TerminalWidthProfile;
use crate::resource::ResourceContext;
use crate::sequence::layout::SequenceLayout;
use crate::sequence::render::SequenceChars;
use crate::sequence::text::{SequenceLine, padded_line};

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

#[allow(clippy::too_many_arguments)]
pub(super) fn materialize_control_frames(
    lines: Vec<SequenceLine>,
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame<'_>],
    frame_plans: &[SequenceControlFramePlan],
    output_admission: SequenceControlOutputAdmission,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    let line_count = lines.len();
    let width_profile = lines
        .first()
        .map(SequenceLine::width_profile)
        .ok_or_else(invalid_control_frame)?;
    let mut remaining_lines = Vec::new();
    remaining_lines
        .try_reserve_exact(line_count)
        .map_err(|_| allocation_failed())?;
    remaining_lines.extend(lines.into_iter().map(Some));

    let mut rendered_nodes = Vec::new();
    rendered_nodes
        .try_reserve_exact(forest.nodes.len())
        .map_err(|_| allocation_failed())?;
    rendered_nodes.resize_with(forest.nodes.len(), || None);

    let mut traversal = Vec::new();
    traversal
        .try_reserve_exact(forest.nodes.len())
        .map_err(|_| allocation_failed())?;
    traversal.extend(forest.roots.iter().rev().map(|root| (*root, false)));

    while let Some((node_index, expanded)) = traversal.pop() {
        if expanded {
            let rendered = render_frame_node_iterative(
                node_index,
                forest,
                frames,
                frame_plans,
                &mut remaining_lines,
                layout,
                chars,
                width_profile,
                &mut rendered_nodes,
                resources,
            )?;
            let slot = rendered_nodes
                .get_mut(node_index)
                .ok_or_else(invalid_control_frame)?;
            if slot.replace(rendered).is_some() {
                return Err(invalid_control_frame());
            }
            continue;
        }

        let node = forest
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
        traversal.extend(node.children.iter().rev().map(|child| (*child, false)));
    }

    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(output_admission.height)
        .map_err(|_| allocation_failed())?;
    let mut row = 0;

    for root in &forest.roots {
        let node = forest.nodes.get(*root).ok_or_else(invalid_control_frame)?;
        let frame = frames
            .get(node.frame_index)
            .ok_or_else(invalid_control_frame)?;
        let Some(node_end) = valid_frame_end_row(frame, line_count) else {
            continue;
        };

        if row < frame.start_row {
            extend_taken_lines(&mut rendered, &mut remaining_lines, row..frame.start_row)?;
        }
        let frame_lines = rendered_nodes
            .get_mut(*root)
            .and_then(Option::take)
            .ok_or_else(invalid_control_frame)?;
        extend_owned_lines(&mut rendered, frame_lines)?;
        row = resources.checked_grid_add(node_end, 1)?;
    }

    if row < line_count {
        extend_taken_lines(&mut rendered, &mut remaining_lines, row..line_count)?;
    }
    if remaining_lines.iter().any(Option::is_some) || rendered_nodes.iter().any(Option::is_some) {
        return Err(invalid_control_frame());
    }
    output_admission.validate(&rendered, resources)?;
    Ok(rendered)
}

#[allow(clippy::too_many_arguments)]
fn render_frame_node_iterative(
    node_index: usize,
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame<'_>],
    frame_plans: &[SequenceControlFramePlan],
    lines: &mut [Option<SequenceLine>],
    layout: &SequenceLayout,
    chars: &SequenceChars,
    width_profile: TerminalWidthProfile,
    rendered_nodes: &mut [Option<Vec<SequenceLine>>],
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    let node = forest
        .nodes
        .get(node_index)
        .ok_or_else(invalid_control_frame)?;
    let frame = frames
        .get(node.frame_index)
        .ok_or_else(invalid_control_frame)?;
    let plan = frame_plans
        .get(node_index)
        .copied()
        .ok_or_else(invalid_control_frame)?;
    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(plan.row_count)
        .map_err(|_| allocation_failed())?;
    let body = render_frame_body_iterative(
        node_index,
        forest,
        frames,
        lines,
        rendered_nodes,
        plan.body_rows,
        resources,
    )?;
    let mut content = body.content.into_iter();
    rendered.push(render_top_border(
        frame,
        plan,
        layout,
        chars,
        width_profile,
        resources,
    )?);

    for row in body.rows {
        match row {
            SequenceControlBodyRow::Content => {
                let line = content.next().ok_or_else(invalid_control_frame)?;
                rendered.push(render_content_row(
                    line,
                    plan,
                    chars,
                    frame.background,
                    resources,
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
                    plan,
                    layout,
                    chars,
                    width_profile,
                    resources,
                )?);
            }
        }
    }
    if content.next().is_some() {
        return Err(invalid_control_frame());
    }

    rendered.push(render_bottom_border(frame, plan, layout, chars, resources)?);
    Ok(rendered)
}

fn render_frame_body_iterative(
    node_index: usize,
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame<'_>],
    lines: &mut [Option<SequenceLine>],
    rendered_nodes: &mut [Option<Vec<SequenceLine>>],
    planned_rows: usize,
    resources: &mut ResourceContext,
) -> Result<SequenceControlBody> {
    let node = forest
        .nodes
        .get(node_index)
        .ok_or_else(invalid_control_frame)?;
    let frame = frames
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
        while frame
            .separators
            .get(separator_index)
            .is_some_and(|separator| separator.row == row)
        {
            body_rows.push(SequenceControlBodyRow::Separator(separator_index));
            separator_index = resources.checked_grid_add(separator_index, 1)?;
        }

        if let Some(child_node_index) = node.children.get(child_index).copied() {
            let child = forest
                .nodes
                .get(child_node_index)
                .ok_or_else(invalid_control_frame)?;
            let child_frame = frames
                .get(child.frame_index)
                .ok_or_else(invalid_control_frame)?;
            if child_frame.start_row == row {
                let child_lines = rendered_nodes
                    .get_mut(child_node_index)
                    .and_then(Option::take)
                    .ok_or_else(invalid_control_frame)?;
                body_rows.extend(std::iter::repeat_n(
                    SequenceControlBodyRow::Content,
                    child_lines.len(),
                ));
                content.extend(child_lines);
                row = resources
                    .checked_grid_add(child_frame.end_row.ok_or_else(invalid_control_frame)?, 1)?;
                child_index = resources.checked_grid_add(child_index, 1)?;
                continue;
            }
        }

        let line = lines
            .get_mut(row)
            .and_then(Option::take)
            .ok_or_else(invalid_control_frame)?;
        body_rows.push(SequenceControlBodyRow::Content);
        content.push(line);
        row = resources.checked_grid_add(row, 1)?;
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
    plan: SequenceControlFramePlan,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let title = frame_title_plan(frame, width_profile, resources)?.materialize(resources)?;
    let base = frame
        .start_boundary
        .render_lifeline(layout, chars, resources)?;
    render_border_row(
        base,
        chars.top_left,
        chars.top_right,
        chars.horizontal,
        plan,
        Some(&title),
        frame.background,
        resources,
    )
}

fn render_bottom_border(
    frame: &SequenceControlFrame<'_>,
    plan: SequenceControlFramePlan,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let base = frame
        .end_boundary
        .as_ref()
        .ok_or_else(invalid_control_frame)?
        .render_lifeline(layout, chars, resources)?;
    render_border_row(
        base,
        chars.bottom_left,
        chars.bottom_right,
        chars.horizontal,
        plan,
        None,
        frame.background,
        resources,
    )
}

fn render_separator_border(
    frame: &SequenceControlFrame<'_>,
    separator: &SequenceControlFrameSeparator<'_>,
    plan: SequenceControlFramePlan,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let title =
        separator_title_plan(frame, separator, width_profile, resources)?.materialize(resources)?;
    let base = separator
        .boundary
        .render_lifeline(layout, chars, resources)?;
    render_border_row(
        base,
        chars.tee_right,
        chars.tee_left,
        chars.horizontal,
        plan,
        Some(&title),
        frame.background,
        resources,
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
    plan: SequenceControlFramePlan,
    label: Option<&str>,
    background: Option<AsciiRgb>,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let mut row = padded_line(base, plan.total_width)?;
    let left_index = plan.bounds.left();
    let right_index = plan.bounds.right();
    let frame_end = plan.bounds.right_exclusive(resources)?;
    paint_row_background(&mut row, left_index..frame_end, background);
    for x in left_index..frame_end {
        row.try_set_role(x, horizontal, AsciiColorRole::SequenceFrame)?;
    }
    row.try_set_role(left_index, left, AsciiColorRole::SequenceFrame)?;
    row.try_set_role(right_index, right, AsciiColorRole::SequenceFrame)?;
    if let Some(label) = label {
        row.try_write_text_role(
            resources.checked_grid_add(left_index, 1)?,
            label,
            AsciiColorRole::Text,
        )?;
    }
    Ok(row)
}

fn render_content_row(
    row: SequenceLine,
    plan: SequenceControlFramePlan,
    chars: &SequenceChars,
    background: Option<AsciiRgb>,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let mut row = padded_line(row, plan.total_width)?;
    let frame_end = plan.bounds.right_exclusive(resources)?;
    paint_row_background_if_unset(&mut row, plan.bounds.left()..frame_end, background);
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
) {
    let Some(background) = background else {
        return;
    };
    for x in range {
        row.set_background_color(x, background);
    }
}

fn paint_row_background_if_unset(
    row: &mut SequenceLine,
    range: impl Iterator<Item = usize>,
    background: Option<AsciiRgb>,
) {
    let Some(background) = background else {
        return;
    };
    for x in range {
        row.set_background_color_if_unset(x, background);
    }
}

fn extend_taken_lines(
    target: &mut Vec<SequenceLine>,
    source: &mut [Option<SequenceLine>],
    range: std::ops::Range<usize>,
) -> Result<()> {
    let source = source.get_mut(range).ok_or_else(invalid_control_frame)?;
    target
        .try_reserve(source.len())
        .map_err(|_| allocation_failed())?;
    for line in source {
        target.push(line.take().ok_or_else(invalid_control_frame)?);
    }
    Ok(())
}

fn extend_owned_lines(target: &mut Vec<SequenceLine>, source: Vec<SequenceLine>) -> Result<()> {
    target
        .try_reserve(source.len())
        .map_err(|_| allocation_failed())?;
    target.extend(source);
    Ok(())
}
