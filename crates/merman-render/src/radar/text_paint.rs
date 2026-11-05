use std::collections::BTreeMap;
use std::sync::OnceLock;

use merman_model::families::radar::RadarDiagramModel;
use merman_model::presentation::PresentationConfig;

use super::paint::{PaintRules, PendingPaintRules, record_pending, static_paint};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence,
    FamilyThemeOutcomes, FamilyThemeResidualReason, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::RadarDiagramLayout;
use crate::resources::OperationWorkMeter;

/// Generic text paint owns inherited axis/legend fill and the unclaimed title fallback.
#[derive(Debug)]
pub(crate) struct RadarTextPaintPlan {
    text_color: Box<str>,
    title_color: Box<str>,
    axis_visible: Box<[bool]>,
    legend_visible: Box<[bool]>,
    title_visible: bool,
    paint: Option<DirectStaticPaint>,
    pending_rule: Option<usize>,
    title_fallback_rule: Option<usize>,
    legend_paints: Vec<Option<DirectPaintExpectation>>,
    legend_rules: PendingPaintRules,
    evidence: FamilyThemeOutcomes,
    terminal_receipt: OnceLock<()>,
}

#[derive(Default)]
struct RuleObservation {
    pending: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

impl RadarTextPaintPlan {
    pub(crate) fn baseline_with_binding(
        binding: &super::RadarCssBinding,
        layout: &RadarDiagramLayout,
        model: &RadarDiagramModel,
        title: Option<&str>,
    ) -> Self {
        Self {
            text_color: binding.text_color.clone().into_boxed_str(),
            title_color: binding.title_color.clone().into_boxed_str(),
            axis_visible: layout
                .axes
                .iter()
                .map(|axis| !axis.label.trim().is_empty())
                .collect(),
            legend_visible: layout
                .legend_items
                .iter()
                .map(|item| {
                    model
                        .curves
                        .get(item.class_index as usize)
                        .is_some_and(|curve| !curve.label.trim().is_empty())
                })
                .collect(),
            title_visible: title.is_some_and(|title| !title.trim().is_empty()),
            paint: None,
            pending_rule: None,
            title_fallback_rule: None,
            legend_paints: vec![None; layout.legend_items.len()],
            legend_rules: PendingPaintRules::new(),
            evidence: FamilyThemeOutcomes::default(),
            terminal_receipt: OnceLock::new(),
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "Prepared paint and its evidence retain separate owners."
    )]
    pub(crate) fn resolve_with_binding(
        theme: Option<&ResolvedDiagramTheme>,
        config: &PresentationConfig,
        binding: &super::RadarCssBinding,
        layout: &RadarDiagramLayout,
        model: &RadarDiagramModel,
        title: Option<&str>,
        title_plan: &super::RadarTitleThemePlan,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        work_meter.charge(layout.axes.len().saturating_add(layout.legend_items.len()))?;
        let mut plan = Self::baseline_with_binding(binding, layout, model, title);
        let Some(theme) = theme else { return Ok(plan) };
        plan.evidence = FamilyThemeOutcomes::default();
        let owns_text = config.config_paint_overrides_typed_default("themeVariables.textColor");
        let title_style = theme.style_with_work_meter(
            ThemeTarget::Title,
            ThemeVariant::Default,
            Some(1),
            work_meter,
        )?;
        let owns_title = config.config_paint_overrides_typed_default("themeVariables.titleColor")
            || title_style.fill_resolution().winner().is_some();
        let static_style = theme.style_with_work_meter(
            ThemeTarget::Text,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        plan.paint = resolve_direct_static_fill(
            theme,
            &static_style,
            &[ThemeTarget::Text],
            DirectStaticSelectorDomain::Default,
        );
        if let Some(paint) = &plan.paint {
            if !owns_text {
                plan.text_color = paint.css().into();
            }
            if !owns_title {
                plan.title_color = paint.css().into();
            }
        }
        if let Some(fill) = title_plan.fill_css() {
            plan.title_color = fill.into();
        }
        let mut legend_rules = PaintRules::new(theme, ThemeTarget::Legend);
        let mut legend_occurrences = Vec::new();
        let mut legend_fill_owned = vec![owns_text; layout.legend_items.len()];
        let mut text_owners = plan
            .axis_visible
            .iter()
            .filter(|visible| **visible)
            .map(|_| owns_text)
            .collect::<Vec<_>>();
        for (index, visible) in plan.legend_visible.iter().copied().enumerate() {
            if !visible {
                continue;
            }
            legend_occurrences.push((index + 1, ThemeVariant::Default));
            let style = theme.style_with_work_meter(
                ThemeTarget::Legend,
                ThemeVariant::Default,
                Some(index + 1),
                work_meter,
            )?;
            if !owns_text {
                plan.legend_paints[index] = static_paint(
                    theme,
                    style.fill_resolution(),
                    ThemeTarget::Legend,
                    ResolvedStyleProperty::Fill,
                    &binding.text_color,
                );
            }
            legend_rules.observe(
                &style,
                [owns_text, false],
                [plan.legend_paints[index].as_ref(), None],
                work_meter,
            )?;
            let role_owns_fill = style.fill_resolution().winner().is_some();
            text_owners.push(owns_text || role_owns_fill);
            // A winning Text fill already paints unclaimed legend labels. It does not
            // consume a dedicated Legend palette or an independent effect request.
            legend_fill_owned[index] |= !role_owns_fill && plan.paint.is_some();
        }
        plan.legend_rules = legend_rules.finish(&mut plan.evidence);
        let base_count = text_owners.len();
        let visible_count = base_count.saturating_add(usize::from(plan.title_visible));
        if plan.title_visible {
            text_owners.push(owns_title);
        }
        let routes = theme
            .family_mechanism_routes()
            .iter()
            .copied()
            .filter(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Text,
                        ..
                    }
                )
            })
            .collect::<Vec<_>>();
        let mut observations = BTreeMap::<usize, RuleObservation>::new();
        for route in &routes {
            if let FamilyThemeMechanism::RuleFacet { rule_index, .. } = route.mechanism() {
                observations.entry(rule_index).or_default();
            }
        }
        let has_ordinal = routes.iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    selector: FamilyThemeSelectorShape::Ordinal { .. },
                    ..
                }
            )
        });
        for (index, source_owns_fill) in text_owners.iter().copied().enumerate() {
            let is_title = plan.title_visible && index == base_count;
            let ordinal_style;
            let style = if has_ordinal {
                ordinal_style = theme.style_with_work_meter(
                    ThemeTarget::Text,
                    ThemeVariant::Default,
                    Some(index + 1),
                    work_meter,
                )?;
                &ordinal_style
            } else {
                &static_style
            };
            work_meter.charge(routes.len())?;
            for route in &routes {
                let FamilyThemeMechanism::RuleFacet {
                    rule_index, facet, ..
                } = route.mechanism()
                else {
                    unreachable!()
                };
                let property = resolved_style_property_for_facet(facet);
                if property == ResolvedStyleProperty::Fill && source_owns_fill {
                    continue;
                }
                if !style.winner_rule_properties().any(|(candidate, origin)| {
                    candidate == property && origin.rule_index() == rule_index
                }) {
                    continue;
                }
                let observation = observations
                    .get_mut(&rule_index)
                    .expect("registered Radar text rule");
                match route.disposition() {
                    FamilyThemeDisposition::Unsupported => {
                        observation
                            .residual
                            .get_or_insert(unsupported_residual_for_facet(facet));
                    }
                    FamilyThemeDisposition::TypedAdapter
                        if property == ResolvedStyleProperty::Fill
                            && plan
                                .paint
                                .as_ref()
                                .is_some_and(|paint| paint.rule_index() == rule_index) =>
                    {
                        observation.pending = true;
                        if is_title {
                            plan.title_fallback_rule = Some(rule_index);
                        }
                    }
                    _ => observation.incomplete = true,
                }
            }
        }
        let domain = UnsupportedTerminalDomain::fallbacks_only(
            ThemeTarget::Text,
            TerminalVariantDomain::uniform(visible_count, ThemeVariant::Default),
        )
        .with_source_owned_fill(&text_owners);
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[
                domain,
                // Legend paint targets actual non-empty labels, independently of swatches.
                UnsupportedTerminalDomain::fallbacks_only(
                    ThemeTarget::Legend,
                    TerminalVariantDomain::indexed(&legend_occurrences),
                )
                .with_source_owned_fill(&legend_fill_owned),
            ],
            work_meter,
        )?;
        for (index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::Text,
            };
            if let Some(reason) = observation.residual {
                plan.evidence.mark_residual(key, reason);
            } else if observation.incomplete { /* Unconsumed sibling facets stay incomplete. */
            } else if observation.pending {
                plan.pending_rule = Some(index);
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        Ok(plan)
    }

    pub(crate) fn text_color(&self) -> &str {
        &self.text_color
    }
    pub(crate) fn title_color(&self) -> &str {
        &self.title_color
    }
    pub(crate) fn legend_fill(&self, index: usize) -> Option<&str> {
        self.legend_paints
            .get(index)
            .and_then(Option::as_ref)
            .map(DirectPaintExpectation::css)
    }
    fn needs_receipt(&self) -> bool {
        self.pending_rule.is_some()
            || self.title_fallback_rule.is_some()
            || !self.legend_rules.is_empty()
    }
    pub(crate) fn begin_terminal_receipt(&self) -> Option<RadarTextPaintReceipt> {
        self.needs_receipt()
            .then(|| RadarTextPaintReceipt::new(self))
    }
    pub(crate) fn record_terminal(&self, receipt: RadarTextPaintReceipt) -> bool {
        self.needs_receipt() && receipt.proves(self) && self.terminal_receipt.set(()).is_ok()
    }
    /// True only after a visible title consumed the winning Text fill in finalized SVG.
    pub(crate) fn has_verified_title_fallback(&self) -> bool {
        self.title_fallback_rule.is_some()
            && self.title_visible
            && self.terminal_receipt.get().is_some()
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
        record_pending(
            &self.legend_rules,
            self.terminal_receipt.get().is_some(),
            evidence,
        );
        if let Some(index) = self.pending_rule
            && self.terminal_receipt.get().is_some()
            && let Some(paint) = &self.paint
        {
            evidence.mark_applied_with_capabilities(
                FamilyThemeMechanismKey::Rule {
                    index,
                    target: ThemeTarget::Text,
                },
                [paint.capability()],
            );
        }
    }
}

#[derive(Debug)]
pub(crate) struct RadarTextPaintReceipt {
    expected_text: Box<str>,
    expected_title: Box<str>,
    base_css: bool,
    title_css: bool,
    axes: Vec<bool>,
    legends: Vec<bool>,
    legend_fills: Vec<Option<Box<str>>>,
    title: Option<bool>,
    valid: bool,
}

impl RadarTextPaintReceipt {
    fn new(plan: &RadarTextPaintPlan) -> Self {
        Self {
            expected_text: plan.text_color.clone(),
            expected_title: plan.title_color.clone(),
            base_css: false,
            title_css: false,
            axes: Vec::with_capacity(plan.axis_visible.len()),
            legends: Vec::with_capacity(plan.legend_visible.len()),
            legend_fills: Vec::with_capacity(plan.legend_visible.len()),
            title: None,
            valid: true,
        }
    }
    pub(crate) fn record_base_css(&mut self, fill: &str) {
        self.valid &= !self.base_css
            && self.axes.is_empty()
            && self.legends.is_empty()
            && self.title.is_none()
            && fill == self.expected_text.as_ref();
        self.base_css = true;
    }
    pub(crate) fn record_title_css(&mut self, class: &str, color: &str, fill: &str) {
        self.valid &= self.base_css
            && !self.title_css
            && self.axes.is_empty()
            && self.legends.is_empty()
            && self.title.is_none()
            && class == "radarTitle"
            && color == self.expected_title.as_ref()
            && fill == self.expected_title.as_ref();
        self.title_css = true;
    }
    pub(crate) fn record_axis(&mut self, index: usize, class: &str, label: &str) {
        self.valid &= self.base_css
            && self.title_css
            && self.legends.is_empty()
            && self.title.is_none()
            && index == self.axes.len()
            && class == "radarAxisLabel";
        self.axes.push(!label.trim().is_empty());
    }
    #[cfg(test)]
    pub(crate) fn record_legend(&mut self, index: usize, class: &str, label: &str) {
        self.record_legend_paint(index, class, label, None);
    }
    pub(crate) fn record_legend_paint(
        &mut self,
        index: usize,
        class: &str,
        label: &str,
        fill: Option<&str>,
    ) {
        self.valid &= self.base_css
            && self.title_css
            && self.title.is_none()
            && index == self.legends.len()
            && class == "radarLegendText";
        self.legends.push(!label.trim().is_empty());
        self.legend_fills.push(fill.map(Into::into));
    }
    pub(crate) fn record_title(&mut self, class: &str, visible: bool) {
        self.valid &=
            self.base_css && self.title_css && self.title.is_none() && class == "radarTitle";
        self.title = Some(visible);
    }
    fn proves(&self, plan: &RadarTextPaintPlan) -> bool {
        self.valid
            && self.base_css
            && self.title_css
            && self.axes.as_slice() == plan.axis_visible.as_ref()
            && self.legends.as_slice() == plan.legend_visible.as_ref()
            && self
                .legend_fills
                .iter()
                .enumerate()
                .all(|(index, fill)| fill.as_deref() == plan.legend_fill(index))
            && self.title == Some(plan.title_visible)
            && (plan.title_visible
                || self
                    .axes
                    .iter()
                    .chain(self.legends.iter())
                    .any(|visible| *visible))
            && self.expected_text == plan.text_color
            && self.expected_title == plan.title_color
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn plan() -> RadarTextPaintPlan {
        RadarTextPaintPlan {
            text_color: "#123456".into(),
            title_color: "#abcdef".into(),
            axis_visible: vec![true].into(),
            legend_visible: vec![true].into(),
            title_visible: true,
            paint: None,
            pending_rule: None,
            title_fallback_rule: None,
            legend_paints: vec![None],
            legend_rules: PendingPaintRules::new(),
            evidence: FamilyThemeOutcomes::default(),
            terminal_receipt: OnceLock::new(),
        }
    }
    fn css(plan: &RadarTextPaintPlan) -> RadarTextPaintReceipt {
        let mut receipt = RadarTextPaintReceipt::new(plan);
        receipt.record_base_css(plan.text_color());
        receipt.record_title_css("radarTitle", plan.title_color(), plan.title_color());
        receipt
    }
    fn terminals(receipt: &mut RadarTextPaintReceipt) {
        receipt.record_axis(0, "radarAxisLabel", "Axis");
        receipt.record_legend(0, "radarLegendText", "Legend");
        receipt.record_title("radarTitle", true);
    }

    #[test]
    fn legend_paint_receipt_requires_the_emitted_label_fill() {
        let mut plan = plan();
        plan.legend_paints[0] = Some(DirectPaintExpectation::new(
            0,
            "#112233",
            crate::diagram_theme::ThemeCapability::SolidPaint,
        ));
        for fill in [None, Some("#badbad"), Some("#112233")] {
            let mut receipt = css(&plan);
            receipt.record_axis(0, "radarAxisLabel", "Axis");
            receipt.record_legend_paint(0, "radarLegendText", "Legend", fill);
            receipt.record_title("radarTitle", true);
            assert_eq!(receipt.proves(&plan), fill == Some("#112233"));
        }
    }
    #[test]
    fn title_fallback_is_proven_only_by_a_completed_visible_title_receipt() {
        let mut fallback = plan();
        fallback.title_fallback_rule = Some(0);
        assert!(!fallback.has_verified_title_fallback());
        let mut incomplete = css(&fallback);
        incomplete.record_axis(0, "radarAxisLabel", "Axis");
        assert!(!fallback.record_terminal(incomplete));
        assert!(!fallback.has_verified_title_fallback());
        let mut complete = css(&fallback);
        terminals(&mut complete);
        assert!(fallback.record_terminal(complete));
        assert!(fallback.has_verified_title_fallback());

        let mut source_owned_title = plan();
        source_owned_title.pending_rule = Some(0);
        let mut complete = css(&source_owned_title);
        terminals(&mut complete);
        assert!(source_owned_title.record_terminal(complete));
        assert!(!source_owned_title.has_verified_title_fallback());
    }

    #[test]
    fn text_receipt_requires_css_complete_ordered_visible_terminals() {
        let plan = plan();
        let mut complete = css(&plan);
        terminals(&mut complete);
        assert!(complete.proves(&plan));
        complete.record_title("radarTitle", true);
        assert!(!complete.proves(&plan));
        let mut missing = css(&plan);
        missing.record_axis(0, "radarAxisLabel", "Axis");
        assert!(!missing.proves(&plan));
        let mut no_css = RadarTextPaintReceipt::new(&plan);
        terminals(&mut no_css);
        assert!(!no_css.proves(&plan));
        let mut duplicate = css(&plan);
        duplicate.record_base_css(plan.text_color());
        terminals(&mut duplicate);
        assert!(!duplicate.proves(&plan));
        let mut misordered = css(&plan);
        misordered.record_legend(0, "radarLegendText", "Legend");
        misordered.record_axis(0, "radarAxisLabel", "Axis");
        misordered.record_title("radarTitle", true);
        assert!(!misordered.proves(&plan));
    }
    #[test]
    fn text_receipt_rejects_wrong_css_and_absent_visible_text() {
        let mut plan = plan();
        for title_wrong in [false, true] {
            let mut wrong = RadarTextPaintReceipt::new(&plan);
            wrong.record_base_css(if title_wrong {
                plan.text_color()
            } else {
                "wrong"
            });
            wrong.record_title_css(
                "radarTitle",
                plan.title_color(),
                if title_wrong {
                    "wrong"
                } else {
                    plan.title_color()
                },
            );
            terminals(&mut wrong);
            assert!(!wrong.proves(&plan));
        }
        plan.axis_visible = vec![false].into();
        plan.legend_visible = vec![false].into();
        plan.title_visible = false;
        let mut empty = css(&plan);
        empty.record_axis(0, "radarAxisLabel", " ");
        empty.record_legend(0, "radarLegendText", "");
        empty.record_title("radarTitle", false);
        assert!(!empty.proves(&plan));
    }
}
