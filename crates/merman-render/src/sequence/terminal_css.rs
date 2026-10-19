use super::{SequencePreparedTerminalTheme, SequenceTextSurface, SequenceTypographyPlan};
use crate::diagram_theme::ThemeTypographyProperty;

#[derive(Debug, Clone)]
pub(crate) struct SequencePreparedTextCssGroup {
    pub(crate) declarations: String,
    pub(crate) surfaces: Vec<SequenceTextSurface>,
}

#[derive(Debug, Clone)]
pub(crate) struct SequencePreparedTextCss {
    pub(crate) fill: String,
    pub(crate) typed_fill: bool,
    pub(crate) baseline_declaration: String,
}

/// Sequence CSS values that require formatting or grouping during preparation.
#[derive(Debug, Clone)]
pub(crate) struct SequencePreparedCss {
    pub(crate) typed_font: bool,
    pub(crate) font_size_override: Option<String>,
    pub(crate) lifeline_width: Option<String>,
    pub(crate) message_width_override: Option<String>,
    pub(crate) text: [SequencePreparedTextCss; SequenceTextSurface::COUNT],
    pub(crate) text_groups: Vec<SequencePreparedTextCssGroup>,
}

impl SequencePreparedCss {
    pub(super) fn resolve(
        typography: &SequenceTypographyPlan,
        terminal: &SequencePreparedTerminalTheme,
    ) -> Self {
        let typed_font = typography
            .base_typed_properties()
            .contains(&ThemeTypographyProperty::FontStack);
        let font_size_override = typography
            .base_typed_properties()
            .contains(&ThemeTypographyProperty::FontSize)
            .then(|| {
                format!(
                    "{}px",
                    crate::number_format::canonical_number(typography.base_font_size_px())
                )
            });
        let text = SequenceTextSurface::ALL.map(|surface| {
            let role = typography.role(surface.role());
            let typed_fill = role.typed_fill_for(surface).is_some();
            let fill = typography
                .terminal_text_style(surface)
                .foreground
                .to_owned();
            SequencePreparedTextCss {
                baseline_declaration: if typed_fill {
                    String::new()
                } else {
                    format!("fill:{fill};")
                },
                fill,
                typed_fill,
            }
        });
        let mut text_groups = Vec::new();
        for surfaces in [
            &[
                SequenceTextSurface::ParticipantLabel,
                SequenceTextSurface::BoxTitle,
            ][..],
            &[SequenceTextSurface::MessageLabel][..],
            &[SequenceTextSurface::NoteLabel][..],
            &[
                SequenceTextSurface::ControlPrimaryTitle,
                SequenceTextSurface::ControlSectionTitle,
                SequenceTextSurface::ControlKeyword,
            ][..],
        ] {
            let mut groups: Vec<SequencePreparedTextCssGroup> = Vec::new();
            for &surface in surfaces {
                let declarations = typography
                    .role(surface.role())
                    .css_declarations_for(surface)
                    .unwrap_or_default();
                if let Some(group) = groups
                    .iter_mut()
                    .find(|group| group.declarations == declarations)
                {
                    group.surfaces.push(surface);
                } else {
                    groups.push(SequencePreparedTextCssGroup {
                        declarations,
                        surfaces: vec![surface],
                    });
                }
            }
            text_groups.extend(
                groups
                    .into_iter()
                    .filter(|group| !group.declarations.is_empty()),
            );
        }
        Self {
            typed_font,
            font_size_override,
            lifeline_width: terminal
                .lifeline
                .typed_stroke_width
                .map(|width| crate::number_format::canonical_number(f64::from(width)).to_string()),
            message_width_override: terminal.message.typed_stroke_width.map(|width| {
                format!(
                    "{}px",
                    crate::number_format::canonical_number(f64::from(width))
                )
            }),
            text,
            text_groups,
        }
    }
}
