use crate::config::{config_css_number_or_string, config_string};
use crate::svg::PreparedCommonCss;
use merman_core::theme_color::{ColorChannel, ThemeColor, invert};

#[derive(Debug)]
pub(crate) struct TreemapCssBinding {
    pub(crate) common: PreparedCommonCss,
    pub(crate) title_color: String,
    pub(crate) label_color: String,
    pub(crate) value_color: String,
    pub(crate) section_stroke_color: String,
    pub(crate) section_stroke_width: String,
    pub(crate) section_fill_color: String,
    pub(crate) leaf_stroke_color: String,
    pub(crate) leaf_stroke_width: String,
    pub(crate) leaf_fill_color: String,
    pub(crate) label_font_size: String,
    pub(crate) value_font_size: String,
    pub(crate) title_font_size: String,
    pub(crate) color_scale: Vec<String>,
    pub(crate) color_scale_peer: Vec<String>,
    pub(crate) color_scale_label: Vec<String>,
    text_color: String,
}

impl TreemapCssBinding {
    pub(super) fn new(config: &serde_json::Value, font_family: &str) -> crate::Result<Self> {
        let optional_color = |key: &str| config_string(config, &["themeVariables", key]);
        let color =
            |key: &str, fallback: &str| optional_color(key).unwrap_or_else(|| fallback.to_owned());
        let style_option = |key: &str, fallback: &str| {
            config_css_number_or_string(config, &["treemap", key])
                .unwrap_or_else(|| fallback.to_owned())
        };

        let text_color = color("textColor", "#333");
        let title_color = config_string(config, &["treemap", "titleColor"])
            .or_else(|| optional_color("titleColor"))
            .unwrap_or_else(|| text_color.clone());
        let raw_theme_name =
            config_string(config, &["theme"]).unwrap_or_else(|| "default".to_owned());
        let default_theme = raw_theme_name == "default";
        let theme_name = raw_theme_name.trim().to_ascii_lowercase();
        let label_text_color = color("labelTextColor", "black");
        let label_text_is_calculated = label_text_color.trim() == "calculated";
        let scale_label_color = color("scaleLabelColor", &label_text_color);
        let neutral_special_label_color = color("cScale1", default_c_scale(1));

        let color_scale = (0..12)
            .map(|i| {
                if default_theme {
                    default_c_scale(i).to_string()
                } else {
                    color(&format!("cScale{i}"), default_c_scale(i))
                }
            })
            .collect();
        let color_scale_peer = (0..12)
            .map(|i| {
                if default_theme {
                    default_c_scale_peer(i).to_string()
                } else {
                    color(&format!("cScalePeer{i}"), default_c_scale_peer(i))
                }
            })
            .collect();
        let color_scale_label = (0..12)
            .map(|i| -> crate::Result<String> {
                if let Some(color) = optional_color(&format!("cScaleLabel{i}")) {
                    return Ok(color);
                }
                Ok(match theme_name.as_str() {
                    "dark" | "forest" => scale_label_color.clone(),
                    "neutral" => {
                        if i == 0 || i == 2 {
                            neutral_special_label_color.clone()
                        } else {
                            scale_label_color.clone()
                        }
                    }
                    _ => {
                        if label_text_is_calculated {
                            scale_label_color.clone()
                        } else if i == 0 || i == 3 {
                            invert(&label_text_color)?
                        } else {
                            label_text_color.clone()
                        }
                    }
                })
            })
            .collect::<crate::Result<Vec<_>>>()?;

        Ok(Self {
            common: PreparedCommonCss::new(config, Some(font_family)),
            title_color,
            label_color: config_string(config, &["treemap", "labelColor"])
                .unwrap_or_else(|| text_color.clone()),
            value_color: config_string(config, &["treemap", "valueColor"])
                .unwrap_or_else(|| text_color.clone()),
            section_stroke_color: style_option("sectionStrokeColor", "black"),
            section_stroke_width: style_option("sectionStrokeWidth", "1"),
            section_fill_color: style_option("sectionFillColor", "#efefef"),
            leaf_stroke_color: style_option("leafStrokeColor", "black"),
            leaf_stroke_width: style_option("leafStrokeWidth", "1"),
            leaf_fill_color: style_option("leafFillColor", "#efefef"),
            label_font_size: style_option("labelFontSize", "12px"),
            value_font_size: style_option("valueFontSize", "10px"),
            title_font_size: style_option("titleFontSize", "14px"),
            color_scale,
            color_scale_peer,
            color_scale_label,
            text_color,
        })
    }

    pub(crate) fn readable_leaf_label_fill(
        &self,
        leaf_fill: &str,
        leaf_rect_style: &str,
        leaf_label_fill: String,
    ) -> String {
        if is_transparent(leaf_fill)
            && !style_has_non_empty_decl(leaf_rect_style, "fill")
            && is_white_like(&leaf_label_fill)
        {
            self.text_color.clone()
        } else {
            leaf_label_fill
        }
    }
}

fn default_c_scale(index: usize) -> &'static str {
    match index {
        0 => "hsl(240, 100%, 76.2745098039%)",
        1 => "hsl(60, 100%, 73.5294117647%)",
        2 => "hsl(80, 100%, 76.2745098039%)",
        3 => "hsl(270, 100%, 76.2745098039%)",
        4 => "hsl(300, 100%, 76.2745098039%)",
        5 => "hsl(330, 100%, 76.2745098039%)",
        6 => "hsl(0, 100%, 76.2745098039%)",
        7 => "hsl(30, 100%, 76.2745098039%)",
        8 => "hsl(90, 100%, 76.2745098039%)",
        9 => "hsl(150, 100%, 76.2745098039%)",
        10 => "hsl(180, 100%, 76.2745098039%)",
        _ => "hsl(210, 100%, 76.2745098039%)",
    }
}

fn default_c_scale_peer(index: usize) -> &'static str {
    match index {
        0 => "hsl(240, 100%, 61.2745098039%)",
        1 => "hsl(60, 100%, 48.5294117647%)",
        2 => "hsl(80, 100%, 56.2745098039%)",
        3 => "hsl(270, 100%, 61.2745098039%)",
        4 => "hsl(300, 100%, 61.2745098039%)",
        5 => "hsl(330, 100%, 61.2745098039%)",
        6 => "hsl(0, 100%, 61.2745098039%)",
        7 => "hsl(30, 100%, 61.2745098039%)",
        8 => "hsl(90, 100%, 61.2745098039%)",
        9 => "hsl(150, 100%, 61.2745098039%)",
        10 => "hsl(180, 100%, 61.2745098039%)",
        _ => "hsl(210, 100%, 61.2745098039%)",
    }
}

fn is_transparent(color: &str) -> bool {
    ThemeColor::parse(color.trim()).is_ok_and(|color| color.channel(ColorChannel::Alpha) == 0.0)
}

fn is_white_like(color: &str) -> bool {
    ThemeColor::parse(color.trim()).is_ok_and(|color| {
        color.channel(ColorChannel::Red) >= 250.0
            && color.channel(ColorChannel::Green) >= 250.0
            && color.channel(ColorChannel::Blue) >= 250.0
    })
}

fn style_has_non_empty_decl(style: &str, property: &str) -> bool {
    style.split(';').any(|declaration| {
        let Some((key, value)) = declaration.split_once(':') else {
            return false;
        };
        key.trim().eq_ignore_ascii_case(property) && !value.trim().is_empty()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prepared_roles_keep_root_options_and_raw_css_spelling() {
        let binding = TreemapCssBinding::new(
            &json!({
                "theme": "custom",
                "themeVariables": {
                    "textColor": "#101010", "titleColor": "#202020",
                    "labelTextColor": "rgb(10, 20, 30)",
                    "cScale0": "#010203", "cScalePeer0": "#040506", "cScaleLabel0": "#070809",
                    "treemap": {"titleColor": "ignored", "sectionFillColor": "ignored"}
                },
                "treemap": {
                    "titleColor": "#777777", "labelColor": "#555555", "valueColor": "#666666",
                    "sectionStrokeColor": "#111111", "sectionStrokeWidth": 2,
                    "sectionFillColor": "#222222", "leafStrokeColor": "#333333",
                    "leafStrokeWidth": "3px", "leafFillColor": "#444444",
                    "labelFontSize": "13px", "valueFontSize": "11px", "titleFontSize": "15px"
                }
            }),
            "Inter",
        )
        .expect("prepared Treemap roles");
        assert_eq!(binding.title_color, "#777777");
        assert_eq!(binding.label_color, "#555555");
        assert_eq!(binding.value_color, "#666666");
        assert_eq!(binding.section_stroke_color, "#111111");
        assert_eq!(binding.section_stroke_width, "2");
        assert_eq!(binding.section_fill_color, "#222222");
        assert_eq!(binding.leaf_stroke_color, "#333333");
        assert_eq!(binding.leaf_stroke_width, "3px");
        assert_eq!(binding.leaf_fill_color, "#444444");
        assert_eq!(binding.label_font_size, "13px");
        assert_eq!(binding.value_font_size, "11px");
        assert_eq!(binding.title_font_size, "15px");
        assert_eq!(binding.color_scale[0], "#010203");
        assert_eq!(binding.color_scale_peer[0], "#040506");
        assert_eq!(binding.color_scale_label[0], "#070809");
    }

    #[test]
    fn prepared_default_scales_keep_inversion_and_transparent_readability() {
        let binding = TreemapCssBinding::new(&json!({
            "themeVariables": {"labelTextColor": "rgb(10, 20, 30)", "cScale0": "ignored", "cScalePeer0": "ignored"}
        }), "Inter").expect("prepared default scales");
        assert_eq!(binding.color_scale[0], "hsl(240, 100%, 76.2745098039%)");
        assert_eq!(
            binding.color_scale_peer[0],
            "hsl(240, 100%, 61.2745098039%)"
        );
        assert_eq!(binding.color_scale_label[0], "#f5ebe1");
        assert_eq!(binding.color_scale_label[1], "rgb(10, 20, 30)");
        assert_eq!(binding.color_scale_label[3], "#f5ebe1");
        assert_eq!(
            binding.readable_leaf_label_fill("transparent", "", "#ffffff".into()),
            "#333"
        );
        assert_eq!(
            binding.readable_leaf_label_fill(
                "rgba(10 20 30 / 0)",
                "fill:var(--fill)",
                "white".into()
            ),
            "white"
        );
        assert_eq!(
            binding.readable_leaf_label_fill("transparent", "", "var(--label)".into()),
            "var(--label)"
        );
        assert_eq!(
            binding.readable_leaf_label_fill("rgba(10 20 30 / 0)", "", "rgb(250 251 252)".into()),
            "#333"
        );
    }

    #[test]
    fn calculated_labels_and_theme_specific_fallbacks_do_not_parse_sentinel() {
        let calculated = TreemapCssBinding::new(&json!({
            "theme": "base", "themeVariables": {"labelTextColor": "calculated", "scaleLabelColor": "var(--label)"}
        }), "Inter").expect("calculated label sentinel");
        assert!(
            calculated
                .color_scale_label
                .iter()
                .all(|color| color == "var(--label)")
        );
        let neutral = TreemapCssBinding::new(&json!({
            "theme": "neutral", "themeVariables": {"cScale1": "#112233", "scaleLabelColor": "#445566"}
        }), "Inter").expect("neutral label roles");
        assert_eq!(neutral.color_scale_label[0], "#112233");
        assert_eq!(neutral.color_scale_label[2], "#112233");
        assert_eq!(neutral.color_scale_label[1], "#445566");
        let fallback = TreemapCssBinding::new(
            &json!({"themeVariables": {"textColor": "#101010", "titleColor": "#202020"}}),
            "Inter",
        )
        .unwrap();
        assert_eq!(fallback.title_color, "#202020");
        assert_eq!(fallback.label_color, "#101010");
        assert_eq!(fallback.value_color, "#101010");
        assert!(matches!(
            TreemapCssBinding::new(
                &json!({"themeVariables":{"labelTextColor":"not-a-color"}}),
                "Inter"
            ),
            Err(crate::Error::Color(_))
        ));
    }
}
