use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemePaintKind, FamilyThemeRuleFacet, FamilyThemeSelectorShape, ResolvedDiagramTheme,
    ResolvedStyleProperty, Specified, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

const COMMIT_LINE_COLOR_PATH: &str = "themeVariables.commitLineColor";
const LINE_COLOR_PATH: &str = "themeVariables.lineColor";

#[derive(Debug, Clone, PartialEq, Eq)]
struct GitGraphBranchStroke {
    css: Box<str>,
    rule_index: usize,
    property: ResolvedStyleProperty,
    capability: ThemeCapability,
}

#[derive(Debug, Default)]
struct RuleObservation {
    applicable: bool,
    pending: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
}

/// Direct GitGraph branch-line stroke behind the existing family theme-plan seam.
#[derive(Debug)]
pub(super) struct GitGraphBranchStrokePlan {
    stroke: Option<GitGraphBranchStroke>,
    expected_branch_line_count: usize,
    pending_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<GitGraphBranchStrokeReceipt>,
}

impl GitGraphBranchStrokePlan {
    pub(super) fn baseline(expected_branch_line_count: usize) -> Self {
        Self {
            stroke: None,
            expected_branch_line_count,
            pending_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(super) fn resolve(
        theme: &ResolvedDiagramTheme,
        effective_config: &MermaidConfig,
        expected_branch_line_count: usize,
        work_meter: &OperationWorkMeter,
        evidence: &mut FamilyThemeEvidence,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline(expected_branch_line_count);
        let config_owns_stroke = mermaid_owns_branch_stroke(effective_config);
        // A fill fallback formerly created commitLineColor. An explicit lineColor alone
        // cannot own this new source, unlike the existing direct Stroke route.
        let config_owns_fill = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            COMMIT_LINE_COLOR_PATH,
        );
        let has_ordinal_edge_rules = theme.family_rules().any(|(_, rule)| {
            rule.target() == ThemeTarget::Edge
                && matches!(rule.variant(), None | Some(ThemeVariant::Default))
                && rule.ordinal().is_some()
        });
        let mut winner_properties = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut first_candidate = None::<Option<GitGraphBranchStroke>>;
        let mut candidates_match = true;

        if expected_branch_line_count > 0 {
            if has_ordinal_edge_rules {
                for ordinal in 1..=expected_branch_line_count {
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
        let stroke_winner = stroke
            .as_ref()
            .map(|stroke| (stroke.rule_index, stroke.property));
        // The retired bridge resolved without an ordinal. Preserve that static fill in
        // BestEffort CSS when ordinal winners prevent a uniform terminal proof. This is
        // deliberately separate from stroke_winner, which alone authorizes Applied.
        let compatibility_fill = if stroke.is_none()
            && has_ordinal_edge_rules
            && expected_branch_line_count > 0
            && !config_owns_fill
        {
            let style = theme.style_with_work_meter(
                ThemeTarget::Edge,
                ThemeVariant::Default,
                None,
                work_meter,
            )?;
            typed_static_branch_paint(theme, &style)
                .filter(|paint| paint.property == ResolvedStyleProperty::Fill)
        } else {
            None
        };
        let mut observations = BTreeMap::<usize, RuleObservation>::new();

        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Edge,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !selector
                        .ordinal_domain_intersects_occurrence_count(expected_branch_line_count)
                    {
                        continue;
                    }
                    let property = resolved_style_property_for_facet(facet);
                    let route_won = winner_properties.contains(&(rule_index, property));
                    if !route_won {
                        continue;
                    }

                    if (property == ResolvedStyleProperty::Fill && config_owns_fill)
                        || (property == ResolvedStyleProperty::Stroke && config_owns_stroke)
                    {
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
                            FamilyThemeSelectorShape::Static {
                                variant: None | Some(ThemeVariant::Default),
                            },
                            FamilyThemeRuleFacet::Stroke(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            )
                            | FamilyThemeRuleFacet::Fill(
                                FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid,
                            ),
                        ) if stroke_winner == Some((rule_index, property)) => {
                            observation.pending = true;
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        if let Some(fill) = &compatibility_fill {
            let observation = observations.entry(fill.rule_index).or_default();
            observation.applicable = true;
            observation.incomplete = true;
        }

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
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint);
            } else if observation.pending {
                debug_assert!(plan.pending_key.is_none());
                plan.pending_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        if let Some(stroke) = stroke.or(compatibility_fill) {
            let owned = if stroke.property == ResolvedStyleProperty::Fill {
                config_owns_fill
            } else {
                config_owns_stroke
            };
            // Preserve a fill's BestEffort CSS even when a sibling property leaves a residual.
            // The pending key is deliberately absent for such a mixed rule.
            if !owned
                && (stroke.property == ResolvedStyleProperty::Fill || plan.pending_key.is_some())
            {
                plan.stroke = Some(stroke);
            }
        }
        Ok(plan)
    }

    pub(super) fn terminal_css(&self) -> Option<&str> {
        self.stroke
            .as_ref()
            .filter(|stroke| stroke.property == ResolvedStyleProperty::Stroke)
            .map(|stroke| stroke.css.as_ref())
    }

    pub(super) fn stylesheet_css(&self) -> Option<&str> {
        self.stroke
            .as_ref()
            .filter(|stroke| stroke.property == ResolvedStyleProperty::Fill)
            .map(|stroke| stroke.css.as_ref())
    }

    pub(super) fn begin_terminal_receipt(&self) -> Option<GitGraphBranchStrokeReceipt> {
        self.stroke.as_ref().map(|stroke| {
            GitGraphBranchStrokeReceipt::new(
                stroke.css.clone(),
                self.expected_branch_line_count,
                stroke.property == ResolvedStyleProperty::Fill,
            )
        })
    }

    pub(super) fn record_terminal(&self, receipt: GitGraphBranchStrokeReceipt) -> bool {
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

/// Writer-owned proof for `.branch` CSS and each line, with either inherited or inline stroke.
#[derive(Debug)]
pub(crate) struct GitGraphBranchStrokeReceipt {
    expected_stroke: Box<str>,
    expected_branch_line_count: usize,
    next_branch_line: usize,
    values_match: bool,
    stylesheet_driven: bool,
    css_recorded: bool,
}

impl GitGraphBranchStrokeReceipt {
    fn new(
        expected_stroke: Box<str>,
        expected_branch_line_count: usize,
        stylesheet_driven: bool,
    ) -> Self {
        Self {
            expected_stroke,
            expected_branch_line_count,
            next_branch_line: 0,
            values_match: true,
            stylesheet_driven,
            css_recorded: false,
        }
    }

    pub(crate) fn record_css(&mut self, emitted_stroke: &str) {
        self.values_match &= !self.css_recorded
            && (!self.stylesheet_driven || emitted_stroke == self.expected_stroke.as_ref());
        self.css_recorded = true;
    }

    pub(super) fn record_branch_line(&mut self, emitted_class: &str, emitted_style: Option<&str>) {
        self.values_match &= emitted_class
            .split_ascii_whitespace()
            .any(|class| class == "branch");
        self.values_match &= if self.stylesheet_driven {
            self.css_recorded && emitted_style.is_none()
        } else {
            emitted_style.and_then(terminal_stroke) == Some(self.expected_stroke.as_ref())
        };
        self.next_branch_line = self.next_branch_line.saturating_add(1);
    }

    fn proves(&self, plan: &GitGraphBranchStrokePlan) -> bool {
        self.values_match
            && self.css_recorded
            && plan.stroke.as_ref().is_some_and(|stroke| {
                stroke.css.as_ref() == self.expected_stroke.as_ref()
                    && (stroke.property == ResolvedStyleProperty::Fill) == self.stylesheet_driven
            })
            && self.next_branch_line == self.expected_branch_line_count
            && self.expected_branch_line_count == plan.expected_branch_line_count
    }
}

fn observe_style(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
    winner_properties: &mut BTreeSet<(usize, ResolvedStyleProperty)>,
) -> Option<GitGraphBranchStroke> {
    winner_properties.extend(
        style
            .winner_rule_properties()
            .filter(|(property, _)| {
                *property != ResolvedStyleProperty::Fill
                    || matches!(
                        style.stroke_resolution().specified(),
                        Specified::Unspecified
                    )
            })
            .map(|(property, origin)| (origin.rule_index(), property)),
    );
    typed_static_branch_paint(theme, style)
}

fn typed_static_branch_paint(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<GitGraphBranchStroke> {
    let (resolution, property, facet) = if matches!(
        style.stroke_resolution().specified(),
        Specified::Unspecified,
    ) {
        (
            style.fill_resolution(),
            ResolvedStyleProperty::Fill,
            FamilyThemeRuleFacet::fill(style.fill_resolution().specified())?,
        )
    } else {
        (
            style.stroke_resolution(),
            ResolvedStyleProperty::Stroke,
            FamilyThemeRuleFacet::stroke(style.stroke_resolution().specified())?,
        )
    };
    let origin = resolution.winner()?;
    let rule = theme
        .family_rules()
        .find_map(|(index, rule)| (index == origin.rule_index()).then_some(rule))?;
    if !matches!(rule.variant(), None | Some(ThemeVariant::Default))
        || rule.ordinal().is_some()
        || theme.rule_facet_disposition(origin.rule_index(), facet)
            != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }

    match resolution.specified() {
        Specified::Value(CanvasPaint::Transparent) => Some(GitGraphBranchStroke {
            css: "transparent".into(),
            rule_index: origin.rule_index(),
            property,
            capability: ThemeCapability::TransparentPaint,
        }),
        Specified::Value(CanvasPaint::Solid(color)) => Some(GitGraphBranchStroke {
            css: color.as_css().into_boxed_str(),
            rule_index: origin.rule_index(),
            property,
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

fn mermaid_owns_branch_stroke(config: &MermaidConfig) -> bool {
    if config.get_str(COMMIT_LINE_COLOR_PATH).is_some() {
        merman_core::__private::config_path_overrides_typed_default(config, COMMIT_LINE_COLOR_PATH)
    } else {
        merman_core::__private::config_path_overrides_typed_default(config, LINE_COLOR_PATH)
    }
}

fn terminal_stroke(style: &str) -> Option<&str> {
    style.split(';').find_map(|declaration| {
        let (property, value) = declaration.split_once(':')?;
        (property.trim() == "stroke").then(|| value.trim())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt_plan() -> GitGraphBranchStrokePlan {
        GitGraphBranchStrokePlan {
            stroke: Some(GitGraphBranchStroke {
                css: "#123456".into(),
                rule_index: 7,
                property: ResolvedStyleProperty::Stroke,
                capability: ThemeCapability::SolidPaint,
            }),
            expected_branch_line_count: 2,
            pending_key: Some(FamilyThemeMechanismKey::Rule {
                index: 7,
                target: ThemeTarget::Edge,
            }),
            terminal_receipt: OnceLock::new(),
        }
    }

    #[test]
    fn fill_receipt_requires_matching_css_and_unmodified_branch_lines() {
        let mut plan = receipt_plan();
        plan.stroke.as_mut().unwrap().property = ResolvedStyleProperty::Fill;
        for case in [
            "valid",
            "missing-css",
            "duplicate-css",
            "wrong-css",
            "inline",
            "missing-line",
        ] {
            let mut receipt = plan.begin_terminal_receipt().unwrap();
            if case != "missing-css" {
                receipt.record_css(if case == "wrong-css" {
                    "#abcdef"
                } else {
                    "#123456"
                });
            }
            if case == "duplicate-css" {
                receipt.record_css("#123456");
            }
            receipt.record_branch_line(
                "branch branch0",
                if case == "inline" {
                    Some("stroke:#123456;")
                } else {
                    None
                },
            );
            if case != "missing-line" {
                receipt.record_branch_line("branch branch1", None);
            }
            assert_eq!(receipt.proves(&plan), case == "valid", "{case}");
        }
    }

    #[test]
    fn terminal_receipt_requires_every_matching_branch_line() {
        let plan = receipt_plan();
        let mut complete = GitGraphBranchStrokeReceipt::new("#123456".into(), 2, false);
        complete.record_css("#baseline");
        complete.record_branch_line("branch branch0", Some("stroke:#123456;"));
        complete.record_branch_line("branch branch1", Some("stroke:#123456;"));
        assert!(complete.proves(&plan));

        let mut missing = GitGraphBranchStrokeReceipt::new("#123456".into(), 2, false);
        missing.record_css("#baseline");
        missing.record_branch_line("branch branch0", Some("stroke:#123456;"));
        assert!(!missing.proves(&plan));

        let mut wrong_value = GitGraphBranchStrokeReceipt::new("#123456".into(), 2, false);
        wrong_value.record_css("#baseline");
        wrong_value.record_branch_line("branch branch0", Some("stroke:#abcdef;"));
        wrong_value.record_branch_line("branch branch1", Some("stroke:#123456;"));
        assert!(!wrong_value.proves(&plan));

        let mut wrong_surface = GitGraphBranchStrokeReceipt::new("#123456".into(), 2, false);
        wrong_surface.record_css("#baseline");
        wrong_surface.record_branch_line("arrow arrow0", Some("stroke:#123456;"));
        wrong_surface.record_branch_line("branch branch1", Some("stroke:#123456;"));
        assert!(!wrong_surface.proves(&plan));
    }
}
