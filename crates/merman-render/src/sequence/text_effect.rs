use crate::diagram_theme::{
    ResolvedDiagramTheme, ResolvedThemeEffect, ResolvedThemeStyle, SvgShadowEffect,
};

use super::{SequenceTypographyRole, SequenceTypographyThemeReceipt};

#[derive(Debug, Clone, Default)]
pub(crate) struct SequencePreparedTextEffect {
    effect: Option<SvgShadowEffect>,
    requested: bool,
    binding_used: bool,
    cleared: bool,
    unhandled: bool,
}

impl SequencePreparedTextEffect {
    pub(super) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        role: SequenceTypographyRole,
        style: Option<&ResolvedThemeStyle>,
    ) -> Self {
        let Some(theme) = theme else {
            return Self::default();
        };
        let resolution = style
            .map(|style| style.effect_resolution().clone())
            .unwrap_or_default();
        let Some(resolved) = theme.resolve_effect(role.target(), &resolution) else {
            return Self::default();
        };
        let mut prepared = Self {
            requested: true,
            ..Self::default()
        };
        match resolved {
            ResolvedThemeEffect::ClearedByRule => prepared.cleared = true,
            ResolvedThemeEffect::Rule { graph } => {
                prepared.effect = graph.and_then(SvgShadowEffect::from_graph);
            }
            ResolvedThemeEffect::Binding { graph, .. } => {
                prepared.binding_used = true;
                prepared.effect = graph.and_then(SvgShadowEffect::from_graph);
            }
        }
        prepared.unhandled = !prepared.cleared && prepared.effect.is_none();
        prepared
    }

    pub(crate) fn effect(&self) -> Option<&SvgShadowEffect> {
        self.effect.as_ref()
    }

    pub(crate) const fn cleared(&self) -> bool {
        self.cleared
    }

    pub(crate) fn configure_receipt(
        &self,
        role: SequenceTypographyRole,
        receipt: &mut SequenceTypographyThemeReceipt,
    ) {
        if self.requested {
            receipt.configure_text_effect(role, self.binding_used, self.unhandled);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        DiagramEffectSet, DiagramThemeCompiler, DiagramThemeSpec, EffectBinding, EffectGraph,
        EffectInput, EffectPrimitive, Specified, ThemeColorValue, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeVariant,
    };

    #[test]
    fn prepared_text_effect_preserves_binding_clear_and_unsupported_rule() {
        let role = SequenceTypographyRole::Note;
        assert!(!SequencePreparedTextEffect::resolve(None, role, None).requested);
        for (spread, clear) in [(0.0, false), (0.0, true), (2.0, false)] {
            let effects = DiagramEffectSet::default()
                .with_graph(
                    EffectGraph::new(
                        "shadow",
                        [EffectPrimitive::DropShadow {
                            input: EffectInput::SourceGraphic,
                            offset_x: 1.0,
                            offset_y: 1.0,
                            blur_radius: 2.0,
                            spread,
                            color: ThemeColorValue::parse("#000000").unwrap(),
                        }],
                    )
                    .unwrap(),
                )
                .unwrap()
                .with_binding(EffectBinding::new(role.target(), "shadow").unwrap())
                .unwrap();
            let mut patch = ThemeStylePatch::default();
            patch.effects.effect = if clear {
                Specified::Clear
            } else {
                Specified::Value("shadow".to_owned())
            };
            let spec = DiagramThemeSpec::new().with_effects(effects);
            let spec = if clear || spread != 0.0 {
                spec.with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(role.target(), patch)),
                )
            } else {
                spec
            };
            let theme = DiagramThemeCompiler::new().compile(spec).unwrap();
            let resolved = theme.resolve(crate::DiagramFamilyId::SEQUENCE);
            let style = resolved.style(role.target(), ThemeVariant::Default, None);
            let prepared = SequencePreparedTextEffect::resolve(Some(&resolved), role, Some(&style));
            assert!(prepared.requested);
            assert_eq!(prepared.cleared, clear);
            assert_eq!(prepared.binding_used, !clear && spread == 0.0);
            assert_eq!(prepared.unhandled, spread != 0.0);
            assert_eq!(prepared.effect.is_some(), !clear && spread == 0.0);
        }
    }
}
