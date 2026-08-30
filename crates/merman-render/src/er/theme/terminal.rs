use std::collections::{BTreeMap, BTreeSet};

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

    pub(super) fn entity_id(&self) -> Option<&str> {
        match self {
            Self::EntityName(entity_id) | Self::Attribute { entity_id, .. } => Some(entity_id),
            Self::RelationLabel(_) => None,
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
    pub(super) paint: Option<ExpectedPaint>,
    pub(super) visible_run_count: usize,
    pub(super) inherited_color_run_count: usize,
    pub(super) inherited_font_family_run_count: usize,
    pub(super) unverified_font_family_run_count: usize,
}

impl TextTerminalExpectation {
    fn portable_color_runs_verified(&self) -> bool {
        self.inherited_color_run_count == self.visible_run_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ErRelationTerminalExpectation {
    edge_id: String,
    start_marker_id: Option<String>,
    end_marker_id: Option<String>,
    stroke: Option<ExpectedPaint>,
}

impl ErRelationTerminalExpectation {
    pub(crate) fn new(
        edge_id: String,
        start_marker_id: Option<String>,
        end_marker_id: Option<String>,
        stroke: Option<(usize, &str)>,
    ) -> Self {
        Self {
            edge_id,
            start_marker_id,
            end_marker_id,
            stroke: stroke.map(|(rule_index, css)| ExpectedPaint {
                rule_index,
                css: css.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RelationTerminalCheckpoint {
    start_marker_id: Option<String>,
    end_marker_id: Option<String>,
    stroke: Option<ExpectedPaint>,
    checkpointed: bool,
}

#[derive(Debug, Clone)]
struct TextTerminalCheckpoint {
    expectation: TextTerminalExpectation,
    checkpointed: bool,
}

#[derive(Debug, Clone)]
struct DiagramTitleCheckpoint {
    expected_text: Box<str>,
    measurement_checkpointed: bool,
    emission_checkpointed: bool,
}

#[derive(Debug, Clone)]
struct PaintTerminalCheckpoint {
    expectation: Option<ExpectedPaint>,
    checkpointed: bool,
}

/// Writer-owned proof that every semantic ER terminal reached its canonical SVG checkpoint with
/// the exact family-plan winner. The receipt records semantic occurrence identities rather than
/// re-discovering terminals by parsing the finished SVG.
#[derive(Debug, Clone)]
pub(crate) struct ErEntityThemeReceipt {
    expectations: Vec<EntityExpectation>,
    checkpointed_entities: Vec<bool>,
    text_terminals: BTreeMap<ErTextTerminalId, TextTerminalCheckpoint>,
    table_rows: BTreeMap<ErTableRowTerminalId, PaintTerminalCheckpoint>,
    relation_paths: BTreeMap<String, RelationTerminalCheckpoint>,
    relation_markers: BTreeMap<String, PaintTerminalCheckpoint>,
    records_text_terminals: bool,
    typography_title: Option<DiagramTitleCheckpoint>,
    attributes_match: bool,
    typography_expected_font_family: Option<Box<str>>,
    typography_emitted_font_family: Option<Box<str>>,
    effective_by_rule: BTreeMap<usize, usize>,
    emitted_by_rule: BTreeMap<usize, usize>,
    unverified_rules: BTreeSet<usize>,
}

impl ErEntityThemeReceipt {
    pub(super) fn new(
        expectations: Vec<EntityExpectation>,
        text_terminals: BTreeMap<ErTextTerminalId, TextTerminalExpectation>,
        table_rows: BTreeMap<ErTableRowTerminalId, Option<ExpectedPaint>>,
        typography_expected_font_family: Option<Box<str>>,
    ) -> Self {
        Self {
            checkpointed_entities: vec![false; expectations.len()],
            expectations,
            text_terminals: text_terminals
                .into_iter()
                .map(|(id, expectation)| {
                    (
                        id,
                        TextTerminalCheckpoint {
                            expectation,
                            checkpointed: false,
                        },
                    )
                })
                .collect(),
            table_rows: table_rows
                .into_iter()
                .map(|(id, expectation)| {
                    (
                        id,
                        PaintTerminalCheckpoint {
                            expectation,
                            checkpointed: false,
                        },
                    )
                })
                .collect(),
            relation_paths: BTreeMap::new(),
            relation_markers: BTreeMap::new(),
            records_text_terminals: true,
            typography_title: None,
            attributes_match: true,
            typography_expected_font_family,
            typography_emitted_font_family: None,
            effective_by_rule: BTreeMap::new(),
            emitted_by_rule: BTreeMap::new(),
            unverified_rules: BTreeSet::new(),
        }
    }

    pub(super) fn without_text_terminals(mut self) -> Self {
        self.records_text_terminals = false;
        self.text_terminals.clear();
        self
    }

    pub(super) fn with_typography_title(mut self, title: Option<Box<str>>) -> Self {
        self.typography_title = title.map(|expected_text| DiagramTitleCheckpoint {
            expected_text,
            measurement_checkpointed: false,
            emission_checkpointed: false,
        });
        self
    }

    pub(crate) const fn records_text_terminals(&self) -> bool {
        self.records_text_terminals
    }

    pub(crate) fn record_typography_css_emission(&mut self, font_family: &str) {
        let Some(expected) = self.typography_expected_font_family.as_deref() else {
            return;
        };
        if self.typography_emitted_font_family.is_some() {
            self.attributes_match = false;
            return;
        }
        self.typography_emitted_font_family = Some(font_family.into());
        self.attributes_match &= expected == font_family;
    }

    pub(crate) fn record_diagram_title_measurement(
        &mut self,
        measured_text: &str,
        measured_font_family: Option<&str>,
    ) {
        let Some(checkpoint) = self.typography_title.as_mut() else {
            return;
        };
        if checkpoint.measurement_checkpointed {
            self.attributes_match = false;
            return;
        }
        checkpoint.measurement_checkpointed = true;
        self.attributes_match &= checkpoint.expected_text.as_ref() == measured_text;
        self.attributes_match &=
            self.typography_expected_font_family.as_deref() == measured_font_family;
    }

    pub(crate) fn record_diagram_title_emission(
        &mut self,
        emitted_class: &str,
        emitted_text: &str,
    ) {
        let Some(checkpoint) = self.typography_title.as_mut() else {
            return;
        };
        if checkpoint.emission_checkpointed {
            self.attributes_match = false;
            return;
        }
        checkpoint.emission_checkpointed = true;
        self.attributes_match &= emitted_class == "erDiagramTitleText";
        self.attributes_match &= checkpoint.expected_text.as_ref() == emitted_text;
    }

    pub(super) fn with_relation_terminals(
        mut self,
        relation_terminals: Vec<ErRelationTerminalExpectation>,
    ) -> Self {
        let expected_path_count = relation_terminals.len();
        for terminal in relation_terminals {
            for marker_id in [
                terminal.start_marker_id.as_deref(),
                terminal.end_marker_id.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                self.attributes_match &= merge_relation_marker_expectation(
                    &mut self.relation_markers,
                    marker_id,
                    terminal.stroke.as_ref(),
                );
            }
            let checkpoint = RelationTerminalCheckpoint {
                start_marker_id: terminal.start_marker_id,
                end_marker_id: terminal.end_marker_id,
                stroke: terminal.stroke,
                checkpointed: false,
            };
            self.attributes_match &= self
                .relation_paths
                .insert(terminal.edge_id, checkpoint)
                .is_none();
        }
        self.attributes_match &= self.relation_paths.len() == expected_path_count;
        self
    }

    pub(crate) fn record_checkpointed_entity(
        &mut self,
        entity_index: usize,
        emitted_fill: Option<(usize, &str)>,
        emitted_stroke: Option<(usize, &str)>,
    ) {
        let Some(checkpointed) = self.checkpointed_entities.get_mut(entity_index) else {
            self.attributes_match = false;
            return;
        };
        if *checkpointed {
            self.attributes_match = false;
            return;
        }
        *checkpointed = true;
        let expectation = self
            .expectations
            .get(entity_index)
            .cloned()
            .unwrap_or_default();
        self.attributes_match &= record_paint_checkpoint(
            expectation.fill.as_ref(),
            emitted_fill,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
        self.attributes_match &= record_paint_checkpoint(
            expectation.stroke.as_ref(),
            emitted_stroke,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
    }

    pub(crate) fn record_entity_name_text(
        &mut self,
        entity_id: &str,
        actual_terminal_style: &str,
        visible_run_count: usize,
        inherited_color_run_count: usize,
        inherited_font_family_run_count: usize,
        unverified_font_family_run_count: usize,
    ) {
        self.record_text_terminal(
            ErTextTerminalId::entity_name(entity_id),
            actual_terminal_style,
            visible_run_count,
            inherited_color_run_count,
            inherited_font_family_run_count,
            unverified_font_family_run_count,
        );
    }

    pub(crate) fn record_attribute_text(
        &mut self,
        entity_id: &str,
        row_index: usize,
        role: ErAttributeTextRole,
        actual_terminal_style: &str,
        visible_run_count: usize,
        inherited_color_run_count: usize,
        inherited_font_family_run_count: usize,
        unverified_font_family_run_count: usize,
    ) {
        self.record_text_terminal(
            ErTextTerminalId::attribute(entity_id, row_index, role),
            actual_terminal_style,
            visible_run_count,
            inherited_color_run_count,
            inherited_font_family_run_count,
            unverified_font_family_run_count,
        );
    }

    pub(crate) fn record_relation_label_text(
        &mut self,
        relationship_index: usize,
        actual_terminal_style: &str,
        visible_run_count: usize,
        inherited_color_run_count: usize,
        inherited_font_family_run_count: usize,
        unverified_font_family_run_count: usize,
    ) {
        self.record_text_terminal(
            ErTextTerminalId::relation_label(relationship_index),
            actual_terminal_style,
            visible_run_count,
            inherited_color_run_count,
            inherited_font_family_run_count,
            unverified_font_family_run_count,
        );
    }

    fn record_text_terminal(
        &mut self,
        id: ErTextTerminalId,
        actual_terminal_style: &str,
        visible_run_count: usize,
        inherited_color_run_count: usize,
        inherited_font_family_run_count: usize,
        unverified_font_family_run_count: usize,
    ) {
        if !self.records_text_terminals {
            return;
        }
        let Some(checkpoint) = self.text_terminals.get_mut(&id) else {
            self.attributes_match = false;
            return;
        };
        if checkpoint.checkpointed {
            self.attributes_match = false;
            return;
        }
        checkpoint.checkpointed = true;
        self.attributes_match &= checkpoint.expectation.visible_run_count == visible_run_count;
        self.attributes_match &=
            checkpoint.expectation.inherited_color_run_count == inherited_color_run_count;
        self.attributes_match &= checkpoint.expectation.inherited_font_family_run_count
            == inherited_font_family_run_count;
        self.attributes_match &= checkpoint.expectation.unverified_font_family_run_count
            == unverified_font_family_run_count;
        // The generic native fallback emits one fill per label, so mixed inherited and inline
        // colors cannot attest a portable typed foreground even when the browser terminal is exact.
        if let Some(expected) = checkpoint.expectation.paint.as_ref()
            && !checkpoint.expectation.portable_color_runs_verified()
        {
            self.unverified_rules.insert(expected.rule_index);
        }
        self.attributes_match &= record_terminal_style_checkpoint(
            checkpoint.expectation.paint.as_ref(),
            actual_terminal_style,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
    }

    pub(crate) fn record_table_row(
        &mut self,
        entity_id: &str,
        row_index: usize,
        actual_terminal_style: &str,
    ) {
        let id = ErTableRowTerminalId::new(entity_id, row_index);
        let Some(checkpoint) = self.table_rows.get_mut(&id) else {
            self.attributes_match = false;
            return;
        };
        if checkpoint.checkpointed {
            self.attributes_match = false;
            return;
        }
        checkpoint.checkpointed = true;
        self.attributes_match &= record_paint_style_checkpoint(
            checkpoint.expectation.as_ref(),
            actual_terminal_style,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
    }

    pub(crate) fn expects_relation_marker(&self, marker_id: &str) -> bool {
        self.relation_markers.contains_key(marker_id)
    }

    pub(crate) fn relation_marker_stroke(&self, marker_id: &str) -> Option<(usize, String)> {
        self.relation_markers
            .get(marker_id)?
            .expectation
            .as_ref()
            .map(|expected| (expected.rule_index, expected.css.clone()))
    }

    pub(crate) fn record_checkpointed_relation_path(
        &mut self,
        path_id: &str,
        emitted_stroke: Option<(usize, &str)>,
        emitted_start_marker_id: Option<&str>,
        emitted_end_marker_id: Option<&str>,
    ) {
        let Some(checkpoint) = self.relation_paths.get_mut(path_id) else {
            self.attributes_match = false;
            return;
        };
        if checkpoint.checkpointed {
            self.attributes_match = false;
            return;
        }
        checkpoint.checkpointed = true;
        self.attributes_match &= checkpoint.start_marker_id.as_deref() == emitted_start_marker_id;
        self.attributes_match &= checkpoint.end_marker_id.as_deref() == emitted_end_marker_id;
        self.attributes_match &= record_paint_checkpoint(
            checkpoint.stroke.as_ref(),
            emitted_stroke,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
    }

    pub(crate) fn record_checkpointed_relation_marker(
        &mut self,
        marker_id: &str,
        emitted_stroke: Option<(usize, &str)>,
    ) {
        let Some(checkpoint) = self.relation_markers.get_mut(marker_id) else {
            self.attributes_match = false;
            return;
        };
        if checkpoint.checkpointed {
            self.attributes_match = false;
            return;
        }
        checkpoint.checkpointed = true;
        self.attributes_match &= record_paint_checkpoint(
            checkpoint.expectation.as_ref(),
            emitted_stroke,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
    }

    pub(super) fn proves_complete(&self) -> bool {
        self.attributes_match
            && self.checkpointed_entities.iter().all(|entry| *entry)
            && self.text_terminals.values().all(|entry| entry.checkpointed)
            && self.table_rows.values().all(|entry| entry.checkpointed)
            && self.relation_paths.values().all(|entry| entry.checkpointed)
            && self
                .relation_markers
                .values()
                .all(|entry| entry.checkpointed)
            && self
                .typography_title
                .as_ref()
                .is_none_or(|entry| entry.measurement_checkpointed && entry.emission_checkpointed)
    }

    pub(super) fn has_effective_rule(&self, rule_index: usize) -> bool {
        self.effective_by_rule
            .get(&rule_index)
            .copied()
            .unwrap_or(0)
            != 0
    }

    pub(super) fn proves_rule(&self, rule_index: usize) -> bool {
        self.proves_complete()
            && !self.unverified_rules.contains(&rule_index)
            && self.emitted_by_rule.get(&rule_index).copied().unwrap_or(0) != 0
    }

    pub(super) fn proves_typography(&self) -> bool {
        let Some(expected) = self.typography_expected_font_family.as_deref() else {
            return false;
        };
        self.proves_complete()
            && self.typography_emitted_font_family.as_deref() == Some(expected)
            && self.text_terminals.values().all(|entry| {
                entry.expectation.inherited_font_family_run_count
                    == entry.expectation.visible_run_count
                    && entry.expectation.unverified_font_family_run_count == 0
            })
            && (self.typography_title.is_some()
                || self.text_terminals.values().any(|entry| {
                    entry.expectation.visible_run_count > 0
                        && entry.expectation.inherited_font_family_run_count
                            == entry.expectation.visible_run_count
                        && entry.expectation.unverified_font_family_run_count == 0
                }))
    }
}

fn merge_relation_marker_expectation(
    markers: &mut BTreeMap<String, PaintTerminalCheckpoint>,
    marker_id: &str,
    expected: Option<&ExpectedPaint>,
) -> bool {
    let Some(checkpoint) = markers.get_mut(marker_id) else {
        markers.insert(
            marker_id.to_string(),
            PaintTerminalCheckpoint {
                expectation: expected.cloned(),
                checkpointed: false,
            },
        );
        return true;
    };
    match (&checkpoint.expectation, expected) {
        (None, Some(expected)) => checkpoint.expectation = Some(expected.clone()),
        (Some(current), Some(expected)) if current != expected => return false,
        (None, None) | (Some(_), None) | (Some(_), Some(_)) => {}
    }
    true
}

fn record_paint_checkpoint(
    expected: Option<&ExpectedPaint>,
    emitted: Option<(usize, &str)>,
    effective_by_rule: &mut BTreeMap<usize, usize>,
    emitted_by_rule: &mut BTreeMap<usize, usize>,
) -> bool {
    match (expected, emitted) {
        (None, None) => true,
        (Some(expected), Some((rule_index, css))) => {
            *effective_by_rule.entry(expected.rule_index).or_default() += 1;
            let matches = expected.rule_index == rule_index && expected.css == css;
            if matches {
                *emitted_by_rule.entry(rule_index).or_default() += 1;
            }
            matches
        }
        (Some(expected), None) => {
            *effective_by_rule.entry(expected.rule_index).or_default() += 1;
            false
        }
        (None, Some(_)) => false,
    }
}

fn record_terminal_style_checkpoint(
    expected: Option<&ExpectedPaint>,
    actual_style: &str,
    effective_by_rule: &mut BTreeMap<usize, usize>,
    emitted_by_rule: &mut BTreeMap<usize, usize>,
) -> bool {
    match expected {
        None => actual_style.is_empty(),
        Some(expected) => {
            *effective_by_rule.entry(expected.rule_index).or_default() += 1;
            let matches = terminal_style_property(actual_style, "color")
                == Some(expected.css.as_str())
                && terminal_style_property(actual_style, "fill") == Some(expected.css.as_str());
            if matches {
                *emitted_by_rule.entry(expected.rule_index).or_default() += 1;
            }
            matches
        }
    }
}

fn record_paint_style_checkpoint(
    expected: Option<&ExpectedPaint>,
    actual_style: &str,
    effective_by_rule: &mut BTreeMap<usize, usize>,
    emitted_by_rule: &mut BTreeMap<usize, usize>,
) -> bool {
    match expected {
        None => actual_style.is_empty(),
        Some(expected) => {
            *effective_by_rule.entry(expected.rule_index).or_default() += 1;
            let matches =
                terminal_style_property(actual_style, "fill") == Some(expected.css.as_str());
            if matches {
                *emitted_by_rule.entry(expected.rule_index).or_default() += 1;
            }
            matches
        }
    }
}

fn terminal_style_property<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style.split(';').find_map(|declaration| {
        let (candidate, value) = declaration.split_once(':')?;
        (candidate.trim() == property).then(|| value.trim())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_receipt(expectations: Vec<EntityExpectation>) -> ErEntityThemeReceipt {
        ErEntityThemeReceipt::new(expectations, BTreeMap::new(), BTreeMap::new(), None)
    }

    #[test]
    fn entity_receipt_requires_every_terminal_checkpoint_and_exact_values() {
        let mut receipt = empty_receipt(vec![EntityExpectation {
            fill: Some(ExpectedPaint {
                rule_index: 2,
                css: "#123456".to_string(),
            }),
            stroke: None,
        }]);
        receipt.record_checkpointed_entity(0, None, None);
        assert!(!receipt.proves_complete());

        let mut receipt = empty_receipt(vec![EntityExpectation {
            fill: Some(ExpectedPaint {
                rule_index: 2,
                css: "#123456".to_string(),
            }),
            stroke: None,
        }]);
        receipt.record_checkpointed_entity(0, Some((2, "#123456")), None);
        assert!(receipt.proves_complete());
        assert!(receipt.has_effective_rule(2));
        assert!(receipt.proves_rule(2));
    }

    #[test]
    fn source_owned_paint_is_not_an_effective_typed_route() {
        let mut receipt = empty_receipt(vec![EntityExpectation::default()]);
        receipt.record_checkpointed_entity(0, None, None);

        assert!(receipt.proves_complete());
        assert!(!receipt.has_effective_rule(2));
        assert!(!receipt.proves_rule(2));
    }

    #[test]
    fn text_receipt_requires_exact_occurrence_style_and_inherited_run_count() {
        let id = ErTextTerminalId::attribute("entity-A-0", 1, ErAttributeTextRole::Name);
        let mut receipt = ErEntityThemeReceipt::new(
            Vec::new(),
            BTreeMap::from([(
                id,
                TextTerminalExpectation {
                    paint: Some(ExpectedPaint {
                        rule_index: 7,
                        css: "#123456".to_string(),
                    }),
                    visible_run_count: 2,
                    inherited_color_run_count: 2,
                    inherited_font_family_run_count: 2,
                    unverified_font_family_run_count: 0,
                },
            )]),
            BTreeMap::new(),
            None,
        );
        receipt.record_attribute_text(
            "entity-A-0",
            1,
            ErAttributeTextRole::Name,
            "color:#123456;fill:#123456",
            2,
            2,
            2,
            0,
        );

        assert!(receipt.proves_complete());
        assert!(receipt.proves_rule(7));
    }

    #[test]
    fn mixed_inline_and_inherited_color_runs_cannot_prove_portable_text_paint() {
        let id = ErTextTerminalId::relation_label(0);
        let mut receipt = ErEntityThemeReceipt::new(
            Vec::new(),
            BTreeMap::from([(
                id,
                TextTerminalExpectation {
                    paint: Some(ExpectedPaint {
                        rule_index: 7,
                        css: "#123456".to_string(),
                    }),
                    visible_run_count: 3,
                    inherited_color_run_count: 2,
                    inherited_font_family_run_count: 3,
                    unverified_font_family_run_count: 0,
                },
            )]),
            BTreeMap::new(),
            None,
        );
        receipt.record_relation_label_text(0, "color:#123456;fill:#123456", 3, 2, 3, 0);

        assert!(receipt.proves_complete());
        assert!(receipt.has_effective_rule(7));
        assert!(!receipt.proves_rule(7));
    }

    #[test]
    fn typography_receipt_requires_writer_css_and_inherited_font_runs() {
        let id = ErTextTerminalId::entity_name("entity-A-0");
        let mut receipt = ErEntityThemeReceipt::new(
            Vec::new(),
            BTreeMap::from([(
                id,
                TextTerminalExpectation {
                    paint: None,
                    visible_run_count: 1,
                    inherited_color_run_count: 1,
                    inherited_font_family_run_count: 1,
                    unverified_font_family_run_count: 0,
                },
            )]),
            BTreeMap::new(),
            Some("Inter, sans-serif".into()),
        );

        receipt.record_typography_css_emission("Inter, sans-serif");
        receipt.record_entity_name_text("entity-A-0", "", 1, 1, 1, 0);
        assert!(receipt.proves_typography());

        let mut unverified = ErEntityThemeReceipt::new(
            Vec::new(),
            BTreeMap::from([(
                ErTextTerminalId::entity_name("entity-A-0"),
                TextTerminalExpectation {
                    paint: None,
                    visible_run_count: 1,
                    inherited_color_run_count: 1,
                    inherited_font_family_run_count: 0,
                    unverified_font_family_run_count: 1,
                },
            )]),
            BTreeMap::new(),
            Some("Inter, sans-serif".into()),
        );
        unverified.record_typography_css_emission("Inter, sans-serif");
        unverified.record_entity_name_text("entity-A-0", "", 1, 1, 0, 1);
        assert!(!unverified.proves_typography());
    }

    #[test]
    fn typography_receipt_accepts_only_the_expected_diagram_title_terminal() {
        fn receipt() -> ErEntityThemeReceipt {
            ErEntityThemeReceipt::new(
                Vec::new(),
                BTreeMap::new(),
                BTreeMap::new(),
                Some("Inter, sans-serif".into()),
            )
            .with_typography_title(Some("ER overview".into()))
        }

        let mut complete = receipt();
        complete.record_typography_css_emission("Inter, sans-serif");
        complete.record_diagram_title_measurement("ER overview", Some("Inter, sans-serif"));
        complete.record_diagram_title_emission("erDiagramTitleText", "ER overview");
        assert!(complete.proves_typography());

        let mut missing_measurement = receipt();
        missing_measurement.record_typography_css_emission("Inter, sans-serif");
        missing_measurement.record_diagram_title_emission("erDiagramTitleText", "ER overview");
        assert!(!missing_measurement.proves_typography());

        let mut missing_emission = receipt();
        missing_emission.record_typography_css_emission("Inter, sans-serif");
        missing_emission.record_diagram_title_measurement("ER overview", Some("Inter, sans-serif"));
        assert!(!missing_emission.proves_typography());

        let mut wrong_measured_text = receipt();
        wrong_measured_text.record_typography_css_emission("Inter, sans-serif");
        wrong_measured_text
            .record_diagram_title_measurement("Wrong title", Some("Inter, sans-serif"));
        wrong_measured_text.record_diagram_title_emission("erDiagramTitleText", "ER overview");
        assert!(!wrong_measured_text.proves_typography());

        let mut wrong_emitted_text = receipt();
        wrong_emitted_text.record_typography_css_emission("Inter, sans-serif");
        wrong_emitted_text
            .record_diagram_title_measurement("ER overview", Some("Inter, sans-serif"));
        wrong_emitted_text.record_diagram_title_emission("erDiagramTitleText", "Wrong title");
        assert!(!wrong_emitted_text.proves_typography());

        let mut wrong_class = receipt();
        wrong_class.record_typography_css_emission("Inter, sans-serif");
        wrong_class.record_diagram_title_measurement("ER overview", Some("Inter, sans-serif"));
        wrong_class.record_diagram_title_emission("wrongTitleClass", "ER overview");
        assert!(!wrong_class.proves_typography());

        let mut wrong_measurement_font = receipt();
        wrong_measurement_font.record_typography_css_emission("Inter, sans-serif");
        wrong_measurement_font.record_diagram_title_measurement("ER overview", Some("serif"));
        wrong_measurement_font.record_diagram_title_emission("erDiagramTitleText", "ER overview");
        assert!(!wrong_measurement_font.proves_typography());

        let mut duplicate_measurement = complete.clone();
        duplicate_measurement
            .record_diagram_title_measurement("ER overview", Some("Inter, sans-serif"));
        assert!(!duplicate_measurement.proves_typography());

        let mut duplicate_emission = complete;
        duplicate_emission.record_diagram_title_emission("erDiagramTitleText", "ER overview");
        assert!(!duplicate_emission.proves_typography());
    }

    #[test]
    fn table_receipt_rejects_a_wrong_or_duplicate_row_terminal() {
        let id = ErTableRowTerminalId::new("entity-A-0", 0);
        let expectation = ExpectedPaint {
            rule_index: 8,
            css: "#abcdef".to_string(),
        };
        let mut wrong = ErEntityThemeReceipt::new(
            Vec::new(),
            BTreeMap::new(),
            BTreeMap::from([(id.clone(), Some(expectation.clone()))]),
            None,
        );
        wrong.record_table_row("entity-A-0", 0, "fill:#123456");
        assert!(!wrong.proves_complete());

        let mut duplicate = ErEntityThemeReceipt::new(
            Vec::new(),
            BTreeMap::new(),
            BTreeMap::from([(id, Some(expectation))]),
            None,
        );
        duplicate.record_table_row("entity-A-0", 0, "color:#abcdef;fill:#abcdef");
        duplicate.record_table_row("entity-A-0", 0, "color:#abcdef;fill:#abcdef");
        assert!(!duplicate.proves_complete());
    }

    #[test]
    fn relation_receipt_requires_every_path_and_referenced_marker_with_exact_stroke() {
        let mut complete = empty_receipt(Vec::new()).with_relation_terminals(vec![
            ErRelationTerminalExpectation::new(
                "edge-1".to_string(),
                Some("marker-start".to_string()),
                None,
                Some((4, "#123456")),
            ),
            ErRelationTerminalExpectation::new(
                "edge-2".to_string(),
                None,
                Some("marker-end".to_string()),
                Some((4, "#123456")),
            ),
        ]);
        complete.record_checkpointed_relation_marker("marker-start", Some((4, "#123456")));
        complete.record_checkpointed_relation_marker("marker-end", Some((4, "#123456")));
        complete.record_checkpointed_relation_path(
            "edge-1",
            Some((4, "#123456")),
            Some("marker-start"),
            None,
        );
        complete.record_checkpointed_relation_path(
            "edge-2",
            Some((4, "#123456")),
            None,
            Some("marker-end"),
        );
        assert!(complete.proves_complete());
        assert!(complete.proves_rule(4));

        let mut missing_segment = empty_receipt(Vec::new()).with_relation_terminals(vec![
            ErRelationTerminalExpectation::new(
                "edge-1".to_string(),
                Some("marker-start".to_string()),
                None,
                Some((4, "#123456")),
            ),
            ErRelationTerminalExpectation::new(
                "edge-2".to_string(),
                None,
                None,
                Some((4, "#123456")),
            ),
        ]);
        missing_segment.record_checkpointed_relation_marker("marker-start", Some((4, "#123456")));
        missing_segment.record_checkpointed_relation_path(
            "edge-1",
            Some((4, "#123456")),
            Some("marker-start"),
            None,
        );
        assert!(!missing_segment.proves_complete());
    }

    #[test]
    fn relation_receipt_rejects_duplicate_or_mismatched_terminals() {
        let mut wrong_marker = empty_receipt(Vec::new()).with_relation_terminals(vec![
            ErRelationTerminalExpectation::new(
                "edge-1".to_string(),
                Some("marker-start".to_string()),
                None,
                Some((4, "transparent")),
            ),
        ]);
        wrong_marker.record_checkpointed_relation_marker("marker-start", Some((4, "#123456")));
        wrong_marker.record_checkpointed_relation_path(
            "edge-1",
            Some((4, "transparent")),
            Some("marker-start"),
            None,
        );
        assert!(!wrong_marker.proves_complete());

        let mut duplicate_path = empty_receipt(Vec::new()).with_relation_terminals(vec![
            ErRelationTerminalExpectation::new(
                "edge-1".to_string(),
                None,
                None,
                Some((4, "transparent")),
            ),
        ]);
        duplicate_path.record_checkpointed_relation_path(
            "edge-1",
            Some((4, "transparent")),
            None,
            None,
        );
        duplicate_path.record_checkpointed_relation_path(
            "edge-1",
            Some((4, "transparent")),
            None,
            None,
        );
        assert!(!duplicate_path.proves_complete());
    }
}
