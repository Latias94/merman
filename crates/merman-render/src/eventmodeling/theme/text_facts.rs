use crate::diagram_theme::{ThemeTypographyProperty, ThemeVariant};
use crate::resources::OperationWorkMeter;
use crate::text::VisibleTextStyleFacts;

/// DOM ordinals stay stable when a prepared terminal has no visible glyphs.
#[derive(Debug, Default)]
pub(super) struct EventModelingTextFacts {
    pub(super) swimlanes: Vec<VisibleTextStyleFacts>,
    pub(super) boxes: Vec<VisibleTextStyleFacts>,
}

impl EventModelingTextFacts {
    pub(super) fn from_prepared(
        layout: &crate::model::EventModelingDiagramLayout,
        labels: &super::super::EventModelingPreparedLabels,
        font_size_css: Option<&str>,
        work: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let size = font_size_css.map_or(Some(16.0), |size| {
            crate::text::visible_text_font_size_after_style(
                Some(16.0),
                &format!("font-size:{size}"),
            )
        });
        let mut facts = Self::default();
        for lane in &layout.swimlanes {
            work.charge(1)?;
            if !lane.label.trim().is_empty() {
                facts
                    .swimlanes
                    .push(VisibleTextStyleFacts::plain_text_at_font_size(
                        &lane.label,
                        size,
                    ));
            }
        }
        for box_layout in &layout.boxes {
            work.charge(1)?;
            if !box_layout.text.trim().is_empty() {
                let fragment = labels.box_xhtml(box_layout.index)?;
                work.charge(fragment.len().div_ceil(256))?;
                facts.boxes.push(
                    VisibleTextStyleFacts::from_eventmodeling_xhtml_fragment_at_font_size(
                        fragment,
                        size,
                        font_size_css,
                    ),
                );
            }
        }
        Ok(facts)
    }

    pub(super) fn all(&self) -> impl Iterator<Item = &VisibleTextStyleFacts> {
        self.swimlanes.iter().chain(&self.boxes)
    }

    pub(super) fn visible_ordinals<'facts>(
        facts: impl Iterator<Item = &'facts VisibleTextStyleFacts>,
    ) -> Vec<(usize, ThemeVariant)> {
        facts
            .enumerate()
            .filter(|(_, facts)| visible(facts))
            .map(|(index, _)| (index + 1, ThemeVariant::Default))
            .collect()
    }

    pub(super) fn typography(&self, property: ThemeTypographyProperty) -> (bool, bool) {
        let mut inherited = false;
        let mut unknown = false;
        for facts in self.all().filter(|facts| visible(facts)) {
            unknown |= !facts.parse_valid() || !facts.font_visibility_verified();
            match property {
                ThemeTypographyProperty::FontStack => {
                    inherited |= facts.inherited_font_family_run_count() != 0;
                    unknown |= facts.unknown_font_family_run_count() != 0;
                }
                ThemeTypographyProperty::FontSize => {
                    inherited |= facts.inherited_font_size_run_count() != 0;
                }
                _ => unreachable!("only inherited font family and size have typed consumers"),
            }
        }
        (inherited, unknown)
    }
}

pub(super) fn visible(facts: &VisibleTextStyleFacts) -> bool {
    !facts.parse_valid() || facts.has_visible_runs()
}

pub(super) fn source_owns_fill(facts: &VisibleTextStyleFacts) -> bool {
    facts.parse_valid()
        && (!facts.has_visible_runs()
            || (facts.inherited_color_run_count() == 0
                && facts.unverified_portable_color_run_count() == 0))
}

pub(super) fn unverified_fill(facts: &VisibleTextStyleFacts) -> bool {
    !facts.parse_valid()
        || !facts.font_visibility_verified()
        || facts.unverified_portable_color_run_count() != 0
}
