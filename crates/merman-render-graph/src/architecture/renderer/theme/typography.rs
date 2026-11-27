use merman_model::presentation::PresentationConfig;

use super::super::{ArchitectureCssBinding, ArchitectureNativeStyle, native_theme::authored};
use merman_render_core::config::configured_theme_font_size_css_or_root_number_px;
use merman_render_core::mermaid_style::{css_font_size_value_override, parse_style_declaration};
use merman_theme_contract::v2::FamilyTheme;

/// Resolves Architecture's inherited SVG typography without taking ownership of Cytoscape text.
///
/// Architecture has two intentionally separate font-size consumers. The base theme size applies
/// to SVG service/group/edge labels and CSS-inherited `iconText`; `architecture.fontSize` remains
/// the Cytoscape layout and group-icon-alignment owner. Keeping those values separate preserves
/// Mermaid's layout semantics.
#[derive(Debug)]
pub struct ArchitectureTypographyThemePlan {
    css_binding: ArchitectureCssBinding,
    font_family_css: String,
    font_size_px: f64,
}

impl ArchitectureTypographyThemePlan {
    pub(crate) fn resolve(
        config: &PresentationConfig,
        native: Option<(&FamilyTheme, &ArchitectureNativeStyle)>,
    ) -> Self {
        use merman_render_core::number_format::{canonical_number, canonicalize_number};

        let config_value = config.as_value();
        let configured_size = configured_theme_font_size_css_or_root_number_px(config_value);
        let native_size = native.and_then(|(theme, _)| {
            let explicit_size = authored(config, &["themeVariables.fontSize"])
                .is_some_and(|size| css_font_size_value_override(size).is_some())
                || configured_size.is_some_and(|(_, path)| config.explicit_config_owns_path(path));
            (!explicit_size).then_some(theme.typography.base_size_px)
        });
        let (font_size_px, font_size_css) = if let Some(size) = native_size {
            (size, format!("{}px", canonical_number(size)))
        } else {
            let relative_size = config.get_str("themeVariables.fontSize").filter(|raw| {
                // The root is an ordinary CSS declaration, unlike C4's element CSSOM value.
                configured_size.is_none_or(|(_, path)| path != "themeVariables.fontSize")
                    && parse_style_declaration(&format!("font-size:{raw}")).is_some_and(
                        |declaration| {
                            css_font_size_value_override(declaration.source_value()).is_some()
                        },
                    )
            });
            if let Some(css) = relative_size {
                // Root-relative CSS retains its declaration. Layout uses the numeric fallback.
                (16.0, css.trim().to_owned())
            } else {
                let size = canonicalize_number(configured_size.map_or(16.0, |(size, _)| size));
                (size, format!("{}px", canonical_number(size)))
            }
        };

        let font_family_css = native.map_or_else(
            || merman_render_core::config::config_font_family_css(config),
            |(_, style)| style.layout_text.font_family.clone().unwrap_or_default(),
        );
        let mut css_binding =
            ArchitectureCssBinding::resolve(config_value, &font_family_css, font_size_css);
        if let Some((theme, style)) = native {
            css_binding.apply_native(theme, style, config);
        }
        Self {
            css_binding,
            font_family_css,
            font_size_px,
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub const fn css_binding(&self) -> &ArchitectureCssBinding {
        &self.css_binding
    }

    pub(crate) const fn font_size_px(&self) -> f64 {
        self.font_size_px
    }

    pub fn svg_text_style(&self) -> merman_render_core::text::TextStyle {
        merman_render_core::text::TextStyle {
            font_family: Some(self.font_family_css().to_owned()),
            font_size: self.font_size_px(),
            font_weight: None,
            font_style: None,
        }
    }
}
