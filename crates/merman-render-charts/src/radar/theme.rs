use merman_model::families::radar::RadarDiagramModel;
use merman_model::presentation::PresentationConfig;
use merman_render_core::config::{
    config_css_number_or_string, config_f64_css_px, config_font_family_css,
};
use merman_theme_contract::v2::FamilyTheme;

use super::{css_binding::RadarCssBinding, native_theme::RadarNativeStyle};

#[cfg(test)]
mod tests;

const RADAR_COLOR_SCALE_KEYS: [&str; 12] = [
    "cScale0", "cScale1", "cScale2", "cScale3", "cScale4", "cScale5", "cScale6", "cScale7",
    "cScale8", "cScale9", "cScale10", "cScale11",
];

/// The resolved presentation of a Radar chart.
#[derive(Debug)]
pub struct RadarStyle {
    pub(super) typography: RadarTypographyThemePlan,
    pub(super) series_paint: RadarSeriesPaintPlan,
}

impl RadarStyle {
    pub(super) fn resolve(
        theme: Option<&FamilyTheme>,
        config: &PresentationConfig,
        model: &RadarDiagramModel,
        fallback_c_scale: &[&str; 12],
    ) -> Self {
        let typography = RadarTypographyThemePlan::resolve(theme, config);
        let series_paint = RadarSeriesPaintPlan::resolve(theme, config, model, fallback_c_scale);
        Self {
            typography,
            series_paint,
        }
    }
}

#[derive(Debug)]
pub(super) struct RadarTypographyThemePlan {
    css_binding: RadarCssBinding,
    native: Option<RadarNativeStyle>,
    font_family_css: String,
    font_size_css: String,
}

impl RadarTypographyThemePlan {
    pub(super) fn resolve(theme: Option<&FamilyTheme>, config: &PresentationConfig) -> Self {
        let configured_size =
            config_css_number_or_string(config.as_value(), &["themeVariables", "fontSize"]);
        let font_size_css = match theme {
            Some(theme) if !config.explicit_config_owns_path("themeVariables.fontSize") => {
                let size = if config.explicit_config_owns_path("fontSize") {
                    config_f64_css_px(config.as_value(), &["fontSize"])
                        .unwrap_or(theme.typography.base_size_px)
                } else {
                    theme.typography.base_size_px
                };
                format!("{size}px")
            }
            _ => RadarCssBinding::font_size_css_or_default(configured_size),
        };
        let mut css_binding = RadarCssBinding::resolve_styles(config.as_value());
        let native = theme
            .map(|theme| RadarNativeStyle::bind(theme, config, &font_size_css, &mut css_binding));
        let font_family_css = native
            .as_ref()
            .and_then(|native| native.body_family.as_deref())
            .map_or_else(|| config_font_family_css(config), str::to_owned);
        Self {
            css_binding,
            native,
            font_family_css,
            font_size_css,
        }
    }

    pub(super) fn native_text_style(&self) -> Option<(&str, u16, u16)> {
        let native = self.native.as_ref()?;
        Some((
            native
                .title_family
                .as_deref()
                .unwrap_or(self.font_family_css()),
            native.title_weight,
            native.label_weight,
        ))
    }

    pub(super) fn show_grid(&self) -> bool {
        self.native.as_ref().is_none_or(|native| native.show_grid)
    }

    pub(super) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub(super) fn css_binding(&self) -> &RadarCssBinding {
        &self.css_binding
    }

    pub(super) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }
}

#[derive(Debug)]
pub(super) struct RadarSeriesPaintPlan {
    colors: [String; RADAR_COLOR_SCALE_KEYS.len()],
    native_palette: Option<RadarNativePalette>,
}

#[derive(Debug)]
struct RadarNativePalette {
    ink: Box<[String]>,
    fill: Box<[String]>,
}

impl RadarSeriesPaintPlan {
    fn resolve(
        theme: Option<&FamilyTheme>,
        config: &PresentationConfig,
        model: &RadarDiagramModel,
        fallback_c_scale: &[&str; 12],
    ) -> Self {
        let mut plan = Self::baseline(config.as_value(), fallback_c_scale);
        if let Some(theme) = theme {
            let (ink, fill): (Vec<_>, Vec<_>) = (0..model.curves.len())
                .map(|index| {
                    if RADAR_COLOR_SCALE_KEYS.get(index).is_some_and(|key| {
                        config.explicit_config_owns_path(&format!("themeVariables.{key}"))
                    }) {
                        let color = plan.colors[index].clone();
                        (color.clone(), color)
                    } else {
                        (
                            theme.data.ink(index).to_string(),
                            theme.data.fill(index).to_string(),
                        )
                    }
                })
                .unzip();
            plan.native_palette = Some(RadarNativePalette {
                ink: ink.into_boxed_slice(),
                fill: fill.into_boxed_slice(),
            });
        }
        plan
    }

    pub(super) fn baseline(config: &serde_json::Value, fallback_c_scale: &[&str; 12]) -> Self {
        Self {
            colors: std::array::from_fn(|index| {
                RadarCssBinding::configured_color(config, RADAR_COLOR_SCALE_KEYS[index])
                    .unwrap_or(fallback_c_scale[index])
                    .to_owned()
            }),
            native_palette: None,
        }
    }

    pub(super) fn colors(&self) -> &[String] {
        self.native_palette
            .as_ref()
            .map_or(self.colors.as_slice(), |palette| palette.ink.as_ref())
    }

    pub(super) fn fill_css(&self, index: usize) -> &str {
        self.native_palette
            .as_ref()
            .map_or(self.colors.as_slice(), |palette| palette.fill.as_ref())[index]
            .as_str()
    }
}
