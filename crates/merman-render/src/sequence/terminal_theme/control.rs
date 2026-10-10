use super::*;
use crate::diagram_theme::SvgShadowEffect;
#[derive(Debug, Default)]
pub(crate) struct SequencePreparedControlTheme {
    pub(crate) stroke_width: Option<f32>,
    pub(crate) typed_fill: Option<String>,
    pub(crate) typed_stroke: Option<String>,
    pub(crate) fill_overridden: bool,
    pub(crate) stroke_overridden: bool,
    pub(crate) effect: Option<SvgShadowEffect>,
    effect_requested: bool,
    effect_binding_used: bool,
    effect_cleared: bool,
    effect_unhandled: bool,
    selected_style: Option<ResolvedThemeStyle>,
}

impl SequencePreparedControlTheme {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        target: crate::diagram_theme::ThemeTarget,
        fill_overridden: bool,
        stroke_overridden: bool,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<SequencePreparedControlTheme> {
        use crate::diagram_theme::{
            FamilyThemeMechanism, FamilyThemeSelectorShape, ThemeTarget, ThemeVariant,
        };

        let Some(theme) = theme else {
            return Ok(Self {
                fill_overridden,
                stroke_overridden,
                ..Self::default()
            });
        };
        let has_rule_routes = theme.family_mechanism_routes().iter().any(|route| {
        matches!(route.mechanism(),
            FamilyThemeMechanism::RuleFacet {
                target: route_target,
                selector: FamilyThemeSelectorShape::Static { variant: None | Some(ThemeVariant::Default) },
                ..
            } | FamilyThemeMechanism::EffectBinding { target: route_target, .. }
            if route_target == target)
    });
        if !has_rule_routes {
            return Ok(Self {
                fill_overridden,
                stroke_overridden,
                ..Self::default()
            });
        }

        let style = theme.style_with_work_meter(target, ThemeVariant::Default, None, work_meter)?;
        let typed_fill = (target == ThemeTarget::LoopLabelBackground && !fill_overridden)
            .then(|| typed_static_sequence_fill(theme, &style))
            .flatten();
        let typed_stroke = (!stroke_overridden)
            .then(|| typed_static_sequence_stroke(theme, &style))
            .flatten();
        let SequencePreparedEffect {
            effect,
            effect_requested,
            effect_binding_used,
            effect_cleared,
            effect_unhandled,
        } = prepare_sequence_effect(theme, target, style.effect_resolution());
        Ok(SequencePreparedControlTheme {
            effect,
            stroke_width: style.stroke_width(),
            typed_fill,
            typed_stroke,
            fill_overridden,
            stroke_overridden,
            effect_requested,
            effect_binding_used,
            effect_cleared,
            effect_unhandled,
            selected_style: Some(style),
        })
    }

    pub(crate) fn fresh_receipt(&self) -> crate::sequence::SequenceControlThemeReceipt {
        let mut receipt = crate::sequence::SequenceControlThemeReceipt::default();
        if let Some(style) = &self.selected_style {
            receipt.record_static_style(style);
        }
        receipt.effect_requested = self.effect_requested;
        receipt.effect_binding_used = self.effect_binding_used;
        receipt.effect_cleared = self.effect_cleared;
        receipt.effect_unhandled.set(self.effect_unhandled);
        receipt
    }
}
