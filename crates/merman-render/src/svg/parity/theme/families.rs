use super::*;
use merman_core::theme_color::invert;

impl<'a> MermaidThemeAdapter<'a> {
    #[cfg(feature = "diagram-treemap")]
    pub(crate) fn treemap(&self) -> crate::Result<TreemapTheme> {
        let text_color = self.raw.color("textColor", "#333");
        let title_color = self
            .raw
            .optional_root_scoped_string("treemap", "titleColor")
            .or_else(|| self.raw.optional_color("titleColor"))
            .unwrap_or_else(|| text_color.clone());
        let raw_theme_name = self.common.theme_name.clone();
        let default_theme = raw_theme_name == "default";
        let theme_name = raw_theme_name.trim().to_ascii_lowercase();
        let label_text_color = self.raw.color("labelTextColor", "black");
        let label_text_is_calculated = label_text_color.trim() == "calculated";
        let scale_label_color = self.raw.color("scaleLabelColor", &label_text_color);
        let neutral_special_label_color = self.raw.color("cScale1", default_c_scale(1));

        let color_scale = (0..12)
            .map(|i| {
                if default_theme {
                    default_c_scale(i).to_string()
                } else {
                    self.raw.color(&format!("cScale{i}"), default_c_scale(i))
                }
            })
            .collect();
        let color_scale_peer = (0..12)
            .map(|i| {
                if default_theme {
                    default_c_scale_peer(i).to_string()
                } else {
                    self.raw
                        .color(&format!("cScalePeer{i}"), default_c_scale_peer(i))
                }
            })
            .collect();
        let color_scale_label = (0..12)
            .map(|i| -> crate::Result<String> {
                if let Some(color) = self.raw.optional_color(&format!("cScaleLabel{i}")) {
                    return Ok(color);
                }
                Ok(match theme_name.as_str() {
                    "dark" | "forest" => scale_label_color.clone(),
                    "neutral" => {
                        if i == 0 || i == 2 {
                            neutral_special_label_color.clone()
                        } else {
                            scale_label_color.clone()
                        }
                    }
                    _ => {
                        if label_text_is_calculated {
                            scale_label_color.clone()
                        } else if i == 0 || i == 3 {
                            invert(&label_text_color)?
                        } else {
                            label_text_color.clone()
                        }
                    }
                })
            })
            .collect::<crate::Result<Vec<_>>>()?;

        Ok(TreemapTheme {
            title_color,
            label_color: self
                .raw
                .optional_root_scoped_string("treemap", "labelColor")
                .unwrap_or_else(|| text_color.clone()),
            value_color: self
                .raw
                .optional_root_scoped_string("treemap", "valueColor")
                .unwrap_or_else(|| text_color.clone()),
            section_stroke_color: self.treemap_style_option("sectionStrokeColor", "black"),
            section_stroke_width: self.treemap_style_option("sectionStrokeWidth", "1"),
            section_fill_color: self.treemap_style_option("sectionFillColor", "#efefef"),
            leaf_stroke_color: self.treemap_style_option("leafStrokeColor", "black"),
            leaf_stroke_width: self.treemap_style_option("leafStrokeWidth", "1"),
            leaf_fill_color: self.treemap_style_option("leafFillColor", "#efefef"),
            label_font_size: self.treemap_style_option("labelFontSize", "12px"),
            value_font_size: self.treemap_style_option("valueFontSize", "10px"),
            title_font_size: self.treemap_style_option("titleFontSize", "14px"),
            color_scale,
            color_scale_peer,
            color_scale_label,
            text_color,
        })
    }

    #[cfg(feature = "diagram-event-modeling")]
    pub(crate) fn eventmodeling(&self) -> EventModelingTheme {
        EventModelingTheme {
            text_color: self.raw.color("textColor", "#333"),
            ui_fill: self.raw.color("emUiFill", "white"),
            ui_stroke: self.raw.color("emUiStroke", "#dbdada"),
            processor_fill: self.raw.color("emProcessorFill", "#edb3f6"),
            processor_stroke: self.raw.color("emProcessorStroke", "#b88cbf"),
            read_model_fill: self.raw.color("emReadModelFill", "#d3f1a2"),
            read_model_stroke: self.raw.color("emReadModelStroke", "#a3b732"),
            command_fill: self.raw.color("emCommandFill", "#bcd6fe"),
            command_stroke: self.raw.color("emCommandStroke", "#679ac3"),
            event_fill: self.raw.color("emEventFill", "#ffb778"),
            event_stroke: self.raw.color("emEventStroke", "#c19a0f"),
            swimlane_background_fill: self
                .raw
                .optional_color("emSwimlaneBackgroundOdd")
                .or_else(|| self.raw.optional_color("emSwimlaneBackground"))
                .unwrap_or_else(|| "rgb(250,250,250)".to_string()),
            swimlane_background_stroke: self
                .raw
                .optional_color("emSwimlaneBackgroundStroke")
                .or_else(|| self.raw.optional_color("emSwimlaneBorder"))
                .unwrap_or_else(|| "rgb(240,240,240)".to_string()),
            relation_stroke: self.raw.color("emRelationStroke", "#000"),
            arrowhead_fill: self.raw.color("emArrowhead", "#000000"),
        }
    }

    pub(in crate::svg::parity) fn common(&self) -> &CommonCssTheme {
        &self.common
    }

    #[cfg(feature = "diagram-treemap")]
    fn treemap_style_option(&self, key: &str, default_value: &str) -> String {
        self.raw
            .optional_root_scoped_css_value("treemap", key)
            .unwrap_or_else(|| default_value.to_string())
    }

    #[cfg(any(
        feature = "diagram-agentflow",
        feature = "diagram-mindmap",
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-block"
    ))]
    pub(in crate::svg::parity) fn node_diagram(&self) -> NodeDiagramTheme {
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

    #[cfg(feature = "diagram-sequence")]
    pub(in crate::svg::parity) fn sequence_diagram(&self) -> SequenceDiagramTheme {
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
            label_box_filter: if self.common.is_neo() {
                self.raw.css_value("dropShadow", "none")
            } else {
                "none".to_string()
            },
        }
    }

    #[cfg(feature = "diagram-state")]
    pub(in crate::svg::parity) fn state_diagram(&self) -> StateDiagramTheme {
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
