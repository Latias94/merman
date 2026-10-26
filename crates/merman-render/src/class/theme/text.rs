use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeOutcomes, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan, resolved_style_property_for_facet,
};
use crate::text::TextStyle;

/// Class separates Mermaid's wrapping probe from the font inherited by displayed labels.
///
/// The probe reads root `fontFamily` first; final measurement follows the stylesheet's
/// `themeVariables.fontFamily` winner. A directly owned FontStack replaces both families.
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
    font_size_px: f64,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    node_style_facts_requested: bool,
    node_style_facts: OnceLock<BTreeMap<Box<str>, ClassNodeLabelStyleFacts>>,
    evidence: FamilyThemeOutcomes,
    outcomes: OnceLock<ClassTextOutcomes>,
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

/// Text facts accumulated at the writers that already know visibility and source ownership.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClassTextOutcomes {
    css_emitted: bool,
    node_paints: BTreeMap<Box<str>, usize>,
    namespace_paint: Option<usize>,
    edge_paint: Option<usize>,
    title_paint: Option<usize>,
    facts: ClassTextTerminalFacts,
    paint_outcomes: BTreeMap<usize, (bool, bool)>,
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
            style
                .filter(|style| !style.is_empty())
                .map(|_| (self.rule_index, self.css.as_str())),
            style.unwrap_or(""),
        )
    }
}

impl ClassTextThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::presentation::PresentationConfig,
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
                merman_core::__private::config_paint_overrides_typed_default(effective_config, path)
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
        let typed_font_size = theme.filter(|_| typed_font_size_active).map(|theme| {
            crate::number_format::canonicalize_number(f64::from(theme.typography().font_size_px()))
                .max(1.0)
        });
        let (font_size_px, font_size_css) = if let Some(font_size_px) = typed_font_size {
            (
                font_size_px,
                format!("{}px", crate::number_format::canonical_number(font_size_px)),
            )
        } else {
            let font_size_css = crate::config::config_css_number_or_string(
                effective_config.as_value(),
                &["themeVariables", "fontSize"],
            )
            .unwrap_or_else(|| "16px".to_string());
            let font_size_px = crate::config::config_f64_explicit_css_px(
                effective_config.as_value(),
                &["themeVariables", "fontSize"],
            )
            .unwrap_or(16.0)
            .max(1.0);
            (font_size_px, font_size_css)
        };
        let stylesheet_font_family_css = if inherited_font_stack.typed_font_stack_active() {
            crate::config::normalize_css_font_family(inherited_font_stack.font_family_css())
        } else {
            inherited_font_stack.font_family_css().to_owned()
        };
        let layout_font_family_css = if inherited_font_stack.typed_font_stack_active() {
            stylesheet_font_family_css.clone()
        } else {
            super::super::config::ClassConfigView::new(effective_config.as_value())
                .layout_font_family_css()
        };
        Self {
            css_binding: super::ClassCssThemeBinding::resolve(effective_config.as_value()),
            common_css: crate::svg::PreparedCommonCss::bind_normalized(
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
            font_size_px,
            typed_font_size_requested,
            typed_font_size_active,
            node_style_facts_requested: theme.is_some_and(|theme| {
                theme.family_mechanism_routes().iter().any(|route| {
                    matches!(
                        route.mechanism(),
                        FamilyThemeMechanism::RuleFacet {
                            target: ThemeTarget::NodeLabel | ThemeTarget::Text,
                            ..
                        } | FamilyThemeMechanism::OrdinalPalette {
                            target: ThemeTarget::NodeLabel | ThemeTarget::Text
                        } | FamilyThemeMechanism::EffectBinding {
                            target: ThemeTarget::NodeLabel | ThemeTarget::Text,
                            ..
                        }
                    )
                })
            }),
            node_style_facts: OnceLock::new(),
            evidence: FamilyThemeOutcomes::default(),
            outcomes: OnceLock::new(),
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

    pub(crate) fn bind_text_paints(
        &self,
        receipt: &mut ClassTextOutcomes,
        nodes: impl IntoIterator<Item = (impl AsRef<str>, Option<usize>)>,
        namespace: Option<(usize, &str)>,
        note_ids: impl IntoIterator<Item = impl AsRef<str>>,
    ) {
        let generic = |rule| self.text_rules.contains_key(&rule).then_some(rule);
        receipt
            .node_paints
            .extend(nodes.into_iter().filter_map(|(id, rule)| {
                rule.and_then(generic)
                    .map(|rule| (id.as_ref().into(), rule))
            }));
        if let Some(paint) = &self.note_paint {
            receipt.node_paints.extend(
                note_ids
                    .into_iter()
                    .map(|id| (id.as_ref().into(), paint.rule_index)),
            );
        }
        receipt.namespace_paint = namespace.and_then(|(rule, _)| generic(rule));
        receipt.edge_paint = self.edge_paint.as_ref().map(|paint| paint.rule_index);
        receipt.title_paint = self.title_paint.as_ref().map(|paint| paint.rule_index);
    }

    pub(crate) fn layout_font_family_css(&self) -> &str {
        &self.layout_font_family_css
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    pub(crate) const fn typed_font_size_active(&self) -> bool {
        self.typed_font_size_active
    }

    pub(crate) fn requires_text_outcomes(&self) -> bool {
        !self.text_rules.is_empty()
            || self.inherited_font_stack.typed_font_stack_active()
            || self.typed_font_size_active
            || self
                .inherited_font_stack
                .has_unsupported_typography_properties()
    }

    pub(crate) fn requires_node_style_facts(&self) -> bool {
        self.node_style_facts_requested
            || self.requires_text_outcomes()
            || self.inherited_font_stack.typography_requested()
            || self.typed_font_size_requested
    }

    pub(crate) fn apply_layout_text_styles(
        &self,
        text_style: &mut TextStyle,
        html_calc_text_style: &mut TextStyle,
    ) {
        text_style.font_family = Some(self.common_css.font_family().to_owned());
        html_calc_text_style.font_family = Some(self.layout_font_family_css().to_owned());
        if self.typed_font_size_active() {
            text_style.font_size = self.font_size_px();
        }
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

    pub(crate) fn begin_text_outcomes(&self) -> Option<ClassTextOutcomes> {
        self.requires_text_outcomes()
            .then(ClassTextOutcomes::default)
    }

    pub(crate) fn record_outcomes(&self, outcomes: Option<ClassTextOutcomes>) {
        if let Some(outcomes) = outcomes {
            let _ = self.outcomes.set(outcomes);
        }
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        self.inherited_font_stack
            .record_unsupported_typography_evidence(
                evidence,
                self.outcomes
                    .get()
                    .is_none_or(ClassTextOutcomes::has_visible_font_run),
            );
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        match self.inherited_font_stack.outcome() {
            InheritedFontStackOutcome::Typed => {
                match self.outcomes.get() {
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
                match self.outcomes.get() {
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
        if let Some(receipt) = self.outcomes.get() {
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
                let (applied, incomplete) = receipt
                    .paint_outcomes
                    .get(&index)
                    .copied()
                    .unwrap_or_default();
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
    pub(crate) const fn visible_run_count(self) -> usize {
        self.visible_runs
    }
    pub(crate) fn with_paint(mut self, emitted: Option<(usize, &str)>, style: &str) -> Self {
        let _ = style;
        self.paint_observation = emitted.map(|(rule, _)| (rule, true));
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

impl ClassTextOutcomes {
    pub(crate) fn record_stylesheet(&mut self) {
        self.css_emitted = true;
    }

    fn record(&mut self, paint: Option<usize>, facts: ClassTextTerminalFacts) {
        self.facts.merge(facts);
        let Some(index) = paint else {
            return;
        };
        if facts.visible_runs == 0 || facts.source_owns_every_visible_run() {
            return;
        }
        let (applied, unverified) = self.paint_outcomes.entry(index).or_default();
        if facts.has_verified_inherited_paint() && facts.paint_observation == Some((index, true)) {
            *applied = true;
        } else {
            *unverified = true;
        }
    }

    pub(crate) fn record_node(&mut self, id: &str, facts: ClassTextTerminalFacts) {
        self.record(self.node_paints.get(id).copied(), facts);
    }
    pub(crate) fn record_namespace(&mut self, _id: &str, facts: ClassTextTerminalFacts) {
        self.record(self.namespace_paint, facts);
    }
    pub(crate) fn record_edge_label(&mut self, _id: &str, facts: ClassTextTerminalFacts) {
        self.record(self.edge_paint, facts);
    }
    pub(crate) fn record_cardinality(
        &mut self,
        _id: &str,
        _slot: usize,
        facts: ClassTextTerminalFacts,
    ) {
        self.record(self.edge_paint, facts);
    }
    pub(crate) fn record_diagram_title(&mut self, facts: ClassTextTerminalFacts) {
        self.record(self.title_paint, facts);
    }
    fn proves_font_family(&self) -> bool {
        self.css_emitted && self.has_applied_font_run() && !self.has_unverified_font_run()
    }
    fn proves_font_size(&self) -> bool {
        self.css_emitted && self.has_applied_font_size_run() && !self.has_unverified_font_size_run()
    }
    fn has_applied_font_run(&self) -> bool {
        self.facts.layout_inherited_runs != 0 || self.facts.writer_inherited_runs != 0
    }
    fn has_visible_font_run(&self) -> bool {
        self.facts.visible_runs != 0
    }
    fn has_unverified_font_run(&self) -> bool {
        self.facts.unverified_runs != 0
    }
    fn has_applied_font_size_run(&self) -> bool {
        self.facts.fully_inherited_size_runs != 0
    }
    fn has_applicable_font_size_run(&self) -> bool {
        self.has_applied_font_size_run()
    }
    fn has_unverified_font_size_run(&self) -> bool {
        self.facts.unverified_size_runs != 0
    }
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
    fn inactive_plan_separates_class_probe_and_displayed_font_precedence() {
        let config = merman_core::presentation::PresentationConfig::from_value(json!({
            "fontFamily": "Class Root, monospace",
            "themeVariables": {
                "fontFamily": "Class Theme, sans-serif"
            }
        }));

        let plan = ClassTextThemePlan::resolve(None, &config);
        let render_config = super::super::super::ClassRenderConfig::resolve(&config, &plan);

        assert_eq!(
            render_config.text_style().font_family.as_deref(),
            Some("Class Theme,sans-serif")
        );
        assert_eq!(
            render_config.html_calc_text_style().font_family.as_deref(),
            Some("Class Root,monospace")
        );
        assert!(!plan.inherited_font_stack.typed_font_stack_active());
    }

    #[test]
    fn node_style_facts_are_sealed_once_and_missing_ids_fail_closed() {
        let config = merman_core::presentation::PresentationConfig::default();
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
                    merman_core::MermaidEngine::new(),
                )
                .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
                .unwrap()
                .unwrap();
                let session = RenderEnvironment::deterministic()
                    .with_theme_portability_requirement(ThemePortabilityRequirement::BestEffort)
                    .begin_session_with_theme(&theme)
                    .unwrap();
                let rendered =
                    crate::family::prepare(&parsed, &crate::LayoutOptions::default(), session)
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
