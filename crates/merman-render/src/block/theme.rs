use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use indexmap::IndexMap;
use merman_core::diagrams::block::BlockClassDefRenderModel;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemePaintKind,
    FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty,
    ResolvedThemeStyle, ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectPaintTerminalLedger, DirectStaticSelectorDomain,
    FamilyThemeEvidence, FamilyThemeResidualReason, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::BlockDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::BlockNodeShellKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockTypographyOutcome {
    Inactive,
    Typed,
    ConfigOwned,
}

/// Final Block typography shared by layout measurement, SVG CSS, and terminal evidence.
#[derive(Debug)]
pub(crate) struct BlockTypographyThemePlan {
    padding: f64,
    text_style: crate::text::TextStyle,
    evidence: FamilyThemeEvidence,
    outcome: BlockTypographyOutcome,
    unsupported_properties: BTreeSet<ThemeTypographyProperty>,
    typed_font_stack_requested: bool,
    typed_font_size_requested: bool,
    config_owns_font_stack: bool,
    config_owns_font_size: bool,
    terminal_receipt: OnceLock<BlockTypographyReceipt>,
}

#[derive(Debug, Default)]
pub(crate) struct BlockTypographyReceipt {
    css_matches: Option<bool>,
    has_visible_label: bool,
}

impl BlockTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
    ) -> Self {
        let settings =
            super::config::BlockConfigView::new(effective_config.as_value()).layout_settings();
        let padding = settings.padding;
        let mut text_style = settings.text_style;
        let Some(theme) = theme else {
            return Self {
                padding,
                text_style,
                evidence: FamilyThemeEvidence::default(),
                outcome: BlockTypographyOutcome::Inactive,
                unsupported_properties: BTreeSet::new(),
                typed_font_stack_requested: false,
                typed_font_size_requested: false,
                config_owns_font_stack: false,
                config_owns_font_size: false,
                terminal_receipt: OnceLock::new(),
            };
        };

        let mut typed_font_stack = false;
        let mut typed_font_size = false;
        let mut unsupported_properties = BTreeSet::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_stack = true;
                }
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_size = true;
                }
                FamilyThemeMechanism::BaseTypography(property)
                    if route.disposition() == FamilyThemeDisposition::Unsupported =>
                {
                    unsupported_properties.insert(property);
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let config_owns_font_stack =
            ["themeVariables.fontFamily", "fontFamily"]
                .into_iter()
                .any(|path| {
                    merman_core::__private::config_path_overrides_typed_default(
                        effective_config,
                        path,
                    )
                });
        let config_owns_font_size =
            ["themeVariables.fontSize", "fontSize"]
                .into_iter()
                .any(|path| {
                    merman_core::__private::config_path_overrides_typed_default(
                        effective_config,
                        path,
                    )
                });
        let typed_font_stack_applied = typed_font_stack && !config_owns_font_stack;
        let typed_font_size_applied = typed_font_size && !config_owns_font_size;
        if typed_font_stack_applied {
            text_style.font_family = Some(theme.typography().font_stack().as_css());
        }
        if typed_font_size_applied {
            text_style.font_size = f64::from(theme.typography().font_size_px()).max(1.0);
        }

        let requested_typed = typed_font_stack || typed_font_size;
        let applied_typed = typed_font_stack_applied || typed_font_size_applied;
        let outcome = if applied_typed {
            BlockTypographyOutcome::Typed
        } else if requested_typed {
            BlockTypographyOutcome::ConfigOwned
        } else {
            BlockTypographyOutcome::Inactive
        };
        Self {
            padding,
            text_style,
            evidence: FamilyThemeEvidence::from_theme(Some(theme)),
            outcome,
            unsupported_properties,
            typed_font_stack_requested: typed_font_stack,
            typed_font_size_requested: typed_font_size,
            config_owns_font_stack,
            config_owns_font_size,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) const fn text_style(&self) -> &crate::text::TextStyle {
        &self.text_style
    }

    pub(crate) const fn padding(&self) -> f64 {
        self.padding
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.text_style.font_family.as_deref().unwrap_or_default()
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.text_style.font_size
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<BlockTypographyReceipt> {
        (self.typed_font_stack_requested
            || self.typed_font_size_requested
            || !self.unsupported_properties.is_empty())
        .then(BlockTypographyReceipt::default)
    }

    pub(crate) fn observe_css(
        &self,
        receipt: Option<&mut BlockTypographyReceipt>,
        font_family_css: &str,
        font_size_css: &str,
    ) -> bool {
        let matches = !self.font_family_css().trim().is_empty()
            && font_family_css == self.font_family_css()
            && font_size_css.parse::<f64>().ok().is_some_and(|value| {
                value.is_finite() && (value - self.font_size_px()).abs() < 1e-6
            });
        if let Some(receipt) = receipt {
            receipt.css_matches = Some(matches && receipt.css_matches.is_none());
        }
        matches
    }

    pub(crate) fn record_terminal(&self, receipt: BlockTypographyReceipt) -> bool {
        receipt.proves_css() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let receipt = self.terminal_receipt.get();
        for property in &self.unsupported_properties {
            let key = FamilyThemeMechanismKey::Typography(*property);
            if receipt.is_some_and(|receipt| !receipt.has_visible_label()) {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
        }
        for (property, requested, config_owned) in [
            (
                ThemeTypographyProperty::FontStack,
                self.typed_font_stack_requested,
                self.config_owns_font_stack,
            ),
            (
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.config_owns_font_size,
            ),
        ] {
            if !requested {
                continue;
            }
            let key = FamilyThemeMechanismKey::Typography(property);
            match self.outcome {
                BlockTypographyOutcome::ConfigOwned if config_owned => {
                    evidence.mark_not_applicable(key);
                }
                BlockTypographyOutcome::Typed if config_owned => {
                    evidence.mark_not_applicable(key);
                }
                BlockTypographyOutcome::Typed => match receipt {
                    Some(receipt) if receipt.proves_css() && receipt.has_visible_label() => {
                        evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
                    }
                    Some(receipt) if receipt.proves_css() && !receipt.has_visible_label() => {
                        evidence.mark_not_applicable(key);
                    }
                    Some(_) | None => {
                        evidence
                            .mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
                    }
                },
                BlockTypographyOutcome::ConfigOwned | BlockTypographyOutcome::Inactive => {}
            }
        }
        evidence
    }
}

impl BlockTypographyReceipt {
    pub(crate) fn record_visible_label(&mut self, text: &str) {
        self.has_visible_label = self.has_visible_label || !text.trim().is_empty();
    }

    fn proves_css(&self) -> bool {
        self.css_matches == Some(true)
    }

    fn has_visible_label(&self) -> bool {
        self.has_visible_label
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NodeExpectation {
    fill: Option<DirectPaintExpectation>,
    stroke: Option<DirectPaintExpectation>,
    shells: Box<[BlockNodeShellKind]>,
}

/// Block node shell paint resolved once and shared by the SVG writer and terminal evidence.
#[derive(Debug)]
pub(crate) struct BlockNodePaintThemePlan {
    expectations: Option<Arc<[NodeExpectation]>>,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeMap<ResolvedStyleProperty, ThemeCapability>>,
    terminal_receipt: OnceLock<Option<BlockNodePaintThemeReceipt>>,
}

impl BlockNodePaintThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        layout: &BlockDiagramLayout,
        source_owned_fill: &[bool],
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline());
        };
        let mut expectations = terminal_domain(layout);

        let node_count = expectations.len();
        let mermaid_owns_fill = mermaid_owns_node_fill(effective_config);
        let mermaid_owns_stroke = mermaid_owns_node_stroke(effective_config);
        let has_ordinal_node_rules = theme.family_rules().any(|(_, rule)| {
            rule.target() == ThemeTarget::Node
                && matches!(rule.variant(), None | Some(ThemeVariant::Default))
                && rule.ordinal().is_some()
        });
        let static_style = if node_count == 0 || has_ordinal_node_rules {
            None
        } else {
            Some(theme.style_with_work_meter(
                ThemeTarget::Node,
                ThemeVariant::Default,
                None,
                work_meter,
            )?)
        };
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut expected_capabilities =
            BTreeMap::<(usize, ResolvedStyleProperty), ThemeCapability>::new();

        if has_ordinal_node_rules {
            for (node_index, expectation) in expectations.iter_mut().enumerate() {
                let style = theme.style_with_work_meter(
                    ThemeTarget::Node,
                    ThemeVariant::Default,
                    Some(node_index + 1),
                    work_meter,
                )?;
                observe_node_style(
                    theme,
                    &style,
                    mermaid_owns_fill,
                    mermaid_owns_stroke,
                    expectation,
                    &mut winner_properties,
                    &mut expected_capabilities,
                );
            }
        } else if let Some(style) = static_style.as_ref() {
            let mut static_expectation = NodeExpectation::default();
            observe_node_style(
                theme,
                style,
                mermaid_owns_fill,
                mermaid_owns_stroke,
                &mut static_expectation,
                &mut winner_properties,
                &mut expected_capabilities,
            );
            for expectation in &mut expectations {
                expectation.fill = static_expectation.fill.clone();
                expectation.stroke = static_expectation.stroke.clone();
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, NodeRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Node,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !selector_matches_any_node(selector, node_count) {
                        continue;
                    }
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

                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static {
                                variant: None | Some(ThemeVariant::Default),
                            },
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
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Node,
                } => {}
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Node,
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
            &[
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Node,
                    TerminalVariantDomain::uniform(node_count, ThemeVariant::Default),
                )
                .with_source_owned_fill(source_owned_fill),
                // Composite labels are node labels; frontmatter does not create a diagram title.
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Title,
                    TerminalVariantDomain::uniform(0, ThemeVariant::Default),
                ),
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::ClusterLabel,
                    TerminalVariantDomain::uniform(0, ThemeVariant::Default),
                ),
            ],
            work_meter,
        )?;

        let mut pending = BTreeMap::new();
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Node,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A mixed rule remains fail-closed until every winning facet has a terminal owner.
            } else if !observation.capabilities.is_empty() {
                pending.insert(key, observation.capabilities);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            expectations: Some(expectations.into()),
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline() -> Self {
        Self {
            expectations: None,
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn typed_stroke(
        &self,
        node_index: usize,
        source_owns_stroke: bool,
    ) -> Option<(usize, &str)> {
        self.typed_paint(
            node_index,
            source_owns_stroke,
            ResolvedStyleProperty::Stroke,
        )
    }

    pub(crate) fn typed_fill(
        &self,
        node_index: usize,
        source_owns_fill: bool,
    ) -> Option<(usize, &str)> {
        self.typed_paint(node_index, source_owns_fill, ResolvedStyleProperty::Fill)
    }

    fn typed_paint(
        &self,
        node_index: usize,
        source_owns: bool,
        property: ResolvedStyleProperty,
    ) -> Option<(usize, &str)> {
        (!source_owns)
            .then(|| self.expectations.as_ref()?.get(node_index)?.paint(property))
            .flatten()
            .map(|expected| (expected.rule_index(), expected.css()))
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<BlockNodePaintThemeReceipt> {
        self.expectations
            .as_ref()
            .map(|expectations| BlockNodePaintThemeReceipt::new(Arc::clone(expectations)))
    }

    pub(crate) fn record_terminal(&self, receipt: Option<BlockNodePaintThemeReceipt>) -> bool {
        let complete = match receipt.as_ref() {
            Some(receipt) => self.expectations.is_some() && receipt.proves_complete(),
            None => self.expectations.is_none(),
        };
        complete && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(Some(receipt)) = self.terminal_receipt.get() else {
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
        evidence
    }
}

impl NodeExpectation {
    fn paint(&self, property: ResolvedStyleProperty) -> Option<&DirectPaintExpectation> {
        match property {
            ResolvedStyleProperty::Fill => self.fill.as_ref(),
            ResolvedStyleProperty::Stroke => self.stroke.as_ref(),
            _ => None,
        }
    }
}

fn terminal_domain(layout: &BlockDiagramLayout) -> Vec<NodeExpectation> {
    let geometries = layout
        .shape_geometries
        .iter()
        .map(|geometry| (geometry.id.as_str(), &geometry.boundary))
        .collect::<BTreeMap<_, _>>();
    layout
        .nodes
        .iter()
        .map(|node| NodeExpectation {
            fill: None,
            stroke: None,
            shells: geometries
                .get(node.id.as_str())
                .map(|boundary| boundary.canonical_shell_kinds().into())
                .unwrap_or_default(),
        })
        .collect()
}

fn observe_node_style(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns_fill: bool,
    mermaid_owns_stroke: bool,
    expectation: &mut NodeExpectation,
    winner_properties: &mut BTreeSet<(usize, ResolvedStyleProperty)>,
    expected_capabilities: &mut BTreeMap<(usize, ResolvedStyleProperty), ThemeCapability>,
) {
    winner_properties.extend(
        style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property)),
    );
    if let Some(expected) = typed_fill_expectation(theme, style, mermaid_owns_fill) {
        expected_capabilities.insert(
            (expected.rule_index(), ResolvedStyleProperty::Fill),
            expected.capability(),
        );
        expectation.fill = Some(expected);
    }
    if let Some(expected) = typed_stroke_expectation(theme, style, mermaid_owns_stroke) {
        expected_capabilities.insert(
            (expected.rule_index(), ResolvedStyleProperty::Stroke),
            expected.capability(),
        );
        expectation.stroke = Some(expected);
    }
}

fn typed_stroke_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns_stroke: bool,
) -> Option<DirectPaintExpectation> {
    if mermaid_owns_stroke {
        return None;
    }
    resolve_direct_static_stroke(
        theme,
        style,
        &[ThemeTarget::Node],
        DirectStaticSelectorDomain::Default,
    )
    .map(DirectPaintExpectation::from_paint)
}

fn typed_fill_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns_fill: bool,
) -> Option<DirectPaintExpectation> {
    if mermaid_owns_fill {
        return None;
    }
    resolve_direct_static_fill(
        theme,
        style,
        &[ThemeTarget::Node],
        DirectStaticSelectorDomain::Default,
    )
    .map(DirectPaintExpectation::from_paint)
}

fn selector_matches_any_node(selector: FamilyThemeSelectorShape, node_count: usize) -> bool {
    match selector {
        FamilyThemeSelectorShape::Static {
            variant: None | Some(ThemeVariant::Default),
        } => node_count != 0,
        FamilyThemeSelectorShape::Ordinal {
            variant: None | Some(ThemeVariant::Default),
            ..
        } => selector.ordinal_domain_intersects_occurrence_count(node_count),
        FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => false,
    }
}

fn mermaid_owns_node_stroke(config: &merman_core::MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.nodeBorder")
}

fn mermaid_owns_node_fill(config: &merman_core::MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.mainBkg")
}

#[derive(Debug, Default)]
struct NodeRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeMap<ResolvedStyleProperty, ThemeCapability>,
}

/// Precomputed Mermaid source-style owners for Block node shell paint.
pub(crate) struct BlockNodePaintSourceOwnership {
    fill_classes: BTreeSet<String>,
    stroke_classes: BTreeSet<String>,
}

impl BlockNodePaintSourceOwnership {
    pub(crate) fn new(class_defs: &IndexMap<String, BlockClassDefRenderModel>) -> Self {
        let (fill_classes, stroke_classes) = classes_owning_node_paint(class_defs);
        Self {
            fill_classes,
            stroke_classes,
        }
    }

    pub(crate) fn owns_fill(&self, inline_owns_fill: bool, assigned_classes: &[String]) -> bool {
        source_owns_property(inline_owns_fill, assigned_classes, &self.fill_classes)
    }

    pub(crate) fn owns_stroke(
        &self,
        inline_owns_stroke: bool,
        assigned_classes: &[String],
    ) -> bool {
        source_owns_property(inline_owns_stroke, assigned_classes, &self.stroke_classes)
    }
}

fn classes_owning_node_paint(
    class_defs: &IndexMap<String, BlockClassDefRenderModel>,
) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut fill_classes = BTreeSet::new();
    let mut stroke_classes = BTreeSet::new();
    for class_def in class_defs.values() {
        for declaration in &class_def.styles {
            let Some(parsed) = crate::mermaid_style::parse_style_declaration(declaration) else {
                continue;
            };
            match parsed.property() {
                "fill" => {
                    fill_classes.insert(class_def.id.clone());
                }
                "stroke" => {
                    stroke_classes.insert(class_def.id.clone());
                }
                _ => {}
            }
        }
    }
    (fill_classes, stroke_classes)
}

fn source_owns_property(
    inline_owns_property: bool,
    assigned_classes: &[String],
    owning_classes: &BTreeSet<String>,
) -> bool {
    inline_owns_property
        || owning_classes.contains("default")
        || assigned_classes
            .iter()
            .any(|assigned| owning_classes.contains(assigned))
}

impl BlockNodePaintSourceOwnership {
    /// Computes source-owned fill for the same visible node order used by the Block writer.
    pub(crate) fn source_owned_fill_mask(
        &self,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        mermaid_owns_fill: bool,
    ) -> Vec<bool> {
        let mut sources = BTreeMap::<String, BlockNodeSourceProperties>::new();
        for node in &model.blocks_flat {
            let source = sources.entry(node.id.clone()).or_default();
            if !node.classes.is_empty() {
                source.classes = node.classes.clone();
            }
            if !node.styles.is_empty() {
                source.inline_owns_fill = node.styles.iter().any(|raw| {
                    crate::mermaid_style::parse_style_declaration(raw)
                        .is_some_and(|declaration| declaration.property() == "fill")
                });
            }
        }

        layout
            .nodes
            .iter()
            .map(|node| {
                let Some(source) = sources.get(&node.id) else {
                    return mermaid_owns_fill;
                };
                mermaid_owns_fill || self.owns_fill(source.inline_owns_fill, &source.classes)
            })
            .collect()
    }
}

#[derive(Default)]
struct BlockNodeSourceProperties {
    classes: Vec<String>,
    inline_owns_fill: bool,
}

/// Writer-owned proof that every semantic Block node reached every canonical SVG shell.
#[derive(Debug, Clone)]
pub(crate) struct BlockNodePaintThemeReceipt {
    expectations: Arc<[NodeExpectation]>,
    checkpointed_nodes: Vec<bool>,
    attributes_match: bool,
    paint_ledger: DirectPaintTerminalLedger,
}

impl BlockNodePaintThemeReceipt {
    fn new(expectations: impl Into<Arc<[NodeExpectation]>>) -> Self {
        let expectations = expectations.into();
        Self {
            checkpointed_nodes: vec![false; expectations.len()],
            expectations,
            attributes_match: true,
            paint_ledger: DirectPaintTerminalLedger::default(),
        }
    }

    pub(crate) fn record_checkpointed_node<'a>(
        &mut self,
        node_index: usize,
        source_owns_fill: bool,
        emitted_fill: Option<(usize, &str)>,
        source_owns_stroke: bool,
        emitted_stroke: Option<(usize, &str)>,
        shells: impl IntoIterator<Item = (BlockNodeShellKind, &'a str)>,
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
        let mut actual_shells = shells.into_iter();
        let mut shell_match = true;
        for expected_kind in expectation.shells.iter().copied() {
            let Some((actual_kind, style)) = actual_shells.next() else {
                shell_match = false;
                break;
            };
            shell_match &= expected_kind == actual_kind;
            let (actual_fill, actual_stroke) = terminal_paints(style);
            if let Some(expected) = expectation.fill.as_ref()
                && !source_owns_fill
            {
                shell_match &= actual_fill == Some(expected.css());
            }
            if let Some(expected) = expectation.stroke.as_ref()
                && !source_owns_stroke
            {
                shell_match &= actual_stroke == Some(expected.css());
            }
        }
        shell_match &= actual_shells.next().is_none();
        self.attributes_match &= shell_match;
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

    #[cfg(test)]
    fn proves_rule(&self, rule_index: usize) -> bool {
        self.paint_ledger
            .proves_rule(self.proves_complete(), rule_index)
    }

    fn proves_property(&self, rule_index: usize, property: ResolvedStyleProperty) -> bool {
        self.paint_ledger
            .proves_property(self.proves_complete(), rule_index, property)
    }
}

fn terminal_paints(style: &str) -> (Option<&str>, Option<&str>) {
    let mut fill = None;
    let mut stroke = None;
    for declaration in style
        .split(';')
        .filter_map(crate::mermaid_style::parse_style_declaration)
    {
        match declaration.property() {
            "fill" => fill = Some(declaration.value()),
            "stroke" => stroke = Some(declaration.value()),
            _ => {}
        }
    }
    (fill, stroke)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typography_css_checks_reject_mismatches_and_duplicate_emissions() {
        let plan =
            BlockTypographyThemePlan::resolve(None, &merman_core::MermaidConfig::empty_object());
        for (family, size, expected) in [
            (plan.font_family_css(), "16", true),
            ("wrong-font", "16", false),
            (plan.font_family_css(), "17", false),
            (plan.font_family_css(), "NaN", false),
            (plan.font_family_css(), "invalid", false),
        ] {
            assert_eq!(plan.observe_css(None, family, size), expected);
            let mut receipt = BlockTypographyReceipt::default();
            assert!(!receipt.proves_css());
            assert_eq!(plan.observe_css(Some(&mut receipt), family, size), expected);
            assert_eq!(receipt.proves_css(), expected);
            plan.observe_css(Some(&mut receipt), family, size);
            assert!(!receipt.proves_css());
        }
    }

    #[test]
    fn unthemed_typography_keeps_config_style_without_terminal_evidence() {
        let config = merman_core::MermaidConfig::from_value(serde_json::json!({
            "fontFamily": "Config Sans, Arial",
            "themeVariables": {"fontSize": "24px"},
            "block": {"padding": 12}
        }));
        let plan = BlockTypographyThemePlan::resolve(None, &config);
        assert_eq!(plan.font_family_css(), "Config Sans,Arial");
        assert_eq!(plan.font_size_px(), 24.0);
        assert_eq!(plan.padding(), 12.0);
        assert!(plan.begin_terminal_receipt().is_none());
        assert!(plan.observe_css(None, "Config Sans,Arial", "24"));
        assert!(plan.terminal_receipt.get().is_none());
    }

    #[test]
    fn unsupported_typography_still_observes_visible_terminals() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, ThemeTextStyle, TypographySpec,
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    crate::DiagramFamilyId::BLOCK,
                    ThemeTextStyle::default().with_font_weight(700).unwrap(),
                ),
            ))
            .unwrap();
        let resolved = theme.resolve(crate::DiagramFamilyId::BLOCK);
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontWeight);
        for (label, not_applicable) in [(" \n", true), ("Visible", false)] {
            let plan = BlockTypographyThemePlan::resolve(
                Some(&resolved),
                &merman_core::MermaidConfig::empty_object(),
            );
            let mut receipt = plan
                .begin_terminal_receipt()
                .expect("unsupported obligation");
            assert!(plan.observe_css(Some(&mut receipt), plan.font_family_css(), "16"));
            receipt.record_visible_label(label);
            assert!(plan.record_terminal(receipt));
            let evidence = plan.finish_evidence();
            assert_eq!(
                evidence.not_applicable_mechanisms().contains(&key),
                not_applicable
            );
            assert_eq!(evidence.residuals().is_empty(), not_applicable);
        }
    }

    #[test]
    fn unthemed_node_paint_plan_keeps_no_per_node_evidence() {
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "block\n  A[\"Alpha\"] --> B[\"Beta\"]\n",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let merman_core::RenderSemanticModel::Block(model) = parsed.model() else {
            panic!("expected a Block render model");
        };
        let config = &parsed.metadata().effective_config;
        let layout = crate::block::layout_block_diagram_typed(
            model,
            config.as_value(),
            &crate::text::DeterministicTextMeasurer::default(),
        )
        .unwrap();
        assert!(!layout.nodes.is_empty());
        let work_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = BlockNodePaintThemePlan::resolve(None, config, &layout, &[], &work_meter)
            .expect("resolve unthemed Block paint");
        assert!(plan.expectations.is_none());
        assert!(plan.pending.is_empty());
        assert!(plan.begin_terminal_receipt().is_none());
        assert!(plan.record_terminal(None));
        assert!(matches!(plan.terminal_receipt.get(), Some(None)));
        assert!(!plan.record_terminal(None));
    }

    fn paint(rule_index: usize, css: &str) -> DirectPaintExpectation {
        DirectPaintExpectation::new(
            rule_index,
            css,
            if css == "transparent" {
                ThemeCapability::TransparentPaint
            } else {
                ThemeCapability::SolidPaint
            },
        )
    }

    fn paint_expectation(shells: &[BlockNodeShellKind]) -> NodeExpectation {
        NodeExpectation {
            fill: Some(paint(3, "#654321")),
            stroke: Some(paint(3, "#123456")),
            shells: shells.into(),
        }
    }

    #[test]
    fn receipt_requires_every_real_shell_with_the_final_paints() {
        let mut rect =
            BlockNodePaintThemeReceipt::new(vec![paint_expectation(&[BlockNodeShellKind::Rect])]);
        rect.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Rect, "fill:#654321;stroke:#123456;")],
        );
        assert!(rect.proves_complete());
        assert!(rect.proves_rule(3));
        assert!(rect.proves_property(3, ResolvedStyleProperty::Fill));
        assert!(rect.proves_property(3, ResolvedStyleProperty::Stroke));

        let mut double_circle = BlockNodePaintThemeReceipt::new(vec![paint_expectation(&[
            BlockNodeShellKind::Circle,
            BlockNodeShellKind::Circle,
        ])]);
        double_circle.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
            ],
        );
        assert!(double_circle.proves_complete());
        assert!(double_circle.proves_rule(3));
    }

    #[test]
    fn receipt_rejects_missing_wrong_duplicate_and_unwritten_shells() {
        let expected = vec![paint_expectation(&[
            BlockNodeShellKind::Circle,
            BlockNodeShellKind::Circle,
        ])];

        let mut missing = BlockNodePaintThemeReceipt::new(expected.clone());
        missing.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;")],
        );
        assert!(!missing.proves_complete());

        let mut wrong = BlockNodePaintThemeReceipt::new(expected.clone());
        wrong.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Path, "fill:#654321;stroke:#123456;"),
                (BlockNodeShellKind::Circle, "fill:#abcdef;stroke:#123456;"),
            ],
        );
        assert!(!wrong.proves_complete());

        let mut duplicate = BlockNodePaintThemeReceipt::new(expected);
        duplicate.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
            ],
        );
        duplicate.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            false,
            Some((3, "#123456")),
            [
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
                (BlockNodeShellKind::Circle, "fill:#654321;stroke:#123456;"),
            ],
        );
        assert!(!duplicate.proves_complete());

        let unwritten =
            BlockNodePaintThemeReceipt::new(vec![paint_expectation(&[BlockNodeShellKind::Rect])]);
        assert!(!unwritten.proves_complete());
    }

    #[test]
    fn receipt_rejects_missing_or_wrong_fill_emission_even_when_terminal_style_matches() {
        let expectation = vec![paint_expectation(&[BlockNodeShellKind::Polygon])];
        let mut missing = BlockNodePaintThemeReceipt::new(expectation.clone());
        missing.record_checkpointed_node(
            0,
            false,
            None,
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Polygon, "fill:#654321;stroke:#123456;")],
        );
        assert!(!missing.proves_complete());

        let mut wrong = BlockNodePaintThemeReceipt::new(expectation);
        wrong.record_checkpointed_node(
            0,
            false,
            Some((3, "#abcdef")),
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Polygon, "fill:#654321;stroke:#123456;")],
        );
        assert!(!wrong.proves_complete());
    }

    #[test]
    fn source_owned_fill_is_independent_from_effective_typed_stroke() {
        let mut receipt = BlockNodePaintThemeReceipt::new(vec![paint_expectation(&[
            BlockNodeShellKind::Polygon,
        ])]);
        receipt.record_checkpointed_node(
            0,
            true,
            None,
            false,
            Some((3, "#123456")),
            [(BlockNodeShellKind::Polygon, "fill:#abcdef;stroke:#123456;")],
        );

        assert!(receipt.proves_complete());
        assert!(receipt.has_effective_rule(3));
        assert!(receipt.proves_rule(3));
        assert!(!receipt.proves_property(3, ResolvedStyleProperty::Fill));
        assert!(receipt.proves_property(3, ResolvedStyleProperty::Stroke));
    }

    #[test]
    fn source_owned_stroke_is_independent_from_effective_typed_fill() {
        let mut receipt =
            BlockNodePaintThemeReceipt::new(vec![paint_expectation(&[BlockNodeShellKind::Rect])]);
        receipt.record_checkpointed_node(
            0,
            false,
            Some((3, "#654321")),
            true,
            None,
            [(BlockNodeShellKind::Rect, "fill:#654321;stroke:#abcdef;")],
        );

        assert!(receipt.proves_complete());
        assert!(receipt.has_effective_rule(3));
        assert!(receipt.proves_rule(3));
        assert!(receipt.proves_property(3, ResolvedStyleProperty::Fill));
        assert!(!receipt.proves_property(3, ResolvedStyleProperty::Stroke));
    }

    #[test]
    fn source_ownership_tracks_fill_and_stroke_independently() {
        let mut class_defs = IndexMap::new();
        let mut accent = BlockClassDefRenderModel::default();
        accent.id = "accent".to_string();
        accent.styles = vec!["fill:#abcdef".to_string()];
        class_defs.insert("accent".to_string(), accent);

        let ownership = BlockNodePaintSourceOwnership::new(&class_defs);
        assert!(ownership.owns_fill(true, &[]));
        assert!(ownership.owns_stroke(true, &[]));
        assert!(ownership.owns_fill(false, &["accent".to_string()]));
        assert!(!ownership.owns_stroke(false, &["accent".to_string()]));
        assert!(!ownership.owns_fill(false, &["other".to_string()]));

        let mut default_class = BlockClassDefRenderModel::default();
        default_class.id = "default".to_string();
        default_class.styles = vec!["FILL:#fedcba".to_string(), "STROKE:#abcdef".to_string()];
        class_defs.insert("default".to_string(), default_class);
        let ownership = BlockNodePaintSourceOwnership::new(&class_defs);
        assert!(ownership.owns_fill(false, &[]));
        assert!(ownership.owns_stroke(false, &[]));
        assert!(ownership.owns_fill(false, &["other".to_string()]));
        assert!(ownership.owns_stroke(false, &["other".to_string()]));
    }
}
