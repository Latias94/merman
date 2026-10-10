use std::sync::Arc;

use super::{
    SequenceCompatBinding, SequencePreparedStaticRectTheme, SequencePreparedTerminalTheme,
    SequenceTextSurface, SequenceTypographyPlan,
};
use crate::diagram_theme::ThemeTypographyProperty;

#[derive(Debug, Clone)]
pub(crate) struct SequencePreparedTextCssGroup {
    pub(crate) declarations: String,
    pub(crate) surfaces: Vec<SequenceTextSurface>,
}

#[derive(Debug, Clone)]
pub(crate) struct SequencePreparedTextCss {
    pub(crate) baseline_fill: String,
    pub(crate) fill: String,
    pub(crate) typed_fill: bool,
    pub(crate) baseline_declaration: String,
}

/// Concrete Sequence declarations selected once, independent of scoped SVG identity.
#[derive(Debug, Clone)]
pub(crate) struct SequencePreparedCss {
    pub(crate) raw: Arc<SequenceCompatBinding>,
    pub(crate) font: String,
    pub(crate) root_font: String,
    pub(crate) font_size: String,
    pub(crate) actor_fill: String,
    pub(crate) actor_stroke: String,
    pub(crate) typed_actor_fill: bool,
    pub(crate) typed_actor_stroke: bool,
    pub(crate) lifeline_stroke: String,
    pub(crate) lifeline_width: Option<String>,
    pub(crate) message_stroke: String,
    pub(crate) message_width: String,
    pub(crate) number_fill: String,
    pub(crate) typed_number_fill: bool,
    pub(crate) keyword_fill: String,
    pub(crate) keyword_stroke: String,
    pub(crate) typed_keyword_fill: bool,
    pub(crate) typed_keyword_stroke: bool,
    pub(crate) frame_stroke: String,
    pub(crate) note_fill: String,
    pub(crate) note_stroke: String,
    pub(crate) activation_fill_override: Option<String>,
    pub(crate) activation_stroke_override: Option<String>,
    pub(crate) text: [SequencePreparedTextCss; SequenceTextSurface::COUNT],
    pub(crate) text_groups: Vec<SequencePreparedTextCssGroup>,
}

impl SequencePreparedCss {
    pub(super) fn resolve(
        typography: &SequenceTypographyPlan,
        terminal: &SequencePreparedTerminalTheme,
        activation: &SequencePreparedStaticRectTheme,
    ) -> Self {
        let raw = typography.shared_compat_binding();
        let typed_font = typography
            .base_typed_properties()
            .contains(&ThemeTypographyProperty::FontStack);
        let font = if typed_font {
            typography.base_font_family_css()
        } else {
            &raw.font_family
        }
        .to_owned();
        let root_font = if typed_font {
            typography.base_font_family_css()
        } else {
            &raw.root_font_family
        }
        .to_owned();
        let font_size = if typography
            .base_typed_properties()
            .contains(&ThemeTypographyProperty::FontSize)
        {
            format!(
                "{}px",
                crate::number_format::canonical_number(typography.base_font_size_px())
            )
        } else {
            raw.font_size_css.clone()
        };
        let baseline_fills = raw.text_surface_fills();
        let text = SequenceTextSurface::ALL.map(|surface| {
            let role = typography.role(surface.role());
            let typed_fill = role.typed_fill_for(surface).is_some();
            let fill = typography
                .terminal_text_style(surface)
                .foreground
                .to_owned();
            SequencePreparedTextCss {
                baseline_fill: baseline_fills[surface.index()].clone(),
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
            font,
            root_font,
            font_size,
            actor_fill: terminal
                .actor
                .typed_fill
                .clone()
                .unwrap_or_else(|| raw.actor_fill.clone()),
            actor_stroke: terminal
                .actor
                .typed_stroke
                .clone()
                .unwrap_or_else(|| raw.actor_border.clone()),
            typed_actor_fill: terminal.actor.typed_fill.is_some(),
            typed_actor_stroke: terminal.actor.typed_stroke.is_some(),
            lifeline_stroke: terminal
                .lifeline
                .typed_stroke
                .clone()
                .unwrap_or_else(|| raw.actor_line.clone()),
            lifeline_width: terminal
                .lifeline
                .typed_stroke_width
                .map(|width| crate::number_format::canonical_number(f64::from(width)).to_string()),
            message_stroke: terminal
                .message
                .typed_stroke
                .clone()
                .unwrap_or_else(|| raw.signal_color.clone()),
            message_width: terminal
                .message
                .typed_stroke_width
                .map(|width| {
                    format!(
                        "{}px",
                        crate::number_format::canonical_number(f64::from(width))
                    )
                })
                .unwrap_or_else(|| "1.5".to_owned()),
            number_fill: terminal
                .number
                .typed_fill()
                .unwrap_or(&raw.sequence_number)
                .to_owned(),
            typed_number_fill: terminal.number.typed_fill().is_some(),
            keyword_fill: terminal
                .keyword
                .typed_fill
                .clone()
                .unwrap_or_else(|| raw.label_box_fill.clone()),
            keyword_stroke: terminal
                .keyword
                .typed_stroke
                .clone()
                .unwrap_or_else(|| raw.label_box_border.clone()),
            typed_keyword_fill: terminal.keyword.typed_fill.is_some(),
            typed_keyword_stroke: terminal.keyword.typed_stroke.is_some(),
            frame_stroke: terminal
                .frame
                .typed_stroke
                .clone()
                .unwrap_or_else(|| raw.label_box_border.clone()),
            note_fill: terminal
                .note
                .typed_fill
                .clone()
                .unwrap_or_else(|| raw.note_fill.clone()),
            note_stroke: terminal
                .note
                .typed_stroke
                .clone()
                .unwrap_or_else(|| raw.note_border.clone()),
            activation_fill_override: activation.typed_fill.clone(),
            activation_stroke_override: activation.typed_stroke.clone(),
            raw,
            text,
            text_groups,
        }
    }
}
