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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ErRelationTerminalExpectation {
    edge_id: String,
    start_marker_id: Option<String>,
    end_marker_id: Option<String>,
}

impl ErRelationTerminalExpectation {
    pub(crate) fn new(
        edge_id: String,
        start_marker_id: Option<String>,
        end_marker_id: Option<String>,
    ) -> Self {
        Self {
            edge_id,
            start_marker_id,
            end_marker_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RelationTerminalCheckpoint {
    start_marker_id: Option<String>,
    end_marker_id: Option<String>,
    checkpointed: bool,
}

/// Writer-owned proof that every semantic ER entity and visible relationship terminal reached its
/// canonical SVG checkpoint, with exact typed values at the final writer owner.
#[derive(Debug, Clone)]
pub(crate) struct ErEntityThemeReceipt {
    expectations: Vec<EntityExpectation>,
    checkpointed_entities: Vec<bool>,
    relation_stroke: Option<ExpectedPaint>,
    relation_stroke_source_owned: bool,
    relation_paths: BTreeMap<String, RelationTerminalCheckpoint>,
    relation_markers: BTreeMap<String, bool>,
    attributes_match: bool,
    effective_by_rule: BTreeMap<usize, usize>,
    emitted_by_rule: BTreeMap<usize, usize>,
}

impl ErEntityThemeReceipt {
    pub(super) fn new(expectations: Vec<EntityExpectation>) -> Self {
        Self {
            checkpointed_entities: vec![false; expectations.len()],
            expectations,
            relation_stroke: None,
            relation_stroke_source_owned: false,
            relation_paths: BTreeMap::new(),
            relation_markers: BTreeMap::new(),
            attributes_match: true,
            effective_by_rule: BTreeMap::new(),
            emitted_by_rule: BTreeMap::new(),
        }
    }

    pub(super) fn with_relation_terminals(
        mut self,
        relation_stroke: Option<ExpectedPaint>,
        relation_stroke_source_owned: bool,
        relation_terminals: Vec<ErRelationTerminalExpectation>,
    ) -> Self {
        let expected_path_count = relation_terminals.len();
        let relation_marker_ids = relation_terminals
            .iter()
            .flat_map(|terminal| {
                [
                    terminal.start_marker_id.as_deref(),
                    terminal.end_marker_id.as_deref(),
                ]
            })
            .flatten()
            .map(ToOwned::to_owned)
            .collect::<BTreeSet<_>>();
        self.relation_paths = relation_terminals
            .into_iter()
            .map(|terminal| {
                (
                    terminal.edge_id,
                    RelationTerminalCheckpoint {
                        start_marker_id: terminal.start_marker_id,
                        end_marker_id: terminal.end_marker_id,
                        checkpointed: false,
                    },
                )
            })
            .collect();
        self.attributes_match &= self.relation_paths.len() == expected_path_count;
        self.relation_markers = relation_marker_ids
            .into_iter()
            .map(|id| (id, false))
            .collect();
        self.relation_stroke = relation_stroke;
        self.relation_stroke_source_owned = relation_stroke_source_owned;
        self
    }

    pub(crate) fn record_checkpointed_entity(
        &mut self,
        entity_index: usize,
        source_owns_fill: bool,
        source_owns_stroke: bool,
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
            source_owns_fill,
            emitted_fill,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
        self.attributes_match &= record_paint_checkpoint(
            expectation.stroke.as_ref(),
            source_owns_stroke,
            emitted_stroke,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
    }

    pub(crate) fn expects_relation_marker(&self, marker_id: &str) -> bool {
        self.relation_markers.contains_key(marker_id)
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
            self.relation_stroke.as_ref(),
            self.relation_stroke_source_owned,
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
        if !record_terminal_id(&mut self.relation_markers, marker_id) {
            self.attributes_match = false;
            return;
        }
        self.attributes_match &= record_paint_checkpoint(
            self.relation_stroke.as_ref(),
            self.relation_stroke_source_owned,
            emitted_stroke,
            &mut self.effective_by_rule,
            &mut self.emitted_by_rule,
        );
    }

    pub(super) fn proves_complete(&self) -> bool {
        self.attributes_match
            && self.checkpointed_entities.iter().all(|entry| *entry)
            && self.relation_paths.values().all(|entry| entry.checkpointed)
            && self.relation_markers.values().all(|entry| *entry)
    }

    pub(super) fn has_effective_rule(&self, rule_index: usize) -> bool {
        self.effective_by_rule
            .get(&rule_index)
            .copied()
            .unwrap_or(0)
            != 0
    }

    pub(super) fn proves_rule(&self, rule_index: usize) -> bool {
        self.proves_complete() && self.emitted_by_rule.get(&rule_index).copied().unwrap_or(0) != 0
    }
}

fn record_terminal_id(terminals: &mut BTreeMap<String, bool>, terminal_id: &str) -> bool {
    let Some(checkpointed) = terminals.get_mut(terminal_id) else {
        return false;
    };
    if *checkpointed {
        return false;
    }
    *checkpointed = true;
    true
}

fn record_paint_checkpoint(
    expected: Option<&ExpectedPaint>,
    source_owns: bool,
    emitted: Option<(usize, &str)>,
    effective_by_rule: &mut BTreeMap<usize, usize>,
    emitted_by_rule: &mut BTreeMap<usize, usize>,
) -> bool {
    if source_owns {
        return emitted.is_none();
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_receipt_requires_every_terminal_checkpoint_and_exact_values() {
        let mut receipt = ErEntityThemeReceipt::new(vec![EntityExpectation {
            fill: Some(ExpectedPaint {
                rule_index: 2,
                css: "#123456".to_string(),
            }),
            stroke: None,
        }]);
        receipt.record_checkpointed_entity(0, false, false, None, None);
        assert!(!receipt.proves_complete());

        let mut receipt = ErEntityThemeReceipt::new(vec![EntityExpectation {
            fill: Some(ExpectedPaint {
                rule_index: 2,
                css: "#123456".to_string(),
            }),
            stroke: None,
        }]);
        receipt.record_checkpointed_entity(0, false, false, Some((2, "#123456")), None);
        assert!(receipt.proves_complete());
        assert!(receipt.has_effective_rule(2));
        assert!(receipt.proves_rule(2));
    }

    #[test]
    fn source_owned_paint_is_not_an_effective_typed_route() {
        let mut receipt = ErEntityThemeReceipt::new(vec![EntityExpectation {
            fill: Some(ExpectedPaint {
                rule_index: 2,
                css: "#123456".to_string(),
            }),
            stroke: None,
        }]);
        receipt.record_checkpointed_entity(0, true, false, None, None);

        assert!(receipt.proves_complete());
        assert!(!receipt.has_effective_rule(2));
        assert!(!receipt.proves_rule(2));
    }

    #[test]
    fn entity_receipt_rejects_duplicate_and_wrong_paint_checkpoint() {
        let mut receipt = ErEntityThemeReceipt::new(vec![EntityExpectation {
            fill: Some(ExpectedPaint {
                rule_index: 2,
                css: "#123456".to_string(),
            }),
            stroke: None,
        }]);
        receipt.record_checkpointed_entity(0, false, false, Some((2, "#abcdef")), None);
        receipt.record_checkpointed_entity(0, false, false, Some((2, "#123456")), None);
        assert!(!receipt.proves_complete());
    }

    #[test]
    fn relation_receipt_requires_every_path_and_referenced_marker_with_exact_stroke() {
        let expected = ExpectedPaint {
            rule_index: 4,
            css: "#123456".to_string(),
        };
        let mut complete = ErEntityThemeReceipt::new(Vec::new()).with_relation_terminals(
            Some(expected.clone()),
            false,
            vec![
                ErRelationTerminalExpectation::new(
                    "edge-1".to_string(),
                    Some("marker-start".to_string()),
                    None,
                ),
                ErRelationTerminalExpectation::new(
                    "edge-2".to_string(),
                    None,
                    Some("marker-end".to_string()),
                ),
            ],
        );
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

        let mut missing_segment = ErEntityThemeReceipt::new(Vec::new()).with_relation_terminals(
            Some(expected),
            false,
            vec![
                ErRelationTerminalExpectation::new(
                    "edge-1".to_string(),
                    Some("marker-start".to_string()),
                    None,
                ),
                ErRelationTerminalExpectation::new("edge-2".to_string(), None, None),
            ],
        );
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
        let expected = ExpectedPaint {
            rule_index: 4,
            css: "transparent".to_string(),
        };
        let mut wrong_marker = ErEntityThemeReceipt::new(Vec::new()).with_relation_terminals(
            Some(expected.clone()),
            false,
            vec![ErRelationTerminalExpectation::new(
                "edge-1".to_string(),
                Some("marker-start".to_string()),
                None,
            )],
        );
        wrong_marker.record_checkpointed_relation_marker("marker-start", Some((4, "#123456")));
        wrong_marker.record_checkpointed_relation_path(
            "edge-1",
            Some((4, "transparent")),
            Some("marker-start"),
            None,
        );
        assert!(!wrong_marker.proves_complete());

        let mut duplicate_path = ErEntityThemeReceipt::new(Vec::new()).with_relation_terminals(
            Some(expected),
            false,
            vec![ErRelationTerminalExpectation::new(
                "edge-1".to_string(),
                None,
                None,
            )],
        );
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
