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

/// Block edge-label background owner; it shares the proven receipt implementation
/// with Flowchart while remaining a family-local plan.
#[derive(Debug)]
pub(crate) struct BlockLabelBackgroundPlan(crate::flowchart::FlowchartLabelBackgroundPlan);

impl BlockLabelBackgroundPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        Ok(Self(
            crate::flowchart::FlowchartLabelBackgroundPlan::resolve(theme, config, work)?,
        ))
    }

    pub(crate) fn requested(&self) -> bool {
        self.0.requested()
    }

    pub(crate) fn color<'a>(&'a self, configured: &'a str) -> &'a str {
        self.0.color(configured)
    }

    pub(crate) fn record_stylesheet(&self) {
        self.0.record_stylesheet();
    }

    pub(crate) fn record_terminal(
        &self,
        has_area: bool,
        background_emitted: bool,
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        self.0.record_terminal(has_area, background_emitted, work)
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        self.0.finish_evidence()
    }
}

/// Per-node label paint, resolved against source ownership and verified after SVG emission.
#[derive(Debug)]
pub(crate) struct BlockNodeLabelPaintPlan {
    expectations: Arc<[Option<BlockNodeLabelExpectation>]>,
    html_labels: bool,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<usize, ThemeCapability>,
    terminal: OnceLock<BlockNodeLabelPaintReceipt>,
}

#[derive(Debug)]
struct BlockNodeLabelExpectation {
    paint: DirectPaintExpectation,
    text: String,
}

#[derive(Debug)]
pub(crate) struct BlockNodeLabelPaintReceipt {
    seen: Vec<bool>,
    valid: bool,
}

impl BlockNodeLabelPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let html_labels = crate::config::config_effective_html_labels(config.as_value());
        let mut plan = Self {
            expectations: Arc::from([]),
            html_labels,
            evidence: FamilyThemeEvidence::from_theme(theme),
            pending: BTreeMap::new(),
            terminal: OnceLock::new(),
        };
        let Some(theme) = theme else { return Ok(plan) };
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::NodeLabel,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::NodeLabel
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::NodeLabel,
                    ..
                }
            )
        }) {
            return Ok(plan);
        }
        let config_owned = [
            "themeVariables.nodeTextColor",
            "themeVariables.primaryTextColor",
            "themeVariables.textColor",
        ]
        .into_iter()
        .any(|path| merman_core::__private::config_path_overrides_typed_default(config, path));
        let has_ordinal = theme.family_rules().any(|(_, rule)| {
            matches!(rule.target(), ThemeTarget::NodeLabel | ThemeTarget::Text)
                && rule.ordinal().is_some()
        });
        let static_style = theme.text_style_with_work_meter(
            ThemeTarget::NodeLabel,
            ThemeVariant::Default,
            None,
            work,
        )?;
        let sources = super::resolve_block_node_sources(model);
        let color_classes = model
            .class_defs
            .values()
            .filter(|class| {
                class
                    .styles
                    .iter()
                    .any(|raw| owns_label_color(raw, "color"))
                    || (!html_labels
                        && class
                            .text_styles
                            .iter()
                            .any(|raw| owns_label_color(raw, "fill")))
            })
            .map(|class| class.id.as_str())
            .collect::<BTreeSet<_>>();
        let mut winners = BTreeSet::new();
        let mut capabilities = BTreeMap::new();
        let mut source_owned = Vec::with_capacity(layout.nodes.len());
        let mut expectations = Vec::with_capacity(layout.nodes.len());
        for (index, node) in layout.nodes.iter().enumerate() {
            work.charge(1)?;
            let source = sources.get(node.id.as_str());
            let owned = config_owned
                || source.is_some_and(|source| {
                    source
                        .styles
                        .iter()
                        .any(|raw| owns_label_color(raw, "color"))
                        || if source.classes.is_empty() {
                            color_classes.contains("default")
                        } else {
                            source
                                .classes
                                .iter()
                                .any(|class| color_classes.contains(class.as_str()))
                        }
                });
            let label = source
                .map(|source| super::decode_block_label_html(source.label))
                .unwrap_or_default();
            let text = if html_labels {
                label
            } else {
                crate::flowchart::flowchart_label_plain_text_for_layout(&label, "text", false)
            };
            let visible = !text.trim().is_empty()
                && node.label_width.unwrap_or(0.0) > 0.0
                && node.label_height.unwrap_or(0.0) > 0.0;
            source_owned.push(owned || !visible);
            let dynamic_style;
            let style = if has_ordinal {
                dynamic_style = theme.text_style_with_work_meter(
                    ThemeTarget::NodeLabel,
                    ThemeVariant::Default,
                    Some(index + 1),
                    work,
                )?;
                &dynamic_style
            } else {
                &static_style
            };
            if visible {
                winners.extend(
                    style
                        .winner_rule_properties()
                        .filter_map(|(property, origin)| {
                            (!(owned && property == ResolvedStyleProperty::Fill))
                                .then_some((origin.rule_index(), property))
                        }),
                );
            }
            let paint = if visible && !owned {
                resolve_direct_static_fill(
                    theme,
                    style,
                    &[ThemeTarget::NodeLabel],
                    DirectStaticSelectorDomain::Default,
                )
                .map(DirectPaintExpectation::from_paint)
            } else {
                None
            };
            if let Some(paint) = paint.as_ref() {
                capabilities.insert(paint.rule_index(), paint.capability());
            }
            expectations.push(paint.map(|paint| BlockNodeLabelExpectation { paint, text }));
        }
        let mut observations = BTreeMap::<usize, NodeRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::NodeLabel,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            let observation = observations.entry(rule_index).or_default();
            if !winners.contains(&(rule_index, resolved_style_property_for_facet(facet))) {
                continue;
            }
            observation.applicable = true;
            match route.disposition() {
                FamilyThemeDisposition::TypedAdapter => {
                    if matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
                        if let Some(capability) = capabilities.get(&rule_index) {
                            observation
                                .capabilities
                                .insert(ResolvedStyleProperty::Fill, *capability);
                        } else {
                            observation.incomplete = true;
                        }
                    } else {
                        observation.incomplete = true;
                    }
                }
                FamilyThemeDisposition::Unsupported => {
                    observation
                        .residual
                        .get_or_insert(unsupported_residual_for_facet(facet));
                }
                FamilyThemeDisposition::LegacyCompatibility => {
                    observation.incomplete = true;
                }
            }
        }
        for (index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::NodeLabel,
            };
            if !observation.applicable {
                plan.evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                plan.evidence.mark_residual(key, reason);
            } else if !observation.incomplete
                && let Some(capability) = observation.capabilities.get(&ResolvedStyleProperty::Fill)
            {
                plan.pending.insert(index, *capability);
            }
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::NodeLabel,
                TerminalVariantDomain::uniform(layout.nodes.len(), ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned)],
            work,
        )?;
        plan.expectations = expectations.into();
        Ok(plan)
    }

    pub(crate) fn color(&self, node_index: usize) -> Option<&str> {
        self.expectations
            .get(node_index)?
            .as_ref()
            .map(|expected| expected.paint.css())
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<BlockNodeLabelPaintReceipt> {
        self.expectations
            .iter()
            .any(Option::is_some)
            .then(|| BlockNodeLabelPaintReceipt {
                seen: vec![false; self.expectations.len()],
                valid: true,
            })
    }

    /// Inspect the checkpointed label fragment, never a separately recomputed CSS value.
    pub(crate) fn observe_label(
        &self,
        receipt: &mut BlockNodeLabelPaintReceipt,
        node_index: usize,
        fragment: &str,
    ) {
        let Some(seen) = receipt.seen.get_mut(node_index) else {
            receipt.valid = false;
            return;
        };
        receipt.valid &= !*seen;
        *seen = true;
        let Some(expected) = self.expectations[node_index].as_ref() else {
            return;
        };
        receipt.valid &= self.label_matches(expected, fragment);
    }

    fn label_matches(&self, expected: &BlockNodeLabelExpectation, fragment: &str) -> bool {
        let Ok(document) = roxmltree::Document::parse(fragment) else {
            return false;
        };
        let mut terminals = document
            .descendants()
            .filter(|node| node.has_tag_name(if self.html_labels { "p" } else { "text" }));
        let Some(terminal) = terminals.next() else {
            return false;
        };
        if terminals.next().is_some() {
            return false;
        }
        let property = if self.html_labels { "color" } else { "fill" };
        let actual_paint = terminal
            .attribute("style")
            .unwrap_or("")
            .split(';')
            .filter_map(|raw| raw.split_once(':'))
            .filter(|(key, _)| key.trim() == property)
            .map(|(_, value)| value.trim())
            .next_back();
        if actual_paint != Some(expected.paint.css()) {
            return false;
        }
        if self.html_labels
            && !document.descendants().any(|node| {
                node.has_tag_name("foreignObject")
                    && ["width", "height"].into_iter().all(|name| {
                        node.attribute(name)
                            .and_then(|value| value.parse::<f64>().ok())
                            .is_some_and(|value| value.is_finite() && value > 0.0)
                    })
            })
        {
            return false;
        }
        terminal
            .descendants()
            .filter(|node| node.is_text())
            .flat_map(|node| node.text().unwrap_or("").chars())
            .filter(|ch| !ch.is_whitespace())
            .eq(expected.text.chars().filter(|ch| !ch.is_whitespace()))
    }

    pub(crate) fn record_terminal(&self, receipt: Option<BlockNodeLabelPaintReceipt>) -> bool {
        let Some(receipt) = receipt else {
            return self.expectations.iter().all(Option::is_none);
        };
        let complete = receipt.valid
            && self
                .expectations
                .iter()
                .enumerate()
                .all(|(index, expected)| {
                    expected.is_none() || receipt.seen.get(index) == Some(&true)
                });
        complete && self.terminal.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if self.terminal.get().is_some() {
            for (&index, &capability) in &self.pending {
                evidence.mark_applied_with_capabilities(
                    FamilyThemeMechanismKey::Rule {
                        index,
                        target: ThemeTarget::NodeLabel,
                    },
                    [capability],
                );
            }
        }
        evidence
    }
}

fn owns_label_color(raw: &str, property: &str) -> bool {
    crate::mermaid_style::parse_style_declaration(raw)
        .is_some_and(|declaration| declaration.property() == property)
}

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
        let sources = super::resolve_block_node_sources(model);

        layout
            .nodes
            .iter()
            .map(|node| {
                let Some(source) = sources.get(node.id.as_str()) else {
                    return mermaid_owns_fill;
                };
                let inline_owns_fill = source.styles.iter().any(|raw| {
                    crate::mermaid_style::parse_style_declaration(raw)
                        .is_some_and(|declaration| declaration.property() == "fill")
                });
                mermaid_owns_fill || self.owns_fill(inline_owns_fill, source.classes)
            })
            .collect()
    }
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

    fn node_label_receipt_plan(html_labels: bool) -> BlockNodeLabelPaintPlan {
        BlockNodeLabelPaintPlan {
            expectations: ["Alpha", "Beta"]
                .into_iter()
                .map(|text| {
                    Some(BlockNodeLabelExpectation {
                        paint: DirectPaintExpectation::new(
                            0,
                            "#123456",
                            ThemeCapability::SolidPaint,
                        ),
                        text: text.to_owned(),
                    })
                })
                .collect::<Vec<_>>()
                .into(),
            html_labels,
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            terminal: OnceLock::new(),
        }
    }

    #[test]
    fn node_label_receipt_requires_each_checkpointed_terminal_exactly_once() {
        for html in [false, true] {
            let fragments = ["Alpha", "Beta"].map(|text| if html {
                format!(r##"<g><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><span><p style="color:#123456;">{text}</p></span></div></foreignObject></g>"##)
            } else {
                format!(r##"<g><text style="fill:#123456;"><tspan>{text}</tspan></text></g>"##)
            });
            for mutation in [
                "none",
                "missing",
                "duplicate",
                "paint",
                "content",
                "identity",
                "zero-area",
            ] {
                if !html && mutation == "zero-area" {
                    continue;
                }
                let plan = node_label_receipt_plan(html);
                let mut receipt = plan.begin_terminal_receipt().unwrap();
                let first = match mutation {
                    "paint" => fragments[0].replace("#123456", "#abcdef"),
                    "content" => fragments[0].replace("Alpha", "Wrong"),
                    "zero-area" => fragments[0].replace("width=\"40\"", "width=\"0\""),
                    _ => fragments[0].clone(),
                };
                let index = if mutation == "identity" { 1 } else { 0 };
                plan.observe_label(&mut receipt, index, &first);
                if mutation == "duplicate" {
                    plan.observe_label(&mut receipt, 0, &fragments[0]);
                }
                if mutation != "missing" {
                    plan.observe_label(&mut receipt, 1, &fragments[1]);
                }
                assert_eq!(
                    plan.record_terminal(Some(receipt)),
                    mutation == "none",
                    "{html}: {mutation}"
                );
                if mutation == "none" {
                    let mut duplicate = plan.begin_terminal_receipt().unwrap();
                    plan.observe_label(&mut duplicate, 0, &fragments[0]);
                    plan.observe_label(&mut duplicate, 1, &fragments[1]);
                    assert!(
                        !plan.record_terminal(Some(duplicate)),
                        "a finalized plan cannot be resealed"
                    );
                }
            }
        }
    }

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
    fn explicit_default_font_size_remains_a_typed_request() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, ThemeTextStyle, TypographySpec,
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(
                    crate::DiagramFamilyId::BLOCK,
                    ThemeTextStyle::default().with_font_size_px(16.0).unwrap(),
                ),
            ))
            .unwrap();
        let resolved = theme.resolve(crate::DiagramFamilyId::BLOCK);
        let plan = BlockTypographyThemePlan::resolve(
            Some(&resolved),
            &merman_core::MermaidConfig::empty_object(),
        );
        assert!(plan.typed_font_size_requested);
        assert_eq!(plan.font_size_px(), 16.0);
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
    fn source_fill_mask_includes_nested_nodes_and_later_source_fragments() {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue, ThemeRuleSet,
        };
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_ordinal_palette(
                    ThemeTarget::Node,
                    OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap(),
                ),
            ))
            .unwrap();
        let theme = theme.resolve(crate::DiagramFamilyId::BLOCK);
        let config = merman_core::MermaidConfig::empty_object();
        let work_meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let palette_key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Node,
        };
        for class_owned in [false, true] {
            let mut value = serde_json::json!({
                "blocksFlat": [{
                    "id": "root", "type": "composite", "columns": 1,
                    "children": [{
                        "id": "A", "type": "square", "label": "Alpha",
                        "styles": ["fill:#bb0000"], "classes": []
                    }]
                }],
                "classes": {}
            });
            if class_owned {
                value["blocksFlat"][0]["children"][0]["styles"] = serde_json::json!([]);
                value["blocksFlat"][0]["children"][0]["classes"] = serde_json::json!(["brand"]);
                value["classes"] = serde_json::json!({"brand": {
                    "id": "brand", "styles": ["fill:#bb0000"]
                }});
            }
            let mut model: merman_core::diagrams::block::BlockDiagramRenderModel =
                serde_json::from_value(value).unwrap();
            let layout = crate::block::layout_block_diagram_typed(
                &model,
                &serde_json::json!({}),
                &crate::text::DeterministicTextMeasurer::default(),
            )
            .unwrap();
            assert_eq!(layout.nodes.len(), 1);
            let owners = BlockNodePaintSourceOwnership::new(&model.class_defs);
            assert_eq!(
                owners.source_owned_fill_mask(&model, &layout, false),
                [true]
            );

            let plan = BlockNodePaintThemePlan::resolve(
                Some(&theme),
                &config,
                &layout,
                &owners.source_owned_fill_mask(&model, &layout, false),
                &work_meter,
            )
            .unwrap();
            let evidence = plan.finish_evidence();
            assert!(evidence.not_applicable_mechanisms().contains(&palette_key));
            assert!(evidence.residuals().is_empty());

            // Empty fragments preserve the nested source, just as the terminal writer does.
            model.blocks_flat.push(
                serde_json::from_value(serde_json::json!({
                    "id": "A", "type": "na"
                }))
                .unwrap(),
            );
            assert_eq!(
                owners.source_owned_fill_mask(&model, &layout, false),
                [true]
            );

            // A later non-empty list replaces the corresponding source list.
            model.blocks_flat.push(
                serde_json::from_value(serde_json::json!({
                    "id": "A", "type": "na", "styles": ["stroke:#00bb00"],
                    "classes": ["unowned"]
                }))
                .unwrap(),
            );
            assert_eq!(
                owners.source_owned_fill_mask(&model, &layout, false),
                [false]
            );
            let plan = BlockNodePaintThemePlan::resolve(
                Some(&theme),
                &config,
                &layout,
                &owners.source_owned_fill_mask(&model, &layout, false),
                &work_meter,
            )
            .unwrap();
            let evidence = plan.finish_evidence();
            assert!(!evidence.not_applicable_mechanisms().contains(&palette_key));
            assert!(!evidence.residuals().is_empty());
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
