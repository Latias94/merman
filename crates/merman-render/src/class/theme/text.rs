use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan, resolved_style_property_for_facet,
};
use crate::text::TextStyle;

const CARDINALITY_SLOT_COUNT: usize = 4;

/// Class keeps Mermaid's two configured font winners until a typed stack owns both layers.
///
/// Layout measurement reads root `fontFamily` first. The stylesheet reads
/// `themeVariables.fontFamily` first. A directly owned FontStack intentionally replaces both;
/// inactive, config-owned, and unsupported routes preserve the upstream split.
#[derive(Debug)]
pub(crate) struct ClassTextThemePlan {
    css_binding: super::ClassCssThemeBinding,
    common_css: crate::svg::PreparedCommonCss,
    inherited_font_stack: InheritedFontStackPlan,
    text_rules: BTreeMap<usize, Option<ThemeCapability>>,
    fully_shadowed_text_rules: BTreeSet<usize>,
    edge_paint: Option<ClassTextPaint>,
    note_paint: Option<ClassTextPaint>,
    title_paint: Option<ClassTextPaint>,
    layout_font_family_css: Box<str>,
    stylesheet_font_family_css: Box<str>,
    font_size_css: Box<str>,
    font_size_px: f64,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    node_style_facts: OnceLock<BTreeMap<Box<str>, ClassNodeLabelStyleFacts>>,
    layout_font_size_px: OnceLock<f64>,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<ClassTextThemeReceipt>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ClassNodeLabelStyleFacts {
    visible_runs: usize,
    inherited_color_runs: usize,
    color_ownership_unverified: bool,
    layout_inherited_font_runs: usize,
    writer_inherited_font_runs: usize,
    unverified_font_runs: usize,
    layout_inherited_font_size_runs: usize,
    writer_inherited_font_size_runs: usize,
    fully_inherited_font_size_runs: usize,
    unverified_font_size_runs: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ClassTextTerminalFacts {
    paint_observation: Option<(usize, bool)>,
    visible_runs: usize,
    inherited_color_runs: usize,
    color_ownership_unverified: bool,
    layout_inherited_runs: usize,
    writer_inherited_runs: usize,
    unverified_runs: usize,
    layout_inherited_size_runs: usize,
    writer_inherited_size_runs: usize,
    fully_inherited_size_runs: usize,
    unverified_size_runs: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ClassTypographyCssEmission {
    font_family_css: Box<str>,
    font_size_css: Box<str>,
    font_family_complete: bool,
    font_size_complete: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct ClassTextEdgeCheckpoint {
    label: ClassTextTerminalCheckpoint,
    cardinalities: [ClassTextTerminalCheckpoint; CARDINALITY_SLOT_COUNT],
}

#[derive(Debug, Clone, Copy, Default)]
struct ClassTextTerminalCheckpoint {
    expected_paint: Option<usize>,
    expected_visible: bool,
    observed: Option<ClassTextTerminalFacts>,
}

/// Renderer-owned text receipt for the Class stylesheet and every text-bearing object.
///
/// The receipt records semantic writer checkpoints. It does not parse the finalized stylesheet or
/// SVG, and it does not reconstruct browser cascade semantics.
#[derive(Debug, Clone)]
pub(crate) struct ClassTextThemeReceipt {
    expected_font_family_css: Box<str>,
    expected_font_size_css: Box<str>,
    expected_font_size_px: f64,
    layout_font_size_px: Option<f64>,
    css_emission: Option<ClassTypographyCssEmission>,
    css_emission_unique: bool,
    nodes: BTreeMap<Box<str>, ClassTextTerminalCheckpoint>,
    namespaces: BTreeMap<Box<str>, ClassTextTerminalCheckpoint>,
    edges: BTreeMap<Box<str>, ClassTextEdgeCheckpoint>,
    diagram_title: ClassTextTerminalCheckpoint,
    terminals_match: bool,
}

/// A selected generic Text fill shared by a writer and its existing text checkpoint.
#[derive(Debug, Clone)]
pub(crate) struct ClassTextPaint {
    rule_index: usize,
    css: String,
    style: String,
}

impl ClassTextPaint {
    pub(crate) fn style(&self) -> &str {
        &self.style
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "The family terminal receipt reconciles emitted geometry, paint, and source ownership."
    )]
    pub(crate) fn observe(
        &self,
        facts: ClassTextTerminalFacts,
        style: Option<&str>,
    ) -> ClassTextTerminalFacts {
        facts.with_paint(
            Some((self.rule_index, self.css.as_str())),
            style.unwrap_or(""),
        )
    }
}

impl ClassTextThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
    ) -> Self {
        let mut text_rules = BTreeMap::new();
        let mut shadowed = BTreeMap::new();
        let static_style =
            theme.map(|theme| theme.style(ThemeTarget::Text, ThemeVariant::Default, None));
        if let Some(theme) = theme {
            let winners = static_style
                .as_ref()
                .expect("theme has a static style")
                .winner_rule_properties()
                .collect::<BTreeMap<_, _>>();
            for route in theme.family_mechanism_routes() {
                if let FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Text,
                    facet,
                    selector,
                } = route.mechanism()
                {
                    // The later rule must cover the same selector and every property of this rule.
                    let facet_shadowed = match selector {
                        FamilyThemeSelectorShape::Static {
                            variant: variant @ (None | Some(ThemeVariant::Default)),
                        } => winners
                            .get(&resolved_style_property_for_facet(facet))
                            .is_some_and(|winner| {
                                winner.rule_index() > rule_index
                                    && winner.target() == ThemeTarget::Text
                                    && winner.ordinal().is_none()
                                    && winner.variant() == variant
                            }),
                        _ => false,
                    };
                    *shadowed.entry(rule_index).or_insert(true) &= facet_shadowed;
                    if facet_shadowed {
                        continue;
                    }
                    let capability = if route.disposition() == FamilyThemeDisposition::TypedAdapter
                    {
                        match facet {
                            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Solid) => {
                                Some(ThemeCapability::SolidPaint)
                            }
                            FamilyThemeRuleFacet::Fill(FamilyThemePaintKind::Transparent) => {
                                Some(ThemeCapability::TransparentPaint)
                            }
                            _ => None,
                        }
                    } else {
                        None
                    };
                    let entry = text_rules.entry(rule_index).or_insert(capability);
                    if capability.is_none() {
                        *entry = None;
                    }
                }
            }
        }
        // Fully shadowed rules still need their NotApplicable evidence key.
        for &rule_index in shadowed.keys() {
            text_rules.entry(rule_index).or_insert(None);
        }
        let selected = theme.and_then(|theme| {
            crate::family::resolve_direct_static_fill(
                theme,
                static_style.as_ref().expect("theme has a static style"),
                &[ThemeTarget::Text],
                crate::family::DirectStaticSelectorDomain::Default,
            )
            .map(|paint| {
                let (css, rule_index, _) = paint.into_parts();
                ClassTextPaint {
                    rule_index,
                    style: format!("color:{css} !important;fill:{css} !important;"),
                    css: css.into_string(),
                }
            })
        });
        let owned = |paths: &[&str]| {
            paths.iter().any(|path| {
                merman_core::__private::config_path_overrides_typed_default(effective_config, path)
            })
        };
        let edge_paint = (!owned(&[
            "themeVariables.classText",
            "themeVariables.primaryTextColor",
            "themeVariables.textColor",
        ]))
        .then(|| selected.clone())
        .flatten();
        let note_paint = (!owned(&["themeVariables.noteTextColor"]))
            .then(|| selected.clone())
            .flatten();
        let title_paint = (!owned(&["themeVariables.textColor"]))
            .then(|| selected.clone())
            .flatten();
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let configured_font_size_css = crate::config::config_css_number_or_string(
            effective_config.as_value(),
            &["themeVariables", "fontSize"],
        )
        .unwrap_or_else(|| "16px".to_string());
        let configured_font_size_px = crate::config::config_f64_explicit_css_px(
            effective_config.as_value(),
            &["themeVariables", "fontSize"],
        )
        .unwrap_or(16.0)
        .max(1.0);
        let typed_font_size_requested = theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                route.mechanism()
                    == crate::diagram_theme::FamilyThemeMechanism::BaseTypography(
                        ThemeTypographyProperty::FontSize,
                    )
                    && route.disposition()
                        == crate::diagram_theme::FamilyThemeDisposition::TypedAdapter
            })
        });
        let config_owns_font_size = typed_font_size_requested
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.fontSize",
            );
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let (font_size_px, font_size_css) = match (theme, typed_font_size_active) {
            (Some(theme), true) => {
                let font_size_px = crate::number_format::canonicalize_number(f64::from(
                    theme.typography().font_size_px(),
                ))
                .max(1.0);
                (
                    font_size_px,
                    format!("{}px", crate::number_format::canonical_number(font_size_px))
                        .into_boxed_str(),
                )
            }
            _ => (
                configured_font_size_px,
                configured_font_size_css.into_boxed_str(),
            ),
        };
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
            css_binding: super::ClassCssThemeBinding::resolve(effective_config.as_value()),
            common_css: crate::svg::PreparedCommonCss::bind(
                effective_config.as_value(),
                &stylesheet_font_family_css,
                &font_size_css,
                inherited_font_stack.typed_font_stack_active(),
            ),
            text_rules,
            fully_shadowed_text_rules: shadowed
                .into_iter()
                .filter_map(|(index, shadowed)| shadowed.then_some(index))
                .collect(),
            edge_paint,
            note_paint,
            title_paint,
            inherited_font_stack,
            layout_font_family_css: layout_font_family_css.into_boxed_str(),
            stylesheet_font_family_css: stylesheet_font_family_css.into_boxed_str(),
            font_size_css,
            font_size_px,
            typed_font_size_requested,
            typed_font_size_active,
            node_style_facts: OnceLock::new(),
            layout_font_size_px: OnceLock::new(),
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn edge_paint(&self) -> Option<&ClassTextPaint> {
        self.edge_paint.as_ref()
    }
    pub(crate) fn css_binding(&self) -> &super::ClassCssThemeBinding {
        &self.css_binding
    }
    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }
    pub(crate) fn note_paint(&self) -> Option<&ClassTextPaint> {
        self.note_paint.as_ref()
    }
    pub(crate) fn title_paint(&self) -> Option<&ClassTextPaint> {
        self.title_paint.as_ref()
    }

    pub(crate) fn bind_paint_expectations(
        &self,
        receipt: &mut ClassTextThemeReceipt,
        nodes: &[super::ClassNodeTerminalExpectation],
        namespace: Option<(usize, &str)>,
        note_ids: impl IntoIterator<Item = impl AsRef<str>>,
    ) {
        let generic = |rule| self.text_rules.contains_key(&rule).then_some(rule);
        for node in nodes {
            if let Some(slot) = receipt.nodes.get_mut(node.id()) {
                slot.expected_paint = (node.label_fill_target() == Some(ThemeTarget::Text))
                    .then(|| node.typed_label_fill(false))
                    .flatten()
                    .and_then(|(rule, _)| generic(rule));
            }
        }
        for id in note_ids {
            if let Some(slot) = receipt.nodes.get_mut(id.as_ref()) {
                slot.expected_paint = self.note_paint.as_ref().map(|paint| paint.rule_index);
            }
        }
        for slot in receipt.namespaces.values_mut() {
            slot.expected_paint = namespace.and_then(|(rule, _)| generic(rule));
        }
        for edge in receipt.edges.values_mut() {
            edge.label.expected_paint = self.edge_paint.as_ref().map(|paint| paint.rule_index);
            for slot in &mut edge.cardinalities {
                slot.expected_paint = edge.label.expected_paint;
            }
        }
        receipt.diagram_title.expected_paint =
            self.title_paint.as_ref().map(|paint| paint.rule_index);
    }

    pub(crate) fn layout_font_family_css(&self) -> &str {
        &self.layout_font_family_css
    }

    pub(crate) fn stylesheet_font_family_css(&self) -> &str {
        &self.stylesheet_font_family_css
    }

    pub(crate) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    pub(crate) const fn typed_font_size_active(&self) -> bool {
        self.typed_font_size_active
    }

    fn requires_terminal_receipt(&self) -> bool {
        !self.text_rules.is_empty()
            || self.inherited_font_stack.typed_font_stack_active()
            || self.typed_font_size_active
            || self
                .inherited_font_stack
                .has_unsupported_typography_properties()
    }

    pub(crate) fn apply_layout_text_styles(
        &self,
        text_style: &mut TextStyle,
        html_calc_text_style: &mut TextStyle,
    ) {
        let font_family = Some(self.layout_font_family_css().to_string());
        text_style.font_family = font_family.clone();
        html_calc_text_style.font_family = font_family;
        if self.typed_font_size_active() {
            text_style.font_size = self.font_size_px();
        }
    }

    pub(crate) fn seal_layout_font_size(&self, font_size_px: f64) -> bool {
        !self.typed_font_size_active || self.layout_font_size_px.set(font_size_px).is_ok()
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
    ) -> Option<ClassTextThemeReceipt> {
        self.requires_terminal_receipt().then(|| {
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
                        ClassTextEdgeCheckpoint {
                            label: ClassTextTerminalCheckpoint::new(label),
                            cardinalities: [
                                ClassTextTerminalCheckpoint::new(false),
                                ClassTextTerminalCheckpoint::new(start),
                                ClassTextTerminalCheckpoint::new(end),
                                ClassTextTerminalCheckpoint::new(false),
                            ],
                        },
                    )
                    .is_some()
                {
                    terminals_match = false;
                }
            }

            ClassTextThemeReceipt {
                expected_font_family_css: self.stylesheet_font_family_css().into(),
                expected_font_size_css: self.font_size_css().into(),
                expected_font_size_px: self.font_size_px(),
                layout_font_size_px: self.layout_font_size_px.get().copied(),
                css_emission: None,
                css_emission_unique: true,
                nodes,
                namespaces,
                edges,
                diagram_title: ClassTextTerminalCheckpoint::new(
                    diagram_title.is_some_and(|title| !title.trim().is_empty()),
                ),
                terminals_match,
            }
        })
    }

    pub(crate) fn record_terminal(&self, receipt: Option<ClassTextThemeReceipt>) -> bool {
        if self.requires_terminal_receipt() {
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
                    .is_none_or(ClassTextThemeReceipt::has_visible_font_run),
            );
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        match self.inherited_font_stack.outcome() {
            InheritedFontStackOutcome::Typed => {
                match self.terminal_receipt.get() {
                    Some(receipt) if !receipt.structurally_complete() => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                    Some(receipt) if receipt.has_unverified_font_run() => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                    Some(receipt) if receipt.proves_font_family() => {
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
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontSize);
        if self.typed_font_size_requested {
            if !self.typed_font_size_active {
                evidence.mark_not_applicable(key);
            } else {
                match self.terminal_receipt.get() {
                    Some(receipt) if !receipt.structurally_complete() => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                    Some(receipt) if receipt.has_unverified_font_size_run() => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                    Some(receipt) if receipt.proves_font_size() => {
                        evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                    }
                    Some(receipt) if !receipt.has_applicable_font_size_run() => {
                        evidence.mark_not_applicable(key)
                    }
                    Some(_) | None => evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography),
                }
            }
        }
        if let Some(receipt) = self.terminal_receipt.get() {
            let mut outcomes = BTreeMap::<usize, (bool, bool)>::new();
            receipt.for_each_checkpoint(|slot| {
                let Some(index) = slot.expected_paint else {
                    return;
                };
                let (applied, incomplete) = outcomes.entry(index).or_default();
                let Some(facts) = slot.observed else {
                    *incomplete = true;
                    return;
                };
                if facts.visible_runs == 0 || facts.source_owns_every_visible_run() {
                    return;
                }
                if facts.has_verified_inherited_paint()
                    && facts.paint_observation == Some((index, true))
                {
                    *applied = true;
                } else {
                    *incomplete = true;
                }
            });
            for (&index, &capability) in &self.text_rules {
                if self.fully_shadowed_text_rules.contains(&index) {
                    evidence.mark_not_applicable(FamilyThemeMechanismKey::Rule {
                        index,
                        target: ThemeTarget::Text,
                    });
                    continue;
                }
                let Some(capability) = capability else {
                    continue;
                };
                let (applied, incomplete) = outcomes.get(&index).copied().unwrap_or_default();
                if incomplete {
                    continue;
                }
                let key = FamilyThemeMechanismKey::Rule {
                    index,
                    target: ThemeTarget::Text,
                };
                if applied {
                    evidence.mark_applied_with_capabilities(key, [capability]);
                } else {
                    evidence.mark_not_applicable(key);
                }
            }
        }
        evidence
    }
}

impl ClassNodeLabelStyleFacts {
    #[allow(
        clippy::too_many_arguments,
        reason = "Records independent color, font, and font-size ownership from layout and the writer."
    )]
    pub(crate) fn observe(
        &mut self,
        facts: &crate::text::VisibleTextStyleFacts,
        parent_owns_color: bool,
        color_ownership_unverified: bool,
        layout_parent_owns_font: bool,
        writer_parent_owns_font: bool,
        font_ownership_unverified: bool,
        layout_parent_owns_font_size: bool,
        writer_parent_owns_font_size: bool,
        font_size_ownership_unverified: bool,
    ) {
        if !facts.parse_valid() {
            self.visible_runs = self.visible_runs.saturating_add(1);
            self.color_ownership_unverified |= !parent_owns_color;
            if font_ownership_unverified || !layout_parent_owns_font || !writer_parent_owns_font {
                self.unverified_font_runs = self.unverified_font_runs.saturating_add(1);
            }
            if font_size_ownership_unverified
                || !layout_parent_owns_font_size
                || !writer_parent_owns_font_size
            {
                self.unverified_font_size_runs = self.unverified_font_size_runs.saturating_add(1);
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
        } else {
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
        if font_size_ownership_unverified
            || layout_parent_owns_font_size != writer_parent_owns_font_size
        {
            self.unverified_font_size_runs =
                self.unverified_font_size_runs.saturating_add(visible_runs);
        } else {
            self.unverified_font_size_runs = self
                .unverified_font_size_runs
                .saturating_add(facts.unverified_font_size_run_count());
            if !layout_parent_owns_font_size {
                self.layout_inherited_font_size_runs = self
                    .layout_inherited_font_size_runs
                    .saturating_add(facts.inherited_font_size_run_count());
            }
            if !writer_parent_owns_font_size {
                self.writer_inherited_font_size_runs = self
                    .writer_inherited_font_size_runs
                    .saturating_add(facts.inherited_font_size_run_count());
            }
            if !layout_parent_owns_font_size && !writer_parent_owns_font_size {
                self.fully_inherited_font_size_runs = self
                    .fully_inherited_font_size_runs
                    .saturating_add(facts.inherited_font_size_run_count());
            }
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
        self.layout_inherited_font_size_runs = self
            .layout_inherited_font_size_runs
            .saturating_add(other.layout_inherited_font_size_runs);
        self.writer_inherited_font_size_runs = self
            .writer_inherited_font_size_runs
            .saturating_add(other.writer_inherited_font_size_runs);
        self.fully_inherited_font_size_runs = self
            .fully_inherited_font_size_runs
            .saturating_add(other.fully_inherited_font_size_runs);
        self.unverified_font_size_runs = self
            .unverified_font_size_runs
            .saturating_add(other.unverified_font_size_runs);
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

    pub(crate) const fn layout_inherited_font_size_run_count(self) -> usize {
        self.layout_inherited_font_size_runs
    }

    pub(crate) const fn writer_inherited_font_size_run_count(self) -> usize {
        self.writer_inherited_font_size_runs
    }

    pub(crate) const fn fully_inherited_font_size_run_count(self) -> usize {
        self.fully_inherited_font_size_runs
    }

    pub(crate) const fn unverified_font_size_run_count(self) -> usize {
        self.unverified_font_size_runs
    }
}

impl ClassTextTerminalFacts {
    pub(crate) fn with_paint(mut self, emitted: Option<(usize, &str)>, style: &str) -> Self {
        self.paint_observation = emitted.map(|(rule, css)| {
            (
                rule,
                super::terminal::terminal_paint(style, "color") == Some(css)
                    && super::terminal::terminal_paint(style, "fill") == Some(css),
            )
        });
        self
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

    pub(crate) const fn paint_ownership_is_unambiguous(self) -> bool {
        !self.has_mixed_color_ownership() && !self.color_ownership_is_unverified()
    }

    pub(crate) const fn has_verified_inherited_paint(self) -> bool {
        self.visible_runs != 0
            && self.inherited_color_runs == self.visible_runs
            && !self.color_ownership_unverified
    }

    #[cfg(test)]
    pub(crate) const fn new(
        visible_runs: usize,
        layout_inherited_runs: usize,
        writer_inherited_runs: usize,
        unverified_runs: usize,
    ) -> Self {
        Self {
            paint_observation: None,
            visible_runs,
            inherited_color_runs: visible_runs,
            color_ownership_unverified: false,
            layout_inherited_runs,
            writer_inherited_runs,
            unverified_runs,
            layout_inherited_size_runs: layout_inherited_runs,
            writer_inherited_size_runs: writer_inherited_runs,
            fully_inherited_size_runs: if layout_inherited_runs < writer_inherited_runs {
                layout_inherited_runs
            } else {
                writer_inherited_runs
            },
            unverified_size_runs: unverified_runs,
        }
    }

    pub(crate) fn from_node_style_facts(facts: ClassNodeLabelStyleFacts) -> Self {
        Self {
            paint_observation: None,
            visible_runs: facts.visible_run_count(),
            inherited_color_runs: facts.inherited_color_runs,
            color_ownership_unverified: facts.color_ownership_unverified,
            layout_inherited_runs: facts.layout_inherited_font_run_count(),
            writer_inherited_runs: facts.writer_inherited_font_run_count(),
            unverified_runs: facts.unverified_font_run_count(),
            layout_inherited_size_runs: facts.layout_inherited_font_size_run_count(),
            writer_inherited_size_runs: facts.writer_inherited_font_size_run_count(),
            fully_inherited_size_runs: facts.fully_inherited_font_size_run_count(),
            unverified_size_runs: facts.unverified_font_size_run_count(),
        }
    }

    pub(crate) fn inherited_text(text: &str) -> Self {
        let visible_runs = crate::text::VisibleTextStyleFacts::plain_text(text).visible_run_count();
        Self {
            paint_observation: None,
            visible_runs,
            inherited_color_runs: visible_runs,
            color_ownership_unverified: false,
            layout_inherited_runs: visible_runs,
            writer_inherited_runs: visible_runs,
            unverified_runs: 0,
            layout_inherited_size_runs: visible_runs,
            writer_inherited_size_runs: visible_runs,
            fully_inherited_size_runs: visible_runs,
            unverified_size_runs: 0,
        }
    }

    pub(crate) fn unverified_text(text: &str) -> Self {
        let visible_runs = crate::text::VisibleTextStyleFacts::plain_text(text).visible_run_count();
        Self {
            paint_observation: None,
            visible_runs,
            inherited_color_runs: 0,
            color_ownership_unverified: visible_runs != 0,
            layout_inherited_runs: 0,
            writer_inherited_runs: 0,
            unverified_runs: visible_runs,
            layout_inherited_size_runs: 0,
            writer_inherited_size_runs: 0,
            fully_inherited_size_runs: 0,
            unverified_size_runs: visible_runs,
        }
    }

    pub(crate) fn fixed_font_size_text(text: &str) -> Self {
        let visible_runs = crate::text::VisibleTextStyleFacts::plain_text(text).visible_run_count();
        Self {
            paint_observation: None,
            visible_runs,
            inherited_color_runs: visible_runs,
            color_ownership_unverified: false,
            layout_inherited_runs: visible_runs,
            writer_inherited_runs: visible_runs,
            unverified_runs: 0,
            layout_inherited_size_runs: 0,
            writer_inherited_size_runs: 0,
            fully_inherited_size_runs: 0,
            unverified_size_runs: 0,
        }
    }

    pub(crate) fn from_visible_style_facts(facts: &crate::text::VisibleTextStyleFacts) -> Self {
        if !facts.parse_valid() {
            return Self {
                paint_observation: None,
                visible_runs: 1,
                inherited_color_runs: 0,
                color_ownership_unverified: true,
                layout_inherited_runs: 0,
                writer_inherited_runs: 0,
                unverified_runs: 1,
                layout_inherited_size_runs: 0,
                writer_inherited_size_runs: 0,
                fully_inherited_size_runs: 0,
                unverified_size_runs: 1,
            };
        }
        let visible_runs = facts.visible_run_count();
        let inherited_runs = facts.inherited_font_family_run_count();
        Self {
            paint_observation: None,
            visible_runs,
            inherited_color_runs: facts.inherited_color_run_count(),
            color_ownership_unverified: facts.unverified_portable_color_run_count() != 0,
            layout_inherited_runs: inherited_runs,
            writer_inherited_runs: inherited_runs,
            unverified_runs: facts.unverified_font_family_run_count(),
            layout_inherited_size_runs: facts.inherited_font_size_run_count(),
            writer_inherited_size_runs: facts.inherited_font_size_run_count(),
            fully_inherited_size_runs: facts.inherited_font_size_run_count(),
            unverified_size_runs: facts.unverified_font_size_run_count(),
        }
    }

    fn merge(&mut self, other: Self) {
        self.visible_runs = self.visible_runs.saturating_add(other.visible_runs);
        self.inherited_color_runs = self
            .inherited_color_runs
            .saturating_add(other.inherited_color_runs);
        self.color_ownership_unverified |= other.color_ownership_unverified;
        self.layout_inherited_runs = self
            .layout_inherited_runs
            .saturating_add(other.layout_inherited_runs);
        self.writer_inherited_runs = self
            .writer_inherited_runs
            .saturating_add(other.writer_inherited_runs);
        self.unverified_runs = self.unverified_runs.saturating_add(other.unverified_runs);
        self.layout_inherited_size_runs = self
            .layout_inherited_size_runs
            .saturating_add(other.layout_inherited_size_runs);
        self.writer_inherited_size_runs = self
            .writer_inherited_size_runs
            .saturating_add(other.writer_inherited_size_runs);
        self.fully_inherited_size_runs = self
            .fully_inherited_size_runs
            .saturating_add(other.fully_inherited_size_runs);
        self.unverified_size_runs = self
            .unverified_size_runs
            .saturating_add(other.unverified_size_runs);
    }
}

impl ClassTypographyCssEmission {
    pub(crate) fn from_successful_writes(
        diagram_root_font_family_css: &str,
        nested_svg_font_family_css: &str,
        diagram_root_font_size_css: &str,
        nested_svg_font_size_css: &str,
        class_group_font_family_css: &str,
        root_variable_font_family_css: &str,
    ) -> Self {
        Self {
            font_family_css: diagram_root_font_family_css.into(),
            font_size_css: diagram_root_font_size_css.into(),
            font_family_complete: [
                nested_svg_font_family_css,
                class_group_font_family_css,
                root_variable_font_family_css,
            ]
            .into_iter()
            .all(|font_family| font_family == diagram_root_font_family_css),
            font_size_complete: nested_svg_font_size_css == diagram_root_font_size_css,
        }
    }
}

impl ClassTextThemeReceipt {
    fn for_each_checkpoint(&self, mut visit: impl FnMut(&ClassTextTerminalCheckpoint)) {
        for slot in self.nodes.values().chain(self.namespaces.values()) {
            visit(slot);
        }
        for edge in self.edges.values() {
            visit(&edge.label);
            for slot in &edge.cardinalities {
                visit(slot);
            }
        }
        visit(&self.diagram_title);
    }

    pub(crate) fn record_css_emission(&mut self, emission: ClassTypographyCssEmission) {
        if self.css_emission.is_some() {
            self.css_emission_unique = false;
            return;
        }
        self.css_emission = Some(emission);
    }

    pub(crate) fn record_node(&mut self, id: &str, facts: ClassTextTerminalFacts) {
        record_terminal_slot(self.nodes.get_mut(id), facts, &mut self.terminals_match);
    }

    pub(crate) fn record_namespace(&mut self, id: &str, facts: ClassTextTerminalFacts) {
        record_terminal_slot(
            self.namespaces.get_mut(id),
            facts,
            &mut self.terminals_match,
        );
    }

    pub(crate) fn record_edge_label(&mut self, id: &str, facts: ClassTextTerminalFacts) {
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
        facts: ClassTextTerminalFacts,
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

    pub(crate) fn record_diagram_title(&mut self, facts: ClassTextTerminalFacts) {
        record_terminal_slot(
            Some(&mut self.diagram_title),
            facts,
            &mut self.terminals_match,
        );
    }

    fn structurally_complete(&self) -> bool {
        self.css_emission_unique
            && self.css_emission.is_some()
            && self.terminals_match
            && self.nodes.values().all(ClassTextTerminalCheckpoint::proves)
            && self
                .namespaces
                .values()
                .all(ClassTextTerminalCheckpoint::proves)
            && self.edges.values().all(|edge| {
                edge.label.proves()
                    && edge
                        .cardinalities
                        .iter()
                        .all(ClassTextTerminalCheckpoint::proves)
            })
            && self.diagram_title.proves()
    }

    fn proves_font_family(&self) -> bool {
        self.css_emission_unique
            && self.css_emission.as_ref().is_some_and(|emission| {
                emission.font_family_complete
                    && emission.font_family_css.as_ref() == self.expected_font_family_css.as_ref()
            })
            && self.has_applied_font_run()
            && !self.has_unverified_font_run()
    }

    fn proves_font_size(&self) -> bool {
        self.css_emission_unique
            && self.css_emission.as_ref().is_some_and(|emission| {
                emission.font_size_complete
                    && emission.font_size_css.as_ref() == self.expected_font_size_css.as_ref()
            })
            && self
                .layout_font_size_px
                .is_some_and(|font_size_px| font_size_px == self.expected_font_size_px)
            && self.has_applied_font_size_run()
            && !self.has_unverified_font_size_run()
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

    fn has_applied_font_size_run(&self) -> bool {
        self.terminal_facts().fully_inherited_size_runs != 0
    }

    fn has_applicable_font_size_run(&self) -> bool {
        self.has_applied_font_size_run()
    }

    fn has_unverified_font_size_run(&self) -> bool {
        self.terminal_facts().unverified_size_runs != 0
    }

    fn terminal_facts(&self) -> ClassTextTerminalFacts {
        let mut facts = ClassTextTerminalFacts::default();
        for terminal in self
            .nodes
            .values()
            .chain(self.namespaces.values())
            .filter_map(ClassTextTerminalCheckpoint::observed)
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
                .filter_map(ClassTextTerminalCheckpoint::observed)
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

impl ClassTextTerminalCheckpoint {
    const fn new(expected_visible: bool) -> Self {
        Self {
            expected_paint: None,
            expected_visible,
            observed: None,
        }
    }

    fn observed(&self) -> Option<&ClassTextTerminalFacts> {
        self.observed.as_ref()
    }

    fn proves(&self) -> bool {
        self.observed
            .is_some_and(|facts| (facts.visible_runs != 0) == self.expected_visible)
    }
}

fn record_terminal_slot(
    slot: Option<&mut ClassTextTerminalCheckpoint>,
    facts: ClassTextTerminalFacts,
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
    terminals: &mut BTreeMap<Box<str>, ClassTextTerminalCheckpoint>,
    id: &str,
    expected_visible: bool,
    terminals_match: &mut bool,
) {
    if terminals
        .insert(
            id.into(),
            ClassTextTerminalCheckpoint::new(expected_visible),
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

    #[test]
    fn node_style_fact_projection_preserves_independent_color_and_typography_counts() {
        let source = ClassNodeLabelStyleFacts {
            visible_runs: 11,
            inherited_color_runs: 9,
            color_ownership_unverified: true,
            layout_inherited_font_runs: 8,
            writer_inherited_font_runs: 7,
            unverified_font_runs: 2,
            layout_inherited_font_size_runs: 6,
            writer_inherited_font_size_runs: 5,
            fully_inherited_font_size_runs: 4,
            unverified_font_size_runs: 3,
        };
        assert_eq!(
            ClassTextTerminalFacts::from_node_style_facts(source),
            ClassTextTerminalFacts {
                paint_observation: None,
                visible_runs: 11,
                inherited_color_runs: 9,
                color_ownership_unverified: true,
                layout_inherited_runs: 8,
                writer_inherited_runs: 7,
                unverified_runs: 2,
                layout_inherited_size_runs: 6,
                writer_inherited_size_runs: 5,
                fully_inherited_size_runs: 4,
                unverified_size_runs: 3,
            }
        );
        assert_eq!(
            ClassTextTerminalFacts::fixed_font_size_text("cardinality"),
            ClassTextTerminalFacts {
                paint_observation: None,
                visible_runs: 1,
                inherited_color_runs: 1,
                color_ownership_unverified: false,
                layout_inherited_runs: 1,
                writer_inherited_runs: 1,
                unverified_runs: 0,
                layout_inherited_size_runs: 0,
                writer_inherited_size_runs: 0,
                fully_inherited_size_runs: 0,
                unverified_size_runs: 0,
            }
        );
    }

    #[test]
    fn text_terminals_preserve_color_ownership_independently_of_fonts() {
        for (markup, inherited, source_owned, verified) in [
            ("<span>Label</span>", 1, false, true),
            (
                r##"<span style="color:#123456">Label</span>"##,
                0,
                true,
                true,
            ),
            (
                r#"<span style="font-family:serif">Label</span>"#,
                1,
                false,
                true,
            ),
            (
                r#"<span style="color:var(--ink)">Label</span>"#,
                0,
                false,
                false,
            ),
            (r#"<span class="custom">Label</span>"#, 0, false, false),
            (
                r##"<span>Before <b style="color:#123456">After</b></span>"##,
                1,
                false,
                false,
            ),
            ("<span>", 0, false, false),
        ] {
            let source = crate::text::VisibleTextStyleFacts::from_xhtml_fragment(markup);
            let terminal = ClassTextTerminalFacts::from_visible_style_facts(&source);
            assert_eq!(terminal.inherited_color_runs, inherited, "{markup}");
            assert_eq!(
                terminal.source_owns_every_visible_run(),
                source_owned,
                "{markup}"
            );
            assert_eq!(
                terminal.paint_ownership_is_unambiguous(),
                verified,
                "{markup}"
            );
        }
    }

    #[test]
    fn inherited_paint_requires_visible_unowned_text() {
        for empty in [
            ClassTextTerminalFacts::default(),
            ClassTextTerminalFacts::inherited_text("  "),
            ClassTextTerminalFacts::unverified_text(""),
        ] {
            assert_eq!(empty.visible_runs, 0);
            assert!(empty.paint_ownership_is_unambiguous());
            assert!(!empty.source_owns_every_visible_run());
            assert!(!empty.has_verified_inherited_paint());
        }
        let inherited = ClassTextTerminalFacts::inherited_text("Label");
        assert!(inherited.has_verified_inherited_paint());
        let math = ClassTextTerminalFacts::unverified_text("Math");
        assert!(!math.paint_ownership_is_unambiguous());
        assert!(!math.has_verified_inherited_paint());
        let owned = ClassTextTerminalFacts::from_visible_style_facts(
            &crate::text::VisibleTextStyleFacts::from_xhtml_fragment(
                r##"<span style="color:#123456">Owned</span>"##,
            ),
        );
        assert!(owned.paint_ownership_is_unambiguous());
        assert!(owned.source_owns_every_visible_run());
        assert!(!owned.has_verified_inherited_paint());
    }

    #[test]
    fn text_receipt_merges_every_terminal_color_domain() {
        let mut receipt = empty_receipt();
        let inherited = ClassTextTerminalFacts::inherited_text("Label");
        let owned = ClassTextTerminalFacts::from_visible_style_facts(
            &crate::text::VisibleTextStyleFacts::from_xhtml_fragment(
                r##"<span style="color:#123456">Owned</span>"##,
            ),
        );
        let unknown = ClassTextTerminalFacts::unverified_text("Math");
        let checkpoint = |facts| ClassTextTerminalCheckpoint {
            expected_paint: None,
            expected_visible: true,
            observed: Some(facts),
        };
        receipt.nodes.insert("node".into(), checkpoint(owned));
        receipt.nodes.insert("note".into(), checkpoint(inherited));
        receipt
            .namespaces
            .insert("namespace".into(), checkpoint(inherited));
        receipt.edges.insert(
            "edge".into(),
            ClassTextEdgeCheckpoint {
                label: checkpoint(unknown),
                cardinalities: [checkpoint(inherited); CARDINALITY_SLOT_COUNT],
            },
        );
        receipt.diagram_title = checkpoint(inherited);
        let facts = receipt.terminal_facts();
        assert_eq!(facts.visible_runs, 9);
        assert_eq!(facts.inherited_color_runs, 7);
        assert!(facts.color_ownership_is_unverified());
        assert!(!facts.source_owns_every_visible_run());
        assert!(!facts.paint_ownership_is_unambiguous());
        assert_eq!(facts.layout_inherited_runs, 8);
        assert_eq!(facts.unverified_runs, 1);
    }

    fn empty_receipt() -> ClassTextThemeReceipt {
        ClassTextThemeReceipt {
            expected_font_family_css: "Inter,sans-serif".into(),
            expected_font_size_css: "16px".into(),
            expected_font_size_px: 16.0,
            layout_font_size_px: Some(16.0),
            css_emission: None,
            css_emission_unique: true,
            nodes: BTreeMap::new(),
            namespaces: BTreeMap::new(),
            edges: BTreeMap::new(),
            diagram_title: ClassTextTerminalCheckpoint::new(false),
            terminals_match: true,
        }
    }

    #[test]
    fn typed_receipt_requires_css_and_the_title_checkpoint_even_for_an_empty_diagram() {
        let mut receipt = empty_receipt();
        receipt.record_css_emission(ClassTypographyCssEmission::from_successful_writes(
            "Inter,sans-serif",
            "Inter,sans-serif",
            "16px",
            "16px",
            "Inter,sans-serif",
            "Inter,sans-serif",
        ));
        assert!(!receipt.structurally_complete());
        receipt.record_diagram_title(ClassTextTerminalFacts::default());
        assert!(receipt.structurally_complete());
        assert!(!receipt.has_applied_font_run());
    }

    #[test]
    fn duplicate_terminal_checkpoints_fail_structural_validation() {
        let mut receipt = empty_receipt();
        receipt.css_emission = Some(ClassTypographyCssEmission::from_successful_writes(
            "Inter,sans-serif",
            "Inter,sans-serif",
            "16px",
            "16px",
            "Inter,sans-serif",
            "Inter,sans-serif",
        ));
        receipt.diagram_title.observed = Some(ClassTextTerminalFacts::default());
        receipt
            .nodes
            .insert("A".into(), ClassTextTerminalCheckpoint::new(true));
        receipt.record_node("A", ClassTextTerminalFacts::new(1, 1, 1, 0));
        assert!(receipt.structurally_complete());
        receipt.record_node("A", ClassTextTerminalFacts::new(1, 1, 1, 0));
        assert!(!receipt.structurally_complete());
    }

    #[test]
    fn font_size_proof_binds_css_layout_and_a_fully_inherited_terminal() {
        fn complete_receipt() -> ClassTextThemeReceipt {
            let mut receipt = empty_receipt();
            receipt.css_emission = Some(ClassTypographyCssEmission::from_successful_writes(
                "Inter,sans-serif",
                "Inter,sans-serif",
                "16px",
                "16px",
                "Inter,sans-serif",
                "Inter,sans-serif",
            ));
            receipt.diagram_title.observed = Some(ClassTextTerminalFacts::default());
            receipt
                .nodes
                .insert("A".into(), ClassTextTerminalCheckpoint::new(true));
            receipt.record_node("A", ClassTextTerminalFacts::new(1, 1, 1, 0));
            receipt
        }

        let receipt = complete_receipt();
        assert!(receipt.structurally_complete());
        assert!(receipt.proves_font_size());

        let mut css_mismatch = complete_receipt();
        css_mismatch.css_emission = Some(ClassTypographyCssEmission::from_successful_writes(
            "Inter,sans-serif",
            "Inter,sans-serif",
            "15px",
            "15px",
            "Inter,sans-serif",
            "Inter,sans-serif",
        ));
        assert!(css_mismatch.structurally_complete());
        assert!(!css_mismatch.proves_font_size());

        let mut layout_mismatch = complete_receipt();
        layout_mismatch.layout_font_size_px = Some(15.0);
        assert!(layout_mismatch.structurally_complete());
        assert!(!layout_mismatch.proves_font_size());
    }

    #[test]
    fn unverified_terminal_is_structurally_complete_but_cannot_prove_application() {
        let mut unverified = empty_receipt();
        unverified.css_emission = Some(ClassTypographyCssEmission::from_successful_writes(
            "Inter,sans-serif",
            "Inter,sans-serif",
            "16px",
            "16px",
            "Inter,sans-serif",
            "Inter,sans-serif",
        ));
        unverified.diagram_title.observed = Some(ClassTextTerminalFacts::default());
        unverified.nodes.insert(
            "A".into(),
            ClassTextTerminalCheckpoint {
                expected_paint: None,
                expected_visible: true,
                observed: Some(ClassTextTerminalFacts::new(1, 0, 0, 1)),
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
            "16px",
            "16px",
            "Inter,sans-serif",
            "Inter,sans-serif",
        ));
        receipt.diagram_title.observed = Some(ClassTextTerminalFacts::default());
        receipt.edges.insert(
            "rel".into(),
            ClassTextEdgeCheckpoint {
                label: ClassTextTerminalCheckpoint::new(true),
                cardinalities: [ClassTextTerminalCheckpoint::new(false); 4],
            },
        );
        receipt.record_edge_label("rel", ClassTextTerminalFacts::default());
        for slot in 0..4 {
            receipt.record_cardinality("rel", slot, ClassTextTerminalFacts::default());
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

        let plan = ClassTextThemePlan::resolve(None, &config);

        assert_eq!(plan.layout_font_family_css(), "Class Root,monospace");
        assert_eq!(plan.stylesheet_font_family_css(), "Class Theme,sans-serif");
        assert!(!plan.inherited_font_stack.typed_font_stack_active());
    }

    #[test]
    fn node_style_facts_are_sealed_once_and_missing_ids_fail_closed() {
        let config = merman_core::MermaidConfig::default();
        let plan = ClassTextThemePlan::resolve(None, &config);

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
        owned.observe(&invalid, true, false, true, true, false, true, true, false);
        let owned_terminal = ClassTextTerminalFacts::from_node_style_facts(owned);
        assert!(owned_terminal.source_owns_every_visible_run());
        assert!(!owned_terminal.color_ownership_is_unverified());
        assert_eq!(owned.unverified_font_run_count(), 0);

        let mut inherited = ClassNodeLabelStyleFacts::default();
        inherited.observe(
            &invalid, false, false, false, false, false, false, false, false,
        );
        let inherited_terminal = ClassTextTerminalFacts::from_node_style_facts(inherited);
        assert!(!inherited_terminal.source_owns_every_visible_run());
        assert!(inherited_terminal.color_ownership_is_unverified());
        assert_eq!(inherited.unverified_font_run_count(), 1);
    }
    #[test]
    fn partially_shadowed_text_evidence_preserves_rule_identity_and_ordinal_residuals() {
        use crate::diagram_theme::{
            CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector, Specified,
            ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStylePatch,
        };
        use crate::environment::RenderEnvironment;
        use crate::svg::{SvgDebugOptions, SvgRenderOptions};

        for ordinal in [false, true] {
            for source_owned in [false, true] {
                let mut mixed =
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap());
                mixed.stroke.paint = Specified::Clear;
                let mut stroke = ThemeStylePatch::default();
                stroke.stroke.paint = Specified::Value(CanvasPaint::solid("#654321").unwrap());
                let mut later = ThemeRule::new(ThemeTarget::Text, stroke);
                if ordinal {
                    later = later.with_ordinal(OrdinalSelector::exact(1).unwrap());
                }
                let theme = DiagramThemeCompiler::new()
                    .compile(
                        DiagramThemeSpec::new().with_styles(
                            ThemeRuleSet::default()
                                .with_rule(ThemeRule::new(ThemeTarget::Text, mixed))
                                .with_rule(later),
                        ),
                    )
                    .unwrap();
                let source = if source_owned {
                    "classDiagram\nclass Account\nstyle Account color:#cc0000\n"
                } else {
                    "classDiagram\nclass Account\n"
                };
                let parsed = crate::__private::install_parse_compatibility(
                    &theme,
                    merman_core::Engine::new(),
                )
                .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
                .unwrap()
                .unwrap();
                let session = RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort)
                    .begin_session_with_theme(&theme)
                    .unwrap();
                let rendered =
                    crate::family::prepare(parsed, &crate::LayoutOptions::default(), session)
                        .unwrap()
                        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                        .unwrap();
                let completion = rendered.into_completion();
                let report = completion.report().style_report();
                let key = FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Text,
                };
                let applied = if !ordinal && !source_owned {
                    vec![key.clone()]
                } else {
                    vec![]
                };
                let not_applicable = if !ordinal && source_owned {
                    vec![key]
                } else {
                    vec![]
                };
                assert_eq!(
                    report.theme_applied_mechanisms(),
                    applied,
                    "ordinal={ordinal}, source={source_owned}"
                );
                assert_eq!(
                    report.theme_not_applicable_mechanisms(),
                    not_applicable,
                    "ordinal={ordinal}, source={source_owned}"
                );
            }
        }
    }
}
