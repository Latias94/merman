use std::collections::BTreeSet;

use crate::text::VisibleTextStyleFacts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExpectedPaint {
    pub(super) rule_index: usize,
    pub(super) css: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct EntityExpectation {
    pub(super) fill: Option<ExpectedPaint>,
    pub(super) stroke: Option<ExpectedPaint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ErAttributeTextRole {
    Type,
    Name,
    Keys,
    Comment,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ErTextTerminalId {
    EntityName(Box<str>),
    Attribute {
        entity_id: Box<str>,
        row_index: usize,
        role: ErAttributeTextRole,
    },
    RelationLabel(usize),
    DiagramTitle,
    SubgraphLabel(Box<str>),
}

impl ErTextTerminalId {
    pub(super) fn entity_name(entity_id: &str) -> Self {
        Self::EntityName(entity_id.into())
    }

    pub(super) fn attribute(entity_id: &str, row_index: usize, role: ErAttributeTextRole) -> Self {
        Self::Attribute {
            entity_id: entity_id.into(),
            row_index,
            role,
        }
    }

    pub(super) const fn relation_label(relationship_index: usize) -> Self {
        Self::RelationLabel(relationship_index)
    }

    pub(super) const fn is_relation_label(&self) -> bool {
        matches!(self, Self::RelationLabel(_))
    }

    pub(super) const fn is_subgraph_label(&self) -> bool {
        matches!(self, Self::SubgraphLabel(_))
    }

    pub(super) fn entity_id(&self) -> Option<&str> {
        match self {
            Self::EntityName(entity_id) | Self::Attribute { entity_id, .. } => Some(entity_id),
            Self::RelationLabel(_) | Self::DiagramTitle | Self::SubgraphLabel(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ErTableRowTerminalId {
    entity_id: Box<str>,
    row_index: usize,
}

impl ErTableRowTerminalId {
    pub(super) fn new(entity_id: &str, row_index: usize) -> Self {
        Self {
            entity_id: entity_id.into(),
            row_index,
        }
    }

    pub(super) fn entity_id(&self) -> &str {
        &self.entity_id
    }

    pub(super) fn variant(&self) -> crate::diagram_theme::ThemeVariant {
        if self.row_index.is_multiple_of(2) {
            crate::diagram_theme::ThemeVariant::Odd
        } else {
            crate::diagram_theme::ThemeVariant::Even
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TextTerminalExpectation {
    pub(super) visual: super::visual::ErTextVisual,
    pub(super) paint: Option<ExpectedPaint>,
    pub(super) visible_run_count: usize,
    pub(super) inherited_color_run_count: usize,
}

/// Compact application facts collected while the existing ER writers emit their output.
#[derive(Debug, Default)]
pub(super) struct ErThemeOutcomes {
    applied_rules: BTreeSet<usize>,
    unverified_rules: BTreeSet<usize>,
    font_stack_visible: bool,
    font_stack_unverified: bool,
    font_size_visible: bool,
    font_size_unverified: bool,
}

impl ErThemeOutcomes {
    pub(super) fn has_effective_rule(&self, index: usize) -> bool {
        self.applied_rules.contains(&index)
    }

    pub(super) fn has_matching_rule(&self, index: usize) -> bool {
        self.applied_rules.contains(&index) && !self.unverified_rules.contains(&index)
    }

    pub(super) fn matches_typography(&self) -> bool {
        self.font_stack_visible && !self.font_stack_unverified
    }

    pub(super) fn matches_font_size(&self) -> bool {
        self.font_size_visible && !self.font_size_unverified
    }
}

/// The writer borrows its existing visual plan instead of copying a second expectation domain.
#[derive(Debug)]
pub(crate) struct ErEntityThemeReceipt<'a> {
    plan: &'a super::ErEntityThemePlan,
    outcomes: ErThemeOutcomes,
}

impl<'a> ErEntityThemeReceipt<'a> {
    pub(super) fn new(plan: &'a super::ErEntityThemePlan) -> Self {
        Self {
            plan,
            outcomes: ErThemeOutcomes::default(),
        }
    }

    pub(super) fn into_outcomes(self) -> ErThemeOutcomes {
        self.outcomes
    }

    pub(crate) fn records_text_terminals(&self) -> bool {
        self.plan.text_terminal_evidence_enabled
    }

    pub(crate) fn record_marker_conflict(&mut self, rule_index: usize) {
        self.outcomes.unverified_rules.insert(rule_index);
    }

    fn record_paint(&mut self, paint: Option<(usize, &str)>) {
        if let Some((index, _)) = paint {
            self.outcomes.applied_rules.insert(index);
        }
    }

    pub(crate) fn record_checkpointed_entity(
        &mut self,
        fill: Option<(usize, &str)>,
        stroke: Option<(usize, &str)>,
    ) {
        self.record_paint(fill);
        self.record_paint(stroke);
    }

    fn record_text(&mut self, id: ErTextTerminalId, facts: &VisibleTextStyleFacts) {
        if !self.records_text_terminals() || !facts.has_visible_runs() {
            return;
        }
        if let Some(paint) = self
            .plan
            .text_terminals
            .get(&id)
            .and_then(|text| text.paint.as_ref())
        {
            self.outcomes.applied_rules.insert(paint.rule_index);
            if facts.inherited_color_run_count() != facts.visible_run_count() {
                self.outcomes.unverified_rules.insert(paint.rule_index);
            }
        }
        if !id.is_subgraph_label() {
            self.outcomes.font_stack_visible = true;
            self.outcomes.font_stack_unverified |=
                facts.inherited_font_family_run_count() != facts.visible_run_count();
            if !id.is_relation_label() {
                self.outcomes.font_size_visible = true;
                self.outcomes.font_size_unverified |=
                    facts.inherited_font_size_run_count() != facts.visible_run_count();
            }
        }
    }

    pub(crate) fn record_entity_name_text(&mut self, id: &str, facts: &VisibleTextStyleFacts) {
        self.record_text(ErTextTerminalId::entity_name(id), facts);
    }

    pub(crate) fn record_attribute_text(
        &mut self,
        id: &str,
        row: usize,
        role: ErAttributeTextRole,
        facts: &VisibleTextStyleFacts,
    ) {
        self.record_text(ErTextTerminalId::attribute(id, row, role), facts);
    }

    pub(crate) fn record_relation_label_text(
        &mut self,
        index: usize,
        facts: &VisibleTextStyleFacts,
    ) {
        self.record_text(ErTextTerminalId::relation_label(index), facts);
    }

    pub(crate) fn record_subgraph_label(&mut self, id: &str, facts: &VisibleTextStyleFacts) {
        self.record_text(ErTextTerminalId::SubgraphLabel(id.into()), facts);
    }

    pub(crate) fn record_diagram_title_paint(&mut self, title: &str) {
        if self.records_text_terminals() {
            self.record_text(
                ErTextTerminalId::DiagramTitle,
                &VisibleTextStyleFacts::plain_text(title),
            );
        }
    }

    pub(crate) fn record_table_row(&mut self, id: &str, row: usize) {
        let index = self.plan.typed_table_row(id, row).map(|(index, _)| index);
        if let Some(index) = index {
            self.outcomes.applied_rules.insert(index);
        }
    }

    pub(crate) fn record_checkpointed_relation_path(&mut self, stroke: Option<(usize, &str)>) {
        self.record_paint(stroke);
    }
}
