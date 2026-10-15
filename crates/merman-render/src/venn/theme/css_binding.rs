use std::collections::{BTreeMap, HashMap};

use merman_core::diagrams::venn::VennDiagramRenderModel;
use merman_core::theme_color::{darken, is_dark, lighten};

use crate::config::config_string;
use crate::model::VennDiagramLayout;

#[derive(Debug)]
pub(crate) struct VennCirclePaintBinding {
    pub(crate) fill: String,
    pub(crate) text: String,
    pub(crate) fill_opacity: String,
    pub(crate) stroke: String,
    pub(crate) stroke_width: String,
}

#[derive(Debug)]
pub(crate) enum VennAreaPaintBinding {
    Circle(VennCirclePaintBinding),
    Intersection {
        fill: Option<String>,
        text: String,
        source_text_owned: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::diagrams::venn::{VennStyleEntryRenderModel, VennSubsetRenderModel};
    use serde_json::json;

    fn work_meter() -> crate::resources::OperationWorkMeter {
        crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::interactive(),
        )
    }

    fn model() -> VennDiagramRenderModel {
        VennDiagramRenderModel {
            subsets: vec![VennSubsetRenderModel {
                sets: vec!["A".into()],
                size: 1.0,
                label: None,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn source_color_wins_and_avoids_unnecessary_color_interpretation() {
        let mut model = model();
        model.style_entries.push(VennStyleEntryRenderModel {
            targets: vec!["A".into()],
            styles: [
                ("fill".into(), "var(--circle)".into()),
                ("color".into(), "currentColor".into()),
            ]
            .into(),
        });
        let layout = crate::venn::layout_venn_diagram_typed(
            &model,
            None,
            &json!({}),
            crate::resources::RenderResourcePolicy::interactive(),
        )
        .unwrap();
        let binding = VennCssBinding::resolve(
            &json!({}),
            &model,
            &layout,
            Some("#123456"),
            Some("transparent"),
            &work_meter(),
        )
        .unwrap();
        assert_eq!(binding.circle(0).fill, "var(--circle)");
        assert_eq!(binding.circle(0).text, "currentColor");
        assert_eq!(binding.title_color, "#123456");
        assert_eq!(binding.set_text_color, "transparent");
    }

    #[test]
    fn contrast_uses_background_and_invalid_color_still_errors() {
        let model = model();
        let layout = crate::venn::layout_venn_diagram_typed(
            &model,
            None,
            &json!({}),
            crate::resources::RenderResourcePolicy::interactive(),
        )
        .unwrap();
        let binding = VennCssBinding::resolve(
            &json!({"theme": "base", "themeVariables": {
                "background": "rebeccapurple", "venn1": "rebeccapurple"
            }}),
            &model,
            &layout,
            None,
            None,
            &work_meter(),
        )
        .unwrap();
        assert_eq!(binding.circle(0).text, "hsl(270, 50%, 70%)");
        assert!(matches!(
            VennCssBinding::resolve(
                &json!({"themeVariables": {
                    "background": "not-a-color"
                }}),
                &model,
                &layout,
                None,
                None,
                &work_meter(),
            ),
            Err(crate::Error::Color(_))
        ));
    }

    #[test]
    fn actual_area_styles_keep_scale_source_order_and_opaque_width() {
        let mut model = model();
        model.style_entries.push(VennStyleEntryRenderModel {
            targets: vec!["A".into()],
            styles: [("stroke-width".into(), "2px".into())].into(),
        });
        model.style_entries.push(VennStyleEntryRenderModel {
            targets: vec!["A".into()],
            styles: [
                ("stroke-width".into(), "calc(1px + 2px)".into()),
                ("fill-opacity".into(), "var(--opacity)".into()),
                ("stroke".into(), "currentColor".into()),
            ]
            .into(),
        });
        model.style_entries.push(VennStyleEntryRenderModel {
            targets: vec!["A".into(), "B".into()],
            styles: [("color".into(), "var(--label)".into())].into(),
        });
        let config = json!({"venn": {"width": 1200}, "handDrawnSeed": 17});
        let mut layout = crate::venn::layout_venn_diagram_typed(
            &model,
            None,
            &config,
            crate::resources::RenderResourcePolicy::interactive(),
        )
        .unwrap();
        let mut intersection = layout.areas[0].clone();
        intersection.sets = vec!["A".into(), "B".into()];
        layout.areas.insert(0, intersection);
        let binding =
            VennCssBinding::resolve(&config, &model, &layout, None, None, &work_meter()).unwrap();
        assert_eq!(binding.areas.len(), layout.areas.len());
        assert!(
            matches!(&binding.areas[0], VennAreaPaintBinding::Intersection {
            fill: None, text, source_text_owned: true,
        } if text == "var(--label)")
        );
        assert_eq!(binding.circle(0).stroke_width, "calc(1px + 2px)");
        assert_eq!(binding.circle(0).stroke, "currentColor");
        assert_eq!(binding.circle(0).fill_opacity, "var(--opacity)");
        assert_eq!(binding.hand_drawn_seed, Some(17.0));
        assert!(!binding.is_hand_drawn);

        model.style_entries.clear();
        let defaults =
            VennCssBinding::resolve(&config, &model, &layout, None, None, &work_meter()).unwrap();
        assert_eq!(defaults.circle(0).stroke_width, "3.75");
    }

    #[test]
    fn default_roles_and_palette_match_mermaid_defaults() {
        let model = model();
        let layout = crate::venn::layout_venn_diagram_typed(
            &model,
            None,
            &json!({}),
            crate::resources::RenderResourcePolicy::interactive(),
        )
        .unwrap();
        let binding = VennCssBinding::resolve(
            &json!({
                "themeVariables": {
                    "venn1": "#123456",
                    "primaryColor": "#987654",
                    "vennTitleTextColor": "#f43f5e",
                    "vennSetTextColor": "#22c55e"
                }
            }),
            &model,
            &layout,
            None,
            None,
            &work_meter(),
        )
        .unwrap();
        assert_eq!(binding.title_color, "#f43f5e");
        assert_eq!(binding.set_text_color, "#22c55e");
        assert_eq!(binding.circle(0).fill, "#123456");

        let defaults =
            VennCssBinding::resolve(&json!({}), &model, &layout, None, None, &work_meter())
                .unwrap();
        assert_eq!(defaults.title_color, "#333");
        assert_eq!(defaults.set_text_color, "#333");
        assert_eq!(defaults.circle(0).fill, "#ECECFF");

        let contrast = VennCssBinding::resolve(
            &json!({"themeVariables": {"primaryColor": "#abc"}}),
            &model,
            &layout,
            None,
            None,
            &work_meter(),
        )
        .unwrap();
        assert_eq!(contrast.circle(0).text, "hsl(210, 25%, 43.3333333333%)");

        let mut palette_layout = layout.clone();
        palette_layout
            .areas
            .extend([layout.areas[0].clone(), layout.areas[0].clone()]);
        let palette = VennCssBinding::resolve(
            &json!({"themeVariables": {
                "background": "#111111", "venn1": "#123456", "venn2": "#abcdef"
            }}),
            &model,
            &palette_layout,
            None,
            None,
            &work_meter(),
        )
        .unwrap();
        assert_eq!(
            palette
                .areas
                .iter()
                .filter_map(|area| match area {
                    VennAreaPaintBinding::Circle(circle) => Some(circle.fill.as_str()),
                    VennAreaPaintBinding::Intersection { .. } => None,
                })
                .collect::<Vec<_>>(),
            ["#123456", "#abcdef", "#123456"]
        );
        assert_eq!(
            palette.circle(0).text,
            "hsl(210, 65.3846153846%, 50.3921568627%)"
        );
    }
}

#[derive(Debug)]
pub(crate) struct VennCssBinding {
    pub(crate) title_color: String,
    pub(crate) set_text_color: String,
    pub(crate) areas: Box<[VennAreaPaintBinding]>,
    pub(crate) text_node_colors: HashMap<String, String>,
    pub(crate) is_hand_drawn: bool,
    pub(crate) hand_drawn_seed: Option<f64>,
}

impl VennCssBinding {
    #[cfg(test)]
    fn circle(&self, index: usize) -> &VennCirclePaintBinding {
        self.areas
            .iter()
            .filter_map(|area| match area {
                VennAreaPaintBinding::Circle(circle) => Some(circle),
                VennAreaPaintBinding::Intersection { .. } => None,
            })
            .nth(index)
            .unwrap()
    }

    pub(super) fn resolve(
        config: &serde_json::Value,
        model: &VennDiagramRenderModel,
        layout: &VennDiagramLayout,
        title_fill: Option<&str>,
        text_fill: Option<&str>,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let option = |key: &str| config_string(config, &["themeVariables", key]);
        let dark = is_dark(&option("background").unwrap_or_else(|| "#f4f4f4".into()))?;
        let palette = (1..=8)
            .filter_map(|index| option(&format!("venn{index}")))
            .collect::<Vec<_>>();
        let primary = option("primaryColor").unwrap_or_else(|| "#ECECFF".into());
        let mut styles = HashMap::<String, BTreeMap<String, String>>::new();
        for entry in &model.style_entries {
            work_meter.charge(1)?;
            styles
                .entry(entry.targets.join("|"))
                .or_default()
                .extend(entry.styles.clone());
        }
        let set_text_color = text_fill
            .map(str::to_owned)
            .or_else(|| option("vennSetTextColor"))
            .or_else(|| option("primaryTextColor"))
            .or_else(|| option("textColor"))
            .unwrap_or_else(|| "#333".into());
        let mut circle_index = 0usize;
        let areas = layout
            .areas
            .iter()
            .map(|area| {
                let source = styles.get(&area.sets.join("|"));
                let value = |key: &str| {
                    source
                        .and_then(|source| source.get(key))
                        .filter(|value| !value.trim().is_empty())
                        .cloned()
                };
                if area.sets.len() != 1 {
                    let source_text = value("color");
                    return Ok(VennAreaPaintBinding::Intersection {
                        fill: value("fill"),
                        source_text_owned: source_text.is_some(),
                        text: source_text.unwrap_or_else(|| set_text_color.clone()),
                    });
                }
                work_meter.charge(1)?;
                let fill = value("fill").unwrap_or_else(|| {
                    palette
                        .get(circle_index % palette.len().max(1))
                        .cloned()
                        .unwrap_or_else(|| primary.clone())
                });
                let text = match value("color") {
                    Some(text) => text,
                    None if dark => lighten(&fill, 30.0)?,
                    None => darken(&fill, 30.0)?,
                };
                circle_index += 1;
                Ok(VennAreaPaintBinding::Circle(VennCirclePaintBinding {
                    fill_opacity: value("fill-opacity").unwrap_or_else(|| "0.1".into()),
                    stroke: value("stroke").unwrap_or_else(|| fill.clone()),
                    stroke_width: value("stroke-width").unwrap_or_else(|| {
                        crate::number_format::canonical_number(5.0 * layout.scale).to_string()
                    }),
                    fill,
                    text,
                }))
            })
            .collect::<crate::Result<Vec<_>>>()?
            .into_boxed_slice();
        let text_node_colors = layout
            .text_nodes
            .iter()
            .filter_map(|node| {
                styles
                    .get(&node.id)
                    .and_then(|style| style.get("color"))
                    .filter(|color| !color.trim().is_empty())
                    .map(|color| (node.id.clone(), color.clone()))
            })
            .collect();
        Ok(Self {
            title_color: title_fill
                .map(str::to_owned)
                .or_else(|| option("vennTitleTextColor"))
                .or_else(|| option("titleColor"))
                .unwrap_or_else(|| "#333".into()),
            set_text_color,
            areas,
            text_node_colors,
            is_hand_drawn: crate::config::config_diagram_look(config).as_str() == "handDrawn",
            hand_drawn_seed: config
                .get("handDrawnSeed")
                .and_then(serde_json::Value::as_f64),
        })
    }
}
