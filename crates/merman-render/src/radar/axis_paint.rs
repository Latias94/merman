use std::collections::BTreeMap;
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::config::value_at;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    TerminalVariantDomain, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_fill, resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::OperationWorkMeter;

/// Radar historically projects one stroke-or-fill Axis value into three CSS properties.
/// Keep that precedence while independently respecting source ownership of both config channels.
#[derive(Debug)]
pub(crate) struct RadarAxisPaintPlan {
    line_color: Box<str>,
    axis_color: Box<str>,
    axis_count: usize,
    paint: Option<DirectStaticPaint>,
    pending_rule: Option<usize>,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<()>,
}

#[derive(Default)]
struct RuleObservation {
    pending: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

impl RadarAxisPaintPlan {
    #[cfg(test)]
    pub(crate) fn baseline(config: &MermaidConfig, axis_count: usize) -> Self {
        Self::baseline_with_binding(
            &super::RadarCssBinding::resolve(config.as_value()),
            axis_count,
        )
    }

    pub(crate) fn baseline_with_binding(
        binding: &super::RadarCssBinding,
        axis_count: usize,
    ) -> Self {
        Self {
            line_color: binding.line_color.clone().into_boxed_str(),
            axis_color: binding.axis_color.clone().into_boxed_str(),
            axis_count,
            paint: None,
            pending_rule: None,
            evidence: FamilyThemeEvidence::default(),
            terminal_receipt: OnceLock::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        axis_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let binding = super::RadarCssBinding::resolve(config.as_value());
        Self::resolve_with_binding(theme, config, &binding, axis_count, work_meter)
    }

    pub(crate) fn resolve_with_binding(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        binding: &super::RadarCssBinding,
        axis_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut plan = Self::baseline_with_binding(binding, axis_count);
        let Some(theme) = theme else { return Ok(plan) };
        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let owns_line = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.lineColor",
        );
        let axis_path = if value_at(config.as_value(), &["radar", "axisColor"])
            .and_then(serde_json::Value::as_str)
            .is_some()
        {
            "radar.axisColor"
        } else {
            "themeVariables.radar.axisColor"
        };
        let owns_axis =
            merman_core::__private::config_path_overrides_typed_default(config, axis_path);
        // Base lineColor styles marker classes that Radar never instantiates. Retain the
        // historical CSS projection, but only axisColor has actual paint terminals here.
        let source_owns_paint = owns_axis;
        let static_style = theme.style_with_work_meter(
            ThemeTarget::Axis,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        // Clear and unsupported strokes also suppress the fill fallback.
        plan.paint = if static_style.stroke_resolution().winner().is_some() {
            resolve_direct_static_stroke(
                theme,
                &static_style,
                &[ThemeTarget::Axis],
                DirectStaticSelectorDomain::Default,
            )
        } else {
            resolve_direct_static_fill(
                theme,
                &static_style,
                &[ThemeTarget::Axis],
                DirectStaticSelectorDomain::Default,
            )
        };
        if let Some(paint) = &plan.paint {
            if !owns_line {
                plan.line_color = paint.css().into();
            }
            if !owns_axis {
                plan.axis_color = paint.css().into();
            }
        }

        let mut observations = BTreeMap::<usize, RuleObservation>::new();
        let routes = theme
            .family_mechanism_routes()
            .iter()
            .copied()
            .filter(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Axis,
                        ..
                    }
                )
            })
            .collect::<Vec<_>>();
        for route in &routes {
            if let FamilyThemeMechanism::RuleFacet { rule_index, .. } = route.mechanism() {
                observations.entry(rule_index).or_default();
            }
        }
        // Static CSS shares one winner across every axis. Only ordinal selectors need a
        // per-axis reconciliation pass, even though their unsupported outcomes remain residual.
        let has_ordinal_rules = routes.iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    selector: FamilyThemeSelectorShape::Ordinal { .. },
                    ..
                }
            )
        });
        // Stroke-or-fill selection suppresses the palette fallback even when the chosen
        // stroke is Clear or Unsupported. A static stroke survives at every occurrence;
        // otherwise collect only the axes where an ordinal stroke actually wins.
        let mut fill_overridden =
            if source_owns_paint || static_style.stroke_resolution().winner().is_some() {
                work_meter.charge(axis_count)?;
                Some(vec![true; axis_count])
            } else {
                None
            };
        let observations_count = if routes.is_empty() {
            0
        } else if has_ordinal_rules {
            axis_count
        } else {
            usize::from(axis_count != 0)
        };
        for ordinal in 1..=observations_count {
            let ordinal_style;
            let style = if has_ordinal_rules {
                ordinal_style = theme.style_with_work_meter(
                    ThemeTarget::Axis,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work_meter,
                )?;
                &ordinal_style
            } else {
                &static_style
            };
            if has_ordinal_rules && style.stroke_resolution().winner().is_some() {
                if fill_overridden.is_none() {
                    work_meter.charge(axis_count)?;
                    fill_overridden = Some(vec![false; axis_count]);
                }
                fill_overridden
                    .as_mut()
                    .expect("allocated stroke fallback mask")[ordinal - 1] = true;
            }
            work_meter.charge(routes.len())?;
            for route in &routes {
                let FamilyThemeMechanism::RuleFacet {
                    rule_index, facet, ..
                } = route.mechanism()
                else {
                    unreachable!()
                };
                let property = resolved_style_property_for_facet(facet);
                if !style.winner_rule_properties().any(|(candidate, origin)| {
                    candidate == property && origin.rule_index() == rule_index
                }) {
                    continue;
                }
                let is_paint = matches!(
                    property,
                    ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke
                );
                if is_paint
                    && (source_owns_paint
                        || (property == ResolvedStyleProperty::Fill
                            && style.stroke_resolution().winner().is_some()))
                {
                    continue;
                }
                let observation = observations
                    .get_mut(&rule_index)
                    .expect("registered Axis route");
                match route.disposition() {
                    FamilyThemeDisposition::Unsupported => {
                        observation
                            .residual
                            .get_or_insert(unsupported_residual_for_facet(facet));
                    }
                    FamilyThemeDisposition::TypedAdapter
                        if is_paint
                            && plan
                                .paint
                                .as_ref()
                                .is_some_and(|paint| paint.rule_index() == rule_index) =>
                    {
                        observation.pending = true;
                    }
                    _ => observation.incomplete = true,
                }
            }
        }
        let mut fallback_domain = UnsupportedTerminalDomain::fallbacks_only(
            ThemeTarget::Axis,
            TerminalVariantDomain::uniform(axis_count, ThemeVariant::Default),
        );
        if let Some(overridden) = fill_overridden.as_deref() {
            // Rule facets are owned above. For fallback reconciliation, either the source
            // or Radar's selected stroke owns the paint channel at these occurrences.
            fallback_domain = fallback_domain.with_source_owned_fill(overridden);
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[fallback_domain],
            work_meter,
        )?;
        for (index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::Axis,
            };
            if let Some(reason) = observation.residual {
                plan.evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // A sibling facet without a terminal owner must remain incomplete.
            } else if observation.pending {
                plan.pending_rule = Some(index);
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        Ok(plan)
    }

    pub(crate) fn line_color(&self) -> &str {
        &self.line_color
    }
    pub(crate) fn axis_color(&self) -> &str {
        &self.axis_color
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<RadarAxisPaintReceipt> {
        self.pending_rule.map(|_| RadarAxisPaintReceipt::new(self))
    }

    pub(crate) fn record_terminal(&self, receipt: RadarAxisPaintReceipt) -> bool {
        self.pending_rule.is_some() && receipt.proves(self) && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(index) = self.pending_rule
            && self.terminal_receipt.get().is_some()
            && let Some(paint) = &self.paint
        {
            evidence.mark_applied_with_capabilities(
                FamilyThemeMechanismKey::Rule {
                    index,
                    target: ThemeTarget::Axis,
                },
                [paint.capability()],
            );
        }
        evidence
    }
}

/// One completed CSS pass and both terminal classes for every actual axis.
#[derive(Debug)]
pub(crate) struct RadarAxisPaintReceipt {
    expected_line: Box<str>,
    expected_axis: Box<str>,
    css: [bool; 3],
    next_line: usize,
    next_label: usize,
    valid: bool,
}

impl RadarAxisPaintReceipt {
    fn new(plan: &RadarAxisPaintPlan) -> Self {
        Self {
            expected_line: plan.line_color.clone(),
            expected_axis: plan.axis_color.clone(),
            css: [false; 3],
            next_line: 0,
            next_label: 0,
            valid: true,
        }
    }

    pub(crate) fn record_base_line_css(&mut self, value: &str) {
        self.valid &= !self.css[0]
            && self.next_line == 0
            && self.next_label == 0
            && value == self.expected_line.as_ref();
        self.css[0] = true;
    }
    pub(crate) fn record_axis_line_css(&mut self, class: &str, stroke: &str) {
        self.valid &= !self.css[1]
            && self.css[0]
            && self.next_line == 0
            && self.next_label == 0
            && class == "radarAxisLine"
            && stroke == self.expected_axis.as_ref();
        self.css[1] = true;
    }
    pub(crate) fn record_axis_label_css(&mut self, class: &str, color: &str) {
        self.valid &= !self.css[2]
            && self.css[1]
            && self.next_line == 0
            && self.next_label == 0
            && class == "radarAxisLabel"
            && color == self.expected_axis.as_ref();
        self.css[2] = true;
    }
    pub(crate) fn record_axis_line(&mut self, index: usize, class: &str) {
        self.valid &= self.css == [true; 3]
            && index == self.next_line
            && self.next_line == self.next_label
            && class == "radarAxisLine";
        self.next_line = self.next_line.saturating_add(1);
    }
    pub(crate) fn record_axis_label(&mut self, index: usize, class: &str) {
        self.valid &= self.css == [true; 3]
            && index == self.next_label
            && self.next_line == self.next_label.saturating_add(1)
            && class == "radarAxisLabel";
        self.next_label = self.next_label.saturating_add(1);
    }
    fn proves(&self, plan: &RadarAxisPaintPlan) -> bool {
        self.valid
            && self.css == [true; 3]
            && plan.axis_count > 0
            && self.next_line == plan.axis_count
            && self.next_label == plan.axis_count
            && self.expected_line == plan.line_color
            && self.expected_axis == plan.axis_color
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(plan: &RadarAxisPaintPlan) -> RadarAxisPaintReceipt {
        let mut receipt = RadarAxisPaintReceipt::new(plan);
        receipt.record_base_line_css(plan.line_color());
        receipt.record_axis_line_css("radarAxisLine", plan.axis_color());
        receipt.record_axis_label_css("radarAxisLabel", plan.axis_color());
        receipt
    }

    fn axis_rule(patch: crate::diagram_theme::ThemeStylePatch) -> crate::diagram_theme::ThemeRule {
        crate::diagram_theme::ThemeRule::new(ThemeTarget::Axis, patch)
            .for_family(crate::DiagramFamilyId::RADAR)
    }

    fn plan_for_rules(
        rules: impl IntoIterator<Item = crate::diagram_theme::ThemeRule>,
        config: MermaidConfig,
        count: usize,
    ) -> RadarAxisPaintPlan {
        use crate::diagram_theme::{DiagramThemeCompiler, DiagramThemeSpec, ThemeRuleSet};
        let rules = rules
            .into_iter()
            .fold(ThemeRuleSet::default(), |set, rule| set.with_rule(rule));
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap()
            .resolve(crate::DiagramFamilyId::RADAR);
        let meter = OperationWorkMeter::new(crate::RenderResourcePolicy::interactive());
        RadarAxisPaintPlan::resolve(Some(&theme), &config, count, &meter).unwrap()
    }

    fn effective_config(site_config: serde_json::Value) -> MermaidConfig {
        merman_core::Engine::new()
            .with_site_config(MermaidConfig::from_value(site_config))
            .parse_metadata_sync("radar-beta\naxis A,B,C\ncurve Current{1,2,3}\n")
            .expect("parse Radar config ownership fixture")
            .effective_config
    }

    #[test]
    fn source_owned_axis_is_not_applied_through_unused_marker_css() {
        use crate::diagram_theme::{CanvasPaint, ThemeStylePatch};
        let rule = axis_rule(
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123456").unwrap()),
        );
        for config in [
            serde_json::json!({"radar":{"axisColor":"#abcdef"}}),
            serde_json::json!({"themeVariables":{"radar":{"axisColor":"#abcdef"}}}),
        ] {
            let plan = plan_for_rules([rule.clone()], effective_config(config), 3);
            assert_eq!(plan.line_color(), "#123456");
            assert_eq!(plan.axis_color(), "#abcdef");
            assert!(plan.begin_terminal_receipt().is_none());
            assert_eq!(plan.finish_evidence().not_applicable_mechanisms().len(), 1);
            assert!(plan.finish_evidence().applied().is_empty());
        }
    }

    #[test]
    fn clear_stroke_blocks_fill_without_certifying_either_facet() {
        use crate::diagram_theme::{CanvasPaint, Specified, ThemeStylePatch};
        let fill =
            axis_rule(ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()));
        let mut clear = ThemeStylePatch::default();
        clear.stroke.paint = Specified::Clear;
        for source_owns in [false, true] {
            let config = if source_owns {
                effective_config(serde_json::json!({"radar":{"axisColor":"#abcdef"}}))
            } else {
                MermaidConfig::empty_object()
            };
            let plan = plan_for_rules([fill.clone(), axis_rule(clear.clone())], config, 3);
            assert_ne!(plan.axis_color(), "#123456");
            assert_ne!(plan.line_color(), "#123456");
            assert!(plan.begin_terminal_receipt().is_none());
            let evidence = plan.finish_evidence();
            assert!(evidence.applied().is_empty());
            assert_eq!(
                evidence.not_applicable_mechanisms().len(),
                if source_owns { 2 } else { 1 }
            );
            assert_eq!(evidence.residuals().len(), usize::from(!source_owns));
        }
    }

    #[test]
    fn ordinal_winners_keep_residuals_and_only_uncovered_static_winners_are_pending() {
        use crate::diagram_theme::{CanvasPaint, OrdinalSelector, ThemeStylePatch};
        let static_rule = axis_rule(
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123456").unwrap()),
        );
        for (ordinal, static_pending) in [
            (
                OrdinalSelector::Cycle {
                    period: 1,
                    offset: 0,
                },
                false,
            ),
            (OrdinalSelector::Exact(2), true),
        ] {
            let dynamic = axis_rule(
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#abcdef").unwrap()),
            )
            .with_ordinal(ordinal);
            let plan = plan_for_rules(
                [static_rule.clone(), dynamic],
                MermaidConfig::empty_object(),
                3,
            );
            // Best-effort CSS retains its static fallback; this is not proof of the ordinal request.
            assert_eq!(plan.axis_color(), "#123456");
            assert_eq!(plan.begin_terminal_receipt().is_some(), static_pending);
            let evidence = plan.finish_evidence();
            assert_eq!(evidence.residuals().len(), 1);
            assert_eq!(
                evidence.not_applicable_mechanisms().len(),
                usize::from(!static_pending)
            );
            assert!(evidence.applied().is_empty());
        }
    }

    fn plan_with_axis_palette(rule: crate::diagram_theme::ThemeRule) -> RadarAxisPaintPlan {
        use crate::diagram_theme::{
            DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue, ThemeRuleSet,
        };
        let palette = OrdinalPalette::new([ThemeColorValue::parse("#fedcba").unwrap()]).unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(rule)
                        .with_ordinal_palette(ThemeTarget::Axis, palette),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::RADAR);
        RadarAxisPaintPlan::resolve(
            Some(&theme),
            &MermaidConfig::empty_object(),
            3,
            &OperationWorkMeter::new(crate::RenderResourcePolicy::interactive()),
        )
        .unwrap()
    }

    #[test]
    fn axis_palette_is_not_applicable_when_static_stroke_owns_every_axis() {
        use crate::diagram_theme::{CanvasPaint, ThemeStylePatch};
        let plan = plan_with_axis_palette(axis_rule(
            ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123456").unwrap()),
        ));
        assert!(plan.begin_terminal_receipt().is_some());
        let palette_key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Axis,
        };
        assert!(
            plan.finish_evidence()
                .not_applicable_mechanisms()
                .contains(&palette_key)
        );
        assert!(plan.finish_evidence().residuals().is_empty());
        let mut terminal = receipt(&plan);
        for index in 0..3 {
            terminal.record_axis_line(index, "radarAxisLine");
            terminal.record_axis_label(index, "radarAxisLabel");
        }
        assert!(plan.record_terminal(terminal));
        assert_eq!(plan.finish_evidence().applied().len(), 1);
        assert!(plan.finish_evidence().residuals().is_empty());
    }

    #[test]
    fn axis_palette_is_covered_by_clear_and_unsupported_static_strokes() {
        use crate::diagram_theme::{
            CanvasPaint, GradientStop, LinearGradient, Specified, ThemeColorValue, ThemeStylePatch,
        };
        let mut clear = ThemeStylePatch::default();
        clear.stroke.paint = Specified::Clear;
        let gradient = ThemeStylePatch::default().with_stroke(CanvasPaint::LinearGradient(
            LinearGradient::new(
                90.0,
                [
                    GradientStop::new(0.0, ThemeColorValue::parse("#123456").unwrap()).unwrap(),
                    GradientStop::new(1.0, ThemeColorValue::parse("#abcdef").unwrap()).unwrap(),
                ],
            )
            .unwrap(),
        ));
        for patch in [clear, gradient] {
            let plan = plan_with_axis_palette(axis_rule(patch));
            assert!(plan.begin_terminal_receipt().is_none());
            let evidence = plan.finish_evidence();
            assert!(evidence.not_applicable_mechanisms().contains(
                &FamilyThemeMechanismKey::OrdinalPalette {
                    target: ThemeTarget::Axis
                }
            ));
            assert_eq!(evidence.residuals().len(), 1);
            assert_eq!(
                evidence.residuals()[0].key(),
                &FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Axis
                }
            );
        }
    }

    #[test]
    fn axis_palette_remains_residual_on_axes_without_an_ordinal_stroke_winner() {
        use crate::diagram_theme::{CanvasPaint, OrdinalSelector, ThemeStylePatch};
        let plan = plan_with_axis_palette(
            axis_rule(
                ThemeStylePatch::default().with_stroke(CanvasPaint::solid("#123456").unwrap()),
            )
            .with_ordinal(OrdinalSelector::Exact(2)),
        );
        let evidence = plan.finish_evidence();
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert_eq!(evidence.residuals().len(), 2);
        assert!(evidence.residuals().iter().any(|residual| residual.key()
            == &FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Axis
            }));
        assert!(plan.begin_terminal_receipt().is_none());
    }

    #[test]
    fn axis_paint_receipt_requires_all_css_and_both_terminal_channels() {
        let plan = RadarAxisPaintPlan::baseline(&MermaidConfig::empty_object(), 1);
        let mut complete = receipt(&plan);
        complete.record_axis_line(0, "radarAxisLine");
        assert!(!complete.proves(&plan));
        complete.record_axis_label(0, "radarAxisLabel");
        assert!(complete.proves(&plan));
        complete.record_axis_label(0, "radarAxisLabel");
        assert!(!complete.proves(&plan));
        let mut missing_css = RadarAxisPaintReceipt::new(&plan);
        missing_css.record_axis_line(0, "radarAxisLine");
        missing_css.record_axis_label(0, "radarAxisLabel");
        assert!(!missing_css.proves(&plan));
        let mut label_only = receipt(&plan);
        label_only.record_axis_label(0, "radarAxisLabel");
        assert!(!label_only.proves(&plan));
        let empty = RadarAxisPaintPlan::baseline(&MermaidConfig::empty_object(), 0);
        assert!(!receipt(&empty).proves(&empty));
    }

    #[test]
    fn axis_paint_receipt_rejects_wrong_values_and_duplicate_css() {
        let plan = RadarAxisPaintPlan::baseline(&MermaidConfig::empty_object(), 1);
        for wrong_channel in 0..3 {
            let mut bad = RadarAxisPaintReceipt::new(&plan);
            bad.record_base_line_css(if wrong_channel == 0 {
                "wrong"
            } else {
                plan.line_color()
            });
            bad.record_axis_line_css(
                "radarAxisLine",
                if wrong_channel == 1 {
                    "wrong"
                } else {
                    plan.axis_color()
                },
            );
            bad.record_axis_label_css(
                "radarAxisLabel",
                if wrong_channel == 2 {
                    "wrong"
                } else {
                    plan.axis_color()
                },
            );
            bad.record_axis_line(0, "radarAxisLine");
            bad.record_axis_label(0, "radarAxisLabel");
            assert!(!bad.proves(&plan));
        }
        let mut duplicate = receipt(&plan);
        duplicate.record_axis_label_css("radarAxisLabel", plan.axis_color());
        duplicate.record_axis_line(0, "radarAxisLine");
        duplicate.record_axis_label(0, "radarAxisLabel");
        assert!(!duplicate.proves(&plan));
    }
}
