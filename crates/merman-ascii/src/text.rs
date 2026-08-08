use crate::canvas::Canvas;
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::error::{AsciiError, Result};
use crate::options::TerminalWidthProfile;
use crate::resource::{
    AsciiResourceLimitId, AsciiResourceLimitPhase, AsciiResourcePolicy, CheckedOutput,
    ResourceContext,
};
use crate::safe_text::{
    SafeLine, SafeText, terminal_char_display_width, terminal_line_display_width,
    visit_safe_line_graphemes,
};
#[cfg(test)]
use crate::terminal::try_mirror_surface;
use crate::terminal::{
    CanvasColor, CanvasStyle, GlyphArena, TerminalCell, is_retained_glyph_budget_error,
    owner_index, primary_width, style_at, try_append_cells_from_surface,
    try_push_primary_grapheme_style_with_policy, try_write_primary_cell_from_surface,
    try_write_primary_grapheme_style_with_policy,
};

pub(crate) type StyledCell = TerminalCell;

#[derive(Debug, Clone)]
pub(crate) struct StyledLine {
    cells: Vec<StyledCell>,
    arena: GlyphArena,
    width_profile: TerminalWidthProfile,
    resources: ResourceContext,
}

impl PartialEq for StyledLine {
    fn eq(&self, other: &Self) -> bool {
        self.cells == other.cells
            && self.arena == other.arena
            && self.width_profile == other.width_profile
            && self.resources.policy() == other.resources.policy()
    }
}

impl Eq for StyledLine {}

impl StyledLine {
    /// Test-only convenience. Production renderers must pass their selected resource policy.
    #[cfg(test)]
    pub(crate) fn with_width_profile(width_profile: TerminalWidthProfile) -> Self {
        Self::with_resource_policy(width_profile, compatibility_policy())
    }

    #[cfg(test)]
    pub(crate) fn with_resource_policy(
        width_profile: TerminalWidthProfile,
        resources: AsciiResourcePolicy,
    ) -> Self {
        let resources = ResourceContext::new(resources);
        Self::with_resources(width_profile, &resources)
    }

    pub(crate) fn with_resources(
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Self {
        Self {
            cells: Vec::new(),
            arena: GlyphArena::default(),
            width_profile,
            resources: resources.clone(),
        }
    }

    pub(crate) fn try_from_surface_cells_with_resources(
        cells: &[StyledCell],
        arena: &GlyphArena,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let line_resources = resources.clone();
        line_resources.charge_document_cells(cells.len())?;
        let (cells, arena) = GlyphArena::try_compact_surface(arena, cells, resources.policy())?;
        Ok(Self {
            cells,
            arena,
            width_profile,
            resources: line_resources,
        })
    }

    /// Test-only convenience. Production renderers must pass their selected resource policy.
    #[cfg(test)]
    pub(crate) fn blank_with_profile(width: usize, width_profile: TerminalWidthProfile) -> Self {
        let resources = compatibility_policy();
        Self::try_blank_with_policy(width, width_profile, resources)
            .expect("test terminal line should fit the unbounded resource policy")
    }

    pub(crate) fn try_blank_with_policy(
        width: usize,
        width_profile: TerminalWidthProfile,
        resources: AsciiResourcePolicy,
    ) -> Result<Self> {
        let resources = ResourceContext::new(resources);
        Self::try_blank_with_resources(width, width_profile, &resources)
    }

    pub(crate) fn try_blank_with_resources(
        width: usize,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let line_resources = resources.clone();
        line_resources.charge_document_cells(width)?;
        resources.check(AsciiResourceLimitId::MaxGridCells, width)?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(width)
            .map_err(|_| document_allocation_failed())?;
        cells.resize(width, StyledCell::blank());
        Ok(Self {
            cells,
            arena: GlyphArena::default(),
            width_profile,
            resources: line_resources,
        })
    }

    /// Test-only convenience. Production renderers must pass their selected resource policy.
    #[cfg(test)]
    pub(crate) fn role_text_with_profile(
        text: &str,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        let resources = compatibility_policy();
        Self::try_role_text_with_policy(text, role, width_profile, resources)
            .expect("test terminal text should fit the unbounded resource policy")
    }

    #[cfg(test)]
    pub(crate) fn try_role_text_with_policy(
        text: &str,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
        resources: AsciiResourcePolicy,
    ) -> Result<Self> {
        let resources = ResourceContext::new(resources);
        let mut line = Self::with_resources(width_profile, &resources);
        line.try_push_role_text(text, role)?;
        Ok(line)
    }

    /// Test-only convenience. Production renderers must pass their selected resource policy.
    #[cfg(test)]
    pub(crate) fn plain_text_with_profile(text: &str, width_profile: TerminalWidthProfile) -> Self {
        let resources = compatibility_policy();
        Self::try_plain_text_with_policy(text, width_profile, resources)
            .expect("test terminal text should fit the unbounded resource policy")
    }

    #[cfg(test)]
    pub(crate) fn try_plain_text_with_policy(
        text: &str,
        width_profile: TerminalWidthProfile,
        resources: AsciiResourcePolicy,
    ) -> Result<Self> {
        let resources = ResourceContext::new(resources);
        let mut line = Self::with_resources(width_profile, &resources);
        line.try_push_plain_text(text)?;
        Ok(line)
    }

    pub(crate) fn len(&self) -> usize {
        self.cells.len()
    }

    pub(crate) fn surface_cells(&self) -> &[StyledCell] {
        &self.cells
    }

    pub(crate) fn surface_arena(&self) -> &GlyphArena {
        &self.arena
    }

    pub(crate) fn trimmed_len(&self, preserve_color: bool) -> usize {
        self.cells
            .iter()
            .rposition(|cell| !cell.is_trimmable_blank(preserve_color))
            .map_or(0, |index| index + 1)
    }

    pub(crate) fn width_profile(&self) -> TerminalWidthProfile {
        self.width_profile
    }

    pub(crate) fn get(&self, index: usize) -> Option<char> {
        self.cells.get(index).and_then(|cell| cell.output_char())
    }

    /// Test-only convenience for assertions that do not need a fallible result.
    #[cfg(test)]
    pub(crate) fn text(&self) -> String {
        self.try_text()
            .expect("test terminal text should fit the unbounded resource policy")
    }

    #[cfg(test)]
    pub(crate) fn try_text(&self) -> Result<String> {
        let mut output = CheckedOutput::new(self.resources.policy());
        self.try_write_plain_to(&mut output)?;
        Ok(output.finish())
    }

    /// Writes this row into an existing checked output without a per-line `String`.
    pub(crate) fn try_write_plain_to(&self, output: &mut CheckedOutput) -> Result<()> {
        for cell in &self.cells {
            if let Some(text) = cell.try_output_text(&self.arena)? {
                match text {
                    crate::terminal::TerminalCellText::Scalar(ch) => output.push_char(ch)?,
                    crate::terminal::TerminalCellText::Grapheme(grapheme) => {
                        output.push_str(grapheme)?;
                    }
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn try_into_text(self) -> Result<String> {
        self.try_text()
    }

    #[cfg(test)]
    pub(crate) fn pad_to(&mut self, width: usize) {
        self.try_pad_to(width)
            .expect("test terminal padding should fit the unbounded resource policy");
    }

    pub(crate) fn try_pad_to(&mut self, width: usize) -> Result<()> {
        if self.cells.len() < width {
            let delta = width - self.cells.len();
            if let Err(error) = self.resources.charge_document_cells(delta) {
                return self.record_error(error);
            }
            if let Err(error) = self
                .resources
                .check(AsciiResourceLimitId::MaxGridCells, width)
            {
                return self.record_error(error);
            }
            if self.cells.try_reserve(width - self.cells.len()).is_err() {
                return self.record_error(document_allocation_failed());
            }
            self.cells.resize(width, StyledCell::blank());
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn push_plain_char(&mut self, ch: char) {
        self.try_push_plain_char(ch)
            .expect("test terminal character should fit the unbounded resource policy");
    }

    pub(crate) fn try_push_plain_char(&mut self, ch: char) -> Result<()> {
        self.try_push_char_style(ch, CanvasStyle::default())
    }

    pub(crate) fn try_push_plain_text(&mut self, text: &str) -> Result<()> {
        let mut resources = self.resources.scoped();
        let result = visit_safe_line_graphemes(
            &mut resources,
            text,
            self.width_profile,
            |grapheme, width| {
                self.try_push_measured_grapheme(grapheme, width, CanvasStyle::default())?;
                Ok(true)
            },
        );
        if let Err(error) = result {
            return self.record_error(error);
        }
        Ok(())
    }

    pub(crate) fn try_push_spaces(&mut self, count: usize) -> Result<()> {
        let Some(final_len) = self.cells.len().checked_add(count) else {
            return self.record_error(document_allocation_failed());
        };
        if let Err(error) = self.resources.charge_document_cells(count) {
            return self.record_error(error);
        }
        if let Err(error) = self
            .resources
            .check(AsciiResourceLimitId::MaxGridCells, final_len)
        {
            return self.record_error(error);
        }
        if self.cells.try_reserve(count).is_err() {
            return self.record_error(document_allocation_failed());
        }
        self.cells
            .extend(std::iter::repeat_n(StyledCell::blank(), count));
        Ok(())
    }

    pub(crate) fn try_push_line(&mut self, line: &StyledLine) -> Result<()> {
        if self.width_profile != line.width_profile {
            return self.record_error(width_profile_mismatch());
        }
        self.resources.charge_document_cells(line.cells.len())?;
        match try_append_cells_from_surface(
            &mut self.cells,
            &mut self.arena,
            &line.cells,
            &line.arena,
            self.resources.policy(),
        ) {
            Ok(()) => Ok(()),
            Err(error) if is_retained_glyph_budget_error(&error) => {
                if let Err(compaction_error) = self.try_compact_arena() {
                    return self.record_error(compaction_error);
                }
                match try_append_cells_from_surface(
                    &mut self.cells,
                    &mut self.arena,
                    &line.cells,
                    &line.arena,
                    self.resources.policy(),
                ) {
                    Ok(()) => Ok(()),
                    Err(error) => self.record_error(error),
                }
            }
            Err(error) => self.record_error(error),
        }
    }

    pub(crate) fn try_push_role_char(&mut self, ch: char, role: AsciiColorRole) -> Result<()> {
        self.try_push_char_style(ch, CanvasStyle::foreground(CanvasColor::Role(role)))
    }

    #[cfg(test)]
    pub(crate) fn push_role_text(&mut self, text: &str, role: AsciiColorRole) {
        self.try_push_role_text(text, role)
            .expect("test terminal text should fit the unbounded resource policy");
    }

    pub(crate) fn try_push_role_text(&mut self, text: &str, role: AsciiColorRole) -> Result<()> {
        let style = CanvasStyle::foreground(CanvasColor::Role(role));
        let mut resources = self.resources.scoped();
        let result = visit_safe_line_graphemes(
            &mut resources,
            text,
            self.width_profile,
            |grapheme, width| {
                self.try_push_measured_grapheme(grapheme, width, style)?;
                Ok(true)
            },
        );
        if let Err(error) = result {
            return self.record_error(error);
        }
        Ok(())
    }

    pub(crate) fn try_push_role_text_with_unstyled_trailing_spaces(
        &mut self,
        text: &str,
        role: AsciiColorRole,
    ) -> Result<()> {
        let trimmed = text.trim_end_matches(' ');
        self.try_push_role_text(trimmed, role)?;
        self.try_push_spaces(text.len() - trimmed.len())
    }

    pub(crate) fn try_push_role_repeat(
        &mut self,
        ch: char,
        count: usize,
        role: AsciiColorRole,
    ) -> Result<()> {
        for _ in 0..count {
            self.try_push_char_style(ch, CanvasStyle::foreground(CanvasColor::Role(role)))?;
        }
        Ok(())
    }

    pub(crate) fn try_set_role(
        &mut self,
        index: usize,
        ch: char,
        role: AsciiColorRole,
    ) -> Result<()> {
        let width = terminal_char_display_width(ch, self.width_profile);
        debug_assert_eq!(
            width, 1,
            "renderer-owned structural glyphs must occupy one terminal cell"
        );
        if width != 1 {
            return Ok(());
        }
        let background = style_at(&self.cells, index).background;
        let mut buffer = [0; 4];
        let grapheme = ch.encode_utf8(&mut buffer);
        let result = try_write_primary_grapheme_style_with_policy(
            &mut self.cells,
            &mut self.arena,
            index,
            grapheme,
            1,
            CanvasStyle {
                foreground: Some(CanvasColor::Role(role)),
                background,
            },
            self.resources.policy(),
        );
        match result {
            Ok(_) => Ok(()),
            Err(error) => self.record_error(error),
        }
    }

    pub(crate) fn set_background_color(&mut self, index: usize, color: AsciiRgb) {
        if let Some(owner) = owner_index(&self.cells, index)
            && let Some(cell) = self.cells.get_mut(owner)
        {
            cell.set_background(CanvasColor::Direct(color));
        }
    }

    pub(crate) fn set_background_color_if_unset(&mut self, index: usize, color: AsciiRgb) {
        let Some(owner) = owner_index(&self.cells, index) else {
            return;
        };
        let Some(cell) = self.cells.get_mut(owner) else {
            return;
        };
        if cell.raw_style().background.is_none() {
            cell.set_background(CanvasColor::Direct(color));
        }
    }

    pub(crate) fn try_write_text_role(
        &mut self,
        start: usize,
        text: &str,
        role: AsciiColorRole,
    ) -> Result<()> {
        let mut resources = self.resources.scoped();
        let mut write_width = 0usize;
        let measured =
            visit_safe_line_graphemes(&mut resources, text, self.width_profile, |_, width| {
                write_width = write_width
                    .checked_add(width)
                    .ok_or_else(document_allocation_failed)?;
                Ok(true)
            });
        if let Err(error) = measured {
            return self.record_error(error);
        }
        if !write_span_fits(self.cells.len(), start, write_width) {
            return self.record_error(terminal_surface_does_not_fit());
        }

        let mut offset = 0;
        let written = visit_safe_line_graphemes(
            &mut resources,
            text,
            self.width_profile,
            |grapheme, width| {
                if width == 0 {
                    return Ok(true);
                }
                let Some(index) = start.checked_add(offset) else {
                    return Err(document_allocation_failed());
                };
                let background = style_at(&self.cells, index).background;
                let result = try_write_primary_grapheme_style_with_policy(
                    &mut self.cells,
                    &mut self.arena,
                    index,
                    grapheme,
                    width,
                    CanvasStyle {
                        foreground: Some(CanvasColor::Role(role)),
                        background,
                    },
                    self.resources.policy(),
                );
                match result {
                    Ok(true) => {}
                    Ok(false) => return Err(terminal_surface_does_not_fit()),
                    Err(error) => return Err(error),
                }
                let Some(next_offset) = offset.checked_add(width) else {
                    return Err(document_allocation_failed());
                };
                offset = next_offset;
                Ok(true)
            },
        );
        if let Err(error) = written {
            return self.record_error(error);
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn write_line(&mut self, start: usize, line: &StyledLine) {
        self.try_write_line(start, line)
            .expect("test terminal composition should fit the unbounded resource policy");
    }

    pub(crate) fn try_write_line(&mut self, start: usize, line: &StyledLine) -> Result<()> {
        if self.width_profile != line.width_profile {
            return self.record_error(width_profile_mismatch());
        }
        let Some(concurrent_cells) = self.cells.len().checked_add(line.cells.len()) else {
            return self.record_error(self.resources.overflow(AsciiResourceLimitId::MaxGridCells));
        };
        if let Err(error) = self
            .resources
            .check(AsciiResourceLimitId::MaxGridCells, concurrent_cells)
        {
            return self.record_error(error);
        }
        if !write_span_fits(self.cells.len(), start, line.cells.len()) {
            return self.record_error(terminal_surface_does_not_fit());
        }
        let mut offset = 0;
        while offset < line.cells.len() {
            let cell = line.cells[offset];
            if cell.is_continuation() {
                offset += 1;
                continue;
            }
            let width = primary_width(&line.cells, offset).max(1);
            let Some(index) = start.checked_add(offset) else {
                return self.record_error(document_allocation_failed());
            };
            let result = try_write_primary_cell_from_surface(
                &mut self.cells,
                &mut self.arena,
                index,
                cell,
                width,
                &line.arena,
                self.resources.policy(),
            );
            match result {
                Ok(true) => {}
                Ok(false) => return self.record_error(terminal_surface_does_not_fit()),
                Err(error) => return self.record_error(error),
            }
            let Some(next_offset) = offset.checked_add(width) else {
                return self.record_error(document_allocation_failed());
            };
            offset = next_offset;
        }
        Ok(())
    }

    pub(crate) fn trim_right(mut self) -> Self {
        while self
            .cells
            .last()
            .is_some_and(|cell| cell.is_trimmable_blank(false))
        {
            self.cells.pop();
        }
        self
    }

    #[cfg(test)]
    pub(crate) fn write_to(&self, canvas: &mut Canvas, y: usize) {
        self.try_write_to(canvas, y)
            .expect("test terminal surface should fit the target canvas");
    }

    #[cfg(test)]
    pub(crate) fn try_write_to(&self, canvas: &mut Canvas, y: usize) -> Result<()> {
        self.try_write_to_at(canvas, 0, y)
    }

    pub(crate) fn try_write_to_at(
        &self,
        canvas: &mut Canvas,
        x_offset: usize,
        y: usize,
    ) -> Result<()> {
        if !canvas.try_write_cells_from_surface(
            x_offset,
            y,
            &self.cells,
            &self.arena,
            self.width_profile,
        )? {
            return Err(AsciiError::InvalidOption {
                field: "terminal_surface",
                message: "terminal surface does not fit the target canvas",
            });
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn try_mirrored(&self) -> Result<Self> {
        let (cells, arena) = try_mirror_surface(&self.cells, &self.arena, self.resources.policy())?;
        Ok(Self {
            cells,
            arena,
            width_profile: self.width_profile,
            resources: self.resources.scoped(),
        })
    }

    fn try_push_char_style(&mut self, ch: char, style: CanvasStyle) -> Result<()> {
        let width = terminal_char_display_width(ch, self.width_profile);
        debug_assert_eq!(
            width, 1,
            "renderer-owned structural glyphs must occupy one terminal cell"
        );
        if width != 1 {
            return Ok(());
        }
        let mut buffer = [0; 4];
        let grapheme = ch.encode_utf8(&mut buffer);
        self.try_push_measured_grapheme(grapheme, 1, style)
    }

    fn try_push_measured_grapheme(
        &mut self,
        grapheme: &str,
        width: usize,
        style: CanvasStyle,
    ) -> Result<()> {
        self.resources.charge_document_cells(width)?;
        let result = try_push_primary_grapheme_style_with_policy(
            &mut self.cells,
            &mut self.arena,
            grapheme,
            width,
            style,
            self.resources.policy(),
        );
        match result {
            Ok(()) => Ok(()),
            Err(error) => self.record_error(error),
        }
    }

    fn record_error<T>(&mut self, error: AsciiError) -> Result<T> {
        Err(error)
    }

    fn try_compact_arena(&mut self) -> Result<()> {
        self.arena
            .try_compact_in_place(&mut self.cells, self.resources.policy())
    }
}

fn width_profile_mismatch() -> AsciiError {
    AsciiError::InvalidOption {
        field: "terminal_width_profile",
        message: "cannot compose terminal surfaces with different width profiles",
    }
}

fn terminal_surface_does_not_fit() -> AsciiError {
    AsciiError::InvalidOption {
        field: "terminal_surface",
        message: "terminal surface does not fit the target line",
    }
}

fn write_span_fits(target_width: usize, start: usize, write_width: usize) -> bool {
    write_width == 0
        || start
            .checked_add(write_width)
            .is_some_and(|end| end <= target_width)
}

fn document_allocation_failed() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::Document.as_str(),
    }
}

#[cfg(test)]
fn compatibility_policy() -> AsciiResourcePolicy {
    AsciiResourcePolicy::for_profile(
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
    )
}

pub(crate) fn display_width_with_profile(text: &str, width_profile: TerminalWidthProfile) -> usize {
    terminal_line_display_width(text, width_profile)
}

pub(crate) fn truncate_display_width_with_profile(
    value: &str,
    width: usize,
    width_profile: TerminalWidthProfile,
) -> String {
    let mut out = String::new();
    let mut used = 0;

    let value = SafeLine::new(value);
    for grapheme in value.graphemes(width_profile) {
        if used + grapheme.width() > width {
            break;
        }
        out.push_str(grapheme.text());
        used += grapheme.width();
    }

    out
}

pub(crate) fn wrap_display_lines_with_profile(
    text: &str,
    max_width: usize,
    width_profile: TerminalWidthProfile,
) -> Vec<String> {
    let max_width = max_width.max(1);
    let mut lines = Vec::new();
    let normalized = SafeText::new(text);

    for paragraph in normalized.lines() {
        wrap_display_paragraph(paragraph, max_width, width_profile, &mut lines);
    }

    lines
}

pub(crate) fn normalize_optional_text(text: Option<&str>) -> Option<String> {
    let normalized = SafeText::new(text?);
    let trimmed = normalized.as_str().trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

pub(crate) fn split_label_lines(raw: &str) -> Vec<String> {
    let normalized = normalize_label_breaks(raw);
    SafeText::new(&normalized)
        .as_str()
        .split('\n')
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn wrap_label_lines_with_profile(
    raw: &str,
    max_width: usize,
    width_profile: TerminalWidthProfile,
) -> Vec<String> {
    let normalized = normalize_label_breaks(raw);
    let mut lines = Vec::new();
    for paragraph in normalized.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
        } else {
            lines.extend(wrap_display_lines_with_profile(
                paragraph,
                max_width,
                width_profile,
            ));
        }
    }
    lines
}

fn normalize_label_breaks(raw: &str) -> String {
    let mut normalized = String::with_capacity(raw.len());
    let mut index = 0;

    while index < raw.len() {
        if let Some(end) = html_break_end(raw, index) {
            normalized.push('\n');
            index = end;
            continue;
        }
        if raw[index..].starts_with("\\n") {
            normalized.push('\n');
            index += 2;
            continue;
        }

        // This scalar advances the UTF-8 syntax scanner; text layout remains grapheme-based.
        let Some(ch) = raw[index..].chars().next() else {
            break;
        };
        normalized.push(ch);
        index += ch.len_utf8();
    }

    normalized
}

pub(crate) fn html_break_end(raw: &str, start: usize) -> Option<usize> {
    let bytes = raw.as_bytes();
    if bytes.get(start).copied()? != b'<' {
        return None;
    }
    if !byte_eq_ignore_ascii_case(bytes.get(start + 1).copied()?, b'b')
        || !byte_eq_ignore_ascii_case(bytes.get(start + 2).copied()?, b'r')
    {
        return None;
    }

    let mut index = start + 3;
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_whitespace())
    {
        index += 1;
    }
    if bytes.get(index).copied() == Some(b'/') {
        index += 1;
    }
    if bytes.get(index).copied() != Some(b'>') {
        return None;
    }
    Some(index + 1)
}

fn byte_eq_ignore_ascii_case(left: u8, right: u8) -> bool {
    left.eq_ignore_ascii_case(&right)
}

fn wrap_display_paragraph(
    text: &str,
    max_width: usize,
    width_profile: TerminalWidthProfile,
    lines: &mut Vec<String>,
) {
    let mut current = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_width = display_width_with_profile(word, width_profile);
        if word_width > max_width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            push_wrapped_word(word, max_width, width_profile, lines);
            continue;
        }

        let separator_width = usize::from(!current.is_empty());
        if current_width + separator_width + word_width > max_width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }

        if !current.is_empty() {
            current.push(' ');
            current_width += 1;
        }
        current.push_str(word);
        current_width += word_width;
    }

    if !current.is_empty() {
        lines.push(current);
    }
}

fn push_wrapped_word(
    word: &str,
    max_width: usize,
    width_profile: TerminalWidthProfile,
    lines: &mut Vec<String>,
) {
    let mut current = String::new();
    let mut current_width = 0;

    let word = SafeLine::new(word);
    for grapheme in word.graphemes(width_profile) {
        if current_width + grapheme.width() > max_width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push_str(grapheme.text());
        current_width += grapheme.width();
    }

    if !current.is_empty() {
        lines.push(current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitExceeded, AsciiResourceLimitId};
    use crate::{AsciiColorMode, AsciiColorTheme, AsciiRenderOptions, AsciiRgb};

    #[test]
    fn styled_line_writes_role_runs_to_canvas() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut line = StyledLine::with_width_profile(TerminalWidthProfile::Unicode);
        line.push_role_text("AB", AsciiColorRole::Text);
        line.push_plain_char('!');
        let mut canvas = Canvas::new(3, 1);

        line.write_to(&mut canvas, 0);

        let output = canvas
            .finish_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::TrueColor)
                    .with_color_theme(theme),
            )
            .expect("test output should fit the unbounded canvas policy");
        assert_eq!(output, "\u{1b}[38;2;1;2;3mAB\u{1b}[0m!\n");
    }

    #[test]
    fn styled_line_counts_wide_chars_by_display_width() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut line = StyledLine::with_width_profile(TerminalWidthProfile::Unicode);
        line.push_role_text("中A", AsciiColorRole::Text);
        let mut canvas = Canvas::new(3, 1);

        assert_eq!(line.len(), 3);
        assert_eq!(line.get(0), Some('中'));
        assert_eq!(line.get(1), None);
        assert_eq!(line.get(2), Some('A'));

        line.write_to(&mut canvas, 0);

        let output = canvas
            .finish_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::TrueColor)
                    .with_color_theme(theme),
            )
            .expect("test output should fit the unbounded canvas policy");
        assert_eq!(output, "\u{1b}[38;2;1;2;3m中A\u{1b}[0m\n");
    }

    #[test]
    fn styled_line_trim_and_pad_use_unstyled_spaces() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut line = StyledLine::role_text_with_profile(
            "A ",
            AsciiColorRole::Text,
            TerminalWidthProfile::Unicode,
        )
        .trim_right();
        line.pad_to(3);
        let mut canvas = Canvas::new(3, 1);

        line.write_to(&mut canvas, 0);

        let output = canvas
            .finish_trimmed_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::TrueColor)
                    .with_color_theme(theme),
            )
            .expect("test output should fit the unbounded canvas policy");
        assert_eq!(output, "\u{1b}[38;2;1;2;3mA\u{1b}[0m\n");
    }

    #[test]
    fn styled_line_write_line_preserves_wide_cell_invariants() {
        let mut target = StyledLine::plain_text_with_profile("abcd", TerminalWidthProfile::Unicode);
        let source = StyledLine::plain_text_with_profile("中", TerminalWidthProfile::Unicode);

        target.write_line(1, &source);

        assert_eq!(target.len(), 4);
        assert_eq!(target.text(), "a中d");
        assert_eq!(target.get(1), Some('中'));
        assert_eq!(target.get(2), None);
        assert_eq!(target.get(3), Some('d'));
    }

    #[test]
    fn styled_line_write_line_rejects_wide_cell_at_final_column() {
        let mut target = StyledLine::plain_text_with_profile("ab", TerminalWidthProfile::Unicode);
        let source = StyledLine::plain_text_with_profile("中", TerminalWidthProfile::Unicode);

        let error = target
            .try_write_line(1, &source)
            .expect_err("the complete source line must fit before writing begins");

        assert_eq!(error, terminal_surface_does_not_fit());
        assert_eq!(target.get(0), Some('a'));
        assert_eq!(target.get(1), Some('b'));
    }

    #[test]
    fn styled_line_write_text_role_rejects_wide_cell_at_final_column() {
        let mut target = StyledLine::plain_text_with_profile("ab", TerminalWidthProfile::Unicode);

        let error = target
            .try_write_text_role(1, "🚀", AsciiColorRole::Text)
            .expect_err("the complete text span must fit before writing begins");

        assert_eq!(error, terminal_surface_does_not_fit());
        assert_eq!(target.get(0), Some('a'));
        assert_eq!(target.get(1), Some('b'));
    }

    #[test]
    fn styled_line_write_line_preflight_prevents_partial_prefix_writes() {
        let mut target = StyledLine::plain_text_with_profile("abc", TerminalWidthProfile::Unicode);
        let source = StyledLine::plain_text_with_profile("XY", TerminalWidthProfile::Unicode);

        let error = target
            .try_write_line(2, &source)
            .expect_err("a source line wider than the remaining target must be rejected");

        assert_eq!(error, terminal_surface_does_not_fit());
        assert_eq!(target.get(0), Some('a'));
        assert_eq!(target.get(1), Some('b'));
        assert_eq!(target.get(2), Some('c'));
    }

    #[test]
    fn styled_line_write_line_ignores_source_continuation_cells() {
        let mut target = StyledLine::plain_text_with_profile("abcd", TerminalWidthProfile::Unicode);
        let source = StyledLine::plain_text_with_profile("中Z", TerminalWidthProfile::Unicode);

        target.write_line(1, &source);

        assert_eq!(target.text(), "a中Z");
        assert_eq!(target.get(1), Some('中'));
        assert_eq!(target.get(2), None);
        assert_eq!(target.get(3), Some('Z'));
    }

    #[test]
    fn styled_line_write_line_preserves_wide_cell_style() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut target = StyledLine::plain_text_with_profile("abcd", TerminalWidthProfile::Unicode);
        let mut source = StyledLine::with_width_profile(TerminalWidthProfile::Unicode);
        source.push_role_text("中", AsciiColorRole::Text);
        source.set_background_color(0, AsciiRgb::new(4, 5, 6));

        target.write_line(1, &source);
        let mut canvas = Canvas::new(4, 1);
        target.write_to(&mut canvas, 0);

        let output = canvas
            .finish_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::TrueColor)
                    .with_color_theme(theme),
            )
            .expect("test output should fit the unbounded canvas policy");
        assert_eq!(
            output,
            "a\u{1b}[38;2;1;2;3m\u{1b}[48;2;4;5;6m中\u{1b}[0md\n"
        );
    }

    #[test]
    fn styled_line_composition_remaps_complex_grapheme_ownership() {
        let mut target = StyledLine::blank_with_profile(5, TerminalWidthProfile::Unicode);
        let source = StyledLine::role_text_with_profile(
            "👩‍💻",
            AsciiColorRole::Text,
            TerminalWidthProfile::Unicode,
        );

        target.write_line(1, &source);

        assert_eq!(target.text(), " 👩‍💻  ");
        let mut canvas = Canvas::new(5, 1);
        target.write_to(&mut canvas, 0);
        let output = canvas
            .finish_with_options(&AsciiRenderOptions::unicode())
            .expect("test output should fit the unbounded canvas policy");
        assert_eq!(output, " 👩‍💻  \n");
    }

    #[test]
    fn styled_line_mirror_compacts_complex_graphemes_without_a_third_cell_surface() {
        let line = StyledLine::try_plain_text_with_policy(
            "👩‍💻A",
            TerminalWidthProfile::Unicode,
            compatibility_policy(),
        )
        .expect("test source line should fit");

        let mirrored = line
            .try_mirrored()
            .expect("mirroring should compact the owned cells in place");

        assert_eq!(
            mirrored.try_text().expect("mirrored text should encode"),
            "A👩‍💻"
        );
        assert_eq!(mirrored.arena.entry_count(), 1);
    }

    #[test]
    fn cjk_profile_controls_wrap_and_truncation_at_ambiguous_graphemes() {
        assert_eq!(
            display_width_with_profile("A·B", TerminalWidthProfile::Unicode),
            3
        );
        assert_eq!(
            display_width_with_profile("A·B", TerminalWidthProfile::Cjk),
            4
        );
        assert_eq!(
            truncate_display_width_with_profile("A·B", 2, TerminalWidthProfile::Unicode),
            "A·"
        );
        assert_eq!(
            truncate_display_width_with_profile("A·B", 2, TerminalWidthProfile::Cjk),
            "A"
        );
        assert_eq!(
            wrap_display_lines_with_profile("A·B", 2, TerminalWidthProfile::Cjk),
            ["A", "·", "B"]
        );
    }

    #[test]
    fn truncate_display_width_preserves_terminal_cell_boundaries() {
        assert_eq!(
            truncate_display_width_with_profile("中国A", 1, TerminalWidthProfile::Unicode),
            ""
        );
        assert_eq!(
            truncate_display_width_with_profile("中国A", 2, TerminalWidthProfile::Unicode),
            "中"
        );
        assert_eq!(
            truncate_display_width_with_profile("中国A", 4, TerminalWidthProfile::Unicode),
            "中国"
        );
        assert_eq!(
            truncate_display_width_with_profile("中国A", 5, TerminalWidthProfile::Unicode),
            "中国A"
        );
    }

    #[test]
    fn styled_line_constructor_checks_document_cells_before_allocating() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, 2)
            .expect("valid test override");

        let error = StyledLine::try_blank_with_policy(3, TerminalWidthProfile::Unicode, policy)
            .expect_err("three cells must exceed a two-cell document policy");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxDocumentCells,
                actual: 3,
                max: 2,
                ..
            })
        ));
    }

    #[test]
    fn styled_line_constructor_checks_primary_grid_cells_before_allocating() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 2)
            .expect("valid test override");

        let error = StyledLine::try_blank_with_policy(3, TerminalWidthProfile::Unicode, policy)
            .expect_err("three primary cells must exceed a two-cell grid policy");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxGridCells,
                actual: 3,
                max: 2,
                ..
            })
        ));
    }

    #[test]
    fn styled_line_reports_grapheme_resource_errors_without_poisoning_future_writes() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 2)
            .expect("valid test override");
        let mut line = StyledLine::with_resource_policy(TerminalWidthProfile::Unicode, policy);

        let error = line
            .try_push_role_text("e\u{301}", AsciiColorRole::Text)
            .expect_err("three-byte grapheme must exceed a two-byte policy");

        assert_eq!(line.len(), 0);
        assert!(matches!(error, AsciiError::ResourceLimitExceeded(_)));
        line.try_push_role_repeat('x', 1, AsciiColorRole::Text)
            .expect("a failed write must not poison later independent writes");
        assert_eq!(line.try_text().expect("valid retained text"), "x");
        assert_eq!(line.try_into_text().expect("valid retained text"), "x");
    }

    #[test]
    fn styled_line_checks_raw_control_grapheme_before_visible_escape() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 1)
            .expect("valid test override");
        let mut line = StyledLine::with_resource_policy(TerminalWidthProfile::Unicode, policy);

        let error = line
            .try_push_role_text("\u{85}", AsciiColorRole::Text)
            .expect_err("a two-byte control grapheme must fail before escaping");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxGraphemeBytes,
                actual: 2,
                max: 1,
                ..
            })
        ));
        assert_eq!(line.len(), 0);
    }

    #[test]
    fn styled_line_budgets_control_escape_before_mutation() {
        let exact_policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 7)
            .expect("valid test override");
        let mut exact =
            StyledLine::with_resource_policy(TerminalWidthProfile::Unicode, exact_policy);
        exact
            .try_push_role_text("\u{85}", AsciiColorRole::Text)
            .expect("one scan plus a six-byte visible escape should fit exactly");
        assert_eq!(
            exact.try_text().expect("escaped text should encode"),
            "\\u{85}"
        );

        let below_policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 6)
            .expect("valid test override");
        let mut below =
            StyledLine::with_resource_policy(TerminalWidthProfile::Unicode, below_policy);
        let error = below
            .try_push_role_text("\u{85}", AsciiColorRole::Text)
            .expect_err("visible escape expansion must be charged before mutation");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxLayoutWorkUnits,
                actual: 7,
                max: 6,
                ..
            })
        ));
        assert_eq!(below.len(), 0);
    }

    #[test]
    fn styled_line_composition_imports_only_referenced_source_glyphs() {
        let mut source =
            StyledLine::with_resource_policy(TerminalWidthProfile::Unicode, compatibility_policy());
        source
            .try_push_role_text("e\u{301}a\u{308}", AsciiColorRole::Text)
            .expect("source test glyphs should fit");
        assert_eq!(source.arena.entry_count(), 2);
        source.cells.truncate(1);

        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 3)
            .expect("valid test override")
            .with_limit(AsciiResourceLimitId::MaxGridCells, 2)
            .expect("valid test override");
        let mut overwritten =
            StyledLine::try_blank_with_policy(1, TerminalWidthProfile::Unicode, policy)
                .expect("target test line should fit");
        overwritten
            .try_write_line(0, &source)
            .expect("write composition must not import the unused source glyph");
        assert_eq!(
            overwritten.try_text().expect("target text should encode"),
            "e\u{301}"
        );
        assert_eq!(overwritten.arena.entry_count(), 1);
        assert_eq!(overwritten.arena.retained_bytes(), 3);

        let mut appended = StyledLine::with_resource_policy(TerminalWidthProfile::Unicode, policy);
        appended
            .try_push_line(&source)
            .expect("append composition must not import the unused source glyph");
        assert_eq!(
            appended.try_text().expect("target text should encode"),
            "e\u{301}"
        );
        assert_eq!(appended.arena.entry_count(), 1);
        assert_eq!(appended.arena.retained_bytes(), 3);
    }
}
