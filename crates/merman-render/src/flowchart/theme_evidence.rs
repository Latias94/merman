use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::Mutex;

use crate::diagram_theme::{
    CanvasPaint, FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeRoute, FamilyThemeRuleFacet, ResolvedDiagramTheme, ResolvedProperty,
    ResolvedStyleProperty, SourceStyleResidual, Specified, ThemeTarget, ThemeVariant,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::resources::{OperationWorkMeter, ResourceLimitExceeded};

#[derive(Debug)]
enum FlowchartPaintOutcome {
    Candidate {
        rule_index: usize,
        value: String,
    },
    Residual {
        rule_index: usize,
        reason: FamilyThemeResidualReason,
    },
}

impl FlowchartPaintOutcome {
    fn value(&self) -> Option<&str> {
        match self {
            Self::Candidate { value, .. } => Some(value),
            Self::Residual { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FlowchartSourcePaintStatus {
    Absent,
    Admitted,
    Unverified,
}

impl FlowchartSourcePaintStatus {
    pub(crate) const fn from_parts(declared: bool, admitted: bool) -> Self {
        if admitted {
            Self::Admitted
        } else if declared {
            Self::Unverified
        } else {
            Self::Absent
        }
    }

    const fn overrides_theme(self) -> bool {
        !matches!(self, Self::Absent)
    }
}

#[derive(Debug, Default)]
pub(crate) struct FlowchartNodeThemeStyle {
    fill: Option<FlowchartPaintOutcome>,
    stroke: Option<FlowchartPaintOutcome>,
    matched_rules: BTreeSet<usize>,
    incomplete_rules: BTreeSet<usize>,
}

impl FlowchartNodeThemeStyle {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, ResourceLimitExceeded> {
        let Some(theme) = theme else {
            return Ok(Self::default());
        };
        let style = theme.style_with_work_meter(
            ThemeTarget::Node,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let mut resolved = Self::default();
        resolved.matched_rules.extend(style.matched_rule_indices());

        for (property, origin) in style.winner_rule_properties() {
            let rule_index = origin.rule_index();
            match property {
                ResolvedStyleProperty::Fill => {
                    resolved.fill = resolve_paint(
                        theme,
                        rule_index,
                        style.fill_resolution(),
                        FamilyThemeRuleFacet::fill,
                    );
                }
                ResolvedStyleProperty::Stroke => {
                    resolved.stroke = resolve_paint(
                        theme,
                        rule_index,
                        style.stroke_resolution(),
                        FamilyThemeRuleFacet::stroke,
                    );
                }
                ResolvedStyleProperty::Typography(property) => record_incomplete_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    FamilyThemeRuleFacet::Typography(property),
                ),
                ResolvedStyleProperty::Effect => record_incomplete_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    FamilyThemeRuleFacet::Effect,
                ),
                ResolvedStyleProperty::StrokeWidth
                | ResolvedStyleProperty::StrokeDasharray
                | ResolvedStyleProperty::StrokeLinecap
                | ResolvedStyleProperty::StrokeLinejoin
                | ResolvedStyleProperty::Opacity
                | ResolvedStyleProperty::FillOpacity
                | ResolvedStyleProperty::StrokeOpacity
                | ResolvedStyleProperty::Radius
                | ResolvedStyleProperty::Padding => record_incomplete_facet(
                    theme,
                    &mut resolved,
                    rule_index,
                    non_paint_facet(property),
                ),
            }
        }

        Ok(resolved)
    }

    pub(crate) fn fill_value(
        &self,
        source: FlowchartSourcePaintStatus,
        channel_emitted: bool,
    ) -> Option<&str> {
        (!source.overrides_theme() && channel_emitted)
            .then(|| self.fill.as_ref().and_then(FlowchartPaintOutcome::value))
            .flatten()
    }

    pub(crate) fn stroke_value(
        &self,
        source: FlowchartSourcePaintStatus,
        channel_emitted: bool,
    ) -> Option<&str> {
        (!source.overrides_theme() && channel_emitted)
            .then(|| self.stroke.as_ref().and_then(FlowchartPaintOutcome::value))
            .flatten()
    }

    pub(crate) fn append_inline_paint(
        &self,
        out: &mut String,
        source_fill: FlowchartSourcePaintStatus,
        source_stroke: FlowchartSourcePaintStatus,
        fill_emitted: bool,
        stroke_emitted: bool,
    ) {
        if let Some(fill) = self.fill_value(source_fill, fill_emitted) {
            push_inline_declaration(out, "fill", fill);
        }
        if let Some(stroke) = self.stroke_value(source_stroke, stroke_emitted) {
            push_inline_declaration(out, "stroke", stroke);
        }
    }
}

fn resolve_paint(
    theme: &ResolvedDiagramTheme,
    rule_index: usize,
    property: &ResolvedProperty<CanvasPaint>,
    facet: fn(&Specified<CanvasPaint>) -> Option<FamilyThemeRuleFacet>,
) -> Option<FlowchartPaintOutcome> {
    let facet = facet(property.specified())?;
    match theme.rule_facet_disposition(rule_index, facet)? {
        FamilyThemeDisposition::LegacyCompatibility => None,
        FamilyThemeDisposition::Unsupported => Some(FlowchartPaintOutcome::Residual {
            rule_index,
            reason: FamilyThemeResidualReason::UnsupportedPaint,
        }),
        FamilyThemeDisposition::TypedAdapter => match property.specified() {
            Specified::Value(CanvasPaint::Transparent) => Some(FlowchartPaintOutcome::Candidate {
                rule_index,
                value: "none".to_string(),
            }),
            Specified::Value(CanvasPaint::Solid(color)) => Some(FlowchartPaintOutcome::Candidate {
                rule_index,
                value: color.as_css(),
            }),
            Specified::Unspecified
            | Specified::Clear
            | Specified::Value(
                CanvasPaint::LinearGradient(_)
                | CanvasPaint::RadialGradient(_)
                | CanvasPaint::Pattern(_),
            ) => Some(FlowchartPaintOutcome::Residual {
                rule_index,
                reason: FamilyThemeResidualReason::UnsupportedPaint,
            }),
        },
    }
}

fn non_paint_facet(property: ResolvedStyleProperty) -> FamilyThemeRuleFacet {
    match property {
        ResolvedStyleProperty::StrokeWidth => FamilyThemeRuleFacet::StrokeWidth,
        ResolvedStyleProperty::StrokeDasharray => FamilyThemeRuleFacet::StrokeDasharray,
        ResolvedStyleProperty::StrokeLinecap => FamilyThemeRuleFacet::StrokeLinecap,
        ResolvedStyleProperty::StrokeLinejoin => FamilyThemeRuleFacet::StrokeLinejoin,
        ResolvedStyleProperty::Opacity => FamilyThemeRuleFacet::Opacity,
        ResolvedStyleProperty::FillOpacity => FamilyThemeRuleFacet::FillOpacity,
        ResolvedStyleProperty::StrokeOpacity => FamilyThemeRuleFacet::StrokeOpacity,
        ResolvedStyleProperty::Radius => FamilyThemeRuleFacet::Radius,
        ResolvedStyleProperty::Padding => FamilyThemeRuleFacet::Padding,
        _ => unreachable!("non-paint facet mapping received a paint property"),
    }
}

fn record_incomplete_facet(
    theme: &ResolvedDiagramTheme,
    resolved: &mut FlowchartNodeThemeStyle,
    rule_index: usize,
    facet: FamilyThemeRuleFacet,
) {
    if !matches!(
        theme.rule_facet_disposition(rule_index, facet),
        None | Some(FamilyThemeDisposition::LegacyCompatibility)
    ) {
        resolved.incomplete_rules.insert(rule_index);
    }
}

fn push_inline_declaration(out: &mut String, property: &str, value: &str) {
    if !out.is_empty() {
        out.push(';');
    }
    let _ = write!(out, "{property}:{value} !important");
}

#[derive(Debug, Default)]
struct FlowchartThemeEvidenceState {
    emitted_node: bool,
    matched_rules: BTreeSet<usize>,
    applied_rules: BTreeSet<usize>,
    residual_rules: BTreeMap<usize, FamilyThemeResidualReason>,
    incomplete_rules: BTreeSet<usize>,
    source_residuals: Vec<SourceStyleResidual>,
}

#[derive(Debug, Default)]
pub(crate) struct FlowchartThemeEvidenceRecorder {
    state: Mutex<FlowchartThemeEvidenceState>,
}

impl FlowchartThemeEvidenceRecorder {
    pub(crate) fn record_node_emission(
        &self,
        style: &FlowchartNodeThemeStyle,
        source_fill: FlowchartSourcePaintStatus,
        source_stroke: FlowchartSourcePaintStatus,
        fill_emitted: bool,
        stroke_emitted: bool,
        source_residuals: &[SourceStyleResidual],
    ) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.emitted_node = true;
        state
            .matched_rules
            .extend(style.matched_rules.iter().copied());
        record_paint_outcome(&mut state, style.fill.as_ref(), source_fill, fill_emitted);
        record_paint_outcome(
            &mut state,
            style.stroke.as_ref(),
            source_stroke,
            stroke_emitted,
        );
        state
            .incomplete_rules
            .extend(style.incomplete_rules.iter().copied());
        for residual in source_residuals {
            if !state.source_residuals.contains(residual) {
                state.source_residuals.push(residual.clone());
            }
        }
    }

    pub(crate) fn finish(
        &self,
        theme: Option<&ResolvedDiagramTheme>,
    ) -> (FamilyThemeEvidence, Vec<SourceStyleResidual>) {
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let source_residuals = state.source_residuals.clone();
        let Some(theme) = theme else {
            return (evidence, source_residuals);
        };

        let mut rule_routes = BTreeMap::<(usize, ThemeTarget), Vec<FamilyThemeRoute>>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index, target, ..
                } => rule_routes
                    .entry((rule_index, target))
                    .or_default()
                    .push(route),
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Node,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if !state.emitted_node {
                        evidence.mark_not_applicable(key);
                    } else {
                        match route.disposition() {
                            FamilyThemeDisposition::LegacyCompatibility => {
                                // The compatibility lane owns this route; its parse evidence is
                                // checked separately from typed family evidence.
                            }
                            FamilyThemeDisposition::TypedAdapter
                            | FamilyThemeDisposition::Unsupported => evidence.mark_residual(
                                key,
                                FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                            ),
                        }
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Node,
                    ..
                } if state.emitted_node => {
                    if !matches!(
                        route.disposition(),
                        FamilyThemeDisposition::LegacyCompatibility
                    ) {
                        evidence.mark_residual(
                            theme.family_mechanism_key(route),
                            FamilyThemeResidualReason::UnsupportedEffect,
                        );
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Node,
                    ..
                } => evidence.mark_not_applicable(theme.family_mechanism_key(route)),
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        for ((rule_index, target), _routes) in rule_routes {
            if target != ThemeTarget::Node {
                continue;
            }
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target,
            };
            if !state.emitted_node || !state.matched_rules.contains(&rule_index) {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = state.residual_rules.get(&rule_index).copied() {
                evidence.mark_residual(key, reason);
            } else if state.incomplete_rules.contains(&rule_index) {
                // A matching winner reached the Node consumer, but this slice does not yet share
                // enough source-shadow information to classify the facet without guessing.
            } else if state.applied_rules.contains(&rule_index) {
                evidence.mark_applied(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        (evidence, source_residuals)
    }
}

fn record_paint_outcome(
    state: &mut FlowchartThemeEvidenceState,
    outcome: Option<&FlowchartPaintOutcome>,
    source: FlowchartSourcePaintStatus,
    channel_emitted: bool,
) {
    if source.overrides_theme() {
        return;
    }
    if !channel_emitted {
        match outcome {
            Some(FlowchartPaintOutcome::Candidate { rule_index, .. }) => {
                state
                    .residual_rules
                    .entry(*rule_index)
                    .or_insert(FamilyThemeResidualReason::UnsupportedPaint);
            }
            Some(FlowchartPaintOutcome::Residual { rule_index, reason }) => {
                state.residual_rules.entry(*rule_index).or_insert(*reason);
            }
            None => {}
        }
        return;
    }
    match outcome {
        Some(FlowchartPaintOutcome::Candidate { rule_index, .. }) => {
            state.applied_rules.insert(*rule_index);
        }
        Some(FlowchartPaintOutcome::Residual { rule_index, reason }) => {
            state.residual_rules.entry(*rule_index).or_insert(*reason);
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue, ThemeRule,
        ThemeRuleSet, ThemeStylePatch,
    };
    use crate::render_family::RenderFamilyKind;

    fn resolved_theme(target: ThemeTarget) -> ResolvedDiagramTheme {
        DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            target,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .for_family(RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("compile Flowchart theme")
            .resolve(RenderFamilyKind::Flowchart)
    }

    #[test]
    fn inline_paint_preserves_existing_style_and_emitted_channels() {
        let style = FlowchartNodeThemeStyle {
            fill: Some(FlowchartPaintOutcome::Candidate {
                rule_index: 0,
                value: "#ef4444".to_string(),
            }),
            stroke: Some(FlowchartPaintOutcome::Candidate {
                rule_index: 0,
                value: "#2563eb".to_string(),
            }),
            matched_rules: BTreeSet::from([0]),
            incomplete_rules: BTreeSet::new(),
        };
        let mut inline = "opacity:0.5".to_string();

        style.append_inline_paint(
            &mut inline,
            FlowchartSourcePaintStatus::Admitted,
            FlowchartSourcePaintStatus::Absent,
            true,
            true,
        );

        assert_eq!(inline, "opacity:0.5;stroke:#2563eb !important");
    }

    #[test]
    fn unemitted_node_paint_is_residual_instead_of_applied() {
        let theme = resolved_theme(ThemeTarget::Node);
        let style = FlowchartNodeThemeStyle {
            fill: Some(FlowchartPaintOutcome::Candidate {
                rule_index: 0,
                value: "#ef4444".to_string(),
            }),
            stroke: None,
            matched_rules: BTreeSet::from([0]),
            incomplete_rules: BTreeSet::new(),
        };
        let recorder = FlowchartThemeEvidenceRecorder::default();

        recorder.record_node_emission(
            &style,
            FlowchartSourcePaintStatus::Absent,
            FlowchartSourcePaintStatus::Absent,
            false,
            false,
            &[],
        );
        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedPaint
        );
    }

    #[test]
    fn unobserved_non_node_rule_remains_unaccounted() {
        let theme = resolved_theme(ThemeTarget::Edge);
        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder.record_node_emission(
            &FlowchartNodeThemeStyle::default(),
            FlowchartSourcePaintStatus::Absent,
            FlowchartSourcePaintStatus::Absent,
            true,
            true,
            &[],
        );

        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert!(evidence.not_applicable_mechanisms().is_empty());
    }

    #[test]
    fn unmatched_node_variant_is_not_applicable() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Node,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#ef4444").unwrap()),
                        )
                        .with_variant(ThemeVariant::Active)
                        .for_family(RenderFamilyKind::Flowchart),
                    ),
                ),
            )
            .expect("compile variant theme")
            .resolve(RenderFamilyKind::Flowchart);
        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder.record_node_emission(
            &FlowchartNodeThemeStyle::default(),
            FlowchartSourcePaintStatus::Absent,
            FlowchartSourcePaintStatus::Absent,
            true,
            true,
            &[],
        );

        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert!(evidence.residuals().is_empty());
        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Node,
            }]
        );
    }

    #[test]
    fn emitted_node_rejects_an_unsupported_ordinal_palette() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_ordinal_palette(
                        ThemeTarget::Node,
                        OrdinalPalette::new([
                            ThemeColorValue::parse("#ef4444").unwrap(),
                            ThemeColorValue::parse("#2563eb").unwrap(),
                        ])
                        .unwrap(),
                    ),
                ),
            )
            .expect("compile ordinal Flowchart theme")
            .resolve(RenderFamilyKind::Flowchart);

        let (empty_evidence, source_residuals) =
            FlowchartThemeEvidenceRecorder::default().finish(Some(&theme));
        assert!(source_residuals.is_empty());
        assert!(empty_evidence.applied().is_empty());
        assert!(empty_evidence.residuals().is_empty());
        assert_eq!(
            empty_evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Node,
            }]
        );

        let recorder = FlowchartThemeEvidenceRecorder::default();
        recorder.record_node_emission(
            &FlowchartNodeThemeStyle::default(),
            FlowchartSourcePaintStatus::Absent,
            FlowchartSourcePaintStatus::Absent,
            true,
            true,
            &[],
        );

        let (evidence, source_residuals) = recorder.finish(Some(&theme));

        assert!(source_residuals.is_empty());
        assert!(evidence.applied().is_empty());
        assert!(evidence.not_applicable_mechanisms().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedOrdinalPalette
        );
    }
}
