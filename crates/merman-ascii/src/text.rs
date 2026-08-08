use crate::canvas::Canvas;
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::options::TerminalWidthProfile;
use crate::safe_text::{
    SafeLine, SafeText, terminal_char_display_width, terminal_line_display_width,
};
use crate::terminal::{
    CanvasColor, CanvasStyle, GlyphArena, TerminalCell, mirror_cells, owner_index, primary_width,
    push_primary_grapheme_style, style_at, write_primary_cell_from_cell,
    write_primary_grapheme_style,
};

pub(crate) type StyledCell = TerminalCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyledLine {
    cells: Vec<StyledCell>,
    arena: GlyphArena,
    width_profile: TerminalWidthProfile,
}

impl StyledLine {
    pub(crate) fn with_width_profile(width_profile: TerminalWidthProfile) -> Self {
        Self {
            cells: Vec::new(),
            arena: GlyphArena::default(),
            width_profile,
        }
    }

    pub(crate) fn from_surface_cells(
        cells: Vec<StyledCell>,
        arena: GlyphArena,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        Self {
            cells,
            arena,
            width_profile,
        }
    }

    pub(crate) fn blank_with_profile(width: usize, width_profile: TerminalWidthProfile) -> Self {
        Self {
            cells: vec![StyledCell::blank(); width],
            arena: GlyphArena::default(),
            width_profile,
        }
    }

    pub(crate) fn role_text_with_profile(
        text: &str,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        let mut line = Self::with_width_profile(width_profile);
        line.push_role_text(text, role);
        line
    }

    pub(crate) fn plain_text_with_profile(text: &str, width_profile: TerminalWidthProfile) -> Self {
        let mut line = Self::with_width_profile(width_profile);
        line.push_plain_text(text);
        line
    }

    pub(crate) fn len(&self) -> usize {
        self.cells.len()
    }

    pub(crate) fn width_profile(&self) -> TerminalWidthProfile {
        self.width_profile
    }

    pub(crate) fn get(&self, index: usize) -> Option<char> {
        self.cells.get(index).and_then(|cell| cell.output_char())
    }

    pub(crate) fn text(&self) -> String {
        let mut output = String::new();
        for cell in &self.cells {
            if let Some(text) = cell.output_text(&self.arena) {
                text.push_to(&mut output);
            }
        }
        output
    }

    pub(crate) fn into_text(self) -> String {
        self.text()
    }

    pub(crate) fn pad_to(&mut self, width: usize) {
        if self.cells.len() < width {
            self.cells.resize(width, StyledCell::blank());
        }
    }

    pub(crate) fn push_plain_char(&mut self, ch: char) {
        self.push_char_style(ch, CanvasStyle::default());
    }

    fn push_plain_text(&mut self, text: &str) {
        let text = SafeLine::new(text);
        for grapheme in text.graphemes(self.width_profile) {
            self.push_measured_grapheme(grapheme.text(), grapheme.width(), CanvasStyle::default());
        }
    }

    pub(crate) fn push_spaces(&mut self, count: usize) {
        self.cells
            .extend(std::iter::repeat_n(StyledCell::blank(), count));
    }

    pub(crate) fn push_line(&mut self, line: &StyledLine) {
        assert_eq!(
            self.width_profile, line.width_profile,
            "cannot compose terminal surfaces with different width profiles"
        );
        let remap = self.arena.import_all(&line.arena);
        self.cells.extend(
            line.cells
                .iter()
                .copied()
                .map(|cell| cell.remap_arena(remap)),
        );
    }

    pub(crate) fn push_role_char(&mut self, ch: char, role: AsciiColorRole) {
        self.push_char_style(ch, CanvasStyle::foreground(CanvasColor::Role(role)));
    }

    pub(crate) fn push_role_text(&mut self, text: &str, role: AsciiColorRole) {
        let text = SafeLine::new(text);
        let style = CanvasStyle::foreground(CanvasColor::Role(role));
        for grapheme in text.graphemes(self.width_profile) {
            self.push_measured_grapheme(grapheme.text(), grapheme.width(), style);
        }
    }

    pub(crate) fn push_role_text_with_unstyled_trailing_spaces(
        &mut self,
        text: &str,
        role: AsciiColorRole,
    ) {
        let normalized = SafeLine::new(text);
        let text = normalized.as_str();
        let trimmed = text.trim_end_matches(' ');
        self.push_role_text(trimmed, role);
        self.push_spaces(text.len() - trimmed.len());
    }

    pub(crate) fn push_role_repeat(&mut self, ch: char, count: usize, role: AsciiColorRole) {
        for _ in 0..count {
            self.push_role_char(ch, role);
        }
    }

    pub(crate) fn push_right_aligned_role_text(
        &mut self,
        text: &str,
        width: usize,
        role: AsciiColorRole,
    ) {
        let len = display_width_with_profile(text, self.width_profile);
        self.push_spaces(width.saturating_sub(len));
        self.push_role_text(text, role);
    }

    pub(crate) fn set_role(&mut self, index: usize, ch: char, role: AsciiColorRole) {
        let width = terminal_char_display_width(ch, self.width_profile);
        debug_assert_eq!(
            width, 1,
            "renderer-owned structural glyphs must occupy one terminal cell"
        );
        if width != 1 {
            return;
        }
        let background = style_at(&self.cells, index).background;
        let mut buffer = [0; 4];
        let grapheme = ch.encode_utf8(&mut buffer);
        write_primary_grapheme_style(
            &mut self.cells,
            &mut self.arena,
            index,
            grapheme,
            1,
            CanvasStyle {
                foreground: Some(CanvasColor::Role(role)),
                background,
            },
        );
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

    pub(crate) fn write_text_role(&mut self, start: usize, text: &str, role: AsciiColorRole) {
        let mut offset = 0;
        let text = SafeLine::new(text);
        for grapheme in text.graphemes(self.width_profile) {
            let index = start.saturating_add(offset);
            let background = style_at(&self.cells, index).background;
            write_primary_grapheme_style(
                &mut self.cells,
                &mut self.arena,
                index,
                grapheme.text(),
                grapheme.width(),
                CanvasStyle {
                    foreground: Some(CanvasColor::Role(role)),
                    background,
                },
            );
            offset = offset.saturating_add(grapheme.width());
        }
    }

    pub(crate) fn write_line(&mut self, start: usize, line: &StyledLine) {
        assert_eq!(
            self.width_profile, line.width_profile,
            "cannot compose terminal surfaces with different width profiles"
        );
        let remap = self.arena.import_all(&line.arena);
        let mut offset = 0;
        while offset < line.cells.len() {
            let cell = line.cells[offset];
            if cell.is_continuation() {
                offset += 1;
                continue;
            }
            let width = primary_width(&line.cells, offset).max(1);
            write_primary_cell_from_cell(
                &mut self.cells,
                start.saturating_add(offset),
                cell,
                width,
                remap,
            );
            offset += width;
        }
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

    pub(crate) fn write_to(&self, canvas: &mut Canvas, y: usize) {
        self.write_to_at(canvas, 0, y);
    }

    pub(crate) fn write_to_at(&self, canvas: &mut Canvas, x_offset: usize, y: usize) {
        assert!(canvas.write_cells_from_surface(
            x_offset,
            y,
            &self.cells,
            &self.arena,
            self.width_profile,
        ));
    }

    pub(crate) fn mirrored(&self) -> Self {
        Self {
            cells: mirror_cells(&self.cells),
            arena: self.arena.clone(),
            width_profile: self.width_profile,
        }
    }

    fn push_char_style(&mut self, ch: char, style: CanvasStyle) {
        let width = terminal_char_display_width(ch, self.width_profile);
        debug_assert_eq!(
            width, 1,
            "renderer-owned structural glyphs must occupy one terminal cell"
        );
        if width != 1 {
            return;
        }
        let mut buffer = [0; 4];
        let grapheme = ch.encode_utf8(&mut buffer);
        self.push_measured_grapheme(grapheme, 1, style);
    }

    fn push_measured_grapheme(&mut self, grapheme: &str, width: usize, style: CanvasStyle) {
        push_primary_grapheme_style(&mut self.cells, &mut self.arena, grapheme, width, style);
    }
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

pub(crate) fn trim_trailing_blank_lines(mut lines: Vec<String>) -> Vec<String> {
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    lines
}

pub(crate) fn push_wrapped_prefixed_line_with_profile(
    lines: &mut Vec<String>,
    first_prefix: &str,
    continuation_prefix: &str,
    text: &str,
    max_width: usize,
    width_profile: TerminalWidthProfile,
) {
    let available = max_width
        .saturating_sub(display_width_with_profile(first_prefix, width_profile))
        .min(max_width.saturating_sub(display_width_with_profile(
            continuation_prefix,
            width_profile,
        )))
        .max(1);
    let wrapped = wrap_display_lines_with_profile(text, available, width_profile);
    if wrapped.is_empty() {
        lines.push(first_prefix.to_string());
        return;
    }

    for (index, line) in wrapped.iter().enumerate() {
        if index == 0 {
            lines.push(format!("{first_prefix}{line}"));
        } else {
            lines.push(format!("{continuation_prefix}{line}"));
        }
    }
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

fn html_break_end(raw: &str, start: usize) -> Option<usize> {
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

        let output = canvas.finish_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::TrueColor)
                .with_color_theme(theme),
        );
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

        let output = canvas.finish_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::TrueColor)
                .with_color_theme(theme),
        );
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

        let output = canvas.finish_trimmed_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::TrueColor)
                .with_color_theme(theme),
        );
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

        target.write_line(1, &source);

        assert_eq!(target.text(), "ab");
        assert_eq!(target.get(1), Some('b'));
    }

    #[test]
    fn styled_line_write_text_role_rejects_wide_cell_at_final_column() {
        let mut target = StyledLine::plain_text_with_profile("ab", TerminalWidthProfile::Unicode);

        target.write_text_role(1, "🚀", AsciiColorRole::Text);

        assert_eq!(target.text(), "ab");
        assert_eq!(target.get(1), Some('b'));
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

        let output = canvas.finish_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::TrueColor)
                .with_color_theme(theme),
        );
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
        let output = canvas.finish_with_options(&AsciiRenderOptions::unicode());
        assert_eq!(output, " 👩‍💻  \n");
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
}
