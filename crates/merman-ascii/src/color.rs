#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AsciiColorMode {
    #[default]
    Plain,
    Auto,
    Ansi16,
    Ansi256,
    TrueColor,
    Html,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsciiRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl AsciiRgb {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub const fn from_hex24(rgb: u32) -> Self {
        Self {
            r: ((rgb >> 16) & 0xff) as u8,
            g: ((rgb >> 8) & 0xff) as u8,
            b: (rgb & 0xff) as u8,
        }
    }

    pub fn parse_css(value: &str) -> Option<Self> {
        crate::style_color::parse_css_color(value)
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AsciiColorRole {
    Text,
    MutedText,
    NodeBorder,
    GroupBorder,
    EdgeLine,
    EdgeArrow,
    EdgeLabel,
    Junction,
    SequenceLifeline,
    SequenceActivation,
    SequenceFrame,
    ChartAxis,
    ChartSeries(usize),
}

const CHART_SERIES_COLORS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AsciiTerminalPalette {
    pub foreground: AsciiRgb,
    pub background: AsciiRgb,
    pub line: Option<AsciiRgb>,
    pub accent: Option<AsciiRgb>,
    pub muted: Option<AsciiRgb>,
    pub surface: Option<AsciiRgb>,
    pub border: Option<AsciiRgb>,
}

impl AsciiTerminalPalette {
    pub const fn new(foreground: AsciiRgb, background: AsciiRgb) -> Self {
        Self {
            foreground,
            background,
            line: None,
            accent: None,
            muted: None,
            surface: None,
            border: None,
        }
    }

    pub const fn with_line(mut self, color: AsciiRgb) -> Self {
        self.line = Some(color);
        self
    }

    pub const fn with_accent(mut self, color: AsciiRgb) -> Self {
        self.accent = Some(color);
        self
    }

    pub const fn with_muted(mut self, color: AsciiRgb) -> Self {
        self.muted = Some(color);
        self
    }

    pub const fn with_surface(mut self, color: AsciiRgb) -> Self {
        self.surface = Some(color);
        self
    }

    pub const fn with_border(mut self, color: AsciiRgb) -> Self {
        self.border = Some(color);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AsciiColorTheme {
    text: AsciiRgb,
    muted_text: AsciiRgb,
    node_border: AsciiRgb,
    group_border: AsciiRgb,
    edge_line: AsciiRgb,
    edge_arrow: AsciiRgb,
    edge_label: AsciiRgb,
    junction: AsciiRgb,
    sequence_lifeline: AsciiRgb,
    sequence_activation: AsciiRgb,
    sequence_frame: AsciiRgb,
    chart_axis: AsciiRgb,
    chart_series: [AsciiRgb; CHART_SERIES_COLORS],
}

impl Default for AsciiColorTheme {
    fn default() -> Self {
        Self::default_light()
    }
}

impl AsciiColorTheme {
    pub const fn default_light() -> Self {
        Self {
            text: AsciiRgb::from_hex24(0x27272a),
            muted_text: AsciiRgb::from_hex24(0x71717a),
            node_border: AsciiRgb::from_hex24(0xa1a1aa),
            group_border: AsciiRgb::from_hex24(0xa1a1aa),
            edge_line: AsciiRgb::from_hex24(0x71717a),
            edge_arrow: AsciiRgb::from_hex24(0x52525b),
            edge_label: AsciiRgb::from_hex24(0x27272a),
            junction: AsciiRgb::from_hex24(0x71717a),
            sequence_lifeline: AsciiRgb::from_hex24(0x71717a),
            sequence_activation: AsciiRgb::from_hex24(0x2563eb),
            sequence_frame: AsciiRgb::from_hex24(0xa1a1aa),
            chart_axis: AsciiRgb::from_hex24(0x71717a),
            chart_series: [
                AsciiRgb::from_hex24(0x2563eb),
                AsciiRgb::from_hex24(0x16a34a),
                AsciiRgb::from_hex24(0xdc2626),
                AsciiRgb::from_hex24(0x9333ea),
                AsciiRgb::from_hex24(0xea580c),
                AsciiRgb::from_hex24(0x0891b2),
                AsciiRgb::from_hex24(0x4f46e5),
                AsciiRgb::from_hex24(0xbe123c),
            ],
        }
    }

    pub const fn default_dark() -> Self {
        Self {
            text: AsciiRgb::from_hex24(0xe4e4e7),
            muted_text: AsciiRgb::from_hex24(0xa1a1aa),
            node_border: AsciiRgb::from_hex24(0x71717a),
            group_border: AsciiRgb::from_hex24(0x71717a),
            edge_line: AsciiRgb::from_hex24(0xa1a1aa),
            edge_arrow: AsciiRgb::from_hex24(0xd4d4d8),
            edge_label: AsciiRgb::from_hex24(0xe4e4e7),
            junction: AsciiRgb::from_hex24(0xa1a1aa),
            sequence_lifeline: AsciiRgb::from_hex24(0xa1a1aa),
            sequence_activation: AsciiRgb::from_hex24(0x60a5fa),
            sequence_frame: AsciiRgb::from_hex24(0x71717a),
            chart_axis: AsciiRgb::from_hex24(0xa1a1aa),
            chart_series: [
                AsciiRgb::from_hex24(0x60a5fa),
                AsciiRgb::from_hex24(0x4ade80),
                AsciiRgb::from_hex24(0xf87171),
                AsciiRgb::from_hex24(0xc084fc),
                AsciiRgb::from_hex24(0xfb923c),
                AsciiRgb::from_hex24(0x22d3ee),
                AsciiRgb::from_hex24(0x818cf8),
                AsciiRgb::from_hex24(0xfb7185),
            ],
        }
    }

    pub const fn from_terminal_palette(palette: AsciiTerminalPalette) -> Self {
        let surface = match palette.surface {
            Some(color) => color,
            None => palette.background,
        };
        let muted = match palette.muted {
            Some(color) => color,
            None => mix_rgb(palette.foreground, palette.background, 55),
        };
        let line = match palette.line {
            Some(color) => color,
            None => mix_rgb(palette.foreground, palette.background, 60),
        };
        let border = match palette.border {
            Some(color) => color,
            None => mix_rgb(palette.foreground, surface, 45),
        };
        let accent = match palette.accent {
            Some(color) => color,
            None => line,
        };
        let accent_soft = mix_rgb(accent, palette.background, 75);
        let accent_muted = mix_rgb(accent, palette.background, 55);
        let line_soft = mix_rgb(line, palette.background, 75);

        Self {
            text: palette.foreground,
            muted_text: muted,
            node_border: border,
            group_border: border,
            edge_line: line,
            edge_arrow: accent,
            edge_label: palette.foreground,
            junction: border,
            sequence_lifeline: line,
            sequence_activation: accent,
            sequence_frame: border,
            chart_axis: line,
            chart_series: [
                accent,
                line,
                accent_soft,
                line_soft,
                accent_muted,
                mix_rgb(line, palette.background, 55),
                mix_rgb(accent, line, 50),
                mix_rgb(palette.foreground, accent, 50),
            ],
        }
    }

    pub fn color_for(&self, role: AsciiColorRole) -> AsciiRgb {
        match role {
            AsciiColorRole::Text => self.text,
            AsciiColorRole::MutedText => self.muted_text,
            AsciiColorRole::NodeBorder => self.node_border,
            AsciiColorRole::GroupBorder => self.group_border,
            AsciiColorRole::EdgeLine => self.edge_line,
            AsciiColorRole::EdgeArrow => self.edge_arrow,
            AsciiColorRole::EdgeLabel => self.edge_label,
            AsciiColorRole::Junction => self.junction,
            AsciiColorRole::SequenceLifeline => self.sequence_lifeline,
            AsciiColorRole::SequenceActivation => self.sequence_activation,
            AsciiColorRole::SequenceFrame => self.sequence_frame,
            AsciiColorRole::ChartAxis => self.chart_axis,
            AsciiColorRole::ChartSeries(index) => self.chart_series[index % CHART_SERIES_COLORS],
        }
    }

    pub fn with_role(mut self, role: AsciiColorRole, color: AsciiRgb) -> Self {
        match role {
            AsciiColorRole::Text => self.text = color,
            AsciiColorRole::MutedText => self.muted_text = color,
            AsciiColorRole::NodeBorder => self.node_border = color,
            AsciiColorRole::GroupBorder => self.group_border = color,
            AsciiColorRole::EdgeLine => self.edge_line = color,
            AsciiColorRole::EdgeArrow => self.edge_arrow = color,
            AsciiColorRole::EdgeLabel => self.edge_label = color,
            AsciiColorRole::Junction => self.junction = color,
            AsciiColorRole::SequenceLifeline => self.sequence_lifeline = color,
            AsciiColorRole::SequenceActivation => self.sequence_activation = color,
            AsciiColorRole::SequenceFrame => self.sequence_frame = color,
            AsciiColorRole::ChartAxis => self.chart_axis = color,
            AsciiColorRole::ChartSeries(index) => {
                self.chart_series[index % CHART_SERIES_COLORS] = color;
            }
        }
        self
    }
}

const fn mix_rgb(foreground: AsciiRgb, background: AsciiRgb, foreground_percent: u8) -> AsciiRgb {
    let fg = foreground_percent as u16;
    let bg = 100 - fg;
    AsciiRgb::new(
        mix_channel(foreground.r, background.r, fg, bg),
        mix_channel(foreground.g, background.g, fg, bg),
        mix_channel(foreground.b, background.b, fg, bg),
    )
}

const fn mix_channel(foreground: u8, background: u8, fg: u16, bg: u16) -> u8 {
    (((foreground as u16 * fg) + (background as u16 * bg) + 50) / 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_palette_derives_stable_roles_from_two_colors() {
        let theme = AsciiColorTheme::from_terminal_palette(AsciiTerminalPalette::new(
            AsciiRgb::from_hex24(0x000000),
            AsciiRgb::from_hex24(0xffffff),
        ));

        assert_eq!(
            theme.color_for(AsciiColorRole::Text),
            AsciiRgb::from_hex24(0x000000)
        );
        assert_eq!(
            theme.color_for(AsciiColorRole::MutedText),
            AsciiRgb::from_hex24(0x737373)
        );
        assert_eq!(
            theme.color_for(AsciiColorRole::NodeBorder),
            AsciiRgb::from_hex24(0x8c8c8c)
        );
        assert_eq!(
            theme.color_for(AsciiColorRole::EdgeLine),
            AsciiRgb::from_hex24(0x666666)
        );
        assert_eq!(
            theme.color_for(AsciiColorRole::EdgeArrow),
            AsciiRgb::from_hex24(0x666666)
        );
        assert_eq!(
            theme.color_for(AsciiColorRole::ChartSeries(0)),
            AsciiRgb::from_hex24(0x666666)
        );
    }

    #[test]
    fn terminal_palette_enrichment_changes_accent_and_preserves_explicit_overrides() {
        let theme = AsciiColorTheme::from_terminal_palette(
            AsciiTerminalPalette::new(
                AsciiRgb::from_hex24(0x101010),
                AsciiRgb::from_hex24(0xf0f0f0),
            )
            .with_line(AsciiRgb::from_hex24(0x202020))
            .with_accent(AsciiRgb::from_hex24(0x3366ff))
            .with_muted(AsciiRgb::from_hex24(0x777777))
            .with_surface(AsciiRgb::from_hex24(0xe0e0e0))
            .with_border(AsciiRgb::from_hex24(0x999999)),
        )
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::from_hex24(0xff0000));

        assert_eq!(
            theme.color_for(AsciiColorRole::Text),
            AsciiRgb::from_hex24(0x101010)
        );
        assert_eq!(
            theme.color_for(AsciiColorRole::MutedText),
            AsciiRgb::from_hex24(0x777777)
        );
        assert_eq!(
            theme.color_for(AsciiColorRole::NodeBorder),
            AsciiRgb::from_hex24(0x999999)
        );
        assert_eq!(
            theme.color_for(AsciiColorRole::EdgeLine),
            AsciiRgb::from_hex24(0x202020)
        );
        assert_eq!(
            theme.color_for(AsciiColorRole::EdgeArrow),
            AsciiRgb::from_hex24(0xff0000)
        );
        assert_eq!(
            theme.color_for(AsciiColorRole::ChartSeries(0)),
            AsciiRgb::from_hex24(0x3366ff)
        );
    }

    #[test]
    fn rgb_parses_css_colors_for_public_theme_inputs() {
        assert_eq!(
            AsciiRgb::parse_css("#123456"),
            Some(AsciiRgb::from_hex24(0x123456))
        );
        assert_eq!(
            AsciiRgb::parse_css("rgb(1, 2, 3)"),
            Some(AsciiRgb::new(1, 2, 3))
        );
        assert_eq!(AsciiRgb::parse_css("transparent"), None);
    }
}
