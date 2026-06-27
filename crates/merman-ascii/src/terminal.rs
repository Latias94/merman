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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CanvasStyle {
    pub(crate) foreground: Option<CanvasColor>,
    pub(crate) background: Option<CanvasColor>,
}

impl CanvasStyle {
    pub(crate) fn foreground(color: CanvasColor) -> Self {
        Self {
            foreground: Some(color),
            background: None,
        }
    }

    pub(crate) fn foreground_option(color: Option<CanvasColor>) -> Self {
        Self {
            foreground: color,
            background: None,
        }
    }

    pub(crate) fn with_foreground(mut self, color: Option<CanvasColor>) -> Self {
        self.foreground = color;
        self
    }

    pub(crate) fn is_plain(self) -> bool {
        self.foreground.is_none() && self.background.is_none()
    }

    pub(crate) fn resolve(self, theme: AsciiColorTheme) -> ResolvedCanvasStyle {
        ResolvedCanvasStyle {
            foreground: self.foreground.map(|color| color.resolve(theme)),
            background: self.background.map(|color| color.resolve(theme)),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResolvedCanvasStyle {
    pub(crate) foreground: Option<AsciiRgb>,
    pub(crate) background: Option<AsciiRgb>,
}

impl ResolvedCanvasStyle {
    pub(crate) fn is_plain(self) -> bool {
        self.foreground.is_none() && self.background.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TerminalCell {
    ch: char,
    style: CanvasStyle,
    continuation: bool,
}

impl TerminalCell {
    pub(crate) fn blank() -> Self {
        Self {
            ch: ' ',
            style: CanvasStyle::default(),
            continuation: false,
        }
    }

    pub(crate) fn with_role(ch: char, role: AsciiColorRole) -> Self {
        Self {
            ch,
            style: CanvasStyle::foreground(CanvasColor::Role(role)),
            continuation: false,
        }
    }

    pub(crate) fn with_style(ch: char, style: CanvasStyle) -> Self {
        Self {
            ch,
            style,
            continuation: false,
        }
    }

    pub(crate) fn continuation() -> Self {
        Self {
            ch: ' ',
            style: CanvasStyle::default(),
            continuation: true,
        }
    }

    pub(crate) fn output_char(self) -> Option<char> {
        (!self.continuation).then_some(self.ch)
    }

    pub(crate) fn output_char_with_style(self) -> Option<(char, CanvasStyle)> {
        (!self.continuation).then_some((self.ch, self.style))
    }

    #[cfg(test)]
    pub(crate) fn color(self) -> Option<CanvasColor> {
        (!self.continuation)
            .then_some(self.style.foreground)
            .flatten()
    }

    pub(crate) fn style(self) -> Option<CanvasStyle> {
        (!self.continuation && !self.style.is_plain()).then_some(self.style)
    }

    pub(crate) fn raw_style(self) -> CanvasStyle {
        if self.continuation {
            CanvasStyle::default()
        } else {
            self.style
        }
    }

    pub(crate) fn set_background(&mut self, color: CanvasColor) {
        if !self.continuation {
            self.style.background = Some(color);
        }
    }

    pub(crate) fn is_continuation(self) -> bool {
        self.continuation
    }

    pub(crate) fn is_trimmable_blank(self, preserve_color: bool) -> bool {
        !self.continuation && self.ch == ' ' && (!preserve_color || self.style.is_plain())
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
    push_primary_cell_style(cells, ch, CanvasStyle::foreground_option(color));
}

pub(crate) fn push_primary_cell_style(cells: &mut Vec<TerminalCell>, ch: char, style: CanvasStyle) {
    cells.push(TerminalCell::with_style(ch, style));
    for _ in 1..char_display_width(ch) {
        cells.push(TerminalCell::continuation());
    }
}

pub(crate) fn write_primary_cell_style(
    cells: &mut [TerminalCell],
    index: usize,
    ch: char,
    style: CanvasStyle,
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

    cells[index] = TerminalCell::with_style(ch, style);
    for offset in 1..width {
        cells[index + offset] = TerminalCell::continuation();
    }
}

pub(crate) fn write_primary_cell_from_cell(
    cells: &mut [TerminalCell],
    index: usize,
    cell: TerminalCell,
) {
    let Some((ch, style)) = cell.output_char_with_style() else {
        return;
    };
    write_primary_cell_style(cells, index, ch, style);
}

fn clear_following_continuation(cells: &mut [TerminalCell], index: usize) {
    if cells
        .get(index + 1)
        .is_some_and(|cell| cell.is_continuation())
    {
        cells[index + 1] = TerminalCell::blank();
    }
}
