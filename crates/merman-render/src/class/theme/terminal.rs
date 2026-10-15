use std::borrow::Cow;
use std::collections::BTreeSet;

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

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct ClassNodePaintTerminalEmission {
    terminal_applicable: bool,
    terminal_verified: bool,
    source_owned: bool,
    source_value: Option<String>,
    emitted: Option<(usize, String)>,
    terminal_value: String,
}

#[cfg(test)]
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
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct ClassNodeTerminalEmission {
    id: String,
    fill: ClassNodePaintTerminalEmission,
    stroke: ClassNodePaintTerminalEmission,
    label_fill: ClassNodePaintTerminalEmission,
}

#[cfg(test)]
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

/// Borrowed terminal values are checked before retaining compact ownership flags.
pub(crate) struct ClassNodePaintTerminal<'a> {
    terminal_applicable: bool,
    terminal_verified: bool,
    source_owned: bool,
    source_value: Option<&'a str>,
    emitted: Option<(usize, &'a str)>,
    terminal_value: &'a str,
}

impl<'a> ClassNodePaintTerminal<'a> {
    pub(crate) fn new(
        source_owned: bool,
        emitted: Option<(usize, &'a str)>,
        terminal_value: &'a str,
    ) -> Self {
        Self {
            terminal_applicable: true,
            terminal_verified: true,
            source_owned,
            source_value: None,
            emitted,
            terminal_value,
        }
    }
    pub(crate) fn not_applicable() -> Self {
        Self {
            terminal_applicable: false,
            terminal_verified: true,
            source_owned: false,
            source_value: None,
            emitted: None,
            terminal_value: "",
        }
    }
    pub(crate) fn with_source_value(mut self, source: Option<&'a str>) -> Self {
        self.source_value = source;
        self
    }
    pub(crate) fn with_terminal_verified(mut self, verified: bool) -> Self {
        self.terminal_verified = verified;
        self
    }
    fn flags(&self) -> ClassNodePaintFlags {
        ClassNodePaintFlags {
            terminal_applicable: self.terminal_applicable,
            terminal_verified: self.terminal_verified,
            source_owned: self.source_owned,
        }
    }
}

#[cfg(test)]
impl ClassNodePaintTerminalEmission {
    fn terminal(&self) -> ClassNodePaintTerminal<'_> {
        ClassNodePaintTerminal {
            terminal_applicable: self.terminal_applicable,
            terminal_verified: self.terminal_verified,
            source_owned: self.source_owned,
            source_value: self.source_value.as_deref(),
            emitted: self
                .emitted
                .as_ref()
                .map(|(index, css)| (*index, css.as_str())),
            terminal_value: &self.terminal_value,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ClassNodePaintFlags {
    terminal_applicable: bool,
    terminal_verified: bool,
    source_owned: bool,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ClassNodeTerminalObservation {
    slot: usize,
    values_match: bool,
    paints: [ClassNodePaintFlags; 3],
}

impl ClassNodeTerminalObservation {
    pub(crate) fn with_terminal_value_check(mut self, matches: bool) -> Self {
        self.values_match &= matches;
        self
    }
}

impl ClassNodeTerminalExpectation {
    pub(crate) fn observe(
        &self,
        slot: usize,
        fill: ClassNodePaintTerminal<'_>,
        stroke: ClassNodePaintTerminal<'_>,
        label: ClassNodePaintTerminal<'_>,
    ) -> ClassNodeTerminalObservation {
        ClassNodeTerminalObservation {
            slot,
            values_match: node_paint_event_matches(self.fill.as_ref(), &fill)
                && node_paint_event_matches(self.stroke.as_ref(), &stroke)
                && node_label_paint_event_matches(self.label_fill.as_ref(), &label),
            paints: [fill.flags(), stroke.flags(), label.flags()],
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

/// Borrowed values from one relation's canonical SVG terminal checkpoint.
pub(crate) struct ClassRelationTerminalEvent<'a> {
    pub(crate) relation_index: usize,
    pub(crate) start_marker: Option<&'static str>,
    pub(crate) end_marker: Option<&'static str>,
    pub(crate) emitted_stroke: Option<(usize, &'a str)>,
    pub(crate) terminal_style: &'a str,
    pub(crate) hand_drawn_stroke: Option<&'a str>,
    pub(crate) typed_width_emitted: bool,
}

/// Cluster expectations are fixed before any terminal checkpoint is recorded.
#[derive(Debug, Default)]
pub(super) struct ClassClusterTerminalExpectation {
    pub(super) ids: Vec<String>,
    pub(super) fill: Option<ExpectedPaint>,
    pub(super) stroke: Option<ExpectedPaint>,
    pub(super) namespace_title: Option<ExpectedPaint>,
}

/// Canonical slots retain exactly-once completion after checking terminal values immediately.
#[derive(Debug, Clone)]
pub(crate) struct ClassRelationThemeReceipt<'nodes> {
    expected_clusters: Vec<String>,
    cluster_domain_valid: bool,
    expected_cluster_fill: Option<ExpectedPaint>,
    expected_cluster_stroke: Option<ExpectedPaint>,
    expected_namespace_title: Option<ExpectedPaint>,
    cluster_events: Vec<Option<ClassClusterTerminalEvent>>,
    clusters_valid: bool,
    expected_nodes: Cow<'nodes, [ClassNodeTerminalExpectation]>,
    node_domain_valid: bool,
    nodes_seen: Vec<bool>,
    nodes_valid: bool,
    winner_rules: BTreeSet<(usize, ThemeTarget, ResolvedStyleProperty)>,
    effective_rules: BTreeSet<(usize, ThemeTarget, ResolvedStyleProperty)>,
    typed_rejections: BTreeSet<(usize, ThemeTarget, ResolvedStyleProperty)>,
    expected_relations: Vec<ClassRelationTerminalExpectation>,
    relations_seen: Vec<bool>,
    relations_valid: bool,
    expected_note_attachments: Vec<usize>,
    notes_seen: Vec<bool>,
    notes_valid: bool,
    expected_markers: Vec<ClassMarkerTerminalExpectation>,
    marker_count: usize,
    markers_valid: bool,
    expected_stroke: Option<ExpectedStroke>,
    strokes_valid: bool,
    hand_drawn: bool,
    checkpointed_typed_width_paths: usize,
    edge_label_background_events: Option<Vec<Option<bool>>>,
    expected_edge_label_background: Option<ExpectedPaint>,
    edge_label_background_paints: Vec<Option<bool>>,
    edge_label_background_valid: bool,
}

impl<'nodes> ClassRelationThemeReceipt<'nodes> {
    #[cfg(test)]
    pub(super) fn new(
        relations: Vec<ClassRelationTerminalExpectation>,
        markers: Vec<ClassMarkerTerminalExpectation>,
        stroke: Option<ExpectedStroke>,
        hand_drawn: bool,
    ) -> Self {
        Self::new_with_clusters(
            relations,
            markers,
            stroke,
            hand_drawn,
            ClassClusterTerminalExpectation::default(),
        )
    }

    pub(super) fn new_with_clusters(
        expected_relations: Vec<ClassRelationTerminalExpectation>,
        expected_markers: Vec<ClassMarkerTerminalExpectation>,
        expected_stroke: Option<ExpectedStroke>,
        hand_drawn: bool,
        mut clusters: ClassClusterTerminalExpectation,
    ) -> Self {
        clusters.ids.sort_unstable();
        let cluster_domain_valid = clusters.ids.windows(2).all(|pair| pair[0] != pair[1]);
        let relations_valid = expected_relations
            .windows(2)
            .all(|pair| pair[0].relation_index < pair[1].relation_index);
        Self {
            cluster_events: vec![None; clusters.ids.len()],
            expected_clusters: clusters.ids,
            cluster_domain_valid,
            expected_cluster_fill: clusters.fill,
            expected_cluster_stroke: clusters.stroke,
            expected_namespace_title: clusters.namespace_title,
            clusters_valid: true,
            expected_nodes: Cow::Borrowed(&[]),
            node_domain_valid: true,
            nodes_seen: Vec::new(),
            nodes_valid: true,
            winner_rules: BTreeSet::new(),
            effective_rules: BTreeSet::new(),
            typed_rejections: BTreeSet::new(),
            relations_seen: vec![false; expected_relations.len()],
            expected_relations,
            relations_valid,
            expected_note_attachments: Vec::new(),
            notes_seen: Vec::new(),
            notes_valid: true,
            expected_markers,
            marker_count: 0,
            markers_valid: true,
            expected_stroke,
            strokes_valid: true,
            hand_drawn,
            checkpointed_typed_width_paths: 0,
            edge_label_background_events: None,
            expected_edge_label_background: None,
            edge_label_background_paints: Vec::new(),
            edge_label_background_valid: true,
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
        style: &str,
    ) {
        let Ok(slot) = self
            .expected_clusters
            .binary_search_by(|expected| expected.as_str().cmp(id))
        else {
            self.clusters_valid = false;
            return;
        };
        let valid = cluster_paint_event_matches(
            self.expected_cluster_fill.as_ref(),
            fill_rule,
            style,
            "fill",
        ) && cluster_paint_event_matches(
            self.expected_cluster_stroke.as_ref(),
            stroke_rule,
            style,
            "stroke",
        );
        self.clusters_valid &= valid && self.cluster_events[slot].is_none();
        self.cluster_events[slot] = Some(ClassClusterTerminalEvent {
            title: None,
            paint_matches: valid,
        });
    }

    pub(crate) fn record_namespace_title(
        &mut self,
        id: &str,
        rule: usize,
        style: &str,
        visible: bool,
        inherits_paint: bool,
    ) {
        let cluster = self
            .expected_clusters
            .binary_search_by(|expected| expected.as_str().cmp(id))
            .ok()
            .and_then(|slot| self.cluster_events[slot].as_mut());
        let Some(cluster) = cluster else {
            self.clusters_valid = false;
            return;
        };
        self.clusters_valid &= cluster.title.is_none();
        let paint_matches = self
            .expected_namespace_title
            .as_ref()
            .is_some_and(|expected| {
                rule == expected.rule_index
                    && terminal_paint(style, "color") == Some(expected.css.as_str())
                    && terminal_paint(style, "fill") == Some(expected.css.as_str())
            });
        self.clusters_valid &= paint_matches;
        cluster.title = Some(ClassNamespaceTitleTerminalEvent {
            paint_matches,
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
            && self.cluster_events.iter().all(|event| {
                event
                    .as_ref()
                    .and_then(|event| event.title.as_ref())
                    .is_some_and(|title| title.visible && title.inherits_paint)
            })
    }

    fn proves_complete_cluster_emission(&self) -> bool {
        self.cluster_domain_valid
            && self.clusters_valid
            && self.cluster_events.iter().all(|event| {
                event.as_ref().is_some_and(|event| {
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
        rule: usize,
        property: ResolvedStyleProperty,
    ) -> bool {
        let paint = match property {
            ResolvedStyleProperty::Fill => self.expected_cluster_fill.as_ref(),
            ResolvedStyleProperty::Stroke => self.expected_cluster_stroke.as_ref(),
            _ => None,
        };
        !self.expected_clusters.is_empty()
            && paint.is_some_and(|paint| paint.rule_index == rule)
            && self.proves_complete_cluster_emission()
    }

    pub(super) fn with_note_attachments(mut self, indices: Vec<usize>) -> Self {
        self.notes_valid = indices.windows(2).all(|pair| pair[0] < pair[1]);
        self.notes_seen = vec![false; indices.len()];
        self.expected_note_attachments = indices;
        self
    }
    pub(super) fn expected_note_attachment_count(&self) -> usize {
        self.expected_note_attachments.len()
    }

    pub(crate) fn record_note_attachment(
        &mut self,
        index: usize,
        emitted: Option<(usize, &str)>,
        style: &str,
        hand_drawn: Option<&str>,
    ) {
        let Ok(slot) = self.expected_note_attachments.binary_search(&index) else {
            self.notes_valid = false;
            return;
        };
        self.notes_valid &= !self.notes_seen[slot]
            && stroke_event_matches(self.expected_stroke.as_ref(), emitted, style)
            && hand_drawn_stroke_matches(
                self.expected_stroke.as_ref(),
                self.hand_drawn,
                hand_drawn,
            );
        self.notes_seen[slot] = true;
    }

    pub(super) fn with_edge_label_backgrounds(mut self, enabled: bool) -> Self {
        self.edge_label_background_events =
            enabled.then(|| vec![None; self.expected_relations.len()]);
        self
    }
    fn relation_slot(&self, index: usize) -> Option<usize> {
        self.expected_relations
            .binary_search_by_key(&index, |expected| expected.relation_index)
            .ok()
    }
    pub(crate) fn record_edge_label_background(&mut self, index: usize, visible: bool) {
        if self.edge_label_background_events.is_none() {
            return;
        }
        let Some(slot) = self.relation_slot(index) else {
            self.edge_label_background_valid = false;
            return;
        };
        let events = self
            .edge_label_background_events
            .as_mut()
            .expect("requested background domain");
        self.edge_label_background_valid &= events[slot].is_none();
        events[slot] = Some(visible);
    }
    pub(super) fn with_edge_label_background_paint(mut self, paint: Option<ExpectedPaint>) -> Self {
        self.edge_label_background_paints = if paint.is_some() {
            vec![None; self.expected_relations.len()]
        } else {
            Vec::new()
        };
        self.expected_edge_label_background = paint;
        self
    }
    pub(crate) fn record_edge_label_background_paint(
        &mut self,
        index: usize,
        rule: Option<usize>,
        styles: &[(&str, &str)],
    ) {
        if self.expected_edge_label_background.is_none() && rule.is_none() {
            return;
        }
        let Some(slot) = self
            .relation_slot(index)
            .filter(|slot| *slot < self.edge_label_background_paints.len())
        else {
            self.edge_label_background_valid = false;
            return;
        };
        let valid = !styles.is_empty()
            && styles.iter().all(|(style, property)| {
                cluster_paint_event_matches(
                    self.expected_edge_label_background.as_ref(),
                    rule,
                    style,
                    property,
                )
            });
        self.edge_label_background_valid &= self.edge_label_background_paints[slot].is_none();
        self.edge_label_background_paints[slot] = Some(valid);
    }
    pub(super) fn proves_edge_label_background(&self, rule: usize) -> bool {
        self.expected_edge_label_background
            .as_ref()
            .is_some_and(|paint| paint.rule_index == rule)
            && self.edge_label_background_count().is_some_and(|count| {
                count > 0
                    && self
                        .edge_label_background_paints
                        .iter()
                        .filter(|paint| paint.is_some())
                        .count()
                        == count
            })
            && self
                .edge_label_background_events
                .as_ref()
                .is_some_and(|events| {
                    events.iter().enumerate().all(|(slot, visible)| {
                        *visible != Some(true)
                            || self.edge_label_background_paints[slot] == Some(true)
                    })
                })
    }
    pub(super) fn edge_label_background_count(&self) -> Option<usize> {
        let events = self.edge_label_background_events.as_ref()?;
        (self.proves_complete()
            && self.edge_label_background_valid
            && events.iter().all(Option::is_some))
        .then(|| {
            events
                .iter()
                .filter(|visible| **visible == Some(true))
                .count()
        })
    }

    #[cfg(test)]
    pub(crate) fn with_nodes(mut self, nodes: Vec<ClassNodeTerminalExpectation>) -> Self {
        let mut ids = BTreeSet::new();
        let valid = nodes.iter().all(|node| ids.insert(node.id()));
        self.bind_nodes(Cow::Owned(nodes), valid);
        self
    }
    pub(crate) fn with_borrowed_nodes(
        mut self,
        nodes: &'nodes [ClassNodeTerminalExpectation],
        valid: bool,
    ) -> Self {
        self.bind_nodes(Cow::Borrowed(nodes), valid);
        self
    }
    fn bind_nodes(&mut self, nodes: Cow<'nodes, [ClassNodeTerminalExpectation]>, valid: bool) {
        self.nodes_seen = vec![false; nodes.len()];
        self.node_domain_valid = valid;
        for expected in nodes.iter() {
            for &(target, property) in &NODE_PAINT_TARGETS {
                if let Some(rule) = expected.winner_rule(target, property) {
                    self.winner_rules.insert((rule, target, property));
                }
            }
        }
        self.expected_nodes = nodes;
    }
    pub(super) fn expected_relation_count(&self) -> usize {
        self.expected_relations.len()
    }
    pub(super) fn expected_node_count(&self) -> usize {
        self.expected_nodes.len()
    }

    #[cfg(test)]
    pub(crate) fn record_node(&mut self, event: ClassNodeTerminalEmission) {
        let Some(slot) = self
            .expected_nodes
            .iter()
            .position(|expected| expected.id() == event.id)
        else {
            self.nodes_valid = false;
            return;
        };
        let observation = self.expected_nodes[slot].observe(
            slot,
            event.fill.terminal(),
            event.stroke.terminal(),
            event.label_fill.terminal(),
        );
        self.record_node_observation(observation);
    }
    pub(crate) fn record_node_observation(&mut self, event: ClassNodeTerminalObservation) {
        let Some(expected) = self.expected_nodes.get(event.slot) else {
            self.nodes_valid = false;
            return;
        };
        self.nodes_valid &= !self.nodes_seen[event.slot] && event.values_match;
        self.nodes_seen[event.slot] = true;
        for (&(target, property), paint) in NODE_PAINT_TARGETS.iter().zip(event.paints) {
            let Some(expected_paint) = expected.expected_paint(target, property) else {
                continue;
            };
            let key = (expected_paint.rule_index, target, property);
            if paint.terminal_applicable && !paint.source_owned {
                self.effective_rules.insert(key);
            }
            if paint.terminal_applicable && !paint.source_owned && !paint.terminal_verified {
                self.typed_rejections.insert(key);
            }
        }
    }
    pub(crate) fn themes_marker(&self, name: &str) -> bool {
        self.expected_stroke.is_some()
            && self
                .expected_markers
                .iter()
                .any(|expected| expected.name == name)
    }
    pub(crate) fn record_marker(
        &mut self,
        name: &'static str,
        fill: bool,
        emitted: Option<(usize, &str)>,
        style: Option<&str>,
    ) {
        let style = style.unwrap_or_default();
        self.markers_valid &= self
            .expected_markers
            .get(self.marker_count)
            .is_some_and(|expected| expected.name == name && expected.fill_follows_stroke == fill)
            && stroke_event_matches(self.expected_stroke.as_ref(), emitted, style)
            && (!fill
                || self.expected_stroke.as_ref().is_none_or(|expected| {
                    terminal_paint(style, "fill") == Some(expected.css.as_str())
                }));
        self.marker_count = self.marker_count.saturating_add(1);
    }
    pub(crate) fn record_relation(&mut self, event: ClassRelationTerminalEvent<'_>) {
        let ClassRelationTerminalEvent {
            relation_index: index,
            start_marker: start,
            end_marker: end,
            emitted_stroke: emitted,
            terminal_style: style,
            hand_drawn_stroke: hand_drawn,
            typed_width_emitted: width,
        } = event;
        let Some(slot) = self.relation_slot(index) else {
            self.relations_valid = false;
            return;
        };
        let expected = &self.expected_relations[slot];
        self.relations_valid &= !self.relations_seen[slot]
            && start == expected.start_marker
            && end == expected.end_marker
            && stroke_event_matches(self.expected_stroke.as_ref(), emitted, style)
            && hand_drawn_stroke_matches(
                self.expected_stroke.as_ref(),
                self.hand_drawn,
                hand_drawn,
            );
        self.relations_seen[slot] = true;
        self.strokes_valid &= self
            .expected_stroke
            .as_ref()
            .is_none_or(|stroke| emitted.map(|(rule, _)| rule) == Some(stroke.rule_index));
        if width {
            self.checkpointed_typed_width_paths =
                self.checkpointed_typed_width_paths.saturating_add(1);
        }
    }
    fn proves_complete_relation_emission(&self) -> bool {
        self.relations_valid && self.relations_seen.iter().all(|seen| *seen)
    }
    fn proves_complete_note_attachment_emission(&self) -> bool {
        self.notes_valid && self.notes_seen.iter().all(|seen| *seen)
    }
    fn proves_complete_marker_emission(&self) -> bool {
        self.markers_valid && self.marker_count == self.expected_markers.len()
    }
    fn proves_complete_node_emission(&self) -> bool {
        self.node_domain_valid && self.nodes_valid && self.nodes_seen.iter().all(|seen| *seen)
    }
    pub(super) fn proves_complete(&self) -> bool {
        self.proves_complete_cluster_emission()
            && self.proves_complete_node_emission()
            && self.proves_complete_relation_emission()
            && self.proves_complete_note_attachment_emission()
            && self.proves_complete_marker_emission()
    }

    #[cfg(test)]
    pub(super) fn terminal_summary(&self) -> ClassTerminalReceiptSummary {
        self.clone().into_terminal_summary()
    }

    pub(super) fn into_terminal_summary(self) -> ClassTerminalReceiptSummary {
        let complete = self.proves_complete();
        let complete_nodes = self.proves_complete_node_emission();
        let background_count = self.edge_label_background_count();
        let background_rule = self
            .expected_edge_label_background
            .as_ref()
            .and_then(|paint| {
                self.proves_edge_label_background(paint.rule_index)
                    .then_some(paint.rule_index)
            });
        let namespace_title_rule = self.expected_namespace_title.as_ref().and_then(|paint| {
            self.proves_typed_namespace_title(paint.rule_index)
                .then_some(paint.rule_index)
        });
        let mut cluster_rules = BTreeSet::new();
        for (property, paint) in [
            (ResolvedStyleProperty::Fill, &self.expected_cluster_fill),
            (ResolvedStyleProperty::Stroke, &self.expected_cluster_stroke),
        ] {
            if let Some(paint) = paint
                && self.proves_typed_cluster_paint(paint.rule_index, property)
            {
                cluster_rules.insert((paint.rule_index, property));
            }
        }
        let typed_stroke = self.expected_stroke.as_ref().and_then(|stroke| {
            self.proves_typed_stroke(stroke.rule_index, stroke.property)
                .then_some((stroke.rule_index, stroke.property))
        });
        let typed_width = self.proves_typed_width();
        let visible_marker_occurrence_count = complete.then(|| {
            self.expected_relations
                .iter()
                .map(|expected| {
                    usize::from(expected.start_marker.is_some())
                        .saturating_add(usize::from(expected.end_marker.is_some()))
                })
                .sum()
        });
        let typed_rules = self
            .effective_rules
            .iter()
            .copied()
            .filter(|key| complete_nodes && !self.typed_rejections.contains(key))
            .collect();
        let work_units = self
            .expected_nodes
            .len()
            .saturating_add(self.nodes_seen.iter().filter(|seen| **seen).count())
            .saturating_mul(NODE_PAINT_TARGETS.len())
            .saturating_add(self.expected_clusters.len().saturating_mul(2))
            .saturating_add(
                self.cluster_events
                    .iter()
                    .filter(|event| event.is_some())
                    .count()
                    .saturating_mul(2),
            )
            .saturating_add(if self.expected_namespace_title.is_some() {
                self.expected_clusters.len().saturating_mul(2)
            } else {
                0
            })
            .saturating_add(self.expected_relations.len())
            .saturating_add(self.relations_seen.iter().filter(|seen| **seen).count())
            .saturating_add(self.expected_note_attachments.len())
            .saturating_add(self.notes_seen.iter().filter(|seen| **seen).count())
            .saturating_add(self.expected_markers.len())
            .saturating_add(self.marker_count)
            .saturating_add(
                self.edge_label_background_events
                    .as_ref()
                    .map_or(0, |events| {
                        events.iter().filter(|event| event.is_some()).count()
                    }),
            )
            .saturating_add(
                self.edge_label_background_paints
                    .iter()
                    .filter(|event| event.is_some())
                    .count(),
            )
            .max(1);
        ClassTerminalReceiptSummary {
            complete,
            complete_nodes,
            visible_marker_occurrence_count,
            node_count: self.expected_nodes.len(),
            background_count,
            background_rule,
            namespace_title_rule,
            cluster_rules,
            typed_stroke,
            typed_width,
            winner_rules: self.winner_rules,
            effective_rules: self.effective_rules,
            typed_rules,
            work_units,
        }
    }
    pub(super) fn proves_typed_width(&self) -> bool {
        self.proves_complete_relation_emission()
            && self.checkpointed_typed_width_paths == self.expected_relations.len()
    }
    pub(super) fn proves_typed_stroke(&self, rule: usize, property: ResolvedStyleProperty) -> bool {
        self.proves_complete()
            && self.strokes_valid
            && self
                .expected_stroke
                .as_ref()
                .is_some_and(|stroke| stroke.rule_index == rule && stroke.property == property)
            && (!self.expected_relations.is_empty() || !self.expected_note_attachments.is_empty())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClassTerminalReceiptSummary {
    complete: bool,
    complete_nodes: bool,
    node_count: usize,
    background_count: Option<usize>,
    background_rule: Option<usize>,
    namespace_title_rule: Option<usize>,
    cluster_rules: BTreeSet<(usize, ResolvedStyleProperty)>,
    typed_stroke: Option<(usize, ResolvedStyleProperty)>,
    typed_width: bool,
    visible_marker_occurrence_count: Option<usize>,
    winner_rules: BTreeSet<(usize, ThemeTarget, ResolvedStyleProperty)>,
    effective_rules: BTreeSet<(usize, ThemeTarget, ResolvedStyleProperty)>,
    typed_rules: BTreeSet<(usize, ThemeTarget, ResolvedStyleProperty)>,
    work_units: usize,
}

impl ClassTerminalReceiptSummary {
    pub(super) fn expected_node_count(&self) -> usize {
        self.node_count
    }
    pub(super) fn edge_label_background_count(&self) -> Option<usize> {
        self.background_count
    }
    pub(super) fn proves_edge_label_background(&self, rule: usize) -> bool {
        self.background_rule == Some(rule)
    }
    pub(super) fn proves_typed_namespace_title(&self, rule: usize) -> bool {
        self.namespace_title_rule == Some(rule)
    }
    pub(super) fn proves_typed_cluster_paint(
        &self,
        rule: usize,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.cluster_rules.contains(&(rule, property))
    }
    pub(super) fn proves_typed_stroke(&self, rule: usize, property: ResolvedStyleProperty) -> bool {
        self.typed_stroke == Some((rule, property))
    }
    pub(super) fn proves_typed_width(&self) -> bool {
        self.typed_width
    }

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

fn typed_paint(expected: &Option<ExpectedPaint>, source_owns: bool) -> Option<(usize, &str)> {
    (!source_owns)
        .then_some(expected.as_ref())
        .flatten()
        .map(|expected| (expected.rule_index, expected.css.as_str()))
}

fn node_paint_event_matches(
    expected: Option<&ExpectedPaint>,
    event: &ClassNodePaintTerminal<'_>,
) -> bool {
    if !event.terminal_applicable {
        return event.emitted.is_none();
    }
    if event.source_owned {
        return event.emitted.is_none()
            && event
                .source_value
                .is_none_or(|source| event.terminal_value == source);
    }
    match (expected, event.emitted) {
        (None, None) => true,
        (Some(expected), Some((rule_index, css))) => {
            expected.rule_index == rule_index
                && expected.css == css
                && event.terminal_value == expected.css
        }
        (Some(_), None) | (None, Some(_)) => false,
    }
}

fn node_label_paint_event_matches(
    expected: Option<&ExpectedPaint>,
    event: &ClassNodePaintTerminal<'_>,
) -> bool {
    if !event.terminal_applicable {
        return event.emitted.is_none();
    }
    if event.source_owned {
        return event.emitted.is_none()
            && event.source_value.is_none_or(|source| {
                terminal_paint(event.terminal_value, "color") == Some(source)
            });
    }
    match (expected, event.emitted) {
        (None, None) => true,
        (Some(expected), Some((rule_index, css))) => {
            expected.rule_index == rule_index
                && expected.css == css
                && terminal_paint(event.terminal_value, "color") == Some(expected.css.as_str())
                && terminal_paint(event.terminal_value, "fill") == Some(expected.css.as_str())
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

    #[test]
    fn completion_only_requires_exact_canonical_domains_with_balanced_bad_counts() {
        let emission = |id| {
            ClassNodeTerminalEmission::new(
                id,
                ClassNodePaintTerminalEmission::new(false, None, "#fff"),
                ClassNodePaintTerminalEmission::new(false, None, "#000"),
                ClassNodePaintTerminalEmission::new(false, None, ""),
            )
        };
        for observed in [["B", "B"], ["A", "foreign"]] {
            let mut receipt = ClassRelationThemeReceipt::new(Vec::new(), Vec::new(), None, false)
                .with_nodes(vec![
                    ClassNodeTerminalExpectation::new("A"),
                    ClassNodeTerminalExpectation::new("B"),
                ]);
            for id in observed {
                receipt.record_node(emission(id));
            }
            assert!(!receipt.proves_complete(), "{observed:?}");
        }
        let mut duplicate_expected =
            ClassRelationThemeReceipt::new(Vec::new(), Vec::new(), None, false).with_nodes(vec![
                ClassNodeTerminalExpectation::new("A"),
                ClassNodeTerminalExpectation::new("A"),
            ]);
        duplicate_expected.record_node(emission("A"));
        assert!(!duplicate_expected.proves_complete());
        let relations = || {
            vec![
                ClassRelationTerminalExpectation::new(2, None, Some("extensionEnd")),
                ClassRelationTerminalExpectation::new(7, Some("aggregationStart"), None),
            ]
        };
        let mut reordered = ClassRelationThemeReceipt::new(relations(), Vec::new(), None, false);
        reordered.record_relation(ClassRelationTerminalEvent {
            relation_index: 7,
            start_marker: Some("aggregationStart"),
            end_marker: None,
            emitted_stroke: None,
            terminal_style: "",
            hand_drawn_stroke: None,
            typed_width_emitted: false,
        });
        reordered.record_relation(ClassRelationTerminalEvent {
            relation_index: 2,
            start_marker: None,
            end_marker: Some("extensionEnd"),
            emitted_stroke: None,
            terminal_style: "",
            hand_drawn_stroke: None,
            typed_width_emitted: false,
        });
        assert!(reordered.proves_complete());
        let mut foreign = ClassRelationThemeReceipt::new(relations(), Vec::new(), None, false);
        foreign.record_relation(ClassRelationTerminalEvent {
            relation_index: 7,
            start_marker: Some("aggregationStart"),
            end_marker: None,
            emitted_stroke: None,
            terminal_style: "",
            hand_drawn_stroke: None,
            typed_width_emitted: false,
        });
        foreign.record_relation(ClassRelationTerminalEvent {
            relation_index: 99,
            start_marker: None,
            end_marker: Some("extensionEnd"),
            emitted_stroke: None,
            terminal_style: "",
            hand_drawn_stroke: None,
            typed_width_emitted: false,
        });
        assert!(!foreign.proves_complete());
        let mut duplicated = ClassRelationThemeReceipt::new(relations(), Vec::new(), None, false);
        duplicated.record_relation(ClassRelationTerminalEvent {
            relation_index: 7,
            start_marker: Some("aggregationStart"),
            end_marker: None,
            emitted_stroke: None,
            terminal_style: "",
            hand_drawn_stroke: None,
            typed_width_emitted: false,
        });
        duplicated.record_relation(ClassRelationTerminalEvent {
            relation_index: 7,
            start_marker: Some("aggregationStart"),
            end_marker: None,
            emitted_stroke: None,
            terminal_style: "",
            hand_drawn_stroke: None,
            typed_width_emitted: false,
        });
        assert!(!duplicated.proves_complete());
    }

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
    ) -> ClassRelationThemeReceipt<'static> {
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
                relation.record_relation(ClassRelationTerminalEvent {
                    relation_index: 0,
                    start_marker: None,
                    end_marker: None,
                    emitted_stroke: None,
                    terminal_style: "",
                    hand_drawn_stroke: emitted,
                    typed_width_emitted: false,
                });
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
            receipt.record_relation(ClassRelationTerminalEvent {
                relation_index: 0,
                start_marker: None,
                end_marker: None,
                emitted_stroke: Some((3, "#123456")),
                terminal_style: "stroke:#123456 !important",
                hand_drawn_stroke: None,
                typed_width_emitted: false,
            });
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
        receipt.record_relation(ClassRelationTerminalEvent {
            relation_index: 0,
            start_marker: None,
            end_marker: None,
            emitted_stroke: Some((3, "#123456")),
            terminal_style: "stroke:#123456 !important;stroke-width:6px !important",
            hand_drawn_stroke: None,
            typed_width_emitted: true,
        });
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
                receipt.record_relation(ClassRelationTerminalEvent {
                    relation_index: index,
                    start_marker: Some(if case == "marker" {
                        "extensionEnd"
                    } else {
                        "compositionStart"
                    }),
                    end_marker: None,
                    emitted_stroke: Some((if case == "rule" { 4 } else { 3 }, "#123456")),
                    terminal_style: if case == "paint" {
                        "stroke:#abcdef !important;"
                    } else {
                        "stroke:#123456 !important;"
                    },
                    hand_drawn_stroke: None,
                    typed_width_emitted: false,
                });
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
        missing_hand_drawn_stroke.record_relation(ClassRelationTerminalEvent {
            relation_index: 0,
            start_marker: Some("compositionStart"),
            end_marker: None,
            emitted_stroke: Some((3, "#123456")),
            terminal_style: ";;;stroke:#123456 !important;",
            hand_drawn_stroke: None,
            typed_width_emitted: false,
        });
        assert!(!missing_hand_drawn_stroke.proves_complete());

        let mut wrong_marker_fill = single_hand_drawn_receipt();
        wrong_marker_fill.record_marker(
            "compositionStart",
            true,
            Some((3, "#123456")),
            Some("stroke:#123456 !important;fill:#abcdef !important"),
        );
        wrong_marker_fill.record_relation(ClassRelationTerminalEvent {
            relation_index: 0,
            start_marker: Some("compositionStart"),
            end_marker: None,
            emitted_stroke: Some((3, "#123456")),
            terminal_style: ";;;stroke:#123456 !important;",
            hand_drawn_stroke: Some("#123456"),
            typed_width_emitted: false,
        });
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
            receipt.record_relation(ClassRelationTerminalEvent {
                relation_index: index,
                start_marker: None,
                end_marker: None,
                emitted_stroke: None,
                terminal_style: "",
                hand_drawn_stroke: None,
                typed_width_emitted: false,
            });
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
        baseline.record_relation(ClassRelationTerminalEvent {
            relation_index: 0,
            start_marker: None,
            end_marker: None,
            emitted_stroke: None,
            terminal_style: "",
            hand_drawn_stroke: None,
            typed_width_emitted: false,
        });
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

    fn single_hand_drawn_receipt() -> ClassRelationThemeReceipt<'static> {
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
        receipt.record_relation(ClassRelationTerminalEvent {
            relation_index,
            start_marker,
            end_marker,
            emitted_stroke: Some((3, "#123456")),
            terminal_style: ";;;stroke:#123456 !important;",
            hand_drawn_stroke: None,
            typed_width_emitted: false,
        });
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
