use crate::diagram_theme::{CanvasPaint, ResolvedStyleProperty, ResolvedThemeStyle, Specified};

use super::terminal::ExpectedPaint;

#[derive(Debug, Clone, Default)]
pub(super) enum ClassPaintAction {
    #[default]
    Inherit,
    Set {
        rule_index: usize,
    },
    Clear {
        rule_index: usize,
    },
    Residual {
        rule_index: usize,
    },
}

/// The selected browser token and recipe action share the exact compatibility origin.
/// Unsupported CSS remains a token, rather than becoming a fabricated native color.
#[derive(Debug, Clone, Default)]
pub(super) struct ClassPaintBinding {
    css: String,
    compatibility_css: String,
    config_origin: Option<&'static str>,
    action: ClassPaintAction,
    typed: Option<ExpectedPaint>,
    selected_property: Option<ResolvedStyleProperty>,
}

impl ClassPaintBinding {
    pub(super) fn compatibility(
        config: &merman_core::MermaidConfig,
        owner_paths: &[&'static str],
        css: String,
    ) -> Self {
        Self {
            compatibility_css: css.clone(),
            css,
            config_origin: owner_paths.iter().copied().find(|path| {
                merman_core::__private::config_path_overrides_typed_default(config, path)
            }),
            ..Self::default()
        }
    }

    pub(super) fn lower(
        &mut self,
        style: &ResolvedThemeStyle,
        stroke: bool,
        candidate: Option<ExpectedPaint>,
    ) {
        if self.config_origin.is_some() {
            return;
        }
        let resolution = if stroke {
            style.stroke_resolution()
        } else {
            style.fill_resolution()
        };
        self.selected_property = Some(if stroke {
            ResolvedStyleProperty::Stroke
        } else {
            ResolvedStyleProperty::Fill
        });
        self.action = resolution
            .winner()
            .map_or(ClassPaintAction::Inherit, |origin| {
                let rule_index = origin.rule_index();
                match resolution.specified() {
                    Specified::Clear => ClassPaintAction::Clear { rule_index },
                    Specified::Value(CanvasPaint::Solid(_) | CanvasPaint::Transparent) => {
                        ClassPaintAction::Set { rule_index }
                    }
                    Specified::Value(_) => ClassPaintAction::Residual { rule_index },
                    Specified::Unspecified => ClassPaintAction::Inherit,
                }
            });
        if let Some(candidate) = &candidate {
            self.css.clone_from(&candidate.css);
        }
        self.typed = candidate;
        debug_assert!(
            self.typed
                .as_ref()
                .is_none_or(|paint| self.action_rule() == Some(paint.rule_index))
        );
    }

    pub(super) fn config_owned(&self) -> bool {
        self.config_origin.is_some()
    }

    pub(super) fn typed(&self) -> Option<&ExpectedPaint> {
        self.typed.as_ref()
    }

    pub(super) fn stroke_receipt(&self) -> Option<super::terminal::ExpectedStroke> {
        let typed = self.typed()?;
        Some(super::terminal::ExpectedStroke {
            rule_index: typed.rule_index,
            property: self.selected_property?,
            css: typed.css.clone(),
        })
    }

    pub(super) fn css(&self) -> &str {
        &self.css
    }

    pub(super) fn compatibility_css(&self) -> &str {
        &self.compatibility_css
    }

    pub(super) fn action_rule(&self) -> Option<usize> {
        match self.action {
            ClassPaintAction::Inherit => None,
            ClassPaintAction::Set { rule_index }
            | ClassPaintAction::Clear { rule_index }
            | ClassPaintAction::Residual { rule_index } => Some(rule_index),
        }
    }
}
