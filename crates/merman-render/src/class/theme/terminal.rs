use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{ResolvedStyleProperty, ThemeTarget};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExpectedStroke {
    pub(super) rule_index: usize,
    pub(super) property: ResolvedStyleProperty,
    pub(super) css: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExpectedPaint {
    pub(super) target: ThemeTarget,
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

    pub(crate) fn label_fill_target(&self) -> Option<ThemeTarget> {
        self.label_fill.as_ref().map(|paint| paint.target)
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

/// Cluster expectations are fixed before any terminal checkpoint is recorded.
#[derive(Debug, Default)]
pub(super) struct ClassClusterTerminalExpectation {
    pub(super) ids: Vec<String>,
    pub(super) fill: Option<ExpectedPaint>,
    pub(super) stroke: Option<ExpectedPaint>,
    pub(super) namespace_title: Option<ExpectedPaint>,
}

/// Writer-owned proof that every semantic Class node, relation, attached note, and referenced marker reached its
/// canonical terminal SVG checkpoint with the exact typed terminal value.
#[derive(Debug, Clone)]
pub(crate) struct ClassRelationThemeReceipt {
    expected_clusters: BTreeSet<String>,
    duplicate_expected_cluster: bool,
    expected_cluster_fill: Option<ExpectedPaint>,
    expected_cluster_stroke: Option<ExpectedPaint>,
    expected_namespace_title: Option<ExpectedPaint>,
    cluster_events: BTreeMap<String, ClassClusterTerminalEvent>,
    duplicate_cluster_event: bool,
    expected_nodes: BTreeMap<String, ClassNodeTerminalExpectation>,
    duplicate_expected_node: bool,
    expected_relations: Vec<ClassRelationTerminalExpectation>,
    expected_note_attachments: Vec<usize>,
    expected_markers: Vec<ClassMarkerTerminalExpectation>,
    expected_stroke: Option<ExpectedStroke>,
    hand_drawn: bool,
    node_events: BTreeMap<String, ClassNodeTerminalEmission>,
    duplicate_node_event: bool,
    relation_events: BTreeMap<usize, ClassRelationTerminalEvent>,
    duplicate_relation_event: bool,
    note_attachment_events: BTreeMap<usize, ClassNoteAttachmentTerminalEvent>,
    duplicate_note_attachment_event: bool,
    marker_events: Vec<ClassMarkerTerminalEvent>,
    checkpointed_typed_width_paths: usize,
    edge_label_background_events: Option<BTreeMap<usize, bool>>,
    expected_edge_label_background: Option<ExpectedPaint>,
    edge_label_background_paints: BTreeMap<usize, bool>,
    duplicate_edge_label_background_event: bool,
}

impl ClassRelationThemeReceipt {
    #[cfg(test)]
    pub(super) fn new(
        expected_relations: Vec<ClassRelationTerminalExpectation>,
        expected_markers: Vec<ClassMarkerTerminalExpectation>,
        expected_stroke: Option<ExpectedStroke>,
        hand_drawn: bool,
    ) -> Self {
        Self::new_with_clusters(
            expected_relations,
            expected_markers,
            expected_stroke,
            hand_drawn,
            ClassClusterTerminalExpectation::default(),
        )
    }

    pub(super) fn new_with_clusters(
        expected_relations: Vec<ClassRelationTerminalExpectation>,
        expected_markers: Vec<ClassMarkerTerminalExpectation>,
        expected_stroke: Option<ExpectedStroke>,
        hand_drawn: bool,
        clusters: ClassClusterTerminalExpectation,
    ) -> Self {
        let mut expected_clusters = BTreeSet::new();
        let mut duplicate_expected_cluster = false;
        for id in clusters.ids {
            if !expected_clusters.insert(id) {
                duplicate_expected_cluster = true;
            }
        }
        Self {
            expected_clusters,
            duplicate_expected_cluster,
            expected_cluster_fill: clusters.fill,
            expected_cluster_stroke: clusters.stroke,
            expected_namespace_title: clusters.namespace_title,
            cluster_events: BTreeMap::new(),
            duplicate_cluster_event: false,
            expected_nodes: BTreeMap::new(),
            duplicate_expected_node: false,
            expected_relations,
            expected_note_attachments: Vec::new(),
            expected_markers,
            expected_stroke,
            hand_drawn,
            node_events: BTreeMap::new(),
            duplicate_node_event: false,
            relation_events: BTreeMap::new(),
            duplicate_relation_event: false,
            note_attachment_events: BTreeMap::new(),
            duplicate_note_attachment_event: false,
            marker_events: Vec::new(),
            checkpointed_typed_width_paths: 0,
            edge_label_background_events: None,
            expected_edge_label_background: None,
            edge_label_background_paints: BTreeMap::new(),
            duplicate_edge_label_background_event: false,
        }
    }

    pub(super) fn expected_cluster_count(&self) -> usize {
        self.expected_clusters.len()
    }

    pub(crate) fn record_cluster(
        &mut self,
        id: &str,
        fill_rule: Option<usize>,
        stroke_rule: Option<usize>,
        terminal_style: &str,
    ) {
        let event = ClassClusterTerminalEvent {
            paint_matches: cluster_paint_event_matches(
                self.expected_cluster_fill.as_ref(),
                fill_rule,
                terminal_style,
                "fill",
            ) && cluster_paint_event_matches(
                self.expected_cluster_stroke.as_ref(),
                stroke_rule,
                terminal_style,
                "stroke",
            ),
            title: None,
        };
        if self.cluster_events.insert(id.to_string(), event).is_some() {
            self.duplicate_cluster_event = true;
        }
    }

    pub(crate) fn record_namespace_title(
        &mut self,
        id: &str,
        rule: usize,
        style: &str,
        visible: bool,
        inherits_paint: bool,
    ) {
        let Some(cluster) = self.cluster_events.get_mut(id) else {
            self.duplicate_cluster_event = true;
            return;
        };
        if cluster.title.is_some() {
            self.duplicate_cluster_event = true;
        }
        cluster.title = Some(ClassNamespaceTitleTerminalEvent {
            paint_matches: self
                .expected_namespace_title
                .as_ref()
                .is_some_and(|expected| {
                    rule == expected.rule_index
                        && terminal_paint(style, "color") == Some(expected.css.as_str())
                        && terminal_paint(style, "fill") == Some(expected.css.as_str())
                }),
            visible,
            inherits_paint,
        });
    }

    pub(super) fn proves_typed_namespace_title(&self, rule: usize) -> bool {
        !self.expected_clusters.is_empty()
            && self
                .expected_namespace_title
                .as_ref()
                .is_some_and(|paint| paint.rule_index == rule)
            && self.proves_complete_cluster_emission()
            && self.cluster_events.values().all(|event| {
                event
                    .title
                    .as_ref()
                    .is_some_and(|title| title.visible && title.inherits_paint)
            })
    }

    fn proves_complete_cluster_emission(&self) -> bool {
        !self.duplicate_expected_cluster
            && !self.duplicate_cluster_event
            && self.expected_clusters.len() == self.cluster_events.len()
            && self.expected_clusters.iter().all(|id| {
                self.cluster_events.get(id).is_some_and(|event| {
                    event.paint_matches
                        && match (&self.expected_namespace_title, &event.title) {
                            (None, None) => true,
                            (Some(_), Some(title)) => title.paint_matches,
                            _ => false,
                        }
                })
            })
    }

    pub(super) fn proves_typed_cluster_paint(
        &self,
        rule_index: usize,
        property: ResolvedStyleProperty,
    ) -> bool {
        let expected = match property {
            ResolvedStyleProperty::Fill => self.expected_cluster_fill.as_ref(),
            ResolvedStyleProperty::Stroke => self.expected_cluster_stroke.as_ref(),
            _ => None,
        };
        !self.expected_clusters.is_empty()
            && expected.is_some_and(|paint| paint.rule_index == rule_index)
            && self.proves_complete_cluster_emission()
    }

    pub(super) fn with_note_attachments(mut self, indices: Vec<usize>) -> Self {
        self.expected_note_attachments = indices;
        self
    }

    pub(super) fn expected_note_attachment_count(&self) -> usize {
        self.expected_note_attachments.len()
    }

    pub(crate) fn record_note_attachment(
        &mut self,
        note_index: usize,
        emitted_stroke: Option<(usize, &str)>,
        terminal_style: &str,
        hand_drawn_stroke: Option<&str>,
    ) {
        let event = ClassNoteAttachmentTerminalEvent {
            paint_matches: stroke_event_matches(
                self.expected_stroke.as_ref(),
                emitted_stroke,
                terminal_style,
            ) && hand_drawn_stroke_matches(
                self.expected_stroke.as_ref(),
                self.hand_drawn,
                hand_drawn_stroke,
            ),
        };
        if self
            .note_attachment_events
            .insert(note_index, event)
            .is_some()
        {
            self.duplicate_note_attachment_event = true;
        }
    }

    pub(super) fn with_edge_label_backgrounds(mut self, enabled: bool) -> Self {
        self.edge_label_background_events = enabled.then(BTreeMap::new);
        self
    }

    pub(crate) fn record_edge_label_background(&mut self, relation_index: usize, visible: bool) {
        if let Some(events) = &mut self.edge_label_background_events
            && events.insert(relation_index, visible).is_some()
        {
            self.duplicate_edge_label_background_event = true;
        }
    }

    pub(super) fn with_edge_label_background_paint(mut self, paint: Option<ExpectedPaint>) -> Self {
        self.expected_edge_label_background = paint;
        self
    }

    pub(crate) fn record_edge_label_background_paint(
        &mut self,
        relation_index: usize,
        rule: Option<usize>,
        styles: &[(&str, &str)],
    ) {
        if self.expected_edge_label_background.is_none() && rule.is_none() {
            return;
        }
        let valid = !styles.is_empty()
            && styles.iter().all(|(style, property)| {
                cluster_paint_event_matches(
                    self.expected_edge_label_background.as_ref(),
                    rule,
                    style,
                    property,
                )
            });
        if self
            .edge_label_background_paints
            .insert(relation_index, valid)
            .is_some()
        {
            self.duplicate_edge_label_background_event = true;
        }
    }

    pub(super) fn proves_edge_label_background(&self, rule_index: usize) -> bool {
        self.expected_edge_label_background
            .as_ref()
            .is_some_and(|paint| paint.rule_index == rule_index)
            && self
                .edge_label_background_count()
                .is_some_and(|count| count > 0 && self.edge_label_background_paints.len() == count)
            && self
                .edge_label_background_events
                .as_ref()
                .is_some_and(|events| {
                    events.iter().all(|(index, visible)| {
                        !visible || self.edge_label_background_paints.get(index) == Some(&true)
                    })
                })
    }

    pub(super) fn edge_label_background_count(&self) -> Option<usize> {
        let events = self.edge_label_background_events.as_ref()?;
        (self.proves_complete()
            && !self.duplicate_edge_label_background_event
            && events.len() == self.expected_relations.len()
            && self
                .expected_relations
                .iter()
                .all(|expected| events.contains_key(&expected.relation_index)))
        .then(|| events.values().filter(|visible| **visible).count())
    }

    pub(crate) fn with_nodes(mut self, expected_nodes: Vec<ClassNodeTerminalExpectation>) -> Self {
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
        let terminal_style = terminal_style.unwrap_or_default();
        self.marker_events.push(ClassMarkerTerminalEvent {
            name,
            fill_follows_stroke,
            paint_matches: stroke_event_matches(
                self.expected_stroke.as_ref(),
                emitted_stroke,
                terminal_style,
            ) && (!fill_follows_stroke
                || self.expected_stroke.as_ref().is_none_or(|stroke| {
                    terminal_paint(terminal_style, "fill") == Some(stroke.css.as_str())
                })),
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
        let event = ClassRelationTerminalEvent {
            start_marker,
            end_marker,
            emitted_stroke_rule: emitted_stroke.map(|(rule_index, _)| rule_index),
            paint_matches: stroke_event_matches(
                self.expected_stroke.as_ref(),
                emitted_stroke,
                terminal_style,
            ) && hand_drawn_stroke_matches(
                self.expected_stroke.as_ref(),
                self.hand_drawn,
                hand_drawn_stroke,
            ),
        };
        if self.relation_events.insert(relation_index, event).is_some() {
            self.duplicate_relation_event = true;
        }
        if typed_width_emitted {
            self.checkpointed_typed_width_paths =
                self.checkpointed_typed_width_paths.saturating_add(1);
        }
    }

    fn proves_complete_relation_emission(&self) -> bool {
        // Extracted namespace roots can emit relations before earlier source declarations.
        // Bind each checkpoint to its source identity, independently of SVG traversal order.
        !self.duplicate_relation_event
            && self
                .expected_relations
                .windows(2)
                .all(|pair| pair[0].relation_index < pair[1].relation_index)
            && self.relation_events.len() == self.expected_relations.len()
            && self.expected_relations.iter().all(|expected| {
                self.relation_events
                    .get(&expected.relation_index)
                    .is_some_and(|event| {
                        event.start_marker == expected.start_marker
                            && event.end_marker == expected.end_marker
                            && event.paint_matches
                    })
            })
    }

    fn proves_complete_note_attachment_emission(&self) -> bool {
        // Namespace groups may emit attachments in a different order from their declarations.
        !self.duplicate_note_attachment_event
            && self
                .expected_note_attachments
                .windows(2)
                .all(|pair| pair[0] < pair[1])
            && self.note_attachment_events.len() == self.expected_note_attachments.len()
            && self.expected_note_attachments.iter().all(|index| {
                self.note_attachment_events
                    .get(index)
                    .is_some_and(|event| event.paint_matches)
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
                        && event.paint_matches
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
        self.proves_complete_cluster_emission()
            && self.proves_complete_node_emission()
            && self.proves_complete_relation_emission()
            && self.proves_complete_note_attachment_emission()
            && self.proves_complete_marker_emission()
    }

    pub(super) fn terminal_summary(&self) -> ClassTerminalReceiptSummary {
        let complete_nodes = self.proves_complete_node_emission();
        let complete_relations = self.proves_complete_relation_emission();
        let complete_markers = self.proves_complete_marker_emission();
        let complete = self.proves_complete_cluster_emission()
            && complete_nodes
            && complete_relations
            && self.proves_complete_note_attachment_emission()
            && complete_markers;
        let visible_marker_occurrence_count = complete.then(|| {
            self.relation_events
                .values()
                .map(|event| {
                    usize::from(event.start_marker.is_some())
                        .saturating_add(usize::from(event.end_marker.is_some()))
                })
                .sum()
        });
        let mut winner_rules = BTreeSet::new();
        let mut effective_rules = BTreeSet::new();
        let mut typed_rejections = BTreeSet::new();
        for expected in self.expected_nodes.values() {
            for &(target, property) in &NODE_PAINT_TARGETS {
                if let Some(rule_index) = expected.winner_rule(target, property) {
                    winner_rules.insert((rule_index, target, property));
                }
                let Some(expected_paint) = expected.expected_paint(target, property) else {
                    continue;
                };
                let key = (expected_paint.rule_index, target, property);
                let Some(event) = self.node_events.get(expected.id()) else {
                    typed_rejections.insert(key);
                    continue;
                };
                let paint = node_paint_emission(event, target, property);
                if paint.terminal_applicable && !paint.source_owned {
                    effective_rules.insert(key);
                }
                if !(!paint.terminal_applicable || paint.source_owned || paint.terminal_verified) {
                    typed_rejections.insert(key);
                }
            }
        }
        let typed_rules = effective_rules
            .iter()
            .copied()
            .filter(|key| complete_nodes && !typed_rejections.contains(key))
            .collect();

        ClassTerminalReceiptSummary {
            complete,
            complete_nodes,
            visible_marker_occurrence_count,
            winner_rules,
            effective_rules,
            typed_rules,
            work_units: self
                .expected_nodes
                .len()
                .saturating_add(self.node_events.len())
                .saturating_mul(NODE_PAINT_TARGETS.len())
                .saturating_add(self.expected_clusters.len().saturating_mul(2))
                .saturating_add(self.cluster_events.len().saturating_mul(2))
                .saturating_add(if self.expected_namespace_title.is_some() {
                    self.expected_clusters.len().saturating_mul(2)
                } else {
                    0
                })
                .saturating_add(self.expected_relations.len())
                .saturating_add(self.relation_events.len())
                .saturating_add(self.expected_note_attachments.len())
                .saturating_add(self.note_attachment_events.len())
                .saturating_add(self.expected_markers.len())
                .saturating_add(self.marker_events.len())
                .saturating_add(
                    self.edge_label_background_events
                        .as_ref()
                        .map_or(0, BTreeMap::len),
                )
                .saturating_add(self.edge_label_background_paints.len())
                .max(1),
        }
    }

    pub(super) fn proves_typed_width(&self) -> bool {
        self.proves_complete_relation_emission()
            && self.checkpointed_typed_width_paths == self.expected_relations.len()
    }

    fn has_effective_stroke_rule(
        &self,
        rule_index: usize,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.expected_stroke.as_ref().is_some_and(|expected| {
            expected.rule_index == rule_index && expected.property == property
        }) && (!self.expected_relations.is_empty() || !self.expected_note_attachments.is_empty())
    }

    pub(super) fn proves_typed_stroke(
        &self,
        rule_index: usize,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.proves_complete()
            && self.has_effective_stroke_rule(rule_index, property)
            && self
                .relation_events
                .values()
                .all(|event| event.emitted_stroke_rule == Some(rule_index))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClassTerminalReceiptSummary {
    complete: bool,
    complete_nodes: bool,
    visible_marker_occurrence_count: Option<usize>,
    winner_rules: BTreeSet<(usize, ThemeTarget, ResolvedStyleProperty)>,
    effective_rules: BTreeSet<(usize, ThemeTarget, ResolvedStyleProperty)>,
    typed_rules: BTreeSet<(usize, ThemeTarget, ResolvedStyleProperty)>,
    work_units: usize,
}

impl ClassTerminalReceiptSummary {
    pub(super) fn is_complete(&self) -> bool {
        self.complete
    }

    pub(super) fn visible_marker_occurrence_count(&self) -> Option<usize> {
        self.visible_marker_occurrence_count
    }

    pub(super) fn has_node_paint_winner_rule(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.winner_rules.contains(&(rule_index, target, property))
    }

    pub(super) fn has_effective_node_paint_rule(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.effective_rules
            .contains(&(rule_index, target, property))
    }

    pub(super) fn proves_typed_node_paint(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.complete_nodes && self.typed_rules.contains(&(rule_index, target, property))
    }

    pub(super) fn work_units(&self) -> usize {
        self.work_units
    }
}

const NODE_PAINT_TARGETS: [(ThemeTarget, ResolvedStyleProperty); 3] = [
    (ThemeTarget::Node, ResolvedStyleProperty::Fill),
    (ThemeTarget::Node, ResolvedStyleProperty::Stroke),
    (ThemeTarget::NodeLabel, ResolvedStyleProperty::Fill),
];

#[derive(Debug, Clone)]
struct ClassNamespaceTitleTerminalEvent {
    paint_matches: bool,
    visible: bool,
    inherits_paint: bool,
}

#[derive(Debug, Clone)]
struct ClassClusterTerminalEvent {
    title: Option<ClassNamespaceTitleTerminalEvent>,
    paint_matches: bool,
}

fn cluster_paint_event_matches(
    expected: Option<&ExpectedPaint>,
    rule: Option<usize>,
    style: &str,
    property: &str,
) -> bool {
    match (expected, rule) {
        (None, None) => terminal_paint(style, property).is_none(),
        (Some(paint), Some(rule)) => {
            paint.rule_index == rule && terminal_paint(style, property) == Some(paint.css.as_str())
        }
        (None, Some(_)) | (Some(_), None) => false,
    }
}

#[derive(Debug, Clone)]
struct ClassRelationTerminalEvent {
    start_marker: Option<&'static str>,
    end_marker: Option<&'static str>,
    emitted_stroke_rule: Option<usize>,
    paint_matches: bool,
}

#[derive(Debug, Clone)]
struct ClassNoteAttachmentTerminalEvent {
    paint_matches: bool,
}

#[derive(Debug, Clone)]
struct ClassMarkerTerminalEvent {
    name: &'static str,
    fill_follows_stroke: bool,
    paint_matches: bool,
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
    emitted: Option<(usize, &str)>,
    terminal_style: &str,
) -> bool {
    match (expected, emitted) {
        (None, None) => true,
        (Some(expected), Some((rule_index, css))) => {
            expected.rule_index == rule_index
                && expected.css == css
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

pub(super) fn terminal_paint<'a>(style: &'a str, property: &str) -> Option<&'a str> {
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
            property: ResolvedStyleProperty::Stroke,
            css: "#123456".to_string(),
        })
    }

    fn expected_paint(rule_index: usize, css: &str) -> Option<ExpectedPaint> {
        Some(ExpectedPaint {
            target: ThemeTarget::Node,
            rule_index,
            css: css.to_string(),
        })
    }

    fn cluster_receipt(
        ids: Vec<String>,
        fill: Option<ExpectedPaint>,
        stroke: Option<ExpectedPaint>,
        namespace_title: Option<ExpectedPaint>,
    ) -> ClassRelationThemeReceipt {
        ClassRelationThemeReceipt::new_with_clusters(
            Vec::new(),
            Vec::new(),
            None,
            false,
            ClassClusterTerminalExpectation {
                ids,
                fill,
                stroke,
                namespace_title,
            },
        )
    }

    fn node_paint(
        source_owned: bool,
        emitted: Option<(usize, &str)>,
        terminal_value: &str,
    ) -> ClassNodePaintTerminalEmission {
        ClassNodePaintTerminalEmission::new(source_owned, emitted, terminal_value)
    }

    #[test]
    fn cluster_receipt_requires_exact_ids_and_each_final_paint_facet() {
        let expected = || {
            cluster_receipt(
                vec!["Outer".into(), "Inner".into()],
                expected_paint(3, "#123456"),
                expected_paint(4, "transparent"),
                None,
            )
        };
        let record = |receipt: &mut ClassRelationThemeReceipt, id: &str| {
            receipt.record_cluster(id, Some(3), Some(4), "fill:#123456;stroke:transparent;");
        };
        let mut complete = expected();
        record(&mut complete, "Inner");
        assert!(!complete.proves_complete());
        record(&mut complete, "Outer");
        assert!(complete.proves_complete());
        assert!(complete.proves_typed_cluster_paint(3, ResolvedStyleProperty::Fill));
        assert!(complete.proves_typed_cluster_paint(4, ResolvedStyleProperty::Stroke));
        assert!(!complete.proves_typed_cluster_paint(3, ResolvedStyleProperty::Stroke));
        assert!(!complete.proves_typed_cluster_paint(9, ResolvedStyleProperty::Fill));

        let mut duplicate = complete.clone();
        record(&mut duplicate, "Inner");
        assert!(!duplicate.proves_complete());
        let mut wrong_id = expected();
        record(&mut wrong_id, "Outer");
        record(&mut wrong_id, "Other");
        assert!(!wrong_id.proves_complete());
        let duplicate_expectation =
            cluster_receipt(vec!["Outer".into(), "Outer".into()], None, None, None);
        assert!(!duplicate_expectation.proves_complete());

        for (fill_rule, stroke_rule, style) in [
            (None, Some(4), "stroke:transparent;"),
            (Some(3), None, "fill:#123456;"),
            (Some(9), Some(4), "fill:#123456;stroke:transparent;"),
            (Some(3), Some(9), "fill:#123456;stroke:transparent;"),
            (Some(3), Some(4), "fill:#654321;stroke:transparent;"),
            (Some(3), Some(4), "fill:#123456;stroke:#654321;"),
            (
                Some(3),
                Some(4),
                "fill:#123456;stroke:transparent;fill:#654321;",
            ),
            (Some(3), Some(4), "fill:#123456stroke:transparent;"),
        ] {
            let mut receipt = expected();
            record(&mut receipt, "Outer");
            receipt.record_cluster("Inner", fill_rule, stroke_rule, style);
            assert!(
                !receipt.proves_complete(),
                "accepted invalid cluster style {style:?}"
            );
            assert!(!receipt.proves_typed_cluster_paint(3, ResolvedStyleProperty::Fill));
        }
    }

    #[test]
    fn cluster_receipt_keeps_empty_paint_and_sibling_ownership_independent() {
        let mut empty = cluster_receipt(vec!["Outer".into()], None, None, None);
        empty.record_cluster("Outer", None, None, "");
        assert!(empty.proves_complete());
        assert!(!empty.proves_typed_cluster_paint(0, ResolvedStyleProperty::Fill));
        let mut fill_only = cluster_receipt(
            vec!["Outer".into()],
            expected_paint(0, "#123456"),
            None,
            None,
        );
        fill_only.record_cluster("Outer", Some(0), None, "fill:#123456;");
        assert!(fill_only.proves_complete());
        assert!(fill_only.proves_typed_cluster_paint(0, ResolvedStyleProperty::Fill));
        assert!(!fill_only.proves_typed_cluster_paint(0, ResolvedStyleProperty::Stroke));
    }

    #[test]
    fn unthemed_cluster_receipt_rejects_unexpected_paint() {
        for (fill_rule, stroke_rule, style) in [
            (None, None, "fill:#123456;"),
            (None, None, "stroke:#123456;"),
            (Some(3), None, "fill:#123456;"),
            (None, Some(3), "stroke:#123456;"),
        ] {
            let mut receipt = cluster_receipt(vec!["Outer".into()], None, None, None);
            receipt.record_cluster("Outer", fill_rule, stroke_rule, style);
            assert!(!receipt.proves_complete(), "accepted {style}");
        }
    }

    #[test]
    fn unthemed_paths_still_require_the_matching_hand_drawn_checkpoint() {
        for hand_drawn in [false, true] {
            for emitted in [None, Some("#123456")] {
                let mut relation = ClassRelationThemeReceipt::new(
                    vec![ClassRelationTerminalExpectation::new(0, None, None)],
                    Vec::new(),
                    None,
                    hand_drawn,
                );
                relation.record_relation(0, None, None, None, "", emitted, false);
                assert_eq!(relation.proves_complete(), hand_drawn == emitted.is_some());
                let mut note =
                    ClassRelationThemeReceipt::new(Vec::new(), Vec::new(), None, hand_drawn)
                        .with_note_attachments(vec![0]);
                note.record_note_attachment(0, None, "", emitted);
                assert_eq!(note.proves_complete(), hand_drawn == emitted.is_some());
            }
        }
    }

    #[test]
    fn relation_receipt_only_proves_the_source_paint_property() {
        for property in [ResolvedStyleProperty::Fill, ResolvedStyleProperty::Stroke] {
            let mut receipt = ClassRelationThemeReceipt::new(
                vec![ClassRelationTerminalExpectation::new(0, None, None)],
                Vec::new(),
                Some(ExpectedStroke {
                    rule_index: 3,
                    property,
                    css: "#123456".to_string(),
                }),
                false,
            );
            assert!(!receipt.proves_typed_stroke(3, property));
            receipt.record_relation(
                0,
                None,
                None,
                Some((3, "#123456")),
                "stroke:#123456 !important",
                None,
                false,
            );
            assert!(receipt.proves_typed_stroke(3, property));
            let sibling = if property == ResolvedStyleProperty::Fill {
                ResolvedStyleProperty::Stroke
            } else {
                ResolvedStyleProperty::Fill
            };
            assert!(!receipt.proves_typed_stroke(3, sibling));
            assert!(!receipt.proves_typed_stroke(4, property));
        }
    }

    #[test]
    fn note_attachment_receipt_binds_source_indices_paint_and_hand_drawn_checkpoint() {
        for property in [ResolvedStyleProperty::Fill, ResolvedStyleProperty::Stroke] {
            for hand_drawn in [false, true] {
                let receipt = || {
                    ClassRelationThemeReceipt::new(
                        Vec::new(),
                        Vec::new(),
                        Some(ExpectedStroke {
                            rule_index: 3,
                            property,
                            css: "#123456".to_string(),
                        }),
                        hand_drawn,
                    )
                    .with_note_attachments(vec![1, 3])
                };
                let record = |receipt: &mut ClassRelationThemeReceipt, index| {
                    receipt.record_note_attachment(
                        index,
                        Some((3, "#123456")),
                        "stroke:#123456 !important",
                        hand_drawn.then_some("#123456"),
                    );
                };
                let mut complete = receipt();
                assert!(!complete.proves_complete());
                record(&mut complete, 1);
                assert!(!complete.proves_typed_stroke(3, property));
                record(&mut complete, 3);
                assert!(complete.proves_complete());
                assert!(complete.proves_typed_stroke(3, property));
                let sibling = if property == ResolvedStyleProperty::Fill {
                    ResolvedStyleProperty::Stroke
                } else {
                    ResolvedStyleProperty::Fill
                };
                assert!(!complete.proves_typed_stroke(3, sibling));
                assert!(!complete.proves_typed_stroke(4, property));
                record(&mut complete, 3);
                assert!(!complete.proves_complete());

                let mut reordered = receipt();
                record(&mut reordered, 3);
                record(&mut reordered, 1);
                assert!(reordered.proves_typed_stroke(3, property));

                for indices in [[1, 1], [0, 1]] {
                    let mut wrong = receipt();
                    for index in indices {
                        record(&mut wrong, index);
                    }
                    assert!(!wrong.proves_complete(), "indices: {indices:?}");
                }
                for (emitted, terminal, rough) in [
                    (
                        None,
                        "stroke:#123456 !important",
                        hand_drawn.then_some("#123456"),
                    ),
                    (
                        Some((4, "#123456")),
                        "stroke:#123456 !important",
                        hand_drawn.then_some("#123456"),
                    ),
                    (
                        Some((3, "#abcdef")),
                        "stroke:#123456 !important",
                        hand_drawn.then_some("#123456"),
                    ),
                    (
                        Some((3, "#123456")),
                        "stroke:#abcdef !important",
                        hand_drawn.then_some("#123456"),
                    ),
                    (
                        Some((3, "#123456")),
                        "stroke:#123456 !important",
                        if hand_drawn { None } else { Some("#123456") },
                    ),
                    (
                        Some((3, "#123456")),
                        "stroke:#123456 !important",
                        Some("#abcdef"),
                    ),
                ] {
                    let mut wrong = receipt();
                    record(&mut wrong, 1);
                    wrong.record_note_attachment(3, emitted, terminal, rough);
                    assert!(!wrong.proves_complete());
                    assert!(!wrong.proves_typed_stroke(3, property));
                }
            }
        }
    }

    #[test]
    fn note_attachment_paint_does_not_expand_relation_width_or_marker_domains() {
        let mut receipt = ClassRelationThemeReceipt::new(
            vec![ClassRelationTerminalExpectation::new(0, None, None)],
            Vec::new(),
            expected_stroke(),
            false,
        )
        .with_note_attachments(vec![2]);
        receipt.record_relation(
            0,
            None,
            None,
            Some((3, "#123456")),
            "stroke:#123456 !important;stroke-width:6px !important",
            None,
            true,
        );
        assert!(receipt.proves_typed_width());
        assert!(!receipt.proves_typed_stroke(3, ResolvedStyleProperty::Stroke));
        receipt.record_note_attachment(2, Some((3, "#123456")), "stroke:#123456 !important", None);
        assert!(receipt.proves_complete());
        assert!(receipt.proves_typed_width());
        assert_eq!(
            receipt.terminal_summary().visible_marker_occurrence_count(),
            Some(0)
        );
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
        let summary = receipt.terminal_summary();
        assert!(
            summary.proves_typed_node_paint(1, ThemeTarget::Node, ResolvedStyleProperty::Fill,)
        );
        assert!(summary.proves_typed_node_paint(
            1,
            ThemeTarget::Node,
            ResolvedStyleProperty::Stroke,
        ));
        assert!(summary.proves_typed_node_paint(
            2,
            ThemeTarget::NodeLabel,
            ResolvedStyleProperty::Fill,
        ));

        assert!(summary.is_complete());
        assert!(summary.has_node_paint_winner_rule(
            1,
            ThemeTarget::Node,
            ResolvedStyleProperty::Fill,
        ));
        assert!(summary.has_effective_node_paint_rule(
            1,
            ThemeTarget::Node,
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
        let summary = receipt.terminal_summary();
        assert!(summary.has_node_paint_winner_rule(
            4,
            ThemeTarget::Node,
            ResolvedStyleProperty::Fill,
        ));
        assert!(summary.has_node_paint_winner_rule(
            5,
            ThemeTarget::NodeLabel,
            ResolvedStyleProperty::Fill,
        ));
        assert!(!summary.proves_typed_node_paint(
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
    fn reordered_relations_preserve_identity_marker_and_paint_checks() {
        for case in [
            "valid",
            "missing",
            "duplicate",
            "foreign",
            "marker",
            "rule",
            "paint",
            "duplicate-expected",
        ] {
            let mut relations = vec![
                ClassRelationTerminalExpectation::new(0, Some("compositionStart"), None),
                ClassRelationTerminalExpectation::new(1, None, Some("extensionEnd")),
            ];
            if case == "duplicate-expected" {
                relations[1] = relations[0].clone();
            }
            let mut receipt = ClassRelationThemeReceipt::new(
                relations,
                vec![
                    ClassMarkerTerminalExpectation::new("extensionEnd", false),
                    ClassMarkerTerminalExpectation::new("compositionStart", true),
                ],
                expected_stroke(),
                false,
            );
            record_valid_markers(&mut receipt);
            record_relation(&mut receipt, 1, None, Some("extensionEnd"));
            if case != "missing" {
                let index = match case {
                    "duplicate" => 1,
                    "foreign" => 2,
                    _ => 0,
                };
                receipt.record_relation(
                    index,
                    Some(if case == "marker" {
                        "extensionEnd"
                    } else {
                        "compositionStart"
                    }),
                    None,
                    Some((if case == "rule" { 4 } else { 3 }, "#123456")),
                    if case == "paint" {
                        "stroke:#abcdef !important;"
                    } else {
                        "stroke:#123456 !important;"
                    },
                    None,
                    false,
                );
            }
            assert_eq!(receipt.proves_complete(), case == "valid", "{case}");
            assert_eq!(
                receipt.proves_typed_stroke(3, ResolvedStyleProperty::Stroke),
                case == "valid",
                "{case}"
            );
        }
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
    fn background_domain_requires_every_source_label_checkpoint() {
        let mut receipt = ClassRelationThemeReceipt::new(
            vec![
                ClassRelationTerminalExpectation::new(0, None, None),
                ClassRelationTerminalExpectation::new(1, None, None),
            ],
            Vec::new(),
            None,
            false,
        )
        .with_edge_label_backgrounds(true);
        for index in 0..2 {
            receipt.record_relation(index, None, None, None, "", None, false);
        }
        assert_eq!(receipt.edge_label_background_count(), None);
        receipt.record_edge_label_background(1, true);
        assert_eq!(receipt.edge_label_background_count(), None);
        receipt.record_edge_label_background(0, false);
        assert_eq!(receipt.edge_label_background_count(), Some(1));
        let mut duplicate = receipt.clone();
        duplicate.record_edge_label_background(0, true);
        assert_eq!(duplicate.edge_label_background_count(), None);
        receipt.record_edge_label_background(2, true);
        assert_eq!(receipt.edge_label_background_count(), None);

        let mut untracked = ClassRelationThemeReceipt::new(Vec::new(), Vec::new(), None, false);
        untracked.record_edge_label_background(0, true);
        assert!(untracked.edge_label_background_events.is_none());
        assert_eq!(untracked.edge_label_background_count(), None);
        assert_eq!(
            untracked
                .with_edge_label_backgrounds(true)
                .edge_label_background_count(),
            Some(0)
        );
    }

    #[test]
    fn background_paint_requires_every_visible_terminal_and_both_html_surfaces() {
        let mut baseline = ClassRelationThemeReceipt::new(
            vec![ClassRelationTerminalExpectation::new(0, None, None)],
            Vec::new(),
            None,
            false,
        )
        .with_edge_label_backgrounds(true)
        .with_edge_label_background_paint(Some(ExpectedPaint {
            target: ThemeTarget::EdgeLabelBackground,
            rule_index: 7,
            css: "#020617".to_owned(),
        }));
        baseline.record_relation(0, None, None, None, "", None, false);
        baseline.record_edge_label_background(0, true);
        assert!(!baseline.proves_edge_label_background(7));
        let mut extra = baseline.clone();
        extra.record_edge_label_background_paint(0, Some(7), &[("fill:#020617;", "fill")]);
        assert!(extra.proves_edge_label_background(7));
        extra.record_edge_label_background_paint(99, Some(7), &[("fill:#020617;", "fill")]);
        assert!(!extra.proves_edge_label_background(7));
        for (rule, styles, expected) in [
            (
                Some(7),
                vec![("background-color:#020617 !important;", "background-color"); 2],
                true,
            ),
            (
                Some(7),
                vec![
                    ("background-color:#020617 !important;", "background-color"),
                    ("", "background-color"),
                ],
                false,
            ),
            (Some(7), vec![("fill:#ffffff;", "fill")], false),
            (Some(8), vec![("fill:#020617;", "fill")], false),
            (None, vec![("fill:#020617;", "fill")], false),
            (Some(7), vec![], false),
        ] {
            let mut receipt = baseline.clone();
            receipt.record_edge_label_background_paint(0, rule, &styles);
            assert_eq!(receipt.proves_edge_label_background(7), expected);
            receipt.record_edge_label_background_paint(0, rule, &styles);
            assert!(
                !receipt.proves_edge_label_background(7),
                "duplicate checkpoint must invalidate evidence"
            );
        }
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
        assert_eq!(
            receipt.terminal_summary().visible_marker_occurrence_count(),
            Some(2)
        );

        let incomplete =
            ClassRelationThemeReceipt::new(relations, Vec::new(), expected_stroke(), false);
        assert_eq!(
            incomplete
                .terminal_summary()
                .visible_marker_occurrence_count(),
            None
        );
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
    #[test]
    fn namespace_title_receipt_requires_the_complete_visible_color_terminal() {
        let expected = || {
            let mut receipt = cluster_receipt(
                vec!["Outer".into()],
                None,
                None,
                expected_paint(3, "#123456"),
            );
            receipt.record_cluster("Outer", None, None, "");
            receipt
        };
        let mut complete = expected();
        assert!(!complete.proves_complete());
        complete.record_namespace_title(
            "Outer",
            3,
            "color:#123456 !important;fill:#123456 !important;",
            true,
            true,
        );
        assert!(complete.proves_complete());
        assert!(complete.proves_typed_namespace_title(3));
        assert!(!complete.proves_typed_namespace_title(4));
        for (id, rule, style, visible, inherits) in [
            ("Foreign", 3, "color:#123456;fill:#123456;", true, true),
            ("Outer", 4, "color:#123456;fill:#123456;", true, true),
            ("Outer", 3, "fill:#123456;", true, true),
            ("Outer", 3, "color:#123456;", true, true),
            ("Outer", 3, "color:#123456;fill:#654321;", true, true),
        ] {
            let mut receipt = expected();
            receipt.record_namespace_title(id, rule, style, visible, inherits);
            assert!(
                !receipt.proves_complete(),
                "accepted {id} {rule} {style} {visible} {inherits}"
            );
        }
        for (visible, inherits) in [(false, true), (true, false)] {
            let mut receipt = expected();
            receipt.record_namespace_title(
                "Outer",
                3,
                "color:#123456;fill:#123456;",
                visible,
                inherits,
            );
            assert!(
                receipt.proves_complete(),
                "a valid terminal can lack paint evidence"
            );
            assert!(!receipt.proves_typed_namespace_title(3));
        }
        complete.record_namespace_title("Outer", 3, "color:#123456;fill:#123456;", true, true);
        assert!(!complete.proves_complete());
    }
}
