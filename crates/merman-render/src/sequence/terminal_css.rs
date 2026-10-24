use super::{
    SequencePreparedTerminalTheme, SequenceTerminalForegroundProvenance, SequenceTextSurface,
    SequenceTypographyPlan,
};
use crate::diagram_theme::ThemeTypographyProperty;

#[derive(Debug, Clone)]
pub(crate) struct SequencePreparedTextCssGroup {
    pub(crate) declarations: String,
    pub(crate) surfaces: Vec<SequenceTextSurface>,
}

/// Sequence CSS values that require formatting or grouping during preparation.
#[derive(Debug, Clone)]
pub(crate) struct SequencePreparedCss {
    pub(crate) typed_font: bool,
    pub(crate) font_size_override: Option<String>,
    pub(crate) lifeline_width: Option<String>,
    pub(crate) message_width_override: Option<String>,
    pub(crate) actor_text_fill: String,
    pub(crate) message_text_fill: String,
    pub(crate) note_text_fill: String,
    pub(crate) label_text_fill: String,
    pub(crate) loop_text_fill: String,
    pub(crate) section_title_fill: String,
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
        let baseline_declaration = |surface| {
            let terminal = typography.terminal_text_style(surface);
            match terminal.foreground_provenance {
                SequenceTerminalForegroundProvenance::TypedTheme => String::new(),
                SequenceTerminalForegroundProvenance::PresentationConfig => {
                    format!("fill:{};", terminal.foreground)
                }
            }
        };
        let actor_text_fill = baseline_declaration(SequenceTextSurface::ParticipantLabel);
        let message_text_fill = baseline_declaration(SequenceTextSurface::MessageLabel);
        let note_text_fill = baseline_declaration(SequenceTextSurface::NoteLabel);
        let label_text_fill = baseline_declaration(SequenceTextSurface::ControlKeyword);
        let loop_text_fill = baseline_declaration(SequenceTextSurface::ControlPrimaryTitle);
        let section_title_fill = baseline_declaration(SequenceTextSurface::ControlSectionTitle);
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
                if declarations.is_empty() {
                    continue;
                }
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
            text_groups.extend(groups);
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
            actor_text_fill,
            message_text_fill,
            note_text_fill,
            label_text_fill,
            loop_text_fill,
            section_title_fill,
            text_groups,
        }
    }
}
