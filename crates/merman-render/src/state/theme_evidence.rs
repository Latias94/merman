use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::OnceLock;

#[cfg(test)]
use crate::diagram_theme::ThemeTypographyProperty;
use crate::diagram_theme::{FamilyThemeMechanismKey, ThemeCapability};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::text::parse_css_font_stack;

/// Stable identity for one State terminal surface written to the SVG artifact.
///
/// The identity is intentionally semantic rather than DOM-position based. State can render nested
/// roots in a different traversal order without weakening duplicate or missing-occurrence checks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StateThemeTerminalOccurrence {
    Title,
    NodeShape(String),
    NodeLabel(String),
    CompositeHeader(String),
    CompositeHeaderLabel(String),
    SpecialStateInner(String),
    EdgePath(String),
    EdgeMarker(String),
    EdgeLabel(String),
    EdgeLabelBackground(String),
}

/// Exact planned terminal value for one State occurrence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StateThemeTerminalExpectation {
    occurrence: StateThemeTerminalOccurrence,
    expected_value: String,
}

impl StateThemeTerminalExpectation {
    pub(crate) fn new(
        occurrence: StateThemeTerminalOccurrence,
        expected_value: impl Into<String>,
    ) -> Self {
        Self {
            occurrence,
            expected_value: expected_value.into(),
        }
    }
}

/// Writer-owned receipt for the exact State occurrences emitted to terminal SVG.
///
/// Missing, duplicate, and wrong-valued events invalidate the whole receipt. This is deliberately
/// fail-closed: a partial receipt must never promote a planned rule to `Applied`.
#[derive(Debug, Clone)]
pub(crate) struct StateThemeTerminalReceipt {
    expected: BTreeMap<StateThemeTerminalOccurrence, String>,
    emissions: BTreeMap<StateThemeTerminalOccurrence, Range<usize>>,
    actual: BTreeMap<StateThemeTerminalOccurrence, String>,
    invalid: bool,
    nodes_filtered: bool,
    edges_filtered: bool,
}

impl StateThemeTerminalReceipt {
    fn new(
        expectations: &BTreeMap<StateThemeTerminalOccurrence, String>,
        nodes_filtered: bool,
        edges_filtered: bool,
    ) -> Self {
        Self {
            expected: expectations.clone(),
            emissions: BTreeMap::new(),
            actual: BTreeMap::new(),
            invalid: false,
            nodes_filtered,
            edges_filtered,
        }
    }

    #[cfg(test)]
    fn record(
        &mut self,
        occurrence: StateThemeTerminalOccurrence,
        actual_value: impl Into<String>,
    ) {
        let actual_value = actual_value.into();
        if !self.expected.contains_key(&occurrence)
            || self.actual.insert(occurrence, actual_value).is_some()
        {
            self.invalid = true;
        }
    }

    /// Record the exact byte range serialized by a State writer branch.
    pub(crate) fn record_emission_if_expected(
        &mut self,
        occurrence: StateThemeTerminalOccurrence,
        emitted_range: Range<usize>,
    ) {
        if !self.expected.contains_key(&occurrence) {
            return;
        }
        if emitted_range.is_empty() || self.emissions.insert(occurrence, emitted_range).is_some() {
            self.invalid = true;
        }
    }

    /// Adjust writer ranges after the deferred SVG root rewrites an earlier prefix.
    pub(crate) fn shift_emissions_after_prefix_rewrite(
        &mut self,
        previous_document_len: usize,
        current_document_len: usize,
    ) {
        if self.invalid || previous_document_len == current_document_len {
            return;
        }
        let grow_by = current_document_len.checked_sub(previous_document_len);
        let shrink_by = previous_document_len.checked_sub(current_document_len);
        for range in self.emissions.values_mut() {
            let shifted = if let Some(grow_by) = grow_by {
                range
                    .start
                    .checked_add(grow_by)
                    .zip(range.end.checked_add(grow_by))
            } else if let Some(shrink_by) = shrink_by {
                range
                    .start
                    .checked_sub(shrink_by)
                    .zip(range.end.checked_sub(shrink_by))
            } else {
                Some((range.start, range.end))
            };
            let Some((start, end)) = shifted else {
                self.invalid = true;
                return;
            };
            *range = start..end;
        }
    }

    /// Reconstruct compact actual signatures from the completed writer artifact.
    ///
    /// Parsing once at finalization avoids retaining or reparsing a copy of every node fragment.
    /// Byte ranges also make a missing writer branch distinguishable from a plan replay.
    pub(crate) fn observe_svg(
        &mut self,
        svg: &str,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        if self.invalid || self.expected.is_empty() || self.emissions.is_empty() {
            return Ok(());
        }
        let Ok(document) = roxmltree::Document::parse(svg) else {
            self.invalid = true;
            return Ok(());
        };
        let mut range_groups = BTreeMap::<(usize, usize), Vec<StateThemeTerminalOccurrence>>::new();
        for (occurrence, range) in &self.emissions {
            if range.end > svg.len()
                || !svg.is_char_boundary(range.start)
                || !svg.is_char_boundary(range.end)
            {
                self.invalid = true;
                return Ok(());
            }
            range_groups
                .entry((range.start, range.end))
                .or_default()
                .push(occurrence.clone());
        }

        let groups = range_groups
            .into_iter()
            .map(|((start, end), occurrences)| StateTerminalRangeGroup {
                range: start..end,
                occurrences,
            })
            .collect::<Vec<_>>();
        if groups
            .windows(2)
            .any(|pair| pair[0].range.end > pair[1].range.start)
        {
            self.invalid = true;
            return Ok(());
        }

        let mut observations = self
            .emissions
            .keys()
            .cloned()
            .map(|occurrence| (occurrence, StateTerminalSurfaceObservation::default()))
            .collect::<BTreeMap<_, _>>();
        let mut group_index = 0usize;
        let mut ancestors = Vec::<StateTerminalAncestor<'_>>::new();
        for node in document.descendants().filter(|node| node.is_element()) {
            work_meter.charge(1)?;
            let node_range = node.range();
            while ancestors
                .last()
                .is_some_and(|ancestor| node_range.start >= ancestor.end)
            {
                ancestors.pop();
            }
            let context = StateTerminalNodeContext::for_node(node, ancestors.last());
            while groups
                .get(group_index)
                .is_some_and(|group| node_range.start >= group.range.end)
            {
                group_index += 1;
            }
            if let Some(group) = groups.get(group_index)
                && node_range.start >= group.range.start
                && node_range.end <= group.range.end
            {
                for occurrence in &group.occurrences {
                    let Some(observation) = observations.get_mut(occurrence) else {
                        self.invalid = true;
                        return Ok(());
                    };
                    observation.observe_node(occurrence, node, &context);
                }
            }
            ancestors.push(context.into_ancestor(node_range.end));
        }

        let mut observed = Vec::with_capacity(self.emissions.len());
        for (occurrence, observation) in observations {
            let Some(expected) = self.expected.get(&occurrence) else {
                self.invalid = true;
                return Ok(());
            };
            observed.push((
                occurrence.clone(),
                observation.actual_signature(&occurrence, expected),
            ));
        }
        for (occurrence, actual) in observed {
            if self.actual.insert(occurrence, actual).is_some() {
                self.invalid = true;
            }
        }
        Ok(())
    }

    fn proves_occurrences(&self, occurrences: &BTreeSet<StateThemeTerminalOccurrence>) -> bool {
        !self.invalid
            && occurrences.iter().all(|occurrence| {
                self.expected
                    .get(occurrence)
                    .zip(self.actual.get(occurrence))
                    .is_some_and(|(expected, actual)| expected == actual)
            })
    }

    fn occurrences_were_filtered(
        &self,
        occurrences: &BTreeSet<StateThemeTerminalOccurrence>,
    ) -> bool {
        let mut missing = occurrences
            .iter()
            .filter(|occurrence| !self.actual.contains_key(*occurrence))
            .peekable();
        missing.peek().is_some()
            && missing.all(|occurrence| match occurrence {
                StateThemeTerminalOccurrence::NodeShape(_)
                | StateThemeTerminalOccurrence::NodeLabel(_)
                | StateThemeTerminalOccurrence::CompositeHeader(_)
                | StateThemeTerminalOccurrence::CompositeHeaderLabel(_)
                | StateThemeTerminalOccurrence::SpecialStateInner(_) => self.nodes_filtered,
                StateThemeTerminalOccurrence::EdgePath(_)
                | StateThemeTerminalOccurrence::EdgeMarker(_)
                | StateThemeTerminalOccurrence::EdgeLabel(_)
                | StateThemeTerminalOccurrence::EdgeLabelBackground(_) => self.edges_filtered,
                StateThemeTerminalOccurrence::Title => false,
            })
    }

    fn can_seal(&self) -> bool {
        !self.invalid
            && self.expected.iter().all(|(occurrence, expected)| {
                self.actual
                    .get(occurrence)
                    .is_some_and(|actual| actual == expected)
                    || (!self.actual.contains_key(occurrence)
                        && occurrence_was_filtered(
                            occurrence,
                            self.nodes_filtered,
                            self.edges_filtered,
                        ))
            })
    }

    #[cfg(test)]
    fn record_expected_for_test(&mut self) {
        for (occurrence, expected_value) in self.expected.clone() {
            self.record(occurrence, expected_value);
        }
    }
}

/// Pending State theme mechanisms and the sole terminal receipt allowed to promote them.
#[derive(Debug, Clone)]
pub(crate) struct StateThemeTerminalPlan {
    expectations: BTreeMap<StateThemeTerminalOccurrence, String>,
    pending_mechanisms: BTreeMap<FamilyThemeMechanismKey, StatePendingTerminalMechanism>,
    valid: bool,
    terminal_receipt: OnceLock<StateThemeTerminalReceipt>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StatePendingTerminalMechanism {
    pub(crate) capabilities: BTreeSet<ThemeCapability>,
    pub(crate) occurrences: BTreeSet<StateThemeTerminalOccurrence>,
}

impl StateThemeTerminalPlan {
    pub(crate) fn new(
        expectations: impl IntoIterator<Item = StateThemeTerminalExpectation>,
        pending_mechanisms: BTreeMap<FamilyThemeMechanismKey, StatePendingTerminalMechanism>,
    ) -> Self {
        let mut expectation_map = BTreeMap::new();
        let mut valid = true;
        for expectation in expectations {
            let expected_value = normalize_terminal_signature(&expectation.expected_value);
            if expectation_map
                .insert(expectation.occurrence, expected_value)
                .is_some()
            {
                // Duplicate planned identities can never produce a unique terminal receipt.
                valid = false;
                break;
            }
        }
        let pending_occurrences = pending_mechanisms
            .values()
            .flat_map(|mechanism| mechanism.occurrences.iter().cloned())
            .collect::<BTreeSet<_>>();
        expectation_map.retain(|occurrence, _| pending_occurrences.contains(occurrence));
        valid &= pending_mechanisms
            .values()
            .all(|mechanism| !mechanism.occurrences.is_empty());
        valid &= pending_occurrences.iter().all(|occurrence| {
            expectation_map
                .get(occurrence)
                .is_some_and(|value| !value.is_empty())
        });
        Self {
            expectations: expectation_map,
            pending_mechanisms,
            valid,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn begin_receipt(
        &self,
        include_nodes: bool,
        include_edges: bool,
    ) -> StateThemeTerminalReceipt {
        StateThemeTerminalReceipt::new(&self.expectations, !include_nodes, !include_edges)
    }

    pub(crate) fn record_terminal(&self, receipt: StateThemeTerminalReceipt) -> bool {
        if self.pending_mechanisms.is_empty() {
            return true;
        }
        self.valid && receipt.can_seal() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self, planned: &FamilyThemeEvidence) -> FamilyThemeEvidence {
        let mut evidence = planned.clone();
        if self.pending_mechanisms.is_empty() || self.terminal_receipt.get().is_none() {
            return evidence;
        }
        let receipt = self
            .terminal_receipt
            .get()
            .expect("checked terminal receipt presence");
        for (key, mechanism) in &self.pending_mechanisms {
            if receipt.proves_occurrences(&mechanism.occurrences) {
                evidence.mark_applied_with_capabilities(
                    key.clone(),
                    mechanism.capabilities.iter().copied(),
                );
            } else if receipt.occurrences_were_filtered(&mechanism.occurrences) {
                evidence.mark_residual(
                    key.clone(),
                    FamilyThemeResidualReason::OutputVisibilityFiltered,
                );
            }
        }
        evidence
    }

    pub(crate) fn has_pending_mechanisms(&self) -> bool {
        !self.pending_mechanisms.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn record_complete_for_plan_test(&self) {
        if self.pending_mechanisms.is_empty() {
            return;
        }
        let mut receipt = self.begin_receipt(true, true);
        receipt.record_expected_for_test();
        assert!(self.record_terminal(receipt));
    }
}

fn occurrence_was_filtered(
    occurrence: &StateThemeTerminalOccurrence,
    nodes_filtered: bool,
    edges_filtered: bool,
) -> bool {
    match occurrence {
        StateThemeTerminalOccurrence::NodeShape(_)
        | StateThemeTerminalOccurrence::NodeLabel(_)
        | StateThemeTerminalOccurrence::CompositeHeader(_)
        | StateThemeTerminalOccurrence::CompositeHeaderLabel(_)
        | StateThemeTerminalOccurrence::SpecialStateInner(_) => nodes_filtered,
        StateThemeTerminalOccurrence::EdgePath(_)
        | StateThemeTerminalOccurrence::EdgeMarker(_)
        | StateThemeTerminalOccurrence::EdgeLabel(_)
        | StateThemeTerminalOccurrence::EdgeLabelBackground(_) => edges_filtered,
        StateThemeTerminalOccurrence::Title => false,
    }
}

fn normalize_terminal_signature(signature: &str) -> String {
    signature
        .split('\u{1f}')
        .map(str::trim)
        .filter(|component| !component.is_empty())
        .map(|component| {
            let declarations = css_declarations(component);
            if declarations.is_empty() {
                component.to_string()
            } else {
                canonical_css_declarations(&declarations)
            }
        })
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

struct StateTerminalRangeGroup {
    range: Range<usize>,
    occurrences: Vec<StateThemeTerminalOccurrence>,
}

#[derive(Clone, Copy, Default)]
struct StateTerminalAncestor<'a> {
    end: usize,
    in_label: bool,
    in_cluster_label: bool,
    in_inner: bool,
    in_outer: bool,
    in_special_inner: bool,
    in_foreign_object: bool,
    in_prepared_foreign_object: bool,
    in_html_label_content: bool,
    self_outer_path: bool,
    self_marker: bool,
    self_foreign_object: bool,
    self_html_label_carrier: bool,
    self_html_label_root: bool,
    label_data_id: Option<&'a str>,
}

#[derive(Clone, Copy, Default)]
struct StateTerminalNodeContext<'a> {
    in_label: bool,
    in_cluster_label: bool,
    in_inner: bool,
    in_outer: bool,
    in_special_inner: bool,
    in_foreign_object: bool,
    in_prepared_foreign_object: bool,
    in_html_label_content: bool,
    self_label: bool,
    self_cluster_label: bool,
    self_outer_path: bool,
    self_marker: bool,
    self_foreign_object: bool,
    self_html_label_carrier: bool,
    self_html_label_root: bool,
    parent_is_marker: bool,
    parent_is_foreign_object: bool,
    label_data_id: Option<&'a str>,
}

impl<'document> StateTerminalNodeContext<'document> {
    fn for_node<'input>(
        node: roxmltree::Node<'document, 'input>,
        parent: Option<&StateTerminalAncestor<'document>>,
    ) -> Self {
        let self_label = node_has_class(node, "label");
        let self_cluster_label = node_has_class(node, "cluster-label");
        let self_inner = node_has_class(node, "inner");
        let self_outer = node_has_class(node, "outer");
        let self_outer_path = node_has_class(node, "outer-path");
        let self_marker = node.tag_name().name() == "marker";
        let self_foreign_object = node.tag_name().name() == "foreignObject";
        let self_prepared_foreign_object = self_foreign_object
            && node
                .attribute(crate::svg::PREPARED_TEXT_LABEL_DATA_ATTR)
                .is_some();
        let self_html_label_carrier = node.tag_name().name() == "div"
            && parent.is_some_and(|parent| parent.self_foreign_object);
        let self_html_label_root = parent.is_some_and(|parent| parent.self_html_label_carrier)
            && matches!(node.tag_name().name(), "span" | "p");
        let starts_special_inner = node.tag_name().name() == "g"
            && node.attribute("class").is_none()
            && parent.is_some_and(|parent| parent.self_outer_path);
        let label_data_id = if self_label {
            node.attribute("data-id")
                .or_else(|| parent.and_then(|parent| parent.label_data_id))
        } else {
            parent.and_then(|parent| parent.label_data_id)
        };
        Self {
            in_label: self_label || parent.is_some_and(|parent| parent.in_label),
            in_cluster_label: self_cluster_label
                || parent.is_some_and(|parent| parent.in_cluster_label),
            in_inner: self_inner || parent.is_some_and(|parent| parent.in_inner),
            in_outer: self_outer || parent.is_some_and(|parent| parent.in_outer),
            in_special_inner: starts_special_inner
                || parent.is_some_and(|parent| parent.in_special_inner),
            in_foreign_object: self_foreign_object
                || parent.is_some_and(|parent| parent.in_foreign_object),
            in_prepared_foreign_object: self_prepared_foreign_object
                || parent.is_some_and(|parent| parent.in_prepared_foreign_object),
            in_html_label_content: parent
                .is_some_and(|parent| parent.in_html_label_content || parent.self_html_label_root),
            self_label,
            self_cluster_label,
            self_outer_path,
            self_marker,
            self_foreign_object,
            self_html_label_carrier,
            self_html_label_root,
            parent_is_marker: parent.is_some_and(|parent| parent.self_marker),
            parent_is_foreign_object: parent.is_some_and(|parent| parent.self_foreign_object),
            label_data_id,
        }
    }

    fn into_ancestor(self, end: usize) -> StateTerminalAncestor<'document> {
        StateTerminalAncestor {
            end,
            in_label: self.in_label,
            in_cluster_label: self.in_cluster_label,
            in_inner: self.in_inner,
            in_outer: self.in_outer,
            in_special_inner: self.in_special_inner,
            in_foreign_object: self.in_foreign_object,
            in_prepared_foreign_object: self.in_prepared_foreign_object,
            in_html_label_content: self.in_html_label_content,
            self_outer_path: self.self_outer_path,
            self_marker: self.self_marker,
            self_foreign_object: self.self_foreign_object,
            self_html_label_carrier: self.self_html_label_carrier,
            self_html_label_root: self.self_html_label_root,
            label_data_id: self.label_data_id,
        }
    }
}

#[derive(Default)]
struct StateTerminalSurfaceObservation {
    direct_styles: Vec<BTreeMap<String, String>>,
    shape_styles: Vec<BTreeMap<String, String>>,
    inner_shape_styles: Vec<BTreeMap<String, String>>,
    outer_shape_styles: Vec<BTreeMap<String, String>>,
    special_inner_styles: Vec<BTreeMap<String, String>>,
    label_container_styles: Vec<BTreeMap<String, String>>,
    html_label_styles: Vec<BTreeMap<String, String>>,
    native_label_styles: Vec<BTreeMap<String, String>>,
    html_descendant_styles: Vec<BTreeMap<String, String>>,
    html_descendant_selector_override: bool,
    background_styles: Vec<BTreeMap<String, String>>,
    fallback_fill_values: Vec<String>,
}

impl StateTerminalSurfaceObservation {
    fn observe_node(
        &mut self,
        occurrence: &StateThemeTerminalOccurrence,
        node: roxmltree::Node<'_, '_>,
        context: &StateTerminalNodeContext<'_>,
    ) {
        let tag = node.tag_name().name();
        match occurrence {
            StateThemeTerminalOccurrence::Title => {
                if tag == "text" && node_has_class(node, "statediagramTitleText") {
                    record_style(node, &mut self.direct_styles);
                }
            }
            StateThemeTerminalOccurrence::NodeShape(_) => {
                if is_shape_terminal_tag(tag)
                    && !context.in_label
                    && !context.in_cluster_label
                    && !context.in_special_inner
                {
                    record_style(node, &mut self.shape_styles);
                    if context.in_inner {
                        record_style(node, &mut self.inner_shape_styles);
                    }
                }
            }
            StateThemeTerminalOccurrence::NodeLabel(_) => {
                self.observe_html_descendant(node, context);
                if context.self_label && !context.in_cluster_label {
                    record_style(node, &mut self.label_container_styles);
                } else if context.in_label
                    && !context.in_cluster_label
                    && tag == "div"
                    && context.parent_is_foreign_object
                {
                    record_style(node, &mut self.html_label_styles);
                } else if context.in_label
                    && !context.in_cluster_label
                    && !context.in_foreign_object
                    && tag == "text"
                {
                    record_style(node, &mut self.native_label_styles);
                }
            }
            StateThemeTerminalOccurrence::CompositeHeader(_) => {
                if is_shape_terminal_tag(tag) && context.in_outer {
                    record_style(node, &mut self.outer_shape_styles);
                }
            }
            StateThemeTerminalOccurrence::CompositeHeaderLabel(_) => {
                self.observe_html_descendant(node, context);
                if context.self_cluster_label {
                    record_style(node, &mut self.label_container_styles);
                } else if context.in_cluster_label
                    && tag == "div"
                    && context.parent_is_foreign_object
                {
                    record_style(node, &mut self.html_label_styles);
                } else if context.in_cluster_label && !context.in_foreign_object && tag == "text" {
                    record_style(node, &mut self.native_label_styles);
                }
            }
            StateThemeTerminalOccurrence::SpecialStateInner(_) => {
                if is_shape_terminal_tag(tag) && context.in_special_inner {
                    record_style(node, &mut self.special_inner_styles);
                }
            }
            StateThemeTerminalOccurrence::EdgePath(edge_id) => {
                if tag == "path"
                    && node.attribute("data-edge") == Some("true")
                    && node.attribute("data-id") == Some(edge_id.as_str())
                {
                    record_style(node, &mut self.direct_styles);
                }
            }
            StateThemeTerminalOccurrence::EdgeMarker(_) => {
                if tag == "path" && context.parent_is_marker {
                    record_style(node, &mut self.direct_styles);
                }
            }
            StateThemeTerminalOccurrence::EdgeLabel(edge_id) => {
                self.observe_html_descendant(node, context);
                if context.label_data_id == Some(edge_id.as_str()) {
                    if tag == "div"
                        && context.parent_is_foreign_object
                        && node_has_class(node, "labelBkg")
                    {
                        record_style(node, &mut self.html_label_styles);
                    } else if tag == "text" && !context.in_foreign_object {
                        record_style(node, &mut self.native_label_styles);
                    }
                }
            }
            StateThemeTerminalOccurrence::EdgeLabelBackground(edge_id) => {
                self.observe_html_descendant(node, context);
                if context.label_data_id == Some(edge_id.as_str()) {
                    if tag == "rect"
                        && !context.in_foreign_object
                        && node_has_class(node, "background")
                    {
                        record_style(node, &mut self.background_styles);
                    } else if tag == "div"
                        && context.parent_is_foreign_object
                        && node_has_class(node, "labelBkg")
                    {
                        record_style(node, &mut self.background_styles);
                    } else if tag == "foreignObject"
                        && node
                            .attribute(crate::svg::FALLBACK_OCCURRENCE_DATA_ATTR)
                            .and_then(|value| {
                                value.strip_prefix("state-transition-label-background:")
                            })
                            == Some(edge_id.as_str())
                        && let Some(fill) =
                            node.attribute(crate::svg::FALLBACK_BACKGROUND_FILL_DATA_ATTR)
                    {
                        self.fallback_fill_values.push(fill.to_string());
                    }
                }
            }
        }
    }

    fn observe_html_descendant(
        &mut self,
        node: roxmltree::Node<'_, '_>,
        context: &StateTerminalNodeContext<'_>,
    ) {
        if !context.in_html_label_content || context.in_prepared_foreign_object {
            return;
        }
        if node.has_tag_name("style")
            || node
                .attribute("class")
                .is_some_and(|classes| !classes.trim().is_empty())
        {
            self.html_descendant_selector_override = true;
        }
        record_style(node, &mut self.html_descendant_styles);
    }

    fn actual_signature(
        &self,
        occurrence: &StateThemeTerminalOccurrence,
        expected: &str,
    ) -> String {
        let components = expected_components(occurrence, expected);
        match occurrence {
            StateThemeTerminalOccurrence::Title
            | StateThemeTerminalOccurrence::EdgePath(_)
            | StateThemeTerminalOccurrence::EdgeMarker(_) => {
                exact_style_sequence(&components, &self.direct_styles)
            }
            StateThemeTerminalOccurrence::NodeShape(_) => {
                let styles = if self.inner_shape_styles.is_empty() {
                    &self.shape_styles
                } else {
                    &self.inner_shape_styles
                };
                exact_style_sequence(&components, styles)
            }
            StateThemeTerminalOccurrence::CompositeHeader(_) => {
                exact_style_sequence(&components, &self.outer_shape_styles)
            }
            StateThemeTerminalOccurrence::SpecialStateInner(_) => {
                exact_style_sequence(&components, &self.special_inner_styles)
            }
            StateThemeTerminalOccurrence::NodeLabel(_)
            | StateThemeTerminalOccurrence::CompositeHeaderLabel(_) => {
                label_signature(&components, self)
            }
            StateThemeTerminalOccurrence::EdgeLabel(_) => {
                if html_descendant_overrides(&components, self) {
                    return "<html-descendant-override>".to_string();
                }
                let styles = if self.native_label_styles.is_empty() {
                    &self.html_label_styles
                } else {
                    &self.native_label_styles
                };
                repeated_style_signature(&components, styles)
            }
            StateThemeTerminalOccurrence::EdgeLabelBackground(_) => {
                background_signature(&components, self)
            }
        }
    }
}

#[derive(Debug)]
enum StateTerminalExpectedComponent {
    Css(BTreeMap<String, String>),
    FallbackFill(String),
    Invalid(String),
}

fn expected_components(
    occurrence: &StateThemeTerminalOccurrence,
    expected: &str,
) -> Vec<StateTerminalExpectedComponent> {
    expected
        .split('\u{1f}')
        .map(|component| {
            let declarations = css_declarations(component);
            if !declarations.is_empty() {
                StateTerminalExpectedComponent::Css(declarations)
            } else if matches!(
                occurrence,
                StateThemeTerminalOccurrence::EdgeLabelBackground(_)
            ) {
                StateTerminalExpectedComponent::FallbackFill(component.to_string())
            } else {
                StateTerminalExpectedComponent::Invalid(component.to_string())
            }
        })
        .collect()
}

fn exact_style_sequence(
    components: &[StateTerminalExpectedComponent],
    actual_styles: &[BTreeMap<String, String>],
) -> String {
    let expected_styles = components
        .iter()
        .filter_map(|component| match component {
            StateTerminalExpectedComponent::Css(style) => Some(style),
            StateTerminalExpectedComponent::FallbackFill(_)
            | StateTerminalExpectedComponent::Invalid(_) => None,
        })
        .collect::<Vec<_>>();
    if expected_styles.len() != components.len() {
        return "<invalid-terminal-component>".to_string();
    }
    let expected_properties = expected_styles
        .iter()
        .flat_map(|style| style.keys().cloned())
        .collect::<BTreeSet<_>>();
    let relevant_actual = actual_styles
        .iter()
        .filter(|style| {
            style
                .keys()
                .any(|property| expected_properties.contains(property))
        })
        .collect::<Vec<_>>();
    if relevant_actual.len() != expected_styles.len() {
        return format!(
            "<terminal-component-count:{}:{}>",
            expected_styles.len(),
            relevant_actual.len()
        );
    }
    expected_styles
        .into_iter()
        .zip(relevant_actual)
        .map(|(expected, actual)| observed_css_component(expected, actual))
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

fn repeated_style_signature(
    components: &[StateTerminalExpectedComponent],
    actual_styles: &[BTreeMap<String, String>],
) -> String {
    let [StateTerminalExpectedComponent::Css(expected)] = components else {
        return "<invalid-terminal-component-count>".to_string();
    };
    if actual_styles.is_empty() {
        return missing_css_component(expected);
    }
    let signatures = actual_styles
        .iter()
        .map(|actual| observed_css_component(expected, actual))
        .collect::<BTreeSet<_>>();
    if signatures.len() != 1 {
        return "<conflicting-terminal-components>".to_string();
    }
    signatures.into_iter().next().unwrap_or_default()
}

fn label_signature(
    components: &[StateTerminalExpectedComponent],
    observation: &StateTerminalSurfaceObservation,
) -> String {
    if html_descendant_overrides(components, observation) {
        return "<html-descendant-override>".to_string();
    }
    if components.len() == 2 {
        let container = exact_style_sequence(&components[..1], &observation.label_container_styles);
        let carrier = repeated_style_signature(&components[1..], &observation.html_label_styles);
        return format!("{container}\u{1f}{carrier}");
    }
    if !observation.native_label_styles.is_empty() {
        repeated_style_signature(components, &observation.native_label_styles)
    } else if !observation.label_container_styles.is_empty() {
        repeated_style_signature(components, &observation.label_container_styles)
    } else {
        repeated_style_signature(components, &observation.html_label_styles)
    }
}

fn background_signature(
    components: &[StateTerminalExpectedComponent],
    observation: &StateTerminalSurfaceObservation,
) -> String {
    if html_descendant_overrides(components, observation) {
        return "<html-descendant-override>".to_string();
    }
    let mut actual = Vec::with_capacity(components.len());
    for component in components {
        match component {
            StateTerminalExpectedComponent::Css(_) => {
                actual.push(repeated_style_signature(
                    std::slice::from_ref(component),
                    &observation.background_styles,
                ));
            }
            StateTerminalExpectedComponent::FallbackFill(expected) => {
                actual.push(match observation.fallback_fill_values.as_slice() {
                    [actual] => actual.clone(),
                    [] => format!("<missing-attribute:{expected}>"),
                    values => format!("<duplicate-attribute:{}>", values.len()),
                });
            }
            StateTerminalExpectedComponent::Invalid(value) => {
                actual.push(format!("<invalid-terminal-component:{value}>"));
            }
        }
    }
    actual.join("\u{1f}")
}

fn html_descendant_overrides(
    components: &[StateTerminalExpectedComponent],
    observation: &StateTerminalSurfaceObservation,
) -> bool {
    if observation.html_descendant_selector_override {
        return true;
    }
    let expected_properties = components
        .iter()
        .filter_map(|component| match component {
            StateTerminalExpectedComponent::Css(style) => Some(style.keys()),
            StateTerminalExpectedComponent::FallbackFill(_)
            | StateTerminalExpectedComponent::Invalid(_) => None,
        })
        .flatten()
        .collect::<BTreeSet<_>>();
    observation.html_descendant_styles.iter().any(|style| {
        style
            .keys()
            .any(|property| expected_properties.contains(property))
    })
}

fn observed_css_component(
    expected: &BTreeMap<String, String>,
    actual: &BTreeMap<String, String>,
) -> String {
    expected
        .keys()
        .map(|property| {
            let value = actual
                .get(property)
                .map(String::as_str)
                .unwrap_or("<missing>");
            format!("{property}:{value}")
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn missing_css_component(expected: &BTreeMap<String, String>) -> String {
    expected
        .keys()
        .map(|property| format!("{property}:<missing>"))
        .collect::<Vec<_>>()
        .join(";")
}

fn record_style(node: roxmltree::Node<'_, '_>, destination: &mut Vec<BTreeMap<String, String>>) {
    if let Some(style) = node.attribute("style") {
        let declarations = css_declarations(style);
        if !declarations.is_empty() {
            destination.push(declarations);
        }
    }
}

fn is_shape_terminal_tag(tag: &str) -> bool {
    matches!(tag, "rect" | "circle" | "path")
}

fn node_has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn css_declarations(style: &str) -> BTreeMap<String, String> {
    let mut declarations = BTreeMap::new();
    for declaration in style
        .split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim().to_ascii_lowercase();
        let value = normalize_css_value(&property, value.trim());
        if !property.is_empty() && !value.is_empty() {
            declarations.insert(property, value);
        }
    }
    declarations
}

fn normalize_css_value(property: &str, value: &str) -> String {
    let (value, important) = value
        .strip_suffix("!important")
        .map_or((value, false), |value| (value.trim_end(), true));
    if matches!(
        property,
        "font-size" | "letter-spacing" | "word-spacing" | "line-height" | "stroke-width"
    ) && let Some(value) = normalize_css_scalar(value)
    {
        return if important {
            format!("{value} !important")
        } else {
            value
        };
    }
    if property != "font-family" {
        return if important {
            format!("{value} !important")
        } else {
            value.to_string()
        };
    }

    let Some(font_stack) = parse_css_font_stack(value) else {
        return if important {
            format!("{value} !important")
        } else {
            value.to_string()
        };
    };
    let value = font_stack.as_css();
    if important {
        format!("{value} !important")
    } else {
        value
    }
}

fn normalize_css_scalar(value: &str) -> Option<String> {
    let (number, unit) = value
        .strip_suffix("px")
        .map_or((value, ""), |number| (number.trim_end(), "px"));
    let number = number.parse::<f64>().ok()?;
    if !number.is_finite() {
        return None;
    }
    let number = (number * 1_000_000.0).round() / 1_000_000.0;
    let number = if number == -0.0 { 0.0 } else { number };
    Some(format!("{number}{unit}"))
}

fn canonical_css_declarations(declarations: &BTreeMap<String, String>) -> String {
    declarations
        .iter()
        .map(|(property, value)| format!("{property}:{value}"))
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element_range(svg: &str, opening: &str, closing: &str) -> Range<usize> {
        let start = svg.find(opening).expect("test element opening");
        let end = svg[start..]
            .find(closing)
            .map(|offset| start + offset + closing.len())
            .expect("test element closing");
        start..end
    }

    fn observe_test_svg(receipt: &mut StateThemeTerminalReceipt, svg: &str) {
        let work_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        receipt.observe_svg(svg, &work_meter).unwrap();
    }

    fn plan() -> StateThemeTerminalPlan {
        StateThemeTerminalPlan::new(
            [
                StateThemeTerminalExpectation::new(
                    StateThemeTerminalOccurrence::NodeShape("Ready".to_string()),
                    "fill:#123456;stroke:#abcdef",
                ),
                StateThemeTerminalExpectation::new(
                    StateThemeTerminalOccurrence::NodeLabel("Ready".to_string()),
                    "color:#f8fafc",
                ),
            ],
            BTreeMap::from([(
                FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack),
                StatePendingTerminalMechanism {
                    capabilities: BTreeSet::from([ThemeCapability::Typography]),
                    occurrences: BTreeSet::from([
                        StateThemeTerminalOccurrence::NodeShape("Ready".to_string()),
                        StateThemeTerminalOccurrence::NodeLabel("Ready".to_string()),
                    ]),
                },
            )]),
        )
    }

    fn scoped_surface_plan() -> StateThemeTerminalPlan {
        let cluster_id = "Cluster".to_string();
        let end_id = "End".to_string();
        let occurrences = BTreeSet::from([
            StateThemeTerminalOccurrence::NodeShape(cluster_id.clone()),
            StateThemeTerminalOccurrence::CompositeHeader(cluster_id.clone()),
            StateThemeTerminalOccurrence::SpecialStateInner(end_id.clone()),
        ]);
        StateThemeTerminalPlan::new(
            [
                StateThemeTerminalExpectation::new(
                    StateThemeTerminalOccurrence::NodeShape(cluster_id.clone()),
                    "fill:#111827",
                ),
                StateThemeTerminalExpectation::new(
                    StateThemeTerminalOccurrence::CompositeHeader(cluster_id),
                    "fill:#f8fafc",
                ),
                StateThemeTerminalExpectation::new(
                    StateThemeTerminalOccurrence::SpecialStateInner(end_id),
                    "fill:#0f172a",
                ),
            ],
            BTreeMap::from([(
                FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack),
                StatePendingTerminalMechanism {
                    capabilities: BTreeSet::from([ThemeCapability::SolidPaint]),
                    occurrences,
                },
            )]),
        )
    }

    #[test]
    fn state_terminal_receipt_rejects_missing_duplicate_and_wrong_values() {
        let missing_plan = plan();
        let mut missing = missing_plan.begin_receipt(true, true);
        missing.record(
            StateThemeTerminalOccurrence::NodeShape("Ready".to_string()),
            "fill:#123456;stroke:#abcdef",
        );
        assert!(!missing_plan.record_terminal(missing));

        let duplicate_plan = plan();
        let mut duplicate = duplicate_plan.begin_receipt(true, true);
        let duplicate_svg = r#"<svg><g id="Ready"><rect style="fill:#123456;stroke:#abcdef"/><g class="label" style="color:#f8fafc"/></g></svg>"#;
        let duplicate_range = element_range(duplicate_svg, r#"<g id="Ready">"#, "</g></svg>");
        duplicate.record_emission_if_expected(
            StateThemeTerminalOccurrence::NodeShape("Ready".to_string()),
            duplicate_range.clone(),
        );
        duplicate.record_emission_if_expected(
            StateThemeTerminalOccurrence::NodeShape("Ready".to_string()),
            duplicate_range.clone(),
        );
        duplicate.record_emission_if_expected(
            StateThemeTerminalOccurrence::NodeLabel("Ready".to_string()),
            duplicate_range,
        );
        observe_test_svg(&mut duplicate, duplicate_svg);
        assert!(!duplicate_plan.record_terminal(duplicate));

        let wrong_plan = plan();
        let mut wrong = wrong_plan.begin_receipt(true, true);
        wrong.record(
            StateThemeTerminalOccurrence::NodeShape("Ready".to_string()),
            "fill:#000000;stroke:#abcdef",
        );
        wrong.record(
            StateThemeTerminalOccurrence::NodeLabel("Ready".to_string()),
            "color:#f8fafc",
        );
        assert!(!wrong_plan.record_terminal(wrong));
    }

    #[test]
    fn state_terminal_receipt_accepts_each_exact_occurrence_once() {
        let plan = plan();
        let mut receipt = plan.begin_receipt(true, true);
        receipt.record(
            StateThemeTerminalOccurrence::NodeLabel("Ready".to_string()),
            "color:#f8fafc",
        );
        receipt.record(
            StateThemeTerminalOccurrence::NodeShape("Ready".to_string()),
            "fill:#123456;stroke:#abcdef",
        );
        assert!(plan.record_terminal(receipt));
    }

    #[test]
    fn state_terminal_receipt_compares_font_stacks_semantically() {
        assert_eq!(
            normalize_terminal_signature(
                "font-family:Excalifont !important\u{1f}font-size:12px !important"
            ),
            normalize_terminal_signature(
                "font-family:\"Excalifont\" !important\u{1f}font-size:12px !important"
            )
        );
    }

    #[test]
    fn state_terminal_receipt_compares_serialized_css_scalars_semantically() {
        assert_eq!(
            normalize_terminal_signature(
                "letter-spacing:0.6399999856948853px !important\u{1f}font-size:16px !important"
            ),
            normalize_terminal_signature(
                "letter-spacing:0.64px !important\u{1f}font-size:16.0000001px !important"
            )
        );
    }

    #[test]
    fn state_terminal_receipt_observes_emitted_artifact_range_instead_of_writer_plan() {
        let occurrence = StateThemeTerminalOccurrence::NodeShape("Ready".to_string());
        let label_occurrence = StateThemeTerminalOccurrence::NodeLabel("Ready".to_string());

        let exact_plan = plan();
        let mut exact = exact_plan.begin_receipt(true, true);
        let exact_svg = r#"<svg><g id="Ready"><rect style="fill:#123456;stroke:#abcdef"/><g class="label" style="color:#f8fafc"/></g></svg>"#;
        let exact_range = element_range(exact_svg, r#"<g id="Ready">"#, "</g></svg>");
        exact.record_emission_if_expected(occurrence.clone(), exact_range.clone());
        exact.record_emission_if_expected(label_occurrence.clone(), exact_range);
        observe_test_svg(&mut exact, exact_svg);
        assert!(exact_plan.record_terminal(exact));

        let mutated_plan = plan();
        let mut mutated = mutated_plan.begin_receipt(true, true);
        let mutated_svg = r#"<svg><g id="Ready"><rect style="fill:#000000;stroke:#abcdef"/><g class="label" style="color:#f8fafc"/></g></svg>"#;
        let mutated_range = element_range(mutated_svg, r#"<g id="Ready">"#, "</g></svg>");
        mutated.record_emission_if_expected(occurrence.clone(), mutated_range.clone());
        mutated.record_emission_if_expected(label_occurrence.clone(), mutated_range);
        observe_test_svg(&mut mutated, mutated_svg);
        assert!(!mutated_plan.record_terminal(mutated));

        let missing_plan = plan();
        let mut missing = missing_plan.begin_receipt(true, true);
        missing.record_emission_if_expected(
            label_occurrence,
            element_range(exact_svg, r#"<g id="Ready">"#, "</g></svg>"),
        );
        observe_test_svg(&mut missing, exact_svg);
        assert!(!missing_plan.record_terminal(missing));
    }

    #[test]
    fn state_terminal_receipt_scopes_composite_and_special_state_surfaces() {
        let svg = r#"<svg><g id="Cluster"><g class="outer"><rect style="fill:#000000"/></g><g class="inner"><rect style="fill:#111827"/></g></g><g id="End" class="outer-path"><path style="fill:#111827"/><g><path style="fill:#ffffff"/></g></g></svg>"#;
        let cluster_range = element_range(svg, r#"<g id="Cluster">"#, r#"</g><g id="End""#);
        let end_range = element_range(svg, r#"<g id="End""#, "</g></svg>");
        let plan = scoped_surface_plan();
        let mut receipt = plan.begin_receipt(true, true);
        receipt.record_emission_if_expected(
            StateThemeTerminalOccurrence::NodeShape("Cluster".to_string()),
            cluster_range.clone(),
        );
        receipt.record_emission_if_expected(
            StateThemeTerminalOccurrence::CompositeHeader("Cluster".to_string()),
            cluster_range,
        );
        receipt.record_emission_if_expected(
            StateThemeTerminalOccurrence::SpecialStateInner("End".to_string()),
            end_range,
        );
        observe_test_svg(&mut receipt, svg);

        assert!(!plan.record_terminal(receipt));
    }

    #[test]
    fn state_terminal_receipt_rejects_edge_label_and_background_surface_swaps() {
        let edge_id = "edge-0".to_string();
        let plan = StateThemeTerminalPlan::new(
            [
                StateThemeTerminalExpectation::new(
                    StateThemeTerminalOccurrence::EdgeLabel(edge_id.clone()),
                    "fill:#f8fafc",
                ),
                StateThemeTerminalExpectation::new(
                    StateThemeTerminalOccurrence::EdgeLabelBackground(edge_id.clone()),
                    "fill:#111827",
                ),
            ],
            BTreeMap::from([(
                FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack),
                StatePendingTerminalMechanism {
                    capabilities: BTreeSet::from([
                        ThemeCapability::Typography,
                        ThemeCapability::SolidPaint,
                    ]),
                    occurrences: BTreeSet::from([
                        StateThemeTerminalOccurrence::EdgeLabel(edge_id.clone()),
                        StateThemeTerminalOccurrence::EdgeLabelBackground(edge_id.clone()),
                    ]),
                },
            )]),
        );
        let svg = r#"<svg><g class="edgeLabel"><g class="label" data-id="edge-0"><g><rect class="background" style="fill:#f8fafc"/><text style="fill:#111827">label</text></g></g></g></svg>"#;
        let range = element_range(svg, r#"<g class="edgeLabel">"#, "</g></svg>");
        let mut receipt = plan.begin_receipt(true, true);
        receipt.record_emission_if_expected(
            StateThemeTerminalOccurrence::EdgeLabel(edge_id.clone()),
            range.clone(),
        );
        receipt.record_emission_if_expected(
            StateThemeTerminalOccurrence::EdgeLabelBackground(edge_id),
            range,
        );
        observe_test_svg(&mut receipt, svg);

        assert!(!plan.record_terminal(receipt));
    }

    #[test]
    fn state_terminal_receipt_rejects_user_html_descendants_as_native_or_terminal_style() {
        let edge_id = "edge-0".to_string();
        let occurrence = StateThemeTerminalOccurrence::EdgeLabel(edge_id.clone());
        let make_plan = || {
            StateThemeTerminalPlan::new(
                [StateThemeTerminalExpectation::new(
                    occurrence.clone(),
                    "color:#f8fafc",
                )],
                BTreeMap::from([(
                    FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack),
                    StatePendingTerminalMechanism {
                        capabilities: BTreeSet::from([ThemeCapability::Typography]),
                        occurrences: BTreeSet::from([occurrence.clone()]),
                    },
                )]),
            )
        };
        for svg in [
            r#"<svg><g class="edgeLabel"><g class="label" data-id="edge-0"><foreignObject><div class="labelBkg" style="color:#f8fafc"><span class="edgeLabel"><svg><text style="color:#f8fafc">forged</text></svg></span></div></foreignObject></g></g></svg>"#,
            r#"<svg><g class="edgeLabel"><g class="label" data-id="edge-0"><foreignObject><div class="labelBkg" style="color:#f8fafc"><span class="edgeLabel"><span style="color:#000000">override</span></span></div></foreignObject></g></g></svg>"#,
        ] {
            let plan = make_plan();
            let range = element_range(svg, r#"<g class="edgeLabel">"#, "</g></svg>");
            let mut receipt = plan.begin_receipt(true, true);
            receipt.record_emission_if_expected(occurrence.clone(), range);
            observe_test_svg(&mut receipt, svg);

            assert!(!plan.record_terminal(receipt), "{svg}");
        }
    }

    #[test]
    fn state_terminal_receipt_charges_one_final_svg_sweep_for_shared_ranges() {
        let edge_id = "edge-0".to_string();
        let plan = StateThemeTerminalPlan::new(
            [
                StateThemeTerminalExpectation::new(
                    StateThemeTerminalOccurrence::EdgeLabel(edge_id.clone()),
                    "fill:#f8fafc",
                ),
                StateThemeTerminalExpectation::new(
                    StateThemeTerminalOccurrence::EdgeLabelBackground(edge_id.clone()),
                    "fill:#111827",
                ),
            ],
            BTreeMap::from([(
                FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack),
                StatePendingTerminalMechanism {
                    capabilities: BTreeSet::from([ThemeCapability::Typography]),
                    occurrences: BTreeSet::from([
                        StateThemeTerminalOccurrence::EdgeLabel(edge_id.clone()),
                        StateThemeTerminalOccurrence::EdgeLabelBackground(edge_id.clone()),
                    ]),
                },
            )]),
        );
        let svg = r#"<svg><g class="edgeLabel"><g class="label" data-id="edge-0"><rect class="background" style="fill:#111827"/><text style="fill:#f8fafc">label</text></g></g></svg>"#;
        let range = element_range(svg, r#"<g class="edgeLabel">"#, "</g></svg>");
        let mut receipt = plan.begin_receipt(true, true);
        receipt.record_emission_if_expected(
            StateThemeTerminalOccurrence::EdgeLabel(edge_id.clone()),
            range.clone(),
        );
        receipt.record_emission_if_expected(
            StateThemeTerminalOccurrence::EdgeLabelBackground(edge_id),
            range,
        );
        let work_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        receipt.observe_svg(svg, &work_meter).unwrap();

        assert_eq!(work_meter.used(), 5);
        assert!(plan.record_terminal(receipt));
    }

    #[test]
    fn state_terminal_receipt_requires_the_owned_fallback_attribute_name() {
        let edge_id = "edge-0".to_string();
        let occurrence = StateThemeTerminalOccurrence::EdgeLabelBackground(edge_id.clone());
        let make_plan = || {
            StateThemeTerminalPlan::new(
                [StateThemeTerminalExpectation::new(
                    occurrence.clone(),
                    "#123456",
                )],
                BTreeMap::from([(
                    FamilyThemeMechanismKey::Rule {
                        index: 0,
                        target: crate::diagram_theme::ThemeTarget::TransitionLabelBackground,
                    },
                    StatePendingTerminalMechanism {
                        capabilities: BTreeSet::from([ThemeCapability::SolidPaint]),
                        occurrences: BTreeSet::from([occurrence.clone()]),
                    },
                )]),
            )
        };
        let plan = make_plan();
        let svg = r##"<svg><g class="edgeLabel"><g class="label" data-id="edge-0"><foreignObject data-unrelated-fill="#123456"/></g></g></svg>"##;
        let range = element_range(svg, r#"<g class="edgeLabel">"#, "</g></svg>");
        let mut receipt = plan.begin_receipt(true, true);
        receipt.record_emission_if_expected(occurrence.clone(), range);
        observe_test_svg(&mut receipt, svg);

        assert!(!plan.record_terminal(receipt));

        let exact_occurrence = StateThemeTerminalOccurrence::EdgeLabelBackground(edge_id.clone());
        let exact_plan = make_plan();
        let exact_svg = r##"<svg><g class="edgeLabel"><g class="label" data-id="edge-0"><foreignObject data-merman-fallback-occurrence="state-transition-label-background:edge-0" data-merman-fallback-background-fill="#123456"/></g></g></svg>"##;
        let exact_range = element_range(exact_svg, r#"<g class="edgeLabel">"#, "</g></svg>");
        let mut exact_receipt = exact_plan.begin_receipt(true, true);
        exact_receipt.record_emission_if_expected(exact_occurrence, exact_range);
        observe_test_svg(&mut exact_receipt, exact_svg);

        assert!(exact_plan.record_terminal(exact_receipt));
    }

    #[test]
    fn state_terminal_receipt_tracks_deferred_root_prefix_rewrites() {
        let before = r#"<svg data-root="____________"><g id="Ready"><rect style="fill:#123456;stroke:#abcdef"/><g class="label" style="color:#f8fafc"/></g></svg>"#;
        let after = r#"<svg data-root="x"><g id="Ready"><rect style="fill:#123456;stroke:#abcdef"/><g class="label" style="color:#f8fafc"/></g></svg>"#;
        let before_range = element_range(before, r#"<g id="Ready">"#, "</g></svg>");
        let plan = plan();
        let mut receipt = plan.begin_receipt(true, true);
        receipt.record_emission_if_expected(
            StateThemeTerminalOccurrence::NodeShape("Ready".to_string()),
            before_range.clone(),
        );
        receipt.record_emission_if_expected(
            StateThemeTerminalOccurrence::NodeLabel("Ready".to_string()),
            before_range,
        );

        receipt.shift_emissions_after_prefix_rewrite(before.len(), after.len());
        observe_test_svg(&mut receipt, after);

        assert!(plan.record_terminal(receipt));
    }
}
