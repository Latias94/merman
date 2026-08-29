use std::collections::BTreeSet;
use std::sync::OnceLock;

use merman_core::MermaidConfig;
use merman_core::diagrams::radar::RadarDiagramRenderModel;

use crate::config::{config_css_number_or_string, value_at};
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::theme::MermaidThemeAdapter;

const RADAR_PALETTE_SLOT_COUNT: usize = 12;

const RADAR_COLOR_SCALE_PATHS: [&str; RADAR_PALETTE_SLOT_COUNT] = [
    "themeVariables.cScale0",
    "themeVariables.cScale1",
    "themeVariables.cScale2",
    "themeVariables.cScale3",
    "themeVariables.cScale4",
    "themeVariables.cScale5",
    "themeVariables.cScale6",
    "themeVariables.cScale7",
    "themeVariables.cScale8",
    "themeVariables.cScale9",
    "themeVariables.cScale10",
    "themeVariables.cScale11",
];

/// Final Radar base typography shared by stylesheet emission and terminal evidence.
#[derive(Debug)]
pub(crate) struct RadarTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    font_size_css: Box<str>,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<RadarTypographyTerminalSeal>,
}

impl RadarTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
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
            _ => config_css_number_or_string(
                effective_config.as_value(),
                &["themeVariables", "fontSize"],
            )
            .unwrap_or_else(|| "16px".to_string()),
        }
        .into_boxed_str();

        Self {
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested,
            typed_font_size_active,
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        axis_count: usize,
        legend_count: usize,
    ) -> Option<RadarTypographyThemeReceipt> {
        self.typography_requested().then(|| {
            RadarTypographyThemeReceipt::new(
                axis_count,
                legend_count,
                self.observes_inherited_text(),
            )
        })
    }

    pub(crate) fn record_terminal(&self, receipt: RadarTypographyThemeReceipt) -> bool {
        receipt
            .seal()
            .is_some_and(|seal| self.terminal_receipt.set(seal).is_ok())
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if !self.typography_requested() {
            return evidence;
        }

        let Some(receipt) = self.terminal_receipt.get() else {
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
            return evidence;
        };
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(
                &mut evidence,
                receipt.has_visible_title
                    || receipt.has_visible_axis_label
                    || receipt.has_visible_legend_label,
            );
        let has_visible_typography = receipt.has_visible_title
            || receipt.has_visible_axis_label
            || receipt.has_visible_legend_label;
        if !has_visible_typography {
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
                    evidence.mark_not_applicable(FamilyThemeMechanismKey::Typography(property));
                }
            }
            return evidence;
        }
        let font_stack_applied =
            self.inherited_font_stack.typed_font_stack_active() && has_visible_typography;
        let font_size_applied = self.typed_font_size_active && receipt.has_visible_title;
        for (property, requested, applied) in [
            (
                ThemeTypographyProperty::FontStack,
                self.inherited_font_stack.typed_font_stack_requested(),
                font_stack_applied,
            ),
            (
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                font_size_applied,
            ),
        ] {
            if !requested {
                continue;
            }
            let key = FamilyThemeMechanismKey::Typography(property);
            if applied {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        evidence
    }

    fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typed_font_stack_requested()
            || self.typed_font_size_requested
            || self.inherited_font_stack.outcome() == InheritedFontStackOutcome::Unsupported
    }

    fn observes_inherited_text(&self) -> bool {
        self.inherited_font_stack.typed_font_stack_requested()
            || self.typed_font_size_requested
            || self.inherited_font_stack.outcome() == InheritedFontStackOutcome::Unsupported
    }
}

/// Milestone issued only after the final Radar stylesheet writer completes successfully.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RadarTypographyCssEmission(());

impl RadarTypographyCssEmission {
    pub(crate) const fn written() -> Self {
        Self(())
    }
}

/// Writer-owned proof that the stylesheet and visible Radar text passes reached the SVG sink.
#[derive(Debug)]
pub(crate) struct RadarTypographyThemeReceipt {
    expected_axis_count: usize,
    expected_legend_count: usize,
    observe_inherited_text: bool,
    css_emitted: bool,
    next_axis_index: usize,
    next_legend_index: usize,
    title_emitted: bool,
    has_visible_title: bool,
    has_visible_axis_label: bool,
    has_visible_legend_label: bool,
    valid: bool,
}

#[derive(Debug)]
struct RadarTypographyTerminalSeal {
    has_visible_title: bool,
    has_visible_axis_label: bool,
    has_visible_legend_label: bool,
}

impl RadarTypographyThemeReceipt {
    fn new(
        expected_axis_count: usize,
        expected_legend_count: usize,
        observe_inherited_text: bool,
    ) -> Self {
        Self {
            expected_axis_count: if observe_inherited_text {
                expected_axis_count
            } else {
                0
            },
            expected_legend_count: if observe_inherited_text {
                expected_legend_count
            } else {
                0
            },
            observe_inherited_text,
            css_emitted: false,
            next_axis_index: 0,
            next_legend_index: 0,
            title_emitted: false,
            has_visible_title: false,
            has_visible_axis_label: false,
            has_visible_legend_label: false,
            valid: true,
        }
    }

    pub(crate) fn record_css_emission(&mut self, _emission: RadarTypographyCssEmission) {
        self.valid &= !self.css_emitted
            && self.next_axis_index == 0
            && self.next_legend_index == 0
            && !self.title_emitted;
        self.css_emitted = true;
    }

    pub(crate) fn record_axis_label(&mut self, axis_index: usize, label: &str) {
        if !self.observe_inherited_text {
            return;
        }
        self.valid &= self.css_emitted
            && !self.title_emitted
            && self.next_legend_index == 0
            && axis_index == self.next_axis_index
            && axis_index < self.expected_axis_count;
        self.next_axis_index = self.next_axis_index.saturating_add(1);
        self.has_visible_axis_label |= !label.trim().is_empty();
    }

    pub(crate) fn record_legend_label(&mut self, legend_index: usize, label: &str) {
        if !self.observe_inherited_text {
            return;
        }
        self.valid &= self.css_emitted
            && !self.title_emitted
            && self.next_axis_index == self.expected_axis_count
            && legend_index == self.next_legend_index
            && legend_index < self.expected_legend_count;
        self.next_legend_index = self.next_legend_index.saturating_add(1);
        self.has_visible_legend_label |= !label.trim().is_empty();
    }

    pub(crate) fn record_title(&mut self, visible: bool) {
        self.valid &= self.css_emitted
            && !self.title_emitted
            && self.next_axis_index == self.expected_axis_count
            && self.next_legend_index == self.expected_legend_count;
        self.title_emitted = true;
        self.has_visible_title = visible;
    }

    fn seal(self) -> Option<RadarTypographyTerminalSeal> {
        (self.valid
            && self.css_emitted
            && self.next_axis_index == self.expected_axis_count
            && self.next_legend_index == self.expected_legend_count
            && self.title_emitted)
            .then_some(RadarTypographyTerminalSeal {
                has_visible_title: self.has_visible_title,
                has_visible_axis_label: self.has_visible_axis_label,
                has_visible_legend_label: self.has_visible_legend_label,
            })
    }
}

/// Resolves Radar curve and legend paint once for terminal CSS and family evidence.
#[derive(Debug)]
pub(crate) struct RadarSeriesPaintPlan {
    colors: [String; RADAR_PALETTE_SLOT_COUNT],
    typed_capabilities: [Option<ThemeCapability>; RADAR_PALETTE_SLOT_COUNT],
    curve_count: usize,
    show_legend: bool,
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

impl RadarSeriesPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        model: &RadarDiagramRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline(effective_config, model);
        let Some(theme) = theme else {
            return Ok(plan);
        };

        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let Some(disposition) = theme.ordinal_palette_disposition(ThemeTarget::ChartSeries) else {
            return Ok(plan);
        };
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::ChartSeries,
        };
        if model.curves.is_empty() {
            plan.evidence.mark_not_applicable(key);
            return Ok(plan);
        }

        match disposition {
            FamilyThemeDisposition::TypedAdapter => {
                if model.curves.len() > RADAR_PALETTE_SLOT_COUNT
                    || !uses_fixed_mermaid_palette_surface(effective_config)
                {
                    plan.evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette);
                    return Ok(plan);
                }

                for curve_index in 0..model.curves.len() {
                    let style = theme.style_with_work_meter(
                        ThemeTarget::ChartSeries,
                        ThemeVariant::Default,
                        Some(curve_index + 1),
                        work_meter,
                    )?;
                    if style.fill_resolution().winner().is_some()
                        || style.stroke_resolution().winner().is_some()
                        || merman_core::__private::config_path_overrides_typed_default(
                            effective_config,
                            RADAR_COLOR_SCALE_PATHS[curve_index],
                        )
                    {
                        continue;
                    }

                    let color = theme
                        .series_color(ThemeTarget::ChartSeries, curve_index + 1)
                        .expect("compiled ordinal palettes are non-empty and one-based");
                    plan.colors[curve_index] = color.as_css();
                    plan.typed_capabilities[curve_index] = Some(if color.is_transparent() {
                        ThemeCapability::TransparentPaint
                    } else {
                        ThemeCapability::SolidPaint
                    });
                }
                plan.palette_key = Some(key);
            }
            FamilyThemeDisposition::Unsupported => plan
                .evidence
                .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette),
            FamilyThemeDisposition::LegacyCompatibility => {}
        }

        Ok(plan)
    }

    pub(crate) fn baseline(
        effective_config: &MermaidConfig,
        model: &RadarDiagramRenderModel,
    ) -> Self {
        let colors = MermaidThemeAdapter::new(effective_config.as_value()).radar_series_colors();
        Self {
            colors,
            typed_capabilities: [None; RADAR_PALETTE_SLOT_COUNT],
            curve_count: model.curves.len(),
            show_legend: model.options.show_legend,
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn colors(&self) -> &[String] {
        &self.colors
    }

    pub(crate) const fn curve_count(&self) -> usize {
        self.curve_count
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<RadarSeriesPaintReceipt> {
        self.palette_key
            .as_ref()
            .map(|_| RadarSeriesPaintReceipt::new())
    }

    pub(crate) fn record_terminal(&self, receipt: RadarSeriesPaintReceipt) -> bool {
        self.palette_key.is_some() && receipt.proves(self) && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(key) = self.palette_key.clone() else {
            return evidence;
        };
        match self.terminal_receipt.get() {
            Some(()) => {
                let capabilities = self
                    .typed_capabilities
                    .iter()
                    .take(self.curve_count)
                    .flatten()
                    .copied()
                    .collect::<BTreeSet<_>>();
                if capabilities.is_empty() {
                    evidence.mark_not_applicable(key);
                } else {
                    evidence.mark_applied_with_capabilities(key, capabilities);
                }
            }
            None => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette)
            }
        }
        evidence
    }
}

fn uses_fixed_mermaid_palette_surface(effective_config: &MermaidConfig) -> bool {
    value_at(
        effective_config.as_value(),
        &["themeVariables", "THEME_COLOR_LIMIT"],
    )
    .is_none_or(|value| value.as_f64() == Some(RADAR_PALETTE_SLOT_COUNT as f64))
}

/// Writer-owned proof that the complete visible Radar series surface reached terminal SVG.
#[derive(Debug)]
pub(crate) struct RadarSeriesPaintReceipt {
    next_rule_index: usize,
    next_curve_index: usize,
    next_legend_index: usize,
    values_match: bool,
}

impl RadarSeriesPaintReceipt {
    fn new() -> Self {
        Self {
            next_rule_index: 0,
            next_curve_index: 0,
            next_legend_index: 0,
            values_match: true,
        }
    }

    pub(crate) fn record_series_rule(
        &mut self,
        plan: &RadarSeriesPaintPlan,
        rule_index: usize,
        emitted_color: &str,
    ) {
        if rule_index != self.next_rule_index || rule_index >= plan.curve_count {
            self.values_match = false;
            return;
        }
        self.next_rule_index += 1;
        self.values_match &= plan
            .colors
            .get(rule_index)
            .is_some_and(|color| color == emitted_color);
    }

    pub(crate) fn record_curve(
        &mut self,
        plan: &RadarSeriesPaintPlan,
        curve_index: usize,
        emitted_class_index: i64,
    ) {
        if curve_index != self.next_curve_index || curve_index >= plan.curve_count {
            self.values_match = false;
            return;
        }
        self.next_curve_index += 1;
        self.values_match &= usize::try_from(emitted_class_index).ok() == Some(curve_index);
    }

    pub(crate) fn record_legend(
        &mut self,
        plan: &RadarSeriesPaintPlan,
        legend_index: usize,
        emitted_class_index: i64,
    ) {
        let expected_legend_count = if plan.show_legend {
            plan.curve_count
        } else {
            0
        };
        if legend_index != self.next_legend_index || legend_index >= expected_legend_count {
            self.values_match = false;
            return;
        }
        self.next_legend_index += 1;
        self.values_match &= usize::try_from(emitted_class_index).ok() == Some(legend_index);
    }

    fn proves(&self, plan: &RadarSeriesPaintPlan) -> bool {
        self.values_match
            && self.next_rule_index == plan.curve_count
            && self.next_curve_index == plan.curve_count
            && self.next_legend_index
                == if plan.show_legend {
                    plan.curve_count
                } else {
                    0
                }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolved_typography() -> ResolvedDiagramTheme {
        let typography = crate::diagram_theme::ThemeTextStyle::default()
            .with_font_stack(
                crate::diagram_theme::FontStack::single("RadarReceipt")
                    .expect("valid Radar receipt font stack"),
            )
            .with_font_size_px(24.0)
            .expect("valid Radar receipt font size");
        crate::diagram_theme::DiagramThemeCompiler::new()
            .compile(
                crate::diagram_theme::DiagramThemeSpec::new().with_typography(
                    crate::diagram_theme::TypographySpec::default()
                        .with_family_style(crate::DiagramFamilyId::RADAR, typography),
                ),
            )
            .expect("compile Radar receipt typography")
            .resolve(crate::DiagramFamilyId::RADAR)
    }

    fn one_curve_plan() -> RadarSeriesPaintPlan {
        RadarSeriesPaintPlan {
            colors: std::array::from_fn(|_| "#123456".to_string()),
            typed_capabilities: [Some(ThemeCapability::SolidPaint); RADAR_PALETTE_SLOT_COUNT],
            curve_count: 1,
            show_legend: true,
            evidence: FamilyThemeEvidence::default(),
            palette_key: Some(FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::ChartSeries,
            }),
            terminal_receipt: OnceLock::new(),
        }
    }

    fn assert_rejected_typography_receipt(
        plan: &RadarTypographyThemePlan,
        record: impl FnOnce(&mut RadarTypographyThemeReceipt),
    ) {
        let mut receipt = plan
            .begin_terminal_receipt(1, 1)
            .expect("typed Radar typography receipt");
        record(&mut receipt);
        assert!(!plan.record_terminal(receipt));
    }

    #[test]
    fn series_paint_receipt_rejects_missing_or_mismatched_terminal_checkpoints() {
        let plan = one_curve_plan();
        let mut incomplete = RadarSeriesPaintReceipt::new();
        incomplete.record_series_rule(&plan, 0, "#123456");
        incomplete.record_curve(&plan, 0, 0);
        assert!(!incomplete.proves(&plan));

        let mut mismatch = RadarSeriesPaintReceipt::new();
        mismatch.record_series_rule(&plan, 0, "#abcdef");
        mismatch.record_curve(&plan, 0, 0);
        mismatch.record_legend(&plan, 0, 0);
        assert!(!mismatch.proves(&plan));
    }

    #[test]
    fn typography_receipt_requires_the_complete_ordered_writer_pass() {
        let config = MermaidConfig::empty_object();
        assert!(
            RadarTypographyThemePlan::resolve(None, &config)
                .begin_terminal_receipt(0, 0)
                .is_none()
        );

        let theme = resolved_typography();
        let complete = RadarTypographyThemePlan::resolve(Some(&theme), &config);
        let mut receipt = complete
            .begin_terminal_receipt(1, 1)
            .expect("typed Radar typography receipt");
        receipt.record_css_emission(RadarTypographyCssEmission::written());
        receipt.record_axis_label(0, "Axis");
        receipt.record_legend_label(0, "Legend");
        receipt.record_title(true);
        assert!(complete.record_terminal(receipt));
        let complete_evidence = complete.finish_evidence();
        let applied = complete_evidence.applied();
        assert!(applied.contains(&FamilyThemeMechanismKey::Typography(
            ThemeTypographyProperty::FontStack,
        )));
        assert!(applied.contains(&FamilyThemeMechanismKey::Typography(
            ThemeTypographyProperty::FontSize,
        )));

        let missing_title = RadarTypographyThemePlan::resolve(Some(&theme), &config);
        let mut receipt = missing_title
            .begin_terminal_receipt(1, 1)
            .expect("typed Radar typography receipt");
        receipt.record_css_emission(RadarTypographyCssEmission::written());
        receipt.record_axis_label(0, "Axis");
        receipt.record_legend_label(0, "Legend");
        assert!(!missing_title.record_terminal(receipt));
        let missing_title_evidence = missing_title.finish_evidence();
        let residuals = missing_title_evidence.residuals();
        assert_eq!(residuals.len(), 2);
        assert!(residuals.iter().all(|residual| {
            matches!(
                residual.key(),
                FamilyThemeMechanismKey::Typography(
                    ThemeTypographyProperty::FontStack | ThemeTypographyProperty::FontSize
                )
            )
        }));
    }

    #[test]
    fn typography_receipt_rejects_duplicate_incomplete_and_misordered_passes() {
        let config = MermaidConfig::empty_object();
        let theme = resolved_typography();
        let plan = RadarTypographyThemePlan::resolve(Some(&theme), &config);

        assert_rejected_typography_receipt(&plan, |receipt| {
            receipt.record_axis_label(0, "Axis");
            receipt.record_legend_label(0, "Legend");
            receipt.record_title(true);
        });
        assert_rejected_typography_receipt(&plan, |receipt| {
            receipt.record_css_emission(RadarTypographyCssEmission::written());
            receipt.record_css_emission(RadarTypographyCssEmission::written());
            receipt.record_axis_label(0, "Axis");
            receipt.record_legend_label(0, "Legend");
            receipt.record_title(true);
        });
        assert_rejected_typography_receipt(&plan, |receipt| {
            receipt.record_css_emission(RadarTypographyCssEmission::written());
            receipt.record_legend_label(0, "Legend");
            receipt.record_axis_label(0, "Axis");
            receipt.record_title(true);
        });
        assert_rejected_typography_receipt(&plan, |receipt| {
            receipt.record_css_emission(RadarTypographyCssEmission::written());
            receipt.record_axis_label(0, "Axis");
            receipt.record_axis_label(1, "Overflow");
            receipt.record_legend_label(0, "Legend");
            receipt.record_title(true);
        });
        assert_rejected_typography_receipt(&plan, |receipt| {
            receipt.record_css_emission(RadarTypographyCssEmission::written());
            receipt.record_axis_label(0, "Axis");
            receipt.record_title(true);
        });

        let evidence = plan.finish_evidence();
        let residuals = evidence.residuals();
        assert_eq!(residuals.len(), 2);
        assert!(residuals.iter().all(|residual| {
            matches!(
                residual.key(),
                FamilyThemeMechanismKey::Typography(
                    ThemeTypographyProperty::FontStack | ThemeTypographyProperty::FontSize
                )
            )
        }));
    }
}
