use crate::config::{config_css_number_or_string, config_string};

#[derive(Debug)]
pub(crate) struct SequenceCompatBinding {
    pub(crate) svg_activation_width: f64,
    pub(crate) font_family: String,
    pub(crate) root_font_family: String,
    pub(crate) font_size_css: String,
    pub(crate) text_color: String,
    pub(crate) line_color: String,
    pub(crate) error_bkg: String,
    pub(crate) error_text: String,
    pub(crate) actor_border: String,
    pub(crate) actor_fill: String,
    pub(crate) stroke_width: String,
    pub(crate) native_stroke_width: Option<f64>,
    pub(crate) drop_shadow: String,
    pub(crate) note_border: String,
    pub(crate) note_fill: String,
    pub(crate) actor_text: String,
    pub(crate) actor_line: String,
    pub(crate) signal_color: String,
    pub(crate) sequence_number: String,
    pub(crate) signal_text: String,
    pub(crate) label_box_border: String,
    pub(crate) label_box_fill: String,
    pub(crate) label_text: String,
    pub(crate) loop_text: String,
    pub(crate) note_text: String,
    pub(crate) activation_fill: String,
    pub(crate) activation_border: String,
    pub(crate) node_border: String,
    pub(crate) label_box_filter: String,
    pub(crate) rect_default_fill: String,
    pub(crate) is_neo: bool,
    pub(crate) neo_flood_color: &'static str,
    redux_palette: bool,
    actor_borders: Vec<SequencePaletteEntry>,
    actor_fills: Vec<SequencePaletteEntry>,
    activation_fill_fallback: Option<String>,
}

#[derive(Debug)]
enum SequencePaletteEntry {
    Null,
    Token(String),
    Other,
}

impl SequenceCompatBinding {
    pub(crate) fn resolve(config: &serde_json::Value) -> Self {
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
        let is_neo = crate::config::config_diagram_look(config).is_neo();
        let theme_name = config.get("theme").and_then(serde_json::Value::as_str);
        let palette = |key: &str| {
            config
                .get("themeVariables")
                .and_then(|theme| theme.get(key))
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .map(|entry| match entry {
                    serde_json::Value::Null => SequencePaletteEntry::Null,
                    serde_json::Value::String(value) => SequencePaletteEntry::Token(value.clone()),
                    _ => SequencePaletteEntry::Other,
                })
                .collect()
        };
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
            font_family: crate::config::config_font_family_css(config),
            root_font_family: crate::config::config_root_font_family_css(config),
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
            redux_palette: matches!(theme_name, Some("redux-color" | "redux-dark-color")),
            actor_borders: palette("borderColorArray"),
            actor_fills: palette("bkgColorArray"),
            activation_fill_fallback: config_string(config, &["themeVariables", "mainBkg"]),
        }
    }

    pub(crate) fn text_surface_fills(&self) -> [String; super::SequenceTextSurface::COUNT] {
        [
            self.actor_text.clone(),
            self.text_color.clone(),
            self.signal_text.clone(),
            self.note_text.clone(),
            self.label_text.clone(),
            self.loop_text.clone(),
            self.loop_text.clone(),
        ]
    }

    pub(crate) fn actor_palette_mode(&self) -> bool {
        self.redux_palette
    }

    pub(crate) fn actor_palette(&self, index: usize) -> (Option<&str>, Option<&str>) {
        (
            palette_token(&self.actor_borders, index),
            palette_token(&self.actor_fills, index),
        )
    }

    pub(crate) fn activation_palette(&self, index: usize) -> (Option<&str>, Option<&str>) {
        if !self.redux_palette {
            return (None, None);
        }
        let fill = match palette_entry(&self.actor_fills, index) {
            None | Some(SequencePaletteEntry::Null) => self.activation_fill_fallback.as_deref(),
            Some(SequencePaletteEntry::Token(value)) => Some(value.as_str()),
            Some(SequencePaletteEntry::Other) => None,
        };
        (palette_token(&self.actor_borders, index), fill)
    }
}

fn palette_entry(palette: &[SequencePaletteEntry], index: usize) -> Option<&SequencePaletteEntry> {
    (!palette.is_empty()).then(|| &palette[index % palette.len()])
}

fn palette_token(palette: &[SequencePaletteEntry], index: usize) -> Option<&str> {
    match palette_entry(palette, index) {
        Some(SequencePaletteEntry::Token(value)) => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::SequenceCompatBinding;
    use serde_json::json;

    #[test]
    fn raw_palette_preserves_slots_and_activation_null_fallback() {
        let binding = SequenceCompatBinding::resolve(&json!({
            "theme": "redux-color",
            "themeVariables": {
                "borderColorArray": [null, "red", 7],
                "bkgColorArray": [null, "blue", 7],
                "mainBkg": "white"
            }
        }));
        assert_eq!(binding.actor_palette(0), (None, None));
        assert_eq!(binding.activation_palette(0), (None, Some("white")));
        assert_eq!(binding.actor_palette(1), (Some("red"), Some("blue")));
        assert_eq!(binding.activation_palette(2), (None, None));
        assert_eq!(binding.activation_palette(3), (None, Some("white")));

        let missing = SequenceCompatBinding::resolve(&json!({
            "theme": "redux-dark-color", "themeVariables": {"mainBkg": "white"}
        }));
        assert_eq!(missing.activation_palette(42), (None, Some("white")));
        let ordinary = SequenceCompatBinding::resolve(&json!({
            "theme": "redux", "themeVariables": {"mainBkg": "white"}
        }));
        assert!(!ordinary.actor_palette_mode());
        assert_eq!(ordinary.activation_palette(42), (None, None));
    }

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
            binding.text_surface_fills(),
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
