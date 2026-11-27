use merman_model::presentation::PresentationConfig;
use merman_render_core::{
    config::{config_font_family_css, config_theme_font_size_css_or_root_number_px_opt},
    mermaid_style::font_family_css,
    number_format::{canonical_number, canonicalize_number},
    svg_css::PreparedCommonCss,
};

use super::{XyChartLayoutTypography, XyChartRoleFonts, XyChartTextRole};

/// Final XY Chart typography shared by layout and SVG emission.
#[derive(Debug)]
pub(super) struct XyChartTypographyThemePlan {
    common_css: PreparedCommonCss,
    role_fonts: XyChartRoleFonts,
    native_title_family: Option<String>,
}

impl XyChartTypographyThemePlan {
    pub(super) fn resolve(effective_config: &PresentationConfig) -> Self {
        // Bind inherited role sizes and common CSS to the same valid root, including zero.
        let root_size =
            config_theme_font_size_css_or_root_number_px_opt(effective_config).unwrap_or(16.0);
        let root_size = canonicalize_number(root_size);
        let role_fonts = XyChartRoleFonts::resolve(effective_config, root_size);
        let common_css = PreparedCommonCss::with_resolved_typography(
            effective_config,
            config_font_family_css(effective_config),
            format!("{}px", canonical_number(root_size)),
        );
        Self {
            common_css,
            role_fonts,
            native_title_family: None,
        }
    }

    pub(super) fn native(
        effective_config: &PresentationConfig,
        typography: &merman_theme_contract::v2::Typography,
    ) -> Self {
        let font_family = |role| font_family_css(typography.stack(role));
        let body_family = font_family(typography.body_family);
        let title_family = font_family(typography.title_family);
        let base = typography.base_size_px;
        let role_fonts = XyChartRoleFonts::native(effective_config, typography);
        let common_css = PreparedCommonCss::with_resolved_typography(
            effective_config,
            body_family,
            format!("{}px", canonical_number(base)),
        );
        Self {
            common_css,
            role_fonts,
            native_title_family: Some(title_family),
        }
    }

    pub(super) fn layout_typography(&self) -> XyChartLayoutTypography<'_> {
        let font_family = self.common_css.font_family();
        XyChartLayoutTypography {
            role_fonts: &self.role_fonts,
            font_family,
            title_font_family: self.native_title_family_css().unwrap_or(font_family),
        }
    }

    /// Preserve the configured SVG font size even when layout uses the inherited size.
    pub(super) fn svg_font_size(&self, role: XyChartTextRole) -> &str {
        self.role_fonts.svg_font_size(role)
    }

    pub(super) fn font_weight(&self, role: XyChartTextRole) -> Option<&str> {
        self.role_fonts.font_weight(role)
    }

    pub(super) fn common_css(&self) -> &PreparedCommonCss {
        &self.common_css
    }

    pub(super) fn native_title_family_css(&self) -> Option<&str> {
        self.native_title_family.as_deref()
    }
}
