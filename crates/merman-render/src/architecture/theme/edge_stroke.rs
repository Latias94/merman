use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme,
    ResolvedStyleProperty, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

const ARCH_EDGE_COLOR_PATH: &str = "themeVariables.archEdgeColor";
const LINE_COLOR_PATH: &str = "themeVariables.lineColor";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArchitectureEdgeStroke {
    css: Box<str>,
    rule_index: usize,
    capability: ThemeCapability,
}

#[derive(Debug, Default)]
struct ArchitectureEdgeRuleObservation {
    applicable: bool,
    pending: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

/// Direct Architecture edge-path stroke owned by the family renderer.
#[derive(Debug)]
pub(super) struct ArchitectureEdgeStrokePlan {
    stroke: Option<ArchitectureEdgeStroke>,
    expected_edge_count: usize,
    pending_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<ArchitectureEdgeThemeReceipt>,
}

impl ArchitectureEdgeStrokePlan {
    pub(super) fn baseline(expected_edge_count: usize) -> Self {
        Self {
            stroke: None,
            expected_edge_count,
            pending_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(super) fn resolve(
        theme: &ResolvedDiagramTheme,
        effective_config: &MermaidConfig,
        expected_edge_count: usize,
        work_meter: &OperationWorkMeter,
        evidence: &mut FamilyThemeEvidence,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline(expected_edge_count);
        let config_owns_stroke = mermaid_owns_edge_stroke(effective_config);
        let has_ordinal_edge_rules = theme.family_rules().any(|(_, rule)| {
            rule.target() == ThemeTarget::Edge
                && matches!(rule.variant(), None | Some(ThemeVariant::Default))
                && rule.ordinal().is_some()
        });
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut first_candidate = None::<Option<ArchitectureEdgeStroke>>;
        let mut candidates_match = true;

        if expected_edge_count > 0 {
            if has_ordinal_edge_rules {
                for ordinal in 1..=expected_edge_count {
                    let style = theme.style_with_work_meter(
                        ThemeTarget::Edge,
                        ThemeVariant::Default,
                        Some(ordinal),
                        work_meter,
                    )?;
                    let candidate = observe_style(theme, &style, &mut winner_properties);
                    match first_candidate.as_ref() {
                        None => first_candidate = Some(candidate),
                        Some(first) => candidates_match &= first == &candidate,
                    }
                }
            } else {
                let style = theme.style_with_work_meter(
                    ThemeTarget::Edge,
                    ThemeVariant::Default,
                    None,
                    work_meter,
                )?;
                first_candidate = Some(observe_style(theme, &style, &mut winner_properties));
            }
        }

        let stroke = candidates_match
            .then(|| first_candidate.flatten())
            .flatten();
        let stroke_rule = stroke.as_ref().map(|stroke| stroke.rule_index);
        let mut observations = BTreeMap::<usize, ArchitectureEdgeRuleObservation>::new();

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Edge,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !selector.ordinal_domain_intersects_occurrence_count(expected_edge_count) {
                        continue;
                    }
                    let route_won = winner_properties
                        .contains(&(rule_index, resolved_style_property_for_facet(facet)));
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    if !route_won && !qualified_variant {
                        continue;
                    }

                    observation.applicable = true;
                    match (route.disposition(), selector, facet) {
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) if stroke_rule == Some(rule_index) => {
                            if !config_owns_stroke {
                                observation.pending = true;
                            }
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Edge,
                } => {
                    // Reconciled below from the final style of each actual edge occurrence.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Edge,
                    ..
                } => {
                    // Reconciled below from the final style of each actual edge occurrence.
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        reconcile_unsupported_terminal_domains(
            theme,
            evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Edge,
                TerminalVariantDomain::uniform(expected_edge_count, ThemeVariant::Default),
            )],
            work_meter,
        )?;

        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Edge,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Selector-qualified, mixed-facet, or compatibility routes remain fail-closed.
            } else if observation.pending {
                debug_assert!(plan.pending_key.is_none());
                plan.pending_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        if !config_owns_stroke && plan.pending_key.is_some() {
            plan.stroke = stroke;
        }
        Ok(plan)
    }

    pub(super) fn terminal_stroke(&self) -> Option<(usize, &str)> {
        self.stroke
            .as_ref()
            .map(|stroke| (stroke.rule_index, stroke.css.as_ref()))
    }

    pub(super) fn begin_terminal_receipt<'a>(
        &self,
        terminals: impl IntoIterator<Item = (usize, &'a str, &'a str)>,
    ) -> Option<ArchitectureEdgeThemeReceipt> {
        self.pending_key
            .as_ref()
            .zip(self.stroke.as_ref())
            .map(|(_, stroke)| {
                ArchitectureEdgeThemeReceipt::new(
                    stroke,
                    terminals
                        .into_iter()
                        .map(
                            |(edge_index, lhs_id, rhs_id)| ArchitectureEdgeTerminalExpectation {
                                edge_index,
                                lhs_id: lhs_id.into(),
                                rhs_id: rhs_id.into(),
                            },
                        )
                        .collect(),
                )
            })
    }

    pub(super) fn record_terminal(&self, receipt: ArchitectureEdgeThemeReceipt) -> bool {
        receipt.proves(self) && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(super) fn finish_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if let (Some(key), Some(stroke), Some(receipt)) = (
            self.pending_key.clone(),
            self.stroke.as_ref(),
            self.terminal_receipt.get(),
        ) && receipt.proves(self)
        {
            evidence.mark_applied_with_capabilities(key, [stroke.capability]);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ArchitectureEdgeTerminalExpectation {
    edge_index: usize,
    lhs_id: Box<str>,
    rhs_id: Box<str>,
}

/// Writer-owned proof for every actual Architecture `.edge` path and its terminal stroke.
#[derive(Debug)]
pub(crate) struct ArchitectureEdgeThemeReceipt {
    expected_stroke: ArchitectureEdgeStroke,
    expected_edges: Vec<ArchitectureEdgeTerminalExpectation>,
    next_edge_position: usize,
    terminals_match: bool,
}

impl ArchitectureEdgeThemeReceipt {
    fn new(
        expected_stroke: &ArchitectureEdgeStroke,
        expected_edges: Vec<ArchitectureEdgeTerminalExpectation>,
    ) -> Self {
        Self {
            expected_stroke: expected_stroke.clone(),
            expected_edges,
            next_edge_position: 0,
            terminals_match: true,
        }
    }

    pub(crate) fn record_checkpointed_edge(
        &mut self,
        edge_index: usize,
        lhs_id: &str,
        rhs_id: &str,
        emitted_class: &str,
        emitted_stroke: Option<(usize, &str)>,
        terminal_style: Option<&str>,
    ) {
        let expected = self.expected_edges.get(self.next_edge_position);
        self.terminals_match &= expected.is_some_and(|expected| {
            expected.edge_index == edge_index
                && expected.lhs_id.as_ref() == lhs_id
                && expected.rhs_id.as_ref() == rhs_id
        });
        self.terminals_match &= emitted_class
            .split_ascii_whitespace()
            .any(|class| class == "edge");
        self.terminals_match &= emitted_stroke.is_some_and(|(rule_index, css)| {
            rule_index == self.expected_stroke.rule_index
                && css == self.expected_stroke.css.as_ref()
        });
        self.terminals_match &= terminal_style.and_then(|style| terminal_paint(style, "stroke"))
            == Some(self.expected_stroke.css.as_ref());
        self.next_edge_position = self.next_edge_position.saturating_add(1);
    }

    fn proves(&self, plan: &ArchitectureEdgeStrokePlan) -> bool {
        plan.stroke.as_ref() == Some(&self.expected_stroke)
            && self.expected_edges.len() == plan.expected_edge_count
            && self.next_edge_position == self.expected_edges.len()
            && self.terminals_match
    }
}

fn observe_style(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
    winner_properties: &mut BTreeSet<(usize, ResolvedStyleProperty)>,
) -> Option<ArchitectureEdgeStroke> {
    winner_properties.extend(
        style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property)),
    );
    typed_static_stroke(theme, style)
}

fn typed_static_stroke(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<ArchitectureEdgeStroke> {
    let origin = style.stroke_resolution().winner()?;
    let rule = theme
        .family_rules()
        .find_map(|(index, rule)| (index == origin.rule_index()).then_some(rule))?;
    let facet = FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())?;
    if rule.variant().is_some()
        || rule.ordinal().is_some()
        || theme.rule_facet_disposition(origin.rule_index(), facet)
            != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }

    match style.stroke_resolution().specified() {
        Specified::Value(CanvasPaint::Transparent) => Some(ArchitectureEdgeStroke {
            css: "transparent".into(),
            rule_index: origin.rule_index(),
            capability: ThemeCapability::TransparentPaint,
        }),
        Specified::Value(CanvasPaint::Solid(color)) => Some(ArchitectureEdgeStroke {
            css: color.as_css().into_boxed_str(),
            rule_index: origin.rule_index(),
            capability: ThemeCapability::SolidPaint,
        }),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(
            CanvasPaint::LinearGradient(_)
            | CanvasPaint::RadialGradient(_)
            | CanvasPaint::Pattern(_),
        ) => None,
    }
}

fn mermaid_owns_edge_stroke(config: &MermaidConfig) -> bool {
    if merman_core::__private::config_path_overrides_typed_default(config, ARCH_EDGE_COLOR_PATH) {
        return true;
    }
    merman_core::__private::config_path_overrides_typed_default(config, LINE_COLOR_PATH)
        && config.get_str(ARCH_EDGE_COLOR_PATH) == config.get_str(LINE_COLOR_PATH)
}

fn terminal_paint<'a>(style: &'a str, property: &str) -> Option<&'a str> {
    style
        .split(';')
        .filter_map(crate::mermaid_style::parse_style_declaration)
        .filter(|declaration| declaration.property() == property)
        .map(|declaration| declaration.value())
        .next_back()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt_plan() -> ArchitectureEdgeStrokePlan {
        ArchitectureEdgeStrokePlan {
            stroke: Some(ArchitectureEdgeStroke {
                css: "#123456".into(),
                rule_index: 7,
                capability: ThemeCapability::SolidPaint,
            }),
            expected_edge_count: 2,
            pending_key: Some(FamilyThemeMechanismKey::Rule {
                index: 7,
                target: ThemeTarget::Edge,
            }),
            terminal_receipt: OnceLock::new(),
        }
    }

    fn receipt() -> ArchitectureEdgeThemeReceipt {
        ArchitectureEdgeThemeReceipt::new(
            &ArchitectureEdgeStroke {
                css: "#123456".into(),
                rule_index: 7,
                capability: ThemeCapability::SolidPaint,
            },
            vec![
                ArchitectureEdgeTerminalExpectation {
                    edge_index: 0,
                    lhs_id: "api".into(),
                    rhs_id: "worker".into(),
                },
                ArchitectureEdgeTerminalExpectation {
                    edge_index: 1,
                    lhs_id: "worker".into(),
                    rhs_id: "db".into(),
                },
            ],
        )
    }

    fn record_first(receipt: &mut ArchitectureEdgeThemeReceipt) {
        receipt.record_checkpointed_edge(
            0,
            "api",
            "worker",
            "edge",
            Some((7, "#123456")),
            Some("stroke:#123456;"),
        );
    }

    fn record_second(receipt: &mut ArchitectureEdgeThemeReceipt) {
        receipt.record_checkpointed_edge(
            1,
            "worker",
            "db",
            "edge",
            Some((7, "#123456")),
            Some("stroke:#123456;"),
        );
    }

    #[test]
    fn edge_theme_receipt_rejects_missing_duplicate_reordered_and_wrong_terminals() {
        let plan = receipt_plan();

        let mut complete = receipt();
        record_first(&mut complete);
        record_second(&mut complete);
        assert!(complete.proves(&plan));

        let mut missing = receipt();
        record_first(&mut missing);
        assert!(!missing.proves(&plan));

        let mut duplicate = receipt();
        record_first(&mut duplicate);
        record_first(&mut duplicate);
        assert!(!duplicate.proves(&plan));

        let mut reordered = receipt();
        record_second(&mut reordered);
        record_first(&mut reordered);
        assert!(!reordered.proves(&plan));

        let mut wrong_endpoint = receipt();
        wrong_endpoint.record_checkpointed_edge(
            0,
            "api",
            "db",
            "edge",
            Some((7, "#123456")),
            Some("stroke:#123456;"),
        );
        record_second(&mut wrong_endpoint);
        assert!(!wrong_endpoint.proves(&plan));

        let mut wrong_class = receipt();
        wrong_class.record_checkpointed_edge(
            0,
            "api",
            "worker",
            "arrow",
            Some((7, "#123456")),
            Some("stroke:#123456;"),
        );
        record_second(&mut wrong_class);
        assert!(!wrong_class.proves(&plan));

        let mut wrong_rule = receipt();
        wrong_rule.record_checkpointed_edge(
            0,
            "api",
            "worker",
            "edge",
            Some((8, "#123456")),
            Some("stroke:#123456;"),
        );
        record_second(&mut wrong_rule);
        assert!(!wrong_rule.proves(&plan));

        let mut wrong_style = receipt();
        wrong_style.record_checkpointed_edge(
            0,
            "api",
            "worker",
            "edge",
            Some((7, "#123456")),
            Some("stroke:#abcdef;"),
        );
        record_second(&mut wrong_style);
        assert!(!wrong_style.proves(&plan));
    }

    #[test]
    fn edge_theme_plan_accepts_one_complete_terminal_receipt_only() {
        let plan = receipt_plan();
        let mut first = receipt();
        record_first(&mut first);
        record_second(&mut first);
        assert!(plan.record_terminal(first));

        let mut duplicate = receipt();
        record_first(&mut duplicate);
        record_second(&mut duplicate);
        assert!(!plan.record_terminal(duplicate));
    }
}
