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
