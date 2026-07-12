use crate::text::TextStyle;

pub(super) struct SequenceRenderSettings {
    pub(super) force_menus: bool,
    pub(super) mirror_actors: bool,
    pub(super) diagram_margin_x: f64,
    pub(super) box_margin: f64,
    pub(super) actor_height: f64,
    pub(super) box_text_margin: f64,
    pub(super) message_align: String,
    pub(super) label_box_height: f64,
    pub(super) right_angles: bool,
    pub(super) wrap_padding: f64,
    pub(super) sequence_width: f64,
    pub(super) activation_width: f64,
    pub(super) actor_label_font_size: f64,
    pub(super) actor_wrap_width: f64,
    pub(super) rect_default_fill: String,
    pub(super) loop_text_style: TextStyle,
    pub(super) note_text_style: TextStyle,
}

impl SequenceRenderSettings {
    pub(super) fn from_effective_config(effective_config: &serde_json::Value) -> Self {
        let config = crate::sequence::config::SequenceConfigView::new(effective_config);

        let force_menus = config
            .sequence_config_bool("forceMenus")
            .or_else(|| config.root_bool("forceMenus"))
            .unwrap_or(false);
        let mirror_actors = config.sequence_bool("mirrorActors", true);
        let diagram_margin_x = config.sequence_json_number_min("diagramMarginX", 50.0, 0.0);
        let box_margin = config.sequence_json_number_min("boxMargin", 10.0, 0.0);
        let actor_height = config.sequence_json_number_min("height", 65.0, 1.0);
        let box_text_margin = config.sequence_json_number_min("boxTextMargin", 5.0, 0.0);
        let message_align = config
            .sequence_string("messageAlign")
            .unwrap_or_else(|| "center".to_string());
        let label_box_height = config.sequence_json_number_min("labelBoxHeight", 20.0, 0.0);
        let right_angles = config.sequence_bool("rightAngles", false);
        let wrap_padding = config.sequence_json_number_min("wrapPadding", 10.0, 0.0);
        let sequence_width = config.sequence_json_number_min("width", 150.0, 1.0);
        let activation_width = config.sequence_json_number_min("activationWidth", 10.0, 1.0);

        // Upstream Mermaid's Sequence renderer treats the global `fontSize` as authoritative.
        // Per-sequence overrides like `sequence.messageFontSize` apply only when the global value
        // is absent.
        let actor_label_font_size = config
            .root_json_number("fontSize")
            .or_else(|| config.sequence_json_number("messageFontSize"))
            .unwrap_or(16.0)
            .max(1.0);
        let loop_text_style = TextStyle {
            font_family: config.root_string("fontFamily"),
            font_size: actor_label_font_size,
            font_weight: Some("400".to_string()),
        };
        let note_text_style = TextStyle {
            font_family: loop_text_style.font_family.clone(),
            font_size: actor_label_font_size,
            font_weight: Some("400".to_string()),
        };
        let actor_wrap_width = (sequence_width - 2.0 * wrap_padding).max(1.0);
        let rect_default_fill =
            crate::config::config_string(effective_config, &["themeVariables", "rectBkgColor"])
                .filter(|fill| !fill.is_empty())
                .or_else(|| {
                    crate::config::config_string(effective_config, &["themeVariables", "actorBkg"])
                        .filter(|fill| !fill.is_empty())
                })
                .unwrap_or_else(|| "rgba(128, 128, 128, 0.5)".to_string());

        Self {
            force_menus,
            mirror_actors,
            diagram_margin_x,
            box_margin,
            actor_height,
            box_text_margin,
            message_align,
            label_box_height,
            right_angles,
            wrap_padding,
            sequence_width,
            activation_width,
            actor_label_font_size,
            actor_wrap_width,
            rect_default_fill,
            loop_text_style,
            note_text_style,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sequence_render_settings_keep_svg_numeric_type_semantics() {
        let cfg = json!({
            "fontSize": "22",
            "sequence": {
                "width": "240",
                "wrapPadding": "12",
                "activationWidth": "0.25",
                "messageFontSize": "18"
            }
        });

        let settings = SequenceRenderSettings::from_effective_config(&cfg);

        assert_eq!(settings.sequence_width, 150.0);
        assert_eq!(settings.wrap_padding, 10.0);
        assert_eq!(settings.activation_width, 10.0);
        assert_eq!(settings.actor_label_font_size, 16.0);
        assert_eq!(settings.actor_wrap_width, 130.0);
    }

    #[test]
    fn sequence_render_settings_apply_number_precedence_and_clamps() {
        let cfg = json!({
            "fontFamily": "Inter, sans-serif",
            "fontSize": 22,
            "forceMenus": true,
            "sequence": {
                "forceMenus": false,
                "mirrorActors": false,
                "diagramMarginX": -5,
                "boxMargin": -1,
                "height": 0,
                "boxTextMargin": -1,
                "messageAlign": "left",
                "labelBoxHeight": -1,
                "rightAngles": true,
                "wrapPadding": -2,
                "width": 0.5,
                "activationWidth": 0,
                "messageFontSize": 18
            }
        });

        let settings = SequenceRenderSettings::from_effective_config(&cfg);

        assert!(!settings.force_menus);
        assert!(!settings.mirror_actors);
        assert_eq!(settings.diagram_margin_x, 0.0);
        assert_eq!(settings.box_margin, 0.0);
        assert_eq!(settings.actor_height, 1.0);
        assert_eq!(settings.box_text_margin, 0.0);
        assert_eq!(settings.message_align, "left");
        assert_eq!(settings.label_box_height, 0.0);
        assert!(settings.right_angles);
        assert_eq!(settings.wrap_padding, 0.0);
        assert_eq!(settings.sequence_width, 1.0);
        assert_eq!(settings.activation_width, 1.0);
        assert_eq!(settings.actor_label_font_size, 22.0);
        assert_eq!(settings.actor_wrap_width, 1.0);
        assert_eq!(
            settings.loop_text_style.font_family.as_deref(),
            Some("Inter, sans-serif")
        );
        assert_eq!(settings.loop_text_style.font_size, 22.0);
        assert_eq!(
            settings.note_text_style.font_family.as_deref(),
            Some("Inter, sans-serif")
        );
    }

    #[test]
    fn sequence_render_settings_fall_back_to_root_force_menus() {
        let cfg = json!({
            "forceMenus": true,
            "sequence": {}
        });

        let settings = SequenceRenderSettings::from_effective_config(&cfg);

        assert!(settings.force_menus);
    }
}
