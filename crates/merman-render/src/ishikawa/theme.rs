use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme,
    ResolvedStyleProperty, ResolvedThemeStyle, Specified, ThemeCapability, ThemeTarget,
    ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackPlan, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::{IshikawaBranchLayout, IshikawaDiagramLayout, IshikawaTextLayout};
use crate::resources::{OperationWorkError, OperationWorkMeter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IshikawaTypographyTerminal {
    Head,
    InheritedBaseSize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedFill {
    rule_index: usize,
    css: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextExpectation {
    class_name: Box<str>,
    typography_terminal: IshikawaTypographyTerminal,
    fill: Option<ExpectedFill>,
}

/// Final Ishikawa text styling shared by terminal SVG emission and family evidence.
///
/// Upstream deterministic spacing and wrapping consume root `fontSize`. The base CSS size remains
/// a separate terminal property, so this plan intentionally does not feed typed CSS typography
/// back into layout.
#[derive(Debug)]
pub(crate) struct IshikawaTextThemePlan {
    expectations: Box<[TextExpectation]>,
    inherited_font_stack: InheritedFontStackPlan,
    font_size_css: Box<str>,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    evidence: FamilyThemeEvidence,
    pending: BTreeMap<FamilyThemeMechanismKey, BTreeSet<ThemeCapability>>,
    terminal_receipt: OnceLock<IshikawaTextThemeReceipt>,
}

impl IshikawaTextThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        layout: &IshikawaDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut expectations = terminal_domain(layout);
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let typed_font_size_requested = theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    && route.disposition() == FamilyThemeDisposition::TypedAdapter
            })
        });
        let config_owns_font_size = typed_font_size_requested
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.fontSize",
            );
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let font_size_css = match theme {
            Some(theme) if typed_font_size_active => {
                format!("{}px", theme.typography().font_size_px())
            }
            _ => crate::ishikawa::IshikawaConfigView::new(effective_config.as_value())
                .stylesheet_font_size_css()
                .unwrap_or_else(|| format!("{}px", layout.font_size)),
        }
        .into_boxed_str();
        let Some(theme) = theme else {
            return Ok(Self::baseline_from_expectations(
                expectations,
                inherited_font_stack,
                font_size_css,
            ));
        };

        let text_count = expectations.len();
        let mermaid_owns_fill = mermaid_owns_text_fill(effective_config);
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut expected_capabilities =
            BTreeMap::<(usize, ResolvedStyleProperty), ThemeCapability>::new();

        if text_count != 0 {
            let style = theme.style_with_work_meter(
                ThemeTarget::Text,
                ThemeVariant::Default,
                None,
                work_meter,
            )?;
            for expectation in &mut expectations {
                observe_text_style(
                    theme,
                    &style,
                    mermaid_owns_fill,
                    expectation,
                    &mut winner_properties,
                    &mut expected_capabilities,
                );
            }
        }

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, TextRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Text,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !selector_matches_text_domain(selector, text_count) {
                        continue;
                    }
                    let property = resolved_style_property_for_facet(facet);
                    let conservative_unsupported = route.disposition()
                        == FamilyThemeDisposition::Unsupported
                        && matches!(
                            selector,
                            FamilyThemeSelectorShape::Ordinal { .. }
                                | FamilyThemeSelectorShape::Static { variant: Some(_) }
                        );
                    if !winner_properties.contains(&(rule_index, property))
                        && !conservative_unsupported
                    {
                        continue;
                    }

                    observation.applicable = true;
                    if mermaid_owns_fill && matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
                        continue;
                    }
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some(capability) = expected_capabilities
                                .get(&(rule_index, ResolvedStyleProperty::Fill))
                            {
                                observation.capabilities.insert(*capability);
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
                    target: ThemeTarget::Text,
                } => {
                    // Reconciled below from each visible text terminal's final fill winner.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Text,
                    ..
                } => {
                    // Reconciled below from each visible text terminal's final style.
                }
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Title,
                    ..
                }
                | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Title,
                }
                | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Title,
                    ..
                } => {
                    // Ishikawa has no visual title terminal. Root accessibility metadata is not
                    // the semantic Title styling target, so every Title mechanism has zero
                    // occurrences regardless of its matrix disposition.
                    evidence.mark_not_applicable(theme.family_mechanism_key(route));
                }
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
                ThemeTarget::Text,
                TerminalVariantDomain::uniform(text_count, ThemeVariant::Default),
            )],
            work_meter,
        )?;

        let mut pending = BTreeMap::new();
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Text,
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
            expectations: expectations.into_boxed_slice(),
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested,
            typed_font_size_active,
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    fn baseline_from_expectations(
        expectations: Vec<TextExpectation>,
        inherited_font_stack: InheritedFontStackPlan,
        font_size_css: Box<str>,
    ) -> Self {
        Self {
            expectations: expectations.into_boxed_slice(),
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested: false,
            typed_font_size_active: false,
            evidence: FamilyThemeEvidence::default(),
            pending: BTreeMap::new(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn typed_fill(&self, terminal_index: usize) -> Option<(usize, &str)> {
        self.expectations
            .get(terminal_index)?
            .fill
            .as_ref()
            .map(|fill| (fill.rule_index, fill.css.as_ref()))
    }

    pub(crate) fn begin_terminal_receipt(&self) -> IshikawaTextThemeReceipt {
        IshikawaTextThemeReceipt::new(
            self.expectations.clone(),
            self.inherited_font_stack.font_family_css(),
            &self.font_size_css,
        )
    }

    pub(crate) fn record_terminal(&self, receipt: IshikawaTextThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            self.inherited_font_stack
                .mark_unsupported_typography_evidence(&mut evidence, !self.expectations.is_empty());
            if self.typography_requested() {
                for (property, requested) in [
                    (
                        ThemeTypographyProperty::FontStack,
                        self.inherited_font_stack.typed_font_stack_requested(),
                    ),
                    (
                        ThemeTypographyProperty::FontSize,
                        self.typed_font_size_requested,
                    ),
                ] {
                    if requested {
                        evidence.mark_residual(
                            FamilyThemeMechanismKey::Typography(property),
                            FamilyThemeResidualReason::UnsupportedTypography,
                        );
                    }
                }
            }
            return evidence;
        };
        for (key, capabilities) in &self.pending {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography(_)
                | FamilyThemeMechanismKey::OrdinalPalette { .. }
                | FamilyThemeMechanismKey::EffectBinding { .. } => continue,
            };
            if receipt.proves_rule(rule_index) {
                evidence.mark_applied_with_capabilities(key.clone(), capabilities.iter().copied());
            } else if !receipt.has_effective_rule(rule_index) {
                evidence.mark_not_applicable(key.clone());
            }
        }
        self.finish_typography_evidence(&mut evidence);
        evidence
    }

    fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested() || self.typed_font_size_requested
    }

    fn finish_typography_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if !self.typography_requested() {
            return;
        }
        let font_size_has_terminal = self.expectations.iter().any(|expectation| {
            expectation.typography_terminal == IshikawaTypographyTerminal::InheritedBaseSize
        });
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(evidence, !self.expectations.is_empty());
        for (property, requested, applied) in [
            (
                ThemeTypographyProperty::FontStack,
                self.inherited_font_stack.typed_font_stack_requested(),
                self.inherited_font_stack.typed_font_stack_active(),
            ),
            (
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.typed_font_size_active && font_size_has_terminal,
            ),
        ] {
            if !requested {
                continue;
            }
            let key = FamilyThemeMechanismKey::Typography(property);
            if self.expectations.is_empty() {
                evidence.mark_not_applicable(key);
            } else if applied {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
    }
}

fn terminal_domain(layout: &IshikawaDiagramLayout) -> Vec<TextExpectation> {
    let mut expectations = Vec::new();
    if let Some(head) = &layout.head {
        push_text_expectation(
            &mut expectations,
            &head.label,
            IshikawaTypographyTerminal::Head,
        );
    }
    for pair in &layout.pairs {
        push_branch_expectations(&mut expectations, &pair.upper);
        if let Some(lower) = &pair.lower {
            push_branch_expectations(&mut expectations, lower);
        }
    }
    expectations
}

fn push_branch_expectations(
    expectations: &mut Vec<TextExpectation>,
    branch: &IshikawaBranchLayout,
) {
    push_text_expectation(
        expectations,
        &branch.label_group.label,
        IshikawaTypographyTerminal::InheritedBaseSize,
    );
    for subgroup in &branch.sub_groups {
        push_text_expectation(
            expectations,
            &subgroup.label,
            IshikawaTypographyTerminal::InheritedBaseSize,
        );
    }
}

fn push_text_expectation(
    expectations: &mut Vec<TextExpectation>,
    text: &IshikawaTextLayout,
    typography_terminal: IshikawaTypographyTerminal,
) {
    if text.text.trim().is_empty() {
        return;
    }
    expectations.push(TextExpectation {
        class_name: text.class_name.as_str().into(),
        typography_terminal,
        fill: None,
    });
}

fn observe_text_style(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns_fill: bool,
    expectation: &mut TextExpectation,
    winner_properties: &mut BTreeSet<(usize, ResolvedStyleProperty)>,
    expected_capabilities: &mut BTreeMap<(usize, ResolvedStyleProperty), ThemeCapability>,
) {
    winner_properties.extend(
        style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property)),
    );
    if let Some(fill) = typed_fill_expectation(theme, style, mermaid_owns_fill) {
        expected_capabilities.insert(
            (fill.rule_index, ResolvedStyleProperty::Fill),
            paint_capability_from_css(&fill.css),
        );
        expectation.fill = Some(fill);
    }
}

fn typed_fill_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns_fill: bool,
) -> Option<ExpectedFill> {
    if mermaid_owns_fill {
        return None;
    }
    let origin = style.fill_resolution().winner()?;
    let rule = theme
        .family_rules()
        .find_map(|(index, rule)| (index == origin.rule_index()).then_some(rule))?;
    let facet = FamilyThemeRuleFacet::fill(style.fill_resolution().specified())?;
    if rule.target() != ThemeTarget::Text
        || rule.variant().is_some()
        || rule.ordinal().is_some()
        || theme.rule_facet_disposition(origin.rule_index(), facet)
            != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    let css = match style.fill_resolution().specified() {
        Specified::Value(CanvasPaint::Transparent) => "transparent".into(),
        Specified::Value(CanvasPaint::Solid(color)) => color.as_css().into_boxed_str(),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(
            CanvasPaint::LinearGradient(_)
            | CanvasPaint::RadialGradient(_)
            | CanvasPaint::Pattern(_),
        ) => return None,
    };
    Some(ExpectedFill {
        rule_index: origin.rule_index(),
        css,
    })
}

fn selector_matches_text_domain(selector: FamilyThemeSelectorShape, text_count: usize) -> bool {
    match selector {
        FamilyThemeSelectorShape::Static {
            variant: None | Some(ThemeVariant::Default),
        }
        | FamilyThemeSelectorShape::Ordinal {
            variant: None | Some(ThemeVariant::Default),
            ..
        } => selector.ordinal_domain_intersects_occurrence_count(text_count),
        FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => false,
    }
}

fn mermaid_owns_text_fill(config: &MermaidConfig) -> bool {
    merman_core::__private::config_path_overrides_typed_default(config, "themeVariables.textColor")
}

fn paint_capability_from_css(css: &str) -> ThemeCapability {
    if css == "transparent" {
        ThemeCapability::TransparentPaint
    } else {
        ThemeCapability::SolidPaint
    }
}

#[derive(Debug, Default)]
struct TextRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

/// Writer-owned proof that the Ishikawa stylesheet and each visible text terminal emitted the
/// planned styling at the renderer boundary.
#[derive(Debug)]
pub(crate) struct IshikawaTextThemeReceipt {
    expectations: Box<[TextExpectation]>,
    expected_font_family_css: Box<str>,
    expected_font_size_css: Box<str>,
    css_emissions: usize,
    next_index: usize,
    terminals_match: bool,
    effective_by_rule: BTreeMap<usize, usize>,
    emitted_by_rule: BTreeMap<usize, usize>,
}

impl IshikawaTextThemeReceipt {
    fn new(
        expectations: Box<[TextExpectation]>,
        expected_font_family_css: &str,
        expected_font_size_css: &str,
    ) -> Self {
        Self {
            expectations,
            expected_font_family_css: expected_font_family_css.into(),
            expected_font_size_css: expected_font_size_css.into(),
            css_emissions: 0,
            next_index: 0,
            terminals_match: true,
            effective_by_rule: BTreeMap::new(),
            emitted_by_rule: BTreeMap::new(),
        }
    }

    /// Append the exact inherited typography declarations and record the single stylesheet event
    /// at the same writer boundary. The later head selector intentionally overrides only the base
    /// font size while preserving the inherited font stack.
    pub(crate) fn write_text_rules(
        &mut self,
        css: &mut impl std::fmt::Write,
        diagram_id: impl Copy + std::fmt::Display,
        text_color: &str,
    ) {
        self.css_emissions = self.css_emissions.saturating_add(1);
        let _ = write!(
            css,
            "#{diagram_id} .ishikawa text {{ font-family: {}; font-size: {}; fill: {}; }}\
#{diagram_id} .ishikawa .ishikawa-head-label {{ font-weight: 600; text-anchor: middle; dominant-baseline: middle; font-size: 14px; }}",
            self.expected_font_family_css, self.expected_font_size_css, text_color
        );
    }

    pub(crate) fn record_checkpointed_text(
        &mut self,
        emitted_index: usize,
        emitted_class: &str,
        emitted_fill: Option<(usize, &str)>,
        emitted_typography_terminal: IshikawaTypographyTerminal,
    ) {
        if emitted_index != self.next_index {
            self.terminals_match = false;
            return;
        }
        self.next_index = self.next_index.saturating_add(1);
        let Some(expected) = self.expectations.get(emitted_index) else {
            self.terminals_match = false;
            return;
        };
        self.terminals_match &= expected.class_name.as_ref() == emitted_class;
        self.terminals_match &= expected.typography_terminal == emitted_typography_terminal;

        match expected.fill.as_ref() {
            Some(expected_fill) => {
                *self
                    .effective_by_rule
                    .entry(expected_fill.rule_index)
                    .or_default() += 1;
                let fill_matches = emitted_fill.is_some_and(|(rule_index, css)| {
                    rule_index == expected_fill.rule_index && css == expected_fill.css.as_ref()
                });
                self.terminals_match &= fill_matches;
                if fill_matches {
                    *self
                        .emitted_by_rule
                        .entry(expected_fill.rule_index)
                        .or_default() += 1;
                }
            }
            None => self.terminals_match &= emitted_fill.is_none(),
        }
    }

    fn proves_complete(&self) -> bool {
        self.css_emissions == 1
            && self.terminals_match
            && self.next_index == self.expectations.len()
    }

    fn has_effective_rule(&self, rule_index: usize) -> bool {
        self.effective_by_rule
            .get(&rule_index)
            .copied()
            .unwrap_or(0)
            != 0
    }

    fn proves_rule(&self, rule_index: usize) -> bool {
        let effective = self
            .effective_by_rule
            .get(&rule_index)
            .copied()
            .unwrap_or(0);
        self.proves_complete()
            && effective != 0
            && self.emitted_by_rule.get(&rule_index).copied() == Some(effective)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding, EffectGraph,
        EffectInput, EffectPrimitive, OrdinalPalette, ThemeColorValue, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;
    use serde_json::json;

    const RECEIPT_DIAGRAM_ID: &str = "ishikawa-receipt";

    fn expectation(
        class_name: &str,
        typography_terminal: IshikawaTypographyTerminal,
        fill: Option<(usize, &str)>,
    ) -> TextExpectation {
        TextExpectation {
            class_name: class_name.into(),
            typography_terminal,
            fill: fill.map(|(rule_index, css)| ExpectedFill {
                rule_index,
                css: css.into(),
            }),
        }
    }

    fn receipt(expectations: Box<[TextExpectation]>) -> IshikawaTextThemeReceipt {
        IshikawaTextThemeReceipt::new(expectations, "Ishikawa Sans,monospace", "18px")
    }

    #[test]
    fn text_receipt_requires_exact_stylesheet_terminal_order_role_value_and_count() {
        let expectations = vec![
            expectation(
                "ishikawa-head-label",
                IshikawaTypographyTerminal::Head,
                Some((0, "#123456")),
            ),
            expectation(
                "ishikawa-label cause",
                IshikawaTypographyTerminal::InheritedBaseSize,
                Some((0, "#123456")),
            ),
        ]
        .into_boxed_slice();
        let mut complete = receipt(expectations.clone());
        let mut css = String::new();
        complete.write_text_rules(&mut css, RECEIPT_DIAGRAM_ID, "#123456");
        complete.record_checkpointed_text(
            0,
            "ishikawa-head-label",
            Some((0, "#123456")),
            IshikawaTypographyTerminal::Head,
        );
        complete.record_checkpointed_text(
            1,
            "ishikawa-label cause",
            Some((0, "#123456")),
            IshikawaTypographyTerminal::InheritedBaseSize,
        );
        assert_eq!(
            css,
            "#ishikawa-receipt .ishikawa text { font-family: Ishikawa Sans,monospace; font-size: 18px; fill: #123456; }#ishikawa-receipt .ishikawa .ishikawa-head-label { font-weight: 600; text-anchor: middle; dominant-baseline: middle; font-size: 14px; }"
        );
        assert!(complete.proves_complete());
        assert!(complete.proves_rule(0));

        let mut missing_css = receipt(expectations.clone());
        missing_css.record_checkpointed_text(
            0,
            "ishikawa-head-label",
            Some((0, "#123456")),
            IshikawaTypographyTerminal::Head,
        );
        missing_css.record_checkpointed_text(
            1,
            "ishikawa-label cause",
            Some((0, "#123456")),
            IshikawaTypographyTerminal::InheritedBaseSize,
        );
        assert!(!missing_css.proves_complete());

        let mut duplicate_css = receipt(expectations.clone());
        let mut css = String::new();
        duplicate_css.write_text_rules(&mut css, RECEIPT_DIAGRAM_ID, "#123456");
        duplicate_css.write_text_rules(&mut css, RECEIPT_DIAGRAM_ID, "#123456");
        duplicate_css.record_checkpointed_text(
            0,
            "ishikawa-head-label",
            Some((0, "#123456")),
            IshikawaTypographyTerminal::Head,
        );
        duplicate_css.record_checkpointed_text(
            1,
            "ishikawa-label cause",
            Some((0, "#123456")),
            IshikawaTypographyTerminal::InheritedBaseSize,
        );
        assert!(!duplicate_css.proves_complete());

        let mut wrong_order = receipt(expectations.clone());
        let mut css = String::new();
        wrong_order.write_text_rules(&mut css, RECEIPT_DIAGRAM_ID, "#123456");
        wrong_order.record_checkpointed_text(
            1,
            "ishikawa-label cause",
            Some((0, "#123456")),
            IshikawaTypographyTerminal::InheritedBaseSize,
        );
        assert!(!wrong_order.proves_complete());

        let mut wrong_role = receipt(expectations.clone());
        let mut css = String::new();
        wrong_role.write_text_rules(&mut css, RECEIPT_DIAGRAM_ID, "#123456");
        wrong_role.record_checkpointed_text(
            0,
            "ishikawa-head-label",
            Some((0, "#123456")),
            IshikawaTypographyTerminal::InheritedBaseSize,
        );
        wrong_role.record_checkpointed_text(
            1,
            "ishikawa-label cause",
            Some((0, "#123456")),
            IshikawaTypographyTerminal::Head,
        );
        assert!(!wrong_role.proves_complete());

        let mut wrong_fill = receipt(expectations);
        let mut css = String::new();
        wrong_fill.write_text_rules(&mut css, RECEIPT_DIAGRAM_ID, "#123456");
        wrong_fill.record_checkpointed_text(
            0,
            "ishikawa-head-label",
            Some((0, "#abcdef")),
            IshikawaTypographyTerminal::Head,
        );
        wrong_fill.record_checkpointed_text(
            1,
            "ishikawa-label cause",
            Some((0, "#123456")),
            IshikawaTypographyTerminal::InheritedBaseSize,
        );
        assert!(!wrong_fill.proves_complete());
        assert!(!wrong_fill.proves_rule(0));
    }

    #[test]
    fn source_owned_receipt_rejects_a_typed_terminal_fill() {
        let expectations = vec![expectation(
            "ishikawa-head-label",
            IshikawaTypographyTerminal::Head,
            None,
        )]
        .into_boxed_slice();
        let mut complete = receipt(expectations.clone());
        let mut css = String::new();
        complete.write_text_rules(&mut css, RECEIPT_DIAGRAM_ID, "#123456");
        complete.record_checkpointed_text(
            0,
            "ishikawa-head-label",
            None,
            IshikawaTypographyTerminal::Head,
        );
        assert!(complete.proves_complete());

        let mut leaked_typed_fill = receipt(expectations);
        let mut css = String::new();
        leaked_typed_fill.write_text_rules(&mut css, RECEIPT_DIAGRAM_ID, "#123456");
        leaked_typed_fill.record_checkpointed_text(
            0,
            "ishikawa-head-label",
            Some((0, "#123456")),
            IshikawaTypographyTerminal::Head,
        );
        assert!(!leaked_typed_fill.proves_complete());
    }

    #[test]
    fn absent_title_terminal_accounts_every_title_mechanism_as_not_applicable() {
        let palette = OrdinalPalette::new([
            ThemeColorValue::parse("#123456").expect("valid Ishikawa title palette color")
        ])
        .expect("non-empty Ishikawa title palette");
        let effect_id = "ishikawa-title-shadow";
        let effect = EffectGraph::new(
            effect_id,
            [EffectPrimitive::DropShadow {
                input: EffectInput::SourceGraphic,
                offset_x: 1.0,
                offset_y: 1.0,
                blur_radius: 2.0,
                spread: 0.0,
                color: ThemeColorValue::parse("#00000080")
                    .expect("valid Ishikawa title shadow color"),
            }],
        )
        .expect("valid Ishikawa title effect");
        let effects = DiagramEffectSet::default()
            .with_graph(effect)
            .expect("unique Ishikawa title effect")
            .with_binding(
                EffectBinding::new(ThemeTarget::Title, effect_id)
                    .expect("valid Ishikawa title effect binding"),
            )
            .expect("unique Ishikawa title effect binding");
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new()
                    .with_styles(
                        ThemeRuleSet::default()
                            .with_rule(ThemeRule::new(
                                ThemeTarget::Title,
                                ThemeStylePatch::default().with_fill(
                                    CanvasPaint::solid("#abcdef")
                                        .expect("valid Ishikawa title fill"),
                                ),
                            ))
                            .with_ordinal_palette(ThemeTarget::Title, palette),
                    )
                    .with_effects(effects),
            )
            .expect("compile Ishikawa title-only theme")
            .resolve(DiagramFamilyId::ISHIKAWA);
        let layout = IshikawaDiagramLayout {
            bounds: None,
            total_width: 0.0,
            total_height: 0.0,
            viewbox_x: 0.0,
            viewbox_y: 0.0,
            padding: 0.0,
            use_max_width: false,
            font_size: 16.0,
            head: None,
            spine: None,
            pairs: Vec::new(),
        };
        let work_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let config = MermaidConfig::from_value(json!({}));

        let plan = IshikawaTextThemePlan::resolve(Some(&resolved), &config, &layout, &work_meter)
            .expect("resolve Ishikawa title-only theme");
        let mut receipt = plan.begin_terminal_receipt();
        let mut css = String::new();
        receipt.write_text_rules(&mut css, RECEIPT_DIAGRAM_ID, "#333333");
        assert!(plan.record_terminal(receipt));

        let evidence = plan.finish_evidence();
        let expected = [
            FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Title,
            },
            FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Title,
            },
            FamilyThemeMechanismKey::EffectBinding {
                target: ThemeTarget::Title,
                effect_id: effect_id.to_string(),
            },
        ];
        assert_eq!(evidence.not_applicable_mechanisms().len(), expected.len());
        for key in expected {
            assert!(evidence.not_applicable_mechanisms().contains(&key));
        }
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
    }
}
