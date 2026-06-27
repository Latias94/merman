use crate::color::{AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRgb};
use crate::options::AsciiRenderOptions;
use crate::terminal::{
    CanvasStyle, ResolvedCanvasStyle, TerminalCell, char_display_width, write_primary_cell_style,
};
use std::env;
use std::fmt::Write as _;
use std::io::{self, IsTerminal};

pub(crate) use crate::terminal::CanvasColor;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<TerminalCell>,
}

impl Canvas {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        let cell_count = width.saturating_mul(height);
        Self {
            width,
            height,
            cells: vec![TerminalCell::blank(); cell_count],
        }
    }

    pub(crate) fn set(&mut self, x: usize, y: usize, ch: char) {
        if let Some(index) = self.index_for_char(x, y, ch) {
            let style = self.cells[index].raw_style().with_foreground(None);
            write_primary_cell_style(&mut self.cells, index, ch, style);
        }
    }

    pub(crate) fn set_role(&mut self, x: usize, y: usize, ch: char, role: AsciiColorRole) {
        self.set_canvas_color(x, y, ch, CanvasColor::Role(role));
    }

    pub(crate) fn set_color(&mut self, x: usize, y: usize, ch: char, color: AsciiRgb) {
        self.set_canvas_color(x, y, ch, CanvasColor::Direct(color));
    }

    pub(crate) fn set_canvas_color(&mut self, x: usize, y: usize, ch: char, color: CanvasColor) {
        if let Some(index) = self.index_for_char(x, y, ch) {
            let style = self.cells[index].raw_style().with_foreground(Some(color));
            write_primary_cell_style(&mut self.cells, index, ch, style);
        }
    }

    pub(crate) fn set_style(&mut self, x: usize, y: usize, ch: char, style: CanvasStyle) {
        if let Some(index) = self.index_for_char(x, y, ch) {
            write_primary_cell_style(&mut self.cells, index, ch, style);
        }
    }

    pub(crate) fn set_background_color(&mut self, x: usize, y: usize, color: AsciiRgb) {
        self.set_background_canvas_color(x, y, CanvasColor::Direct(color));
    }

    pub(crate) fn set_background_canvas_color(&mut self, x: usize, y: usize, color: CanvasColor) {
        if let Some(index) = self.index(x, y) {
            self.cells[index].set_background(color);
        }
    }

    pub(crate) fn get(&self, x: usize, y: usize) -> Option<char> {
        self.index(x, y)
            .and_then(|index| self.cells[index].output_char())
    }

    #[cfg(test)]
    pub(crate) fn get_color(&self, x: usize, y: usize) -> Option<CanvasColor> {
        self.index(x, y).and_then(|index| self.cells[index].color())
    }

    pub(crate) fn get_style(&self, x: usize, y: usize) -> Option<CanvasStyle> {
        self.index(x, y).and_then(|index| self.cells[index].style())
    }

    #[allow(dead_code)]
    pub(crate) fn write_text(&mut self, x: usize, y: usize, text: &str) {
        let mut offset = 0;
        for ch in text.chars() {
            self.set(x + offset, y, ch);
            offset += char_display_width(ch);
        }
    }

    pub(crate) fn write_text_role(&mut self, x: usize, y: usize, text: &str, role: AsciiColorRole) {
        let mut offset = 0;
        for ch in text.chars() {
            self.set_role(x + offset, y, ch, role);
            offset += char_display_width(ch);
        }
    }

    pub(crate) fn write_text_color(&mut self, x: usize, y: usize, text: &str, color: AsciiRgb) {
        let mut offset = 0;
        for ch in text.chars() {
            self.set_color(x + offset, y, ch, color);
            offset += char_display_width(ch);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn finish(self) -> String {
        self.finish_plain(false)
    }

    #[allow(dead_code)]
    pub(crate) fn finish_trimmed(self) -> String {
        self.finish_plain(true)
    }

    pub(crate) fn finish_with_options(self, options: &AsciiRenderOptions) -> String {
        self.finish_with_options_internal(options, false)
    }

    #[allow(dead_code)]
    pub(crate) fn finish_trimmed_with_options(self, options: &AsciiRenderOptions) -> String {
        self.finish_with_options_internal(options, true)
    }

    fn finish_plain(self, trim: bool) -> String {
        if self.width == 0 || self.height == 0 {
            return String::new();
        }

        let mut out = String::new();
        for row_start in (0..self.cells.len()).step_by(self.width) {
            let row_end = if trim {
                self.trimmed_row_end(row_start, row_start + self.width, false)
            } else {
                row_start + self.width
            };
            for cell in &self.cells[row_start..row_end] {
                if let Some(ch) = cell.output_char() {
                    out.push(ch);
                }
            }
            out.push('\n');
        }
        out
    }

    fn finish_with_options_internal(self, options: &AsciiRenderOptions, trim: bool) -> String {
        match resolve_color_mode(options.color_mode) {
            AsciiColorMode::Plain => self.finish_plain(trim),
            AsciiColorMode::Auto => {
                unreachable!("auto color mode must be resolved before encoding")
            }
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

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y * self.width + x)
    }

    fn index_for_char(&self, x: usize, y: usize, ch: char) -> Option<usize> {
        if x.saturating_add(char_display_width(ch)) > self.width {
            return None;
        }
        self.index(x, y)
    }

    fn finish_ansi(self, theme: AsciiColorTheme, mode: AsciiColorMode, trim: bool) -> String {
        if self.width == 0 || self.height == 0 {
            return String::new();
        }

        let mut out = String::new();
        for row_start in (0..self.cells.len()).step_by(self.width) {
            let row_end = if trim {
                self.trimmed_row_end(row_start, row_start + self.width, true)
            } else {
                row_start + self.width
            };
            let mut active_style = ResolvedCanvasStyle::default();
            for cell in &self.cells[row_start..row_end] {
                let Some(ch) = cell.output_char() else {
                    continue;
                };
                let desired_style = cell.raw_style().resolve(theme);
                if desired_style != active_style {
                    if !active_style.is_plain() {
                        out.push_str("\u{1b}[0m");
                    }
                    if !desired_style.is_plain() {
                        push_ansi_start(&mut out, mode, desired_style);
                    }
                    active_style = desired_style;
                }
                out.push(ch);
            }
            if !active_style.is_plain() {
                out.push_str("\u{1b}[0m");
            }
            out.push('\n');
        }
        out
    }

    fn finish_html(self, theme: AsciiColorTheme, trim: bool) -> String {
        if self.width == 0 || self.height == 0 {
            return String::new();
        }

        let mut out = String::new();
        for row_start in (0..self.cells.len()).step_by(self.width) {
            let row_end = if trim {
                self.trimmed_row_end(row_start, row_start + self.width, true)
            } else {
                row_start + self.width
            };
            let mut active_style = ResolvedCanvasStyle::default();
            for cell in &self.cells[row_start..row_end] {
                let Some(ch) = cell.output_char() else {
                    continue;
                };
                let desired_style = cell.raw_style().resolve(theme);
                if desired_style != active_style {
                    if !active_style.is_plain() {
                        out.push_str("</span>");
                    }
                    if !desired_style.is_plain() {
                        push_html_span_start(&mut out, desired_style);
                    }
                    active_style = desired_style;
                }
                push_html_escaped_char(&mut out, ch);
            }
            if !active_style.is_plain() {
                out.push_str("</span>");
            }
            out.push('\n');
        }
        out
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

fn resolve_color_mode(mode: AsciiColorMode) -> AsciiColorMode {
    match mode {
        AsciiColorMode::Plain
        | AsciiColorMode::Ansi16
        | AsciiColorMode::Ansi256
        | AsciiColorMode::TrueColor
        | AsciiColorMode::Html => mode,
        AsciiColorMode::Auto => detect_auto_color_mode(),
    }
}

fn detect_auto_color_mode() -> AsciiColorMode {
    if env::var_os("NO_COLOR").is_some() {
        return AsciiColorMode::Plain;
    }

    let colorterm = env::var("COLORTERM")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let term = env::var("TERM").unwrap_or_default().to_ascii_lowercase();

    if env::var("CLICOLOR_FORCE").is_ok_and(|value| !value.is_empty() && value != "0") {
        return if supports_truecolor(&colorterm) {
            AsciiColorMode::TrueColor
        } else {
            AsciiColorMode::Ansi256
        };
    }

    if !io::stdout().is_terminal() {
        return AsciiColorMode::Plain;
    }

    if supports_truecolor(&colorterm) {
        return AsciiColorMode::TrueColor;
    }

    if term.contains("256color") {
        return AsciiColorMode::Ansi256;
    }

    if !term.is_empty() && term != "dumb" {
        return AsciiColorMode::Ansi16;
    }

    AsciiColorMode::Plain
}

fn supports_truecolor(colorterm: &str) -> bool {
    colorterm.contains("truecolor") || colorterm.contains("24bit")
}

fn push_ansi_start(out: &mut String, mode: AsciiColorMode, style: ResolvedCanvasStyle) {
    if let Some(color) = style.foreground {
        match mode {
            AsciiColorMode::Ansi16 => out.push_str(ansi16_foreground_start(color)),
            AsciiColorMode::Ansi256 => {
                let _ = write!(out, "\u{1b}[38;5;{}m", ansi256_index(color));
            }
            AsciiColorMode::TrueColor => {
                let _ = write!(out, "\u{1b}[38;2;{};{};{}m", color.r, color.g, color.b);
            }
            AsciiColorMode::Plain | AsciiColorMode::Auto | AsciiColorMode::Html => {}
        }
    }
    if let Some(color) = style.background {
        match mode {
            AsciiColorMode::Ansi16 => out.push_str(ansi16_background_start(color)),
            AsciiColorMode::Ansi256 => {
                let _ = write!(out, "\u{1b}[48;5;{}m", ansi256_index(color));
            }
            AsciiColorMode::TrueColor => {
                let _ = write!(out, "\u{1b}[48;2;{};{};{}m", color.r, color.g, color.b);
            }
            AsciiColorMode::Plain | AsciiColorMode::Auto | AsciiColorMode::Html => {}
        }
    }
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

fn push_html_span_start(out: &mut String, style: ResolvedCanvasStyle) {
    let mut wrote_any = false;
    out.push_str("<span style=\"");
    if let Some(color) = style.foreground {
        let _ = write!(out, "color:#{:02x}{:02x}{:02x}", color.r, color.g, color.b);
        wrote_any = true;
    }
    if let Some(color) = style.background {
        if wrote_any {
            out.push(';');
        }
        let _ = write!(
            out,
            "background-color:#{:02x}{:02x}{:02x}",
            color.r, color.g, color.b
        );
        wrote_any = true;
    }
    if !wrote_any {
        out.push_str("color:inherit");
    }
    out.push_str("\">");
}

fn push_html_escaped_char(out: &mut String, ch: char) {
    match ch {
        '&' => out.push_str("&amp;"),
        '<' => out.push_str("&lt;"),
        '>' => out.push_str("&gt;"),
        '"' => out.push_str("&quot;"),
        '\'' => out.push_str("&#39;"),
        _ => out.push(ch),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRenderOptions, AsciiRgb};

    #[test]
    fn finish_plain_ignores_color_roles() {
        let mut canvas = Canvas::new(3, 1);
        canvas.write_text_role(0, 0, "AB", AsciiColorRole::Text);
        canvas.set(2, 0, '!');

        assert_eq!(canvas.clone().finish(), "AB!\n");
        assert_eq!(
            canvas.finish_with_options(
                &AsciiRenderOptions::ascii().with_color_mode(AsciiColorMode::Plain)
            ),
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
        canvas.write_text_role(0, 0, "中A", AsciiColorRole::Text);
        canvas.set_role(1, 0, 'X', AsciiColorRole::EdgeLine);

        assert_eq!(canvas.get(0, 0), Some('中'));
        assert_eq!(canvas.get(1, 0), None);
        assert_eq!(canvas.get(2, 0), Some('A'));
        assert_eq!(canvas.finish(), "中A \n");
    }

    #[test]
    fn wide_text_does_not_cross_canvas_row_boundary() {
        let mut canvas = Canvas::new(2, 2);
        canvas.set(1, 0, '中');
        canvas.set(0, 1, 'B');

        assert_eq!(canvas.get(1, 0), Some(' '));
        assert_eq!(canvas.get(0, 1), Some('B'));
        assert_eq!(canvas.finish(), "  \nB \n");
    }

    #[test]
    fn emoji_text_does_not_cross_canvas_row_boundary() {
        let mut canvas = Canvas::new(2, 2);
        canvas.set(1, 0, '🚀');
        canvas.set(0, 1, 'B');

        assert_eq!(canvas.get(1, 0), Some(' '));
        assert_eq!(canvas.get(0, 1), Some('B'));
        assert_eq!(canvas.finish(), "  \nB \n");
    }

    #[test]
    fn styled_wide_text_does_not_cross_canvas_row_boundary() {
        let mut canvas = Canvas::new(2, 2);
        canvas.set_style(
            1,
            0,
            '中',
            CanvasStyle::foreground(CanvasColor::Role(AsciiColorRole::Text)),
        );
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
        canvas.write_text_role(0, 0, "AB", AsciiColorRole::Text);

        let output = canvas.finish_trimmed_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::TrueColor)
                .with_color_theme(theme),
        );

        assert_eq!(output, "\u{1b}[38;2;1;2;3mAB\u{1b}[0m\n");
    }

    #[test]
    fn finish_truecolor_keeps_role_run_across_wide_text() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut canvas = Canvas::new(3, 1);
        canvas.write_text_role(0, 0, "中A", AsciiColorRole::Text);

        let output = canvas.finish_trimmed_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::TrueColor)
                .with_color_theme(theme),
        );

        assert_eq!(output, "\u{1b}[38;2;1;2;3m中A\u{1b}[0m\n");
    }

    #[test]
    fn finish_trimmed_html_trims_unstyled_trailing_spaces_and_escapes_text() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0xff0000));
        let mut canvas = Canvas::new(4, 1);
        canvas.write_text_role(0, 0, "<&", AsciiColorRole::Text);

        let output = canvas.finish_trimmed_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::Html)
                .with_color_theme(theme),
        );

        assert_eq!(output, "<span style=\"color:#ff0000\">&lt;&amp;</span>\n");
    }

    #[test]
    fn finish_truecolor_groups_same_role_runs() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut canvas = Canvas::new(3, 1);
        canvas.write_text_role(0, 0, "AB", AsciiColorRole::Text);
        canvas.set(2, 0, '!');

        let output = canvas.finish_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::TrueColor)
                .with_color_theme(theme),
        );

        assert_eq!(output, "\u{1b}[38;2;1;2;3mAB\u{1b}[0m!\n");
    }

    #[test]
    fn finish_truecolor_encodes_foreground_and_background() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut canvas = Canvas::new(1, 1);
        canvas.set_background_color(0, 0, AsciiRgb::new(4, 5, 6));
        canvas.set_role(0, 0, 'A', AsciiColorRole::Text);

        let output = canvas.finish_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::TrueColor)
                .with_color_theme(theme),
        );

        assert_eq!(output, "\u{1b}[38;2;1;2;3m\u{1b}[48;2;4;5;6mA\u{1b}[0m\n");
    }

    #[test]
    fn finish_html_wraps_foreground_and_background() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 2, 3));
        let mut canvas = Canvas::new(1, 1);
        canvas.set_background_color(0, 0, AsciiRgb::new(4, 5, 6));
        canvas.set_role(0, 0, 'A', AsciiColorRole::Text);

        let output = canvas.finish_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::Html)
                .with_color_theme(theme),
        );

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

        let output = canvas.finish_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::Ansi256)
                .with_color_theme(theme),
        );

        assert_eq!(output, "\u{1b}[38;5;196mR\u{1b}[0m\n");
    }

    #[test]
    fn finish_ansi16_encodes_nearest_role_foreground() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::EdgeLine, AsciiRgb::from_hex24(0xff0000));
        let mut canvas = Canvas::new(1, 1);
        canvas.set_role(0, 0, 'R', AsciiColorRole::EdgeLine);

        let output = canvas.finish_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::Ansi16)
                .with_color_theme(theme),
        );

        assert_eq!(output, "\u{1b}[91mR\u{1b}[0m\n");
    }

    #[test]
    fn finish_html_wraps_role_runs_and_escapes_text() {
        let theme = AsciiColorTheme::default_light()
            .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0xff0000));
        let mut canvas = Canvas::new(3, 1);
        canvas.write_text_role(0, 0, "<&>", AsciiColorRole::Text);

        let output = canvas.finish_with_options(
            &AsciiRenderOptions::ascii()
                .with_color_mode(AsciiColorMode::Html)
                .with_color_theme(theme),
        );

        assert_eq!(
            output,
            "<span style=\"color:#ff0000\">&lt;&amp;&gt;</span>\n"
        );
    }
}
