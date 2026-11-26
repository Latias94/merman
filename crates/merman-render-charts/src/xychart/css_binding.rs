use merman_render_core::config::ConfigLookup;
use serde_json::Value;

#[derive(Debug)]
pub(super) struct XyChartCssBinding {
    pub(super) background_color: String,
    pub(super) title_color: String,
    pub(super) legend_text_color: String,
    pub(super) data_label_color: String,
    pub(super) x_axis_title_color: String,
    pub(super) x_axis_label_color: String,
    pub(super) x_axis_tick_color: String,
    pub(super) x_axis_line_color: String,
    pub(super) y_axis_title_color: String,
    pub(super) y_axis_label_color: String,
    pub(super) y_axis_tick_color: String,
    pub(super) y_axis_line_color: String,
}

impl XyChartCssBinding {
    pub(super) fn native(roles: &merman_theme_contract::v2::Roles) -> Self {
        let title = roles.text_title.to_string();
        let label = roles.text_muted.to_string();
        let tick = roles.stroke_guide.first_color().to_string();
        let axis = roles.stroke_connector.first_color().to_string();
        Self {
            background_color: "none".to_owned(),
            title_color: title.clone(),
            legend_text_color: roles.text_primary.to_string(),
            data_label_color: roles.text_primary.to_string(),
            x_axis_title_color: title.clone(),
            x_axis_label_color: label.clone(),
            x_axis_tick_color: tick.clone(),
            x_axis_line_color: axis.clone(),
            y_axis_title_color: title,
            y_axis_label_color: label,
            y_axis_tick_color: tick,
            y_axis_line_color: axis,
        }
    }

    fn color<'a>(config: &'a impl ConfigLookup, channel: &str) -> &'a str {
        let variables = config.get("themeVariables").unwrap_or(&Value::Null);
        if let Some(color) = variables["xyChart"][channel].as_str() {
            return color;
        }
        let (root, fallback) = match channel {
            "backgroundColor" => ("background", "white"),
            "dataLabelColor" => ("primaryTextColor", "black"),
            _ => ("primaryTextColor", "#131300"),
        };
        variables[root].as_str().unwrap_or(fallback)
    }

    pub(super) fn resolve(config: &impl ConfigLookup) -> Self {
        let role = |key| Self::color(config, key).to_owned();
        Self {
            background_color: role("backgroundColor"),
            title_color: role("titleColor"),
            legend_text_color: role("legendTextColor"),
            data_label_color: role("dataLabelColor"),
            x_axis_title_color: role("xAxisTitleColor"),
            x_axis_label_color: role("xAxisLabelColor"),
            x_axis_tick_color: role("xAxisTickColor"),
            x_axis_line_color: role("xAxisLineColor"),
            y_axis_title_color: role("yAxisTitleColor"),
            y_axis_label_color: role("yAxisLabelColor"),
            y_axis_tick_color: role("yAxisTickColor"),
            y_axis_line_color: role("yAxisLineColor"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_roles_keep_nested_mermaid_channels() {
        let binding = XyChartCssBinding::resolve(&serde_json::json!({
            "theme": "neo", "themeVariables": {
                "primaryTextColor": "#f8fafc", "background": "#010203",
                "xyChart": { "backgroundColor": "#0f172a", "titleColor": "#f43f5e",
                    "xAxisLabelColor": "#22c55e" }
            }
        }));
        assert_eq!(binding.background_color, "#0f172a");
        assert_eq!(binding.title_color, "#f43f5e");
        assert_eq!(binding.legend_text_color, "#f8fafc");
        assert_eq!(binding.x_axis_title_color, "#f8fafc");
        assert_eq!(binding.x_axis_label_color, "#22c55e");
        assert_eq!(binding.y_axis_line_color, "#f8fafc");
    }

    #[test]
    fn nested_channels_preserve_raw_css_and_only_fallback_on_absence() {
        let binding = XyChartCssBinding::resolve(&serde_json::json!({
            "themeVariables": { "primaryTextColor": "var(--text)",
                "background": "currentColor", "xyChart": {
                    "titleColor": "", "xAxisTickColor": "var(--tick)",
                    "yAxisLabelColor": 7
                }
            }
        }));
        assert_eq!(binding.background_color, "currentColor");
        assert_eq!(binding.title_color, "");
        assert_eq!(binding.x_axis_tick_color, "var(--tick)");
        assert_eq!(binding.y_axis_label_color, "var(--text)");
    }
}
