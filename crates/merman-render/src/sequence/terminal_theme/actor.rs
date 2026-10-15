use super::*;
use crate::diagram_theme::SvgShadowEffect;
#[derive(Debug, Default)]
pub(crate) struct SequencePreparedActorTheme {
    pub(crate) stroke_width: Option<f32>,
    pub(crate) radius: Option<f32>,
    pub(crate) typed_fill: Option<String>,
    pub(crate) typed_stroke: Option<String>,
    pub(crate) fill_overridden: bool,
    pub(crate) stroke_overridden: bool,
    width_overridden: bool,
    ordinal_styles: Vec<ResolvedThemeStyle>,
    pub(crate) effect: Option<SvgShadowEffect>,
    effect_requested: bool,
    effect_binding_used: bool,
    effect_cleared: bool,
    effect_unhandled: bool,
    selected_style: Option<ResolvedThemeStyle>,
}

impl SequencePreparedActorTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        sanitize_config: &merman_core::MermaidConfig,
        model: &merman_core::diagrams::sequence::SequenceDiagramRenderModel,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<SequencePreparedActorTheme> {
        use crate::diagram_theme::{
            FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemePaintKind,
            FamilyThemeRuleFacet, FamilyThemeSelectorShape, ThemeTarget, ThemeVariant,
        };

        let has_actors = !model.actor_order.is_empty();
        let fill_overridden = merman_core::__private::config_path_overrides_typed_default(
            sanitize_config,
            "themeVariables.actorBkg",
        );
        let stroke_overridden = merman_core::__private::config_path_overrides_typed_default(
            sanitize_config,
            "themeVariables.actorBorder",
        );
        let Some(theme) = theme else {
            return Ok(Self {
                fill_overridden,
                stroke_overridden,
                ..Self::default()
            });
        };
        let has_actor_rule_routes = theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Actor,
                    ..
                }
            )
        });
        let has_typed_actor_fill = theme.family_mechanism_routes().iter().any(|route| {
            route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Actor,
                        facet: FamilyThemeRuleFacet::Fill(
                            FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                        ),
                        ..
                    }
                )
        });
        let has_typed_actor_stroke = theme.family_mechanism_routes().iter().any(|route| {
            route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Actor,
                        facet: FamilyThemeRuleFacet::Stroke(
                            FamilyThemePaintKind::Transparent | FamilyThemePaintKind::Solid
                        ),
                        ..
                    }
                )
        });
        let has_ordinal_routes = theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::Actor,
                    selector: FamilyThemeSelectorShape::Ordinal {
                        variant: None | Some(ThemeVariant::Default),
                        ..
                    },
                    ..
                }
            )
        });
        let style = if has_actors && has_actor_rule_routes {
            let style = theme.style_with_work_meter(
                ThemeTarget::Actor,
                ThemeVariant::Default,
                None,
                work_meter,
            )?;
            Some(style)
        } else {
            None
        };
        let typed_fill = (!fill_overridden && has_typed_actor_fill)
            .then(|| style.as_ref().and_then(|style| css_paint(style.fill())))
            .flatten();
        let typed_stroke = (!stroke_overridden && has_typed_actor_stroke)
            .then(|| style.as_ref().and_then(|style| css_paint(style.stroke())))
            .flatten();
        let width_overridden = merman_core::__private::config_path_overrides_typed_default(
            sanitize_config,
            "themeVariables.strokeWidth",
        );
        let stroke_width = style
            .as_ref()
            .and_then(|s| s.stroke_width())
            .filter(|_| !width_overridden);
        let radius = style.as_ref().and_then(|s| s.radius());
        let default_effect_resolution = Default::default();
        let effect_resolution = style
            .as_ref()
            .map(|style| style.effect_resolution())
            .unwrap_or(&default_effect_resolution);
        let SequencePreparedEffect {
            effect,
            effect_requested,
            effect_binding_used,
            effect_cleared,
            effect_unhandled,
        } = prepare_sequence_effect(theme, ThemeTarget::Actor, effect_resolution);
        let mut ordinal_styles = Vec::new();
        if has_ordinal_routes {
            for (index, actor_id) in model.actor_order.iter().enumerate() {
                work_meter.charge(1)?;
                if model.actors.contains_key(actor_id) {
                    ordinal_styles.push(theme.style_with_work_meter(
                        ThemeTarget::Actor,
                        ThemeVariant::Default,
                        Some(index + 1),
                        work_meter,
                    )?);
                }
            }
        }
        Ok(SequencePreparedActorTheme {
            effect,
            stroke_width,
            radius,
            fill_overridden,
            stroke_overridden,
            width_overridden,
            ordinal_styles,
            typed_fill,
            typed_stroke,
            effect_requested,
            effect_binding_used,
            effect_cleared,
            effect_unhandled,
            selected_style: style,
        })
    }

    pub(crate) fn fresh_receipt(&self) -> crate::sequence::SequenceActorThemeReceipt {
        let mut receipt = crate::sequence::SequenceActorThemeReceipt::default();
        if let Some(style) = &self.selected_style {
            receipt.record_static_style(style);
        }
        receipt.effect_requested = self.effect_requested;
        receipt.effect_binding_used = self.effect_binding_used;
        receipt.effect_cleared = self.effect_cleared;
        receipt.effect_unhandled = self.effect_unhandled;
        receipt.record_stroke_width_override(self.width_overridden);
        for style in &self.ordinal_styles {
            receipt.record_ordinal_style(style);
        }
        receipt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, OrdinalSelector, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    #[test]
    fn prepared_ordinals_keep_semantic_positions_when_actor_entries_are_missing() {
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "sequenceDiagram\nA->>B: Hello",
                merman_core::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let merman_core::RenderSemanticModel::Sequence(model) = parsed.model() else {
            panic!("expected Sequence model");
        };
        let mut model = model.clone();
        let removed = model.actor_order[0].clone();
        model.actors.remove(&removed);
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::Actor,
                            ThemeStylePatch::default()
                                .with_fill(CanvasPaint::solid("#123456").unwrap()),
                        )
                        .with_ordinal(OrdinalSelector::exact(2).unwrap()),
                    ),
                ),
            )
            .unwrap();
        let resolved = theme.resolve(crate::DiagramFamilyId::SEQUENCE);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let prepared = SequencePreparedActorTheme::resolve(
            Some(&resolved),
            &merman_core::MermaidConfig::default(),
            &model,
            &meter,
        )
        .unwrap();
        assert_eq!(prepared.ordinal_styles.len(), 1);
        assert_eq!(
            prepared.ordinal_styles[0].fill(),
            Some(&CanvasPaint::solid("#123456").unwrap())
        );
        assert_eq!(prepared.typed_fill, None);
    }
}
