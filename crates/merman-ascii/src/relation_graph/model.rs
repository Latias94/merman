use super::{grid_overflow, layout_allocation_failed};
use crate::Result;
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::options::TerminalWidthProfile;
use crate::resource::ResourceContext;
use crate::safe_text::{DeferredTextLine, DeferredTextRegistry};
use crate::text::StyledLine;
#[cfg(test)]
use crate::text::display_width_with_profile;
use std::rc::Rc;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RelationGraphLine {
    pub(super) line: Rc<StyledLine>,
}

impl Clone for RelationGraphLine {
    fn clone(&self) -> Self {
        self.shared()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphBox {
    pub(super) id: Rc<String>,
    pub(super) lines: Rc<Vec<RelationGraphLine>>,
    pub(super) width: usize,
    pub(super) width_profile: TerminalWidthProfile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RelationGraphBoxStyle {
    pub(crate) top_left: char,
    pub(crate) top_right: char,
    pub(crate) bottom_left: char,
    pub(crate) bottom_right: char,
    pub(crate) horizontal: char,
    pub(crate) vertical: char,
    pub(crate) separator_left: char,
    pub(crate) separator_right: char,
    pub(crate) border_role: AsciiColorRole,
    pub(crate) text_role: AsciiColorRole,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RelationGraphLabel {
    lines: Rc<Vec<DeferredTextLine>>,
    width: usize,
    width_profile: TerminalWidthProfile,
}
impl RelationGraphLabel {
    pub(crate) fn try_new<'a>(
        raw: &'a str,
        width_profile: TerminalWidthProfile,
        deferred: &mut DeferredTextRegistry<'a>,
        resources: &ResourceContext,
    ) -> Result<Option<Self>> {
        let Some(lines) = deferred.try_register_label_lines(raw, width_profile, resources)? else {
            return Ok(None);
        };
        Self::try_from_lines(lines, width_profile, resources).map(Some)
    }

    #[cfg(test)]
    pub(crate) fn new<'a>(
        raw: &'a str,
        width_profile: TerminalWidthProfile,
        deferred: &mut DeferredTextRegistry<'a>,
        resources: &ResourceContext,
    ) -> Option<Self> {
        Self::try_new(raw, width_profile, deferred, resources)
            .expect("test relation label should plan")
    }

    pub(crate) fn try_from_lines(
        lines: Vec<DeferredTextLine>,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let width = lines
            .iter()
            .map(DeferredTextLine::width)
            .max()
            .unwrap_or_default();
        resources.grid_extent(width.max(1), lines.len().max(1))?;
        Ok(Self {
            lines: Rc::new(lines),
            width,
            width_profile,
        })
    }

    pub(crate) fn lines(&self) -> &[DeferredTextLine] {
        &self.lines
    }

    pub(crate) fn shared_lines(&self) -> Rc<Vec<DeferredTextLine>> {
        Rc::clone(&self.lines)
    }

    pub(crate) fn half_width(&self) -> usize {
        self.width / 2
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn width_profile(&self) -> TerminalWidthProfile {
        self.width_profile
    }
}

impl RelationGraphLine {
    #[cfg(test)]
    pub(crate) fn plain(text: String, width_profile: TerminalWidthProfile) -> Self {
        let line = StyledLine::plain_text_with_profile(&text, width_profile);
        Self::from_styled(line)
    }

    pub(crate) fn try_plain(
        text: &str,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_plain_text(text)?;
        Ok(Self::from_styled(line))
    }

    pub(crate) fn try_blank(
        width: usize,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let line = StyledLine::try_blank_with_resources(width, width_profile, resources)?;
        Ok(Self::from_styled(line))
    }

    #[cfg(test)]
    pub(crate) fn with_role(
        text: String,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        let line = StyledLine::role_text_with_profile(&text, role, width_profile);
        Self::from_styled(line)
    }

    pub(crate) fn try_with_role(
        text: &str,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_text(text, role)?;
        Ok(Self::from_styled(line))
    }

    pub(crate) fn try_role_char(
        ch: char,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_char(ch, role)?;
        Ok(Self::from_styled(line))
    }

    pub(crate) fn try_role_repeat(
        ch: char,
        count: usize,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_repeat(ch, count, role)?;
        Ok(Self::from_styled(line))
    }

    pub(crate) fn try_box_border(
        left: char,
        right: char,
        horizontal: char,
        content_width: usize,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_char(left, role)?;
        line.try_push_role_repeat(horizontal, content_width, role)?;
        line.try_push_role_char(right, role)?;
        Ok(Self::from_styled(line))
    }

    #[cfg(test)]
    pub(crate) fn box_content(
        text: &str,
        content_width: usize,
        padding: usize,
        style: RelationGraphBoxStyle,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let text_width = display_width_with_profile(text, width_profile);
        let used_width = resources.checked_grid_add(padding, text_width)?;
        let trailing = content_width
            .checked_sub(used_width)
            .ok_or_else(|| grid_overflow(resources))?;

        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_char(style.vertical, style.border_role)?;
        line.try_push_spaces(padding)?;
        line.try_push_role_text(text, style.text_role)?;
        line.try_push_spaces(trailing)?;
        line.try_push_role_char(style.vertical, style.border_role)?;
        Ok(Self::from_styled(line))
    }

    pub(crate) fn deferred_box_content(
        text: &DeferredTextLine,
        content_width: usize,
        padding: usize,
        style: RelationGraphBoxStyle,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let used_width = resources.checked_grid_add(padding, text.width())?;
        let trailing = content_width
            .checked_sub(used_width)
            .ok_or_else(|| grid_overflow(resources))?;

        let mut line = StyledLine::with_resources(width_profile, resources);
        line.try_push_role_char(style.vertical, style.border_role)?;
        line.try_push_spaces(padding)?;
        line.try_push_deferred_text(text, style.text_role)?;
        line.try_push_spaces(trailing)?;
        line.try_push_role_char(style.vertical, style.border_role)?;
        Ok(Self::from_styled(line))
    }

    #[cfg(test)]
    pub(crate) fn text(&self) -> String {
        self.line.text()
    }

    pub(crate) fn draw_at(&self, canvas: &mut Canvas, x: usize, y: usize) -> Result<()> {
        self.line.try_write_to_at(canvas, x, y)
    }

    pub(crate) fn width(&self) -> usize {
        self.line.len()
    }

    pub(crate) fn width_profile(&self) -> TerminalWidthProfile {
        self.line.width_profile()
    }

    pub(crate) fn from_styled(line: StyledLine) -> Self {
        Self {
            line: Rc::new(line),
        }
    }

    pub(super) fn styled(&self) -> &StyledLine {
        &self.line
    }

    pub(super) fn shared(&self) -> Self {
        Self {
            line: Rc::clone(&self.line),
        }
    }
}

impl RelationGraphBox {
    #[cfg(test)]
    pub(crate) fn new(id: String, lines: Vec<String>, width: usize) -> Self {
        let width_profile = TerminalWidthProfile::Unicode;
        let lines = lines
            .into_iter()
            .map(|line| RelationGraphLine::plain(line, width_profile))
            .collect::<Vec<_>>();
        Self {
            id: Rc::new(id),
            lines: Rc::new(lines),
            width,
            width_profile,
        }
    }

    pub(crate) fn new_with_lines(
        id: String,
        lines: Vec<RelationGraphLine>,
        width: usize,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        debug_assert!(
            lines
                .iter()
                .all(|line| line.width_profile() == width_profile),
            "relation graph box lines must share one terminal width profile"
        );
        Self {
            id: Rc::new(id),
            lines: Rc::new(lines),
            width,
            width_profile,
        }
    }

    pub(crate) fn from_rendered_lines(
        id: String,
        lines: Vec<RelationGraphLine>,
        width_profile: TerminalWidthProfile,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let width = lines.iter().map(line_char_width).max().unwrap_or(0);
        let extent = resources.grid_extent(width, lines.len())?;
        resources.charge_layout_work(extent.cells())?;
        Ok(Self::new_with_lines(id, lines, width, width_profile))
    }

    #[cfg(test)]
    pub(crate) fn from_sections(
        id: String,
        sections: &[Vec<String>],
        padding: usize,
        style: RelationGraphBoxStyle,
        width_profile: TerminalWidthProfile,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let content_width =
            sectioned_box_content_width(sections, padding, width_profile, resources)?;
        let separator_count = sections.len().saturating_sub(1);
        let text_line_count = sections.iter().try_fold(0usize, |total, section| {
            resources.checked_grid_add(total, section.len())
        })?;
        let height = resources.checked_grid_add(
            resources.checked_grid_add(text_line_count, separator_count)?,
            2,
        )?;
        let width = resources.checked_grid_add(content_width, 2)?;
        let extent = resources.grid_extent(width, height)?;
        resources.charge_layout_work(extent.cells())?;
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
            width_profile,
            resources,
        )?);
        for (section_index, section) in sections.iter().enumerate() {
            if section_index > 0 {
                lines.push(RelationGraphLine::try_box_border(
                    style.separator_left,
                    style.separator_right,
                    style.horizontal,
                    content_width,
                    style.border_role,
                    width_profile,
                    resources,
                )?);
            }
            for line in section {
                lines.push(RelationGraphLine::box_content(
                    line,
                    content_width,
                    padding,
                    style,
                    width_profile,
                    resources,
                )?);
            }
        }
        lines.push(RelationGraphLine::try_box_border(
            style.bottom_left,
            style.bottom_right,
            style.horizontal,
            content_width,
            style.border_role,
            width_profile,
            resources,
        )?);

        Ok(Self::new_with_lines(id, lines, width, width_profile))
    }

    pub(crate) fn from_deferred_sections(
        id: String,
        sections: &[Vec<DeferredTextLine>],
        padding: usize,
        style: RelationGraphBoxStyle,
        width_profile: TerminalWidthProfile,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let max_line_width = sections
            .iter()
            .flat_map(|section| section.iter())
            .map(DeferredTextLine::width)
            .max()
            .unwrap_or(0)
            .max(1);
        let content_width =
            resources.checked_grid_add(max_line_width, resources.checked_grid_mul(padding, 2)?)?;
        let separator_count = sections.len().saturating_sub(1);
        let text_line_count = sections.iter().try_fold(0usize, |total, section| {
            resources.checked_grid_add(total, section.len())
        })?;
        let height = resources.checked_grid_add(
            resources.checked_grid_add(text_line_count, separator_count)?,
            2,
        )?;
        let width = resources.checked_grid_add(content_width, 2)?;
        let extent = resources.grid_extent(width, height)?;
        resources.charge_layout_work(extent.cells())?;
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
            width_profile,
            resources,
        )?);
        for (section_index, section) in sections.iter().enumerate() {
            if section_index > 0 {
                lines.push(RelationGraphLine::try_box_border(
                    style.separator_left,
                    style.separator_right,
                    style.horizontal,
                    content_width,
                    style.border_role,
                    width_profile,
                    resources,
                )?);
            }
            for line in section {
                lines.push(RelationGraphLine::deferred_box_content(
                    line,
                    content_width,
                    padding,
                    style,
                    width_profile,
                    resources,
                )?);
            }
        }
        lines.push(RelationGraphLine::try_box_border(
            style.bottom_left,
            style.bottom_right,
            style.horizontal,
            content_width,
            style.border_role,
            width_profile,
            resources,
        )?);

        Ok(Self::new_with_lines(id, lines, width, width_profile))
    }

    pub(crate) fn id(&self) -> &str {
        self.id.as_str()
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn lines(&self) -> &[RelationGraphLine] {
        self.lines.as_slice()
    }

    pub(crate) fn height(&self) -> usize {
        self.lines.len()
    }

    pub(crate) fn width_profile(&self) -> TerminalWidthProfile {
        self.width_profile
    }

    pub(super) fn shared_projection(&self) -> Self {
        Self {
            id: Rc::clone(&self.id),
            lines: Rc::clone(&self.lines),
            width: self.width,
            width_profile: self.width_profile,
        }
    }

    pub(crate) fn draw_at(
        &self,
        canvas: &mut Canvas,
        x: usize,
        y: usize,
        resources: &ResourceContext,
    ) -> Result<()> {
        for (row_index, line) in self.lines.iter().enumerate() {
            let row_y = y
                .checked_add(row_index)
                .ok_or_else(|| grid_overflow(resources))?;
            line.draw_at(canvas, x, row_y)?;
        }
        Ok(())
    }
}

#[cfg(test)]
fn sectioned_box_content_width(
    sections: &[Vec<String>],
    padding: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<usize> {
    let max_line_width = sections
        .iter()
        .flat_map(|section| section.iter())
        .map(|line| display_width_with_profile(line, width_profile))
        .max()
        .unwrap_or(0)
        .max(1);
    let total_padding = resources.checked_grid_mul(padding, 2)?;
    resources.checked_grid_add(max_line_width, total_padding)
}

fn line_char_width(line: &RelationGraphLine) -> usize {
    line.width()
}
