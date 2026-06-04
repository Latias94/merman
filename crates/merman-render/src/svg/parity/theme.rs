use super::util::SvgTheme;
use serde_json::Value;

#[derive(Debug, Clone)]
pub(super) struct CommonCssTheme {
    pub(super) theme_name: String,
    pub(super) look: String,
    pub(super) font_family_css: String,
    pub(super) font_size_px: f64,
    pub(super) text_color: String,
    pub(super) line_color: String,
    pub(super) error_bkg: String,
    pub(super) error_text: String,
}

impl CommonCssTheme {
    pub(super) fn is_dark_theme(&self) -> bool {
        self.theme_name.contains("dark")
    }
}

#[derive(Debug, Clone)]
pub(super) struct NodeDiagramTheme {
    pub(super) common: CommonCssTheme,
    pub(super) node_text_color: String,
    pub(super) title_color: String,
    pub(super) main_bkg: String,
    pub(super) node_border: String,
    pub(super) arrowhead_color: String,
    pub(super) stroke_width: String,
    pub(super) edge_label_background: String,
    pub(super) tertiary: String,
    pub(super) cluster_bkg: String,
    pub(super) cluster_border: String,
}

#[derive(Debug, Clone)]
pub(super) struct ClassDiagramTheme {
    pub(super) common: CommonCssTheme,
    pub(super) class_text: String,
    pub(super) note_text: String,
    pub(super) class_group_text: String,
    pub(super) title_color: String,
    pub(super) text_color: String,
    pub(super) main_bkg: String,
    pub(super) node_border: String,
    pub(super) cluster_bkg: String,
    pub(super) cluster_border: String,
    pub(super) stroke_width: String,
}

#[derive(Debug, Clone)]
pub(super) struct SequenceDiagramTheme {
    pub(super) common: CommonCssTheme,
    pub(super) actor_border: String,
    pub(super) actor_fill: String,
    pub(super) stroke_width: String,
    pub(super) drop_shadow: String,
    pub(super) note_border: String,
    pub(super) note_fill: String,
    pub(super) actor_text: String,
    pub(super) actor_line: String,
    pub(super) signal_color: String,
    pub(super) sequence_number: String,
    pub(super) signal_text: String,
    pub(super) label_box_border: String,
    pub(super) label_box_fill: String,
    pub(super) label_text: String,
    pub(super) loop_text: String,
    pub(super) note_text: String,
    pub(super) activation_fill: String,
    pub(super) activation_border: String,
    pub(super) node_border: String,
    pub(super) note_font_weight: String,
    pub(super) label_box_filter: String,
}

#[derive(Debug, Clone)]
pub(super) struct StateDiagramTheme {
    pub(super) common: CommonCssTheme,
    pub(super) transition_color: String,
    pub(super) node_border: String,
    pub(super) background: String,
    pub(super) main_bkg: String,
    pub(super) alt_background: String,
    pub(super) stroke_width: String,
    pub(super) stroke_width_px: String,
    pub(super) rough_stroke_width_value: f64,
    pub(super) note_border: String,
    pub(super) note_bkg: String,
    pub(super) note_text: String,
    pub(super) label_background: String,
    pub(super) edge_label_background: String,
    pub(super) transition_label_color: String,
    pub(super) special_state_color: String,
    pub(super) inner_end_background: String,
    pub(super) end_outer_fill: String,
    pub(super) end_outer_stroke: String,
    pub(super) end_inner_stroke: String,
    pub(super) composite_background: String,
    pub(super) state_bkg: String,
    pub(super) state_border: String,
    pub(super) composite_title_background: String,
    pub(super) state_label_color: String,
    pub(super) drop_shadow: String,
}

pub(super) struct PresentationTheme<'a> {
    raw: SvgTheme<'a>,
    common: CommonCssTheme,
}

impl<'a> PresentationTheme<'a> {
    pub(super) fn new(effective_config: &'a Value) -> Self {
        let raw = SvgTheme::new(effective_config);
        let common = CommonCssTheme {
            theme_name: raw.theme_name(),
            look: raw.look(),
            font_family_css: raw.font_family_css(),
            font_size_px: raw.font_size_px(),
            text_color: raw.color("textColor", "#333"),
            line_color: raw.color("lineColor", "#333333"),
            error_bkg: raw.color("errorBkgColor", "#552222"),
            error_text: raw.color("errorTextColor", "#552222"),
        };

        Self { raw, common }
    }

    pub(super) fn common(&self) -> &CommonCssTheme {
        &self.common
    }

    pub(super) fn node_diagram(&self) -> NodeDiagramTheme {
        let node_border = self.raw.color("nodeBorder", "#9370DB");
        let main_bkg = self.raw.color("mainBkg", "#ECECFF");

        NodeDiagramTheme {
            common: self.common.clone(),
            node_text_color: self
                .raw
                .color("nodeTextColor", self.common.text_color.as_str()),
            title_color: self
                .raw
                .color("titleColor", self.common.text_color.as_str()),
            main_bkg,
            node_border,
            arrowhead_color: self
                .raw
                .color("arrowheadColor", self.common.line_color.as_str()),
            stroke_width: self.raw.css_value("strokeWidth", "1"),
            edge_label_background: self
                .raw
                .color("edgeLabelBackground", "rgba(232,232,232, 0.8)"),
            tertiary: self
                .raw
                .color("tertiaryColor", "hsl(80, 100%, 96.2745098039%)"),
            cluster_bkg: self.raw.color("clusterBkg", "#ffffde"),
            cluster_border: self.raw.color("clusterBorder", "#aaaa33"),
        }
    }

    pub(super) fn class_diagram(&self) -> ClassDiagramTheme {
        let class_text = self.raw.color(
            "classText",
            &self
                .raw
                .color("primaryTextColor", self.common.text_color.as_str()),
        );

        ClassDiagramTheme {
            common: self.common.clone(),
            class_text: class_text.clone(),
            note_text: self.raw.color("noteTextColor", "#333"),
            class_group_text: self
                .raw
                .optional_color("nodeBorder")
                .unwrap_or_else(|| class_text.clone()),
            title_color: self.raw.color("titleColor", "#333"),
            text_color: self.raw.color("textColor", class_text.as_str()),
            main_bkg: self.raw.color("mainBkg", "#ECECFF"),
            node_border: self.raw.color("nodeBorder", "#9370DB"),
            cluster_bkg: self.raw.color("clusterBkg", "#ffffde"),
            cluster_border: self.raw.color("clusterBorder", "#aaaa33"),
            stroke_width: self.raw.css_value("strokeWidth", "1"),
        }
    }

    pub(super) fn sequence_diagram(&self) -> SequenceDiagramTheme {
        let actor_border = self.raw.color("actorBorder", "#9370DB");
        let actor_fill = self.raw.color("actorBkg", "#ECECFF");
        let actor_text = self.raw.color("actorTextColor", "black");

        SequenceDiagramTheme {
            common: self.common.clone(),
            actor_border: actor_border.clone(),
            actor_fill: actor_fill.clone(),
            stroke_width: self.raw.css_value("strokeWidth", "1"),
            drop_shadow: self.raw.css_value("dropShadow", "none"),
            note_border: self.raw.color("noteBorderColor", "#aaaa33"),
            note_fill: self.raw.color("noteBkgColor", "#fff5ad"),
            actor_text: actor_text.clone(),
            actor_line: self.raw.color("actorLineColor", actor_border.as_str()),
            signal_color: self.raw.color("signalColor", "#333"),
            sequence_number: self.raw.color("sequenceNumberColor", "white"),
            signal_text: self.raw.color("signalTextColor", "#333"),
            label_box_border: self.raw.color("labelBoxBorderColor", actor_border.as_str()),
            label_box_fill: self.raw.color("labelBoxBkgColor", actor_fill.as_str()),
            label_text: self.raw.color("labelTextColor", actor_text.as_str()),
            loop_text: self.raw.color("loopTextColor", actor_text.as_str()),
            note_text: self.raw.color("noteTextColor", "black"),
            activation_fill: self.raw.color("activationBkgColor", "#f4f4f4"),
            activation_border: self.raw.color("activationBorderColor", "#666"),
            node_border: self.raw.color("nodeBorder", actor_border.as_str()),
            note_font_weight: self
                .raw
                .optional_value("noteFontWeight")
                .map(|font_weight| format!("font-weight:{};", font_weight))
                .unwrap_or_default(),
            label_box_filter: if self.common.look == "neo" {
                self.raw.css_value("dropShadow", "none")
            } else {
                "none".to_string()
            },
        }
    }

    pub(super) fn state_diagram(&self) -> StateDiagramTheme {
        let node_border = self.raw.color("nodeBorder", "#9370DB");
        let main_bkg = self.raw.color("mainBkg", "#ECECFF");
        let background = self.raw.color("background", "white");
        let stroke_width = self.raw.css_value("strokeWidth", "1");
        let stroke_width_px = if stroke_width.trim_end().ends_with("px") {
            stroke_width.clone()
        } else {
            format!("{stroke_width}px")
        };
        let stroke_width_value = stroke_width
            .trim()
            .trim_end_matches("px")
            .trim()
            .parse::<f64>()
            .unwrap_or(1.0)
            .max(0.0);
        let rough_stroke_width_value = if (stroke_width_value - 1.0).abs() <= 1e-9 {
            1.3
        } else {
            stroke_width_value
        };
        let transition_color = self
            .raw
            .color("transitionColor", self.common.line_color.as_str());
        let special_state_color = self
            .raw
            .color("specialStateColor", self.common.line_color.as_str());
        let inner_end_background = self.raw.color("innerEndBackground", node_border.as_str());
        let end_outer_fill = if special_state_color.eq_ignore_ascii_case("#333333") {
            "#ECECFF".to_string()
        } else {
            special_state_color.clone()
        };
        let end_outer_stroke = special_state_color.clone();
        let end_inner_stroke = if background.eq_ignore_ascii_case("white") {
            inner_end_background.clone()
        } else {
            background.clone()
        };

        StateDiagramTheme {
            common: self.common.clone(),
            transition_color,
            node_border: node_border.clone(),
            background: background.clone(),
            main_bkg: main_bkg.clone(),
            alt_background: self.raw.color("altBackground", "#efefef"),
            stroke_width,
            stroke_width_px,
            rough_stroke_width_value,
            note_border: self.raw.color("noteBorderColor", "#aaaa33"),
            note_bkg: self.raw.color("noteBkgColor", "#fff5ad"),
            note_text: self.raw.color("noteTextColor", "black"),
            label_background: self.raw.color("labelBackgroundColor", main_bkg.as_str()),
            edge_label_background: self
                .raw
                .color("edgeLabelBackground", "rgba(232,232,232, 0.8)"),
            transition_label_color: self
                .raw
                .optional_color("transitionLabelColor")
                .or_else(|| self.raw.optional_color("tertiaryTextColor"))
                .unwrap_or_else(|| self.common.text_color.clone()),
            special_state_color,
            inner_end_background,
            end_outer_fill,
            end_outer_stroke,
            end_inner_stroke,
            composite_background: self
                .raw
                .optional_color("compositeBackground")
                .unwrap_or_else(|| background.to_string()),
            state_bkg: self
                .raw
                .optional_color("stateBkg")
                .unwrap_or_else(|| main_bkg.clone()),
            state_border: self
                .raw
                .optional_color("stateBorder")
                .unwrap_or_else(|| node_border.clone()),
            composite_title_background: self
                .raw
                .color("compositeTitleBackground", main_bkg.as_str()),
            state_label_color: self.raw.color("stateLabelColor", "#131300"),
            drop_shadow: self
                .raw
                .optional_value("dropShadow")
                .unwrap_or_else(|| "none".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn presentation_theme_node_diagram_uses_shared_fallbacks() {
        let cfg = json!({});
        let theme = PresentationTheme::new(&cfg);
        let node = theme.node_diagram();

        assert_eq!(node.common.text_color, "#333");
        assert_eq!(node.common.line_color, "#333333");
        assert_eq!(node.node_text_color, "#333");
        assert_eq!(node.title_color, "#333");
        assert_eq!(node.main_bkg, "#ECECFF");
        assert_eq!(node.node_border, "#9370DB");
        assert_eq!(node.arrowhead_color, "#333333");
        assert_eq!(node.stroke_width, "1");
    }

    #[test]
    fn presentation_theme_sequence_neo_uses_drop_shadow_for_label_box_filter() {
        let cfg = json!({
            "look": "neo",
            "themeVariables": {
                "dropShadow": "drop-shadow(1px 2px 3px rgba(0,0,0,.4))"
            }
        });
        let theme = PresentationTheme::new(&cfg);

        let sequence = theme.sequence_diagram();
        assert_eq!(
            sequence.label_box_filter,
            "drop-shadow(1px 2px 3px rgba(0,0,0,.4))"
        );
    }
}
