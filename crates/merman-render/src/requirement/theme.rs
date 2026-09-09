use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use merman_core::MermaidConfig;
use merman_core::diagrams::requirement::RequirementDiagramRenderModel;

use super::source_typography::RequirementNodeTypography;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle,
    ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectPaintTerminalLedger, DirectStaticSelectorDomain,
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::mermaid_style::{CssFontFamilyOwnership, CssFontSizeOwnership};
use crate::resources::OperationWorkMeter;

fn configured_font_size_css(effective_config: &MermaidConfig) -> Box<str> {
    crate::config::config_css_number_or_string(
        effective_config.as_value(),
        &["themeVariables", "fontSize"],
    )
    .or_else(|| {
        crate::config::config_css_number_or_string(effective_config.as_value(), &["fontSize"])
    })
    .unwrap_or_else(|| "16px".to_string())
    .into_boxed_str()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NodeExpectation {
    fill: Option<DirectPaintExpectation>,
    stroke: Option<DirectPaintExpectation>,
}

/// Token issued only after the writer has successfully emitted one divider terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RequirementDividerEmission {
    node_index: usize,
    stroke: Arc<str>,
    style: Arc<str>,
}

impl RequirementDividerEmission {
    pub(crate) fn from_successful_write(
        node_index: usize,
        stroke: impl Into<Arc<str>>,
        style: impl Into<Arc<str>>,
    ) -> Self {
        Self {
            node_index,
            stroke: stroke.into(),
            style: style.into(),
        }
    }
}

/// Requirement box paint resolved once for semantic nodes and shared by SVG emission and evidence.
#[derive(Debug)]
pub(crate) struct RequirementPaintThemePlan {
    relation_paint: Option<super::RequirementRelationPaintPlan>,
    node_indices: BTreeMap<String, usize>,
    expectations: Option<Arc<[NodeExpectation]>>,
    inherited_font_stack: InheritedFontStackPlan,
    font_size_css: Box<str>,
    font_size_px: f64,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    title_present: bool,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeMap<ResolvedStyleProperty, ThemeCapability>>,
    terminal_receipt: OnceLock<Option<RequirementPaintThemeReceipt>>,
}

impl RequirementPaintThemePlan {
    pub(crate) fn resolve_with_title(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        model: &RequirementDiagramRenderModel,
        title: Option<&str>,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let configured_font_size_css = configured_font_size_css(effective_config);
        let title_present = title.is_some_and(|title| !title.trim().is_empty());
        let Some(theme) = theme else {
            return Ok(Self {
                relation_paint: None,
                node_indices: BTreeMap::new(),
                expectations: None,
                inherited_font_stack,
                font_size_css: configured_font_size_css,
                font_size_px: crate::config::config_theme_or_root_font_size_px(
                    effective_config.as_value(),
                    16.0,
                )
                .max(1.0),
                typed_font_size_requested: false,
                typed_font_size_active: false,
                title_present,
                evidence: FamilyThemeEvidence::default(),
                pending: BTreeMap::new(),
                terminal_receipt: OnceLock::new(),
            });
        };

        // Node identities and checkpoints belong to theme evidence, not ordinary rendering.
        let mut node_indices = BTreeMap::new();
        for node_id in model
            .requirements
            .iter()
            .map(|node| node.name.as_str())
            .chain(model.elements.iter().map(|node| node.name.as_str()))
            .filter(|node_id| *node_id != "__proto__")
        {
            let next_index = node_indices.len();
            node_indices
                .entry(node_id.to_string())
                .or_insert(next_index);
        }
        let node_count = node_indices.len();
        let expectations = vec![NodeExpectation::default(); node_count];

        let typed_font_size_requested = theme.family_mechanism_routes().iter().any(|route| {
            route.mechanism()
                == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                && route.disposition() == FamilyThemeDisposition::TypedAdapter
        });
        let config_owns_font_size = typed_font_size_requested
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.fontSize",
            );
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let (font_size_px, font_size_css) = if typed_font_size_active {
            let font_size_px = f64::from(theme.typography().font_size_px()).max(1.0);
            (
                font_size_px,
                format!("{}px", theme.typography().font_size_px()).into_boxed_str(),
            )
        } else {
            (
                crate::config::config_theme_or_root_font_size_px(effective_config.as_value(), 16.0)
                    .max(1.0),
                configured_font_size_css,
            )
        };

        let mermaid_owns_fill = mermaid_owns_requirement_fill(effective_config);
        let mermaid_owns_stroke = mermaid_owns_requirement_stroke(effective_config);
        let source_owned_fill =
            requirement_source_owned_fill_mask(model, &node_indices, mermaid_owns_fill);
        let mut expectations = expectations;
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut expected_capabilities =
            BTreeMap::<(usize, ResolvedStyleProperty), ThemeCapability>::new();

        for (node_index, expectation) in expectations.iter_mut().enumerate() {
            let style = theme.style_with_work_meter(
                ThemeTarget::Requirement,
                ThemeVariant::Default,
                Some(node_index + 1),
                work_meter,
            )?;
            for (property, origin) in style.winner_rule_properties() {
                winner_properties.insert((origin.rule_index(), property));
            }
            if let Some(expected) = typed_fill_expectation(theme, &style, mermaid_owns_fill) {
                expected_capabilities.insert(
                    (expected.rule_index(), ResolvedStyleProperty::Fill),
                    expected.capability(),
                );
                expectation.fill = Some(expected);
            }
            if let Some(expected) = typed_stroke_expectation(theme, &style, mermaid_owns_stroke) {
                expected_capabilities.insert(
                    (expected.rule_index(), ResolvedStyleProperty::Stroke),
                    expected.capability(),
                );
                expectation.stroke = Some(expected);
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let relation_paint = theme
            .family_mechanism_routes()
            .iter()
            .any(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Relation,
                        ..
                    } | FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Relation
                    } | FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Relation,
                        ..
                    }
                )
            })
            .then(|| {
                super::RequirementRelationPaintPlan::resolve(
                    theme,
                    effective_config,
                    model,
                    &mut evidence,
                    work_meter,
                )
            })
            .transpose()?;
        let mut observations = BTreeMap::<usize, RequirementPaintRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Requirement,
                    facet,
                    ..
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    if !winner_properties.contains(&(rule_index, property)) {
                        continue;
                    }
                    observation.applicable = true;
                    if (mermaid_owns_fill && matches!(facet, FamilyThemeRuleFacet::Fill(_)))
                        || (mermaid_owns_stroke && matches!(facet, FamilyThemeRuleFacet::Stroke(_)))
                    {
                        continue;
                    }
                    match (route.disposition(), facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            )
                            | FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some(capability) =
                                expected_capabilities.get(&(rule_index, property))
                            {
                                observation.capabilities.insert(property, *capability);
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (FamilyThemeDisposition::Unsupported, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Requirement,
                } => {}
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Requirement,
                    ..
                } => {}
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Requirement,
                TerminalVariantDomain::uniform(node_count, ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned_fill)],
            work_meter,
        )?;

        let mut pending = BTreeMap::new();
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Requirement,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules remain fail-closed until every winning facet has a terminal owner.
            } else if !observation.capabilities.is_empty() {
                pending.insert(key, observation.capabilities);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            relation_paint,
            node_indices,
            expectations: Some(expectations.into()),
            inherited_font_stack,
            font_size_css,
            font_size_px,
            typed_font_size_requested,
            typed_font_size_active,
            title_present,
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn index_for_node_id(&self, node_id: &str) -> Option<usize> {
        self.node_indices.get(node_id).copied()
    }

    pub(crate) fn relation_paint(&self) -> Option<&super::RequirementRelationPaintPlan> {
        self.relation_paint.as_ref()
    }

    pub(crate) fn typed_fill(
        &self,
        node_index: usize,
        source_owns_fill: bool,
    ) -> Option<(usize, &str)> {
        (!source_owns_fill)
            .then(|| self.expectations.as_ref()?.get(node_index)?.fill.as_ref())
            .flatten()
            .map(|expected| (expected.rule_index(), expected.css()))
    }

    pub(crate) fn typed_stroke(
        &self,
        node_index: usize,
        source_owns_stroke: bool,
    ) -> Option<(usize, &str)> {
        (!source_owns_stroke)
            .then(|| self.expectations.as_ref()?.get(node_index)?.stroke.as_ref())
            .flatten()
            .map(|expected| (expected.rule_index(), expected.css()))
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn font_family_override(&self) -> Option<&str> {
        self.inherited_font_stack
            .typed_font_stack_active()
            .then_some(self.font_family_css())
    }

    pub(crate) fn font_size_override(&self) -> Option<f64> {
        self.typed_font_size_active.then_some(self.font_size_px)
    }

    pub(crate) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    pub(crate) fn font_size_override_css(&self) -> Option<&str> {
        self.typed_font_size_active.then_some(self.font_size_css())
    }

    pub(crate) fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested() || self.typed_font_size_requested
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<RequirementPaintThemeReceipt> {
        Some(RequirementPaintThemeReceipt::with_typography(
            Arc::clone(self.expectations.as_ref()?),
            self.title_present,
            self.inherited_font_stack.typed_font_stack_requested(),
            self.typed_font_size_requested,
            self.font_family_css(),
            self.font_size_css(),
        ))
    }

    pub(crate) fn record_terminal(&self, receipt: Option<RequirementPaintThemeReceipt>) -> bool {
        let complete = match receipt.as_ref() {
            Some(receipt) => self.expectations.is_some() && receipt.proves_complete(),
            None => self.expectations.is_none(),
        };
        complete && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(plan) = self.relation_paint.as_ref() {
            plan.finish_evidence(&mut evidence);
        }
        let Some(Some(receipt)) = self.terminal_receipt.get() else {
            self.inherited_font_stack
                .mark_unsupported_typography_evidence(
                    &mut evidence,
                    self.title_present || !self.node_indices.is_empty(),
                );
            for (property, requested, active) in [
                (
                    ThemeTypographyProperty::FontStack,
                    self.inherited_font_stack.typed_font_stack_requested(),
                    self.inherited_font_stack.typed_font_stack_active(),
                ),
                (
                    ThemeTypographyProperty::FontSize,
                    self.typed_font_size_requested,
                    self.typed_font_size_active,
                ),
            ] {
                if !requested {
                    continue;
                }
                let key = FamilyThemeMechanismKey::Typography(property);
                if active {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                } else {
                    evidence.mark_not_applicable(key);
                }
            }
            return evidence;
        };
        for (key, properties) in &self.pending {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography(_)
                | FamilyThemeMechanismKey::OrdinalPalette { .. }
                | FamilyThemeMechanismKey::EffectBinding { .. } => continue,
            };
            let capabilities = properties
                .iter()
                .filter(|(property, _)| receipt.proves_property(rule_index, **property))
                .map(|(_, capability)| *capability)
                .collect::<BTreeSet<_>>();
            if !capabilities.is_empty() {
                evidence.mark_applied_with_capabilities(key.clone(), capabilities);
            } else if receipt.proves_complete() && !receipt.has_effective_rule(rule_index) {
                evidence.mark_not_applicable(key.clone());
            }
        }
        if self.typography_requested() {
            self.inherited_font_stack
                .mark_unsupported_typography_evidence(&mut evidence, receipt.has_visible_text());
            for (property, requested, active, inherited_terminal, unverified_source) in [
                (
                    ThemeTypographyProperty::FontStack,
                    self.inherited_font_stack.typed_font_stack_requested(),
                    self.inherited_font_stack.typed_font_stack_active(),
                    receipt.has_inherited_font_family_terminal(),
                    receipt.unverified_source_font_family,
                ),
                (
                    ThemeTypographyProperty::FontSize,
                    self.typed_font_size_requested,
                    self.typed_font_size_active,
                    receipt.has_inherited_font_size_terminal(),
                    receipt.unverified_source_font_size,
                ),
            ] {
                if !requested {
                    continue;
                }
                let key = FamilyThemeMechanismKey::Typography(property);
                if !active {
                    evidence.mark_not_applicable(key);
                } else if unverified_source {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                } else if !inherited_terminal {
                    evidence.mark_not_applicable(key);
                } else if match property {
                    ThemeTypographyProperty::FontStack => receipt.proves_font_family(),
                    ThemeTypographyProperty::FontSize => receipt.proves_font_size(),
                    _ => false,
                } {
                    evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                } else {
                    evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                }
            }
        }
        evidence
    }
}

fn requirement_source_owned_fill_mask(
    model: &RequirementDiagramRenderModel,
    node_indices: &BTreeMap<String, usize>,
    mermaid_owns_fill: bool,
) -> Vec<bool> {
    let mut owned = vec![mermaid_owns_fill; node_indices.len()];
    for node in model
        .requirements
        .iter()
        .map(|node| (&node.name, &node.css_styles))
        .chain(
            model
                .elements
                .iter()
                .map(|node| (&node.name, &node.css_styles)),
        )
    {
        let Some(index) = node_indices.get(node.0).copied() else {
            continue;
        };
        if node.1.iter().any(|raw| {
            crate::mermaid_style::parse_style_declaration(raw)
                .is_some_and(|declaration| declaration.property() == "fill")
        }) {
            owned[index] = true;
        }
    }
    owned
}

fn typed_fill_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns: bool,
) -> Option<DirectPaintExpectation> {
    if mermaid_owns {
        return None;
    }
    resolve_direct_static_fill(
        theme,
        style,
        &[ThemeTarget::Requirement],
        DirectStaticSelectorDomain::Default,
    )
    .map(DirectPaintExpectation::from_paint)
}

fn typed_stroke_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns: bool,
) -> Option<DirectPaintExpectation> {
    if mermaid_owns {
        return None;
    }
    resolve_direct_static_stroke(
        theme,
        style,
        &[ThemeTarget::Requirement],
        DirectStaticSelectorDomain::Default,
    )
    .map(DirectPaintExpectation::from_paint)
}

fn mermaid_owns_requirement_fill(config: &MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(
        config,
        "themeVariables.requirementBackground",
    ) || matches!(
        config.get_str("theme"),
        Some("redux-color" | "redux-dark-color")
    ) || (merman_core::__private::config_path_overrides_typed_default(
        config,
        "themeVariables.borderColorArray",
    ) && merman_core::__private::config_path_overrides_typed_default(
        config,
        "themeVariables.bkgColorArray",
    ))
}

fn mermaid_owns_requirement_stroke(config: &MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.nodeBorder")
        || matches!(
            config.get_str("theme"),
            Some("redux-color" | "redux-dark-color")
        )
        || merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.borderColorArray",
        )
}

#[derive(Debug, Default)]
struct RequirementPaintRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeMap<ResolvedStyleProperty, ThemeCapability>,
}

/// Writer-owned proof that every semantic Requirement node reached its canonical paint paths.
#[derive(Debug, Clone)]
pub(crate) struct RequirementPaintThemeReceipt {
    expectations: Arc<[NodeExpectation]>,
    checkpointed_nodes: Vec<bool>,
    expected_title_count: usize,
    emitted_title_count: usize,
    title_matches: bool,
    attributes_match: bool,
    paint_ledger: DirectPaintTerminalLedger,
    font_family_requested: bool,
    font_size_requested: bool,
    expected_font_family: Box<str>,
    expected_font_size: Box<str>,
    typography_font_family_recorded: bool,
    typography_font_family_verified: bool,
    typography_font_size_recorded: bool,
    typography_font_size_verified: bool,
    visible_node_count: usize,
    inherited_node_font_family: bool,
    inherited_node_font_size: bool,
    inherited_edge_typography: bool,
    unverified_source_font_family: bool,
    unverified_source_font_size: bool,
}

impl RequirementPaintThemeReceipt {
    fn with_typography(
        expectations: Arc<[NodeExpectation]>,
        title_present: bool,
        font_family_requested: bool,
        font_size_requested: bool,
        expected_font_family: &str,
        expected_font_size: &str,
    ) -> Self {
        Self {
            checkpointed_nodes: vec![false; expectations.len()],
            expectations,
            expected_title_count: usize::from(title_present),
            emitted_title_count: 0,
            title_matches: true,
            attributes_match: true,
            paint_ledger: DirectPaintTerminalLedger::default(),
            font_family_requested,
            font_size_requested,
            expected_font_family: expected_font_family.into(),
            expected_font_size: expected_font_size.into(),
            typography_font_family_recorded: false,
            typography_font_family_verified: false,
            typography_font_size_recorded: false,
            typography_font_size_verified: false,
            visible_node_count: 0,
            inherited_node_font_family: false,
            inherited_node_font_size: false,
            inherited_edge_typography: false,
            unverified_source_font_family: false,
            unverified_source_font_size: false,
        }
    }

    pub(crate) fn record_typography(&mut self, emitted_font_family: &str, emitted_font_size: &str) {
        self.record_font_family(emitted_font_family);
        self.record_font_size(emitted_font_size);
    }

    fn record_font_family(&mut self, emitted_font_family: &str) {
        if !self.font_family_requested {
            return;
        }
        if self.typography_font_family_recorded {
            self.typography_font_family_verified = false;
            return;
        }
        self.typography_font_family_recorded = true;
        self.typography_font_family_verified =
            emitted_font_family == self.expected_font_family.as_ref();
    }

    fn record_font_size(&mut self, emitted_font_size: &str) {
        if !self.font_size_requested {
            return;
        }
        if self.typography_font_size_recorded {
            self.typography_font_size_verified = false;
            return;
        }
        self.typography_font_size_recorded = true;
        self.typography_font_size_verified = emitted_font_size == self.expected_font_size.as_ref();
    }

    pub(crate) fn record_title_text(&mut self, emitted_class: &str, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.emitted_title_count = self.emitted_title_count.saturating_add(1);
        self.title_matches &= emitted_class == "requirementDiagramTitleText";
    }

    pub(crate) fn record_edge_text(&mut self, text: &str) {
        self.inherited_edge_typography |= !text.trim().is_empty();
    }

    pub(crate) fn record_checkpointed_node(
        &mut self,
        node_index: usize,
        has_visible_text: bool,
        source_typography: &RequirementNodeTypography,
        emitted_label_styles: &str,
        source_owns_fill: bool,
        emitted_fill: Option<(usize, &str)>,
        terminal_fill: &str,
        source_owns_stroke: bool,
        emitted_stroke: Option<(usize, &str)>,
        terminal_stroke: &str,
        terminal_stroke_style: &str,
        divider_expected: bool,
        divider_emissions: &[RequirementDividerEmission],
    ) {
        let Some(checkpointed) = self.checkpointed_nodes.get_mut(node_index) else {
            self.attributes_match = false;
            return;
        };
        if *checkpointed {
            self.attributes_match = false;
            return;
        }
        *checkpointed = true;
        let has_visible_text = has_visible_text && !source_typography.has_zero_font_size();
        if has_visible_text {
            self.visible_node_count = self.visible_node_count.saturating_add(1);
        }
        if has_visible_text && (self.font_family_requested || self.font_size_requested) {
            let matches_emission = source_typography.matches_emitted_style(emitted_label_styles);
            self.inherited_node_font_family |=
                source_typography.font_family_ownership() == CssFontFamilyOwnership::Inherited;
            self.inherited_node_font_size |=
                source_typography.font_size_ownership() == CssFontSizeOwnership::Inherited;
            self.unverified_source_font_family |= !matches_emission
                || source_typography.font_family_ownership() == CssFontFamilyOwnership::Unverified;
            self.unverified_source_font_size |= !matches_emission
                || source_typography.font_size_ownership() == CssFontSizeOwnership::Unverified;
        }

        let Some(expectation) = self.expectations.get(node_index) else {
            self.attributes_match = false;
            return;
        };
        if let Some(expected) = expectation.fill.as_ref()
            && !source_owns_fill
        {
            self.attributes_match &= expected.css() == terminal_fill;
        }
        if let Some(expected) = expectation.stroke.as_ref()
            && !source_owns_stroke
        {
            self.attributes_match &= expected.css() == terminal_stroke;
            self.attributes_match &=
                important_style_value(terminal_stroke_style, "stroke") == Some(expected.css());
        }
        self.attributes_match &= match (divider_expected, divider_emissions) {
            (false, []) => true,
            (true, [emission]) => {
                let paint_matches = match expectation.stroke.as_ref() {
                    None => true,
                    Some(expected) => {
                        source_owns_stroke
                            || (emission.stroke.as_ref() == expected.css()
                                && important_style_value(emission.style.as_ref(), "stroke")
                                    == Some(expected.css()))
                    }
                };
                emission.node_index == node_index && paint_matches
            }
            _ => false,
        };
        self.attributes_match &= self.paint_ledger.record(
            expectation.fill.as_ref(),
            source_owns_fill,
            emitted_fill,
            ResolvedStyleProperty::Fill,
        );
        self.attributes_match &= self.paint_ledger.record(
            expectation.stroke.as_ref(),
            source_owns_stroke,
            emitted_stroke,
            ResolvedStyleProperty::Stroke,
        );
    }

    fn proves_complete(&self) -> bool {
        self.attributes_match
            && self.checkpointed_nodes.iter().all(|entry| *entry)
            && self.emitted_title_count == self.expected_title_count
            && self.title_matches
            && self.proves_font_family()
            && self.proves_font_size()
    }

    fn proves_font_family(&self) -> bool {
        !self.font_family_requested
            || (self.typography_font_family_recorded && self.typography_font_family_verified)
    }

    fn proves_font_size(&self) -> bool {
        !self.font_size_requested
            || (self.typography_font_size_recorded && self.typography_font_size_verified)
    }

    fn has_visible_text(&self) -> bool {
        self.visible_node_count != 0
            || self.emitted_title_count != 0
            || self.inherited_edge_typography
    }

    fn has_inherited_font_family_terminal(&self) -> bool {
        self.inherited_node_font_family
            || self.inherited_edge_typography
            || self.emitted_title_count != 0
    }

    fn has_inherited_font_size_terminal(&self) -> bool {
        self.inherited_node_font_size
            || self.inherited_edge_typography
            || self.emitted_title_count != 0
    }

    fn has_effective_rule(&self, rule_index: usize) -> bool {
        self.paint_ledger.has_effective_rule(rule_index)
    }

    fn proves_property(&self, rule_index: usize, property: ResolvedStyleProperty) -> bool {
        self.paint_ledger
            .proves_property(self.proves_complete(), rule_index, property)
    }
}

fn important_style_value<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style.split(';').find_map(|declaration| {
        let (candidate, raw_value) = declaration.split_once(':')?;
        if candidate.trim() != property {
            return None;
        }
        let value = raw_value.trim();
        let (value, important) = value
            .strip_suffix("!important")
            .map_or((value, false), |value| (value.trim(), true));
        important.then_some(value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::ThemeCapability;

    #[test]
    fn unthemed_plan_keeps_config_without_per_node_evidence() {
        use merman_core::diagrams::requirement::RequirementRenderElement;

        let config = MermaidConfig::from_value(serde_json::json!({
            "fontFamily": "Config Sans, Arial",
            "themeVariables": {"fontSize": "24px"}
        }));
        let work_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        for node_count in [0, 1, 4096] {
            let model = RequirementDiagramRenderModel {
                acc_title: None,
                acc_descr: None,
                direction: "TB".to_owned(),
                requirements: Vec::new(),
                elements: (0..node_count)
                    .map(|index| RequirementRenderElement {
                        name: format!("element_{index}"),
                        element_type: "component".to_owned(),
                        doc_ref: String::new(),
                        css_styles: Vec::new(),
                        classes: Vec::new(),
                    })
                    .collect(),
                relationships: Vec::new(),
                classes: BTreeMap::new(),
            };
            let plan = RequirementPaintThemePlan::resolve_with_title(
                None,
                &config,
                &model,
                Some("Visible title"),
                &work_meter,
            )
            .unwrap();
            assert_eq!(plan.font_family_css(), "Config Sans,Arial");
            assert_eq!(plan.font_size_css(), "24px");
            assert_eq!(plan.font_size_px, 24.0);
            assert!(plan.font_family_override().is_none());
            assert!(plan.font_size_override().is_none());
            assert!(plan.node_indices.is_empty(), "node_count={node_count}");
            assert!(plan.expectations.is_none(), "node_count={node_count}");
            assert!(plan.pending.is_empty());
            assert!(plan.begin_terminal_receipt().is_none());
            assert!(
                !plan.record_terminal(Some(RequirementPaintThemeReceipt::with_typography(
                    Vec::new().into(),
                    false,
                    false,
                    false,
                    "",
                    ""
                )))
            );
            assert!(plan.record_terminal(None));
            assert!(matches!(plan.terminal_receipt.get(), Some(None)));
            assert!(!plan.record_terminal(None));
        }
    }

    #[test]
    fn themed_empty_plan_requires_one_complete_receipt() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, ThemeTextStyle, TypographySpec,
        };

        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    crate::DiagramFamilyId::REQUIREMENT,
                    ThemeTextStyle::default().with_font_size_px(24.0).unwrap(),
                ),
            ))
            .unwrap();
        let resolved = theme.resolve(crate::DiagramFamilyId::REQUIREMENT);
        let model = serde_json::from_value(serde_json::json!({})).unwrap();
        let plan = RequirementPaintThemePlan::resolve_with_title(
            Some(&resolved),
            &MermaidConfig::empty_object(),
            &model,
            None,
            &OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .unwrap();

        assert!(plan.node_indices.is_empty());
        assert!(
            !plan.record_terminal(None),
            "empty is not the same as unthemed"
        );
        assert!(
            !plan.record_terminal(plan.begin_terminal_receipt()),
            "missing typography emission"
        );
        let mut receipt = plan
            .begin_terminal_receipt()
            .expect("theme requires evidence");
        receipt.record_typography(plan.font_family_css(), plan.font_size_css());
        let duplicate = receipt.clone();
        assert!(plan.record_terminal(Some(receipt)));
        assert!(!plan.record_terminal(Some(duplicate)));
        assert!(plan.finish_evidence().not_applicable_mechanisms().contains(
            &FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontSize)
        ));
    }

    fn stroke_receipt() -> RequirementPaintThemeReceipt {
        RequirementPaintThemeReceipt::with_typography(
            vec![NodeExpectation {
                stroke: Some(DirectPaintExpectation::new(
                    7,
                    "#123456",
                    ThemeCapability::SolidPaint,
                )),
                ..NodeExpectation::default()
            }]
            .into(),
            false,
            false,
            false,
            "",
            "16px",
        )
    }

    #[test]
    fn visible_edge_text_remains_applicable_without_visible_node_text() {
        let mut receipt = stroke_receipt();
        receipt.record_edge_text(" ");
        assert!(!receipt.has_visible_text());
        receipt.record_edge_text("satisfies");
        assert!(receipt.has_visible_text());
        assert!(receipt.has_inherited_font_family_terminal());
        assert!(receipt.has_inherited_font_size_terminal());
    }

    #[test]
    fn typed_stroke_receipt_requires_the_inline_cascade_winner() {
        let mut receipt = stroke_receipt();
        receipt.record_checkpointed_node(
            0,
            true,
            &RequirementNodeTypography::default(),
            "",
            false,
            None,
            "#ececff",
            false,
            Some((7, "#123456")),
            "#123456",
            "stroke:#123456",
            false,
            &[],
        );

        assert!(
            !receipt.proves_complete(),
            "a stylesheet-only stroke must not be treated as the final typed winner"
        );
    }

    #[test]
    fn typography_receipt_proves_requested_properties_independently() {
        let empty_expectations: Arc<[NodeExpectation]> = Vec::new().into();

        let mut size_only = RequirementPaintThemeReceipt::with_typography(
            Arc::clone(&empty_expectations),
            false,
            false,
            true,
            "unused-family",
            "24px",
        );
        size_only.record_typography("wrong-family", "24px");
        assert!(size_only.proves_complete());

        let mut family_only = RequirementPaintThemeReceipt::with_typography(
            empty_expectations,
            false,
            true,
            false,
            "Requirement Typed",
            "unused-size",
        );
        family_only.record_typography("Requirement Typed", "wrong-size");
        assert!(family_only.proves_complete());
    }

    #[test]
    fn typography_receipt_does_not_treat_empty_node_labels_as_visible_text() {
        let mut receipt = RequirementPaintThemeReceipt::with_typography(
            vec![NodeExpectation::default()].into(),
            false,
            false,
            true,
            "unused-family",
            "24px",
        );
        receipt.record_checkpointed_node(
            0,
            false,
            &RequirementNodeTypography::default(),
            "",
            false,
            None,
            "#ececff",
            false,
            None,
            "#9370db",
            "",
            false,
            &[],
        );
        receipt.record_typography("unused-family", "24px");

        assert!(!receipt.has_visible_text());
        assert!(receipt.proves_complete());
    }

    #[test]
    fn divider_receipt_requires_one_successful_writer_emission() {
        for emissions in [
            Vec::new(),
            vec![
                RequirementDividerEmission::from_successful_write(
                    0,
                    "#123456",
                    "stroke:#123456 !important",
                ),
                RequirementDividerEmission::from_successful_write(
                    0,
                    "#123456",
                    "stroke:#123456 !important",
                ),
            ],
        ] {
            let mut receipt = stroke_receipt();
            receipt.record_checkpointed_node(
                0,
                true,
                &RequirementNodeTypography::default(),
                "",
                false,
                None,
                "#ececff",
                false,
                Some((7, "#123456")),
                "#123456",
                "stroke:#123456 !important",
                true,
                &emissions,
            );

            assert!(
                !receipt.proves_complete(),
                "divider emission count {} must fail closed",
                emissions.len()
            );
        }

        let mut receipt = stroke_receipt();
        let emissions = [RequirementDividerEmission::from_successful_write(
            0,
            "#123456",
            "stroke:#123456 !important",
        )];
        receipt.record_checkpointed_node(
            0,
            true,
            &RequirementNodeTypography::default(),
            "",
            false,
            None,
            "#ececff",
            false,
            Some((7, "#123456")),
            "#123456",
            "stroke:#123456 !important",
            true,
            &emissions,
        );
        assert!(receipt.proves_complete());
    }
}
