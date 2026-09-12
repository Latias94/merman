use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, Specified,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::{QuadrantChartDiagramLayout, QuadrantChartTextData};
use crate::resources::OperationWorkMeter;

/// Text.fill historically feeds point labels, axis labels, and the title, in author order
/// with the Axis/Title rules. Quadrant captions have separate Mermaid color channels.
#[derive(Debug, Default)]
pub(crate) struct QuadrantChartTextPaintPlan {
    terminals: Arc<[(Box<str>, Box<str>)]>,
    pending: BTreeMap<(ThemeTarget, usize), ThemeCapability>,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<()>,
}

#[derive(Default)]
struct RuleObservation {
    pending: Option<ThemeCapability>,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

impl QuadrantChartTextPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        layout: &mut QuadrantChartDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let Some(theme) = theme else {
            return Ok(Self::default());
        };
        let mut plan = Self {
            evidence: FamilyThemeEvidence::from_theme(Some(theme)),
            ..Self::default()
        };
        let routes = theme
            .family_mechanism_routes()
            .iter()
            .copied()
            .filter(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Text | ThemeTarget::Title,
                        ..
                    } | FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Text
                    } | FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Text,
                        ..
                    }
                )
            })
            .collect::<Vec<_>>();
        if routes.is_empty() {
            return Ok(plan);
        }
        let mut observations = BTreeMap::<(ThemeTarget, usize), RuleObservation>::new();
        for route in &routes {
            if let FamilyThemeMechanism::RuleFacet {
                target, rule_index, ..
            } = route.mechanism()
            {
                observations.entry((target, rule_index)).or_default();
            }
        }
        let has_ordinal = theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Text | ThemeTarget::Axis | ThemeTarget::Title,
                    selector: FamilyThemeSelectorShape::Ordinal { .. },
                    ..
                }
            )
        });
        let mut ordinal = 0;
        let mut role_ordinals = BTreeMap::<ThemeTarget, usize>::new();
        let terminal_count = layout
            .points
            .len()
            .saturating_add(layout.axis_labels.len())
            .saturating_add(usize::from(layout.title.is_some()));
        work_meter.charge(terminal_count)?;
        let mut terminals = Vec::with_capacity(terminal_count);
        let mut palette_applies = false;
        let mut effect_applies = false;
        // Ordinals follow the writer's text order: points, visible axis labels, title.
        // Empty text elements are still checkpointed, but have no semantic occurrence.
        let x_count = layout
            .axis_labels
            .iter()
            .take_while(|label| label.rotation == 0.0)
            .count();
        let (x_labels, y_labels) = layout.axis_labels.split_at_mut(x_count);
        for (target, path, labels) in [
            (
                ThemeTarget::Text,
                "themeVariables.quadrantPointTextFill",
                layout
                    .points
                    .iter_mut()
                    .map(|point| &mut point.text)
                    .collect::<Vec<_>>(),
            ),
            (
                ThemeTarget::Axis,
                "themeVariables.quadrantXAxisTextFill",
                x_labels.iter_mut().collect(),
            ),
            (
                ThemeTarget::Axis,
                "themeVariables.quadrantYAxisTextFill",
                y_labels.iter_mut().collect(),
            ),
            (
                ThemeTarget::Title,
                "themeVariables.quadrantTitleFill",
                layout.title.iter_mut().collect(),
            ),
        ] {
            let source_owned =
                merman_core::__private::config_path_overrides_typed_default(config, path);
            let static_style = theme.text_style_with_work_meter(
                target,
                ThemeVariant::Default,
                None,
                work_meter,
            )?;
            let paint = (!source_owned)
                .then(|| {
                    resolve_direct_static_fill(
                        theme,
                        &static_style,
                        &[ThemeTarget::Text, ThemeTarget::Title],
                        DirectStaticSelectorDomain::Default,
                    )
                })
                .flatten();
            let mut observed_static = false;
            for label in labels {
                if let Some(paint) = &paint {
                    label.fill = paint.css().to_string();
                }
                terminals.push((
                    label.text.clone().into_boxed_str(),
                    label.fill.clone().into_boxed_str(),
                ));
                if label.text.trim().is_empty() {
                    continue;
                }
                ordinal += 1;
                let role_ordinal = role_ordinals.entry(target).or_default();
                *role_ordinal += 1;
                if observed_static && !has_ordinal {
                    continue;
                }
                observed_static = true;
                let mut dynamic_style;
                let style = if has_ordinal {
                    dynamic_style = theme.style_with_work_meter(
                        ThemeTarget::Text,
                        ThemeVariant::Default,
                        Some(ordinal),
                        work_meter,
                    )?;
                    if target != ThemeTarget::Text {
                        let role_style = theme.style_with_work_meter(
                            target,
                            ThemeVariant::Default,
                            Some(*role_ordinal),
                            work_meter,
                        )?;
                        // Shared Text and specific roles keep independent occurrence domains;
                        // merge their winners by author order, not by role priority.
                        dynamic_style.merge_from(&role_style);
                    }
                    &dynamic_style
                } else {
                    &static_style
                };
                palette_applies |= !source_owned
                    && matches!(style.fill_resolution().specified(), Specified::Unspecified);
                effect_applies |= matches!(
                    style.effect_resolution().specified(),
                    Specified::Unspecified
                );
                work_meter.charge(routes.len())?;
                for route in &routes {
                    let FamilyThemeMechanism::RuleFacet {
                        target: route_target,
                        rule_index,
                        facet,
                        ..
                    } = route.mechanism()
                    else {
                        continue;
                    };
                    let property = resolved_style_property_for_facet(facet);
                    if source_owned && property == ResolvedStyleProperty::Fill {
                        continue;
                    }
                    if !style.winner_rule_properties().any(|(candidate, origin)| {
                        candidate == property && origin.rule_index() == rule_index
                    }) {
                        continue;
                    }
                    let observation = observations
                        .get_mut(&(route_target, rule_index))
                        .expect("registered chart text rule");
                    match route.disposition() {
                        FamilyThemeDisposition::Unsupported => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        FamilyThemeDisposition::TypedAdapter
                            if property == ResolvedStyleProperty::Fill
                                && paint
                                    .as_ref()
                                    .is_some_and(|paint| paint.rule_index() == rule_index) =>
                        {
                            observation.pending = paint.as_ref().map(|paint| paint.capability());
                        }
                        _ => observation.incomplete = true,
                    }
                }
            }
        }
        for route in &routes {
            let (applies, reason) = match route.mechanism() {
                FamilyThemeMechanism::OrdinalPalette { .. } => (
                    palette_applies,
                    FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                ),
                FamilyThemeMechanism::EffectBinding { .. } => {
                    (effect_applies, FamilyThemeResidualReason::UnsupportedEffect)
                }
                _ => continue,
            };
            let key = theme.family_mechanism_key(*route);
            if applies {
                plan.evidence.mark_residual(key, reason);
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        for ((target, index), observation) in observations {
            let key = FamilyThemeMechanismKey::Rule { index, target };
            if let Some(reason) = observation.residual {
                plan.evidence.mark_residual(key, reason);
            } else if observation.incomplete { /* Winning sibling facets remain incomplete. */
            } else if let Some(capability) = observation.pending {
                plan.pending.insert((target, index), capability);
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        plan.terminals = terminals.into();
        Ok(plan)
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<QuadrantChartTextPaintReceipt> {
        (!self.pending.is_empty()).then(|| QuadrantChartTextPaintReceipt {
            expected: Arc::clone(&self.terminals),
            next: 0,
            valid: true,
        })
    }

    pub(crate) fn record_terminal(&self, receipt: QuadrantChartTextPaintReceipt) -> bool {
        Arc::ptr_eq(&self.terminals, &receipt.expected)
            && receipt.valid
            && receipt.next == self.terminals.len()
            && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if self.terminal_receipt.get().is_some() {
            for (&(target, index), &capability) in &self.pending {
                evidence.mark_applied_with_capabilities(
                    FamilyThemeMechanismKey::Rule { index, target },
                    [capability],
                );
            }
        }
        evidence
    }
}

#[derive(Debug)]
pub(crate) struct QuadrantChartTextPaintReceipt {
    expected: Arc<[(Box<str>, Box<str>)]>,
    next: usize,
    valid: bool,
}

impl QuadrantChartTextPaintReceipt {
    pub(crate) fn record(&mut self, label: &QuadrantChartTextData) {
        self.valid &= self
            .expected
            .get(self.next)
            .is_some_and(|(text, fill)| text.as_ref() == label.text && fill.as_ref() == label.fill);
        self.next = self.next.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(text: &str, fill: &str) -> QuadrantChartTextData {
        QuadrantChartTextData {
            text: text.into(),
            fill: fill.into(),
            x: 0.0,
            y: 0.0,
            font_size: 16.0,
            rotation: 0.0,
            vertical_pos: "center".into(),
            horizontal_pos: "top".into(),
        }
    }

    fn plan() -> QuadrantChartTextPaintPlan {
        use crate::diagram_theme::{
            CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
            ThemeStylePatch,
        };
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::QUADRANT_CHART);
        QuadrantChartTextPaintPlan {
            evidence: FamilyThemeEvidence::from_theme(Some(&theme)),
            terminals: vec![
                ("Point".into(), "#123456".into()),
                ("Title".into(), "#abcdef".into()),
            ]
            .into(),
            pending: BTreeMap::from([((ThemeTarget::Text, 0), ThemeCapability::SolidPaint)]),
            ..QuadrantChartTextPaintPlan::default()
        }
    }

    #[test]
    fn text_receipt_requires_complete_ordered_matching_terminals_from_the_same_plan() {
        let plan = plan();
        assert!(plan.finish_evidence().applied().is_empty());
        let mut missing = plan.begin_terminal_receipt().unwrap();
        missing.record(&label("Point", "#123456"));
        assert!(!plan.record_terminal(missing));
        for (first, second) in [
            (label("Point", "wrong"), label("Title", "#abcdef")),
            (label("Title", "#abcdef"), label("Point", "#123456")),
            (label("Point", "#123456"), label("Point", "#123456")),
        ] {
            let mut receipt = plan.begin_terminal_receipt().unwrap();
            receipt.record(&first);
            receipt.record(&second);
            assert!(!plan.record_terminal(receipt));
        }
        let mut foreign = QuadrantChartTextPaintReceipt {
            expected: plan.terminals.to_vec().into(),
            next: 0,
            valid: true,
        };
        foreign.record(&label("Point", "#123456"));
        foreign.record(&label("Title", "#abcdef"));
        assert!(!plan.record_terminal(foreign));
        let mut complete = plan.begin_terminal_receipt().unwrap();
        complete.record(&label("Point", "#123456"));
        complete.record(&label("Title", "#abcdef"));
        assert!(plan.record_terminal(complete));
        assert_eq!(plan.finish_evidence().applied().len(), 1);
    }
}
