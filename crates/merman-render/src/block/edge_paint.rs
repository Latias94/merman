use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedStyleProperty, Specified, ThemeCapability, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeOutcomes,
    FamilyThemeResidualReason, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::BlockDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

/// Block owns edge paint at the visible path, independently of shared marker definitions.
#[derive(Debug)]
pub(crate) struct BlockEdgePaintPlan {
    declarations: Box<[Cow<'static, str>]>,
    evidence: FamilyThemeOutcomes,
}

#[derive(Default)]
struct RuleObservation {
    applicable: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    capabilities: BTreeSet<ThemeCapability>,
}

impl BlockEdgePaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        Self::resolve_paint(theme, config, model, layout, work)
    }

    fn resolve_paint(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        model: &merman_core::diagrams::block::BlockDiagramRenderModel,
        layout: &BlockDiagramLayout,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        work.charge(model.edges.len())?;
        let mut plan = Self {
            declarations: (0..model.edges.len())
                .map(|_| Cow::Borrowed("undefined;;;undefined"))
                .collect(),
            evidence: FamilyThemeOutcomes::default(),
        };
        let Some(theme) = theme else { return Ok(plan) };
        if !theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Edge,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Edge
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Edge,
                    ..
                }
            )
        }) {
            return Ok(plan);
        }
        let config_owned = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.lineColor",
        );
        // The source class rule `.block > *` matches paths through their actual parent group.
        let source_owned = config_owned
            || model.class_defs.values().any(|class| {
                class.id == "block"
                    && class.styles.iter().any(|raw| {
                        crate::mermaid_style::parse_style_declaration(raw).is_some_and(
                            |declaration| matches!(declaration.property(), "stroke" | "all"),
                        )
                    })
            });
        let has_ordinal = theme
            .family_rules()
            .any(|(_, rule)| rule.target() == ThemeTarget::Edge && rule.ordinal().is_some());
        let static_style =
            theme.style_with_work_meter(ThemeTarget::Edge, ThemeVariant::Default, None, work)?;
        let mut layout_by_id = BTreeMap::new();
        for edge in &layout.edges {
            work.charge(1)?;
            layout_by_id.entry(edge.id.as_str()).or_insert(edge);
        }
        let geometries = layout
            .shape_geometries
            .iter()
            .map(|geometry| (geometry.id.as_str(), geometry))
            .collect::<std::collections::HashMap<_, _>>();
        let mut winners = BTreeSet::new();
        let mut capabilities = BTreeMap::new();
        let mut source_owned_fill = Vec::with_capacity(model.edges.len());
        let mut source_owned_stroke = Vec::with_capacity(model.edges.len());
        for (index, edge) in model.edges.iter().enumerate() {
            work.charge(1)?;
            let dynamic_style;
            let style = if has_ordinal {
                dynamic_style = theme.style_with_work_meter(
                    ThemeTarget::Edge,
                    ThemeVariant::Default,
                    Some(index + 1),
                    work,
                )?;
                &dynamic_style
            } else {
                &static_style
            };
            // Fill is the historical line-paint fallback. Clear and unsupported stroke
            // values still own the terminal, so neither can reactivate the fill winner.
            let fill_fallback = matches!(
                style.stroke_resolution().specified(),
                Specified::Unspecified
            );
            let visibility = layout_by_id.get(edge.id.as_str()).and_then(|layout_edge| {
                crate::svg::block_edge_is_visible(edge, layout_edge, &geometries)
            });
            // A valid degenerate path (for example A --> A) has no edge-paint terminal.
            let invisible = visibility == Some(false);
            source_owned_stroke.push(source_owned || invisible);
            source_owned_fill.push(source_owned || !fill_fallback || invisible);
            winners.extend(
                style
                    .winner_rule_properties()
                    .filter_map(|(property, origin)| {
                        (!invisible
                            && !(property == ResolvedStyleProperty::Fill
                                && (!fill_fallback || source_owned))
                            && !(property == ResolvedStyleProperty::Stroke && source_owned))
                            .then_some((origin.rule_index(), property))
                    }),
            );
            let paint = if source_owned {
                None
            } else if fill_fallback {
                resolve_direct_static_fill(
                    theme,
                    style,
                    &[ThemeTarget::Edge],
                    DirectStaticSelectorDomain::Default,
                )
            } else {
                resolve_direct_static_stroke(
                    theme,
                    style,
                    &[ThemeTarget::Edge],
                    DirectStaticSelectorDomain::Default,
                )
            }
            .map(DirectPaintExpectation::from_paint);
            if let Some(paint) = &paint
                && !invisible
            {
                capabilities.insert(
                    (
                        paint.rule_index(),
                        if fill_fallback {
                            ResolvedStyleProperty::Fill
                        } else {
                            ResolvedStyleProperty::Stroke
                        },
                    ),
                    paint.capability(),
                );
            }
            if let Some(paint) = paint {
                plan.declarations[index] = Cow::Owned(format!("stroke:{};fill:none;", paint.css()));
            }
        }
        let mut observations = BTreeMap::<usize, RuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Edge,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            let observation = observations.entry(rule_index).or_default();
            let property = resolved_style_property_for_facet(facet);
            if !winners.contains(&(rule_index, property)) {
                continue;
            }
            observation.applicable = true;
            match route.disposition() {
                FamilyThemeDisposition::TypedAdapter => {
                    if matches!(
                        facet,
                        FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_)
                    ) && let Some(capability) = capabilities.get(&(rule_index, property))
                    {
                        observation.capabilities.insert(*capability);
                    } else {
                        observation.incomplete = true;
                    }
                }
                FamilyThemeDisposition::Unsupported => {
                    observation
                        .residual
                        .get_or_insert(unsupported_residual_for_facet(facet));
                }
            }
        }
        for (index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::Edge,
            };
            if !observation.applicable {
                plan.evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                plan.evidence.mark_residual(key, reason);
            } else if !observation.incomplete && !observation.capabilities.is_empty() {
                plan.evidence
                    .mark_applied_with_capabilities(key, observation.capabilities);
            }
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Edge,
                TerminalVariantDomain::uniform(model.edges.len(), ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned_fill)
            .with_source_owned_stroke(&source_owned_stroke)],
            work,
        )?;
        Ok(plan)
    }

    pub(crate) fn declaration(&self, edge_index: usize) -> Option<&str> {
        self.declarations.get(edge_index).map(AsRef::as_ref)
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        evidence.merge_borrowed(&self.evidence);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unthemed_edges_have_prepared_historical_declarations() {
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "block-beta\nA --> B\n",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let merman_core::RenderSemanticModel::Block(model) = parsed.model() else {
            panic!("Block model")
        };
        let layout = crate::block::layout_block_diagram_typed(
            model,
            parsed.metadata().effective_config.as_value(),
            &crate::text::DeterministicTextMeasurer::default(),
        )
        .unwrap();
        let work = OperationWorkMeter::new(crate::resources::RenderResourcePolicy::interactive());
        let plan = BlockEdgePaintPlan::resolve(
            None,
            &parsed.metadata().effective_config,
            model,
            &layout,
            &work,
        )
        .unwrap();
        assert_eq!(plan.declarations.len(), model.edges.len());
        assert_eq!(plan.declaration(0), Some("undefined;;;undefined"));
        assert_eq!(plan.declaration(model.edges.len()), None);
    }
}
