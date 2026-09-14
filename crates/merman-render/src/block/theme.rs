use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use indexmap::IndexMap;
use merman_core::diagrams::block::BlockClassDefRenderModel;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
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
    pending: BTreeMap<(usize, ThemeTarget), ThemeCapability>,
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
                FamilyThemeMechanism::RuleFacet { target, .. }
                | FamilyThemeMechanism::OrdinalPalette { target }
                | FamilyThemeMechanism::EffectBinding { target, .. }
                    if matches!(target, ThemeTarget::NodeLabel | ThemeTarget::Text)
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
                    &[ThemeTarget::Text, ThemeTarget::NodeLabel],
                    DirectStaticSelectorDomain::Default,
                )
                .map(DirectPaintExpectation::from_paint)
            } else {
                None
            };
            if let Some(paint) = paint.as_ref()
                && let Some((_, origin)) =
                    style.winner_rule_properties().find(|(property, origin)| {
                        *property == ResolvedStyleProperty::Fill
                            && origin.rule_index() == paint.rule_index()
                    })
            {
                capabilities.insert((paint.rule_index(), origin.target()), paint.capability());
            }
            expectations.push(paint.map(|paint| BlockNodeLabelExpectation { paint, text }));
        }
        let mut observations = BTreeMap::<(usize, ThemeTarget), NodeRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            if !matches!(target, ThemeTarget::NodeLabel | ThemeTarget::Text) {
                continue;
            }
            let observation = observations.entry((rule_index, target)).or_default();
            if !winners.contains(&(rule_index, resolved_style_property_for_facet(facet))) {
                continue;
            }
            observation.applicable = true;
            match route.disposition() {
                FamilyThemeDisposition::TypedAdapter => {
                    if matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
                        if let Some(capability) = capabilities.get(&(rule_index, target)) {
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
        for ((index, target), observation) in observations {
            let key = FamilyThemeMechanismKey::Rule { index, target };
            if !observation.applicable {
                plan.evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                plan.evidence.mark_residual(key, reason);
            } else if !observation.incomplete
                && let Some(capability) = observation.capabilities.get(&ResolvedStyleProperty::Fill)
            {
                plan.pending.insert((index, target), *capability);
            }
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::NodeLabel,
                    TerminalVariantDomain::uniform(layout.nodes.len(), ThemeVariant::Default),
                )
                .with_source_owned_fill(&source_owned),
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Text,
                    TerminalVariantDomain::uniform(layout.nodes.len(), ThemeVariant::Default),
                )
                .with_source_owned_fill(&source_owned),
            ],
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
            for (&(index, target), &capability) in &self.pending {
                evidence.mark_applied_with_capabilities(
                    FamilyThemeMechanismKey::Rule { index, target },
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
    id: String,
    transform: String,
    composite: bool,
    rectangle_attributes: Option<[String; 6]>,
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
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline());
        };
        let mut expectations = terminal_domain(layout);
        let sources = super::resolve_block_node_sources(model);
        let ownership = BlockNodePaintSourceOwnership::new(&model.class_defs);
        let node_config_fill = mermaid_owns_node_fill(effective_config);
        let node_config_stroke = mermaid_owns_node_stroke(effective_config);
        let cluster_config_fill = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.clusterBkg",
        );
        let cluster_config_stroke = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.clusterBorder",
        );
        let has_ordinals = |target| {
            theme
                .family_rules()
                .any(|(_, rule)| rule.target() == target && rule.ordinal().is_some())
        };
        let node_ordinals = has_ordinals(ThemeTarget::Node);
        let cluster_ordinals = has_ordinals(ThemeTarget::Cluster);
        let node_style = theme.style_with_work_meter(
            ThemeTarget::Node,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let cluster_style = theme.style_with_work_meter(
            ThemeTarget::Cluster,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let mut winners = BTreeSet::new();
        let mut capabilities = BTreeMap::new();
        let mut node_owned_fill = Vec::with_capacity(expectations.len());
        let mut node_owned_stroke = Vec::with_capacity(expectations.len());
        let mut cluster_owned_fill = Vec::new();
        let mut cluster_owned_stroke = Vec::new();
        let mut cluster_index = 0;
        for (node_index, expectation) in expectations.iter_mut().enumerate() {
            work_meter.charge(1)?;
            let source = sources.get(expectation.id.as_str());
            let source_fill = source.is_some_and(|source| {
                ownership.owns_fill(
                    source
                        .styles
                        .iter()
                        .any(|raw| owns_label_color(raw, "fill")),
                    source.classes,
                )
            });
            let source_stroke = source.is_some_and(|source| {
                ownership.owns_stroke(
                    source
                        .styles
                        .iter()
                        .any(|raw| owns_label_color(raw, "stroke")),
                    source.classes,
                )
            });
            let owned_fill = source_fill || node_config_fill;
            let owned_stroke = source_stroke || node_config_stroke;
            node_owned_fill.push(owned_fill);
            node_owned_stroke.push(owned_stroke);
            let ordinal_node_style;
            let style = if node_ordinals {
                ordinal_node_style = theme.style_with_work_meter(
                    ThemeTarget::Node,
                    ThemeVariant::Default,
                    Some(node_index + 1),
                    work_meter,
                )?;
                &ordinal_node_style
            } else {
                &node_style
            };
            observe_shell_style(
                theme,
                style,
                ThemeTarget::Node,
                owned_fill,
                owned_stroke,
                expectation,
                &mut winners,
                &mut capabilities,
            );
            if expectation.composite {
                cluster_index += 1;
                // Node paint that actually reached this shell owns its property. Node
                // capability gaps do not erase its residual or block Cluster's fallback.
                let owned_fill = source_fill || cluster_config_fill || expectation.fill.is_some();
                let owned_stroke =
                    source_stroke || cluster_config_stroke || expectation.stroke.is_some();
                cluster_owned_fill.push(owned_fill);
                cluster_owned_stroke.push(owned_stroke);
                let ordinal_cluster_style;
                let style = if cluster_ordinals {
                    ordinal_cluster_style = theme.style_with_work_meter(
                        ThemeTarget::Cluster,
                        ThemeVariant::Default,
                        Some(cluster_index),
                        work_meter,
                    )?;
                    &ordinal_cluster_style
                } else {
                    &cluster_style
                };
                observe_shell_style(
                    theme,
                    style,
                    ThemeTarget::Cluster,
                    owned_fill,
                    owned_stroke,
                    expectation,
                    &mut winners,
                    &mut capabilities,
                );
            }
        }
        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<FamilyThemeMechanismKey, NodeRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            if !matches!(target, ThemeTarget::Node | ThemeTarget::Cluster) {
                continue;
            }
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };
            let observation = observations.entry(key).or_default();
            let property = resolved_style_property_for_facet(facet);
            if !winners.contains(&(rule_index, property)) {
                continue;
            }
            observation.applicable = true;
            match route.disposition() {
                FamilyThemeDisposition::TypedAdapter => {
                    if let Some(capability) = capabilities.get(&(rule_index, property)) {
                        observation.capabilities.insert(property, *capability);
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
        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Node,
                    TerminalVariantDomain::uniform(expectations.len(), ThemeVariant::Default),
                )
                .with_source_owned_fill(&node_owned_fill)
                .with_source_owned_stroke(&node_owned_stroke),
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Cluster,
                    TerminalVariantDomain::uniform(cluster_index, ThemeVariant::Default),
                )
                .with_source_owned_fill(&cluster_owned_fill)
                .with_source_owned_stroke(&cluster_owned_stroke),
                // Composite labels remain node labels, and frontmatter has no title terminal.
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
        for (key, observation) in observations {
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if !observation.incomplete && !observation.capabilities.is_empty() {
                pending.insert(key, observation.capabilities);
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

    pub(crate) fn typed_stroke(&self, node_index: usize) -> Option<&str> {
        self.typed_paint(node_index, ResolvedStyleProperty::Stroke)
    }

    pub(crate) fn typed_fill(&self, node_index: usize) -> Option<&str> {
        self.typed_paint(node_index, ResolvedStyleProperty::Fill)
    }

    fn typed_paint(&self, node_index: usize, property: ResolvedStyleProperty) -> Option<&str> {
        self.expectations
            .as_ref()?
            .get(node_index)?
            .paint(property)
            .map(DirectPaintExpectation::css)
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
            if properties
                .keys()
                .all(|property| receipt.proves_property(rule_index, *property))
            {
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
        .map(|geometry| (geometry.id.as_str(), geometry))
        .collect::<BTreeMap<_, _>>();
    layout
        .nodes
        .iter()
        .map(|node| {
            let geometry = geometries.get(node.id.as_str());
            let boundary = geometry.map(|geometry| &geometry.boundary);
            let rectangle_attributes = match boundary {
                Some(super::BlockShapeBoundary::Rectangle {
                    width,
                    height,
                    radius,
                    ..
                }) => Some(
                    [
                        -width / 2.0,
                        -height / 2.0,
                        *width,
                        *height,
                        *radius,
                        *radius,
                    ]
                    .map(|value| crate::number_format::canonical_number(value).to_string()),
                ),
                _ => None,
            };
            NodeExpectation {
                fill: None,
                stroke: None,
                shells: boundary
                    .map(|boundary| boundary.canonical_shell_kinds().into())
                    .unwrap_or_default(),
                id: node.id.clone(),
                transform: geometry
                    .map(|geometry| {
                        format!(
                            "translate({}, {})",
                            crate::number_format::canonical_number(geometry.allocated.x),
                            crate::number_format::canonical_number(geometry.allocated.y),
                        )
                    })
                    .unwrap_or_default(),
                composite: matches!(
                    boundary,
                    Some(super::BlockShapeBoundary::Rectangle {
                        kind: super::BlockRectangleKind::Composite,
                        ..
                    })
                ),
                rectangle_attributes,
            }
        })
        .collect()
}

fn observe_shell_style(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    target: ThemeTarget,
    owns_fill: bool,
    owns_stroke: bool,
    expectation: &mut NodeExpectation,
    winners: &mut BTreeSet<(usize, ResolvedStyleProperty)>,
    capabilities: &mut BTreeMap<(usize, ResolvedStyleProperty), ThemeCapability>,
) {
    winners.extend(
        style
            .winner_rule_properties()
            .filter_map(|(property, origin)| {
                (!(owns_fill && property == ResolvedStyleProperty::Fill)
                    && !(owns_stroke && property == ResolvedStyleProperty::Stroke))
                    .then_some((origin.rule_index(), property))
            }),
    );
    for (property, owned) in [
        (ResolvedStyleProperty::Fill, owns_fill),
        (ResolvedStyleProperty::Stroke, owns_stroke),
    ] {
        if owned {
            continue;
        }
        let paint = if property == ResolvedStyleProperty::Fill {
            resolve_direct_static_fill(theme, style, &[target], DirectStaticSelectorDomain::Default)
        } else {
            resolve_direct_static_stroke(
                theme,
                style,
                &[target],
                DirectStaticSelectorDomain::Default,
            )
        };
        if let Some(paint) = paint.map(DirectPaintExpectation::from_paint) {
            capabilities.insert((paint.rule_index(), property), paint.capability());
            if property == ResolvedStyleProperty::Fill {
                expectation.fill = Some(paint);
            } else {
                expectation.stroke = Some(paint);
            }
        }
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
struct BlockNodePaintSourceOwnership {
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
        || owning_classes.contains("node")
        || owning_classes.contains("flowchart-label")
        || (assigned_classes.is_empty() && owning_classes.contains("default"))
        || assigned_classes
            .iter()
            .any(|assigned| owning_classes.contains(assigned))
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

    pub(crate) fn observe_checkpointed_node(
        &mut self,
        node_index: usize,
        diagram_id: &str,
        fragment: &str,
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
        let expectation = &self.expectations[node_index];
        // The checkpoint stops after the shell, before the label. Close only the already
        // emitted node group to parse that fragment, never fabricate shell attributes.
        let closed = format!("{fragment}</g>");
        let Ok(document) = roxmltree::Document::parse(&closed) else {
            self.attributes_match = false;
            return;
        };
        let group = document.root_element();
        let id = if diagram_id.is_empty() {
            expectation.id.clone()
        } else {
            format!("{diagram_id}-{}", expectation.id)
        };
        let has_class = |node: roxmltree::Node<'_, '_>, class| {
            node.attribute("class")
                .unwrap_or("")
                .split_ascii_whitespace()
                .any(|candidate| candidate == class)
        };
        let mut matches = group.has_tag_name("g")
            && group.attribute("id") == Some(id.as_str())
            && has_class(group, "node")
            && group.attribute("transform") == Some(expectation.transform.as_str());
        let direct = group
            .children()
            .filter(roxmltree::Node::is_element)
            .collect::<Vec<_>>();
        let mut terminals = Vec::new();
        if direct.len() == 1 && direct[0].has_tag_name("g") {
            let wrapper = direct[0];
            let children = wrapper
                .children()
                .filter(roxmltree::Node::is_element)
                .collect::<Vec<_>>();
            if has_class(wrapper, "outer-path") {
                // RoughJS represents one canonical shell with a fill and outline path.
                matches &= expectation.shells.len() == 1
                    && children.len() == 2
                    && children.iter().all(|child| child.has_tag_name("path"));
                if let Some(kind) = expectation.shells.first().copied() {
                    terminals.extend(children.into_iter().map(|node| (kind, node)));
                }
            } else {
                let child_count = children.len();
                terminals.extend(
                    children
                        .into_iter()
                        .filter_map(|node| shell_kind(node).map(|kind| (kind, node))),
                );
                matches &=
                    terminals.len() == child_count && terminals.len() == expectation.shells.len();
                matches &= terminals
                    .iter()
                    .map(|(kind, _)| *kind)
                    .eq(expectation.shells.iter().copied());
            }
        } else {
            terminals.extend(
                direct
                    .iter()
                    .filter_map(|node| shell_kind(*node).map(|kind| (kind, *node))),
            );
            matches &= direct.len() == terminals.len()
                && terminals
                    .iter()
                    .map(|(kind, _)| *kind)
                    .eq(expectation.shells.iter().copied());
        }
        if expectation.composite {
            matches &= terminals.len() == 1
                && terminals.first().is_some_and(|(_, node)| {
                    node.has_tag_name("rect")
                        && has_class(*node, "composite")
                        && has_class(*node, "cluster")
                });
        }
        if let Some(attributes) = &expectation.rectangle_attributes {
            if let Some((_, rect)) = terminals
                .first()
                .filter(|(_, node)| node.has_tag_name("rect"))
            {
                matches &= ["x", "y", "width", "height", "rx", "ry"]
                    .into_iter()
                    .zip(attributes)
                    .all(|(name, expected)| rect.attribute(name) == Some(expected.as_str()));
            } else if expectation.composite {
                matches = false;
            }
        }
        for (_, terminal) in &terminals {
            let (fill, stroke) = terminal_paints(terminal.attribute("style").unwrap_or(""));
            if let Some(expected) = &expectation.fill {
                matches &= fill == Some(expected.css());
            }
            if let Some(expected) = &expectation.stroke {
                matches &= stroke == Some(expected.css());
            }
        }
        self.attributes_match &= matches && !terminals.is_empty();
        let actual_style = terminals
            .first()
            .map(|(_, node)| node.attribute("style").unwrap_or(""));
        let (fill, stroke) = terminal_paints(actual_style.unwrap_or(""));
        self.attributes_match &= self.paint_ledger.record(
            expectation.fill.as_ref(),
            false,
            expectation
                .fill
                .as_ref()
                .and_then(|expected| fill.map(|css| (expected.rule_index(), css))),
            ResolvedStyleProperty::Fill,
        );
        self.attributes_match &= self.paint_ledger.record(
            expectation.stroke.as_ref(),
            false,
            expectation
                .stroke
                .as_ref()
                .and_then(|expected| stroke.map(|css| (expected.rule_index(), css))),
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

fn shell_kind(node: roxmltree::Node<'_, '_>) -> Option<BlockNodeShellKind> {
    match node.tag_name().name() {
        "rect" => Some(BlockNodeShellKind::Rect),
        "circle" => Some(BlockNodeShellKind::Circle),
        "path" => Some(BlockNodeShellKind::Path),
        "polygon" => Some(BlockNodeShellKind::Polygon),
        _ => None,
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
    fn paint_plan_preserves_nested_source_ownership_across_later_fragments() {
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
            let plan = BlockNodePaintThemePlan::resolve(
                Some(&theme),
                &config,
                &model,
                &layout,
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
            let plan = BlockNodePaintThemePlan::resolve(
                Some(&theme),
                &config,
                &model,
                &layout,
                &work_meter,
            )
            .unwrap();
            let evidence = plan.finish_evidence();
            assert!(evidence.not_applicable_mechanisms().contains(&palette_key));
            assert!(evidence.residuals().is_empty());

            // A later non-empty list replaces the corresponding source list.
            model.blocks_flat.push(
                serde_json::from_value(serde_json::json!({
                    "id": "A", "type": "na", "styles": ["stroke:#00bb00"],
                    "classes": ["unowned"]
                }))
                .unwrap(),
            );
            let plan = BlockNodePaintThemePlan::resolve(
                Some(&theme),
                &config,
                &model,
                &layout,
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
        let plan = BlockNodePaintThemePlan::resolve(None, config, model, &layout, &work_meter)
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
            id: "node".to_string(),
            transform: "translate(10, 20)".to_string(),
            ..NodeExpectation::default()
        }
    }

    #[test]
    fn composite_receipt_reads_actual_shell_identity_paints_and_geometry() {
        let expectation = NodeExpectation {
            fill: Some(paint(3, "#654321")),
            stroke: Some(paint(4, "#123456")),
            shells: vec![BlockNodeShellKind::Rect].into(),
            id: "group".to_string(),
            transform: "translate(10, 20)".to_string(),
            composite: true,
            rectangle_attributes: Some(["-20", "-10", "40", "20", "0", "0"].map(str::to_string)),
        };
        let fragment = r##"<g class="node default" id="test-group" transform="translate(10, 20)"><rect class="basic cluster composite label-container" style="fill:#654321;stroke:#123456;" x="-20" y="-10" width="40" height="20" rx="0" ry="0"/>"##;
        let mut receipt = BlockNodePaintThemeReceipt::new(vec![expectation.clone()]);
        receipt.observe_checkpointed_node(0, "test", fragment);
        assert!(receipt.proves_complete());
        assert!(receipt.proves_property(3, ResolvedStyleProperty::Fill));
        assert!(receipt.proves_property(4, ResolvedStyleProperty::Stroke));
        for mutated in [
            fragment.replace("test-group", "test-other"),
            fragment.replace("translate(10, 20)", "translate(20, 10)"),
            fragment.replace("#654321", "#abcdef"),
            fragment.replace("#123456", "#abcdef"),
            fragment.replace("composite", "ordinary"),
            fragment.replace("width=\"40\"", "width=\"20\""),
            fragment.replace("<rect", "<circle"),
            fragment.split("<rect").next().unwrap().to_string(),
        ] {
            let mut receipt = BlockNodePaintThemeReceipt::new(vec![expectation.clone()]);
            receipt.observe_checkpointed_node(0, "test", &mutated);
            assert!(!receipt.proves_complete(), "{mutated}");
        }
        receipt.observe_checkpointed_node(0, "test", fragment);
        assert!(!receipt.proves_complete(), "duplicate shell checkpoint");
    }

    #[test]
    fn receipt_requires_every_real_shell_with_the_final_paints() {
        for (kinds, shells) in [
            (
                vec![BlockNodeShellKind::Rect],
                r##"<rect style="fill:#654321;stroke:#123456;"/>"##,
            ),
            (
                vec![BlockNodeShellKind::Circle, BlockNodeShellKind::Circle],
                r##"<g class="basic label-container"><circle style="fill:#654321;stroke:#123456;"/><circle style="fill:#654321;stroke:#123456;"/></g>"##,
            ),
            (
                vec![BlockNodeShellKind::Polygon],
                r##"<g class="basic label-container outer-path"><path style="fill:#654321;stroke:#123456;"/><path style="fill:#654321;stroke:#123456;"/></g>"##,
            ),
        ] {
            let mut receipt = BlockNodePaintThemeReceipt::new(vec![paint_expectation(&kinds)]);
            let fragment =
                format!(r#"<g class="node" id="test-node" transform="translate(10, 20)">{shells}"#);
            receipt.observe_checkpointed_node(0, "test", &fragment);
            assert!(receipt.proves_complete());
            assert!(receipt.proves_rule(3));
            assert!(receipt.proves_property(3, ResolvedStyleProperty::Fill));
            assert!(receipt.proves_property(3, ResolvedStyleProperty::Stroke));
        }
    }

    #[test]
    fn receipt_rejects_missing_wrong_duplicate_and_unwritten_shells() {
        let expectation =
            paint_expectation(&[BlockNodeShellKind::Circle, BlockNodeShellKind::Circle]);
        let fragment = r##"<g class="node" id="test-node" transform="translate(10, 20)"><g class="basic label-container"><circle style="fill:#654321;stroke:#123456;"/><circle style="fill:#654321;stroke:#123456;"/></g>"##;
        for mutated in [
            fragment.replacen(r##"<circle style="fill:#654321;stroke:#123456;"/>"##, "", 1),
            fragment.replacen("<circle", "<path", 1),
            fragment.replacen("fill:#654321;", "", 1),
            fragment.replacen("fill:#654321;", "fill:#abcdef;", 1),
            fragment.replace("translate(10, 20)", "translate(20, 10)"),
        ] {
            let mut receipt = BlockNodePaintThemeReceipt::new(vec![expectation.clone()]);
            receipt.observe_checkpointed_node(0, "test", &mutated);
            assert!(!receipt.proves_complete(), "{mutated}");
        }
        let mut duplicate = BlockNodePaintThemeReceipt::new(vec![expectation.clone()]);
        duplicate.observe_checkpointed_node(0, "test", fragment);
        duplicate.observe_checkpointed_node(0, "test", fragment);
        assert!(!duplicate.proves_complete());
        assert!(!BlockNodePaintThemeReceipt::new(vec![expectation]).proves_complete());
    }

    #[test]
    fn source_owned_fill_is_independent_from_effective_typed_stroke() {
        let mut expectation = paint_expectation(&[BlockNodeShellKind::Polygon]);
        expectation.fill = None;
        let mut receipt = BlockNodePaintThemeReceipt::new(vec![expectation]);
        receipt.observe_checkpointed_node(0, "test",
            r##"<g class="node" id="test-node" transform="translate(10, 20)"><polygon style="fill:#abcdef;stroke:#123456;"/>"##);
        assert!(receipt.proves_complete());
        assert!(receipt.has_effective_rule(3));
        assert!(receipt.proves_rule(3));
        assert!(!receipt.proves_property(3, ResolvedStyleProperty::Fill));
        assert!(receipt.proves_property(3, ResolvedStyleProperty::Stroke));
    }

    #[test]
    fn source_owned_stroke_is_independent_from_effective_typed_fill() {
        let mut expectation = paint_expectation(&[BlockNodeShellKind::Rect]);
        expectation.stroke = None;
        let mut receipt = BlockNodePaintThemeReceipt::new(vec![expectation]);
        receipt.observe_checkpointed_node(0, "test",
            r##"<g class="node" id="test-node" transform="translate(10, 20)"><rect style="fill:#654321;stroke:#abcdef;"/>"##);
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
        assert!(!ownership.owns_fill(false, &["other".to_string()]));
        assert!(!ownership.owns_stroke(false, &["other".to_string()]));
        assert!(ownership.owns_fill(false, &["default".to_string()]));
        assert!(ownership.owns_stroke(false, &["default".to_string()]));
    }
}
