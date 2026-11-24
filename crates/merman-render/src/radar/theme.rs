use merman_model::families::radar::RadarDiagramModel;
use merman_model::presentation::PresentationConfig;

use crate::config::config_css_number_or_string;
use crate::theme::FamilyTheme;

const RADAR_COLOR_SCALE_KEYS: [&str; 12] = [
    "cScale0", "cScale1", "cScale2", "cScale3", "cScale4", "cScale5", "cScale6", "cScale7",
    "cScale8", "cScale9", "cScale10", "cScale11",
];

#[derive(Debug)]
pub(crate) struct RadarTypographyThemePlan {
    css_binding: super::RadarCssBinding,
    native: Option<super::native_theme::RadarNativeStyle>,
    font_family_css: String,
    font_size_css: String,
}

impl RadarTypographyThemePlan {
    pub(crate) fn resolve(theme: Option<&FamilyTheme>, config: &PresentationConfig) -> Self {
        let configured_size =
            config_css_number_or_string(config.as_value(), &["themeVariables", "fontSize"]);
        let font_size_css = match theme {
            Some(theme) if !config.explicit_config_owns_path("themeVariables.fontSize") => {
                let size = if config.explicit_config_owns_path("fontSize") {
                    crate::config::config_f64_css_px(config.as_value(), &["fontSize"])
                        .unwrap_or(theme.typography.base_size_px)
                } else {
                    theme.typography.base_size_px
                };
                format!("{size}px")
            }
            _ => super::RadarCssBinding::font_size_css_or_default(configured_size),
        };
        let mut css_binding = super::RadarCssBinding::resolve_styles(config.as_value());
        let native = theme.map(|theme| {
            super::native_theme::RadarNativeStyle::bind(
                theme,
                config,
                &font_size_css,
                &mut css_binding,
            )
        });
        let font_family_css = native
            .as_ref()
            .and_then(|native| native.body_family.as_deref())
            .map_or_else(
                || crate::config::config_font_family_css(config),
                str::to_owned,
            );
        Self {
            css_binding,
            native,
            font_family_css,
            font_size_css,
        }
    }

    pub(crate) fn native_text_style(&self) -> Option<(&str, u16, u16)> {
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

    pub(crate) fn show_grid(&self) -> bool {
        self.native.as_ref().is_none_or(|native| native.show_grid)
    }

    pub(crate) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub(crate) fn css_binding(&self) -> &super::RadarCssBinding {
        &self.css_binding
    }

    pub(crate) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }
}

#[derive(Debug)]
pub(crate) struct RadarSeriesPaintPlan {
    colors: [String; RADAR_COLOR_SCALE_KEYS.len()],
    native_palette: Option<RadarNativePalette>,
}

#[derive(Debug)]
struct RadarNativePalette {
    ink: Box<[String]>,
    fill: Box<[String]>,
}

impl RadarSeriesPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&FamilyTheme>,
        config: &PresentationConfig,
        model: &RadarDiagramModel,
    ) -> Self {
        let mut plan = Self::baseline(config.as_value(), model);
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

    pub(crate) fn baseline(config: &serde_json::Value, _model: &RadarDiagramModel) -> Self {
        Self {
            colors: std::array::from_fn(|index| {
                super::RadarCssBinding::configured_color(config, RADAR_COLOR_SCALE_KEYS[index])
                    .map(str::to_owned)
                    .unwrap_or_else(|| {
                        crate::svg::render_theme::radar_default_series_color(index).to_string()
                    })
            }),
            native_palette: None,
        }
    }

    pub(crate) fn colors(&self) -> &[String] {
        self.native_palette
            .as_ref()
            .map_or(self.colors.as_slice(), |palette| palette.ink.as_ref())
    }

    pub(crate) fn fill_css(&self, index: usize) -> &str {
        self.native_palette
            .as_ref()
            .map_or(self.colors.as_slice(), |palette| palette.fill.as_ref())[index]
            .as_str()
    }
}
