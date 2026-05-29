use crate::error::{AsciiError, Result};
use std::borrow::Cow;

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
pub struct AsciiRenderOptions {
    pub charset: AsciiCharset,
    pub fallback_direction: AsciiDirection,
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

    /// Applies leading `mermaid-ascii` source directives such as `paddingX=2` and `paddingY=1`.
    ///
    /// These directives are not Mermaid syntax. They are accepted for compatibility with copied
    /// `mermaid-ascii` fixtures and CLI-style input before the Mermaid graph declaration.
    pub fn apply_mermaid_ascii_directives<'a>(&self, source: &'a str) -> (Self, Cow<'a, str>) {
        let mut options = *self;
        let mut changed = false;
        let mut output = String::new();
        let mut before_diagram = true;

        for line in source.lines() {
            let trimmed = line.trim();
            if before_diagram {
                if let Some((axis, value)) = parse_padding_directive(trimmed) {
                    match axis {
                        PaddingAxis::X => options.graph_padding_x = value,
                        PaddingAxis::Y => options.graph_padding_y = value,
                    }
                    changed = true;
                    continue;
                }
                if is_diagram_header(trimmed) {
                    before_diagram = false;
                }
            }
            output.push_str(line);
            output.push('\n');
        }

        if changed {
            (options, Cow::Owned(output))
        } else {
            (options, Cow::Borrowed(source))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaddingAxis {
    X,
    Y,
}

fn parse_padding_directive(line: &str) -> Option<(PaddingAxis, usize)> {
    let (key, value) = line.split_once('=')?;
    let axis = if key.trim().eq_ignore_ascii_case("paddingX") {
        PaddingAxis::X
    } else if key.trim().eq_ignore_ascii_case("paddingY") {
        PaddingAxis::Y
    } else {
        return None;
    };
    let value = value.trim().parse().ok()?;
    Some((axis, value))
}

fn is_diagram_header(line: &str) -> bool {
    line.starts_with("graph ") || line.starts_with("flowchart ")
}
