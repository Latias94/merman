use std::collections::BTreeSet;

use merman_model::presentation::PresentationConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    TerminalVariantDomain, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_fill,
};
use crate::model::GanttExcludeRangeLayout;
use crate::resources::OperationWorkMeter;

const TARGET: ThemeTarget = ThemeTarget::ExcludedPeriod;

#[derive(Debug, Default)]
pub(super) struct GanttExcludedPeriodPlan {
    theme: Option<ResolvedDiagramTheme>,
    source_owned: bool,
}

impl GanttExcludedPeriodPlan {
    pub(super) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &PresentationConfig,
    ) -> Self {
        let theme = theme.filter(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet { target: TARGET, .. }
                        | FamilyThemeMechanism::OrdinalPalette { target: TARGET }
                        | FamilyThemeMechanism::EffectBinding { target: TARGET, .. }
                )
            })
        });
        Self {
            theme: theme.cloned(),
            source_owned: config
                .config_paint_overrides_typed_default("themeVariables.excludeBkgColor"),
        }
    }

    pub(super) fn requested(&self) -> bool {
        self.theme.is_some()
    }

    pub(super) fn begin_terminal_receipt(
        &self,
        excludes: &[GanttExcludeRangeLayout],
        work: &OperationWorkMeter,
    ) -> crate::Result<Option<GanttExcludedPeriodReceipt>> {
        let Some(theme) = &self.theme else {
            return Ok(None);
        };
        let mut expected = BTreeSet::new();
        let mut ordinals = Vec::new();
        let mut paint = None;
        let mut valid = true;
        for (index, rect) in excludes.iter().enumerate() {
            work.charge(1usize.saturating_add(rect.id.len()))?;
            if visible(rect.width, rect.height) {
                valid &= expected.insert(rect.id.clone().into_boxed_str());
                let ordinal = index + 1;
                ordinals.push((ordinal, ThemeVariant::Default));
                // Supported fills are static; one actual winner binds the shared CSS.
                if paint.is_none() && !self.source_owned {
                    let style = theme.style_with_work_meter(
                        TARGET,
                        ThemeVariant::Default,
                        Some(ordinal),
                        work,
                    )?;
                    paint = resolve_direct_static_fill(
                        theme,
                        &style,
                        &[TARGET],
                        DirectStaticSelectorDomain::Default,
                    );
                }
            }
        }
        let mut outcomes = FamilyThemeOutcomes::default();
        reconcile_unsupported_terminal_domains(
            theme,
            &mut outcomes,
            &[
                UnsupportedTerminalDomain::direct(
                    TARGET,
                    TerminalVariantDomain::indexed(&ordinals),
                )
                .with_fill_fully_overridden(self.source_owned),
            ],
            work,
        )?;
        let mut rules = BTreeSet::new();
        for route in theme.family_mechanism_routes() {
            work.charge(1)?;
            if let FamilyThemeMechanism::RuleFacet {
                target: TARGET,
                rule_index,
                ..
            } = route.mechanism()
                && route.disposition() == FamilyThemeDisposition::TypedAdapter
            {
                rules.insert(rule_index);
            }
        }
        Ok(Some(GanttExcludedPeriodReceipt {
            expected,
            seen: BTreeSet::new(),
            paint,
            rules,
            outcomes,
            css_recorded: false,
            valid,
        }))
    }
}

fn visible(width: f64, height: f64) -> bool {
    width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0
}

/// CSS ownership is proved together with every visible excluded calendar rectangle.
#[derive(Debug)]
pub(super) struct GanttExcludedPeriodReceipt {
    expected: BTreeSet<Box<str>>,
    seen: BTreeSet<Box<str>>,
    paint: Option<DirectStaticPaint>,
    rules: BTreeSet<usize>,
    outcomes: FamilyThemeOutcomes,
    css_recorded: bool,
    valid: bool,
}

impl GanttExcludedPeriodReceipt {
    pub(super) fn fill(&self) -> Option<&str> {
        self.paint.as_ref().map(DirectStaticPaint::css)
    }

    pub(super) fn record_css(&mut self, diagram_id: &str, css: &str) {
        if let Some(paint) = &self.paint {
            self.valid &= !self.css_recorded
                && css.contains(&format!(
                    "#{diagram_id} .exclude-range{{fill:{};}}",
                    paint.css()
                ));
            self.css_recorded = true;
        }
    }

    pub(super) fn record_rect(
        &mut self,
        diagram_id: &str,
        rect: &GanttExcludeRangeLayout,
        emitted_id: &str,
        class: &str,
    ) {
        if visible(rect.width, rect.height) {
            self.valid &= self.expected.contains(rect.id.as_str())
                && self.seen.insert(rect.id.clone().into_boxed_str())
                && emitted_id == format!("{diagram_id}-{}", rect.id)
                && class == "exclude-range";
        }
    }

    pub(super) fn complete(&self) -> bool {
        self.valid
            && self.seen.len() == self.expected.len()
            && (self.paint.is_none() || self.css_recorded)
    }

    pub(super) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.outcomes);
        for &index in &self.rules {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: TARGET,
            };
            if let Some(paint) = self
                .paint
                .as_ref()
                .filter(|paint| paint.rule_index() == index)
            {
                if self.complete() {
                    evidence.mark_applied_with_capabilities(key, [paint.capability()]);
                }
            } else {
                evidence.mark_not_applicable(key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    fn plan() -> GanttExcludedPeriodPlan {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        TARGET,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#14324a").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::GANTT);
        GanttExcludedPeriodPlan::resolve(
            Some(&theme),
            &crate::test_support::presentation_config(serde_json::json!({})),
        )
    }

    fn rect() -> GanttExcludeRangeLayout {
        GanttExcludeRangeLayout {
            id: "weekend".into(),
            start_ms: 0,
            end_ms: 1,
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        }
    }

    #[test]
    fn receipt_requires_correct_css_and_every_visible_rectangle_exactly_once() {
        let plan = plan();
        let rect = rect();
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        for failure in 0..8 {
            let mut receipt = plan
                .begin_terminal_receipt(std::slice::from_ref(&rect), &work)
                .unwrap()
                .unwrap();
            if failure != 0 {
                receipt.record_css(
                    "g",
                    if failure == 1 {
                        "#g .exclude-range{fill:#eeeeee;}"
                    } else {
                        "#g .exclude-range{fill:#14324a;}"
                    },
                );
            }
            if failure != 5 {
                receipt.record_rect(
                    "g",
                    &rect,
                    if failure == 2 {
                        "wrong-weekend"
                    } else {
                        "g-weekend"
                    },
                    if failure == 3 {
                        "section"
                    } else {
                        "exclude-range"
                    },
                );
            }
            if failure == 4 {
                receipt.record_rect("g", &rect, "g-weekend", "exclude-range");
            }
            if failure == 6 {
                receipt.record_css("g", "#g .exclude-range{fill:#14324a;}");
            }
            assert_eq!(receipt.complete(), failure == 7, "failure case {failure}");
        }
    }

    #[test]
    fn zero_area_exclusions_do_not_bind_a_fill_or_require_a_visible_terminal() {
        let plan = plan();
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        for (width, height) in [(0.0, 40.0), (30.0, 0.0)] {
            let rect = GanttExcludeRangeLayout {
                width,
                height,
                ..rect()
            };
            let mut receipt = plan
                .begin_terminal_receipt(std::slice::from_ref(&rect), &work)
                .unwrap()
                .unwrap();
            assert_eq!(receipt.fill(), None);
            receipt.record_rect("g", &rect, "g-weekend", "exclude-range");
            assert!(receipt.complete());
        }
    }
}
