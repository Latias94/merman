use serde_json::Value;

#[derive(Debug)]
pub(crate) struct QuadrantChartCssBinding {
    pub(crate) quadrant1_fill: String,
    pub(crate) quadrant2_fill: String,
    pub(crate) quadrant3_fill: String,
    pub(crate) quadrant4_fill: String,
    pub(crate) quadrant1_text_fill: String,
    pub(crate) quadrant2_text_fill: String,
    pub(crate) quadrant3_text_fill: String,
    pub(crate) quadrant4_text_fill: String,
    pub(crate) quadrant_point_fill: String,
    pub(crate) quadrant_point_text_fill: String,
    pub(crate) quadrant_x_axis_text_fill: String,
    pub(crate) quadrant_y_axis_text_fill: String,
    pub(crate) quadrant_title_fill: String,
    pub(crate) quadrant_internal_border_stroke_fill: String,
    pub(crate) quadrant_external_border_stroke_fill: String,
}

impl QuadrantChartCssBinding {
    pub(crate) fn resolve(config: &Value) -> Self {
        let value = |key: &str, fallback: &str| {
            config
                .get("themeVariables")
                .and_then(|variables| variables.get(key))
                .and_then(Value::as_str)
                .unwrap_or(fallback)
                .to_owned()
        };
        let primary_text = value("primaryTextColor", "#131300");
        Self {
            quadrant1_fill: value("quadrant1Fill", "#ECECFF"),
            quadrant2_fill: value("quadrant2Fill", "#f1f1ff"),
            quadrant3_fill: value("quadrant3Fill", "#f6f6ff"),
            quadrant4_fill: value("quadrant4Fill", "#fbfbff"),
            quadrant1_text_fill: value("quadrant1TextFill", &primary_text),
            quadrant2_text_fill: value("quadrant2TextFill", "#0e0e00"),
            quadrant3_text_fill: value("quadrant3TextFill", "#090900"),
            quadrant4_text_fill: value("quadrant4TextFill", "#040400"),
            quadrant_point_fill: value("quadrantPointFill", "hsl(240, 100%, NaN%)"),
            quadrant_point_text_fill: value("quadrantPointTextFill", &primary_text),
            quadrant_x_axis_text_fill: value("quadrantXAxisTextFill", &primary_text),
            quadrant_y_axis_text_fill: value("quadrantYAxisTextFill", &primary_text),
            quadrant_title_fill: value("quadrantTitleFill", &primary_text),
            quadrant_internal_border_stroke_fill: value(
                "quadrantInternalBorderStrokeFill",
                "hsl(240, 60%, 86.2745098039%)",
            ),
            quadrant_external_border_stroke_fill: value(
                "quadrantExternalBorderStrokeFill",
                "hsl(240, 60%, 86.2745098039%)",
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chart_roles_keep_their_mermaid_channels() {
        let cfg = serde_json::json!({
            "theme": "redux-dark",
            "themeVariables": {
                "primaryColor": "#123456",
                "primaryTextColor": "#f8fafc",
                "primaryBorderColor": "#445566",
                "quadrant1Fill": "#010203",
                "quadrant2Fill": "#020304",
                "quadrant3Fill": "#030405",
                "quadrant4Fill": "#040506",
                "quadrantPointFill": "#facc15",
                "quadrantPointTextFill": "#111827",
                "quadrantXAxisTextFill": "#22c55e",
                "quadrantYAxisTextFill": "#38bdf8",
                "quadrantTitleFill": "#f43f5e",
                "quadrantInternalBorderStrokeFill": "#aabbcc",
                "quadrantExternalBorderStrokeFill": "#ddeeff"
            }
        });
        let quadrant = QuadrantChartCssBinding::resolve(&cfg);
        assert_eq!(quadrant.quadrant1_fill, "#010203");
        assert_eq!(quadrant.quadrant2_fill, "#020304");
        assert_eq!(quadrant.quadrant1_text_fill, "#f8fafc");
        assert_eq!(quadrant.quadrant_point_fill, "#facc15");
        assert_eq!(quadrant.quadrant_point_text_fill, "#111827");
        assert_eq!(quadrant.quadrant_x_axis_text_fill, "#22c55e");
        assert_eq!(quadrant.quadrant_y_axis_text_fill, "#38bdf8");
        assert_eq!(quadrant.quadrant_title_fill, "#f43f5e");
        assert_eq!(quadrant.quadrant_internal_border_stroke_fill, "#aabbcc");
        assert_eq!(quadrant.quadrant_external_border_stroke_fill, "#ddeeff");
    }

    #[test]
    fn raw_css_and_default_sentinel_are_preserved() {
        let binding = QuadrantChartCssBinding::resolve(&serde_json::json!({
            "themeVariables": {
                "primaryTextColor": "var(--caption)",
                "quadrant1Fill": "currentColor",
                "quadrant2Fill": 4
            }
        }));
        assert_eq!(binding.quadrant1_fill, "currentColor");
        assert_eq!(binding.quadrant2_fill, "#f1f1ff");
        assert_eq!(binding.quadrant1_text_fill, "var(--caption)");
        assert_eq!(binding.quadrant_point_fill, "hsl(240, 100%, NaN%)");
    }
}
