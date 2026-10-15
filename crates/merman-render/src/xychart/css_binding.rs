use serde_json::Value;

#[derive(Debug)]
pub(super) struct XyChartCssBinding {
    pub(super) background_color: String,
    pub(super) title_color: String,
    pub(super) legend_text_color: String,
    pub(super) x_axis_title_color: String,
    pub(super) x_axis_label_color: String,
    pub(super) x_axis_tick_color: String,
    pub(super) x_axis_line_color: String,
    pub(super) y_axis_title_color: String,
    pub(super) y_axis_label_color: String,
    pub(super) y_axis_tick_color: String,
    pub(super) y_axis_line_color: String,
    pub(super) plot_color_palette: Vec<String>,
}

impl XyChartCssBinding {
    pub(super) fn resolve(config: &Value) -> Self {
        let variables = config.get("themeVariables");
        let raw = |key: &str| {
            variables
                .and_then(|variables| variables.get(key))
                .and_then(Value::as_str)
        };
        let nested = |key: &str| {
            variables
                .and_then(|variables| variables.get("xyChart"))
                .and_then(|variables| variables.get(key))
                .and_then(Value::as_str)
        };
        let primary_text = raw("primaryTextColor").unwrap_or("#131300");
        let role = |key| nested(key).unwrap_or(primary_text).to_owned();
        Self {
            background_color: nested("backgroundColor")
                .or_else(|| raw("background"))
                .unwrap_or("white")
                .to_owned(),
            title_color: role("titleColor"),
            legend_text_color: role("legendTextColor"),
            x_axis_title_color: role("xAxisTitleColor"),
            x_axis_label_color: role("xAxisLabelColor"),
            x_axis_tick_color: role("xAxisTickColor"),
            x_axis_line_color: role("xAxisLineColor"),
            y_axis_title_color: role("yAxisTitleColor"),
            y_axis_label_color: role("yAxisLabelColor"),
            y_axis_tick_color: role("yAxisTickColor"),
            y_axis_line_color: role("yAxisLineColor"),
            plot_color_palette: crate::chart_palette::resolve_xychart_plot_palette(
                config
                    .get("theme")
                    .and_then(Value::as_str)
                    .unwrap_or("default"),
                nested("plotColorPalette"),
            ),
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
                    "xAxisLabelColor": "#22c55e", "plotColorPalette": "#001122, #334455" }
            }
        }));
        assert_eq!(binding.background_color, "#0f172a");
        assert_eq!(binding.title_color, "#f43f5e");
        assert_eq!(binding.legend_text_color, "#f8fafc");
        assert_eq!(binding.x_axis_title_color, "#f8fafc");
        assert_eq!(binding.x_axis_label_color, "#22c55e");
        assert_eq!(binding.y_axis_line_color, "#f8fafc");
        assert_eq!(binding.plot_color_palette, ["#001122", "#334455"]);
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

    #[test]
    fn missing_and_empty_palette_keep_named_theme_fallback() {
        let missing = XyChartCssBinding::resolve(&serde_json::json!({ "theme": "dark" }));
        let empty = XyChartCssBinding::resolve(&serde_json::json!({
            "theme": "dark", "themeVariables": { "xyChart": { "plotColorPalette": " , " } }
        }));
        assert_eq!(missing.plot_color_palette, empty.plot_color_palette);
        assert_eq!(missing.plot_color_palette[0], "#3498db");
        let explicit = XyChartCssBinding::resolve(&serde_json::json!({
            "theme": "dark", "themeVariables": { "xyChart": { "plotColorPalette": "var(--series), currentColor" } }
        }));
        assert_eq!(
            explicit.plot_color_palette,
            ["var(--series)", "currentColor"]
        );
    }
}
