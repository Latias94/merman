use crate::canvas::Canvas;
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::terminal::{
    CanvasColor, CanvasStyle, TerminalCell, char_display_width,
    display_width as terminal_display_width, push_primary_cell, write_primary_cell_from_cell,
    write_primary_cell_style,
};

pub(crate) type StyledCell = TerminalCell;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyledLine {
    cells: Vec<StyledCell>,
}

impl StyledLine {
    pub(crate) fn new() -> Self {
        Self { cells: Vec::new() }
    }

    pub(crate) fn blank(width: usize) -> Self {
        Self {
            cells: vec![StyledCell::blank(); width],
        }
    }

    pub(crate) fn role_text(text: &str, role: AsciiColorRole) -> Self {
        let mut line = Self::new();
        line.push_role_text(text, role);
        line
    }

    pub(crate) fn plain_text(text: &str) -> Self {
        let mut line = Self::new();
        for ch in text.chars() {
            line.push_plain_char(ch);
        }
        line
    }

    pub(crate) fn text_with_roles(text: &str, roles: Vec<Option<AsciiColorRole>>) -> Self {
        assert_eq!(text.chars().count(), roles.len());
        let mut line = Self::new();
        for (ch, role) in text.chars().zip(roles) {
            match role {
                Some(role) => line.push_role_char(ch, role),
                None => line.push_plain_char(ch),
            }
        }
        line
    }

    pub(crate) fn len(&self) -> usize {
        self.cells.len()
    }

    pub(crate) fn get(&self, index: usize) -> Option<char> {
        self.cells.get(index).and_then(|cell| cell.output_char())
    }

    pub(crate) fn text(&self) -> String {
        self.cells
            .iter()
            .filter_map(|cell| cell.output_char())
            .collect()
    }

    pub(crate) fn into_text(self) -> String {
        self.cells
            .into_iter()
            .filter_map(|cell| cell.output_char())
            .collect()
    }

    pub(crate) fn pad_to(&mut self, width: usize) {
        if self.cells.len() < width {
            self.cells.resize(width, StyledCell::blank());
        }
    }

    pub(crate) fn push_plain_char(&mut self, ch: char) {
        push_primary_cell(&mut self.cells, ch, None);
    }

    pub(crate) fn push_spaces(&mut self, count: usize) {
        self.cells
            .extend(std::iter::repeat_n(StyledCell::blank(), count));
    }

    pub(crate) fn push_line(&mut self, line: &StyledLine) {
        self.cells.extend(line.cells.iter().copied());
    }

    pub(crate) fn push_role_char(&mut self, ch: char, role: AsciiColorRole) {
        push_primary_cell(&mut self.cells, ch, Some(CanvasColor::Role(role)));
    }

    pub(crate) fn push_role_text(&mut self, text: &str, role: AsciiColorRole) {
        for ch in text.chars() {
            self.push_role_char(ch, role);
        }
    }

    pub(crate) fn push_role_text_with_unstyled_trailing_spaces(
        &mut self,
        text: &str,
        role: AsciiColorRole,
    ) {
        let trimmed = text.trim_end_matches(' ');
        self.push_role_text(trimmed, role);
        self.push_spaces(text.chars().count() - trimmed.chars().count());
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
        let len = display_width(text);
        self.push_spaces(width.saturating_sub(len));
        self.push_role_text(text, role);
    }

    pub(crate) fn push_cells(&mut self, cells: &[StyledCell]) {
        self.cells.extend(cells.iter().copied());
    }

    pub(crate) fn set_role(&mut self, index: usize, ch: char, role: AsciiColorRole) {
        let background = self
            .cells
            .get(index)
            .map(|cell| cell.raw_style().background)
            .unwrap_or_default();
        write_primary_cell_style(
            &mut self.cells,
            index,
            ch,
            CanvasStyle {
                foreground: Some(CanvasColor::Role(role)),
                background,
            },
        );
    }

    pub(crate) fn set_background_color(&mut self, index: usize, color: AsciiRgb) {
        if let Some(cell) = self.cells.get_mut(index) {
            cell.set_background(CanvasColor::Direct(color));
        }
    }

    pub(crate) fn set_background_color_if_unset(&mut self, index: usize, color: AsciiRgb) {
        let Some(cell) = self.cells.get_mut(index) else {
            return;
        };
        if cell.raw_style().background.is_none() {
            cell.set_background(CanvasColor::Direct(color));
        }
    }

    pub(crate) fn write_text_role(&mut self, start: usize, text: &str, role: AsciiColorRole) {
        let mut offset = 0;
        for ch in text.chars() {
            self.set_role(start + offset, ch, role);
            offset += char_display_width(ch);
        }
    }

    pub(crate) fn write_line(&mut self, start: usize, line: &StyledLine) {
        for (offset, cell) in line.cells.iter().copied().enumerate() {
            write_primary_cell_from_cell(&mut self.cells, start + offset, cell);
        }
    }

    pub(crate) fn trim_right(mut self) -> Self {
        while self
            .cells
            .last()
            .is_some_and(|cell| cell.output_char() == Some(' '))
        {
            self.cells.pop();
        }
        self
    }

    pub(crate) fn write_to(&self, canvas: &mut Canvas, y: usize) {
        self.write_to_at(canvas, 0, y);
    }

    pub(crate) fn write_to_at(&self, canvas: &mut Canvas, x_offset: usize, y: usize) {
        for (x, cell) in self.cells.iter().enumerate() {
            if cell.is_continuation() {
                continue;
            }
            if let Some(style) = cell.style() {
                canvas.set_style(x_offset + x, y, cell.output_char().unwrap_or(' '), style);
            } else {
                canvas.set(x_offset + x, y, cell.output_char().unwrap_or(' '));
            }
        }
    }
}

pub(crate) fn display_width(text: &str) -> usize {
    terminal_display_width(text)
}

pub(crate) fn wrap_display_lines(text: &str, max_width: usize) -> Vec<String> {
    let max_width = max_width.max(1);
    let mut lines = Vec::new();

    for paragraph in text.split('\n') {
        wrap_display_paragraph(paragraph, max_width, &mut lines);
    }

    lines
}

pub(crate) fn split_label_lines(raw: &str) -> Vec<String> {
    normalize_label_breaks(raw)
        .split('\n')
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn wrap_label_lines(raw: &str, max_width: usize) -> Vec<String> {
    let normalized = normalize_label_breaks(raw);
    let mut lines = Vec::new();
    for paragraph in normalized.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
        } else {
            lines.extend(wrap_display_lines(paragraph, max_width));
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

fn wrap_display_paragraph(text: &str, max_width: usize, lines: &mut Vec<String>) {
    let mut current = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_width = display_width(word);
        if word_width > max_width {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
                current_width = 0;
            }
            push_wrapped_word(word, max_width, lines);
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

fn push_wrapped_word(word: &str, max_width: usize, lines: &mut Vec<String>) {
    let mut current = String::new();
    let mut current_width = 0;

    for ch in word.chars() {
        let ch_width = char_display_width(ch);
        if current_width + ch_width > max_width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
        }
        current.push(ch);
        current_width += ch_width;
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
        let mut line = StyledLine::new();
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
        let mut line = StyledLine::new();
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
        let mut line = StyledLine::role_text("A ", AsciiColorRole::Text).trim_right();
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
        let mut target = StyledLine::plain_text("abcd");
        let source = StyledLine::plain_text("中");

        target.write_line(1, &source);

        assert_eq!(target.len(), 4);
        assert_eq!(target.text(), "a中d");
        assert_eq!(target.get(1), Some('中'));
        assert_eq!(target.get(2), None);
        assert_eq!(target.get(3), Some('d'));
    }

    #[test]
    fn styled_line_write_line_ignores_source_continuation_cells() {
        let mut target = StyledLine::plain_text("abcd");
        let source = StyledLine::plain_text("中Z");

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
        let mut target = StyledLine::plain_text("abcd");
        let mut source = StyledLine::new();
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
}
