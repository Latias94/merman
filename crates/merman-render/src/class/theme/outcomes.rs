use std::collections::BTreeSet;

use crate::diagram_theme::{ResolvedStyleProperty, ThemeTarget};

type PaintKey = (usize, ThemeTarget, ResolvedStyleProperty);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ClassSelectedPaint {
    pub(super) target: ThemeTarget,
    pub(super) rule_index: usize,
    pub(super) css: String,
}

/// Rule origins retained beside the selected paint used by a concrete writer.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClassNodePaintOutcomes {
    winners: [Option<usize>; 3],
    applied: [bool; 3],
    label_verified: bool,
}

impl ClassNodePaintOutcomes {
    #[allow(
        clippy::too_many_arguments,
        reason = "Each actual node paint retains its selected origin and whether the writer consumed it."
    )]
    pub(crate) fn new(
        fill_winner: Option<usize>,
        stroke_winner: Option<usize>,
        label_winner: Option<usize>,
        fill_applied: bool,
        stroke_applied: bool,
        label_applied: bool,
        label_verified: bool,
    ) -> Self {
        Self {
            winners: [fill_winner, stroke_winner, label_winner],
            applied: [fill_applied, stroke_applied, label_applied],
            label_verified,
        }
    }
}

/// Marker names are drawing inputs: only referenced definitions receive the edge paint.
#[derive(Debug, Clone)]
pub(crate) struct ClassMarkerPaintBinding {
    name: &'static str,
}

impl ClassMarkerPaintBinding {
    pub(crate) const fn new(name: &'static str) -> Self {
        Self { name }
    }
}

/// Actual facts from a successfully emitted relation path.
pub(crate) struct ClassRelationPaintOutcome<'a> {
    pub(crate) start_marker: Option<&'static str>,
    pub(crate) end_marker: Option<&'static str>,
    pub(crate) emitted_stroke: Option<(usize, &'a str)>,
    pub(crate) typed_width_emitted: bool,
}

/// Aggregate only requested theme outcomes, without pre-enumerating semantic completion slots.
#[derive(Debug)]
pub(crate) struct ClassThemeOutcomes {
    enabled: bool,
    themed_markers: Vec<ClassMarkerPaintBinding>,
    stroke_property: Option<ResolvedStyleProperty>,
    summary: ClassThemeOutcomeSummary,
}

impl ClassThemeOutcomes {
    pub(super) fn new(
        enabled: bool,
        themed_markers: Vec<ClassMarkerPaintBinding>,
        stroke_property: Option<ResolvedStyleProperty>,
        track_backgrounds: bool,
    ) -> Self {
        Self {
            enabled,
            themed_markers,
            stroke_property,
            summary: ClassThemeOutcomeSummary {
                background_count: track_backgrounds.then_some(0),
                ..ClassThemeOutcomeSummary::default()
            },
        }
    }

    pub(crate) fn record_cluster(&mut self, fill_rule: Option<usize>, stroke_rule: Option<usize>) {
        if !self.enabled {
            return;
        }
        self.summary
            .cluster_rules
            .extend(fill_rule.map(|rule| (rule, ResolvedStyleProperty::Fill)));
        self.summary
            .cluster_rules
            .extend(stroke_rule.map(|rule| (rule, ResolvedStyleProperty::Stroke)));
    }

    pub(crate) fn record_namespace_title(
        &mut self,
        rule: usize,
        visible: bool,
        inherits_paint: bool,
    ) {
        if !self.enabled || !visible {
            return;
        }
        self.summary.namespace_title_rules.insert(rule);
        if !inherits_paint {
            self.summary.unverified_namespace_titles.insert(rule);
        }
    }

    pub(crate) fn record_node(&mut self, event: ClassNodePaintOutcomes) {
        if !self.enabled {
            return;
        }
        self.summary.node_count += 1;
        for (index, rule) in event.winners.into_iter().enumerate() {
            let Some(rule) = rule else {
                continue;
            };
            let (target, property) = match index {
                0 => (ThemeTarget::Node, ResolvedStyleProperty::Fill),
                1 => (ThemeTarget::Node, ResolvedStyleProperty::Stroke),
                _ => (ThemeTarget::NodeLabel, ResolvedStyleProperty::Fill),
            };
            let key = (rule, target, property);
            self.summary.selected_node_rules.insert(key);
            if !event.applied[index] {
                continue;
            }
            self.summary.node_rules.insert(key);
            if index == 2 && !event.label_verified {
                self.summary.unverified_node_rules.insert(key);
            }
        }
    }

    pub(crate) fn themes_marker(&self, name: &str) -> bool {
        self.themed_markers.iter().any(|marker| marker.name == name)
    }

    pub(crate) fn record_relation(&mut self, event: ClassRelationPaintOutcome<'_>) {
        if !self.enabled {
            return;
        }
        self.summary.marker_count +=
            usize::from(event.start_marker.is_some()) + usize::from(event.end_marker.is_some());
        self.record_stroke(event.emitted_stroke);
        self.summary.typed_width |= event.typed_width_emitted;
        self.summary.unverified_width |= !event.typed_width_emitted;
    }

    pub(crate) fn record_stroke(&mut self, emitted: Option<(usize, &str)>) {
        if !self.enabled {
            return;
        }
        if let Some((rule, _)) = emitted
            && let Some(property) = self.stroke_property
        {
            self.summary.typed_stroke = Some((rule, property));
        }
    }

    pub(crate) fn record_unavailable_path(
        &mut self,
        stroke_rule: Option<usize>,
        typed_width: bool,
    ) {
        if !self.enabled {
            return;
        }
        self.summary.unverified_strokes.extend(stroke_rule);
        self.summary.unverified_width |= typed_width;
    }

    pub(crate) fn record_edge_label_background(&mut self, visible: bool) {
        if self.enabled
            && visible
            && let Some(count) = &mut self.summary.background_count
        {
            *count += 1;
        }
    }

    pub(crate) fn record_edge_label_background_paint(&mut self, rule: Option<usize>) {
        if self.enabled {
            self.summary.background_rules.extend(rule);
        }
    }

    pub(super) fn into_summary(self) -> ClassThemeOutcomeSummary {
        self.summary
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ClassThemeOutcomeSummary {
    node_count: usize,
    background_count: Option<usize>,
    background_rules: BTreeSet<usize>,
    namespace_title_rules: BTreeSet<usize>,
    unverified_namespace_titles: BTreeSet<usize>,
    cluster_rules: BTreeSet<(usize, ResolvedStyleProperty)>,
    typed_stroke: Option<(usize, ResolvedStyleProperty)>,
    typed_width: bool,
    unverified_strokes: BTreeSet<usize>,
    unverified_width: bool,
    marker_count: usize,
    selected_node_rules: BTreeSet<PaintKey>,
    node_rules: BTreeSet<PaintKey>,
    unverified_node_rules: BTreeSet<PaintKey>,
}

impl ClassThemeOutcomeSummary {
    pub(super) fn node_count(&self) -> usize {
        self.node_count
    }
    pub(super) fn edge_label_background_count(&self) -> Option<usize> {
        self.background_count
    }
    pub(super) fn applied_edge_label_background(&self, rule: usize) -> bool {
        self.background_rules.contains(&rule)
    }
    pub(super) fn applied_namespace_title(&self, rule: usize) -> bool {
        self.namespace_title_rules.contains(&rule)
            && !self.unverified_namespace_titles.contains(&rule)
    }
    pub(super) fn applied_cluster_paint(
        &self,
        rule: usize,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.cluster_rules.contains(&(rule, property))
    }
    pub(super) fn applied_stroke(&self, rule: usize, property: ResolvedStyleProperty) -> bool {
        self.typed_stroke == Some((rule, property)) && !self.unverified_strokes.contains(&rule)
    }
    pub(super) fn applied_width(&self) -> bool {
        self.typed_width && !self.unverified_width
    }
    pub(super) fn visible_marker_occurrence_count(&self) -> usize {
        self.marker_count
    }
    pub(super) fn selected_node_paint(
        &self,
        rule: usize,
        target: ThemeTarget,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.selected_node_rules.contains(&(rule, target, property))
    }
    pub(super) fn has_node_paint(
        &self,
        rule: usize,
        target: ThemeTarget,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.node_rules.contains(&(rule, target, property))
    }
    pub(super) fn applied_node_paint(
        &self,
        rule: usize,
        target: ThemeTarget,
        property: ResolvedStyleProperty,
    ) -> bool {
        self.has_node_paint(rule, target, property)
            && !self
                .unverified_node_rules
                .contains(&(rule, target, property))
    }
}
