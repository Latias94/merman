use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    config::{config_css_number_or_string, config_font_family_css, normalized_config_font_family},
    mermaid_style::{
        CssFontSizeOwnership, css_font_size_is_zero, css_font_size_value_override, font_family_css,
        resolve_mermaid_absolute_font_size_px, strip_css_important,
    },
    text::TextStyle,
};
use merman_theme_contract::v2::FamilyTheme;

mod css_binding;
pub(super) use css_binding::IshikawaCssBinding;

/// Final typography prepared before measuring any Ishikawa text.
/// Root `fontSize` separately owns Mermaid's bone spacing, wrapping, and tspan line step.
#[derive(Debug)]
pub(super) struct IshikawaTypographyPlan {
    font_size_css: Box<str>,
    text_style: TextStyle,
    head_style: TextStyle,
    head_uses_base_size: bool,
}

impl IshikawaTypographyPlan {
    pub(super) fn resolve(config: &PresentationConfig, spacing_font_size: f64) -> Self {
        let font_size_css = config_css_number_or_string(config, &["themeVariables", "fontSize"])
            .or_else(|| config_css_number_or_string(config, &["fontSize"]))
            .unwrap_or_else(|| format!("{spacing_font_size}px"));
        let (resolved_size, important) = resolve_text_size(&font_size_css);
        let font_size = resolved_size.unwrap_or(16.0);
        let text_style = TextStyle {
            font_family: Some(config_font_family_css(config)),
            font_size,
            ..Default::default()
        };
        // A source !important on the shared rule outranks the normal head-size rule.
        let head_style = TextStyle {
            font_size: if important { font_size } else { 14.0 },
            font_weight: Some("600".to_string()),
            ..text_style.clone()
        };
        Self {
            font_size_css: font_size_css.into_boxed_str(),
            text_style,
            head_style,
            head_uses_base_size: important,
        }
    }

    pub(super) fn apply_native(
        &mut self,
        theme: &FamilyTheme,
        config: &PresentationConfig,
        settings: &mut super::IshikawaLayoutSettings,
    ) {
        let typography = &theme.typography;
        let authored_font = ["themeVariables.fontFamily", "fontFamily"]
            .into_iter()
            .filter(|path| config.explicit_config_owns_path(path))
            .find_map(|path| config.get_str(path).and_then(normalized_config_font_family));
        self.text_style.font_family = Some(
            authored_font
                .clone()
                .unwrap_or_else(|| font_family_css(typography.stack(typography.body_family))),
        );
        self.head_style.font_family = Some(
            authored_font
                .unwrap_or_else(|| font_family_css(typography.stack(typography.title_family))),
        );
        self.head_style.font_weight = Some(typography.title_weight.to_string());

        let authored_size = [
            (
                "themeVariables.fontSize",
                &["themeVariables", "fontSize"][..],
            ),
            ("fontSize", &["fontSize"][..]),
        ]
        .into_iter()
        .filter(|(path, _)| config.explicit_config_owns_path(path))
        .find_map(|(_, path)| {
            config_css_number_or_string(config, path)
                .filter(|size| css_font_size_value_override(size).is_some())
        });
        if let Some(size_css) = authored_size {
            let (size, important) = resolve_text_size(&size_css);
            self.font_size_css = size_css.into_boxed_str();
            self.text_style.font_size = size.unwrap_or(16.0);
            self.head_style.font_size = if important {
                self.text_style.font_size
            } else {
                14.0
            };
            self.head_uses_base_size = important;
        } else {
            let size = typography.base_size_px;
            self.font_size_css = format!("{size}px").into_boxed_str();
            self.text_style.font_size = size;
            self.head_style.font_size = size;
            self.head_uses_base_size = false;
            settings.font_size = size.max(1.0);
        }
    }

    pub(super) fn text_style(&self, class_name: &str) -> &TextStyle {
        if class_name == "ishikawa-head-label" {
            &self.head_style
        } else {
            &self.text_style
        }
    }
}

fn resolve_text_size(css: &str) -> (Option<f64>, bool) {
    let size_override = css_font_size_value_override(css);
    let size = match size_override {
        // With no root typography rule, inheritance uses the browser's medium size.
        None => Some(16.0),
        Some(size) if size.ownership() == CssFontSizeOwnership::Inherited => Some(16.0),
        Some(_) => resolve_mermaid_absolute_font_size_px(css)
            .or_else(|| css_font_size_is_zero(css).then_some(0.0))
            .and_then(|size| admit_painter_font_size_px(css, size)),
    };
    (size, size_override.is_some_and(|size| size.important()))
}

/// Certify literal zero or positive normal binary32 sizes at both painter input boundaries.
/// Subnormal/overflowed coefficients or converted pixels remain unresolved; this is a numeric
/// representation contract, not a visual epsilon or a change to CSS evaluation.
fn admit_painter_font_size_px(css: &str, size: f64) -> Option<f64> {
    if size == 0.0 {
        return Some(0.0);
    }
    if size <= 0.0 || !(size as f32).is_normal() {
        return None;
    }
    let mut parser = cssparser::Parser::new(strip_css_important(css));
    matches!(parser.next(), Ok(cssparser::Token::Dimension { value, .. })
        if *value > 0.0 && value.is_normal())
    .then_some(size)
}

/// Styles shared by the classic and hand-drawn Ishikawa SVG writers.
#[derive(Debug)]
pub struct IshikawaTextThemePlan {
    css_binding: IshikawaCssBinding,
    typography: IshikawaTypographyPlan,
}

impl IshikawaTextThemePlan {
    pub(super) fn resolve(
        native: Option<&FamilyTheme>,
        config: &PresentationConfig,
        typography: IshikawaTypographyPlan,
    ) -> Self {
        let mut css_binding = IshikawaCssBinding::resolve(config);
        if let Some(theme) = native {
            css_binding.apply_native(theme, config);
        }
        Self {
            css_binding,
            typography,
        }
    }

    pub(super) fn write_text_rules(
        &self,
        css: &mut impl std::fmt::Write,
        diagram_id: impl Copy + std::fmt::Display,
    ) {
        let typography = &self.typography;
        let head_size = if typography.head_uses_base_size {
            14.0
        } else {
            typography.head_style.font_size
        };
        let _ = write!(
            css,
            "#{diagram_id} .ishikawa text {{ font-family: {}; font-size: {}; fill: {}; }}\
#{diagram_id} .ishikawa .ishikawa-head-label {{ font-weight: {}; text-anchor: middle; dominant-baseline: middle; font-size: {head_size}px;",
            typography
                .text_style
                .font_family
                .as_deref()
                .unwrap_or_default(),
            typography.font_size_css,
            self.css_binding.text_color,
            typography
                .head_style
                .font_weight
                .as_deref()
                .unwrap_or("600"),
        );
        if let Some(native) = &self.css_binding.native {
            let head_font = typography
                .head_style
                .font_family
                .as_deref()
                .unwrap_or_default();
            let _ = write!(css, " font-family: {head_font}; fill: {};", native.head_ink);
        }
        let _ = css.write_str(" }");
    }

    pub(super) fn css_binding(&self) -> &IshikawaCssBinding {
        &self.css_binding
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::presentation_config;
    use super::*;
    use serde_json::json;

    #[test]
    fn text_measurement_uses_the_render_theme_family_and_head_weight() {
        let config = presentation_config(json!({
            "fontFamily": "Cousine, sans-serif",
            "themeVariables": {"fontFamily": "Different, sans-serif", "fontSize": "18px"}
        }));
        let typography = IshikawaTypographyPlan::resolve(&config, 16.0);
        let head = typography.text_style("ishikawa-head-label");
        assert_eq!(head.font_family.as_deref(), Some("Different,sans-serif"));
        assert_eq!(head.font_size, 14.0);
        assert_eq!(head.font_weight.as_deref(), Some("600"));

        let cause = typography.text_style("ishikawa-label cause");
        assert_eq!(cause.font_family.as_deref(), Some("Different,sans-serif"));
        assert_eq!(cause.font_size, 18.0);
        assert_eq!(cause.font_weight, None);
    }

    #[test]
    fn prepared_typography_preserves_source_units_and_head_priority() {
        for (config, css, body_px, head_px) in [
            (
                json!({"fontSize":"18px !important;"}),
                "18px !important",
                18.0,
                18.0,
            ),
            (
                json!({"fontSize":"18px", "themeVariables":{"fontSize":"24pt"}}),
                "24pt",
                32.0,
                14.0,
            ),
        ] {
            let config = presentation_config(config);
            let typography = IshikawaTypographyPlan::resolve(&config, 16.0);
            assert_eq!(typography.font_size_css.as_ref(), css);
            assert_eq!(
                typography.text_style("ishikawa-label cause").font_size,
                body_px
            );
            assert_eq!(
                typography.text_style("ishikawa-head-label").font_size,
                head_px
            );
        }
    }
}
