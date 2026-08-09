use super::model::SequenceControlKind;
use super::render::SequenceChars;
use super::text::{
    SequenceBatchExtent, SequenceExtentLedger, SequenceLine, blank_line, charge_text_work,
    padded_line, trim_right,
};
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::error::{AsciiError, Result};
use crate::options::TerminalWidthProfile;
#[cfg(test)]
use crate::resource::AsciiResourceLimitId;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::text::display_width_with_profile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceControlFrame {
    pub(super) kind: SequenceControlKind,
    pub(super) label: String,
    pub(super) background: Option<AsciiRgb>,
    pub(super) start_row: usize,
    pub(super) separators: Vec<SequenceControlFrameSeparator>,
    pub(super) end_row: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceControlFrameSeparator {
    pub(super) label: String,
    pub(super) row: usize,
}

impl SequenceControlFrame {
    pub(super) fn current_section_start_row(&self) -> usize {
        self.separators
            .last()
            .map(|separator| separator.row)
            .unwrap_or(self.start_row)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct SequenceControlFrameNode {
    frame_index: usize,
    children: Vec<usize>,
    depth: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct SequenceControlFrameForest {
    nodes: Vec<SequenceControlFrameNode>,
    roots: Vec<usize>,
}

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

pub(super) fn render_sequence_control_frames(
    lines: Vec<SequenceLine>,
    frames: &[SequenceControlFrame],
    chars: &SequenceChars,
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    if frames.is_empty() || lines.is_empty() {
        return Ok(lines);
    }

    let line_count = lines.len();
    let width_profile = lines[0].width_profile();
    let input_width = lines.iter().map(SequenceLine::len).max().unwrap_or(0);
    resources.grid_extent(input_width, line_count)?;
    charge_work_product(resources, frames.len(), 2)?;
    resources.grid_extent(frames.len(), 1)?;
    let forest = control_frame_tree(frames, line_count, resources)?;
    if forest.nodes.is_empty() {
        return Ok(lines);
    }

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
    for root in forest.roots.iter().rev() {
        let root_node = forest.nodes.get(*root).ok_or_else(invalid_control_frame)?;
        resources.check_nesting_depth(root_node.depth)?;
        traversal.push((*root, false));
    }

    while let Some((node_index, expanded)) = traversal.pop() {
        if expanded {
            let rendered = render_frame_node_iterative(
                node_index,
                &forest,
                frames,
                &mut remaining_lines,
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
        for child in node.children.iter().rev() {
            let child_node = forest.nodes.get(*child).ok_or_else(invalid_control_frame)?;
            resources.check_nesting_depth(child_node.depth)?;
            traversal.push((*child, false));
        }
    }

    let mut rendered = Vec::new();
    let mut rendered_extent = SequenceExtentLedger::default();
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
            extend_taken_lines(
                &mut rendered,
                &mut remaining_lines,
                row..frame.start_row,
                &mut rendered_extent,
                resources,
            )?;
        }
        let frame_lines = rendered_nodes
            .get_mut(*root)
            .and_then(Option::take)
            .ok_or_else(invalid_control_frame)?;
        extend_owned_lines(&mut rendered, frame_lines, &mut rendered_extent, resources)?;
        row = resources.checked_grid_add(node_end, 1)?;
    }

    if row < line_count {
        extend_taken_lines(
            &mut rendered,
            &mut remaining_lines,
            row..line_count,
            &mut rendered_extent,
            resources,
        )?;
    }
    if remaining_lines.iter().any(Option::is_some) || rendered_nodes.iter().any(Option::is_some) {
        return Err(invalid_control_frame());
    }
    Ok(rendered)
}

#[allow(clippy::too_many_arguments)]
fn render_frame_node_iterative(
    node_index: usize,
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame],
    lines: &mut [Option<SequenceLine>],
    chars: &SequenceChars,
    width_profile: TerminalWidthProfile,
    rendered_nodes: &mut [Option<Vec<SequenceLine>>],
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    let node = forest
        .nodes
        .get(node_index)
        .ok_or_else(invalid_control_frame)?;
    resources.check_nesting_depth(node.depth)?;
    resources.charge_layout_work(1)?;
    let inset_levels = node
        .depth
        .checked_sub(1)
        .ok_or_else(invalid_control_frame)?;
    let inset = resources.checked_grid_mul(inset_levels, 2)?;
    let frame = frames
        .get(node.frame_index)
        .ok_or_else(invalid_control_frame)?;
    charge_text_work(&frame.label, width_profile, resources)?;
    for separator in &frame.separators {
        charge_text_work(&separator.label, width_profile, resources)?;
    }
    let (planned_body_rows, max_content_width) =
        planned_frame_body_extent(node_index, forest, frames, lines, rendered_nodes, resources)?;
    let width = frame_width(frame, max_content_width, inset, width_profile, resources)?;
    let row_count = resources.checked_grid_add(planned_body_rows, 2)?;
    let total_width = resources.checked_grid_add(inset, width)?;
    resources.grid_extent(total_width, row_count)?;
    charge_work_product(resources, total_width, row_count)?;
    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;
    let body = render_frame_body_iterative(
        node_index,
        forest,
        frames,
        lines,
        rendered_nodes,
        planned_body_rows,
        resources,
    )?;
    let mut content = body.content.into_iter();
    rendered.push(render_top_border(
        frame,
        inset,
        width,
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
                    inset,
                    width,
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
                    inset,
                    width,
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

    rendered.push(render_bottom_border(
        inset,
        width,
        chars,
        frame.background,
        width_profile,
        resources,
    )?);
    Ok(rendered)
}

fn planned_frame_body_extent(
    node_index: usize,
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame],
    lines: &[Option<SequenceLine>],
    rendered_nodes: &[Option<Vec<SequenceLine>>],
    resources: &mut ResourceContext,
) -> Result<(usize, usize)> {
    let node = forest
        .nodes
        .get(node_index)
        .ok_or_else(invalid_control_frame)?;
    let frame = frames
        .get(node.frame_index)
        .ok_or_else(invalid_control_frame)?;
    let end_row = frame.end_row.ok_or_else(invalid_control_frame)?;
    let mut planned_rows = 0;
    let mut max_content_width = 0;
    let mut row = frame.start_row;
    let mut child_index = 0;
    let mut separator_index = 0;

    while row <= end_row {
        resources.charge_layout_work(1)?;
        while frame
            .separators
            .get(separator_index)
            .is_some_and(|separator| separator.row == row)
        {
            planned_rows = resources.checked_grid_add(planned_rows, 1)?;
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
                    .get(child_node_index)
                    .and_then(Option::as_ref)
                    .ok_or_else(invalid_control_frame)?;
                planned_rows = resources.checked_grid_add(planned_rows, child_lines.len())?;
                max_content_width = max_content_width
                    .max(child_lines.iter().map(SequenceLine::len).max().unwrap_or(0));
                row = resources
                    .checked_grid_add(child_frame.end_row.ok_or_else(invalid_control_frame)?, 1)?;
                child_index = resources.checked_grid_add(child_index, 1)?;
                continue;
            }
        }

        let line = lines
            .get(row)
            .and_then(Option::as_ref)
            .ok_or_else(invalid_control_frame)?;
        planned_rows = resources.checked_grid_add(planned_rows, 1)?;
        max_content_width = max_content_width.max(line.len());
        row = resources.checked_grid_add(row, 1)?;
    }

    Ok((planned_rows, max_content_width))
}

fn render_frame_body_iterative(
    node_index: usize,
    forest: &SequenceControlFrameForest,
    frames: &[SequenceControlFrame],
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
            body_rows.try_reserve(1).map_err(|_| allocation_failed())?;
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
                resources.check_nesting_depth(child.depth)?;
                let child_lines = rendered_nodes
                    .get_mut(child_node_index)
                    .and_then(Option::take)
                    .ok_or_else(invalid_control_frame)?;
                body_rows
                    .try_reserve(child_lines.len())
                    .map_err(|_| allocation_failed())?;
                content
                    .try_reserve(child_lines.len())
                    .map_err(|_| allocation_failed())?;
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
        body_rows.try_reserve(1).map_err(|_| allocation_failed())?;
        content.try_reserve(1).map_err(|_| allocation_failed())?;
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

fn control_frame_tree(
    frames: &[SequenceControlFrame],
    line_count: usize,
    resources: &mut ResourceContext,
) -> Result<SequenceControlFrameForest> {
    let mut forest = SequenceControlFrameForest {
        nodes: Vec::<SequenceControlFrameNode>::new(),
        roots: Vec::new(),
    };
    let mut active = Vec::<usize>::new();

    for (frame_index, frame) in frames.iter().enumerate() {
        resources.charge_layout_work(1)?;
        if valid_frame_end_row(frame, line_count).is_none() {
            continue;
        }

        while let Some(node_index) = active.last().copied() {
            let node = forest
                .nodes
                .get(node_index)
                .ok_or_else(invalid_control_frame)?;
            let active_frame = frames
                .get(node.frame_index)
                .ok_or_else(invalid_control_frame)?;
            if active_frame
                .end_row
                .is_some_and(|end_row| end_row < frame.start_row)
            {
                active.pop();
            } else {
                break;
            }
        }

        let depth = active
            .len()
            .checked_add(1)
            .ok_or_else(|| nesting_overflow(resources))?;
        resources.check_nesting_depth(depth)?;
        forest
            .nodes
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        active.try_reserve(1).map_err(|_| allocation_failed())?;
        let parent_index = active.last().copied();
        if let Some(parent_index) = parent_index {
            forest
                .nodes
                .get_mut(parent_index)
                .ok_or_else(invalid_control_frame)?
                .children
                .try_reserve(1)
                .map_err(|_| allocation_failed())?;
        } else {
            forest
                .roots
                .try_reserve(1)
                .map_err(|_| allocation_failed())?;
        }

        let node_index = forest.nodes.len();
        forest.nodes.push(SequenceControlFrameNode {
            frame_index,
            children: Vec::new(),
            depth,
        });

        if let Some(parent_index) = parent_index {
            let parent = forest
                .nodes
                .get_mut(parent_index)
                .ok_or_else(invalid_control_frame)?;
            parent.children.push(node_index);
        } else {
            forest.roots.push(node_index);
        }
        active.push(node_index);
    }

    Ok(forest)
}

fn valid_frame_end_row(frame: &SequenceControlFrame, line_count: usize) -> Option<usize> {
    let end_row = frame.end_row?;
    (frame.start_row < line_count && end_row < line_count && frame.start_row <= end_row)
        .then_some(end_row)
}

fn frame_width(
    frame: &SequenceControlFrame,
    max_content_width: usize,
    inset: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<usize> {
    let max_row_width = max_content_width.saturating_sub(inset);
    let title = frame_title(frame, resources)?;
    let title_width = display_width_with_profile(&title, width_profile);
    let mut separator_width = 0;
    for separator in &frame.separators {
        let title = separator_title(frame, separator, resources)?;
        separator_width = separator_width.max(display_width_with_profile(&title, width_profile));
    }

    Ok(resources
        .checked_grid_add(max_row_width, 3)?
        .max(resources.checked_grid_add(title_width, 2)?)
        .max(3)
        .max(resources.checked_grid_add(separator_width, 2)?))
}

fn render_top_border(
    frame: &SequenceControlFrame,
    inset: usize,
    width: usize,
    chars: &SequenceChars,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let title = frame_title(frame, resources)?;
    render_border_row(
        chars.top_left,
        chars.top_right,
        chars.horizontal,
        inset,
        width,
        Some(&title),
        frame.background,
        width_profile,
        resources,
    )
}

fn render_bottom_border(
    inset: usize,
    width: usize,
    chars: &SequenceChars,
    background: Option<AsciiRgb>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    render_border_row(
        chars.bottom_left,
        chars.bottom_right,
        chars.horizontal,
        inset,
        width,
        None,
        background,
        width_profile,
        resources,
    )
}

fn render_separator_border(
    frame: &SequenceControlFrame,
    separator: &SequenceControlFrameSeparator,
    inset: usize,
    width: usize,
    chars: &SequenceChars,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let title = separator_title(frame, separator, resources)?;
    render_border_row(
        chars.tee_right,
        chars.tee_left,
        chars.horizontal,
        inset,
        width,
        Some(&title),
        frame.background,
        width_profile,
        resources,
    )
}

// The arguments map one-to-one to the terminal border primitive: geometry, optional label,
// background, and the selected width profile are intentionally kept explicit at call sites.
#[allow(clippy::too_many_arguments)]
fn render_border_row(
    left: char,
    right: char,
    horizontal: char,
    inset: usize,
    width: usize,
    label: Option<&str>,
    background: Option<AsciiRgb>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let total_width = resources.checked_grid_add(inset, width)?;
    resources.grid_extent(total_width, 1)?;
    let mut row = blank_line(total_width, width_profile, resources)?;
    paint_row_background(&mut row, inset..total_width, background);
    for x in inset..total_width {
        row.try_set_role(x, horizontal, AsciiColorRole::SequenceFrame)?;
    }
    row.try_set_role(inset, left, AsciiColorRole::SequenceFrame)?;
    row.try_set_role(
        total_width
            .checked_sub(1)
            .ok_or_else(invalid_control_frame)?,
        right,
        AsciiColorRole::SequenceFrame,
    )?;
    if let Some(label) = label {
        row.try_write_text_role(
            resources.checked_grid_add(inset, 1)?,
            label,
            AsciiColorRole::Text,
        )?;
    }
    trim_right(row)
}

fn render_content_row(
    row: SequenceLine,
    inset: usize,
    width: usize,
    chars: &SequenceChars,
    background: Option<AsciiRgb>,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let total_width = resources.checked_grid_add(inset, width)?;
    resources.grid_extent(total_width, 1)?;
    let mut row = padded_line(row, total_width)?;
    paint_row_background_if_unset(&mut row, inset..total_width, background);
    row.try_set_role(inset, chars.vertical, AsciiColorRole::SequenceFrame)?;
    row.try_set_role(
        total_width
            .checked_sub(1)
            .ok_or_else(invalid_control_frame)?,
        chars.vertical,
        AsciiColorRole::SequenceFrame,
    )?;
    trim_right(row)
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

fn frame_title(frame: &SequenceControlFrame, resources: &ResourceContext) -> Result<String> {
    control_title(frame.kind.keyword(), &frame.label, resources)
}

fn separator_title(
    frame: &SequenceControlFrame,
    separator: &SequenceControlFrameSeparator,
    resources: &ResourceContext,
) -> Result<String> {
    control_title(
        frame
            .kind
            .separator_keyword()
            .unwrap_or_else(|| frame.kind.keyword()),
        &separator.label,
        resources,
    )
}

fn control_title(keyword: &str, label: &str, resources: &ResourceContext) -> Result<String> {
    let separator_bytes = if label.is_empty() { 2 } else { 3 };
    let capacity = keyword
        .len()
        .checked_add(label.len())
        .and_then(|length| length.checked_add(separator_bytes))
        .ok_or_else(|| work_overflow(resources))?;
    let mut title = String::new();
    title
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_failed())?;
    title.push(' ');
    title.push_str(keyword);
    if !label.is_empty() {
        title.push(' ');
        title.push_str(label);
    }
    title.push(' ');
    Ok(title)
}

fn extend_taken_lines(
    target: &mut Vec<SequenceLine>,
    source: &mut [Option<SequenceLine>],
    range: std::ops::Range<usize>,
    extent: &mut SequenceExtentLedger,
    resources: &mut ResourceContext,
) -> Result<()> {
    let source = source.get_mut(range).ok_or_else(invalid_control_frame)?;
    let batch = batch_extent_from_optional_lines(source, resources)?;
    let reservation = extent.reserve(batch, resources)?;
    let start = target.len();
    target
        .try_reserve(source.len())
        .map_err(|_| allocation_failed())?;
    for line in source {
        target.push(line.take().ok_or_else(invalid_control_frame)?);
    }
    reservation.commit(extent, &target[start..], resources)?;
    Ok(())
}

fn extend_owned_lines(
    target: &mut Vec<SequenceLine>,
    source: Vec<SequenceLine>,
    extent: &mut SequenceExtentLedger,
    resources: &mut ResourceContext,
) -> Result<()> {
    let batch = batch_extent_from_lines(&source, resources)?;
    let reservation = extent.reserve(batch, resources)?;
    let start = target.len();
    target
        .try_reserve(source.len())
        .map_err(|_| allocation_failed())?;
    target.extend(source);
    reservation.commit(extent, &target[start..], resources)?;
    Ok(())
}

fn batch_extent_from_lines(
    lines: &[SequenceLine],
    resources: &ResourceContext,
) -> Result<SequenceBatchExtent> {
    SequenceBatchExtent::from_line_lengths(0, lines.iter().map(SequenceLine::len), resources)
}

fn batch_extent_from_optional_lines(
    lines: &[Option<SequenceLine>],
    resources: &ResourceContext,
) -> Result<SequenceBatchExtent> {
    SequenceBatchExtent::try_from_line_lengths(
        0,
        lines
            .iter()
            .map(|line| Ok(line.as_ref().ok_or_else(invalid_control_frame)?.len())),
        resources,
    )
}

fn charge_work_product(resources: &mut ResourceContext, left: usize, right: usize) -> Result<()> {
    resources.charge_layout_work_product(left, right)
}

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
}

fn nesting_overflow(resources: &ResourceContext) -> AsciiError {
    resources.nesting_overflow()
}

fn invalid_control_frame() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "control block ordering",
    }
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::AsciiResourcePolicy;
    #[cfg(not(target_arch = "wasm32"))]
    use merman_core::resources::ResourceProfile;

    #[test]
    fn nested_frames_fail_at_the_configured_depth_before_rendering() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxNestingDepth, 1)
            .expect("the nesting override should be valid");
        let mut resources = ResourceContext::new(policy);
        let line = blank_line(1, TerminalWidthProfile::Unicode, &resources)
            .expect("the seed row should fit");
        let frames = vec![
            SequenceControlFrame {
                kind: SequenceControlKind::Loop,
                label: String::new(),
                background: None,
                start_row: 0,
                separators: Vec::new(),
                end_row: Some(0),
            },
            SequenceControlFrame {
                kind: SequenceControlKind::Opt,
                label: String::new(),
                background: None,
                start_row: 0,
                separators: Vec::new(),
                end_row: Some(0),
            },
        ];

        let error =
            render_sequence_control_frames(vec![line], &frames, &ascii_chars(), &mut resources)
                .expect_err("the second frame should exceed the nesting policy");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxNestingDepth
                    && details.actual == 2
                    && details.max == 1
        ));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn deeply_nested_frames_render_on_a_small_stack() {
        const DEPTH: usize = 96;

        let rendered_len = std::thread::Builder::new()
            .name("sequence-control-small-stack".to_string())
            .stack_size(64 * 1024)
            .spawn(|| {
                let policy =
                    AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
                let mut resources = ResourceContext::new(policy);
                let line = blank_line(1, TerminalWidthProfile::Unicode, &resources)
                    .expect("the seed row should fit");
                let frames = vec![
                    SequenceControlFrame {
                        kind: SequenceControlKind::Loop,
                        label: String::new(),
                        background: None,
                        start_row: 0,
                        separators: Vec::new(),
                        end_row: Some(0),
                    };
                    DEPTH
                ];

                render_sequence_control_frames(vec![line], &frames, &ascii_chars(), &mut resources)
                    .expect("iterative rendering should not depend on the thread stack")
                    .len()
            })
            .expect("the small-stack thread should start")
            .join()
            .expect("the small-stack thread should finish");

        assert_eq!(rendered_len, DEPTH * 2 + 1);
    }

    fn ascii_chars() -> SequenceChars {
        SequenceChars {
            top_left: '+',
            top_right: '+',
            bottom_left: '+',
            bottom_right: '+',
            horizontal: '-',
            vertical: '|',
            active_vertical: '#',
            destroyed_mark: 'x',
            tee_down: '+',
            tee_up: '+',
            tee_right: '+',
            tee_left: '+',
            filled_arrow_right: '>',
            filled_arrow_left: '<',
            open_arrow_right: '>',
            open_arrow_left: '<',
            solid_line: '-',
            dotted_line: '.',
            self_top_right: '+',
            self_bottom: '+',
            unicode_markers: false,
        }
    }
}
