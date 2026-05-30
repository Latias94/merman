use crate::color::{AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRgb};
use crate::options::AsciiRenderOptions;
use std::env;
use std::fmt::Write as _;
use std::io::{self, IsTerminal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<char>,
    roles: Vec<Option<AsciiColorRole>>,
}

impl Canvas {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        let cell_count = width.saturating_mul(height);
        Self {
            width,
            height,
            cells: vec![' '; cell_count],
            roles: vec![None; cell_count],
        }
    }

    pub(crate) fn set(&mut self, x: usize, y: usize, ch: char) {
        if let Some(index) = self.index(x, y) {
            self.cells[index] = ch;
            self.roles[index] = None;
        }
    }

    pub(crate) fn set_role(&mut self, x: usize, y: usize, ch: char, role: AsciiColorRole) {
        if let Some(index) = self.index(x, y) {
            self.cells[index] = ch;
            self.roles[index] = Some(role);
        }
    }

    pub(crate) fn get(&self, x: usize, y: usize) -> Option<char> {
        self.index(x, y).map(|index| self.cells[index])
    }

    pub(crate) fn get_role(&self, x: usize, y: usize) -> Option<AsciiColorRole> {
        self.index(x, y).and_then(|index| self.roles[index])
    }

    pub(crate) fn write_text(&mut self, x: usize, y: usize, text: &str) {
        for (offset, ch) in text.chars().enumerate() {
            self.set(x + offset, y, ch);
        }
    }

    pub(crate) fn write_text_role(&mut self, x: usize, y: usize, text: &str, role: AsciiColorRole) {
        for (offset, ch) in text.chars().enumerate() {
            self.set_role(x + offset, y, ch, role);
        }
    }

    pub(crate) fn finish(self) -> String {
        if self.width == 0 || self.height == 0 {
            return String::new();
        }

        let mut out = String::new();
        for row in self.cells.chunks(self.width) {
            for ch in row {
                out.push(*ch);
            }
            out.push('\n');
        }
        out
    }

    pub(crate) fn finish_with_options(self, options: &AsciiRenderOptions) -> String {
        match resolve_color_mode(options.color_mode) {
            AsciiColorMode::Plain => self.finish(),
            AsciiColorMode::Auto => {
                unreachable!("auto color mode must be resolved before encoding")
            }
            AsciiColorMode::Ansi16 => self.finish_ansi(options.color_theme, AsciiColorMode::Ansi16),
            AsciiColorMode::Ansi256 => {
                self.finish_ansi(options.color_theme, AsciiColorMode::Ansi256)
            }
            AsciiColorMode::TrueColor => {
                self.finish_ansi(options.color_theme, AsciiColorMode::TrueColor)
            }
            AsciiColorMode::Html => self.finish_html(options.color_theme),
        }
    }

    fn index(&self, x: usize, y: usize) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        Some(y * self.width + x)
    }

    fn finish_ansi(self, theme: AsciiColorTheme, mode: AsciiColorMode) -> String {
        if self.width == 0 || self.height == 0 {
            return String::new();
        }

        let mut out = String::new();
        for row_start in (0..self.cells.len()).step_by(self.width) {
            let row_end = row_start + self.width;
            let mut active_color = None;
            for index in row_start..row_end {
                let desired_color = self.roles[index].map(|role| theme.color_for(role));
                if desired_color != active_color {
                    if active_color.is_some() {
                        out.push_str("\u{1b}[0m");
                    }
                    if let Some(color) = desired_color {
                        push_ansi_start(&mut out, mode, color);
                    }
                    active_color = desired_color;
                }
                out.push(self.cells[index]);
            }
            if active_color.is_some() {
                out.push_str("\u{1b}[0m");
            }
            out.push('\n');
        }
        out
    }

    fn finish_html(self, theme: AsciiColorTheme) -> String {
        if self.width == 0 || self.height == 0 {
            return String::new();
        }

        let mut out = String::new();
        for row_start in (0..self.cells.len()).step_by(self.width) {
            let row_end = row_start + self.width;
            let mut active_color = None;
            for index in row_start..row_end {
                let desired_color = self.roles[index].map(|role| theme.color_for(role));
                if desired_color != active_color {
                    if active_color.is_some() {
                        out.push_str("</span>");
                    }
                    if let Some(color) = desired_color {
                        push_html_span_start(&mut out, color);
                    }
                    active_color = desired_color;
                }
                push_html_escaped_char(&mut out, self.cells[index]);
            }
            if active_color.is_some() {
                out.push_str("</span>");
            }
            out.push('\n');
        }
        out
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

fn push_ansi_start(out: &mut String, mode: AsciiColorMode, color: AsciiRgb) {
    match mode {
        AsciiColorMode::Ansi16 => out.push_str(ansi16_start(color)),
        AsciiColorMode::Ansi256 => {
            let _ = write!(out, "\u{1b}[38;5;{}m", ansi256_index(color));
        }
        AsciiColorMode::TrueColor => {
            let _ = write!(out, "\u{1b}[38;2;{};{};{}m", color.r, color.g, color.b);
        }
        AsciiColorMode::Plain | AsciiColorMode::Auto | AsciiColorMode::Html => {}
    }
}

fn ansi256_index(color: AsciiRgb) -> u16 {
    let r = color.r as u16 * 5 / 255;
    let g = color.g as u16 * 5 / 255;
    let b = color.b as u16 * 5 / 255;
    16 + 36 * r + 6 * g + b
}

fn ansi16_start(color: AsciiRgb) -> &'static str {
    const PALETTE: [(AsciiRgb, &str); 16] = [
        (AsciiRgb::new(0x00, 0x00, 0x00), "\u{1b}[30m"),
        (AsciiRgb::new(0x80, 0x00, 0x00), "\u{1b}[31m"),
        (AsciiRgb::new(0x00, 0x80, 0x00), "\u{1b}[32m"),
        (AsciiRgb::new(0x80, 0x80, 0x00), "\u{1b}[33m"),
        (AsciiRgb::new(0x00, 0x00, 0x80), "\u{1b}[34m"),
        (AsciiRgb::new(0x80, 0x00, 0x80), "\u{1b}[35m"),
        (AsciiRgb::new(0x00, 0x80, 0x80), "\u{1b}[36m"),
        (AsciiRgb::new(0xc0, 0xc0, 0xc0), "\u{1b}[37m"),
        (AsciiRgb::new(0x80, 0x80, 0x80), "\u{1b}[90m"),
        (AsciiRgb::new(0xff, 0x00, 0x00), "\u{1b}[91m"),
        (AsciiRgb::new(0x00, 0xff, 0x00), "\u{1b}[92m"),
        (AsciiRgb::new(0xff, 0xff, 0x00), "\u{1b}[93m"),
        (AsciiRgb::new(0x00, 0x00, 0xff), "\u{1b}[94m"),
        (AsciiRgb::new(0xff, 0x00, 0xff), "\u{1b}[95m"),
        (AsciiRgb::new(0x00, 0xff, 0xff), "\u{1b}[96m"),
        (AsciiRgb::new(0xff, 0xff, 0xff), "\u{1b}[97m"),
    ];

    PALETTE
        .iter()
        .min_by_key(|(candidate, _)| color_distance(*candidate, color))
        .map(|(_, code)| *code)
        .unwrap_or("\u{1b}[37m")
}

fn color_distance(a: AsciiRgb, b: AsciiRgb) -> u32 {
    let dr = a.r as i32 - b.r as i32;
    let dg = a.g as i32 - b.g as i32;
    let db = a.b as i32 - b.b as i32;
    (dr * dr + dg * dg + db * db) as u32
}

fn push_html_span_start(out: &mut String, color: AsciiRgb) {
    let _ = write!(
        out,
        "<span style=\"color:#{:02x}{:02x}{:02x}\">",
        color.r, color.g, color.b
    );
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
