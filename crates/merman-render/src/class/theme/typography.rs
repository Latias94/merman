use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan,
};
use crate::text::TextStyle;

const CARDINALITY_SLOT_COUNT: usize = 4;

/// Class keeps Mermaid's two configured font winners until a typed stack owns both layers.
///
/// Layout measurement reads root `fontFamily` first. The stylesheet reads
/// `themeVariables.fontFamily` first. A directly owned FontStack intentionally replaces both;
/// inactive, config-owned, and unsupported routes preserve the upstream split.
#[derive(Debug)]
pub(crate) struct ClassTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    layout_font_family_css: Box<str>,
    stylesheet_font_family_css: Box<str>,
    node_style_facts: OnceLock<BTreeMap<Box<str>, ClassNodeLabelStyleFacts>>,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<ClassTypographyThemeReceipt>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ClassNodeLabelStyleFacts {
    visible_runs: usize,
    inherited_color_runs: usize,
    color_ownership_unverified: bool,
    layout_inherited_font_runs: usize,
    writer_inherited_font_runs: usize,
    unverified_font_runs: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ClassTypographyTerminalFacts {
    visible_runs: usize,
    layout_inherited_runs: usize,
    writer_inherited_runs: usize,
    unverified_runs: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ClassTypographyCssEmission {
    font_family_css: Box<str>,
    complete: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct ClassTypographyEdgeCheckpoint {
    label: ClassTypographyTerminalCheckpoint,
    cardinalities: [ClassTypographyTerminalCheckpoint; CARDINALITY_SLOT_COUNT],
}

#[derive(Debug, Clone, Copy, Default)]
struct ClassTypographyTerminalCheckpoint {
    expected_visible: bool,
    observed: Option<ClassTypographyTerminalFacts>,
}

/// Renderer-owned proof for the Class font-stack stylesheet and every text-bearing object.
///
/// The receipt records semantic writer checkpoints. It does not parse the finalized stylesheet or
/// SVG, and it does not reconstruct browser cascade semantics.
#[derive(Debug, Clone)]
pub(crate) struct ClassTypographyThemeReceipt {
    expected_font_family_css: Box<str>,
    css_emission: Option<ClassTypographyCssEmission>,
    css_emission_unique: bool,
    nodes: BTreeMap<Box<str>, ClassTypographyTerminalCheckpoint>,
    namespaces: BTreeMap<Box<str>, ClassTypographyTerminalCheckpoint>,
    edges: BTreeMap<Box<str>, ClassTypographyEdgeCheckpoint>,
    diagram_title: ClassTypographyTerminalCheckpoint,
    terminals_match: bool,
}

impl ClassTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
    ) -> Self {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let configured_layout_font =
            super::super::config::ClassConfigView::new(effective_config.as_value())
                .layout_font_family_css();
        let stylesheet_font_family_css =
            crate::config::normalize_css_font_family(inherited_font_stack.font_family_css());
        let layout_font_family_css = if inherited_font_stack.typed_font_stack_active() {
            stylesheet_font_family_css.clone()
        } else {
            configured_layout_font
        };
        Self {
            inherited_font_stack,
            layout_font_family_css: layout_font_family_css.into_boxed_str(),
            stylesheet_font_family_css: stylesheet_font_family_css.into_boxed_str(),
            node_style_facts: OnceLock::new(),
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn layout_font_family_css(&self) -> &str {
        &self.layout_font_family_css
    }

    pub(crate) fn stylesheet_font_family_css(&self) -> &str {
        &self.stylesheet_font_family_css
    }

    pub(crate) fn apply_layout_text_styles(
        &self,
        text_style: &mut TextStyle,
        html_calc_text_style: &mut TextStyle,
    ) {
        let font_family = Some(self.layout_font_family_css().to_string());
        text_style.font_family = font_family.clone();
        html_calc_text_style.font_family = font_family;
    }

    pub(crate) fn seal_node_style_facts(
        &self,
        facts: BTreeMap<Box<str>, ClassNodeLabelStyleFacts>,
    ) -> bool {
        self.node_style_facts.set(facts).is_ok()
    }

    pub(crate) fn require_node_style_facts(
        &self,
        id: &str,
    ) -> crate::Result<ClassNodeLabelStyleFacts> {
        self.node_style_facts
            .get()
            .and_then(|facts| facts.get(id))
            .copied()
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!(
                    "Class label style facts were not prepared for semantic node `{id}`"
                ),
            })
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        model: &merman_core::models::class_diagram::ClassDiagram,
        diagram_title: Option<&str>,
        diagram_use_html_labels: bool,
        edge_use_html_labels: bool,
        sanitize_config: Option<&merman_core::MermaidConfig>,
    ) -> Option<ClassTypographyThemeReceipt> {
        (self.inherited_font_stack.typed_font_stack_active()
            || self
                .inherited_font_stack
                .has_unsupported_typography_properties())
        .then(|| {
            let mut terminals_match = true;
            let mut nodes = BTreeMap::new();
            let node_style_facts = self.node_style_facts.get();
            terminals_match &= node_style_facts.is_some_and(|facts| {
                facts.len() == model.classes.len()
                    && facts
                        .keys()
                        .all(|id| model.classes.contains_key(id.as_ref()))
            });
            for (id, _) in &model.classes {
                let expected_visible = node_style_facts
                    .and_then(|facts| facts.get(id.as_str()))
                    .map(|facts| facts.visible_run_count() != 0)
                    .unwrap_or_else(|| {
                        terminals_match = false;
                        false
                    });
                insert_terminal_expectation(&mut nodes, id, expected_visible, &mut terminals_match);
            }
            for note in &model.notes {
                insert_terminal_expectation(
                    &mut nodes,
                    &note.id,
                    class_note_expected_visibility(
                        &note.text,
                        diagram_use_html_labels,
                        sanitize_config,
                    ),
                    &mut terminals_match,
                );
            }
            for interface in &model.interfaces {
                let label = crate::entities::decode_entities_minimal_cow(interface.label.trim());
                insert_terminal_expectation(
                    &mut nodes,
                    &interface.id,
                    !label.trim().is_empty(),
                    &mut terminals_match,
                );
            }

            let mut namespaces = BTreeMap::new();
            for id in model.namespaces.keys() {
                insert_terminal_expectation(
                    &mut namespaces,
                    id,
                    !super::super::class_namespace_label(model, id)
                        .trim()
                        .is_empty(),
                    &mut terminals_match,
                );
            }

            let mut edges = BTreeMap::new();
            for relation in &model.relations {
                let label =
                    class_edge_label_expected_visibility(&relation.title, edge_use_html_labels);
                let start = class_terminal_expected_visibility(
                    relation.relation_title_1.as_deref().unwrap_or_default(),
                );
                let end = class_terminal_expected_visibility(
                    relation.relation_title_2.as_deref().unwrap_or_default(),
                );
                if edges
                    .insert(
                        relation.id.as_str().into(),
                        ClassTypographyEdgeCheckpoint {
                            label: ClassTypographyTerminalCheckpoint::new(label),
                            cardinalities: [
                                ClassTypographyTerminalCheckpoint::new(false),
                                ClassTypographyTerminalCheckpoint::new(start),
                                ClassTypographyTerminalCheckpoint::new(end),
                                ClassTypographyTerminalCheckpoint::new(false),
                            ],
                        },
                    )
                    .is_some()
                {
                    terminals_match = false;
                }
            }

            ClassTypographyThemeReceipt {
                expected_font_family_css: self.stylesheet_font_family_css().into(),
                css_emission: None,
                css_emission_unique: true,
                nodes,
                namespaces,
                edges,
                diagram_title: ClassTypographyTerminalCheckpoint::new(
                    diagram_title.is_some_and(|title| !title.trim().is_empty()),
                ),
                terminals_match,
            }
        })
    }

    pub(crate) fn record_terminal(&self, receipt: Option<ClassTypographyThemeReceipt>) -> bool {
        if self.inherited_font_stack.typed_font_stack_active()
            || self
                .inherited_font_stack
                .has_unsupported_typography_properties()
        {
            receipt.is_some_and(|receipt| {
                receipt.structurally_complete() && self.terminal_receipt.set(receipt).is_ok()
            })
        } else {
            receipt.is_none()
        }
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(
                &mut evidence,
                self.terminal_receipt
                    .get()
                    .is_none_or(ClassTypographyThemeReceipt::has_visible_font_run),
            );
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        match self.inherited_font_stack.outcome() {
            InheritedFontStackOutcome::Typed => {
                match self.terminal_receipt.get() {
                    Some(receipt) if !receipt.structurally_complete() => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                    Some(receipt) if receipt.has_unverified_font_run() => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                    Some(receipt) if receipt.has_applied_font_run() => {
                        evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                    }
                    Some(_) => evidence.mark_not_applicable(key),
                    None => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                }
            }
            InheritedFontStackOutcome::ConfigOwned => evidence.mark_not_applicable(key),
            InheritedFontStackOutcome::Unsupported => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography)
            }
            InheritedFontStackOutcome::Inactive => {}
        }
        evidence
    }
}

impl ClassNodeLabelStyleFacts {
    pub(crate) fn observe(
        &mut self,
        facts: &crate::text::VisibleTextStyleFacts,
        parent_owns_color: bool,
        color_ownership_unverified: bool,
        layout_parent_owns_font: bool,
        writer_parent_owns_font: bool,
        font_ownership_unverified: bool,
    ) {
        if !facts.parse_valid() {
            self.visible_runs = self.visible_runs.saturating_add(1);
            self.color_ownership_unverified |= !parent_owns_color;
            if font_ownership_unverified || !layout_parent_owns_font || !writer_parent_owns_font {
                self.unverified_font_runs = self.unverified_font_runs.saturating_add(1);
            }
            return;
        }
        let visible_runs = facts.visible_run_count();
        self.visible_runs = self.visible_runs.saturating_add(visible_runs);
        self.color_ownership_unverified |= color_ownership_unverified;
        if !parent_owns_color {
            self.inherited_color_runs = self
                .inherited_color_runs
                .saturating_add(facts.inherited_color_run_count());
        }
        if font_ownership_unverified {
            self.unverified_font_runs = self.unverified_font_runs.saturating_add(visible_runs);
            return;
        }
        self.unverified_font_runs = self
            .unverified_font_runs
            .saturating_add(facts.unverified_font_family_run_count());
        if !layout_parent_owns_font {
            self.layout_inherited_font_runs = self
                .layout_inherited_font_runs
                .saturating_add(facts.inherited_font_family_run_count());
        }
        if !writer_parent_owns_font {
            self.writer_inherited_font_runs = self
                .writer_inherited_font_runs
                .saturating_add(facts.inherited_font_family_run_count());
        }
    }

    pub(crate) fn merge(&mut self, other: Self) {
        self.visible_runs = self.visible_runs.saturating_add(other.visible_runs);
        self.inherited_color_runs = self
            .inherited_color_runs
            .saturating_add(other.inherited_color_runs);
        self.color_ownership_unverified |= other.color_ownership_unverified;
        self.layout_inherited_font_runs = self
            .layout_inherited_font_runs
            .saturating_add(other.layout_inherited_font_runs);
        self.writer_inherited_font_runs = self
            .writer_inherited_font_runs
            .saturating_add(other.writer_inherited_font_runs);
        self.unverified_font_runs = self
            .unverified_font_runs
            .saturating_add(other.unverified_font_runs);
    }

    pub(crate) const fn source_owns_every_visible_run(self) -> bool {
        self.visible_runs != 0 && self.inherited_color_runs == 0 && !self.color_ownership_unverified
    }

    pub(crate) const fn has_mixed_color_ownership(self) -> bool {
        self.inherited_color_runs != 0 && self.inherited_color_runs < self.visible_runs
    }

    pub(crate) const fn color_ownership_is_unverified(self) -> bool {
        self.color_ownership_unverified
    }

    pub(crate) const fn visible_run_count(self) -> usize {
        self.visible_runs
    }

    pub(crate) const fn layout_inherited_font_run_count(self) -> usize {
        self.layout_inherited_font_runs
    }

    pub(crate) const fn writer_inherited_font_run_count(self) -> usize {
        self.writer_inherited_font_runs
    }

    pub(crate) const fn unverified_font_run_count(self) -> usize {
        self.unverified_font_runs
    }
}

impl ClassTypographyTerminalFacts {
    pub(crate) const fn new(
        visible_runs: usize,
        layout_inherited_runs: usize,
        writer_inherited_runs: usize,
        unverified_runs: usize,
    ) -> Self {
        Self {
            visible_runs,
            layout_inherited_runs,
            writer_inherited_runs,
            unverified_runs,
        }
    }

    pub(crate) fn from_node_style_facts(facts: ClassNodeLabelStyleFacts) -> Self {
        Self::new(
            facts.visible_run_count(),
            facts.layout_inherited_font_run_count(),
            facts.writer_inherited_font_run_count(),
            facts.unverified_font_run_count(),
        )
    }

    pub(crate) fn inherited_text(text: &str) -> Self {
        let visible_runs = crate::text::VisibleTextStyleFacts::plain_text(text).visible_run_count();
        Self::new(visible_runs, visible_runs, visible_runs, 0)
    }

    pub(crate) fn unverified_text(text: &str) -> Self {
        let visible_runs = crate::text::VisibleTextStyleFacts::plain_text(text).visible_run_count();
        Self::new(visible_runs, 0, 0, visible_runs)
    }

    pub(crate) fn from_visible_style_facts(facts: &crate::text::VisibleTextStyleFacts) -> Self {
        if !facts.parse_valid() {
            return Self::new(1, 0, 0, 1);
        }
        let visible_runs = facts.visible_run_count();
        let inherited_runs = facts.inherited_font_family_run_count();
        Self::new(
            visible_runs,
            inherited_runs,
            inherited_runs,
            facts.unverified_font_family_run_count(),
        )
    }

    fn merge(&mut self, other: Self) {
        self.visible_runs = self.visible_runs.saturating_add(other.visible_runs);
        self.layout_inherited_runs = self
            .layout_inherited_runs
            .saturating_add(other.layout_inherited_runs);
        self.writer_inherited_runs = self
            .writer_inherited_runs
            .saturating_add(other.writer_inherited_runs);
        self.unverified_runs = self.unverified_runs.saturating_add(other.unverified_runs);
    }
}

impl ClassTypographyCssEmission {
    pub(crate) fn from_successful_writes(
        diagram_root_font_family_css: &str,
        nested_svg_font_family_css: &str,
        class_group_font_family_css: &str,
        root_variable_font_family_css: &str,
    ) -> Self {
        Self {
            font_family_css: diagram_root_font_family_css.into(),
            complete: [
                nested_svg_font_family_css,
                class_group_font_family_css,
                root_variable_font_family_css,
            ]
            .into_iter()
            .all(|font_family| font_family == diagram_root_font_family_css),
        }
    }
}

impl ClassTypographyThemeReceipt {
    pub(crate) fn record_css_emission(&mut self, emission: ClassTypographyCssEmission) {
        if self.css_emission.is_some() {
            self.css_emission_unique = false;
            return;
        }
        self.css_emission = Some(emission);
    }

    pub(crate) fn record_node(&mut self, id: &str, facts: ClassTypographyTerminalFacts) {
        record_terminal_slot(self.nodes.get_mut(id), facts, &mut self.terminals_match);
    }

    pub(crate) fn record_namespace(&mut self, id: &str, facts: ClassTypographyTerminalFacts) {
        record_terminal_slot(
            self.namespaces.get_mut(id),
            facts,
            &mut self.terminals_match,
        );
    }

    pub(crate) fn record_edge_label(&mut self, id: &str, facts: ClassTypographyTerminalFacts) {
        let Some(edge) = self.edges.get_mut(id) else {
            self.terminals_match = false;
            return;
        };
        record_terminal_slot(Some(&mut edge.label), facts, &mut self.terminals_match);
    }

    pub(crate) fn record_cardinality(
        &mut self,
        id: &str,
        slot: usize,
        facts: ClassTypographyTerminalFacts,
    ) {
        let Some(edge) = self.edges.get_mut(id) else {
            self.terminals_match = false;
            return;
        };
        let Some(checkpoint) = edge.cardinalities.get_mut(slot) else {
            self.terminals_match = false;
            return;
        };
        record_terminal_slot(Some(checkpoint), facts, &mut self.terminals_match);
    }

    pub(crate) fn record_diagram_title(&mut self, facts: ClassTypographyTerminalFacts) {
        record_terminal_slot(
            Some(&mut self.diagram_title),
            facts,
            &mut self.terminals_match,
        );
    }

    fn structurally_complete(&self) -> bool {
        self.proves_stylesheet_emission()
            && self.terminals_match
            && self
                .nodes
                .values()
                .all(ClassTypographyTerminalCheckpoint::proves)
            && self
                .namespaces
                .values()
                .all(ClassTypographyTerminalCheckpoint::proves)
            && self.edges.values().all(|edge| {
                edge.label.proves()
                    && edge
                        .cardinalities
                        .iter()
                        .all(ClassTypographyTerminalCheckpoint::proves)
            })
            && self.diagram_title.proves()
    }

    fn proves_stylesheet_emission(&self) -> bool {
        self.css_emission_unique
            && self.css_emission.as_ref().is_some_and(|emission| {
                emission.complete
                    && emission.font_family_css.as_ref() == self.expected_font_family_css.as_ref()
            })
    }

    fn has_applied_font_run(&self) -> bool {
        let facts = self.terminal_facts();
        facts.layout_inherited_runs != 0 || facts.writer_inherited_runs != 0
    }

    fn has_visible_font_run(&self) -> bool {
        self.terminal_facts().visible_runs != 0
    }

    fn has_unverified_font_run(&self) -> bool {
        self.terminal_facts().unverified_runs != 0
    }

    fn terminal_facts(&self) -> ClassTypographyTerminalFacts {
        let mut facts = ClassTypographyTerminalFacts::default();
        for terminal in self
            .nodes
            .values()
            .chain(self.namespaces.values())
            .filter_map(ClassTypographyTerminalCheckpoint::observed)
        {
            facts.merge(*terminal);
        }
        for edge in self.edges.values() {
            if let Some(label) = edge.label.observed {
                facts.merge(label);
            }
            for terminal in edge
                .cardinalities
                .iter()
                .filter_map(ClassTypographyTerminalCheckpoint::observed)
            {
                facts.merge(*terminal);
            }
        }
        if let Some(title) = self.diagram_title.observed {
            facts.merge(title);
        }
        facts
    }
}

impl ClassTypographyTerminalCheckpoint {
    const fn new(expected_visible: bool) -> Self {
        Self {
            expected_visible,
            observed: None,
        }
    }

    fn observed(&self) -> Option<&ClassTypographyTerminalFacts> {
        self.observed.as_ref()
    }

    fn proves(&self) -> bool {
        self.observed
            .is_some_and(|facts| (facts.visible_runs != 0) == self.expected_visible)
    }
}

fn record_terminal_slot(
    slot: Option<&mut ClassTypographyTerminalCheckpoint>,
    facts: ClassTypographyTerminalFacts,
    terminals_match: &mut bool,
) {
    let Some(slot) = slot else {
        *terminals_match = false;
        return;
    };
    if slot.observed.replace(facts).is_some() {
        *terminals_match = false;
    }
}

fn insert_terminal_expectation(
    terminals: &mut BTreeMap<Box<str>, ClassTypographyTerminalCheckpoint>,
    id: &str,
    expected_visible: bool,
    terminals_match: &mut bool,
) {
    if terminals
        .insert(
            id.into(),
            ClassTypographyTerminalCheckpoint::new(expected_visible),
        )
        .is_some()
    {
        *terminals_match = false;
    }
}

fn class_note_expected_visibility(
    text: &str,
    diagram_use_html_labels: bool,
    sanitize_config: Option<&merman_core::MermaidConfig>,
) -> bool {
    let source = text.trim();
    let decoded = crate::entities::decode_entities_minimal_cow(source);
    if !diagram_use_html_labels && !crate::math::contains_delimited_math(source) {
        return crate::class::class_svg_label_visible_style_facts(decoded.as_ref())
            .has_visible_runs();
    }
    let Some(config) = sanitize_config else {
        return !decoded.trim().is_empty();
    };
    let html = format!(
        "<p>{}</p>",
        crate::class::class_note_html_fragment(source, config)
    );
    crate::text::VisibleTextStyleFacts::from_xhtml_fragment(&html).has_visible_runs()
}

fn class_edge_label_expected_visibility(text: &str, edge_use_html_labels: bool) -> bool {
    let decoded = crate::entities::decode_entities_minimal_cow(text);
    let text = decoded.trim();
    if text.is_empty() {
        return false;
    }
    if edge_use_html_labels || crate::math::contains_delimited_math(text) {
        return crate::class::class_html_label_visible_style_facts(text).has_visible_runs();
    }
    crate::class::class_svg_label_visible_style_facts(text).has_visible_runs()
}

fn class_terminal_expected_visibility(text: &str) -> bool {
    !crate::entities::decode_entities_minimal_cow(text)
        .trim()
        .is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn empty_receipt() -> ClassTypographyThemeReceipt {
        ClassTypographyThemeReceipt {
            expected_font_family_css: "Inter,sans-serif".into(),
            css_emission: None,
            css_emission_unique: true,
            nodes: BTreeMap::new(),
            namespaces: BTreeMap::new(),
            edges: BTreeMap::new(),
            diagram_title: ClassTypographyTerminalCheckpoint::new(false),
            terminals_match: true,
        }
    }

    #[test]
    fn typed_receipt_requires_css_and_the_title_checkpoint_even_for_an_empty_diagram() {
        let mut receipt = empty_receipt();
        receipt.record_css_emission(ClassTypographyCssEmission::from_successful_writes(
            "Inter,sans-serif",
            "Inter,sans-serif",
            "Inter,sans-serif",
            "Inter,sans-serif",
        ));
        assert!(!receipt.structurally_complete());
        receipt.record_diagram_title(ClassTypographyTerminalFacts::default());
        assert!(receipt.structurally_complete());
        assert!(!receipt.has_applied_font_run());
    }

    #[test]
    fn duplicate_terminal_checkpoints_fail_structural_validation() {
        let mut receipt = empty_receipt();
        receipt.css_emission = Some(ClassTypographyCssEmission::from_successful_writes(
            "Inter,sans-serif",
            "Inter,sans-serif",
            "Inter,sans-serif",
            "Inter,sans-serif",
        ));
        receipt.diagram_title.observed = Some(ClassTypographyTerminalFacts::default());
        receipt
            .nodes
            .insert("A".into(), ClassTypographyTerminalCheckpoint::new(true));
        receipt.record_node("A", ClassTypographyTerminalFacts::new(1, 1, 1, 0));
        assert!(receipt.structurally_complete());
        receipt.record_node("A", ClassTypographyTerminalFacts::new(1, 1, 1, 0));
        assert!(!receipt.structurally_complete());
    }

    #[test]
    fn unverified_terminal_is_structurally_complete_but_cannot_prove_application() {
        let mut unverified = empty_receipt();
        unverified.css_emission = Some(ClassTypographyCssEmission::from_successful_writes(
            "Inter,sans-serif",
            "Inter,sans-serif",
            "Inter,sans-serif",
            "Inter,sans-serif",
        ));
        unverified.diagram_title.observed = Some(ClassTypographyTerminalFacts::default());
        unverified.nodes.insert(
            "A".into(),
            ClassTypographyTerminalCheckpoint {
                expected_visible: true,
                observed: Some(ClassTypographyTerminalFacts::new(1, 0, 0, 1)),
            },
        );
        assert!(unverified.structurally_complete());
        assert!(unverified.has_unverified_font_run());
    }

    #[test]
    fn visible_relation_terminal_cannot_be_sealed_as_empty() {
        let mut receipt = empty_receipt();
        receipt.css_emission = Some(ClassTypographyCssEmission::from_successful_writes(
            "Inter,sans-serif",
            "Inter,sans-serif",
            "Inter,sans-serif",
            "Inter,sans-serif",
        ));
        receipt.diagram_title.observed = Some(ClassTypographyTerminalFacts::default());
        receipt.edges.insert(
            "rel".into(),
            ClassTypographyEdgeCheckpoint {
                label: ClassTypographyTerminalCheckpoint::new(true),
                cardinalities: [ClassTypographyTerminalCheckpoint::new(false); 4],
            },
        );
        receipt.record_edge_label("rel", ClassTypographyTerminalFacts::default());
        for slot in 0..4 {
            receipt.record_cardinality("rel", slot, ClassTypographyTerminalFacts::default());
        }

        assert!(!receipt.structurally_complete());
    }

    #[test]
    fn inactive_plan_preserves_class_layout_and_stylesheet_font_precedence() {
        let config = merman_core::MermaidConfig::from_value(json!({
            "fontFamily": "Class Root, monospace",
            "themeVariables": {
                "fontFamily": "Class Theme, sans-serif"
            }
        }));

        let plan = ClassTypographyThemePlan::resolve(None, &config);

        assert_eq!(plan.layout_font_family_css(), "Class Root,monospace");
        assert_eq!(plan.stylesheet_font_family_css(), "Class Theme,sans-serif");
        assert!(!plan.inherited_font_stack.typed_font_stack_active());
    }

    #[test]
    fn node_style_facts_are_sealed_once_and_missing_ids_fail_closed() {
        let config = merman_core::MermaidConfig::default();
        let plan = ClassTypographyThemePlan::resolve(None, &config);

        assert!(matches!(
            plan.require_node_style_facts("A"),
            Err(crate::Error::InvalidModel { .. })
        ));

        let mut facts = BTreeMap::new();
        facts.insert("A".into(), ClassNodeLabelStyleFacts::default());
        assert!(plan.seal_node_style_facts(facts.clone()));
        assert_eq!(
            plan.require_node_style_facts("A").unwrap(),
            ClassNodeLabelStyleFacts::default()
        );
        assert!(matches!(
            plan.require_node_style_facts("B"),
            Err(crate::Error::InvalidModel { .. })
        ));
        assert!(!plan.seal_node_style_facts(facts));
    }

    #[test]
    fn invalid_embedded_markup_preserves_explicit_parent_ownership() {
        let invalid = crate::text::VisibleTextStyleFacts::from_xhtml_fragment("<span>");
        assert!(!invalid.parse_valid());

        let mut owned = ClassNodeLabelStyleFacts::default();
        owned.observe(&invalid, true, false, true, true, false);
        assert!(owned.source_owns_every_visible_run());
        assert!(!owned.color_ownership_is_unverified());
        assert_eq!(owned.unverified_font_run_count(), 0);

        let mut inherited = ClassNodeLabelStyleFacts::default();
        inherited.observe(&invalid, false, false, false, false, false);
        assert!(!inherited.source_owns_every_visible_run());
        assert!(inherited.color_ownership_is_unverified());
        assert_eq!(inherited.unverified_font_run_count(), 1);
    }
}
