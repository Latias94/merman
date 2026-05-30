use crate::canvas::{Canvas, CanvasColor};
use crate::color::AsciiColorRole;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StyledCell {
    ch: char,
    color: Option<CanvasColor>,
}

impl StyledCell {
    pub(crate) fn blank() -> Self {
        Self {
            ch: ' ',
            color: None,
        }
    }

    fn plain(ch: char) -> Self {
        Self { ch, color: None }
    }

    pub(crate) fn with_role(ch: char, role: AsciiColorRole) -> Self {
        Self {
            ch,
            color: Some(CanvasColor::Role(role)),
        }
    }
}

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

    pub(crate) fn len(&self) -> usize {
        self.cells.len()
    }

    pub(crate) fn get(&self, index: usize) -> Option<char> {
        self.cells.get(index).map(|cell| cell.ch)
    }

    pub(crate) fn text(&self) -> String {
        self.cells.iter().map(|cell| cell.ch).collect()
    }

    pub(crate) fn into_text(self) -> String {
        self.cells.into_iter().map(|cell| cell.ch).collect()
    }

    pub(crate) fn pad_to(&mut self, width: usize) {
        if self.cells.len() < width {
            self.cells.resize(width, StyledCell::blank());
        }
    }

    pub(crate) fn push_plain_char(&mut self, ch: char) {
        self.cells.push(StyledCell::plain(ch));
    }

    pub(crate) fn push_spaces(&mut self, count: usize) {
        self.cells
            .extend(std::iter::repeat_n(StyledCell::blank(), count));
    }

    pub(crate) fn push_line(&mut self, line: &StyledLine) {
        self.cells.extend(line.cells.iter().copied());
    }

    pub(crate) fn push_role_char(&mut self, ch: char, role: AsciiColorRole) {
        self.cells.push(StyledCell::with_role(ch, role));
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
        let len = text.chars().count();
        self.push_spaces(width.saturating_sub(len));
        self.push_role_text(text, role);
    }

    pub(crate) fn push_cells(&mut self, cells: &[StyledCell]) {
        self.cells.extend(cells.iter().copied());
    }

    pub(crate) fn set_role(&mut self, index: usize, ch: char, role: AsciiColorRole) {
        if let Some(cell) = self.cells.get_mut(index) {
            *cell = StyledCell::with_role(ch, role);
        }
    }

    pub(crate) fn write_text_role(&mut self, start: usize, text: &str, role: AsciiColorRole) {
        for (offset, ch) in text.chars().enumerate() {
            self.set_role(start + offset, ch, role);
        }
    }

    pub(crate) fn write_line(&mut self, start: usize, line: &StyledLine) {
        for (offset, cell) in line.cells.iter().copied().enumerate() {
            if let Some(target) = self.cells.get_mut(start + offset) {
                *target = cell;
            }
        }
    }

    pub(crate) fn trim_right(mut self) -> Self {
        while self.cells.last().is_some_and(|cell| cell.ch == ' ') {
            self.cells.pop();
        }
        self
    }

    pub(crate) fn write_to(&self, canvas: &mut Canvas, y: usize) {
        for (x, cell) in self.cells.iter().enumerate() {
            if let Some(color) = cell.color {
                canvas.set_canvas_color(x, y, cell.ch, color);
            } else {
                canvas.set(x, y, cell.ch);
            }
        }
    }
}

pub(crate) fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub(crate) fn wrap_display_lines(text: &str, max_width: usize) -> Vec<String> {
    let max_width = max_width.max(1);
    let mut lines = Vec::new();

    for paragraph in text.split('\n') {
        wrap_display_paragraph(paragraph, max_width, &mut lines);
    }

    lines
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
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0).max(1);
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
}
