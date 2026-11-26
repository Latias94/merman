use super::{SequenceTextSurface, SequenceTypographyPlan};

#[derive(Debug, Clone)]
pub struct SequencePreparedTextCssGroup {
    pub declarations: String,
    pub surfaces: Vec<SequenceTextSurface>,
}

/// Sequence CSS values that require formatting or grouping during preparation.
#[derive(Debug, Clone)]
pub struct SequencePreparedCss {
    pub actor_text_fill: String,
    pub message_text_fill: String,
    pub note_text_fill: String,
    pub label_text_fill: String,
    pub loop_text_fill: String,
    pub section_title_fill: String,
    pub text_groups: Vec<SequencePreparedTextCssGroup>,
}

impl SequencePreparedCss {
    pub(super) fn resolve(typography: &SequenceTypographyPlan) -> Self {
        let baseline_declaration = |surface| {
            format!(
                "fill:{};",
                typography.compat_binding().text_surface_fill(surface)
            )
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
                    .css_declarations()
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
