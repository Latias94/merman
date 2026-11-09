use crate::text::VisibleTextStyleFacts;

#[derive(Debug)]
pub(crate) struct KanbanVisibleTextStyleFacts {
    ownership: VisibleTextStyleFacts,
}

impl std::ops::Deref for KanbanVisibleTextStyleFacts {
    type Target = VisibleTextStyleFacts;

    fn deref(&self) -> &Self::Target {
        &self.ownership
    }
}

impl KanbanVisibleTextStyleFacts {
    pub(super) const fn not_requested() -> Self {
        Self {
            ownership: VisibleTextStyleFacts::not_requested(),
        }
    }

    pub(crate) fn parse_valid(&self) -> bool {
        self.ownership.font_visibility_verified() && self.ownership.parse_valid()
    }

    pub(crate) fn inherited_color_run_count(&self) -> usize {
        if self.ownership.font_visibility_verified() {
            self.ownership.inherited_color_run_count()
        } else {
            0
        }
    }

    pub(crate) fn unverified_color_run_count(&self) -> usize {
        if self.ownership.font_visibility_verified() {
            self.ownership.unverified_color_run_count()
        } else {
            self.ownership.visible_run_count()
        }
    }
}

/// Keeps Kanban's receipt policy local while the shared collector owns XHTML visibility.
pub(super) fn visible_style_facts(
    fragment: &str,
    base_font_size: f64,
) -> KanbanVisibleTextStyleFacts {
    KanbanVisibleTextStyleFacts {
        ownership: VisibleTextStyleFacts::from_xhtml_fragment_at_font_size(
            fragment,
            Some(base_font_size),
        ),
    }
}
