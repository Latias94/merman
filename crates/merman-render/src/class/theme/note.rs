use crate::diagram_theme::{
    ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, resolve_direct_static_fill, resolve_direct_static_stroke,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::outcomes::ClassSelectedPaint;
use super::paint::ClassPaintBinding;

/// Note paths retain independent compatibility owners for their fill and outline.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClassNoteThemePlan {
    pub(super) fill: ClassPaintBinding,
    pub(super) stroke: ClassPaintBinding,
}

impl ClassNoteThemePlan {
    pub(super) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_model::presentation::PresentationConfig,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self {
            fill: ClassPaintBinding::compatibility(
                config,
                &["themeVariables.noteBkgColor"],
                crate::config::config_string(
                    config.as_value(),
                    &["themeVariables", "noteBkgColor"],
                )
                .unwrap_or_else(|| "#fff5ad".into()),
            ),
            stroke: ClassPaintBinding::compatibility(
                config,
                &["themeVariables.noteBorderColor"],
                crate::config::config_string(
                    config.as_value(),
                    &["themeVariables", "noteBorderColor"],
                )
                .unwrap_or_else(|| "#aaaa33".into()),
            ),
        };
        let Some(theme) = theme else { return Ok(plan) };
        let style = theme.style_with_work_meter(
            ThemeTarget::Note,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        for (binding, stroke) in [(&mut plan.fill, false), (&mut plan.stroke, true)] {
            let candidate = if stroke {
                resolve_direct_static_stroke(
                    theme,
                    &style,
                    &[ThemeTarget::Note],
                    DirectStaticSelectorDomain::Default,
                )
            } else {
                resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::Note],
                    DirectStaticSelectorDomain::Default,
                )
            }
            .map(|paint| {
                let (css, rule_index, _) = paint.into_parts();
                ClassSelectedPaint {
                    target: ThemeTarget::Note,
                    rule_index,
                    css: css.into_string(),
                }
            });
            binding.lower(&style, stroke, candidate);
        }
        Ok(plan)
    }

    pub(crate) fn fill(&self) -> &str {
        self.fill.css()
    }

    pub(crate) fn stroke(&self) -> &str {
        self.stroke.css()
    }

    pub(crate) fn fill_rule(&self) -> Option<usize> {
        self.fill.typed().map(|paint| paint.rule_index)
    }

    pub(crate) fn stroke_rule(&self) -> Option<usize> {
        self.stroke.typed().map(|paint| paint.rule_index)
    }

    pub(super) fn property_overridden(&self, property: ResolvedStyleProperty) -> bool {
        match property {
            ResolvedStyleProperty::Fill => self.fill.config_owned(),
            ResolvedStyleProperty::Stroke => self.stroke.config_owned(),
            _ => false,
        }
    }
}
