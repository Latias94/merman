use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, Specified,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::{
    QuadrantChartBorderLineData, QuadrantChartDiagramLayout, QuadrantChartTextData,
};
use crate::resources::OperationWorkMeter;

/// Text.fill historically feeds point labels, axis labels, and the title, in author order
/// with the Axis/Title rules. Axis stroke (or its fill fallback) feeds the borders.
/// Quadrant captions have separate Mermaid color channels.
#[derive(Debug, Default)]
pub(crate) struct QuadrantChartPaintPlan {
    terminals: Arc<[(Box<str>, Box<str>)]>,
    pending: BTreeMap<(ThemeTarget, usize), BTreeSet<ThemeCapability>>,
    borders: Arc<[BorderTerminal]>,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<()>,
}

#[derive(Default)]
struct RuleObservation {
    pending: BTreeSet<ThemeCapability>,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

impl QuadrantChartPaintPlan {
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
                        target: ThemeTarget::Text | ThemeTarget::Title | ThemeTarget::Axis,
                        ..
                    } | FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Text | ThemeTarget::Axis
                    } | FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Text | ThemeTarget::Axis,
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
        let mut palette_targets = BTreeSet::new();
        let mut effect_targets = BTreeSet::new();
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
                        &[ThemeTarget::Text, ThemeTarget::Title, ThemeTarget::Axis],
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
                if !source_owned
                    && matches!(style.fill_resolution().specified(), Specified::Unspecified)
                {
                    palette_targets.insert(ThemeTarget::Text);
                    if target == ThemeTarget::Axis {
                        palette_targets.insert(target);
                    }
                }
                if matches!(
                    style.effect_resolution().specified(),
                    Specified::Unspecified
                ) {
                    effect_targets.insert(ThemeTarget::Text);
                    if target == ThemeTarget::Axis {
                        effect_targets.insert(target);
                    }
                }
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
                    // Axis stroke is consumed by borders, independently of text inheritance.
                    if route_target == ThemeTarget::Axis
                        && property == ResolvedStyleProperty::Stroke
                    {
                        continue;
                    }
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
                            observation
                                .pending
                                .insert(paint.as_ref().expect("matched paint").capability());
                        }
                        _ => observation.incomplete = true,
                    }
                }
            }
        }
        // Borders have their own Axis occurrence domain in writer order: four outer sides,
        // then the two dividers. Text inheritance never supplies their fill fallback.
        let axis_style = theme.style_with_work_meter(
            ThemeTarget::Axis,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let static_fill_fallback = matches!(
            axis_style.stroke_resolution().specified(),
            Specified::Unspecified
        );
        let border_paint = if static_fill_fallback {
            resolve_direct_static_fill(
                theme,
                &axis_style,
                &[ThemeTarget::Axis],
                DirectStaticSelectorDomain::Default,
            )
        } else {
            resolve_direct_static_stroke(
                theme,
                &axis_style,
                &[ThemeTarget::Axis],
                DirectStaticSelectorDomain::Default,
            )
        };
        let mut borders = Vec::with_capacity(layout.border_lines.len());
        work_meter.charge(layout.border_lines.len())?;
        for (index, line) in layout.border_lines.iter_mut().enumerate() {
            let path = if index < 4 {
                "themeVariables.quadrantExternalBorderStrokeFill"
            } else {
                "themeVariables.quadrantInternalBorderStrokeFill"
            };
            let source_owned =
                merman_core::__private::config_path_overrides_typed_default(config, path);
            if !source_owned {
                if let Some(paint) = &border_paint {
                    line.stroke_fill = paint.css().to_owned();
                }
            }
            borders.push(BorderTerminal::from_line(line));
            if line.stroke_width <= 0.0 || (line.x1 == line.x2 && line.y1 == line.y2) {
                continue;
            }
            let dynamic_style;
            let style = if has_ordinal {
                dynamic_style = theme.style_with_work_meter(
                    ThemeTarget::Axis,
                    ThemeVariant::Default,
                    Some(index + 1),
                    work_meter,
                )?;
                &dynamic_style
            } else {
                &axis_style
            };
            let paint_property = if matches!(
                style.stroke_resolution().specified(),
                Specified::Unspecified
            ) {
                ResolvedStyleProperty::Fill
            } else {
                ResolvedStyleProperty::Stroke
            };
            if !source_owned
                && paint_property == ResolvedStyleProperty::Fill
                && matches!(style.fill_resolution().specified(), Specified::Unspecified)
            {
                palette_targets.insert(ThemeTarget::Axis);
            }
            if matches!(
                style.effect_resolution().specified(),
                Specified::Unspecified
            ) {
                effect_targets.insert(ThemeTarget::Axis);
            }
            work_meter.charge(routes.len())?;
            for route in &routes {
                let FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Axis,
                    rule_index,
                    facet,
                    ..
                } = route.mechanism()
                else {
                    continue;
                };
                let property = resolved_style_property_for_facet(facet);
                if matches!(
                    property,
                    ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke
                ) && (property != paint_property || source_owned)
                {
                    continue;
                }
                if !style.winner_rule_properties().any(|(candidate, origin)| {
                    candidate == property && origin.rule_index() == rule_index
                }) {
                    continue;
                }
                let observation = observations
                    .get_mut(&(ThemeTarget::Axis, rule_index))
                    .expect("registered axis rule");
                match route.disposition() {
                    FamilyThemeDisposition::Unsupported => {
                        observation
                            .residual
                            .get_or_insert(unsupported_residual_for_facet(facet));
                    }
                    FamilyThemeDisposition::TypedAdapter
                        if property == paint_property
                            && border_paint
                                .as_ref()
                                .is_some_and(|paint| paint.rule_index() == rule_index) =>
                    {
                        observation.pending.insert(
                            border_paint
                                .as_ref()
                                .expect("matched border paint")
                                .capability(),
                        );
                    }
                    _ => observation.incomplete = true,
                }
            }
        }
        plan.borders = borders.into();
        for route in &routes {
            let (applies, reason) = match route.mechanism() {
                FamilyThemeMechanism::OrdinalPalette { target } => (
                    palette_targets.contains(&target),
                    FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                ),
                FamilyThemeMechanism::EffectBinding { target, .. } => (
                    effect_targets.contains(&target),
                    FamilyThemeResidualReason::UnsupportedEffect,
                ),
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
            } else if !observation.pending.is_empty() {
                plan.pending.insert((target, index), observation.pending);
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        plan.terminals = terminals.into();
        Ok(plan)
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<QuadrantChartPaintReceipt> {
        (!self.pending.is_empty()).then(|| QuadrantChartPaintReceipt {
            expected: Arc::clone(&self.terminals),
            expected_borders: Arc::clone(&self.borders),
            next_border: 0,
            next: 0,
            valid: true,
        })
    }

    pub(crate) fn record_terminal(&self, receipt: QuadrantChartPaintReceipt) -> bool {
        Arc::ptr_eq(&self.terminals, &receipt.expected)
            && Arc::ptr_eq(&self.borders, &receipt.expected_borders)
            && receipt.next_border == self.borders.len()
            && receipt.valid
            && receipt.next == self.terminals.len()
            && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if self.terminal_receipt.get().is_some() {
            for (&(target, index), capabilities) in &self.pending {
                evidence.mark_applied_with_capabilities(
                    FamilyThemeMechanismKey::Rule { index, target },
                    capabilities.iter().copied(),
                );
            }
        }
        evidence
    }
}

#[derive(Debug)]
pub(crate) struct QuadrantChartPaintReceipt {
    expected: Arc<[(Box<str>, Box<str>)]>,
    expected_borders: Arc<[BorderTerminal]>,
    next_border: usize,
    next: usize,
    valid: bool,
}

impl QuadrantChartPaintReceipt {
    pub(crate) fn record_border(&mut self, line: &QuadrantChartBorderLineData) {
        self.valid &= self
            .expected_borders
            .get(self.next_border)
            .is_some_and(|expected| expected.matches(line));
        self.next_border = self.next_border.saturating_add(1);
    }

    pub(crate) fn record(&mut self, label: &QuadrantChartTextData) {
        self.valid &= self
            .expected
            .get(self.next)
            .is_some_and(|(text, fill)| text.as_ref() == label.text && fill.as_ref() == label.fill);
        self.next = self.next.saturating_add(1);
    }
}

#[derive(Debug)]
struct BorderTerminal {
    geometry: [f64; 5],
    paint: Box<str>,
}

impl BorderTerminal {
    fn from_line(line: &QuadrantChartBorderLineData) -> Self {
        Self {
            geometry: [line.x1, line.y1, line.x2, line.y2, line.stroke_width],
            paint: line.stroke_fill.clone().into_boxed_str(),
        }
    }

    fn matches(&self, line: &QuadrantChartBorderLineData) -> bool {
        self.geometry == [line.x1, line.y1, line.x2, line.y2, line.stroke_width]
            && self.paint.as_ref() == line.stroke_fill
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

    fn plan() -> QuadrantChartPaintPlan {
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
        QuadrantChartPaintPlan {
            evidence: FamilyThemeEvidence::from_theme(Some(&theme)),
            terminals: vec![
                ("Point".into(), "#123456".into()),
                ("Title".into(), "#abcdef".into()),
            ]
            .into(),
            pending: BTreeMap::from([(
                (ThemeTarget::Text, 0),
                BTreeSet::from([ThemeCapability::SolidPaint]),
            )]),
            ..QuadrantChartPaintPlan::default()
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
        let mut foreign = QuadrantChartPaintReceipt {
            expected: plan.terminals.to_vec().into(),
            expected_borders: Arc::clone(&plan.borders),
            next_border: 0,
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
    #[test]
    fn border_receipt_rejects_missing_reordered_changed_and_foreign_terminals() {
        let mut plan = plan();
        let line = |x| QuadrantChartBorderLineData {
            x1: x,
            y1: 0.0,
            x2: x + 1.0,
            y2: 0.0,
            stroke_width: 2.0,
            stroke_fill: "#123456".into(),
        };
        plan.borders = vec![
            BorderTerminal::from_line(&line(0.0)),
            BorderTerminal::from_line(&line(2.0)),
        ]
        .into();
        let text_complete = || {
            let mut receipt = plan.begin_terminal_receipt().unwrap();
            receipt.record(&label("Point", "#123456"));
            receipt.record(&label("Title", "#abcdef"));
            receipt
        };
        assert!(!plan.record_terminal(text_complete()));
        let mut wrong_paint = line(0.0);
        wrong_paint.stroke_fill = "#abcdef".into();
        let mut wrong_width = line(0.0);
        wrong_width.stroke_width = 3.0;
        for first in [line(2.0), wrong_paint, wrong_width] {
            let mut receipt = text_complete();
            receipt.record_border(&first);
            receipt.record_border(&line(2.0));
            assert!(!plan.record_terminal(receipt));
        }
        let mut foreign = text_complete();
        foreign.expected_borders = vec![
            BorderTerminal::from_line(&line(0.0)),
            BorderTerminal::from_line(&line(2.0)),
        ]
        .into();
        foreign.record_border(&line(0.0));
        foreign.record_border(&line(2.0));
        assert!(!plan.record_terminal(foreign));
        let mut complete = text_complete();
        complete.record_border(&line(0.0));
        complete.record_border(&line(2.0));
        assert!(plan.record_terminal(complete));
        assert_eq!(plan.finish_evidence().applied().len(), 1);
    }
}
