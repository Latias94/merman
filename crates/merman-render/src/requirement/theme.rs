use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use merman_core::MermaidConfig;
use merman_core::diagrams::requirement::RequirementDiagramRenderModel;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectPaintTerminalLedger, DirectStaticSelectorDomain,
    FamilyThemeEvidence, FamilyThemeResidualReason, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::OperationWorkMeter;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NodeExpectation {
    fill: Option<DirectPaintExpectation>,
    stroke: Option<DirectPaintExpectation>,
}

/// Requirement box paint resolved once for semantic nodes and shared by SVG emission and evidence.
#[derive(Debug)]
pub(crate) struct RequirementPaintThemePlan {
    node_indices: BTreeMap<String, usize>,
    expectations: Arc<[NodeExpectation]>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeMap<ResolvedStyleProperty, ThemeCapability>>,
    terminal_receipt: OnceLock<RequirementPaintThemeReceipt>,
}

impl RequirementPaintThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        model: &RequirementDiagramRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
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
        let Some(theme) = theme else {
            return Ok(Self {
                node_indices,
                expectations: expectations.into(),
                evidence: FamilyThemeEvidence::default(),
                pending: BTreeMap::new(),
                terminal_receipt: OnceLock::new(),
            });
        };

        let mermaid_owns_fill = mermaid_owns_requirement_fill(effective_config);
        let mermaid_owns_stroke = mermaid_owns_requirement_stroke(effective_config);
        let mut expectations = expectations;
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut expected_capabilities =
            BTreeMap::<(usize, ResolvedStyleProperty), ThemeCapability>::new();

        for node_index in 0..node_count {
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
                expectations[node_index].fill = Some(expected);
            }
            if let Some(expected) = typed_stroke_expectation(theme, &style, mermaid_owns_stroke) {
                expected_capabilities.insert(
                    (expected.rule_index(), ResolvedStyleProperty::Stroke),
                    expected.capability(),
                );
                expectations[node_index].stroke = Some(expected);
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
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
                } => {
                    let key = theme.family_mechanism_key(route);
                    if node_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Requirement,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if node_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() != FamilyThemeDisposition::LegacyCompatibility {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

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
            node_indices,
            expectations: expectations.into(),
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn index_for_node_id(&self, node_id: &str) -> Option<usize> {
        self.node_indices.get(node_id).copied()
    }

    pub(crate) fn typed_fill(
        &self,
        node_index: usize,
        source_owns_fill: bool,
    ) -> Option<(usize, &str)> {
        (!source_owns_fill)
            .then(|| self.expectations.get(node_index)?.fill.as_ref())
            .flatten()
            .map(|expected| (expected.rule_index(), expected.css()))
    }

    pub(crate) fn typed_stroke(
        &self,
        node_index: usize,
        source_owns_stroke: bool,
    ) -> Option<(usize, &str)> {
        (!source_owns_stroke)
            .then(|| self.expectations.get(node_index)?.stroke.as_ref())
            .flatten()
            .map(|expected| (expected.rule_index(), expected.css()))
    }

    pub(crate) fn begin_terminal_receipt(&self) -> RequirementPaintThemeReceipt {
        RequirementPaintThemeReceipt::new(Arc::clone(&self.expectations))
    }

    pub(crate) fn record_terminal(&self, receipt: RequirementPaintThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        for (key, properties) in &self.pending {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography
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
        evidence
    }
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
        DirectStaticSelectorDomain::Unqualified,
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
        DirectStaticSelectorDomain::Unqualified,
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
    attributes_match: bool,
    paint_ledger: DirectPaintTerminalLedger,
}

impl RequirementPaintThemeReceipt {
    fn new(expectations: Arc<[NodeExpectation]>) -> Self {
        Self {
            checkpointed_nodes: vec![false; expectations.len()],
            expectations,
            attributes_match: true,
            paint_ledger: DirectPaintTerminalLedger::default(),
        }
    }

    pub(crate) fn record_checkpointed_node(
        &mut self,
        node_index: usize,
        source_owns_fill: bool,
        emitted_fill: Option<(usize, &str)>,
        terminal_fill: &str,
        source_owns_stroke: bool,
        emitted_stroke: Option<(usize, &str)>,
        terminal_stroke: &str,
        divider_expected: bool,
        terminal_divider_stroke: Option<&str>,
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
            if let Some(divider_stroke) = terminal_divider_stroke {
                self.attributes_match &= expected.css() == divider_stroke;
            }
        }
        self.attributes_match &= divider_expected == terminal_divider_stroke.is_some();
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
        self.attributes_match && self.checkpointed_nodes.iter().all(|entry| *entry)
    }

    fn has_effective_rule(&self, rule_index: usize) -> bool {
        self.paint_ledger.has_effective_rule(rule_index)
    }

    fn proves_property(&self, rule_index: usize, property: ResolvedStyleProperty) -> bool {
        self.paint_ledger
            .proves_property(self.proves_complete(), rule_index, property)
    }
}
