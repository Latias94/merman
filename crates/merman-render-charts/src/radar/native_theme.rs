use merman_model::presentation::PresentationConfig;
use merman_render_core::mermaid_style::{
    font_family_css, resolve_explicit_css_absolute_font_size_px,
};
use merman_theme_contract::v2::FamilyTheme;

use super::css_binding::{RadarCssBinding, RadarRoleFontSize};

#[derive(Debug)]
pub(super) struct RadarNativeStyle {
    pub(super) body_family: Option<String>,
    pub(super) title_family: Option<String>,
    pub(super) title_weight: u16,
    pub(super) label_weight: u16,
    pub(super) show_grid: bool,
}

impl RadarNativeStyle {
    pub(super) fn bind(
        theme: &FamilyTheme,
        config: &PresentationConfig,
        font_size_css: &str,
        binding: &mut RadarCssBinding,
    ) -> Self {
        let roles = &theme.roles;
        let typography = &theme.typography;
        let owns = |path: &str| config.explicit_config_owns_path(path);
        let scoped = |key: &str| {
            owns(&format!("radar.{key}")) || owns(&format!("themeVariables.radar.{key}"))
        };
        let owns_fill = |key: &str| owns(&format!("themeVariables.{key}"));
        if !owns_fill("textColor") {
            binding.text_color = roles.text_primary.to_string();
        }
        if !owns_fill("titleColor") {
            binding.title_color = roles.text_title.to_string();
        }
        if !scoped("axisColor") {
            binding.axis_color = roles.stroke_border.first_color().to_string();
        }
        binding.native_axis_label_color = Some(if scoped("axisColor") {
            binding.axis_color.clone()
        } else if owns_fill("textColor") {
            binding.text_color.clone()
        } else {
            roles.text_muted.to_string()
        });
        if !scoped("graticuleColor") {
            binding.graticule_color = roles.stroke_guide.first_color().to_string();
        }
        if !scoped("graticuleOpacity") {
            binding.graticule_opacity = 0.0;
        }
        if !scoped("curveOpacity") {
            binding.curve_opacity = 0.28;
        }
        if !scoped("axisStrokeWidth") {
            binding.axis_stroke_width = theme.form.stroke_width;
        }
        if !scoped("curveStrokeWidth") {
            binding.curve_stroke_width = theme.form.connector_width;
        }
        let role_size = resolve_explicit_css_absolute_font_size_px(font_size_css)
            .unwrap_or(typography.base_size_px);
        if !scoped("axisLabelFontSize") {
            binding.axis_label_font_size = RadarRoleFontSize::resolve(role_size);
        }
        if !scoped("legendFontSize") {
            binding.legend_font_size = RadarRoleFontSize::resolve(role_size);
        }
        let theme_font = !owns("fontFamily") && !owns("themeVariables.fontFamily");
        Self {
            body_family: theme_font
                .then(|| font_family_css(typography.stack(typography.body_family))),
            title_family: theme_font
                .then(|| font_family_css(typography.stack(typography.title_family))),
            title_weight: typography.title_weight,
            label_weight: typography.label_weight,
            show_grid: theme.form.chart.gridlines
                || scoped("graticuleColor")
                || scoped("graticuleOpacity")
                || scoped("graticuleStrokeWidth"),
        }
    }
}
