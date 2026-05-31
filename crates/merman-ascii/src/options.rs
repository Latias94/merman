use crate::color::{AsciiColorMode, AsciiColorTheme};
use crate::error::{AsciiError, Result};

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiCharset {
    Unicode,
    Ascii,
}

impl Default for AsciiCharset {
    fn default() -> Self {
        Self::Unicode
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiDirection {
    LeftRight,
    TopDown,
}

impl Default for AsciiDirection {
    fn default() -> Self {
        Self::LeftRight
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AsciiRenderOptions {
    pub charset: AsciiCharset,
    pub fallback_direction: AsciiDirection,
    pub color_mode: AsciiColorMode,
    pub color_theme: AsciiColorTheme,
    pub box_border_padding: usize,
    pub graph_padding_x: usize,
    pub graph_padding_y: usize,
    pub sequence_participant_spacing: usize,
    pub sequence_message_spacing: usize,
    pub sequence_self_message_width: usize,
    pub max_grid_cells: usize,
}

impl Default for AsciiRenderOptions {
    fn default() -> Self {
        Self {
            charset: AsciiCharset::Unicode,
            fallback_direction: AsciiDirection::LeftRight,
            color_mode: AsciiColorMode::Plain,
            color_theme: AsciiColorTheme::default_light(),
            box_border_padding: 1,
            graph_padding_x: 5,
            graph_padding_y: 5,
            sequence_participant_spacing: 5,
            sequence_message_spacing: 1,
            sequence_self_message_width: 4,
            max_grid_cells: 250_000,
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

    pub fn validate(&self) -> Result<()> {
        if self.sequence_self_message_width < 2 {
            return Err(AsciiError::InvalidOption {
                field: "sequence_self_message_width",
                message: "must be at least 2",
            });
        }

        if self.max_grid_cells == 0 {
            return Err(AsciiError::InvalidOption {
                field: "max_grid_cells",
                message: "must be greater than 0",
            });
        }

        Ok(())
    }
}
