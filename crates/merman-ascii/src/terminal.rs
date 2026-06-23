use crate::color::{AsciiColorRole, AsciiColorTheme, AsciiRgb};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CanvasColor {
    Role(AsciiColorRole),
    Direct(AsciiRgb),
}

impl CanvasColor {
    pub(crate) fn resolve(self, theme: AsciiColorTheme) -> AsciiRgb {
        match self {
            Self::Role(role) => theme.color_for(role),
            Self::Direct(color) => color,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TerminalCell {
    ch: char,
    color: Option<CanvasColor>,
    continuation: bool,
}

impl TerminalCell {
    pub(crate) fn blank() -> Self {
        Self {
            ch: ' ',
            color: None,
            continuation: false,
        }
    }

    pub(crate) fn plain(ch: char) -> Self {
        Self {
            ch,
            color: None,
            continuation: false,
        }
    }

    pub(crate) fn with_role(ch: char, role: AsciiColorRole) -> Self {
        Self {
            ch,
            color: Some(CanvasColor::Role(role)),
            continuation: false,
        }
    }

    pub(crate) fn with_canvas_color(ch: char, color: CanvasColor) -> Self {
        Self {
            ch,
            color: Some(color),
            continuation: false,
        }
    }

    pub(crate) fn continuation() -> Self {
        Self {
            ch: ' ',
            color: None,
            continuation: true,
        }
    }

    pub(crate) fn output_char(self) -> Option<char> {
        (!self.continuation).then_some(self.ch)
    }

    pub(crate) fn color(self) -> Option<CanvasColor> {
        (!self.continuation).then_some(self.color).flatten()
    }

    pub(crate) fn is_continuation(self) -> bool {
        self.continuation
    }

    pub(crate) fn is_trimmable_blank(self, preserve_color: bool) -> bool {
        !self.continuation && self.ch == ' ' && (!preserve_color || self.color.is_none())
    }
}

pub(crate) fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

pub(crate) fn char_display_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0).max(1)
}

pub(crate) fn push_primary_cell(
    cells: &mut Vec<TerminalCell>,
    ch: char,
    color: Option<CanvasColor>,
) {
    cells.push(primary_cell(ch, color));
    for _ in 1..char_display_width(ch) {
        cells.push(TerminalCell::continuation());
    }
}

pub(crate) fn write_primary_cell(
    cells: &mut [TerminalCell],
    index: usize,
    ch: char,
    color: Option<CanvasColor>,
) {
    if index >= cells.len() || cells[index].is_continuation() {
        return;
    }

    let width = char_display_width(ch);
    if index + width > cells.len() {
        return;
    }

    for offset in 0..width {
        clear_following_continuation(cells, index + offset);
    }

    cells[index] = primary_cell(ch, color);
    for offset in 1..width {
        cells[index + offset] = TerminalCell::continuation();
    }
}

fn primary_cell(ch: char, color: Option<CanvasColor>) -> TerminalCell {
    match color {
        Some(color) => TerminalCell::with_canvas_color(ch, color),
        None => TerminalCell::plain(ch),
    }
}

fn clear_following_continuation(cells: &mut [TerminalCell], index: usize) {
    if cells
        .get(index + 1)
        .is_some_and(|cell| cell.is_continuation())
    {
        cells[index + 1] = TerminalCell::blank();
    }
}
