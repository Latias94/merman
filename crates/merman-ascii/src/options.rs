use crate::color::{AsciiColorMode, AsciiColorTheme};
use crate::error::{AsciiError, Result};

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AsciiCharset {
    #[default]
    Unicode,
    Ascii,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AsciiDirection {
    #[default]
    LeftRight,
    TopDown,
}

/// Terminal display-width convention used by every text measurement and placement operation.
///
/// `Unicode` follows the non-CJK width table exposed by the pinned `unicode-width` dependency.
/// `Cjk` additionally treats East Asian ambiguous characters as wide. Hosts should select the
/// profile that matches the terminal in which the rendered text will be displayed.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerminalWidthProfile {
    #[default]
    Unicode,
    Cjk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AsciiRenderOptions {
    pub charset: AsciiCharset,
    pub terminal_width_profile: TerminalWidthProfile,
    pub default_direction: AsciiDirection,
    pub color_mode: AsciiColorMode,
    pub color_theme: AsciiColorTheme,
    pub box_border_padding: usize,
    pub graph_padding_x: usize,
    pub graph_padding_y: usize,
    pub flowchart_node_label_wrap_width: usize,
    pub sequence_participant_spacing: usize,
    pub sequence_message_spacing: usize,
    pub sequence_self_message_width: usize,
    pub sequence_mirror_actors: bool,
    pub xychart_vertical_plot_height: usize,
    pub xychart_category_band_width: usize,
    pub xychart_horizontal_plot_width: usize,
    pub relation_summary_diagnostics: bool,
}

impl Default for AsciiRenderOptions {
    fn default() -> Self {
        Self {
            charset: AsciiCharset::Unicode,
            terminal_width_profile: TerminalWidthProfile::Unicode,
            default_direction: AsciiDirection::LeftRight,
            color_mode: AsciiColorMode::Plain,
            color_theme: AsciiColorTheme::default_light(),
            box_border_padding: 1,
            graph_padding_x: 5,
            graph_padding_y: 5,
            flowchart_node_label_wrap_width: 40,
            sequence_participant_spacing: 5,
            sequence_message_spacing: 1,
            sequence_self_message_width: 4,
            sequence_mirror_actors: false,
            xychart_vertical_plot_height: 5,
            xychart_category_band_width: 3,
            xychart_horizontal_plot_width: 10,
            relation_summary_diagnostics: false,
        }
    }
}

impl AsciiRenderOptions {
    pub fn ascii() -> Self {
        Self {
            charset: AsciiCharset::Ascii,
            ..Self::default()
        }
    }

    pub fn unicode() -> Self {
        Self::default()
    }

    pub fn with_color_mode(mut self, color_mode: AsciiColorMode) -> Self {
        self.color_mode = color_mode;
        self
    }

    pub fn with_terminal_width_profile(mut self, profile: TerminalWidthProfile) -> Self {
        self.terminal_width_profile = profile;
        self
    }

    pub fn with_color_theme(mut self, color_theme: AsciiColorTheme) -> Self {
        self.color_theme = color_theme;
        self
    }

    pub fn with_sequence_mirror_actors(mut self, mirror_actors: bool) -> Self {
        self.sequence_mirror_actors = mirror_actors;
        self
    }

    pub fn with_flowchart_node_label_wrap_width(mut self, width: usize) -> Self {
        self.flowchart_node_label_wrap_width = width;
        self
    }

    pub fn with_xychart_vertical_plot_height(mut self, height: usize) -> Self {
        self.xychart_vertical_plot_height = height;
        self
    }

    pub fn with_xychart_category_band_width(mut self, width: usize) -> Self {
        self.xychart_category_band_width = width;
        self
    }

    pub fn with_xychart_horizontal_plot_width(mut self, width: usize) -> Self {
        self.xychart_horizontal_plot_width = width;
        self
    }

    pub fn with_relation_summary_diagnostics(mut self, enabled: bool) -> Self {
        self.relation_summary_diagnostics = enabled;
        self
    }

    /// Returns the structural glyph set that can preserve one-cell grid topology.
    ///
    /// Unicode box-drawing and marker characters are East Asian Ambiguous. The CJK width table
    /// assigns them two cells, while every current planner models a structural token as one cell.
    /// Falling back to ASCII structure keeps authored CJK/ambiguous text profile-correct without
    /// corrupting borders, routes, or alignment.
    pub(crate) fn structural_charset(&self) -> AsciiCharset {
        match self.terminal_width_profile {
            TerminalWidthProfile::Unicode => self.charset,
            TerminalWidthProfile::Cjk => AsciiCharset::Ascii,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.flowchart_node_label_wrap_width == 0 {
            return Err(AsciiError::InvalidOption {
                field: "flowchart_node_label_wrap_width",
                message: "must be greater than 0",
            });
        }

        if self.sequence_self_message_width < 2 {
            return Err(AsciiError::InvalidOption {
                field: "sequence_self_message_width",
                message: "must be at least 2",
            });
        }

        if self.xychart_vertical_plot_height < 2 {
            return Err(AsciiError::InvalidOption {
                field: "xychart_vertical_plot_height",
                message: "must be at least 2",
            });
        }

        if self.xychart_category_band_width == 0 {
            return Err(AsciiError::InvalidOption {
                field: "xychart_category_band_width",
                message: "must be greater than 0",
            });
        }

        if self.xychart_horizontal_plot_width < 2 {
            return Err(AsciiError::InvalidOption {
                field: "xychart_horizontal_plot_width",
                message: "must be at least 2",
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cjk_profile_uses_single_cell_ascii_structure() {
        let options =
            AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk);

        assert_eq!(options.structural_charset(), AsciiCharset::Ascii);
    }
}
