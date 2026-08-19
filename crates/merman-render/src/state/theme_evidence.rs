use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::OnceLock;

use crate::diagram_theme::{FamilyThemeMechanismKey, ThemeCapability};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
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
    pub(crate) fn observe_svg(&mut self, svg: &str) {
        if self.invalid || self.expected.is_empty() || self.emissions.is_empty() {
            return;
        }
        let Ok(document) = roxmltree::Document::parse(svg) else {
            self.invalid = true;
            return;
        };
        let mut observed = Vec::with_capacity(self.emissions.len());
        for (occurrence, range) in &self.emissions {
            if range.end > svg.len()
                || !svg.is_char_boundary(range.start)
                || !svg.is_char_boundary(range.end)
            {
                self.invalid = true;
                return;
            }
            let Some(expected) = self.expected.get(occurrence) else {
                self.invalid = true;
                return;
            };
            observed.push((
                occurrence.clone(),
                observe_terminal_signature(occurrence, expected, range, &document),
            ));
        }
        for (occurrence, actual) in observed {
            if self.actual.insert(occurrence, actual).is_some() {
                self.invalid = true;
            }
        }
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

fn observe_terminal_signature(
    occurrence: &StateThemeTerminalOccurrence,
    expected: &str,
    emitted_range: &Range<usize>,
    document: &roxmltree::Document<'_>,
) -> String {
    expected
        .split('\u{1f}')
        .map(|component| observe_terminal_component(occurrence, component, emitted_range, document))
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

fn observe_terminal_component(
    occurrence: &StateThemeTerminalOccurrence,
    component: &str,
    emitted_range: &Range<usize>,
    document: &roxmltree::Document<'_>,
) -> String {
    let expected_declarations = css_declarations(component);
    let composite_inner_exists = matches!(occurrence, StateThemeTerminalOccurrence::NodeShape(_))
        && document
            .descendants()
            .filter(|candidate| node_is_within(*candidate, emitted_range))
            .any(|candidate| {
                node_has_class(candidate, "inner")
                    && !node_or_ancestor_has_class(candidate, "label")
                    && !node_or_ancestor_has_class(candidate, "cluster-label")
            });
    if expected_declarations.is_empty() {
        return document
            .descendants()
            .filter(|node| node_is_within(*node, emitted_range))
            .filter(|node| terminal_candidate(*node, occurrence, composite_inner_exists))
            .flat_map(|node| node.attributes())
            .find(|attribute| attribute.value() == component)
            .map_or_else(
                || format!("<missing-attribute:{component}>"),
                |_| component.to_string(),
            );
    }

    let mut best = None::<(usize, usize, BTreeMap<String, String>)>;
    for style in document
        .descendants()
        .filter(|node| node_is_within(*node, emitted_range))
        .filter(|node| terminal_candidate(*node, occurrence, composite_inner_exists))
        .filter_map(|node| node.attribute("style"))
    {
        let actual = css_declarations(style);
        let present = expected_declarations
            .keys()
            .filter(|property| actual.contains_key(*property))
            .count();
        let exact = expected_declarations
            .iter()
            .filter(|(property, value)| actual.get(*property) == Some(*value))
            .count();
        if best.as_ref().is_none_or(|(best_present, best_exact, _)| {
            (present, exact) > (*best_present, *best_exact)
        }) {
            best = Some((present, exact, actual));
        }
    }

    let Some((_, _, actual)) = best else {
        return expected_declarations
            .keys()
            .map(|property| format!("{property}:<missing>"))
            .collect::<Vec<_>>()
            .join(";");
    };
    expected_declarations
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

fn node_is_within(node: roxmltree::Node<'_, '_>, emitted_range: &Range<usize>) -> bool {
    let node_range = node.range();
    node_range.start >= emitted_range.start && node_range.end <= emitted_range.end
}

fn terminal_candidate(
    node: roxmltree::Node<'_, '_>,
    occurrence: &StateThemeTerminalOccurrence,
    composite_inner_exists: bool,
) -> bool {
    if !node.is_element() {
        return false;
    }
    match occurrence {
        StateThemeTerminalOccurrence::NodeLabel(_) => {
            node_or_ancestor_has_class(node, "label")
                && !node_or_ancestor_has_class(node, "cluster-label")
        }
        StateThemeTerminalOccurrence::CompositeHeaderLabel(_) => {
            node_or_ancestor_has_class(node, "cluster-label")
        }
        StateThemeTerminalOccurrence::CompositeHeader(_) => {
            node_or_ancestor_has_class(node, "outer")
        }
        StateThemeTerminalOccurrence::SpecialStateInner(_) => is_special_state_inner(node),
        StateThemeTerminalOccurrence::NodeShape(_) => {
            if composite_inner_exists {
                node_or_ancestor_has_class(node, "inner")
            } else {
                !node_or_ancestor_has_class(node, "label")
                    && !node_or_ancestor_has_class(node, "cluster-label")
                    && !is_special_state_inner(node)
            }
        }
        StateThemeTerminalOccurrence::Title
        | StateThemeTerminalOccurrence::EdgePath(_)
        | StateThemeTerminalOccurrence::EdgeMarker(_)
        | StateThemeTerminalOccurrence::EdgeLabel(_)
        | StateThemeTerminalOccurrence::EdgeLabelBackground(_) => true,
    }
}

fn node_has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    node.attribute("class")
        .is_some_and(|classes| classes.split_ascii_whitespace().any(|value| value == class))
}

fn node_or_ancestor_has_class(node: roxmltree::Node<'_, '_>, class: &str) -> bool {
    std::iter::once(node)
        .chain(node.ancestors())
        .any(|ancestor| node_has_class(ancestor, class))
}

fn is_special_state_inner(node: roxmltree::Node<'_, '_>) -> bool {
    std::iter::once(node)
        .chain(node.ancestors())
        .any(|ancestor| {
            ancestor.is_element()
                && ancestor.tag_name().name() == "g"
                && ancestor.attribute("class").is_none()
                && ancestor
                    .parent()
                    .is_some_and(|parent| node_has_class(parent, "outer-path"))
        })
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
                FamilyThemeMechanismKey::Typography,
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
                FamilyThemeMechanismKey::Typography,
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
        duplicate.observe_svg(duplicate_svg);
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
        exact.observe_svg(exact_svg);
        assert!(exact_plan.record_terminal(exact));

        let mutated_plan = plan();
        let mut mutated = mutated_plan.begin_receipt(true, true);
        let mutated_svg = r#"<svg><g id="Ready"><rect style="fill:#000000;stroke:#abcdef"/><g class="label" style="color:#f8fafc"/></g></svg>"#;
        let mutated_range = element_range(mutated_svg, r#"<g id="Ready">"#, "</g></svg>");
        mutated.record_emission_if_expected(occurrence.clone(), mutated_range.clone());
        mutated.record_emission_if_expected(label_occurrence.clone(), mutated_range);
        mutated.observe_svg(mutated_svg);
        assert!(!mutated_plan.record_terminal(mutated));

        let missing_plan = plan();
        let mut missing = missing_plan.begin_receipt(true, true);
        missing.record_emission_if_expected(
            label_occurrence,
            element_range(exact_svg, r#"<g id="Ready">"#, "</g></svg>"),
        );
        missing.observe_svg(exact_svg);
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
        receipt.observe_svg(svg);

        assert!(!plan.record_terminal(receipt));
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
        receipt.observe_svg(after);

        assert!(plan.record_terminal(receipt));
    }
}
