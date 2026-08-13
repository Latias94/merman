use crate::color::{AsciiColorMode, AsciiColorTheme};
use crate::error::{AsciiError, Result};

/// Stable binding metadata for the terminal-grid budget owned by the ASCII renderer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AsciiResourceLimitDescriptor {
    pub stable_id: &'static str,
    pub phase: &'static str,
    pub description: &'static str,
    pub overridable: bool,
    pub minimum_value: usize,
}

pub const MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID: &str = "max_ascii_grid_cells";
pub const ASCII_RESOURCE_LIMIT_DESCRIPTORS: [AsciiResourceLimitDescriptor; 1] =
    [AsciiResourceLimitDescriptor {
        stable_id: MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID,
        phase: "ascii_layout",
        description: "Maximum terminal grid cells allocated by ASCII layout",
        overridable: true,
        minimum_value: 1,
    }];

/// Returns the profile value for an ASCII-owned resource limit.
pub fn ascii_resource_profile_value(
    profile: merman_core::resources::ResourceProfile,
    stable_id: &str,
) -> Option<usize> {
    if stable_id != MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID {
        return None;
    }
    match profile {
        merman_core::resources::ResourceProfile::Interactive => Some(250_000),
        merman_core::resources::ResourceProfile::Constrained => Some(125_000),
        merman_core::resources::ResourceProfile::TrustedNative => Some(1_000_000),
        merman_core::resources::ResourceProfile::UnboundedForTrustedInput => None,
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AsciiRenderOptions {
    pub charset: AsciiCharset,
    pub default_direction: AsciiDirection,
    pub color_mode: AsciiColorMode,
    pub color_theme: AsciiColorTheme,
    pub box_border_padding: usize,
    pub graph_padding_x: usize,
    pub graph_padding_y: usize,
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
            default_direction: AsciiDirection::LeftRight,
            color_mode: AsciiColorMode::Plain,
            color_theme: AsciiColorTheme::default_light(),
            box_border_padding: 1,
            graph_padding_x: 5,
            graph_padding_y: 5,
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

    pub fn with_color_theme(mut self, color_theme: AsciiColorTheme) -> Self {
        self.color_theme = color_theme;
        self
    }

    pub fn with_sequence_mirror_actors(mut self, mirror_actors: bool) -> Self {
        self.sequence_mirror_actors = mirror_actors;
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

    pub fn validate(&self) -> Result<()> {
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
