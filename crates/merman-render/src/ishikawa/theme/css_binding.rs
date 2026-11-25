use merman_model::presentation::PresentationConfig;

use crate::config::{ConfigLookup, config_string};
use crate::theme::FamilyTheme;

#[derive(Debug)]
pub(crate) struct IshikawaCssBinding {
    pub(crate) line_color: String,
    pub(crate) main_bkg: String,
    pub(crate) node_stroke: String,
    pub(crate) marker_fill: String,
    pub(crate) text_color: String,
    pub(crate) look: String,
    pub(crate) hand_drawn_seed: Option<f64>,
    pub(crate) native: Option<IshikawaNativePaint>,
}

#[derive(Debug)]
pub(crate) struct IshikawaNativePaint {
    pub(crate) head_fill: String,
    pub(crate) head_ink: String,
}

impl IshikawaCssBinding {
    pub(super) fn resolve(config: &impl ConfigLookup) -> Self {
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
            node_stroke: color("lineColor", "#333"),
            marker_fill: color("lineColor", "#333"),
            text_color: color("textColor", "#333"),
            native: None,
        }
    }

    pub(super) fn apply_native(&mut self, theme: &FamilyTheme, config: &PresentationConfig) {
        let roles = &theme.roles;
        let owns = |path| config.explicit_config_owns_path(path) && config.get_str(path).is_some();
        let has_authored_line = owns("themeVariables.lineColor");
        let has_authored_surface = owns("themeVariables.mainBkg");
        let has_authored_text = owns("themeVariables.textColor");
        if !has_authored_line {
            self.line_color = roles.stroke_connector.first_color().to_string();
            self.marker_fill.clone_from(&self.line_color);
            self.node_stroke = roles.stroke_border.first_color().to_string();
        }
        if !has_authored_surface {
            self.main_bkg = roles.surface_node.first_color().to_string();
        }
        if !has_authored_text {
            self.text_color = roles.text_primary.to_string();
        }
        self.native = Some(IshikawaNativePaint {
            head_fill: if has_authored_surface {
                self.main_bkg.clone()
            } else {
                roles.surface_emphasis.first_color().to_string()
            },
            head_ink: if has_authored_text || has_authored_surface {
                self.text_color.clone()
            } else {
                roles.text_on_emphasis.to_string()
            },
        });
    }

    pub(crate) fn head_fill(&self) -> &str {
        self.native
            .as_ref()
            .map_or(&self.main_bkg, |paint| &paint.head_fill)
    }

    pub(crate) fn sub_branch_stroke(&self) -> &str {
        if self.native.is_some() {
            &self.node_stroke
        } else {
            &self.line_color
        }
    }
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
