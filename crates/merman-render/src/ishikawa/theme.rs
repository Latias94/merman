use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme,
    ResolvedStyleProperty, ResolvedThemeStyle, Specified, ThemeCapability, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::{IshikawaBranchLayout, IshikawaDiagramLayout, IshikawaTextLayout};
use crate::resources::{OperationWorkError, OperationWorkMeter};

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedFill {
    rule_index: usize,
    css: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextExpectation {
    class_name: Box<str>,
    fill: Option<ExpectedFill>,
}

/// Final Ishikawa text fill shared by terminal SVG emission and family evidence.
#[derive(Debug)]
pub(crate) struct IshikawaTextThemePlan {
    expectations: Box<[TextExpectation]>,
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
        let Some(theme) = theme else {
            return Ok(Self::baseline_from_expectations(expectations));
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
                        observation.suppressed = true;
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
                            } else {
                                observation.suppressed = true;
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
                    let key = theme.family_mechanism_key(route);
                    if text_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Text,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if text_count == 0 {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() != FamilyThemeDisposition::LegacyCompatibility {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    }
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
            } else if observation.suppressed {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            expectations: expectations.into_boxed_slice(),
            evidence,
            pending,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(layout: &IshikawaDiagramLayout) -> Self {
        Self::baseline_from_expectations(terminal_domain(layout))
    }

    fn baseline_from_expectations(expectations: Vec<TextExpectation>) -> Self {
        Self {
            expectations: expectations.into_boxed_slice(),
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
        IshikawaTextThemeReceipt::new(self.expectations.clone())
    }

    pub(crate) fn record_terminal(&self, receipt: IshikawaTextThemeReceipt) -> bool {
        receipt.proves_complete() && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        for (key, capabilities) in &self.pending {
            let rule_index = match key {
                FamilyThemeMechanismKey::Rule { index, .. } => *index,
                FamilyThemeMechanismKey::Typography
                | FamilyThemeMechanismKey::OrdinalPalette { .. }
                | FamilyThemeMechanismKey::EffectBinding { .. } => continue,
            };
            if receipt.proves_rule(rule_index) {
                evidence.mark_applied_with_capabilities(key.clone(), capabilities.iter().copied());
            } else if receipt.proves_complete() && !receipt.has_effective_rule(rule_index) {
                evidence.mark_not_applicable(key.clone());
            }
        }
        evidence
    }
}

fn terminal_domain(layout: &IshikawaDiagramLayout) -> Vec<TextExpectation> {
    let mut expectations = Vec::new();
    if let Some(head) = &layout.head {
        push_text_expectation(&mut expectations, &head.label);
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
    push_text_expectation(expectations, &branch.label_group.label);
    for subgroup in &branch.sub_groups {
        push_text_expectation(expectations, &subgroup.label);
    }
}

fn push_text_expectation(expectations: &mut Vec<TextExpectation>, text: &IshikawaTextLayout) {
    if text.text.trim().is_empty() {
        return;
    }
    expectations.push(TextExpectation {
        class_name: text.class_name.as_str().into(),
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
            .into_iter()
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
    suppressed: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

/// Writer-owned proof that each visible Ishikawa text terminal emitted its planned fill.
#[derive(Debug, Clone)]
pub(crate) struct IshikawaTextThemeReceipt {
    expectations: Box<[TextExpectation]>,
    next_index: usize,
    terminals_match: bool,
    effective_by_rule: BTreeMap<usize, usize>,
    emitted_by_rule: BTreeMap<usize, usize>,
}

impl IshikawaTextThemeReceipt {
    fn new(expectations: Box<[TextExpectation]>) -> Self {
        Self {
            expectations,
            next_index: 0,
            terminals_match: true,
            effective_by_rule: BTreeMap::new(),
            emitted_by_rule: BTreeMap::new(),
        }
    }

    pub(crate) fn record_checkpointed_text(
        &mut self,
        emitted_index: usize,
        emitted_class: &str,
        emitted_fill: Option<(usize, &str)>,
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
        self.terminals_match && self.next_index == self.expectations.len()
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

    fn expectation(class_name: &str, fill: Option<(usize, &str)>) -> TextExpectation {
        TextExpectation {
            class_name: class_name.into(),
            fill: fill.map(|(rule_index, css)| ExpectedFill {
                rule_index,
                css: css.into(),
            }),
        }
    }

    #[test]
    fn text_receipt_requires_exact_terminal_order_value_and_count() {
        let expectations = vec![
            expectation("ishikawa-head-label", Some((0, "#123456"))),
            expectation("ishikawa-label cause", Some((0, "#123456"))),
        ]
        .into_boxed_slice();
        let mut complete = IshikawaTextThemeReceipt::new(expectations.clone());
        complete.record_checkpointed_text(0, "ishikawa-head-label", Some((0, "#123456")));
        complete.record_checkpointed_text(1, "ishikawa-label cause", Some((0, "#123456")));
        assert!(complete.proves_complete());
        assert!(complete.proves_rule(0));

        let mut wrong_order = IshikawaTextThemeReceipt::new(expectations.clone());
        wrong_order.record_checkpointed_text(1, "ishikawa-label cause", Some((0, "#123456")));
        assert!(!wrong_order.proves_complete());

        let mut wrong_fill = IshikawaTextThemeReceipt::new(expectations);
        wrong_fill.record_checkpointed_text(0, "ishikawa-head-label", Some((0, "#abcdef")));
        wrong_fill.record_checkpointed_text(1, "ishikawa-label cause", Some((0, "#123456")));
        assert!(!wrong_fill.proves_complete());
        assert!(!wrong_fill.proves_rule(0));
    }

    #[test]
    fn source_owned_receipt_rejects_a_typed_terminal_fill() {
        let expectations = vec![expectation("ishikawa-head-label", None)].into_boxed_slice();
        let mut complete = IshikawaTextThemeReceipt::new(expectations.clone());
        complete.record_checkpointed_text(0, "ishikawa-head-label", None);
        assert!(complete.proves_complete());

        let mut leaked_typed_fill = IshikawaTextThemeReceipt::new(expectations);
        leaked_typed_fill.record_checkpointed_text(0, "ishikawa-head-label", Some((0, "#123456")));
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
        assert!(plan.record_terminal(plan.begin_terminal_receipt()));

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
