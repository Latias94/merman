use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle,
    Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod terminal;

use terminal::ExpectedStroke;
pub(crate) use terminal::{
    ClassMarkerTerminalExpectation, ClassRelationTerminalExpectation, ClassRelationThemeReceipt,
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum ClassRelationStrokeWidth {
    Unspecified,
    Clear { rule_index: usize },
    // Keep Mermaid ownership even when its CSS value cannot be reduced to a finite paint width;
    // an unmeasurable source value must not accidentally fall back to the typed default.
    MermaidOwned { paint_width: Option<f32> },
    Typed { rule_index: usize, value: f32 },
}

impl Default for ClassRelationStrokeWidth {
    fn default() -> Self {
        Self::Unspecified
    }
}

/// Prepared final Class relation paint winner shared by bounds, SVG emission, and evidence.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClassRelationThemePlan {
    stroke: Option<ExpectedStroke>,
    mermaid_owns_stroke: bool,
    stroke_width: ClassRelationStrokeWidth,
    static_winner_rules: BTreeMap<(ThemeTarget, ResolvedStyleProperty), usize>,
    ordinal_winner_rules: BTreeSet<(ThemeTarget, usize, ResolvedStyleProperty)>,
}

impl ClassRelationThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        relation_count: usize,
        node_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mermaid_owns_stroke_width = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.strokeWidth",
        );
        let mermaid_stroke_width = if mermaid_owns_stroke_width {
            crate::class::config::ClassConfigView::new(effective_config.as_value())
                .relation_stroke_width_for_bounds()
        } else {
            None
        };
        let mermaid_owns_stroke = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.lineColor",
        );
        let Some(theme) = theme else {
            return Ok(Self {
                stroke: None,
                mermaid_owns_stroke,
                stroke_width: if mermaid_owns_stroke_width {
                    ClassRelationStrokeWidth::MermaidOwned {
                        paint_width: mermaid_stroke_width,
                    }
                } else {
                    ClassRelationStrokeWidth::Unspecified
                },
                ..Self::default()
            });
        };
        let style = theme.style_with_work_meter(
            ThemeTarget::Edge,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let static_winner_rules = style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| ((ThemeTarget::Edge, property), origin.rule_index()))
            .collect::<BTreeMap<_, _>>();
        let node_style = theme.style_with_work_meter(
            ThemeTarget::Node,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let mut static_winner_rules = static_winner_rules;
        static_winner_rules.extend(
            node_style
                .winner_rule_properties()
                .into_iter()
                .map(|(property, origin)| ((ThemeTarget::Node, property), origin.rule_index())),
        );
        let mut ordinal_winner_rules = BTreeSet::new();
        for (target, target_count) in [
            (ThemeTarget::Edge, relation_count),
            (ThemeTarget::Node, node_count),
        ] {
            let has_ordinal_rules = theme
                .family_rules()
                .any(|(_, rule)| rule.target() == target && rule.ordinal().is_some());
            if !has_ordinal_rules {
                continue;
            }
            for ordinal in 1..=target_count {
                let ordinal_style = theme.style_with_work_meter(
                    target,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work_meter,
                )?;
                ordinal_winner_rules.extend(
                    ordinal_style
                        .winner_rule_properties()
                        .into_iter()
                        .map(|(property, origin)| (target, origin.rule_index(), property)),
                );
            }
        }
        let has_ordinal_stroke_rules = theme.family_rules().any(|(_, rule)| {
            rule.target() == ThemeTarget::Edge
                && rule.ordinal().is_some()
                && !matches!(rule.style().stroke.paint, Specified::Unspecified)
        });
        let stroke = if has_ordinal_stroke_rules {
            None
        } else {
            typed_stroke_expectation(theme, &style, mermaid_owns_stroke)
        };
        let typed_stroke_width = style
            .stroke_width_resolution()
            .winner()
            .and_then(|origin| {
                let rule_index = origin.rule_index();
                (theme.rule_facet_disposition(rule_index, FamilyThemeRuleFacet::StrokeWidth)
                    == Some(FamilyThemeDisposition::TypedAdapter))
                .then_some((rule_index, style.stroke_width_resolution().specified()))
            })
            .map_or(
                ClassRelationStrokeWidth::Unspecified,
                |(rule_index, specified)| match specified {
                    Specified::Unspecified => ClassRelationStrokeWidth::Unspecified,
                    Specified::Clear => ClassRelationStrokeWidth::Clear { rule_index },
                    Specified::Value(value) => ClassRelationStrokeWidth::Typed {
                        rule_index,
                        value: *value,
                    },
                },
            );
        let stroke_width = if mermaid_owns_stroke_width {
            ClassRelationStrokeWidth::MermaidOwned {
                paint_width: mermaid_stroke_width,
            }
        } else {
            typed_stroke_width
        };
        Ok(Self {
            stroke,
            mermaid_owns_stroke,
            stroke_width,
            static_winner_rules,
            ordinal_winner_rules,
        })
    }

    pub(crate) const fn paint_stroke_width(&self) -> Option<f32> {
        match self.stroke_width {
            ClassRelationStrokeWidth::MermaidOwned { paint_width } => paint_width,
            ClassRelationStrokeWidth::Typed { value, .. } => Some(value),
            ClassRelationStrokeWidth::Unspecified | ClassRelationStrokeWidth::Clear { .. } => None,
        }
    }

    pub(crate) const fn typed_stroke_width(&self) -> Option<f32> {
        match self.stroke_width {
            ClassRelationStrokeWidth::Typed { value, .. } => Some(value),
            ClassRelationStrokeWidth::Unspecified
            | ClassRelationStrokeWidth::Clear { .. }
            | ClassRelationStrokeWidth::MermaidOwned { .. } => None,
        }
    }

    pub(crate) fn typed_stroke(&self) -> Option<(usize, &str)> {
        self.stroke
            .as_ref()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        relations: Vec<ClassRelationTerminalExpectation>,
        markers: Vec<ClassMarkerTerminalExpectation>,
        hand_drawn: bool,
    ) -> ClassRelationThemeReceipt {
        ClassRelationThemeReceipt::new(
            relations,
            if self.stroke.is_some() {
                markers
            } else {
                Vec::new()
            },
            self.stroke.clone(),
            hand_drawn,
        )
    }

    fn typed_stroke_width_emission(&self) -> Option<(usize, f32)> {
        match self.stroke_width {
            ClassRelationStrokeWidth::Typed { rule_index, value } => Some((rule_index, value)),
            ClassRelationStrokeWidth::Unspecified
            | ClassRelationStrokeWidth::Clear { .. }
            | ClassRelationStrokeWidth::MermaidOwned { .. } => None,
        }
    }

    fn stroke_width_is_clear_for(&self, rule_index: usize) -> bool {
        matches!(
            self.stroke_width,
            ClassRelationStrokeWidth::Clear {
                rule_index: winner
            } if winner == rule_index
        )
    }

    fn route_won(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        if target == ThemeTarget::Edge
            && ((facet == FamilyThemeRuleFacet::StrokeWidth
                && matches!(
                    self.stroke_width,
                    ClassRelationStrokeWidth::MermaidOwned { .. }
                ))
                || (matches!(facet, FamilyThemeRuleFacet::Stroke(_)) && self.mermaid_owns_stroke))
        {
            return false;
        }
        let property = resolved_style_property_for_facet(facet);
        match selector {
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default),
            } => self.static_winner_rules.get(&(target, property)).copied() == Some(rule_index),
            FamilyThemeSelectorShape::Ordinal {
                variant: None | Some(ThemeVariant::Default),
                ..
            } => self
                .ordinal_winner_rules
                .contains(&(target, rule_index, property)),
            FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => {
                false
            }
        }
    }
}

fn typed_stroke_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns_stroke: bool,
) -> Option<ExpectedStroke> {
    if mermaid_owns_stroke {
        return None;
    }
    let origin = style.stroke_resolution().winner()?;
    let facet = FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())?;
    let has_direct_static_route = theme
        .family_mechanism_routes()
        .iter()
        .copied()
        .any(|route| {
            route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        rule_index,
                        target: ThemeTarget::Edge,
                        selector: FamilyThemeSelectorShape::Static { variant: None },
                        facet: route_facet,
                    } if rule_index == origin.rule_index() && route_facet == facet
                )
        });
    if !has_direct_static_route {
        return None;
    }
    let css = match style.stroke_resolution().specified() {
        Specified::Value(crate::diagram_theme::CanvasPaint::Transparent) => "transparent".into(),
        Specified::Value(crate::diagram_theme::CanvasPaint::Solid(color)) => color.as_css(),
        Specified::Unspecified
        | Specified::Clear
        | Specified::Value(crate::diagram_theme::CanvasPaint::LinearGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::RadialGradient(_))
        | Specified::Value(crate::diagram_theme::CanvasPaint::Pattern(_)) => return None,
    };
    Some(ExpectedStroke {
        rule_index: origin.rule_index(),
        css,
    })
}

/// Terminal Class theme ledger. The SVG writer may seal it only after the completed root exists.
#[derive(Debug)]
pub(crate) struct ClassThemeEvidenceRecorder {
    expected_relation_paths: usize,
    node_count: usize,
    terminal_receipt: OnceLock<ClassRelationThemeReceipt>,
}

impl ClassThemeEvidenceRecorder {
    pub(crate) fn new(expected_relation_paths: usize, node_count: usize) -> Self {
        Self {
            expected_relation_paths,
            node_count,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn record_terminal(&self, receipt: ClassRelationThemeReceipt) -> bool {
        if receipt.expected_relation_count() != self.expected_relation_paths
            || !receipt.proves_complete()
        {
            return false;
        }
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
        plan: &ClassRelationThemePlan,
    ) -> FamilyThemeEvidence {
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        let Some(theme) = theme else {
            return evidence;
        };
        let receipt = self.terminal_receipt.get();
        let has_relations = self.expected_relation_paths != 0;
        let mut rules = BTreeMap::<(usize, ThemeTarget), ClassRuleObservation>::new();

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target,
                    selector,
                    facet,
                } if matches!(target, ThemeTarget::Edge | ThemeTarget::Node) => {
                    let observation = rules.entry((rule_index, target)).or_default();
                    let route_applies = match target {
                        ThemeTarget::Edge => has_relations,
                        ThemeTarget::Node => self.node_count != 0,
                        _ => unreachable!("guarded Class evidence target"),
                    } && plan.route_won(rule_index, target, selector, facet);
                    if !route_applies {
                        continue;
                    }
                    observation.applicable = true;
                    if target == ThemeTarget::Node {
                        match route.disposition() {
                            FamilyThemeDisposition::Unsupported => {
                                observation
                                    .residual
                                    .get_or_insert(unsupported_residual_for_facet(facet));
                            }
                            FamilyThemeDisposition::TypedAdapter
                            | FamilyThemeDisposition::LegacyCompatibility => {
                                observation.incomplete = true;
                            }
                        }
                        continue;
                    }
                    match (route.disposition(), facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) => {
                            if let Some((winner, css)) = plan.typed_stroke() {
                                if winner != rule_index {
                                    observation.incomplete = true;
                                } else if receipt
                                    .is_some_and(|receipt| receipt.proves_typed_stroke(rule_index))
                                {
                                    observation.capabilities.insert(if css == "transparent" {
                                        ThemeCapability::TransparentPaint
                                    } else {
                                        ThemeCapability::SolidPaint
                                    });
                                } else {
                                    observation.incomplete = true;
                                }
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeRuleFacet::StrokeWidth,
                        ) => {
                            if plan.stroke_width_is_clear_for(rule_index) {
                                observation.residual =
                                    Some(FamilyThemeResidualReason::UnsupportedGeometry);
                            } else if let Some((winner, _)) = plan.typed_stroke_width_emission() {
                                if winner != rule_index {
                                    observation.incomplete = true;
                                } else if receipt
                                    .is_some_and(ClassRelationThemeReceipt::proves_typed_width)
                                {
                                    observation
                                        .capabilities
                                        .insert(ThemeCapability::BorderStyling);
                                } else {
                                    observation.incomplete = true;
                                }
                            } else {
                                observation.incomplete = true;
                            }
                        }
                        (FamilyThemeDisposition::TypedAdapter, _) => {
                            observation.incomplete = true;
                        }
                        (FamilyThemeDisposition::Unsupported, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::LegacyCompatibility, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Edge,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if !has_relations {
                        evidence.mark_not_applicable(key);
                    } else if route.disposition() == FamilyThemeDisposition::Unsupported {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Edge,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if !has_relations {
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

        for ((rule_index, target), observation) in rules {
            let key = crate::diagram_theme::FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Leaving the mechanism unaccounted makes strict completion fail closed.
            } else if !observation.capabilities.is_empty() {
                evidence.mark_applied_with_capabilities(key, observation.capabilities);
            }
        }

        evidence
    }
}

#[derive(Debug, Default)]
struct ClassRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_width_receipt_requires_every_terminal_checkpoint() {
        let plan = ClassRelationThemePlan {
            stroke: None,
            mermaid_owns_stroke: false,
            stroke_width: ClassRelationStrokeWidth::Typed {
                rule_index: 3,
                value: 6.0,
            },
            static_winner_rules: BTreeMap::new(),
            ordinal_winner_rules: BTreeSet::new(),
        };
        let relations = (0..2)
            .map(|index| ClassRelationTerminalExpectation::new(index, None, None))
            .collect();
        let mut receipt = plan.begin_terminal_receipt(relations, Vec::new(), false);
        receipt.record_relation(0, None, None, None, ";;;", None, true);
        assert!(!receipt.proves_typed_width());

        receipt.record_relation(1, None, None, None, ";;;", None, true);
        assert!(receipt.proves_typed_width());
    }

    #[test]
    fn terminal_recorder_rejects_wrong_counts_and_duplicate_seals() {
        let complete_receipt = |count| {
            let relations = (0..count)
                .map(|index| ClassRelationTerminalExpectation::new(index, None, None))
                .collect::<Vec<_>>();
            let mut receipt = ClassRelationThemeReceipt::new(relations, Vec::new(), None, false);
            for index in 0..count {
                receipt.record_relation(index, None, None, None, ";;;", None, false);
            }
            receipt
        };
        let recorder = ClassThemeEvidenceRecorder::new(2, 0);
        assert!(!recorder.record_terminal(complete_receipt(1)));
        assert!(recorder.record_terminal(complete_receipt(2)));
        assert!(!recorder.record_terminal(complete_receipt(2)));
    }
}
