#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExpectedStroke {
    pub(super) rule_index: usize,
    pub(super) css: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassRelationTerminalExpectation {
    pub(super) relation_index: usize,
    pub(super) start_marker: Option<&'static str>,
    pub(super) end_marker: Option<&'static str>,
}

impl ClassRelationTerminalExpectation {
    pub(crate) const fn new(
        relation_index: usize,
        start_marker: Option<&'static str>,
        end_marker: Option<&'static str>,
    ) -> Self {
        Self {
            relation_index,
            start_marker,
            end_marker,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassMarkerTerminalExpectation {
    pub(super) name: &'static str,
    pub(super) fill_follows_stroke: bool,
}

impl ClassMarkerTerminalExpectation {
    pub(crate) const fn new(name: &'static str, fill_follows_stroke: bool) -> Self {
        Self {
            name,
            fill_follows_stroke,
        }
    }
}

/// Writer-owned proof that every semantic Class relation and referenced marker reached its
/// canonical terminal SVG checkpoint in semantic order with the exact typed stroke value.
#[derive(Debug, Clone)]
pub(crate) struct ClassRelationThemeReceipt {
    expected_relations: Vec<ClassRelationTerminalExpectation>,
    expected_markers: Vec<ClassMarkerTerminalExpectation>,
    expected_stroke: Option<ExpectedStroke>,
    hand_drawn: bool,
    relation_events: Vec<ClassRelationTerminalEvent>,
    marker_events: Vec<ClassMarkerTerminalEvent>,
    checkpointed_typed_width_paths: usize,
}

impl ClassRelationThemeReceipt {
    pub(super) fn new(
        expected_relations: Vec<ClassRelationTerminalExpectation>,
        expected_markers: Vec<ClassMarkerTerminalExpectation>,
        expected_stroke: Option<ExpectedStroke>,
        hand_drawn: bool,
    ) -> Self {
        Self {
            expected_relations,
            expected_markers,
            expected_stroke,
            hand_drawn,
            relation_events: Vec::new(),
            marker_events: Vec::new(),
            checkpointed_typed_width_paths: 0,
        }
    }

    pub(super) fn expected_relation_count(&self) -> usize {
        self.expected_relations.len()
    }

    pub(crate) fn themes_marker(&self, marker_name: &str) -> bool {
        self.expected_stroke.is_some()
            && self
                .expected_markers
                .iter()
                .any(|expected| expected.name == marker_name)
    }

    pub(crate) fn record_marker(
        &mut self,
        name: &'static str,
        fill_follows_stroke: bool,
        emitted_stroke: Option<(usize, &str)>,
        terminal_style: Option<&str>,
    ) {
        self.marker_events.push(ClassMarkerTerminalEvent {
            name,
            fill_follows_stroke,
            emitted_stroke: emitted_stroke.map(|(rule_index, css)| (rule_index, css.to_string())),
            terminal_style: terminal_style.map(str::to_string),
        });
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_relation(
        &mut self,
        relation_index: usize,
        start_marker: Option<&'static str>,
        end_marker: Option<&'static str>,
        emitted_stroke: Option<(usize, &str)>,
        terminal_style: &str,
        hand_drawn_stroke: Option<&str>,
        typed_width_emitted: bool,
    ) {
        self.relation_events.push(ClassRelationTerminalEvent {
            relation_index,
            start_marker,
            end_marker,
            emitted_stroke: emitted_stroke.map(|(rule_index, css)| (rule_index, css.to_string())),
            terminal_style: terminal_style.to_string(),
            hand_drawn_stroke: hand_drawn_stroke.map(str::to_string),
        });
        if typed_width_emitted {
            self.checkpointed_typed_width_paths =
                self.checkpointed_typed_width_paths.saturating_add(1);
        }
    }

    fn proves_complete_relation_emission(&self) -> bool {
        self.relation_events.len() == self.expected_relations.len()
            && self
                .relation_events
                .iter()
                .zip(&self.expected_relations)
                .all(|(event, expected)| {
                    event.relation_index == expected.relation_index
                        && event.start_marker == expected.start_marker
                        && event.end_marker == expected.end_marker
                        && stroke_event_matches(
                            self.expected_stroke.as_ref(),
                            event.emitted_stroke.as_ref(),
                            &event.terminal_style,
                        )
                        && hand_drawn_stroke_matches(
                            self.expected_stroke.as_ref(),
                            self.hand_drawn,
                            event.hand_drawn_stroke.as_deref(),
                        )
                })
    }

    fn proves_complete_marker_emission(&self) -> bool {
        self.marker_events.len() == self.expected_markers.len()
            && self
                .marker_events
                .iter()
                .zip(&self.expected_markers)
                .all(|(event, expected)| {
                    event.name == expected.name
                        && event.fill_follows_stroke == expected.fill_follows_stroke
                        && stroke_event_matches(
                            self.expected_stroke.as_ref(),
                            event.emitted_stroke.as_ref(),
                            event.terminal_style.as_deref().unwrap_or_default(),
                        )
                        && (!expected.fill_follows_stroke
                            || self.expected_stroke.as_ref().is_none_or(|stroke| {
                                terminal_paint(
                                    event.terminal_style.as_deref().unwrap_or_default(),
                                    "fill",
                                ) == Some(stroke.css.as_str())
                            }))
                })
    }

    pub(super) fn proves_complete(&self) -> bool {
        self.proves_complete_relation_emission() && self.proves_complete_marker_emission()
    }

    pub(super) fn proves_typed_width(&self) -> bool {
        self.proves_complete_relation_emission()
            && self.checkpointed_typed_width_paths == self.expected_relations.len()
    }

    fn has_effective_stroke_rule(&self, rule_index: usize) -> bool {
        self.expected_stroke
            .as_ref()
            .is_some_and(|expected| expected.rule_index == rule_index)
            && !self.expected_relations.is_empty()
    }

    pub(super) fn proves_typed_stroke(&self, rule_index: usize) -> bool {
        self.proves_complete()
            && self.has_effective_stroke_rule(rule_index)
            && self.relation_events.iter().all(|event| {
                event
                    .emitted_stroke
                    .as_ref()
                    .is_some_and(|(emitted_rule, _)| *emitted_rule == rule_index)
            })
    }
}

#[derive(Debug, Clone)]
struct ClassRelationTerminalEvent {
    relation_index: usize,
    start_marker: Option<&'static str>,
    end_marker: Option<&'static str>,
    emitted_stroke: Option<(usize, String)>,
    terminal_style: String,
    hand_drawn_stroke: Option<String>,
}

#[derive(Debug, Clone)]
struct ClassMarkerTerminalEvent {
    name: &'static str,
    fill_follows_stroke: bool,
    emitted_stroke: Option<(usize, String)>,
    terminal_style: Option<String>,
}

fn stroke_event_matches(
    expected: Option<&ExpectedStroke>,
    emitted: Option<&(usize, String)>,
    terminal_style: &str,
) -> bool {
    match (expected, emitted) {
        (None, None) => true,
        (Some(expected), Some((rule_index, css))) => {
            expected.rule_index == *rule_index
                && expected.css == *css
                && terminal_paint(terminal_style, "stroke") == Some(expected.css.as_str())
        }
        (Some(_), None) | (None, Some(_)) => false,
    }
}

fn hand_drawn_stroke_matches(
    expected: Option<&ExpectedStroke>,
    hand_drawn: bool,
    emitted: Option<&str>,
) -> bool {
    match (hand_drawn, emitted) {
        (false, None) => true,
        (true, Some(emitted)) => expected.is_none_or(|expected| emitted == expected.css),
        (false, Some(_)) | (true, None) => false,
    }
}

fn terminal_paint<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style
        .split(';')
        .filter_map(crate::mermaid_style::parse_style_declaration)
        .filter(|declaration| declaration.property() == property)
        .map(|declaration| declaration.value())
        .next_back()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected_stroke() -> Option<ExpectedStroke> {
        Some(ExpectedStroke {
            rule_index: 3,
            css: "#123456".to_string(),
        })
    }

    #[test]
    fn relation_stroke_receipt_rejects_missing_wrong_duplicate_and_reordered_events() {
        let relations = vec![
            ClassRelationTerminalExpectation::new(0, Some("compositionStart"), None),
            ClassRelationTerminalExpectation::new(1, None, Some("extensionEnd")),
        ];
        let markers = vec![
            ClassMarkerTerminalExpectation::new("extensionEnd", false),
            ClassMarkerTerminalExpectation::new("compositionStart", true),
        ];

        let mut missing = ClassRelationThemeReceipt::new(
            relations.clone(),
            markers.clone(),
            expected_stroke(),
            false,
        );
        record_valid_markers(&mut missing);
        record_relation(&mut missing, 0, Some("compositionStart"), None);
        assert!(!missing.proves_complete());

        let mut wrong = ClassRelationThemeReceipt::new(
            relations.clone(),
            markers.clone(),
            expected_stroke(),
            false,
        );
        wrong.record_marker(
            "extensionEnd",
            false,
            Some((3, "#abcdef")),
            Some("stroke:#abcdef !important"),
        );
        wrong.record_marker(
            "compositionStart",
            true,
            Some((3, "#123456")),
            Some("stroke:#123456 !important;fill:#123456 !important"),
        );
        for expected in &relations {
            record_relation(
                &mut wrong,
                expected.relation_index,
                expected.start_marker,
                expected.end_marker,
            );
        }
        assert!(!wrong.proves_complete());

        let mut reordered =
            ClassRelationThemeReceipt::new(relations, markers, expected_stroke(), false);
        reordered.record_marker(
            "compositionStart",
            true,
            Some((3, "#123456")),
            Some("stroke:#123456 !important;fill:#123456 !important"),
        );
        reordered.record_marker(
            "extensionEnd",
            false,
            Some((3, "#123456")),
            Some("stroke:#123456 !important"),
        );
        record_relation(&mut reordered, 1, None, Some("extensionEnd"));
        record_relation(&mut reordered, 1, None, Some("extensionEnd"));
        assert!(!reordered.proves_complete());
    }

    #[test]
    fn hand_drawn_path_and_filled_marker_must_match_the_typed_stroke() {
        let mut missing_hand_drawn_stroke = single_hand_drawn_receipt();
        missing_hand_drawn_stroke.record_marker(
            "compositionStart",
            true,
            Some((3, "#123456")),
            Some("stroke:#123456 !important;fill:#123456 !important"),
        );
        missing_hand_drawn_stroke.record_relation(
            0,
            Some("compositionStart"),
            None,
            Some((3, "#123456")),
            ";;;stroke:#123456 !important;",
            None,
            false,
        );
        assert!(!missing_hand_drawn_stroke.proves_complete());

        let mut wrong_marker_fill = single_hand_drawn_receipt();
        wrong_marker_fill.record_marker(
            "compositionStart",
            true,
            Some((3, "#123456")),
            Some("stroke:#123456 !important;fill:#abcdef !important"),
        );
        wrong_marker_fill.record_relation(
            0,
            Some("compositionStart"),
            None,
            Some((3, "#123456")),
            ";;;stroke:#123456 !important;",
            Some("#123456"),
            false,
        );
        assert!(!wrong_marker_fill.proves_complete());
    }

    fn single_hand_drawn_receipt() -> ClassRelationThemeReceipt {
        ClassRelationThemeReceipt::new(
            vec![ClassRelationTerminalExpectation::new(
                0,
                Some("compositionStart"),
                None,
            )],
            vec![ClassMarkerTerminalExpectation::new(
                "compositionStart",
                true,
            )],
            expected_stroke(),
            true,
        )
    }

    fn record_valid_markers(receipt: &mut ClassRelationThemeReceipt) {
        receipt.record_marker(
            "extensionEnd",
            false,
            Some((3, "#123456")),
            Some("stroke:#123456 !important"),
        );
        receipt.record_marker(
            "compositionStart",
            true,
            Some((3, "#123456")),
            Some("stroke:#123456 !important;fill:#123456 !important"),
        );
    }

    fn record_relation(
        receipt: &mut ClassRelationThemeReceipt,
        relation_index: usize,
        start_marker: Option<&'static str>,
        end_marker: Option<&'static str>,
    ) {
        receipt.record_relation(
            relation_index,
            start_marker,
            end_marker,
            Some((3, "#123456")),
            ";;;stroke:#123456 !important;",
            None,
            false,
        );
    }
}
