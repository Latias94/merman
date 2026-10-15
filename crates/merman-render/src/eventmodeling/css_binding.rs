/// Operation-owned Event Modeling CSS tokens; raw browser paints retain their spelling.
#[derive(Debug)]
pub(crate) struct EventModelingCssBinding {
    pub(crate) layout_settings: super::config::EventModelingLayoutSettings,
    pub(crate) text_color: String,
    pub(crate) ui_fill: String,
    pub(crate) ui_stroke: String,
    pub(crate) processor_fill: String,
    pub(crate) processor_stroke: String,
    pub(crate) read_model_fill: String,
    pub(crate) read_model_stroke: String,
    pub(crate) command_fill: String,
    pub(crate) command_stroke: String,
    pub(crate) event_fill: String,
    pub(crate) event_stroke: String,
    pub(crate) swimlane_background_fill: String,
    pub(crate) swimlane_background_stroke: String,
    pub(crate) relation_stroke: String,
    pub(crate) arrowhead_fill: String,
}

impl EventModelingCssBinding {
    pub(crate) fn resolve(config: &serde_json::Value) -> Self {
        let optional_color =
            |key: &str| crate::config::config_string(config, &["themeVariables", key]);
        let color =
            |key: &str, fallback: &str| optional_color(key).unwrap_or_else(|| fallback.to_string());
        Self {
            layout_settings: super::config::EventModelingConfigView::new(config).layout_settings(),
            text_color: color("textColor", "#333"),
            ui_fill: color("emUiFill", "white"),
            ui_stroke: color("emUiStroke", "#dbdada"),
            processor_fill: color("emProcessorFill", "#edb3f6"),
            processor_stroke: color("emProcessorStroke", "#b88cbf"),
            read_model_fill: color("emReadModelFill", "#d3f1a2"),
            read_model_stroke: color("emReadModelStroke", "#a3b732"),
            command_fill: color("emCommandFill", "#bcd6fe"),
            command_stroke: color("emCommandStroke", "#679ac3"),
            event_fill: color("emEventFill", "#ffb778"),
            event_stroke: color("emEventStroke", "#c19a0f"),
            swimlane_background_fill: optional_color("emSwimlaneBackgroundOdd")
                .or_else(|| optional_color("emSwimlaneBackground"))
                .unwrap_or_else(|| "rgb(250,250,250)".to_string()),
            swimlane_background_stroke: optional_color("emSwimlaneBackgroundStroke")
                .or_else(|| optional_color("emSwimlaneBorder"))
                .unwrap_or_else(|| "rgb(240,240,240)".to_string()),
            relation_stroke: color("emRelationStroke", "#000"),
            arrowhead_fill: color("emArrowhead", "#000000"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn raw_css_and_alias_fallbacks_bind_without_color_validation() {
        let theme = EventModelingCssBinding::resolve(&json!({
            "themeVariables": {
                "emUiFill": "var(--ui)", "emUiStroke": "currentColor",
                "emSwimlaneBackgroundOdd": null, "emSwimlaneBackground": "var(--lane)",
                "emSwimlaneBackgroundStroke": "", "emSwimlaneBorder": "red",
                "emArrowhead": "none"
            }
        }));
        assert_eq!(theme.ui_fill, "var(--ui)");
        assert_eq!(theme.ui_stroke, "currentColor");
        assert_eq!(theme.swimlane_background_fill, "var(--lane)");
        assert_eq!(theme.swimlane_background_stroke, "");
        assert_eq!(theme.arrowhead_fill, "none");
    }

    #[test]
    fn binding_resolves_eventmodeling_roles() {
        let cfg = json!({
            "themeVariables": {
                "textColor": "#111111",
                "emUiFill": "#fefefe",
                "emUiStroke": "#222222",
                "emCommandFill": "#DDEEFF",
                "emCommandStroke": "#336699",
                "emSwimlaneBackgroundOdd": "#fafafa",
                "emSwimlaneBackgroundStroke": "#efefef",
                "emRelationStroke": "#135790",
                "emArrowhead": "#02468a"
            }
        });

        let eventmodeling = EventModelingCssBinding::resolve(&cfg);

        assert_eq!(eventmodeling.text_color, "#111111");
        assert_eq!(eventmodeling.ui_fill, "#fefefe");
        assert_eq!(eventmodeling.ui_stroke, "#222222");
        assert_eq!(eventmodeling.command_fill, "#DDEEFF");
        assert_eq!(eventmodeling.command_stroke, "#336699");
        assert_eq!(eventmodeling.swimlane_background_fill, "#fafafa");
        assert_eq!(eventmodeling.swimlane_background_stroke, "#efefef");
        assert_eq!(eventmodeling.relation_stroke, "#135790");
        assert_eq!(eventmodeling.arrowhead_fill, "#02468a");
    }

    #[test]
    fn binding_uses_default_eventmodeling_roles() {
        let cfg = json!({});

        let eventmodeling = EventModelingCssBinding::resolve(&cfg);

        assert_eq!(eventmodeling.text_color, "#333");
        assert_eq!(eventmodeling.ui_fill, "white");
        assert_eq!(eventmodeling.ui_stroke, "#dbdada");
        assert_eq!(eventmodeling.swimlane_background_fill, "rgb(250,250,250)");
        assert_eq!(eventmodeling.swimlane_background_stroke, "rgb(240,240,240)");
        assert_eq!(eventmodeling.relation_stroke, "#000");
        assert_eq!(eventmodeling.arrowhead_fill, "#000000");
    }
}
