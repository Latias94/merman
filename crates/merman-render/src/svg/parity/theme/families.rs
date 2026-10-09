use super::*;

impl<'a> MermaidThemeAdapter<'a> {
    pub(in crate::svg::parity) fn common(&self) -> &CommonCssTheme {
        &self.common
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
