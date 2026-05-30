use super::layout::SequenceLayout;
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceLine {
    chars: Vec<char>,
    roles: Vec<Option<AsciiColorRole>>,
}

impl SequenceLine {
    pub(super) fn blank(width: usize) -> Self {
        Self {
            chars: vec![' '; width],
            roles: vec![None; width],
        }
    }

    pub(super) fn len(&self) -> usize {
        self.chars.len()
    }

    pub(super) fn get(&self, index: usize) -> Option<char> {
        self.chars.get(index).copied()
    }

    pub(super) fn into_text(self) -> String {
        self.chars.into_iter().collect()
    }

    pub(super) fn pad_to(&mut self, width: usize) {
        if self.chars.len() < width {
            let needed = width - self.chars.len();
            self.chars.extend(std::iter::repeat_n(' ', needed));
            self.roles.extend(std::iter::repeat_n(None, needed));
        }
    }

    pub(super) fn push_spaces(&mut self, count: usize) {
        self.chars.extend(std::iter::repeat_n(' ', count));
        self.roles.extend(std::iter::repeat_n(None, count));
    }

    pub(super) fn push_line(&mut self, line: &SequenceLine) {
        self.chars.extend(line.chars.iter().copied());
        self.roles.extend(line.roles.iter().copied());
    }

    pub(super) fn set_role(&mut self, index: usize, ch: char, role: AsciiColorRole) {
        if let Some(cell) = self.chars.get_mut(index) {
            *cell = ch;
            self.roles[index] = Some(role);
        }
    }

    pub(super) fn write_text_role(&mut self, start: usize, text: &str, role: AsciiColorRole) {
        for (offset, ch) in text.chars().enumerate() {
            self.set_role(start + offset, ch, role);
        }
    }

    pub(super) fn write_line(&mut self, start: usize, line: &SequenceLine) {
        for (offset, (&ch, &role)) in line.chars.iter().zip(line.roles.iter()).enumerate() {
            let index = start + offset;
            if let Some(cell) = self.chars.get_mut(index) {
                *cell = ch;
                self.roles[index] = role;
            }
        }
    }

    pub(super) fn trim_right(mut self) -> Self {
        while self.chars.last() == Some(&' ') {
            self.chars.pop();
            self.roles.pop();
        }
        self
    }

    pub(super) fn write_to(&self, canvas: &mut Canvas, y: usize) {
        for (x, (&ch, &role)) in self.chars.iter().zip(self.roles.iter()).enumerate() {
            if let Some(role) = role {
                canvas.set_role(x, y, ch, role);
            } else {
                canvas.set(x, y, ch);
            }
        }
    }
}

pub(super) fn padded_line(mut line: SequenceLine, width: usize) -> SequenceLine {
    line.pad_to(width);
    line
}

pub(super) fn ensure_self_width(
    line: SequenceLine,
    layout: &SequenceLayout,
    needed: usize,
) -> SequenceLine {
    let width = (layout.total_width + layout.self_message_width + 1).max(needed);
    padded_line(line, width)
}

pub(super) fn write_text_role(
    line: &mut SequenceLine,
    start: usize,
    text: &str,
    role: AsciiColorRole,
) {
    line.write_text_role(start, text, role);
}

pub(super) fn trim_right(line: SequenceLine) -> SequenceLine {
    line.trim_right()
}
