use crate::config::{config_css_number_or_string, config_string};
use crate::svg::PreparedCommonCss;
use crate::text::TextStyle;

/// A prepared stylesheet token retains Mermaid's authored spelling, including browser-only CSS.
#[derive(Debug)]
pub(crate) struct PieCssBinding {
    pub(crate) common: PreparedCommonCss,
    pub(crate) slice_stroke: PieSurfaceStroke,
    pub(crate) slice_stroke_width: String,
    pub(crate) slice_opacity: String,
    pub(crate) outer_stroke: PieSurfaceStroke,
    pub(crate) outer_stroke_width: String,
    pub(crate) title_text_size: String,
    pub(crate) title_text_color: String,
    pub(crate) section_text_size: String,
    pub(crate) section_text_color: String,
    pub(crate) legend_text_size: String,
    pub(crate) legend_text_color: String,
}

/// An absent surface keeps its baseline CSS without admitting a typed terminal application.
#[derive(Debug)]
pub(crate) struct PieSurfaceStroke {
    visible: String,
    absent: Option<String>,
}

impl PieSurfaceStroke {
    fn baseline(css: String) -> Self {
        Self {
            visible: css,
            absent: None,
        }
    }

    pub(super) fn bind_typed(&mut self, css: &str) {
        self.absent = Some(std::mem::replace(&mut self.visible, css.to_owned()));
    }

    pub(crate) fn css(&self, surface_present: bool) -> &str {
        if surface_present {
            &self.visible
        } else {
            self.absent.as_deref().unwrap_or(&self.visible)
        }
    }
}

impl PieCssBinding {
    pub(super) fn new(
        effective_config: &serde_json::Value,
        font_family_override: Option<&str>,
    ) -> Self {
        let common = PreparedCommonCss::new(effective_config, font_family_override);
        let color = |key: &str, fallback: &str| {
            config_string(effective_config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_owned())
        };
        let value = |key: &str, fallback: &str| {
            config_css_number_or_string(effective_config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_owned())
        };
        let task_text_dark = color("taskTextDarkColor", "black");
        let title_text_color = color("pieTitleTextColor", &task_text_dark);
        let section_text_color = color("pieSectionTextColor", common.text_color());
        let legend_text_color = color("pieLegendTextColor", &task_text_dark);
        Self {
            common,
            slice_stroke: PieSurfaceStroke::baseline(value("pieStrokeColor", "black")),
            slice_stroke_width: value("pieStrokeWidth", "2px"),
            slice_opacity: value("pieOpacity", "0.7"),
            outer_stroke: PieSurfaceStroke::baseline(value("pieOuterStrokeColor", "black")),
            outer_stroke_width: value("pieOuterStrokeWidth", "2px"),
            title_text_size: value("pieTitleTextSize", "25px"),
            title_text_color,
            section_text_size: value("pieSectionTextSize", "17px"),
            section_text_color,
            legend_text_size: value("pieLegendTextSize", "17px"),
            legend_text_color,
        }
    }

    pub(crate) fn legend_measurement_style(&self) -> TextStyle {
        self.measurement_style(17.0)
    }

    pub(crate) fn title_measurement_style(&self) -> TextStyle {
        self.measurement_style(25.0)
    }

    fn measurement_style(&self, font_size: f64) -> TextStyle {
        // Pie's pinned measurement model uses fixed metrics; emitted CSS may remain opaque.
        TextStyle {
            font_family: Some(self.common.font_family().to_owned()),
            font_size,
            font_weight: None,
            font_style: None,
        }
    }
}
