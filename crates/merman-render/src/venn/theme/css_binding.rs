use std::collections::{BTreeMap, HashMap};

use merman_core::diagrams::venn::VennDiagramRenderModel;
use merman_core::theme_color::{darken, is_dark, lighten};

use crate::config::config_string;
use crate::model::VennDiagramLayout;

#[derive(Debug)]
pub(crate) struct VennCirclePaintBinding {
    pub(crate) fill: String,
    pub(crate) text: String,
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
        assert_eq!(binding.circles[0].fill, "var(--circle)");
        assert_eq!(binding.circles[0].text, "currentColor");
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
        assert_eq!(binding.circles[0].text, "hsl(270, 50%, 70%)");
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
        assert_eq!(binding.circles[0].fill, "#123456");

        let defaults =
            VennCssBinding::resolve(&json!({}), &model, &layout, None, None, &work_meter())
                .unwrap();
        assert_eq!(defaults.title_color, "#333");
        assert_eq!(defaults.set_text_color, "#333");
        assert_eq!(defaults.circles[0].fill, "#ECECFF");

        let contrast = VennCssBinding::resolve(
            &json!({"themeVariables": {"primaryColor": "#abc"}}),
            &model,
            &layout,
            None,
            None,
            &work_meter(),
        )
        .unwrap();
        assert_eq!(contrast.circles[0].text, "hsl(210, 25%, 43.3333333333%)");

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
                .circles
                .iter()
                .map(|circle| circle.fill.as_str())
                .collect::<Vec<_>>(),
            ["#123456", "#abcdef", "#123456"]
        );
        assert_eq!(
            palette.circles[0].text,
            "hsl(210, 65.3846153846%, 50.3921568627%)"
        );
    }
}

#[derive(Debug)]
pub(crate) struct VennCssBinding {
    pub(crate) title_color: String,
    pub(crate) set_text_color: String,
    pub(crate) circles: Box<[VennCirclePaintBinding]>,
    pub(crate) source_styles: HashMap<String, BTreeMap<String, String>>,
}

impl VennCssBinding {
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
        let circles = layout
            .areas
            .iter()
            .filter(|area| area.sets.len() == 1)
            .enumerate()
            .map(|(index, area)| {
                work_meter.charge(1)?;
                let source = styles.get(&area.sets.join("|"));
                let value = |key: &str| {
                    source
                        .and_then(|source| source.get(key))
                        .filter(|value| !value.trim().is_empty())
                        .cloned()
                };
                let fill = value("fill").unwrap_or_else(|| {
                    palette
                        .get(index % palette.len().max(1))
                        .cloned()
                        .unwrap_or_else(|| primary.clone())
                });
                let text = match value("color") {
                    Some(text) => text,
                    None if dark => lighten(&fill, 30.0)?,
                    None => darken(&fill, 30.0)?,
                };
                Ok(VennCirclePaintBinding { fill, text })
            })
            .collect::<crate::Result<Vec<_>>>()?
            .into_boxed_slice();
        Ok(Self {
            title_color: title_fill
                .map(str::to_owned)
                .or_else(|| option("vennTitleTextColor"))
                .or_else(|| option("titleColor"))
                .unwrap_or_else(|| "#333".into()),
            set_text_color: text_fill
                .map(str::to_owned)
                .or_else(|| option("vennSetTextColor"))
                .or_else(|| option("primaryTextColor"))
                .or_else(|| option("textColor"))
                .unwrap_or_else(|| "#333".into()),
            circles,
            source_styles: styles,
        })
    }
}
