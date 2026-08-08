use super::super::{RelationGraphLabel, RelationGraphLine, try_concat_relation_lines};
use crate::Result;
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::options::TerminalWidthProfile;
use crate::resource::ResourceContext;
use crate::text::display_width_with_profile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RelationLineChars {
    line_chars: [char; 4],
    junction: char,
}

impl RelationLineChars {
    pub(crate) fn new(line_chars: [char; 4], junction: char) -> Self {
        Self {
            line_chars,
            junction,
        }
    }

    fn contains(self, ch: char) -> bool {
        self.line_chars.contains(&ch) || ch == self.junction
    }
}

pub(super) fn draw_relation_span_inclusive(
    canvas: &mut Canvas,
    x: usize,
    start_y: usize,
    end_y: usize,
    ch: char,
    chars: RelationLineChars,
) -> Result<()> {
    let start = start_y.min(end_y);
    let end = start_y.max(end_y);
    for y in start..=end {
        put_relation_char(canvas, x, y, ch, chars)?;
    }
    Ok(())
}

pub(super) fn draw_relation_span_exclusive(
    canvas: &mut Canvas,
    x: usize,
    start_y: usize,
    end_y: usize,
    ch: char,
    chars: RelationLineChars,
) -> Result<()> {
    if start_y <= end_y {
        for y in start_y..end_y {
            put_relation_char(canvas, x, y, ch, chars)?;
        }
        return Ok(());
    }

    if let Some(first_y) = end_y.checked_add(1) {
        for y in first_y..=start_y {
            put_relation_char(canvas, x, y, ch, chars)?;
        }
    }
    Ok(())
}

pub(crate) fn marker_line_with_role(
    marker: char,
    center: usize,
    role: AsciiColorRole,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<RelationGraphLine> {
    try_concat_relation_lines(
        vec![
            RelationGraphLine::try_blank(center, width_profile, resources)?,
            RelationGraphLine::try_role_char(marker, role, width_profile, resources)?,
        ],
        width_profile,
        resources,
    )
}

pub(crate) fn centered_text_line_with_role(
    text: &str,
    center: usize,
    role: AsciiColorRole,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<RelationGraphLine> {
    let half_width = display_width_with_profile(text, width_profile) / 2;
    let left_padding = center.checked_sub(half_width).ok_or_else(|| {
        resources
            .policy()
            .overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
    })?;
    try_concat_relation_lines(
        vec![
            RelationGraphLine::try_blank(left_padding, width_profile, resources)?,
            RelationGraphLine::try_with_role(text, role, width_profile, resources)?,
        ],
        width_profile,
        resources,
    )
}

pub(crate) fn centered_label_lines_with_role(
    label: &RelationGraphLabel,
    center: usize,
    role: AsciiColorRole,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let mut lines = Vec::new();
    lines.try_reserve_exact(label.line_count()).map_err(|_| {
        crate::AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        }
    })?;
    for line in label.lines() {
        lines.push(centered_text_line_with_role(
            line,
            center,
            role,
            label.width_profile(),
            resources,
        )?);
    }
    Ok(lines)
}

pub(crate) fn label_lines_with_role(
    label: &RelationGraphLabel,
    role: AsciiColorRole,
    resources: &ResourceContext,
) -> Result<Vec<RelationGraphLine>> {
    let mut lines = Vec::new();
    lines.try_reserve_exact(label.line_count()).map_err(|_| {
        crate::AsciiError::AllocationFailed {
            phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
        }
    })?;
    for line in label.lines() {
        lines.push(RelationGraphLine::try_with_role(
            line,
            role,
            label.width_profile(),
            resources,
        )?);
    }
    Ok(lines)
}

pub(crate) fn put_relation_char(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    ch: char,
    chars: RelationLineChars,
) -> Result<()> {
    let next = match canvas.get(x, y) {
        Some(existing) if existing == ' ' || existing == ch => ch,
        Some(existing) if chars.contains(existing) && chars.contains(ch) => chars.junction,
        _ => ch,
    };
    let role = if next == chars.junction {
        AsciiColorRole::Junction
    } else {
        AsciiColorRole::EdgeLine
    };
    canvas.try_set_role(x, y, next, role)
}

pub(crate) fn write_centered_relation_text(
    canvas: &mut Canvas,
    center_x: usize,
    y: usize,
    text: &str,
    role: AsciiColorRole,
    width_profile: TerminalWidthProfile,
) -> Result<()> {
    let text_half_width = display_width_with_profile(text, width_profile) / 2;
    let Some(start_x) = center_x.checked_sub(text_half_width) else {
        return Ok(());
    };
    canvas.write_text_role(start_x, y, text, role)
}

pub(crate) fn write_centered_relation_label(
    canvas: &mut Canvas,
    center_x: usize,
    start_y: usize,
    label: &RelationGraphLabel,
    role: AsciiColorRole,
) -> Result<()> {
    for (offset, line) in label.lines().iter().enumerate() {
        let Some(y) = start_y.checked_add(offset) else {
            return Ok(());
        };
        write_centered_relation_text(canvas, center_x, y, line, role, label.width_profile())?;
    }
    Ok(())
}
