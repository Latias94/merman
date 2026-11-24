use crate::config::config_string;

#[derive(Debug)]
pub(crate) struct JourneyCssBinding {
    pub(crate) text_color: String,
    pub(crate) face_color: String,
    pub(crate) main_bkg: String,
    pub(crate) node_border: String,
    pub(crate) arrowhead_color: String,
    pub(crate) edge_label_background: String,
    pub(crate) title_color: String,
    pub(crate) tertiary_color: String,
    pub(crate) border2: String,
    pub(crate) fill_types: Vec<String>,
    pub(crate) actor_colors: Vec<Option<String>>,
}

impl JourneyCssBinding {
    pub(super) fn new(config: &serde_json::Value) -> Self {
        let color = |key: &str, fallback: &str| {
            config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.to_owned())
        };
        let text_color = color("textColor", "#333");
        let title_color = color("titleColor", &text_color);
        Self {
            text_color,
            face_color: color("faceColor", "#FFF8DC"),
            main_bkg: color("mainBkg", "#ECECFF"),
            node_border: color("nodeBorder", "#9370DB"),
            arrowhead_color: color("arrowheadColor", "#333333"),
            edge_label_background: color("edgeLabelBackground", "rgba(232,232,232, 0.8)"),
            title_color,
            tertiary_color: color("tertiaryColor", "hsl(80, 100%, 96.2745098039%)"),
            border2: color("border2", "#aaaa33"),
            fill_types: (0..8)
                .map(|index| color(&format!("fillType{index}"), default_fill_type(index)))
                .collect(),
            actor_colors: (0..6)
                .map(|index| config_string(config, &["themeVariables", &format!("actor{index}")]))
                .collect(),
        }
    }
}

fn default_fill_type(index: usize) -> &'static str {
    match index {
        0 => "#ECECFF",
        1 => "#ffffde",
        2 => "hsl(304, 100%, 96.2745098039%)",
        3 => "hsl(124, 100%, 93.5294117647%)",
        4 => "hsl(176, 100%, 96.2745098039%)",
        5 => "hsl(-4, 100%, 93.5294117647%)",
        6 => "hsl(8, 100%, 96.2745098039%)",
        _ => "hsl(188, 100%, 93.5294117647%)",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_binding_preserves_mermaid_stylesheet_colors() {
        let config = serde_json::json!({
            "themeVariables": {
                "fillType0": "var(--css-section-color)",
                "actor0": "var(--css-actor-color)",
                "textColor": "var(--base-text-color)"
            }
        });
        let binding = JourneyCssBinding::new(&config);
        assert_eq!(binding.fill_types[0], "var(--css-section-color)");
        assert_eq!(
            binding.actor_colors[0].as_deref(),
            Some("var(--css-actor-color)")
        );
        assert_eq!(binding.actor_colors[1], None);
        assert_eq!(binding.text_color, "var(--base-text-color)");
        assert_eq!(binding.title_color, "var(--base-text-color)");
    }
}
