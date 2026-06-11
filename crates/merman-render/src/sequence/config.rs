use crate::text::TextStyle;
use serde_json::Value;

pub(crate) struct SequenceConfigView<'a> {
    effective_config: &'a Value,
    sequence_config: &'a Value,
}

impl<'a> SequenceConfigView<'a> {
    pub(crate) fn new(effective_config: &'a Value) -> Self {
        Self {
            effective_config,
            sequence_config: effective_config.get("sequence").unwrap_or(&Value::Null),
        }
    }

    pub(crate) fn sequence_bool(&self, key: &str, default: bool) -> bool {
        self.sequence_config_bool(key).unwrap_or(default)
    }

    pub(crate) fn sequence_config_bool(&self, key: &str) -> Option<bool> {
        self.sequence_config.get(key).and_then(Value::as_bool)
    }

    fn sequence_json_number_or(&self, key: &str, default: f64) -> f64 {
        self.sequence_json_number(key).unwrap_or(default)
    }

    pub(crate) fn sequence_json_number(&self, key: &str) -> Option<f64> {
        self.sequence_config.get(key).and_then(Value::as_f64)
    }

    pub(crate) fn sequence_json_number_min(&self, key: &str, default: f64, min: f64) -> f64 {
        self.sequence_json_number_or(key, default).max(min)
    }

    pub(crate) fn root_json_number(&self, key: &str) -> Option<f64> {
        self.effective_config.get(key).and_then(Value::as_f64)
    }

    pub(crate) fn root_bool(&self, key: &str) -> Option<bool> {
        self.effective_config.get(key).and_then(Value::as_bool)
    }

    pub(crate) fn root_string(&self, key: &str) -> Option<String> {
        crate::config::config_string(self.effective_config, &[key])
    }

    pub(crate) fn sequence_string(&self, key: &str) -> Option<String> {
        crate::config::config_string(self.sequence_config, &[key])
    }

    fn sequence_compat_f64(&self, key: &str, default: f64) -> f64 {
        crate::config::config_f64(self.sequence_config, &[key]).unwrap_or(default)
    }

    fn sequence_compat_f64_min(&self, key: &str, default: f64, min: f64) -> f64 {
        self.sequence_compat_f64(key, default).max(min)
    }

    fn root_compat_f64(&self, key: &str) -> Option<f64> {
        crate::config::config_f64(self.effective_config, &[key])
    }

    fn layout_text_style(
        &self,
        root_font_family: &Option<String>,
        root_font_size: Option<f64>,
        root_font_weight: &Option<String>,
        family_key: &str,
        size_key: &str,
        weight_key: &str,
    ) -> TextStyle {
        let font_family = root_font_family
            .clone()
            .or_else(|| self.sequence_string(family_key));
        let font_size = root_font_size
            .or_else(|| crate::config::config_f64(self.sequence_config, &[size_key]))
            .unwrap_or(16.0);
        let font_weight = root_font_weight
            .clone()
            .or_else(|| self.sequence_string(weight_key));

        TextStyle {
            font_family,
            font_size,
            font_weight,
        }
    }
}

pub(super) struct SequenceLayoutSettings {
    pub(super) diagram_margin_x: f64,
    pub(super) diagram_margin_y: f64,
    pub(super) bottom_margin_adj: f64,
    pub(super) box_margin: f64,
    pub(super) actor_margin: f64,
    pub(super) sequence_default_width: f64,
    pub(super) actor_height: f64,
    pub(super) message_margin: f64,
    pub(super) wrap_padding: f64,
    pub(super) box_text_margin: f64,
    pub(super) label_box_height: f64,
    pub(super) mirror_actors: bool,
    pub(super) message_width_scale: f64,
    pub(super) message_text_line_height: f64,
    pub(super) message_step: f64,
    pub(super) msg_label_offset: f64,
    pub(super) activation_width: f64,
    pub(super) actor_text_style: TextStyle,
    pub(super) note_text_style: TextStyle,
    pub(super) msg_text_style: TextStyle,
}

impl SequenceLayoutSettings {
    pub(super) fn from_effective_config(effective_config: &Value) -> Self {
        let config = SequenceConfigView::new(effective_config);

        let diagram_margin_x = config.sequence_compat_f64("diagramMarginX", 50.0);
        let diagram_margin_y = config.sequence_compat_f64("diagramMarginY", 10.0);
        let bottom_margin_adj = config.sequence_compat_f64("bottomMarginAdj", 1.0);
        let box_margin = config.sequence_compat_f64("boxMargin", 10.0);
        let actor_margin = config.sequence_compat_f64("actorMargin", 50.0);
        let sequence_default_width = config.sequence_compat_f64("width", 150.0);
        let actor_height = config.sequence_compat_f64("height", 65.0);
        let message_margin = config.sequence_compat_f64("messageMargin", 35.0);
        let wrap_padding = config.sequence_compat_f64("wrapPadding", 10.0);
        let box_text_margin = config.sequence_compat_f64("boxTextMargin", 5.0);
        let label_box_height = config.sequence_compat_f64("labelBoxHeight", 20.0);
        let mirror_actors = config.sequence_bool("mirrorActors", true);
        let activation_width = config.sequence_compat_f64_min("activationWidth", 10.0, 1.0);

        // Mermaid's `sequenceRenderer.setConf(...)` overrides per-sequence font settings whenever
        // the global `fontFamily` / `fontSize` / `fontWeight` are present.
        let root_font_family = config.root_string("fontFamily");
        let root_font_size = config.root_compat_f64("fontSize");
        let root_font_weight = config.root_string("fontWeight");
        let actor_text_style = config.layout_text_style(
            &root_font_family,
            root_font_size,
            &root_font_weight,
            "actorFontFamily",
            "actorFontSize",
            "actorFontWeight",
        );
        let note_text_style = config.layout_text_style(
            &root_font_family,
            root_font_size,
            &root_font_weight,
            "noteFontFamily",
            "noteFontSize",
            "noteFontWeight",
        );
        let msg_text_style = config.layout_text_style(
            &root_font_family,
            root_font_size,
            &root_font_weight,
            "messageFontFamily",
            "messageFontSize",
            "messageFontWeight",
        );

        // Upstream sequence uses SVG `getBBox()` when computing message widths. Keep this scale at
        // 1.0 and let the selected TextMeasurer account for SVG/browser residuals.
        let message_width_scale = 1.0;

        let message_text_line_height =
            crate::sequence::sequence_text_dimensions_height_px(msg_text_style.font_size);
        let message_step = box_margin + 2.0 * message_text_line_height;
        let msg_label_offset = (2.0 * message_text_line_height - wrap_padding / 2.0).max(0.0);

        Self {
            diagram_margin_x,
            diagram_margin_y,
            bottom_margin_adj,
            box_margin,
            actor_margin,
            sequence_default_width,
            actor_height,
            message_margin,
            wrap_padding,
            box_text_margin,
            label_box_height,
            mirror_actors,
            message_width_scale,
            message_text_line_height,
            message_step,
            msg_label_offset,
            activation_width,
            actor_text_style,
            note_text_style,
            msg_text_style,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sequence_layout_settings_preserve_layout_numeric_string_config() {
        let cfg = json!({
            "fontFamily": "Global, Arial",
            "fontSize": "22",
            "fontWeight": "700",
            "sequence": {
                "width": "240",
                "height": "80",
                "activationWidth": "0.25",
                "mirrorActors": false,
                "messageFontFamily": "Message",
                "messageFontSize": "18",
                "messageFontWeight": "300"
            }
        });

        let settings = SequenceLayoutSettings::from_effective_config(&cfg);

        assert_eq!(settings.sequence_default_width, 240.0);
        assert_eq!(settings.actor_height, 80.0);
        assert_eq!(settings.activation_width, 1.0);
        assert!(!settings.mirror_actors);
        assert_eq!(
            settings.msg_text_style.font_family.as_deref(),
            Some("Global, Arial")
        );
        assert_eq!(settings.msg_text_style.font_size, 22.0);
        assert_eq!(settings.msg_text_style.font_weight.as_deref(), Some("700"));
    }

    #[test]
    fn sequence_layout_settings_use_family_font_fallbacks_without_global_font() {
        let cfg = json!({
            "sequence": {
                "actorFontFamily": "Actor",
                "actorFontSize": "19",
                "actorFontWeight": "500",
                "noteFontFamily": "Note",
                "noteFontSize": 20,
                "noteFontWeight": "600",
                "messageFontFamily": "Message",
                "messageFontSize": 21,
                "messageFontWeight": "700"
            }
        });

        let settings = SequenceLayoutSettings::from_effective_config(&cfg);

        assert_eq!(
            settings.actor_text_style.font_family.as_deref(),
            Some("Actor")
        );
        assert_eq!(settings.actor_text_style.font_size, 19.0);
        assert_eq!(
            settings.actor_text_style.font_weight.as_deref(),
            Some("500")
        );
        assert_eq!(
            settings.note_text_style.font_family.as_deref(),
            Some("Note")
        );
        assert_eq!(settings.note_text_style.font_size, 20.0);
        assert_eq!(settings.note_text_style.font_weight.as_deref(), Some("600"));
        assert_eq!(
            settings.msg_text_style.font_family.as_deref(),
            Some("Message")
        );
        assert_eq!(settings.msg_text_style.font_size, 21.0);
        assert_eq!(settings.msg_text_style.font_weight.as_deref(), Some("700"));
    }
}
