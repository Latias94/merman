use crate::diagram_theme::{CanvasPaint, ResolvedStyleProperty, ResolvedThemeStyle, Specified};

use super::outcomes::ClassSelectedPaint;

/// Select the same string token as Class CSS before querying its frozen ownership.
/// Derived ownership belongs to that terminal path, not to unused fallback inputs.
#[derive(Debug, Clone, Copy)]
pub(super) struct ClassConfigPaint<'a> {
    css: &'a str,
    path: Option<&'static str>,
}

impl<'a> ClassConfigPaint<'a> {
    pub(super) fn select(
        config: &'a serde_json::Value,
        paths: &[&'static str],
        fallback: &'a str,
    ) -> Self {
        paths
            .iter()
            .find_map(|&path| {
                let key = path.strip_prefix("themeVariables.")?;
                let css = config.get("themeVariables")?.get(key)?.as_str()?;
                Some(Self {
                    css,
                    path: Some(path),
                })
            })
            .unwrap_or(Self {
                css: fallback,
                path: None,
            })
    }

    pub(super) fn node_fill(config: &'a serde_json::Value) -> Self {
        Self::select(
            config,
            &["themeVariables.mainBkg", "themeVariables.primaryColor"],
            "#ECECFF",
        )
    }

    pub(super) fn node_stroke(config: &'a serde_json::Value) -> Self {
        Self::select(
            config,
            &[
                "themeVariables.nodeBorder",
                "themeVariables.primaryBorderColor",
            ],
            "#9370DB",
        )
    }

    pub(super) fn class_text(config: &'a serde_json::Value) -> Self {
        Self::select(
            config,
            &[
                "themeVariables.classText",
                "themeVariables.primaryTextColor",
                "themeVariables.textColor",
            ],
            "#333",
        )
    }

    pub(super) fn background(config: &'a serde_json::Value) -> Self {
        Self::select(config, &["themeVariables.mainBkg"], "#ECECFF")
    }

    pub(super) fn css(self) -> &'a str {
        self.css
    }

    pub(super) fn config_owned(
        self,
        config: &merman_model::presentation::PresentationConfig,
    ) -> bool {
        self.path
            .is_some_and(|path| config.config_paint_overrides_typed_default(path))
    }
}

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
    typed: Option<ClassSelectedPaint>,
    selected_property: Option<ResolvedStyleProperty>,
}

impl ClassPaintBinding {
    pub(super) fn compatibility(
        config: &merman_model::presentation::PresentationConfig,
        owner_paths: &[&'static str],
        css: String,
    ) -> Self {
        let config_origin = ClassConfigPaint::select(config.as_value(), owner_paths, &css)
            .path
            .filter(|path| config.config_paint_overrides_typed_default(path));
        Self {
            compatibility_css: css.clone(),
            css,
            config_origin,
            ..Self::default()
        }
    }

    pub(super) fn lower(
        &mut self,
        style: &ResolvedThemeStyle,
        stroke: bool,
        candidate: Option<ClassSelectedPaint>,
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

    pub(super) fn typed(&self) -> Option<&ClassSelectedPaint> {
        self.typed.as_ref()
    }

    pub(super) fn stroke_property(&self) -> Option<ResolvedStyleProperty> {
        self.typed.as_ref().and(self.selected_property)
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
