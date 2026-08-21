use std::collections::BTreeMap;

use crate::diagram_theme::{ResolvedStyleProperty, ThemeTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExpectedStroke {
    pub(super) rule_index: usize,
    pub(super) css: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExpectedPaint {
    pub(super) rule_index: usize,
    pub(super) css: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClassNodeTerminalExpectation {
    id: String,
    fill_winner: Option<usize>,
    stroke_winner: Option<usize>,
    label_fill_winner: Option<usize>,
    fill: Option<ExpectedPaint>,
    stroke: Option<ExpectedPaint>,
    label_fill: Option<ExpectedPaint>,
}

impl ClassNodeTerminalExpectation {
    pub(crate) fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            fill_winner: None,
            stroke_winner: None,
            label_fill_winner: None,
            fill: None,
            stroke: None,
            label_fill: None,
        }
    }

    pub(super) fn with_paints(
        mut self,
        fill_winner: Option<usize>,
        stroke_winner: Option<usize>,
        label_fill_winner: Option<usize>,
        fill: Option<ExpectedPaint>,
        stroke: Option<ExpectedPaint>,
        label_fill: Option<ExpectedPaint>,
    ) -> Self {
        self.fill_winner = fill_winner;
        self.stroke_winner = stroke_winner;
        self.label_fill_winner = label_fill_winner;
        self.fill = fill;
        self.stroke = stroke;
        self.label_fill = label_fill;
        self
    }

    pub(crate) fn id(&self) -> &str {
        self.id.as_str()
    }

    pub(crate) fn typed_fill(&self, source_owns: bool) -> Option<(usize, &str)> {
        typed_paint(&self.fill, source_owns)
    }

    pub(crate) fn typed_stroke(&self, source_owns: bool) -> Option<(usize, &str)> {
        typed_paint(&self.stroke, source_owns)
    }

    pub(crate) fn typed_label_fill(&self, source_owns: bool) -> Option<(usize, &str)> {
        typed_paint(&self.label_fill, source_owns)
    }

    fn expected_paint(
        &self,
        target: ThemeTarget,
        property: ResolvedStyleProperty,
    ) -> Option<&ExpectedPaint> {
        match (target, property) {
            (ThemeTarget::Node, ResolvedStyleProperty::Fill) => self.fill.as_ref(),
            (ThemeTarget::Node, ResolvedStyleProperty::Stroke) => self.stroke.as_ref(),
            (ThemeTarget::NodeLabel, ResolvedStyleProperty::Fill) => self.label_fill.as_ref(),
            _ => None,
        }
    }

    fn winner_rule(&self, target: ThemeTarget, property: ResolvedStyleProperty) -> Option<usize> {
        match (target, property) {
            (ThemeTarget::Node, ResolvedStyleProperty::Fill) => self.fill_winner,
            (ThemeTarget::Node, ResolvedStyleProperty::Stroke) => self.stroke_winner,
            (ThemeTarget::NodeLabel, ResolvedStyleProperty::Fill) => self.label_fill_winner,
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClassNodePaintTerminalEmission {
    terminal_applicable: bool,
    terminal_verified: bool,
    source_owned: bool,
    source_value: Option<String>,
    emitted: Option<(usize, String)>,
    terminal_value: String,
}

impl ClassNodePaintTerminalEmission {
    pub(crate) fn new(
        source_owned: bool,
        emitted: Option<(usize, &str)>,
        terminal_value: &str,
    ) -> Self {
        Self {
            terminal_applicable: true,
            terminal_verified: true,
            source_owned,
            source_value: None,
            emitted: emitted.map(|(rule_index, css)| (rule_index, css.to_string())),
            terminal_value: terminal_value.to_string(),
        }
    }

    pub(crate) fn not_applicable() -> Self {
        Self {
            terminal_applicable: false,
            terminal_verified: true,
            source_owned: false,
            source_value: None,
            emitted: None,
            terminal_value: String::new(),
        }
    }

    pub(crate) fn with_source_value(mut self, source_value: Option<&str>) -> Self {
        self.source_value = source_value.map(str::to_string);
        self
    }

    pub(crate) const fn with_terminal_verified(mut self, verified: bool) -> Self {
        self.terminal_verified = verified;
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClassNodeTerminalEmission {
    id: String,
    fill: ClassNodePaintTerminalEmission,
    stroke: ClassNodePaintTerminalEmission,
    label_fill: ClassNodePaintTerminalEmission,
}

impl ClassNodeTerminalEmission {
    pub(crate) fn new(
        id: impl Into<String>,
        fill: ClassNodePaintTerminalEmission,
        stroke: ClassNodePaintTerminalEmission,
        label_fill: ClassNodePaintTerminalEmission,
    ) -> Self {
        Self {
            id: id.into(),
            fill,
            stroke,
            label_fill,
        }
    }
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

/// Writer-owned proof that every semantic Class node, relation, and referenced marker reached its
/// canonical terminal SVG checkpoint with the exact typed terminal value.
#[derive(Debug, Clone)]
pub(crate) struct ClassRelationThemeReceipt {
    expected_nodes: BTreeMap<String, ClassNodeTerminalExpectation>,
    duplicate_expected_node: bool,
    expected_relations: Vec<ClassRelationTerminalExpectation>,
    expected_markers: Vec<ClassMarkerTerminalExpectation>,
    expected_stroke: Option<ExpectedStroke>,
    hand_drawn: bool,
    node_events: BTreeMap<String, ClassNodeTerminalEmission>,
    duplicate_node_event: bool,
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
            expected_nodes: BTreeMap::new(),
            duplicate_expected_node: false,
            expected_relations,
            expected_markers,
            expected_stroke,
            hand_drawn,
            node_events: BTreeMap::new(),
            duplicate_node_event: false,
            relation_events: Vec::new(),
            marker_events: Vec::new(),
            checkpointed_typed_width_paths: 0,
        }
    }

    pub(super) fn with_nodes(mut self, expected_nodes: Vec<ClassNodeTerminalExpectation>) -> Self {
        for expectation in expected_nodes {
            let id = expectation.id.clone();
            if self.expected_nodes.insert(id, expectation).is_some() {
                self.duplicate_expected_node = true;
            }
        }
        self
    }

    pub(super) fn expected_relation_count(&self) -> usize {
        self.expected_relations.len()
    }

    pub(super) fn expected_node_count(&self) -> usize {
        self.expected_nodes.len()
    }

    pub(crate) fn record_node(&mut self, emission: ClassNodeTerminalEmission) {
        let id = emission.id.clone();
        if self.node_events.insert(id, emission).is_some() {
            self.duplicate_node_event = true;
        }
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

    fn proves_complete_node_emission(&self) -> bool {
        !self.duplicate_expected_node
            && !self.duplicate_node_event
            && self.node_events.len() == self.expected_nodes.len()
            && self.expected_nodes.iter().all(|(id, expected)| {
                self.node_events.get(id).is_some_and(|event| {
                    node_paint_event_matches(expected.fill.as_ref(), &event.fill)
                        && node_paint_event_matches(expected.stroke.as_ref(), &event.stroke)
                        && node_label_paint_event_matches(
                            expected.label_fill.as_ref(),
                            &event.label_fill,
                        )
                })
            })
    }

    pub(super) fn proves_complete(&self) -> bool {
        self.proves_complete_node_emission()
            && self.proves_complete_relation_emission()
            && self.proves_complete_marker_emission()
    }

    pub(super) fn visible_marker_occurrence_count(&self) -> Option<usize> {
        self.proves_complete().then(|| {
            self.relation_events
                .iter()
                .map(|event| {
                    usize::from(event.start_marker.is_some())
                        .saturating_add(usize::from(event.end_marker.is_some()))
                })
                .sum()
        })
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

    pub(super) fn has_effective_node_paint_rule(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.expected_nodes.iter().any(|(id, expected)| {
            expected
                .expected_paint(target, property)
                .is_some_and(|paint| paint.rule_index == rule_index)
                && self.node_events.get(id).is_some_and(|event| {
                    let paint = node_paint_emission(event, target, property);
                    paint.terminal_applicable && !paint.source_owned
                })
        })
    }

    pub(super) fn proves_typed_node_paint(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.proves_complete_node_emission()
            && self.has_effective_node_paint_rule(rule_index, target, property)
            && self.expected_nodes.iter().all(|(id, expected)| {
                expected
                    .expected_paint(target, property)
                    .is_none_or(|paint| paint.rule_index != rule_index)
                    || self.node_events.get(id).is_some_and(|event| {
                        let paint = node_paint_emission(event, target, property);
                        !paint.terminal_applicable || paint.source_owned || paint.terminal_verified
                    })
            })
    }

    pub(super) fn has_node_paint_winner_rule(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.expected_nodes
            .values()
            .any(|expected| expected.winner_rule(target, property) == Some(rule_index))
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

fn node_paint_emission(
    event: &ClassNodeTerminalEmission,
    target: ThemeTarget,
    property: ResolvedStyleProperty,
) -> &ClassNodePaintTerminalEmission {
    match (target, property) {
        (ThemeTarget::Node, ResolvedStyleProperty::Fill) => &event.fill,
        (ThemeTarget::Node, ResolvedStyleProperty::Stroke) => &event.stroke,
        (ThemeTarget::NodeLabel, ResolvedStyleProperty::Fill) => &event.label_fill,
        _ => unreachable!("guarded Class node paint terminal"),
    }
}

fn typed_paint(expected: &Option<ExpectedPaint>, source_owns: bool) -> Option<(usize, &str)> {
    (!source_owns)
        .then_some(expected.as_ref())
        .flatten()
        .map(|expected| (expected.rule_index, expected.css.as_str()))
}

fn node_paint_event_matches(
    expected: Option<&ExpectedPaint>,
    event: &ClassNodePaintTerminalEmission,
) -> bool {
    if !event.terminal_applicable {
        return event.emitted.is_none();
    }
    if event.source_owned {
        return event.emitted.is_none()
            && event
                .source_value
                .as_deref()
                .is_none_or(|source| event.terminal_value == source);
    }
    match (expected, event.emitted.as_ref()) {
        (None, None) => true,
        (Some(expected), Some((rule_index, css))) => {
            expected.rule_index == *rule_index
                && expected.css == *css
                && event.terminal_value == expected.css
        }
        (Some(_), None) | (None, Some(_)) => false,
    }
}

fn node_label_paint_event_matches(
    expected: Option<&ExpectedPaint>,
    event: &ClassNodePaintTerminalEmission,
) -> bool {
    if !event.terminal_applicable {
        return event.emitted.is_none();
    }
    if event.source_owned {
        return event.emitted.is_none()
            && event.source_value.as_deref().is_none_or(|source| {
                terminal_paint(&event.terminal_value, "color") == Some(source)
            });
    }
    match (expected, event.emitted.as_ref()) {
        (None, None) => true,
        (Some(expected), Some((rule_index, css))) => {
            expected.rule_index == *rule_index
                && expected.css == *css
                && terminal_paint(&event.terminal_value, "color") == Some(expected.css.as_str())
                && terminal_paint(&event.terminal_value, "fill") == Some(expected.css.as_str())
        }
        (Some(_), None) | (None, Some(_)) => false,
    }
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

    fn expected_paint(rule_index: usize, css: &str) -> Option<ExpectedPaint> {
        Some(ExpectedPaint {
            rule_index,
            css: css.to_string(),
        })
    }

    fn node_paint(
        source_owned: bool,
        emitted: Option<(usize, &str)>,
        terminal_value: &str,
    ) -> ClassNodePaintTerminalEmission {
        ClassNodePaintTerminalEmission::new(source_owned, emitted, terminal_value)
    }

    #[test]
    fn node_receipt_requires_exact_checkpointed_terminal_values() {
        let mut receipt = ClassRelationThemeReceipt::new(Vec::new(), Vec::new(), None, false)
            .with_nodes(vec![
                ClassNodeTerminalExpectation::new("SourceOwned"),
                ClassNodeTerminalExpectation::new("ThemeOwned").with_paints(
                    Some(1),
                    Some(1),
                    Some(2),
                    expected_paint(1, "#112233"),
                    expected_paint(1, "#445566"),
                    expected_paint(2, "#ddeeff"),
                ),
            ]);
        receipt.record_node(ClassNodeTerminalEmission::new(
            "SourceOwned",
            node_paint(true, None, "#aa0000"),
            node_paint(true, None, "#bb0000"),
            node_paint(true, None, "color:#cc0000"),
        ));
        receipt.record_node(ClassNodeTerminalEmission::new(
            "ThemeOwned",
            node_paint(false, Some((1, "#112233")), "#112233"),
            node_paint(false, Some((1, "#445566")), "#445566"),
            node_paint(false, Some((2, "#ddeeff")), "color:#ddeeff;fill:#ddeeff"),
        ));

        assert!(receipt.proves_complete());
        assert!(
            receipt.proves_typed_node_paint(1, ThemeTarget::Node, ResolvedStyleProperty::Fill,)
        );
        assert!(receipt.proves_typed_node_paint(
            1,
            ThemeTarget::Node,
            ResolvedStyleProperty::Stroke,
        ));
        assert!(receipt.proves_typed_node_paint(
            2,
            ThemeTarget::NodeLabel,
            ResolvedStyleProperty::Fill,
        ));
    }

    #[test]
    fn node_receipt_rejects_missing_duplicate_and_wrong_label_emissions() {
        let receipt = || {
            ClassRelationThemeReceipt::new(Vec::new(), Vec::new(), None, false).with_nodes(vec![
                ClassNodeTerminalExpectation::new("A").with_paints(
                    Some(1),
                    None,
                    Some(2),
                    expected_paint(1, "#112233"),
                    None,
                    expected_paint(2, "#ddeeff"),
                ),
            ])
        };
        assert!(!receipt().proves_complete());

        let mut duplicate = receipt();
        for _ in 0..2 {
            duplicate.record_node(ClassNodeTerminalEmission::new(
                "A",
                node_paint(false, Some((1, "#112233")), "#112233"),
                node_paint(false, None, "#9370DB"),
                node_paint(false, Some((2, "#ddeeff")), "color:#ddeeff;fill:#ddeeff"),
            ));
        }
        assert!(!duplicate.proves_complete());

        let mut wrong_label = receipt();
        wrong_label.record_node(ClassNodeTerminalEmission::new(
            "A",
            node_paint(false, Some((1, "#112233")), "#112233"),
            node_paint(false, None, "#9370DB"),
            node_paint(false, Some((2, "#ddeeff")), "color:#ffffff;fill:#ffffff"),
        ));
        assert!(!wrong_label.proves_complete());
    }

    #[test]
    fn node_receipt_denominator_includes_lollipop_interfaces_and_unsupported_winners() {
        let mut receipt = ClassRelationThemeReceipt::new(Vec::new(), Vec::new(), None, false)
            .with_nodes(vec![
                ClassNodeTerminalExpectation::new("Service"),
                ClassNodeTerminalExpectation::new("interface0").with_paints(
                    Some(4),
                    None,
                    Some(5),
                    None,
                    None,
                    None,
                ),
            ]);
        receipt.record_node(ClassNodeTerminalEmission::new(
            "Service",
            node_paint(false, None, "#ECECFF"),
            node_paint(false, None, "#9370DB"),
            node_paint(false, None, ""),
        ));
        assert!(!receipt.proves_complete());

        receipt.record_node(ClassNodeTerminalEmission::new(
            "interface0",
            node_paint(false, None, ""),
            node_paint(false, None, ""),
            node_paint(false, None, ""),
        ));
        assert!(receipt.proves_complete());
        assert!(receipt.has_node_paint_winner_rule(
            4,
            ThemeTarget::Node,
            ResolvedStyleProperty::Fill,
        ));
        assert!(receipt.has_node_paint_winner_rule(
            5,
            ThemeTarget::NodeLabel,
            ResolvedStyleProperty::Fill,
        ));
        assert!(!receipt.proves_typed_node_paint(
            4,
            ThemeTarget::Node,
            ResolvedStyleProperty::Fill,
        ));
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

    #[test]
    fn marker_occurrence_count_comes_from_complete_relation_events() {
        let relations = vec![
            ClassRelationTerminalExpectation::new(0, Some("compositionStart"), None),
            ClassRelationTerminalExpectation::new(1, None, Some("extensionEnd")),
        ];
        let markers = vec![
            ClassMarkerTerminalExpectation::new("extensionEnd", false),
            ClassMarkerTerminalExpectation::new("compositionStart", true),
        ];
        let mut receipt =
            ClassRelationThemeReceipt::new(relations.clone(), markers, expected_stroke(), false);
        record_valid_markers(&mut receipt);
        for expected in &relations {
            record_relation(
                &mut receipt,
                expected.relation_index,
                expected.start_marker,
                expected.end_marker,
            );
        }
        assert_eq!(receipt.visible_marker_occurrence_count(), Some(2));

        let incomplete =
            ClassRelationThemeReceipt::new(relations, Vec::new(), expected_stroke(), false);
        assert_eq!(incomplete.visible_marker_occurrence_count(), None);
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
