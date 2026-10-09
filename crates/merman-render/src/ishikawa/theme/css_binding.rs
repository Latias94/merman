use crate::config::config_string;

#[derive(Debug)]
pub(crate) struct IshikawaCssBinding {
    pub(crate) line_color: String,
    pub(crate) main_bkg: String,
    pub(crate) text_color: String,
    pub(crate) look: String,
    pub(crate) hand_drawn_seed: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn raw_css_spelling_is_not_restricted_to_native_colors() {
        let binding = IshikawaCssBinding::resolve(&json!({"themeVariables": {
            "lineColor": "var(--line)", "mainBkg": "currentColor", "textColor": "red"
        }}));
        assert_eq!(binding.line_color, "var(--line)");
        assert_eq!(binding.main_bkg, "currentColor");
        assert_eq!(binding.text_color, "red");
        let defaults = IshikawaCssBinding::resolve(&json!({}));
        assert_eq!(defaults.line_color, "#333");
        assert_eq!(defaults.main_bkg, "#fff");
        assert_eq!(defaults.text_color, "#333");
    }
}

impl IshikawaCssBinding {
    pub(super) fn resolve(config: &serde_json::Value) -> Self {
        let color = |key, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_owned())
        };
        Self {
            look: crate::config::config_diagram_look(config)
                .as_str()
                .to_owned(),
            hand_drawn_seed: config
                .get("handDrawnSeed")
                .and_then(serde_json::Value::as_f64),
            line_color: color("lineColor", "#333"),
            main_bkg: color("mainBkg", "#fff"),
            text_color: color("textColor", "#333"),
        }
    }
}
