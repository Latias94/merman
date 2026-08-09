use crate::color::{AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRgb};
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
#[cfg(test)]
use crate::resource::AsciiResourcePolicy;
use crate::resource::{AsciiResourceLimitPhase, CheckedOutput, LogicalExtent, ResourceContext};
use crate::safe_text::{
    push_html_escaped_text as push_html_escaped_str, terminal_char_display_width,
    visit_safe_line_graphemes,
};
pub(crate) use crate::terminal::CanvasColor;
use crate::terminal::{
    CanvasStyle, GlyphArena, ResolvedCanvasStyle, TerminalCell, TerminalCellText, owner_index,
    primary_width, style_at, try_write_primary_cell_from_surface,
    try_write_primary_grapheme_style_with_policy,
};

#[derive(Debug)]
pub(crate) struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<TerminalCell>,
    arena: GlyphArena,
    width_profile: TerminalWidthProfile,
    resources: ResourceContext,
}

impl Canvas {
    #[cfg(test)]
    pub(crate) fn new(width: usize, height: usize) -> Self {
        Self::try_with_policy(
            width,
            height,
            TerminalWidthProfile::Unicode,
            AsciiResourcePolicy::for_profile(
                merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
            ),
        )
        .expect("test canvas extent must be representable")
    }

    #[cfg(test)]
    pub(crate) fn with_width_profile(
        width: usize,
        height: usize,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        Self::try_with_policy(
            width,
            height,
            width_profile,
            AsciiResourcePolicy::for_profile(
                merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
            ),
        )
        .expect("test canvas extent must be representable")
    }

    #[cfg(test)]
    pub(crate) fn try_with_options(
        width: usize,
        height: usize,
        options: &AsciiRenderOptions,
    ) -> crate::Result<Self> {
        let resources = ResourceContext::new(options.resources);
        Self::try_with_resources(width, height, options.terminal_width_profile, &resources)
    }

    #[cfg(test)]
    pub(crate) fn try_with_policy(
        width: usize,
        height: usize,
        width_profile: TerminalWidthProfile,
        resources: AsciiResourcePolicy,
    ) -> crate::Result<Self> {
        let resources = ResourceContext::new(resources);
        Self::try_with_resources(width, height, width_profile, &resources)
    }

    pub(crate) fn try_with_resources(
        width: usize,
        height: usize,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> crate::Result<Self> {
        let extent = resources.grid_extent(width, height)?;
        Self::from_extent(extent, width_profile, resources.scoped())
    }

    fn from_extent(
        extent: LogicalExtent,
        width_profile: TerminalWidthProfile,
        resources: ResourceContext,
    ) -> crate::Result<Self> {
        let mut cells = Vec::new();
        cells.try_reserve_exact(extent.cells()).map_err(|_| {
            crate::AsciiError::AllocationFailed {
                phase: AsciiResourceLimitPhase::Layout.as_str(),
            }
        })?;
        cells.resize(extent.cells(), TerminalCell::blank());
        Ok(Self {
            width: extent.width(),
            height: extent.height(),
            cells,
            arena: GlyphArena::default(),
            width_profile,
            resources,
        })
    }

    #[cfg(test)]
    pub(crate) fn set(&mut self, x: usize, y: usize, ch: char) {
        self.try_set(x, y, ch)
            .expect("test structural glyph should fit the unbounded resource policy");
    }

    pub(crate) fn try_set(&mut self, x: usize, y: usize, ch: char) -> crate::Result<()> {
        if let Some(index) = self.index_for_structural_char(x, y, ch) {
            let style = style_at(&self.cells, index).with_foreground(None);
            self.write_structural_char(index, ch, style)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn set_role(&mut self, x: usize, y: usize, ch: char, role: AsciiColorRole) {
        self.try_set_role(x, y, ch, role)
            .expect("test structural glyph should fit the unbounded resource policy");
    }

    pub(crate) fn try_set_role(
        &mut self,
        x: usize,
        y: usize,
        ch: char,
        role: AsciiColorRole,
    ) -> crate::Result<()> {
        self.try_set_canvas_color(x, y, ch, CanvasColor::Role(role))
    }

    pub(crate) fn try_set_color(
        &mut self,
        x: usize,
        y: usize,
        ch: char,
        color: AsciiRgb,
    ) -> crate::Result<()> {
        self.try_set_canvas_color(x, y, ch, CanvasColor::Direct(color))
    }

    pub(crate) fn try_set_canvas_color(
        &mut self,
        x: usize,
        y: usize,
        ch: char,
        color: CanvasColor,
    ) -> crate::Result<()> {
        if let Some(index) = self.index_for_structural_char(x, y, ch) {
            let style = style_at(&self.cells, index).with_foreground(Some(color));
            self.write_structural_char(index, ch, style)?;
        }
        Ok(())
    }

    pub(crate) fn set_background_color(&mut self, x: usize, y: usize, color: AsciiRgb) {
        self.set_background_canvas_color(x, y, CanvasColor::Direct(color));
    }

    pub(crate) fn set_background_canvas_color(&mut self, x: usize, y: usize, color: CanvasColor) {
        if let Some(index) = self.index(x, y) {
            let owner = owner_index(&self.cells, index).unwrap_or(index);
            self.cells[owner].set_background(color);
        }
    }

    pub(crate) fn get(&self, x: usize, y: usize) -> Option<char> {
        self.index(x, y)
            .and_then(|index| self.cells[index].output_char())
    }

    #[cfg(test)]
    pub(crate) fn get_text(&self, x: usize, y: usize) -> Option<TerminalCellText<'_>> {
        self.index(x, y)
            .and_then(|index| self.cells[index].output_text(&self.arena))
    }

    #[cfg(test)]
    pub(crate) fn get_color(&self, x: usize, y: usize) -> Option<CanvasColor> {
        self.index(x, y).and_then(|index| self.cells[index].color())
    }

    #[cfg(test)]
    pub(crate) fn get_style(&self, x: usize, y: usize) -> Option<CanvasStyle> {
        self.index(x, y).and_then(|index| self.cells[index].style())
    }

    #[cfg(test)]
    pub(crate) fn visit_plain_row_display_range(
        &self,
        x: usize,
        y: usize,
        width: usize,
        mut visitor: impl FnMut(TerminalCellText<'_>) -> crate::Result<bool>,
    ) -> crate::Result<Option<bool>> {
        if y >= self.height || x > self.width {
            return Ok(None);
        }

        let end_x = x.checked_add(width).ok_or_else(|| {
            self.resources
                .overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
        })?;
        if end_x > self.width {
            return Ok(None);
        }

        let row_start = y.checked_mul(self.width).ok_or_else(|| {
            self.resources
                .overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
        })?;
        let range_start = row_start.checked_add(x).ok_or_else(|| {
            self.resources
                .overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
        })?;
        let range_end = row_start.checked_add(end_x).ok_or_else(|| {
            self.resources
                .overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
        })?;

        if width > 0 && self.cells[range_start].is_continuation() {
            return Ok(None);
        }
        if end_x < self.width && self.cells[range_end].is_continuation() {
            return Ok(None);
        }

        for cell in &self.cells[range_start..range_end] {
            if let Some(text) = cell.try_output_text(&self.arena)?
                && !visitor(text)?
            {
                return Ok(Some(false));
            }
        }
        Ok(Some(true))
    }

    #[cfg(test)]
    pub(crate) fn write_text(&mut self, x: usize, y: usize, text: &str) {
        self.write_text_style(x, y, text, CanvasStyle::default())
            .expect("test terminal text should fit the unbounded resource policy");
    }

    pub(crate) fn write_text_role(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        role: AsciiColorRole,
    ) -> crate::Result<()> {
        self.write_text_style(x, y, text, CanvasStyle::foreground(CanvasColor::Role(role)))
    }

    pub(crate) fn write_text_color(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        color: AsciiRgb,
    ) -> crate::Result<()> {
        self.write_text_style(
            x,
            y,
            text,
            CanvasStyle::foreground(CanvasColor::Direct(color)),
        )
    }

    fn write_text_style(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        style: CanvasStyle,
    ) -> crate::Result<()> {
        let policy = self.resources.policy();
        let canvas_width = self.width;
        let canvas_height = self.height;
        let mut offset = 0;
        visit_safe_line_graphemes(
            &mut self.resources,
            text,
            self.width_profile,
            |grapheme, width| {
                let target_x = x.checked_add(offset).ok_or_else(|| {
                    policy.overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
                })?;
                let target_end = target_x.checked_add(width).ok_or_else(|| {
                    policy.overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
                })?;
                if target_end > canvas_width || y >= canvas_height {
                    return Ok(false);
                }
                let index = y * canvas_width + target_x;
                let wrote = try_write_primary_grapheme_style_with_policy(
                    &mut self.cells,
                    &mut self.arena,
                    index,
                    grapheme,
                    width,
                    style,
                    policy,
                )?;
                if wrote {
                    offset = offset.checked_add(width).ok_or_else(|| {
                        policy.overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
                    })?;
                }
                Ok(wrote)
            },
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn finish(self) -> String {
        self.finish_plain(false)
            .expect("test canvas encoding should fit the unbounded policy")
    }

    #[cfg(test)]
    pub(crate) fn finish_trimmed(self) -> String {
        self.finish_plain(true)
            .expect("test canvas encoding should fit the unbounded policy")
    }

    pub(crate) fn finish_with_options(self, options: &AsciiRenderOptions) -> crate::Result<String> {
        self.finish_with_options_internal(options, false)
    }

    #[cfg(test)]
    pub(crate) fn finish_trimmed_with_options(
        self,
        options: &AsciiRenderOptions,
    ) -> crate::Result<String> {
        self.finish_with_options_internal(options, true)
    }

    pub(crate) fn try_write_cells_from_surface(
        &mut self,
        x: usize,
        y: usize,
        cells: &[TerminalCell],
        arena: &GlyphArena,
        width_profile: TerminalWidthProfile,
    ) -> crate::Result<bool> {
        if width_profile != self.width_profile || x >= self.width || y >= self.height {
            return Ok(false);
        }

        let target_row_end = x.checked_add(cells.len()).ok_or_else(|| {
            self.resources
                .overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
        })?;
        if target_row_end > self.width {
            return Ok(false);
        }

        let row_start = y.checked_mul(self.width).ok_or_else(|| {
            self.resources
                .overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
        })?;
        let mut offset = 0;
        while offset < cells.len() {
            let cell = cells[offset];
            if cell.is_continuation() {
                offset += 1;
                continue;
            }

            let width = primary_width(cells, offset).max(1);
            let target_x = x.checked_add(offset).ok_or_else(|| {
                self.resources
                    .overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
            })?;
            let Some(target_index) = row_start.checked_add(target_x) else {
                return Err(self
                    .resources
                    .overflow(crate::resource::AsciiResourceLimitId::MaxGridCells));
            };
            try_write_primary_cell_from_surface(
                &mut self.cells,
                &mut self.arena,
                target_index,
                cell,
                width,
                arena,
                self.resources.policy(),
            )?;
            offset += width;
        }
        Ok(true)
    }

    #[cfg(test)]
    pub(crate) fn into_styled_lines_trimmed(self) -> crate::Result<Vec<crate::text::StyledLine>> {
        self.into_styled_lines(true)
    }

    /// Converts every canvas row without changing the admitted canvas extent.
    ///
    /// Planned renderers use this form when a parent document validates the
    /// materialized region against its geometry. The final encoder remains
    /// responsible for trimming unstyled trailing cells from visible output.
    pub(crate) fn into_styled_lines_preserving_extent(
        self,
    ) -> crate::Result<Vec<crate::text::StyledLine>> {
        self.into_styled_lines(false)
    }

    fn into_styled_lines(
        self,
        trim_trailing_cells: bool,
    ) -> crate::Result<Vec<crate::text::StyledLine>> {
        if self.width == 0 || self.height == 0 {
            return Ok(Vec::new());
        }

        let output_cells =
            (0..self.cells.len())
                .step_by(self.width)
                .try_fold(0usize, |total, row_start| {
                    let row_end = if trim_trailing_cells {
                        self.trimmed_row_end(row_start, row_start + self.width, true)
                    } else {
                        row_start + self.width
                    };
                    self.resources.checked_grid_add(total, row_end - row_start)
                })?;
        let concurrent_cells = self
            .resources
            .checked_grid_add(self.cells.len(), output_cells)?;
        self.resources.grid_extent(concurrent_cells, 1)?;
        self.resources.charge_document_cells(output_cells)?;
        self.resources.charge_layout_work(output_cells.max(1))?;

        let mut lines = Vec::new();
        lines.try_reserve_exact(self.height).map_err(|_| {
            crate::AsciiError::allocation_failed(AsciiResourceLimitPhase::Document.as_str())
        })?;
        let line_resources = self.resources.scoped();
        for row_start in (0..self.cells.len()).step_by(self.width) {
            let row_end = if trim_trailing_cells {
                self.trimmed_row_end(row_start, row_start + self.width, true)
            } else {
                row_start + self.width
            };
            lines.push(
                crate::text::StyledLine::try_from_surface_cells_with_resources(
                    &self.cells[row_start..row_end],
                    &self.arena,
                    self.width_profile,
                    &line_resources,
                )?,
            );
        }
        Ok(lines)
    }

    #[cfg(test)]
    fn finish_plain(mut self, trim: bool) -> crate::Result<String> {
        self.check_document_cells(trim)?;
        if self.width == 0 || self.height == 0 {
            return Ok(String::new());
        }

        let mut out = CheckedOutput::new(self.resources.policy());
        for row_start in (0..self.cells.len()).step_by(self.width) {
            let row_end = if trim {
                self.trimmed_row_end(row_start, row_start + self.width, false)
            } else {
                row_start + self.width
            };
            for cell in &self.cells[row_start..row_end] {
                if let Some(text) = cell.try_output_text(&self.arena)? {
                    push_terminal_text(&mut out, text)?;
                }
            }
            out.push_char('\n')?;
        }
        Ok(out.finish())
    }

    fn finish_with_options_internal(
        mut self,
        options: &AsciiRenderOptions,
        trim: bool,
    ) -> crate::Result<String> {
        self.check_document_cells(trim)?;
        match options.color_mode {
            AsciiColorMode::Plain => self.encode_plain(trim),
            AsciiColorMode::Ansi16 => {
                self.finish_ansi(options.color_theme, AsciiColorMode::Ansi16, trim)
            }
            AsciiColorMode::Ansi256 => {
                self.finish_ansi(options.color_theme, AsciiColorMode::Ansi256, trim)
            }
            AsciiColorMode::TrueColor => {
                self.finish_ansi(options.color_theme, AsciiColorMode::TrueColor, trim)
            }
            AsciiColorMode::Html => self.finish_html(options.color_theme, trim),
        }
    }

    fn check_document_cells(&mut self, trim: bool) -> crate::Result<()> {
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }
        for row_start in (0..self.cells.len()).step_by(self.width) {
            let row_end = if trim {
                self.trimmed_row_end(row_start, row_start + self.width, true)
            } else {
                row_start + self.width
            };
            let row_cells = row_end - row_start;
            self.resources.charge_document_cells(row_cells)?;
            self.resources.charge_layout_work(row_cells.max(1))?;
        }
        Ok(())
    }

    fn encode_plain(self, trim: bool) -> crate::Result<String> {
        if self.width == 0 || self.height == 0 {
            return Ok(String::new());
        }
        let mut out = CheckedOutput::new(self.resources.policy());
        for row_start in (0..self.cells.len()).step_by(self.width) {
            let row_end = if trim {
                self.trimmed_row_end(row_start, row_start + self.width, false)
            } else {
                row_start + self.width
            };
            for cell in &self.cells[row_start..row_end] {
                if let Some(text) = cell.try_output_text(&self.arena)? {
                    push_terminal_text(&mut out, text)?;
                }
            }
            out.push_char('\n')?;
        }
        Ok(out.finish())
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y * self.width + x)
    }

    fn index_for_structural_char(&self, x: usize, y: usize, ch: char) -> Option<usize> {
        let width = terminal_char_display_width(ch, self.width_profile);
        debug_assert_eq!(
            width, 1,
            "renderer-owned structural glyphs must occupy one terminal cell"
        );
        if width != 1 {
            return None;
        }
        self.index(x, y)
    }

    fn write_structural_char(
        &mut self,
        index: usize,
        ch: char,
        style: CanvasStyle,
    ) -> crate::Result<()> {
        let mut buffer = [0; 4];
        let grapheme = ch.encode_utf8(&mut buffer);
        let result = try_write_primary_grapheme_style_with_policy(
            &mut self.cells,
            &mut self.arena,
            index,
            grapheme,
            1,
            style,
            self.resources.policy(),
        );
        match result {
            Ok(wrote) => {
                debug_assert!(wrote, "validated structural cell write must fit");
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn finish_ansi(
        self,
        theme: AsciiColorTheme,
        mode: AsciiColorMode,
        trim: bool,
    ) -> crate::Result<String> {
        if self.width == 0 || self.height == 0 {
            return Ok(String::new());
        }

        let mut out = CheckedOutput::new(self.resources.policy());
        for row_start in (0..self.cells.len()).step_by(self.width) {
            let row_end = if trim {
                self.trimmed_row_end(row_start, row_start + self.width, true)
            } else {
                row_start + self.width
            };
            let mut active_style = ResolvedCanvasStyle::default();
            for cell in &self.cells[row_start..row_end] {
                let Some(text) = cell.try_output_text(&self.arena)? else {
                    continue;
                };
                let desired_style = cell.raw_style().resolve(theme);
                if desired_style != active_style {
                    if !active_style.is_plain() {
                        out.push_str("\u{1b}[0m")?;
                    }
                    if !desired_style.is_plain() {
                        push_ansi_start(&mut out, mode, desired_style)?;
                    }
                    active_style = desired_style;
                }
                push_terminal_text(&mut out, text)?;
            }
            if !active_style.is_plain() {
                out.push_str("\u{1b}[0m")?;
            }
            out.push_char('\n')?;
        }
        Ok(out.finish())
    }

    fn finish_html(self, theme: AsciiColorTheme, trim: bool) -> crate::Result<String> {
        if self.width == 0 || self.height == 0 {
            return Ok(String::new());
        }

        let mut out = CheckedOutput::new(self.resources.policy());
        for row_start in (0..self.cells.len()).step_by(self.width) {
            let row_end = if trim {
                self.trimmed_row_end(row_start, row_start + self.width, true)
            } else {
                row_start + self.width
            };
            let mut active_style = ResolvedCanvasStyle::default();
            for cell in &self.cells[row_start..row_end] {
                let Some(text) = cell.try_output_text(&self.arena)? else {
                    continue;
                };
                let desired_style = cell.raw_style().resolve(theme);
                if desired_style != active_style {
                    if !active_style.is_plain() {
                        out.push_str("</span>")?;
                    }
                    if !desired_style.is_plain() {
                        push_html_span_start(&mut out, desired_style)?;
                    }
                    active_style = desired_style;
                }
                push_html_escaped_text(&mut out, text)?;
            }
            if !active_style.is_plain() {
                out.push_str("</span>")?;
            }
            out.push_char('\n')?;
        }
        Ok(out.finish())
    }

    fn trimmed_row_end(&self, row_start: usize, mut row_end: usize, preserve_roles: bool) -> usize {
        while row_end > row_start {
            let index = row_end - 1;
            if !self.cells[index].is_trimmable_blank(preserve_roles) {
                break;
            }
            row_end -= 1;
        }
        row_end
    }
}

/// Encodes planned terminal rows directly instead of copying them into a second `Canvas`.
///
/// Row-oriented renderers such as Sequence already own complete `StyledLine` values. Building a
/// same-sized canvas would duplicate their cells and grapheme arenas. This entry point reuses the
/// final canvas encoding rules so every color mode keeps identical escaping and output budgeting,
/// while the document budget is checked before the first encoded byte is emitted.
pub(crate) fn finish_styled_lines_with_options(
    lines: &[crate::text::StyledLine],
    options: &AsciiRenderOptions,
    trim: bool,
) -> crate::Result<String> {
    let mut resources = ResourceContext::new(options.resources);
    finish_styled_lines_with_resources(lines, options, trim, &mut resources)
}

pub(crate) fn finish_styled_lines_with_resources(
    lines: &[crate::text::StyledLine],
    options: &AsciiRenderOptions,
    trim: bool,
    resources: &mut ResourceContext,
) -> crate::Result<String> {
    finish_styled_line_iter_with_resources(lines.iter(), options, trim, resources)
}

pub(crate) fn finish_styled_line_iter_with_resources<'a, I>(
    lines: I,
    options: &AsciiRenderOptions,
    trim: bool,
    resources: &mut ResourceContext,
) -> crate::Result<String>
where
    I: Clone + Iterator<Item = &'a crate::text::StyledLine>,
{
    let (line_count, width) = lines
        .clone()
        .fold((0usize, 0usize), |(count, width), line| {
            (count + 1, width.max(line.len()))
        });
    if line_count == 0 {
        return Ok(String::new());
    }
    let document_resources = resources.scoped();
    if width == 0 {
        let mut output = CheckedOutput::new(options.resources);
        for _ in lines.clone() {
            document_resources.charge_layout_work(1)?;
            output.push_char('\n')?;
        }
        return Ok(output.finish());
    }

    document_resources.grid_extent(width, line_count)?;
    for line in lines.clone() {
        let row_end = if trim {
            line.trimmed_len(true)
        } else {
            line.len()
        };
        document_resources.charge_document_cells(row_end)?;
        document_resources.charge_layout_work(row_end.max(1))?;
    }

    let mut output = CheckedOutput::new(options.resources);
    match options.color_mode {
        AsciiColorMode::Plain => {
            for line in lines.clone() {
                let row_end = if trim {
                    line.trimmed_len(false)
                } else {
                    line.len()
                };
                encode_styled_line_plain(&mut output, line, row_end)?;
                output.push_char('\n')?;
            }
        }
        AsciiColorMode::Ansi16 | AsciiColorMode::Ansi256 | AsciiColorMode::TrueColor => {
            let mode = options.color_mode;
            for line in lines.clone() {
                let row_end = if trim {
                    line.trimmed_len(true)
                } else {
                    line.len()
                };
                encode_styled_line_ansi(&mut output, line, row_end, options.color_theme, mode)?;
                output.push_char('\n')?;
            }
        }
        AsciiColorMode::Html => {
            for line in lines {
                let row_end = if trim {
                    line.trimmed_len(true)
                } else {
                    line.len()
                };
                encode_styled_line_html(&mut output, line, row_end, options.color_theme)?;
                output.push_char('\n')?;
            }
        }
    }
    Ok(output.finish())
}

fn encode_styled_line_plain(
    output: &mut CheckedOutput,
    line: &crate::text::StyledLine,
    row_end: usize,
) -> crate::Result<()> {
    for cell in &line.surface_cells()[..row_end] {
        if let Some(text) = cell.try_output_text(line.surface_arena())? {
            push_terminal_text(output, text)?;
        }
    }
    Ok(())
}

fn encode_styled_line_ansi(
    output: &mut CheckedOutput,
    line: &crate::text::StyledLine,
    row_end: usize,
    theme: AsciiColorTheme,
    mode: AsciiColorMode,
) -> crate::Result<()> {
    let mut active_style = ResolvedCanvasStyle::default();
    for cell in &line.surface_cells()[..row_end] {
        let Some(text) = cell.try_output_text(line.surface_arena())? else {
            continue;
        };
        let desired_style = cell.raw_style().resolve(theme);
        if desired_style != active_style {
            if !active_style.is_plain() {
                output.push_str("\u{1b}[0m")?;
            }
            if !desired_style.is_plain() {
                push_ansi_start(output, mode, desired_style)?;
            }
            active_style = desired_style;
        }
        push_terminal_text(output, text)?;
    }
    if !active_style.is_plain() {
        output.push_str("\u{1b}[0m")?;
    }
    Ok(())
}

fn encode_styled_line_html(
    output: &mut CheckedOutput,
    line: &crate::text::StyledLine,
    row_end: usize,
    theme: AsciiColorTheme,
) -> crate::Result<()> {
    let mut active_style = ResolvedCanvasStyle::default();
    for cell in &line.surface_cells()[..row_end] {
        let Some(text) = cell.try_output_text(line.surface_arena())? else {
            continue;
        };
        let desired_style = cell.raw_style().resolve(theme);
        if desired_style != active_style {
            if !active_style.is_plain() {
                output.push_str("</span>")?;
            }
            if !desired_style.is_plain() {
                push_html_span_start(output, desired_style)?;
            }
            active_style = desired_style;
        }
        push_html_escaped_text(output, text)?;
    }
    if !active_style.is_plain() {
        output.push_str("</span>")?;
    }
    Ok(())
}

fn push_terminal_text(out: &mut CheckedOutput, text: TerminalCellText<'_>) -> crate::Result<()> {
    match text {
        TerminalCellText::Scalar(ch) => out.push_char(ch),
        TerminalCellText::Grapheme(grapheme) => out.push_str(grapheme),
    }
}

fn push_ansi_start(
    out: &mut CheckedOutput,
    mode: AsciiColorMode,
    style: ResolvedCanvasStyle,
) -> crate::Result<()> {
    if let Some(color) = style.foreground {
        match mode {
            AsciiColorMode::Ansi16 => out.push_str(ansi16_foreground_start(color))?,
            AsciiColorMode::Ansi256 => {
                out.write_fmt(format_args!("\u{1b}[38;5;{}m", ansi256_index(color)))?;
            }
            AsciiColorMode::TrueColor => {
                out.write_fmt(format_args!(
                    "\u{1b}[38;2;{};{};{}m",
                    color.r, color.g, color.b
                ))?;
            }
            AsciiColorMode::Plain | AsciiColorMode::Html => {}
        }
    }
    if let Some(color) = style.background {
        match mode {
            AsciiColorMode::Ansi16 => out.push_str(ansi16_background_start(color))?,
            AsciiColorMode::Ansi256 => {
                out.write_fmt(format_args!("\u{1b}[48;5;{}m", ansi256_index(color)))?;
            }
            AsciiColorMode::TrueColor => {
                out.write_fmt(format_args!(
                    "\u{1b}[48;2;{};{};{}m",
                    color.r, color.g, color.b
                ))?;
            }
            AsciiColorMode::Plain | AsciiColorMode::Html => {}
        }
    }
    Ok(())
}

fn ansi256_index(color: AsciiRgb) -> u16 {
    let r = color.r as u16 * 5 / 255;
    let g = color.g as u16 * 5 / 255;
    let b = color.b as u16 * 5 / 255;
    16 + 36 * r + 6 * g + b
}

fn ansi16_foreground_start(color: AsciiRgb) -> &'static str {
    ansi16_start(color, false)
}

fn ansi16_background_start(color: AsciiRgb) -> &'static str {
    ansi16_start(color, true)
}

fn ansi16_start(color: AsciiRgb, background: bool) -> &'static str {
    const PALETTE: [(AsciiRgb, &str, &str); 16] = [
        (AsciiRgb::new(0x00, 0x00, 0x00), "\u{1b}[30m", "\u{1b}[40m"),
        (AsciiRgb::new(0x80, 0x00, 0x00), "\u{1b}[31m", "\u{1b}[41m"),
        (AsciiRgb::new(0x00, 0x80, 0x00), "\u{1b}[32m", "\u{1b}[42m"),
        (AsciiRgb::new(0x80, 0x80, 0x00), "\u{1b}[33m", "\u{1b}[43m"),
        (AsciiRgb::new(0x00, 0x00, 0x80), "\u{1b}[34m", "\u{1b}[44m"),
        (AsciiRgb::new(0x80, 0x00, 0x80), "\u{1b}[35m", "\u{1b}[45m"),
        (AsciiRgb::new(0x00, 0x80, 0x80), "\u{1b}[36m", "\u{1b}[46m"),
        (AsciiRgb::new(0xc0, 0xc0, 0xc0), "\u{1b}[37m", "\u{1b}[47m"),
        (AsciiRgb::new(0x80, 0x80, 0x80), "\u{1b}[90m", "\u{1b}[100m"),
        (AsciiRgb::new(0xff, 0x00, 0x00), "\u{1b}[91m", "\u{1b}[101m"),
        (AsciiRgb::new(0x00, 0xff, 0x00), "\u{1b}[92m", "\u{1b}[102m"),
        (AsciiRgb::new(0xff, 0xff, 0x00), "\u{1b}[93m", "\u{1b}[103m"),
        (AsciiRgb::new(0x00, 0x00, 0xff), "\u{1b}[94m", "\u{1b}[104m"),
        (AsciiRgb::new(0xff, 0x00, 0xff), "\u{1b}[95m", "\u{1b}[105m"),
        (AsciiRgb::new(0x00, 0xff, 0xff), "\u{1b}[96m", "\u{1b}[106m"),
        (AsciiRgb::new(0xff, 0xff, 0xff), "\u{1b}[97m", "\u{1b}[107m"),
    ];

    PALETTE
        .iter()
        .min_by_key(|(candidate, _, _)| color_distance(*candidate, color))
        .map(|(_, fg, bg)| if background { *bg } else { *fg })
        .unwrap_or(if background {
            "\u{1b}[47m"
        } else {
            "\u{1b}[37m"
        })
}

fn color_distance(a: AsciiRgb, b: AsciiRgb) -> u32 {
    let dr = a.r as i32 - b.r as i32;
    let dg = a.g as i32 - b.g as i32;
    let db = a.b as i32 - b.b as i32;
    (dr * dr + dg * dg + db * db) as u32
}

fn push_html_span_start(out: &mut CheckedOutput, style: ResolvedCanvasStyle) -> crate::Result<()> {
    let mut wrote_any = false;
    out.push_str("<span style=\"")?;
    if let Some(color) = style.foreground {
        out.write_fmt(format_args!(
            "color:#{:02x}{:02x}{:02x}",
            color.r, color.g, color.b
        ))?;
        wrote_any = true;
    }
    if let Some(color) = style.background {
        if wrote_any {
            out.push_char(';')?;
        }
        out.write_fmt(format_args!(
            "background-color:#{:02x}{:02x}{:02x}",
            color.r, color.g, color.b
        ))?;
        wrote_any = true;
    }
    if !wrote_any {
        out.push_str("color:inherit")?;
    }
    out.push_str("\">")?;
    Ok(())
}

fn push_html_escaped_text(
    out: &mut CheckedOutput,
    text: TerminalCellText<'_>,
) -> crate::Result<()> {
    match text {
        TerminalCellText::Scalar(ch) => {
            let mut buffer = [0u8; 4];
            push_html_escaped_str(out, ch.encode_utf8(&mut buffer))
        }
        TerminalCellText::Grapheme(grapheme) => push_html_escaped_str(out, grapheme),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitExceeded, AsciiResourceLimitId, AsciiResourcePolicy};
    use crate::{AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRenderOptions, AsciiRgb};

    fn options_with_limit(id: AsciiResourceLimitId, limit: usize) -> AsciiRenderOptions {
        AsciiRenderOptions::unicode()
            .with_resource_limit(id, limit)
            .expect("valid test resource limit")
    }

    #[test]
    fn canvas_checks_grid_extent_before_allocation() {
        let exact = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 6)
            .expect("valid test resource limit");
        Canvas::try_with_policy(3, 2, TerminalWidthProfile::Unicode, exact)
            .expect("exact grid extent should fit");

        let below = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 5)
            .expect("valid test resource limit");
        let error = Canvas::try_with_policy(3, 2, TerminalWidthProfile::Unicode, below)
            .expect_err("grid extent above the limit must fail");

        assert!(matches!(
            error,
            crate::AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxGridCells,
                actual: 6,
                max: 5,
                ..
            })
        ));
    }

    #[test]
    fn canvas_counts_complete_logical_document_cells() {
        let exact = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 2);
        let mut canvas = Canvas::try_with_options(2, 1, &exact).expect("canvas should allocate");
        canvas.set(0, 0, 'A');
        assert_eq!(
            canvas
                .finish_with_options(&exact)
                .expect("exact document extent should fit"),
            "A \n"
        );

        let below = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 1);
        let mut canvas = Canvas::try_with_options(2, 1, &below).expect("canvas should allocate");
        canvas.set(0, 0, 'A');
        let error = canvas
            .finish_with_options(&below)
            .expect_err("document extent above the limit must fail");
        assert!(matches!(
            error,
            crate::AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxDocumentCells,
                actual: 2,
                max: 1,
                ..
            })
        ));
    }

    #[test]
    fn surface_copy_rejects_an_oversized_row_without_partial_writes() {
        let mut source = Canvas::new(3, 1);
        source.write_text(0, 0, "ABC");

        let mut target = Canvas::new(2, 1);
        target.set(0, 0, 'X');
        assert!(
            !target
                .try_write_cells_from_surface(
                    0,
                    0,
                    &source.cells,
                    &source.arena,
                    source.width_profile,
                )
                .expect("surface extent should be representable")
        );
        assert_eq!(target.finish(), "X \n");
    }

    #[test]
    fn styled_line_extraction_checks_the_aggregate_document_before_copying_rows() {
        let exact = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 3);
        let mut canvas = Canvas::try_with_options(2, 2, &exact).expect("canvas should allocate");
        canvas.set(1, 0, 'A');
        canvas.set(0, 1, 'B');
        assert_eq!(
            canvas
                .into_styled_lines_trimmed()
                .expect("exact aggregate document should fit")
                .len(),
            2
        );

        let below = options_with_limit(AsciiResourceLimitId::MaxDocumentCells, 2);
        let mut canvas = Canvas::try_with_options(2, 2, &below).expect("canvas should allocate");
        canvas.set(1, 0, 'A');
        canvas.set(0, 1, 'B');
        let error = canvas
            .into_styled_lines_trimmed()
            .expect_err("aggregate document above the limit must fail");
        assert!(matches!(
            error,
            crate::AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxDocumentCells,
                actual: 3,
                max: 2,
                ..
            })
        ));
    }

    #[test]
    fn canvas_checks_raw_control_grapheme_bytes_before_visible_escape() {
        let options = options_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 1);
        let mut canvas = Canvas::try_with_options(8, 1, &options).expect("canvas should allocate");
        let error = canvas
            .write_text_role(0, 0, "\u{85}", AsciiColorRole::Text)
            .expect_err("a two-byte control grapheme must fail before escaping");
        assert!(matches!(
            error,
            crate::AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxGraphemeBytes,
                actual: 2,
                max: 1,
                ..
            })
        ));
        assert_eq!(canvas.get(0, 0), Some(' '));
    }

    #[test]
    fn canvas_checks_visible_escape_work_before_normalized_materialization() {
        let measured = ResourceContext::new(AsciiResourcePolicy::default());
        let mut canvas = Canvas::try_with_resources(8, 1, TerminalWidthProfile::Unicode, &measured)
            .expect("canvas should allocate");
        canvas
            .write_text_role(0, 0, "\u{1b}", AsciiColorRole::Text)
            .expect("unbounded escape work should fit");
        let exact_work = measured.layout_work_used();

        let exact = options_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work);
        let mut canvas = Canvas::try_with_options(8, 1, &exact).expect("canvas should allocate");
        canvas
            .write_text_role(0, 0, "\u{1b}", AsciiColorRole::Text)
            .expect("exact escape work should fit");

        let below = options_with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1);
        let mut canvas = Canvas::try_with_options(8, 1, &below).expect("canvas should allocate");
        let error = canvas
            .write_text_role(0, 0, "\u{1b}", AsciiColorRole::Text)
            .expect_err("escape expansion above the work limit must fail");
        assert!(matches!(
            error,
            crate::AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxLayoutWorkUnits,
                actual,
                max,
                ..
            }) if actual == exact_work && max == exact_work - 1
        ));
        assert_eq!(canvas.get(0, 0), Some(' '));
    }

    #[test]
    fn canvases_share_the_render_wide_layout_work_ledger() {
        fn paint_one(resources: &ResourceContext) -> crate::Result<()> {
            let mut canvas =
                Canvas::try_with_resources(1, 1, TerminalWidthProfile::Unicode, resources)?;
            canvas.write_text_role(0, 0, "A", AsciiColorRole::Text)?;
            canvas.into_styled_lines_trimmed()?;
            Ok(())
        }

        let measured = ResourceContext::new(AsciiResourcePolicy::default());
        paint_one(&measured).expect("the first measured canvas should fit");
        paint_one(&measured).expect("the second measured canvas should fit");
        let exact_work = measured.layout_work_used();

        let exact_policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("valid exact work limit");
        let exact = ResourceContext::new(exact_policy);
        paint_one(&exact).expect("the first exact canvas should fit");
        paint_one(&exact).expect("the cumulative exact work should fit");
        assert_eq!(exact.layout_work_used(), exact_work);

        let below_policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("valid below-exact work limit");
        let below = ResourceContext::new(below_policy);
        paint_one(&below).expect("the first bounded canvas should fit independently");
        let error = paint_one(&below)
            .expect_err("the second canvas must observe work charged by the first canvas");
        assert!(matches!(
            error,
            crate::AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                limit: AsciiResourceLimitId::MaxLayoutWorkUnits,
                actual,
                max,
                ..
            }) if actual == exact_work && max == exact_work - 1
        ));
    }

    #[test]
    fn every_encoder_counts_actual_output_bytes() {
        for mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let base = AsciiRenderOptions::unicode().with_color_mode(mode);
            let mut canvas = Canvas::try_with_options(1, 1, &base).expect("canvas should allocate");
            canvas.set_role(0, 0, 'A', AsciiColorRole::Text);
            let expected = canvas
                .finish_with_options(&base)
                .expect("unmodified profile should encode");

            let exact = base
                .with_resource_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len())
                .expect("valid exact output limit");
            let mut canvas =
                Canvas::try_with_options(1, 1, &exact).expect("exact canvas should allocate");
            canvas.set_role(0, 0, 'A', AsciiColorRole::Text);
            assert_eq!(
                canvas
                    .finish_with_options(&exact)
                    .expect("exact output byte limit should fit"),
                expected,
                "mode={mode:?}"
            );

            let below = base
                .with_resource_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len() - 1)
                .expect("valid below-exact output limit");
            let mut canvas =
                Canvas::try_with_options(1, 1, &below).expect("bounded canvas should allocate");
            canvas.set_role(0, 0, 'A', AsciiColorRole::Text);
            let error = canvas
                .finish_with_options(&below)
                .expect_err("output byte limit below the encoded size must fail");
            assert!(matches!(
                error,
                crate::AsciiError::ResourceLimitExceeded(AsciiResourceLimitExceeded {
                    limit: AsciiResourceLimitId::MaxOutputBytes,
                    actual,
                    max,
                    ..
                }) if actual == expected.len() && max == expected.len() - 1
            ));
        }
    }

    #[test]
    fn finish_plain_ignores_color_roles() {
        let build_canvas = || {
            let mut canvas = Canvas::new(3, 1);
            canvas
                .write_text_role(0, 0, "AB", AsciiColorRole::Text)
                .expect("test text should fit");
            canvas.set(2, 0, '!');
            canvas
        };

        assert_eq!(build_canvas().finish(), "AB!\n");
        assert_eq!(
            build_canvas()
                .finish_with_options(
                    &AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::Plain)
                )
                .expect("plain canvas should encode"),
            "AB!\n"
        );
    }

    #[test]
    fn finish_trimmed_plain_trims_trailing_spaces() {
        let mut canvas = Canvas::new(4, 2);
        canvas.write_text(0, 0, "AB");

        assert_eq!(canvas.finish_trimmed(), "AB\n\n");
    }

    #[test]
    fn wide_text_reserves_continuation_cells() {
        let mut canvas = Canvas::new(4, 1);
        canvas
            .write_text_role(0, 0, "中A", AsciiColorRole::Text)
            .expect("test text should fit");

        assert_eq!(canvas.get(0, 0), Some('中'));
        assert_eq!(canvas.get(1, 0), None);
        assert_eq!(canvas.get(2, 0), Some('A'));
        assert_eq!(canvas.finish(), "中A \n");
    }

    #[test]
    fn cjk_profile_reserves_a_continuation_for_ambiguous_width_text() {
        let mut unicode = Canvas::with_width_profile(2, 1, TerminalWidthProfile::Unicode);
        unicode.write_text(0, 0, "·A");
        assert_eq!(unicode.get(0, 0), Some('·'));
        assert_eq!(unicode.get(1, 0), Some('A'));

        let mut cjk = Canvas::with_width_profile(3, 1, TerminalWidthProfile::Cjk);
        cjk.write_text(0, 0, "·A");
        assert_eq!(cjk.get(0, 0), Some('·'));
        assert_eq!(cjk.get(1, 0), None);
        assert_eq!(cjk.get(2, 0), Some('A'));

        assert_eq!(unicode.finish(), "·A\n");
        assert_eq!(cjk.finish(), "·A\n");
    }

    #[test]
    fn complex_grapheme_survives_plain_ansi_and_html_encoding() {
        let text = "Cafe\u{301} 👩‍💻 🇺🇸";

        for mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let mut canvas = Canvas::new(13, 1);
            canvas
                .write_text_role(0, 0, text, AsciiColorRole::Text)
                .expect("test grapheme should fit");
            let output = canvas
                .finish_trimmed_with_options(&AsciiRenderOptions::unicode().with_color_mode(mode))
                .expect("complex grapheme should encode in every mode");
            assert!(output.contains(text), "mode={mode:?}: {output:?}");
        }
    }

    #[test]
    fn borrowed_plain_display_range_preserves_complete_graphemes() {
        let text = "Cafe\u{301} 👩‍💻";
        let mut canvas = Canvas::new(7, 1);
        canvas.write_text(0, 0, text);

        let mut observed = String::new();
        let visited = canvas
            .visit_plain_row_display_range(0, 0, 7, |text| {
                match text {
                    TerminalCellText::Scalar(ch) => observed.push(ch),
                    TerminalCellText::Grapheme(grapheme) => observed.push_str(grapheme),
                }
                Ok(true)
            })
            .expect("borrowed row visit should succeed");

        assert_eq!(visited, Some(true));
        assert_eq!(observed, text);
        assert_eq!(
            canvas
                .visit_plain_row_display_range(6, 0, 1, |_| Ok(true))
                .expect("continuation-boundary validation should succeed"),
            None
        );
    }

    #[test]
    fn overwriting_a_complex_grapheme_continuation_clears_its_owner() {
        let mut canvas = Canvas::new(4, 1);
        canvas.write_text(0, 0, "🇺🇸");

        canvas.set(1, 0, 'X');

        assert_eq!(canvas.finish(), " X  \n");
    }

    #[test]
    fn wide_text_does_not_cross_canvas_row_boundary() {
        let mut canvas = Canvas::new(2, 2);
        canvas.write_text(1, 0, "中");
        canvas.set(0, 1, 'B');

        assert_eq!(canvas.get(1, 0), Some(' '));
        assert_eq!(canvas.get(0, 1), Some('B'));
        assert_eq!(canvas.finish(), "  \nB \n");
    }

    #[test]
    fn emoji_text_does_not_cross_canvas_row_boundary() {
        let mut canvas = Canvas::new(2, 2);
        canvas.write_text(1, 0, "🚀");
        canvas.set(0, 1, 'B');

        assert_eq!(canvas.get(1, 0), Some(' '));
        assert_eq!(canvas.get(0, 1), Some('B'));
        assert_eq!(canvas.finish(), "  \nB \n");
    }

    #[test]
    fn styled_wide_text_does_not_cross_canvas_row_boundary() {
        let mut canvas = Canvas::new(2, 2);
        canvas
            .write_text_style(
                1,
                0,
                "中",
                CanvasStyle::foreground(CanvasColor::Role(AsciiColorRole::Text)),
            )
            .expect("test text should fit");
        canvas.set_role(0, 1, 'B', AsciiColorRole::EdgeLine);

        assert_eq!(canvas.get(1, 0), Some(' '));
        assert_eq!(canvas.get(0, 1), Some('B'));
        assert_eq!(
            canvas.get_color(0, 1),
            Some(CanvasColor::Role(AsciiColorRole::EdgeLine))
        );
        assert_eq!(canvas.finish(), "  \nB \n");
    }

    #[test]
    fn overwriting_wide_text_clears_old_continuation_cell() {
        let mut canvas = Canvas::new(3, 1);
        canvas.write_text(0, 0, "中");
        canvas.set(0, 0, 'A');
        canvas.set(1, 0, 'B');

        assert_eq!(canvas.finish(), "AB \n");
    }

    #[test]
    fn finish_trimmed_truecolor_trims_unstyled_trailing_spaces() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut canvas = Canvas::new(4, 1);
        canvas
            .write_text_role(0, 0, "AB", AsciiColorRole::Text)
            .expect("test text should fit");

        let output = canvas
            .finish_trimmed_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::TrueColor)
                    .with_color_theme(theme),
            )
            .expect("TrueColor canvas should encode");

        assert_eq!(output, "\u{1b}[38;2;1;2;3mAB\u{1b}[0m\n");
    }

    #[test]
    fn finish_truecolor_keeps_role_run_across_wide_text() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut canvas = Canvas::new(3, 1);
        canvas
            .write_text_role(0, 0, "中A", AsciiColorRole::Text)
            .expect("test text should fit");

        let output = canvas
            .finish_trimmed_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::TrueColor)
                    .with_color_theme(theme),
            )
            .expect("TrueColor canvas should encode");

        assert_eq!(output, "\u{1b}[38;2;1;2;3m中A\u{1b}[0m\n");
    }

    #[test]
    fn finish_trimmed_html_trims_unstyled_trailing_spaces_and_escapes_text() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0xff0000));
        let mut canvas = Canvas::new(4, 1);
        canvas
            .write_text_role(0, 0, "<&", AsciiColorRole::Text)
            .expect("test text should fit");

        let output = canvas
            .finish_trimmed_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::Html)
                    .with_color_theme(theme),
            )
            .expect("HTML canvas should encode");

        assert_eq!(output, "<span style=\"color:#ff0000\">&lt;&amp;</span>\n");
    }

    #[test]
    fn finish_truecolor_groups_same_role_runs() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut canvas = Canvas::new(3, 1);
        canvas
            .write_text_role(0, 0, "AB", AsciiColorRole::Text)
            .expect("test text should fit");
        canvas.set(2, 0, '!');

        let output = canvas
            .finish_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::TrueColor)
                    .with_color_theme(theme),
            )
            .expect("TrueColor canvas should encode");

        assert_eq!(output, "\u{1b}[38;2;1;2;3mAB\u{1b}[0m!\n");
    }

    #[test]
    fn finish_truecolor_encodes_foreground_and_background() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut canvas = Canvas::new(1, 1);
        canvas.set_background_color(0, 0, AsciiRgb::new(4, 5, 6));
        canvas.set_role(0, 0, 'A', AsciiColorRole::Text);

        let output = canvas
            .finish_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::TrueColor)
                    .with_color_theme(theme),
            )
            .expect("TrueColor canvas should encode");

        assert_eq!(output, "\u{1b}[38;2;1;2;3m\u{1b}[48;2;4;5;6mA\u{1b}[0m\n");
    }

    #[test]
    fn finish_html_wraps_foreground_and_background() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut canvas = Canvas::new(1, 1);
        canvas.set_background_color(0, 0, AsciiRgb::new(4, 5, 6));
        canvas.set_role(0, 0, 'A', AsciiColorRole::Text);

        let output = canvas
            .finish_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::Html)
                    .with_color_theme(theme),
            )
            .expect("HTML canvas should encode");

        assert_eq!(
            output,
            "<span style=\"color:#010203;background-color:#040506\">A</span>\n"
        );
    }

    #[test]
    fn finish_ansi256_encodes_role_foreground() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::EdgeLine, AsciiRgb::from_hex24(0xff0000));
        let mut canvas = Canvas::new(1, 1);
        canvas.set_role(0, 0, 'R', AsciiColorRole::EdgeLine);

        let output = canvas
            .finish_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::Ansi256)
                    .with_color_theme(theme),
            )
            .expect("ANSI256 canvas should encode");

        assert_eq!(output, "\u{1b}[38;5;196mR\u{1b}[0m\n");
    }

    #[test]
    fn finish_ansi16_encodes_nearest_role_foreground() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::EdgeLine, AsciiRgb::from_hex24(0xff0000));
        let mut canvas = Canvas::new(1, 1);
        canvas.set_role(0, 0, 'R', AsciiColorRole::EdgeLine);

        let output = canvas
            .finish_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::Ansi16)
                    .with_color_theme(theme),
            )
            .expect("ANSI16 canvas should encode");

        assert_eq!(output, "\u{1b}[91mR\u{1b}[0m\n");
    }

    #[test]
    fn finish_html_wraps_role_runs_and_escapes_text() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0xff0000));
        let mut canvas = Canvas::new(3, 1);
        canvas
            .write_text_role(0, 0, "<&>", AsciiColorRole::Text)
            .expect("test text should fit");

        let output = canvas
            .finish_with_options(
                &AsciiRenderOptions::ascii()
                    .with_color_mode(AsciiColorMode::Html)
                    .with_color_theme(theme),
            )
            .expect("HTML canvas should encode");

        assert_eq!(
            output,
            "<span style=\"color:#ff0000\">&lt;&amp;&gt;</span>\n"
        );
    }
}
