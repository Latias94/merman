use super::model::GraphDirection;
use crate::canvas::{Canvas, CanvasColor};
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::options::TerminalWidthProfile;
use crate::text::display_width_with_profile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum OutputTransform {
    Identity,
    HorizontalMirror,
    VerticalMirror,
}

impl OutputTransform {
    pub(super) fn for_direction(direction: GraphDirection) -> Self {
        match direction {
            GraphDirection::LeftRight | GraphDirection::TopDown => Self::Identity,
            GraphDirection::RightLeft => Self::HorizontalMirror,
            GraphDirection::BottomTop => Self::VerticalMirror,
        }
    }

    pub(super) const fn is_identity(self) -> bool {
        matches!(self, Self::Identity)
    }

    pub(super) fn text_x(self, x: usize, text_width: usize, width: usize) -> usize {
        match self {
            Self::HorizontalMirror => width.saturating_sub(x).saturating_sub(text_width),
            Self::Identity | Self::VerticalMirror => x,
        }
    }

    pub(super) fn text_y(self, y: usize, height: usize) -> usize {
        match self {
            Self::VerticalMirror => height.saturating_sub(1).saturating_sub(y),
            Self::Identity | Self::HorizontalMirror => y,
        }
    }

    pub(super) fn map_char(self, ch: char) -> char {
        match self {
            Self::Identity => ch,
            Self::HorizontalMirror => mirror_horizontal_char(ch),
            Self::VerticalMirror => mirror_vertical_char(ch),
        }
    }
}

pub(super) trait GraphSurface {
    fn is_identity(&self) -> bool;
    fn get(&self, x: usize, y: usize) -> Option<char>;
    fn set(&mut self, x: usize, y: usize, ch: char) -> crate::Result<()>;
    fn set_role(&mut self, x: usize, y: usize, ch: char, role: AsciiColorRole)
    -> crate::Result<()>;
    fn set_color(&mut self, x: usize, y: usize, ch: char, color: AsciiRgb) -> crate::Result<()>;
    fn set_canvas_color(
        &mut self,
        x: usize,
        y: usize,
        ch: char,
        color: CanvasColor,
    ) -> crate::Result<()>;
    fn set_background_color(&mut self, x: usize, y: usize, color: AsciiRgb);
    fn write_text_role(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        role: AsciiColorRole,
    ) -> crate::Result<()>;
    fn write_text_color(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        color: AsciiRgb,
    ) -> crate::Result<()>;
}

impl GraphSurface for Canvas {
    fn is_identity(&self) -> bool {
        true
    }

    fn get(&self, x: usize, y: usize) -> Option<char> {
        Canvas::get(self, x, y)
    }

    fn set(&mut self, x: usize, y: usize, ch: char) -> crate::Result<()> {
        Canvas::try_set(self, x, y, ch)
    }

    fn set_role(
        &mut self,
        x: usize,
        y: usize,
        ch: char,
        role: AsciiColorRole,
    ) -> crate::Result<()> {
        Canvas::try_set_role(self, x, y, ch, role)
    }

    fn set_color(&mut self, x: usize, y: usize, ch: char, color: AsciiRgb) -> crate::Result<()> {
        Canvas::try_set_color(self, x, y, ch, color)
    }

    fn set_canvas_color(
        &mut self,
        x: usize,
        y: usize,
        ch: char,
        color: CanvasColor,
    ) -> crate::Result<()> {
        Canvas::try_set_canvas_color(self, x, y, ch, color)
    }

    fn set_background_color(&mut self, x: usize, y: usize, color: AsciiRgb) {
        Canvas::set_background_color(self, x, y, color);
    }

    fn write_text_role(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        role: AsciiColorRole,
    ) -> crate::Result<()> {
        Canvas::write_text_role(self, x, y, text, role)
    }

    fn write_text_color(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        color: AsciiRgb,
    ) -> crate::Result<()> {
        Canvas::write_text_color(self, x, y, text, color)
    }
}

pub(super) struct TransformedSurface<'a> {
    canvas: &'a mut Canvas,
    transform: OutputTransform,
    width: usize,
    height: usize,
    width_profile: TerminalWidthProfile,
}

impl<'a> TransformedSurface<'a> {
    pub(super) const fn new(
        canvas: &'a mut Canvas,
        transform: OutputTransform,
        width: usize,
        height: usize,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        Self {
            canvas,
            transform,
            width,
            height,
            width_profile,
        }
    }

    fn map_cell(&self, x: usize, y: usize) -> Option<(usize, usize)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some((
            self.transform.text_x(x, 1, self.width),
            self.transform.text_y(y, self.height),
        ))
    }

    fn map_text(&self, x: usize, y: usize, text: &str) -> Option<(usize, usize)> {
        let text_width = display_width_with_profile(text, self.width_profile);
        if y >= self.height || x.checked_add(text_width)? > self.width {
            return None;
        }
        Some((
            self.transform.text_x(x, text_width, self.width),
            self.transform.text_y(y, self.height),
        ))
    }
}

impl GraphSurface for TransformedSurface<'_> {
    fn is_identity(&self) -> bool {
        self.transform.is_identity()
    }

    fn get(&self, x: usize, y: usize) -> Option<char> {
        let (target_x, target_y) = self.map_cell(x, y)?;
        Canvas::get(self.canvas, target_x, target_y).map(|ch| self.transform.map_char(ch))
    }

    fn set(&mut self, x: usize, y: usize, ch: char) -> crate::Result<()> {
        if let Some((target_x, target_y)) = self.map_cell(x, y) {
            self.canvas
                .try_set(target_x, target_y, self.transform.map_char(ch))?;
        }
        Ok(())
    }

    fn set_role(
        &mut self,
        x: usize,
        y: usize,
        ch: char,
        role: AsciiColorRole,
    ) -> crate::Result<()> {
        if let Some((target_x, target_y)) = self.map_cell(x, y) {
            self.canvas
                .try_set_role(target_x, target_y, self.transform.map_char(ch), role)?;
        }
        Ok(())
    }

    fn set_color(&mut self, x: usize, y: usize, ch: char, color: AsciiRgb) -> crate::Result<()> {
        if let Some((target_x, target_y)) = self.map_cell(x, y) {
            self.canvas
                .try_set_color(target_x, target_y, self.transform.map_char(ch), color)?;
        }
        Ok(())
    }

    fn set_canvas_color(
        &mut self,
        x: usize,
        y: usize,
        ch: char,
        color: CanvasColor,
    ) -> crate::Result<()> {
        if let Some((target_x, target_y)) = self.map_cell(x, y) {
            self.canvas.try_set_canvas_color(
                target_x,
                target_y,
                self.transform.map_char(ch),
                color,
            )?;
        }
        Ok(())
    }

    fn set_background_color(&mut self, x: usize, y: usize, color: AsciiRgb) {
        if let Some((target_x, target_y)) = self.map_cell(x, y) {
            self.canvas.set_background_color(target_x, target_y, color);
        }
    }

    fn write_text_role(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        role: AsciiColorRole,
    ) -> crate::Result<()> {
        if let Some((target_x, target_y)) = self.map_text(x, y, text) {
            self.canvas
                .write_text_role(target_x, target_y, text, role)?;
        }
        Ok(())
    }

    fn write_text_color(
        &mut self,
        x: usize,
        y: usize,
        text: &str,
        color: AsciiRgb,
    ) -> crate::Result<()> {
        if let Some((target_x, target_y)) = self.map_text(x, y, text) {
            self.canvas
                .write_text_color(target_x, target_y, text, color)?;
        }
        Ok(())
    }
}

fn mirror_horizontal_char(ch: char) -> char {
    match ch {
        '>' => '<',
        '<' => '>',
        '▷' => '◁',
        '◁' => '▷',
        '►' => '◄',
        '◄' => '►',
        '/' => '\\',
        '\\' => '/',
        '┌' => '┐',
        '┐' => '┌',
        '└' => '┘',
        '┘' => '└',
        '├' => '┤',
        '┤' => '├',
        '╭' => '╮',
        '╮' => '╭',
        '╰' => '╯',
        '╯' => '╰',
        '⌜' => '⌝',
        '⌝' => '⌜',
        '⌞' => '⌟',
        '⌟' => '⌞',
        '(' => ')',
        ')' => '(',
        ch => ch,
    }
}

fn mirror_vertical_char(ch: char) -> char {
    match ch {
        '^' => 'v',
        'v' => '^',
        '▲' => '▼',
        '▼' => '▲',
        '/' => '\\',
        '\\' => '/',
        '┌' => '└',
        '└' => '┌',
        '┐' => '┘',
        '┘' => '┐',
        '┬' => '┴',
        '┴' => '┬',
        '╭' => '╰',
        '╰' => '╭',
        '╮' => '╯',
        '╯' => '╮',
        '⌜' => '⌞',
        '⌞' => '⌜',
        '⌝' => '⌟',
        '⌟' => '⌝',
        ch => ch,
    }
}
