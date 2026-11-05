use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ResolvedStyleProperty, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeOutcomes, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolve_direct_static_stroke, resolved_style_property_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use merman_model::presentation::PresentationConfig;

#[derive(Debug)]
pub(super) struct PacketBlockThemePlan {
    theme: ResolvedDiagramTheme,
    fill: Option<DirectStaticPaint>,
    stroke: Option<DirectStaticPaint>,
    source_fill: bool,
    source_stroke: bool,
}

impl PacketBlockThemePlan {
    pub(super) fn resolve(
        theme: &ResolvedDiagramTheme,
        config: &PresentationConfig,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let packet = crate::packet::PacketConfigView::new(config.as_value());
        let owns = |key: &str, path: &str| {
            config.config_paint_overrides_typed_default(path) && packet.style_value(key).is_some()
        };
        let source_fill = owns("blockFillColor", "themeVariables.packet.blockFillColor");
        let source_stroke = owns("blockStrokeColor", "themeVariables.packet.blockStrokeColor");
        let style =
            theme.style_with_work_meter(ThemeTarget::Node, ThemeVariant::Default, None, work)?;
        let fill = (!source_fill)
            .then(|| {
                resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::Node],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten();
        let stroke = (!source_stroke)
            .then(|| {
                resolve_direct_static_stroke(
                    theme,
                    &style,
                    &[ThemeTarget::Node],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten();
        Ok(Self {
            theme: theme.clone(),
            fill,
            stroke,
            source_fill,
            source_stroke,
        })
    }

    pub(super) fn apply_css(&self, style: &mut crate::packet::config::PacketStyleSettings) {
        if let Some(fill) = &self.fill {
            style.block_fill_color = fill.css().to_owned().into();
        }
        if let Some(stroke) = &self.stroke {
            style.block_stroke_color = stroke.css().to_owned().into();
        }
    }

    pub(super) fn finish(
        &self,
        emitted_blocks: usize,
        work: &OperationWorkMeter,
    ) -> Result<FamilyThemeOutcomes, OperationWorkError> {
        work.charge(emitted_blocks.saturating_mul(2))?;
        let fill_owners = vec![self.source_fill; emitted_blocks];
        let stroke_owners = vec![self.source_stroke; emitted_blocks];
        let mut fallbacks = FamilyThemeOutcomes::default();
        reconcile_unsupported_terminal_domains(
            &self.theme,
            &mut fallbacks,
            &[UnsupportedTerminalDomain::direct(
                ThemeTarget::Node,
                TerminalVariantDomain::uniform(emitted_blocks, ThemeVariant::Default),
            )
            .with_source_owned_fill(&fill_owners)
            .with_source_owned_stroke(&stroke_owners)],
            work,
        )?;
        let mut evidence = FamilyThemeOutcomes::default();
        for residual in fallbacks.residuals() {
            evidence.mark_residual(residual.key().clone(), residual.reason());
        }
        let mut occurrence_winners = BTreeSet::new();
        if emitted_blocks != 0 {
            let has_ordinal_rules = self.theme.family_rules().any(|(_, rule)| {
                rule.target() == ThemeTarget::Node
                    && rule.ordinal().is_some()
                    && matches!(rule.variant(), None | Some(ThemeVariant::Default))
            });
            for ordinal in 1..=if has_ordinal_rules { emitted_blocks } else { 1 } {
                work.charge(1)?;
                let style = self.theme.style_with_work_meter(
                    ThemeTarget::Node,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work,
                )?;
                occurrence_winners.extend(
                    style
                        .winner_rule_properties()
                        .filter(|(property, _)| {
                            matches!(
                                property,
                                ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke
                            )
                        })
                        .map(|(property, origin)| (origin.rule_index(), property)),
                );
            }
        }
        let mut typed_rules = BTreeSet::new();
        let mut applied = BTreeMap::<usize, BTreeSet<ThemeCapability>>::new();
        for route in self.theme.family_mechanism_routes() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Node,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            if route.disposition() != FamilyThemeDisposition::TypedAdapter {
                continue;
            }
            typed_rules.insert(rule_index);
            if emitted_blocks == 0 {
                continue;
            }
            let property = resolved_style_property_for_facet(facet);
            let paint = match property {
                ResolvedStyleProperty::Fill => self.fill.as_ref(),
                ResolvedStyleProperty::Stroke => self.stroke.as_ref(),
                _ => None,
            };
            if let Some(paint) = paint.filter(|paint| {
                paint.rule_index() == rule_index
                    && occurrence_winners.contains(&(rule_index, property))
            }) {
                applied
                    .entry(rule_index)
                    .or_default()
                    .insert(paint.capability());
            }
        }
        for index in typed_rules {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::Node,
            };
            if let Some(capabilities) = applied.remove(&index) {
                evidence.mark_applied_with_capabilities(key, capabilities);
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        evidence.merge_accounted_from(fallbacks);
        Ok(evidence)
    }
}
