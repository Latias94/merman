use merman_model::presentation::PresentationConfig;

use crate::config::configured_theme_font_size_css_or_root_number_px;
use crate::mermaid_style::{css_font_size_value_override, parse_style_declaration};

/// Resolves Architecture's inherited SVG typography without taking ownership of Cytoscape text.
///
/// Architecture has two intentionally separate font-size consumers. The base theme size applies
/// to SVG service/group/edge labels and CSS-inherited `iconText`; `architecture.fontSize` remains
/// the Cytoscape layout and group-icon-alignment owner. Keeping those values separate preserves
/// Mermaid's layout semantics.
#[derive(Debug)]
pub(crate) struct ArchitectureTypographyThemePlan {
    css_binding: super::super::ArchitectureCssBinding,
    font_family_css: String,
    font_size_css: Box<str>,
    font_size_px: f64,
}

impl ArchitectureTypographyThemePlan {
    pub(super) fn apply_native(
        &mut self,
        theme: &crate::theme::FamilyTheme,
        native: &super::super::ArchitectureNativeStyle,
        config: &PresentationConfig,
    ) {
        let explicit_size =
            super::super::native_theme::authored(config, &["themeVariables.fontSize"])
                .is_some_and(|size| css_font_size_value_override(size).is_some())
                || configured_theme_font_size_css_or_root_number_px(config.as_value())
                    .is_some_and(|(_, path)| config.explicit_config_owns_path(path));
        if !explicit_size {
            self.font_size_px = theme.typography.base_size_px;
            self.font_size_css = format!(
                "{}px",
                crate::number_format::canonical_number(self.font_size_px)
            )
            .into();
        }
        let family = native
            .layout_text
            .font_family
            .as_deref()
            .unwrap_or_default();
        self.font_family_css = family.to_owned();
        self.css_binding = super::super::ArchitectureCssBinding::resolve(
            config.as_value(),
            family,
            &self.font_size_css,
        );
        self.css_binding.apply_native(theme, native, config);
    }

    pub(crate) fn resolve(effective_config: &PresentationConfig) -> Self {
        let configured_size =
            configured_theme_font_size_css_or_root_number_px(effective_config.as_value());
        let configured_css = effective_config
            .get_str("themeVariables.fontSize")
            .filter(|raw| {
                // The root is an ordinary CSS declaration, unlike C4's element CSSOM value.
                parse_style_declaration(&format!("font-size:{raw}")).is_some_and(|declaration| {
                    css_font_size_value_override(declaration.source_value()).is_some()
                })
            });
        let font_size_known = configured_css.is_none()
            || configured_size.is_some_and(|(_, path)| path == "themeVariables.fontSize");
        let font_size_px = if font_size_known {
            configured_size.map_or(16.0, |(size, _)| size)
        } else {
            16.0
        };
        let font_size_px = crate::number_format::canonicalize_number(font_size_px);
        let font_size_css: Box<str> = if font_size_known {
            format!("{}px", crate::number_format::canonical_number(font_size_px)).into_boxed_str()
        } else {
            // Root-relative sizes and host expressions retain their CSS owner. The numeric
            // fallback is used for the layout metrics.
            configured_css
                .expect("unknown root size retains its CSS")
                .trim()
                .into()
        };

        let font_family_css = crate::config::config_font_family_css(effective_config);
        let css_binding = super::super::ArchitectureCssBinding::resolve(
            effective_config.as_value(),
            &font_family_css,
            &font_size_css,
        );
        Self {
            css_binding,
            font_family_css,
            font_size_css,
            font_size_px,
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub(crate) const fn css_binding(&self) -> &super::super::ArchitectureCssBinding {
        &self.css_binding
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    pub(crate) fn svg_text_style(&self) -> crate::text::TextStyle {
        crate::text::TextStyle {
            font_family: Some(self.font_family_css().to_owned()),
            font_size: self.font_size_px(),
            font_weight: None,
            font_style: None,
        }
    }
}
