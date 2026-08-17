use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::diagrams::tree_view::{
    TreeViewDiagramRenderModel, TreeViewNodeRenderModel as TreeViewNode,
};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, Specified,
    ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::config::DEFAULT_LINE_THICKNESS;

/// Final Tree View edge width shared by layout, terminal SVG emission, and family evidence.
#[derive(Debug)]
pub(crate) struct TreeViewEdgeThemePlan {
    line_thickness_override: Option<TreeViewTerminalStrokeWidth>,
    expected_line_count: usize,
    evidence: FamilyThemeEvidence,
    pending_stroke_width_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

#[derive(Debug)]
struct TreeViewTerminalStrokeWidth {
    value_px: f64,
    token: Box<str>,
}

impl TreeViewEdgeThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        model: &TreeViewDiagramRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline(0));
        };
        let expected_line_count = tree_view_line_count(&model.root);

        let mermaid_owns_line_thickness =
            merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "treeView.lineThickness",
            );
        let static_style = theme.style_with_work_meter(
            ThemeTarget::Edge,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let static_winners = static_style
            .winner_rule_properties()
            .into_iter()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        let static_line_thickness_candidate = (!mermaid_owns_line_thickness)
            .then(|| typed_line_thickness(theme, &static_style))
            .flatten();
        let static_winner_rule = static_line_thickness_candidate.as_ref().and_then(|_| {
            static_style
                .stroke_width_resolution()
                .winner()
                .map(|origin| origin.rule_index())
        });
        let direct_static_winner_rule = static_winner_rule
            .filter(|rule_index| has_direct_static_stroke_width_route(theme, *rule_index));

        let has_ordinal_edge_rules = theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::Edge && rule.ordinal().is_some());
        let mut occurrence_winners = BTreeSet::<(usize, ResolvedStyleProperty)>::new();
        let mut static_width_wins_every_line =
            expected_line_count != 0 && direct_static_winner_rule.is_some();
        if expected_line_count != 0 {
            if has_ordinal_edge_rules {
                for ordinal in 1..=expected_line_count {
                    let style = theme.style_with_work_meter(
                        ThemeTarget::Edge,
                        ThemeVariant::Default,
                        Some(ordinal),
                        work_meter,
                    )?;
                    occurrence_winners.extend(
                        style
                            .winner_rule_properties()
                            .into_iter()
                            .map(|(property, origin)| (origin.rule_index(), property)),
                    );
                    static_width_wins_every_line &= style
                        .stroke_width_resolution()
                        .winner()
                        .map(|origin| origin.rule_index())
                        == direct_static_winner_rule;
                }
            } else {
                occurrence_winners.extend(static_winners.iter().copied());
            }
        }
        let terminal_winner_rule = static_width_wins_every_line
            .then_some(direct_static_winner_rule)
            .flatten();
        let line_thickness_override = terminal_winner_rule.and(static_line_thickness_candidate);

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, TreeViewEdgeRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Edge,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    if !selector.ordinal_domain_intersects_occurrence_count(expected_line_count) {
                        continue;
                    }
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    let route_won = occurrence_winners.contains(&(rule_index, property));
                    if (!route_won && !qualified_variant)
                        || (mermaid_owns_line_thickness
                            && facet == FamilyThemeRuleFacet::StrokeWidth)
                    {
                        continue;
                    }

                    observation.applicable = true;
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::StrokeWidth,
                        ) if terminal_winner_rule == Some(rule_index) => {
                            observation.stroke_width_pending = true;
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
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
                    let key = theme.family_mechanism_key(route);
                    if expected_line_count == 0 {
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
                    if expected_line_count == 0 {
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

        let mut pending_stroke_width_key = None;
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
                // A mixed rule stays unaccounted until every winning facet has a terminal owner.
            } else if observation.stroke_width_pending {
                debug_assert!(pending_stroke_width_key.is_none());
                pending_stroke_width_key = Some(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            line_thickness_override,
            expected_line_count,
            evidence,
            pending_stroke_width_key,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline(expected_line_count: usize) -> Self {
        Self {
            line_thickness_override: None,
            expected_line_count,
            evidence: FamilyThemeEvidence::default(),
            pending_stroke_width_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn line_thickness_px(&self, mermaid_line_thickness: f64) -> f64 {
        self.line_thickness_override
            .as_ref()
            .map_or(mermaid_line_thickness, |width| width.value_px)
    }

    pub(crate) fn terminal_stroke_width_token(&self, emitted_stroke_width_px: f64) -> Option<&str> {
        self.line_thickness_override
            .as_ref()
            .filter(|width| width.value_px.to_bits() == emitted_stroke_width_px.to_bits())
            .map(|width| width.token.as_ref())
    }

    pub(crate) fn additional_paint_outset_px(&self) -> f64 {
        self.line_thickness_override.as_ref().map_or(0.0, |width| {
            ((width.value_px - DEFAULT_LINE_THICKNESS).max(0.0)) / 2.0
        })
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<TreeViewEdgeStrokeWidthThemeReceipt> {
        self.pending_stroke_width_key.as_ref().map(|_| {
            TreeViewEdgeStrokeWidthThemeReceipt::new(
                self.expected_line_count,
                self.line_thickness_override
                    .as_ref()
                    .expect("pending Tree View stroke width has a typed terminal value")
                    .token
                    .clone(),
            )
        })
    }

    pub(crate) fn record_terminal(&self, receipt: TreeViewEdgeStrokeWidthThemeReceipt) -> bool {
        self.pending_stroke_width_key.is_some()
            && receipt.proves(self.expected_line_count)
            && self.terminal_receipt.set(()).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(key) = self.pending_stroke_width_key.clone()
            && self.terminal_receipt.get().is_some()
        {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::BorderStyling]);
        }
        evidence
    }
}

fn typed_line_thickness(
    theme: &ResolvedDiagramTheme,
    style: &crate::diagram_theme::ResolvedThemeStyle,
) -> Option<TreeViewTerminalStrokeWidth> {
    let origin = style.stroke_width_resolution().winner()?;
    if theme.rule_facet_disposition(origin.rule_index(), FamilyThemeRuleFacet::StrokeWidth)
        != Some(FamilyThemeDisposition::TypedAdapter)
    {
        return None;
    }
    match style.stroke_width_resolution().specified() {
        Specified::Value(value) => Some(TreeViewTerminalStrokeWidth {
            value_px: f64::from(*value),
            token: value.to_string().into_boxed_str(),
        }),
        Specified::Clear => Some(TreeViewTerminalStrokeWidth {
            value_px: DEFAULT_LINE_THICKNESS,
            token: DEFAULT_LINE_THICKNESS.to_string().into_boxed_str(),
        }),
        Specified::Unspecified => None,
    }
}

fn has_direct_static_stroke_width_route(
    theme: &ResolvedDiagramTheme,
    expected_rule_index: usize,
) -> bool {
    theme
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
                        facet: FamilyThemeRuleFacet::StrokeWidth,
                    } if rule_index == expected_rule_index
                )
        })
}

fn tree_view_line_count(root: &TreeViewNode) -> usize {
    let mut line_count = 0usize;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        line_count = line_count.saturating_add(1);
        if !node.children.is_empty() {
            line_count = line_count.saturating_add(1);
        }
        stack.extend(node.children.iter());
    }
    line_count
}

/// Writer-owned proof that every Tree View line emitted the resolved terminal width once.
#[derive(Debug)]
pub(crate) struct TreeViewEdgeStrokeWidthThemeReceipt {
    expected_line_count: usize,
    expected_stroke_width_token: Box<str>,
    next_line_index: usize,
    attributes_match: bool,
}

impl TreeViewEdgeStrokeWidthThemeReceipt {
    fn new(expected_line_count: usize, expected_stroke_width_token: Box<str>) -> Self {
        Self {
            expected_line_count,
            expected_stroke_width_token,
            next_line_index: 0,
            attributes_match: true,
        }
    }

    pub(crate) fn record_checkpointed_line(
        &mut self,
        line_index: usize,
        emitted_stroke_width_token: Option<&str>,
    ) {
        if line_index != self.next_line_index {
            self.attributes_match = false;
            return;
        }
        self.next_line_index = self.next_line_index.saturating_add(1);
        self.attributes_match &=
            emitted_stroke_width_token == Some(self.expected_stroke_width_token.as_ref());
    }

    fn proves(&self, expected_line_count: usize) -> bool {
        self.expected_line_count == expected_line_count
            && self.next_line_index == expected_line_count
            && self.attributes_match
    }
}

#[derive(Debug, Default)]
struct TreeViewEdgeRuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    stroke_width_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::TreeViewEdgeStrokeWidthThemeReceipt;

    #[test]
    fn edge_width_receipt_requires_ordered_matching_terminal_lines() {
        let mut complete = TreeViewEdgeStrokeWidthThemeReceipt::new(1, "6".into());
        complete.record_checkpointed_line(0, Some("6"));
        assert!(complete.proves(1));

        let mut incomplete = TreeViewEdgeStrokeWidthThemeReceipt::new(2, "6".into());
        incomplete.record_checkpointed_line(0, Some("6"));
        assert!(!incomplete.proves(2));

        let mut duplicate = TreeViewEdgeStrokeWidthThemeReceipt::new(1, "6".into());
        duplicate.record_checkpointed_line(0, Some("6"));
        duplicate.record_checkpointed_line(0, Some("6"));
        assert!(!duplicate.proves(1));

        let mut out_of_order = TreeViewEdgeStrokeWidthThemeReceipt::new(2, "6".into());
        out_of_order.record_checkpointed_line(1, Some("6"));
        out_of_order.record_checkpointed_line(0, Some("6"));
        assert!(!out_of_order.proves(2));

        let mut mismatch = TreeViewEdgeStrokeWidthThemeReceipt::new(1, "5.9999995".into());
        mismatch.record_checkpointed_line(0, Some("6"));
        assert!(!mismatch.proves(1));
    }
}
