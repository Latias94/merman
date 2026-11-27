use crate::activation::SequenceActorPalette;
use merman_render_core::{
    config::{
        ConfigLookup, config_css_number_or_string, config_diagram_look, config_font_family_css,
        config_root_font_family_css, config_string,
    },
    mermaid_style::font_family_css,
};
use merman_theme_contract::v2::{Color, FamilyTheme};

#[derive(Debug)]
pub struct SequenceCompatBinding {
    pub svg_activation_width: f64,
    pub font_family: String,
    pub root_font_family: String,
    pub font_size_css: String,
    pub text_color: String,
    pub line_color: String,
    pub error_bkg: String,
    pub error_text: String,
    pub actor_border: String,
    pub actor_fill: String,
    pub stroke_width: String,
    pub native_stroke_width: Option<f64>,
    pub drop_shadow: String,
    pub note_border: String,
    pub note_fill: String,
    pub actor_text: String,
    pub actor_line: String,
    pub signal_color: String,
    pub sequence_number: String,
    pub signal_text: String,
    pub label_box_border: String,
    pub label_box_fill: String,
    pub label_text: String,
    pub loop_text: String,
    pub note_text: String,
    pub activation_fill: String,
    pub activation_border: String,
    pub node_border: String,
    pub label_box_filter: String,
    pub rect_default_fill: String,
    pub is_neo: bool,
    pub neo_flood_color: &'static str,
    pub actor_palette: SequenceActorPalette,
}

impl SequenceCompatBinding {
    pub(super) fn for_native(
        config: &dyn ConfigLookup,
        theme: &FamilyTheme,
        background: Option<Color>,
    ) -> Self {
        let roles = &theme.roles;
        let typography = &theme.typography;
        let font_family = font_family_css(typography.stack(typography.body_family));
        let connector = roles.stroke_connector.first_color();
        let canvas = background.unwrap_or_else(|| roles.canvas.first_color());
        let badge = connector.composite(canvas, 1.0);
        let black = Color::rgb(0, 0, 0);
        let white = Color::rgb(255, 255, 255);
        let ink = if badge.contrast(black) >= badge.contrast(white) {
            black
        } else {
            white
        };
        Self {
            svg_activation_width: super::config::SequenceConfigView::new(config)
                .sequence_json_number_min("activationWidth", 10.0, 1.0),
            root_font_family: font_family.clone(),
            font_family,
            font_size_css: format!("{}px", typography.base_size_px),
            text_color: roles.text_primary.to_string(),
            line_color: connector.to_string(),
            error_bkg: roles.status_danger.to_string(),
            error_text: roles.text_primary.to_string(),
            actor_border: roles.stroke_border.first_color().to_string(),
            actor_fill: roles.surface_node.first_color().to_string(),
            stroke_width: theme.form.stroke_width.to_string(),
            native_stroke_width: Some(theme.form.stroke_width),
            drop_shadow: "none".to_owned(),
            note_border: roles.stroke_border.first_color().to_string(),
            note_fill: roles.surface_callout.first_color().to_string(),
            actor_text: roles.text_primary.to_string(),
            actor_line: roles.stroke_guide.first_color().to_string(),
            signal_color: connector.to_string(),
            sequence_number: ink.to_string(),
            signal_text: roles.text_primary.to_string(),
            label_box_border: roles.stroke_border.first_color().to_string(),
            label_box_fill: roles.surface_emphasis.first_color().to_string(),
            label_text: roles.text_on_emphasis.to_string(),
            loop_text: roles.text_primary.to_string(),
            note_text: roles.text_callout.to_string(),
            activation_fill: roles.surface_emphasis.first_color().to_string(),
            activation_border: connector.to_string(),
            node_border: roles.stroke_border.first_color().to_string(),
            label_box_filter: "none".to_owned(),
            rect_default_fill: roles.surface_container.first_color().to_string(),
            is_neo: config_diagram_look(config).is_neo(),
            neo_flood_color: "#000000",
            actor_palette: SequenceActorPalette::default(),
        }
    }

    pub(crate) fn resolve(config: &dyn ConfigLookup) -> Self {
        let color = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_owned())
        };
        let value = |key: &str, fallback: &str| {
            config_css_number_or_string(config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_owned())
        };
        let actor_border = color("actorBorder", "#9370DB");
        let actor_fill = color("actorBkg", "#ECECFF");
        let actor_text = color("actorTextColor", "black");
        let stroke_width = value("strokeWidth", "1");
        let native_stroke_width = stroke_width
            .trim()
            .strip_suffix("px")
            .unwrap_or(stroke_width.trim())
            .parse::<f64>()
            .ok()
            .filter(|width| width.is_finite() && *width >= 0.0);
        let drop_shadow = value("dropShadow", "none");
        let is_neo = config_diagram_look(config).is_neo();
        let theme_name = config.get("theme").and_then(serde_json::Value::as_str);
        let rect_default_fill = config_string(config, &["themeVariables", "rectBkgColor"])
            .filter(|fill| !fill.is_empty())
            .or_else(|| {
                config_string(config, &["themeVariables", "actorBkg"])
                    .filter(|fill| !fill.is_empty())
            })
            .unwrap_or_else(|| "rgba(128, 128, 128, 0.5)".to_owned());
        Self {
            svg_activation_width: super::config::SequenceConfigView::new(config)
                .sequence_json_number_min("activationWidth", 10.0, 1.0),
            font_family: config_font_family_css(config),
            root_font_family: config_root_font_family_css(config),
            font_size_css: value("fontSize", "16px"),
            text_color: color("textColor", "#333"),
            line_color: color("lineColor", "#333333"),
            error_bkg: color("errorBkgColor", "#552222"),
            error_text: color("errorTextColor", "#552222"),
            actor_line: color("actorLineColor", &actor_border),
            label_box_border: color("labelBoxBorderColor", &actor_border),
            label_box_fill: color("labelBoxBkgColor", &actor_fill),
            label_text: color("labelTextColor", &actor_text),
            loop_text: color("loopTextColor", &actor_text),
            node_border: color("nodeBorder", &actor_border),
            actor_border,
            actor_fill,
            actor_text,
            stroke_width,
            native_stroke_width,
            label_box_filter: if is_neo {
                drop_shadow.clone()
            } else {
                "none".to_owned()
            },
            drop_shadow,
            note_border: color("noteBorderColor", "#aaaa33"),
            note_fill: color("noteBkgColor", "#fff5ad"),
            signal_color: color("signalColor", "#333"),
            sequence_number: color("sequenceNumberColor", "white"),
            signal_text: color("signalTextColor", "#333"),
            note_text: color("noteTextColor", "black"),
            activation_fill: color("activationBkgColor", "#f4f4f4"),
            activation_border: color("activationBorderColor", "#666"),
            rect_default_fill,
            is_neo,
            neo_flood_color: if matches!(theme_name, Some("redux" | "redux-color")) {
                "#000000"
            } else {
                "#FFFFFF"
            },
            actor_palette: SequenceActorPalette::resolve(config),
        }
    }

    pub(crate) fn text_surface_fill(&self, surface: super::SequenceTextSurface) -> &str {
        use super::SequenceTextSurface;

        match surface {
            SequenceTextSurface::ParticipantLabel => &self.actor_text,
            SequenceTextSurface::BoxTitle => &self.text_color,
            SequenceTextSurface::MessageLabel => &self.signal_text,
            SequenceTextSurface::NoteLabel => &self.note_text,
            SequenceTextSurface::ControlKeyword => &self.label_text,
            SequenceTextSurface::ControlPrimaryTitle | SequenceTextSurface::ControlSectionTitle => {
                &self.loop_text
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SequenceCompatBinding;
    use serde_json::json;

    #[test]
    fn dependent_raw_colors_share_their_configured_parent() {
        let binding = SequenceCompatBinding::resolve(&json!({
            "themeVariables": {
                "actorBorder": "red", "actorBkg": "blue", "actorTextColor": "green",
                "textColor": "purple", "signalTextColor": "yellow", "noteTextColor": "cyan"
            }
        }));
        assert_eq!(binding.actor_line, "red");
        assert_eq!(binding.label_box_border, "red");
        assert_eq!(binding.node_border, "red");
        assert_eq!(binding.label_box_fill, "blue");
        assert_eq!(binding.rect_default_fill, "blue");
        assert_eq!(
            super::super::SequenceTextSurface::ALL
                .map(|surface| binding.text_surface_fill(surface)),
            [
                "green", "purple", "yellow", "cyan", "green", "green", "green"
            ]
        );
    }

    #[test]
    fn native_shadow_width_only_accepts_absolute_finite_nonnegative_values() {
        for (value, expected) in [
            ("2px", Some(2.0)),
            (" 3 ", Some(3.0)),
            ("0", Some(0.0)),
            ("2em", None),
            ("-1", None),
            ("NaN", None),
        ] {
            let binding = SequenceCompatBinding::resolve(&json!({
                "look": "neo", "theme": "redux",
                "themeVariables": {"strokeWidth": value, "dropShadow": "url(#drop-shadow)"}
            }));
            assert_eq!(binding.stroke_width, value.trim());
            assert_eq!(binding.native_stroke_width, expected);
            assert_eq!(binding.label_box_filter, "url(#drop-shadow)");
            assert_eq!(binding.neo_flood_color, "#000000");
        }
        let classic = SequenceCompatBinding::resolve(&json!({
            "themeVariables": {"dropShadow": "url(#custom)"}
        }));
        assert_eq!(classic.drop_shadow, "url(#custom)");
        assert_eq!(classic.label_box_filter, "none");
    }
}
