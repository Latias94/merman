use std::collections::{BTreeMap, HashMap};

use merman_markup::theme_color::{darken, is_dark, lighten};
use merman_model::families::venn::VennDiagramModel;

use crate::config::config_string;
use crate::model::VennDiagramLayout;

#[derive(Debug)]
pub(crate) struct VennCirclePaintBinding {
    pub(crate) fill: String,
    pub(crate) text: String,
    pub(crate) fill_opacity: String,
    pub(crate) stroke: String,
    pub(crate) stroke_width: String,
    pub(super) source_fill_owned: bool,
    pub(super) source_stroke_owned: bool,
    pub(super) source_text_owned: bool,
    pub(super) fill_config_role: Option<usize>,
}

impl VennCirclePaintBinding {
    pub(super) fn apply_theme_fill(&mut self, fill: &str, dark: bool) -> crate::Result<()> {
        self.fill = fill.to_owned();
        // Mermaid derives these defaults from the final circle fill. Explicit source values
        // remain independent, and compiled stroke/label paints are applied afterward.
        if !self.source_stroke_owned {
            self.stroke.clone_from(&self.fill);
        }
        if !self.source_text_owned {
            self.text = circle_text_color(fill, dark)?;
        }
        Ok(())
    }
}

fn circle_text_color(fill: &str, dark: bool) -> crate::Result<String> {
    Ok(if dark {
        lighten(fill, 30.0)?
    } else {
        darken(fill, 30.0)?
    })
}

#[derive(Debug)]
pub(crate) enum VennAreaPaintBinding {
    Circle(VennCirclePaintBinding),
    Intersection {
        fill: Option<String>,
        text: Option<String>,
        source_text_owned: bool,
        config_text_role: Option<&'static str>,
        typed_text_fill: bool,
    },
}

struct VennColorDeclaration {
    css: String,
    inline: Option<String>,
    important: bool,
}

fn color_declaration(value: &str, svg_fill: bool) -> Option<VennColorDeclaration> {
    let property = if svg_fill { "fill" } else { "color" };
    let source = format!("{property}:{value}");
    let declaration = crate::mermaid_style::parse_style_declaration(&source)?;
    if !crate::mermaid_style::is_safe_browser_css_color_value(declaration.value())
        && !(svg_fill && declaration.value().eq_ignore_ascii_case("none"))
    {
        return None;
    }
    Some(VennColorDeclaration {
        css: declaration.source_value().to_owned(),
        // D3 uses setProperty with an empty priority; declaration syntax is not a value.
        inline: (!declaration.important() && !value.trim_end().ends_with(';'))
            .then(|| declaration.value().to_owned()),
        important: declaration.important(),
    })
}

fn intersection_text(
    config: &serde_json::Value,
    source: Option<&str>,
    stylesheet: Option<&VennColorDeclaration>,
    root_color: Option<&VennColorDeclaration>,
) -> (Option<String>, bool, Option<&'static str>) {
    // Nullish selection precedes CSS admission. A non-string value must not reveal an unused
    // primaryTextColor fallback: D3's rejected assignment retains venn.js's initial #444.
    let selected = ["vennSetTextColor", "primaryTextColor", "textColor"]
        .into_iter()
        .find_map(|key| {
            config
                .get("themeVariables")?
                .get(key)
                .filter(|value| !value.is_null())
                .map(|value| (key, value))
        });
    let raw = source.or_else(|| selected.and_then(|(_, value)| value.as_str()));
    let admitted = raw.and_then(|value| color_declaration(value, true));
    let inline = if raw == Some("") || (source.is_none() && selected.is_none()) {
        None
    } else {
        Some(
            admitted
                .as_ref()
                .and_then(|color| color.inline.clone())
                .unwrap_or_else(|| "#444".into()),
        )
    };
    if stylesheet.is_some_and(|color| color.important) {
        return (inline, false, Some("themeVariables.vennSetTextColor"));
    }
    if admitted
        .as_ref()
        .is_some_and(|color| color.inline.is_some())
    {
        let role = if source.is_some() {
            None
        } else {
            selected.map(|(key, _)| match key {
                "vennSetTextColor" => "themeVariables.vennSetTextColor",
                "primaryTextColor" => "themeVariables.primaryTextColor",
                _ => "themeVariables.textColor",
            })
        };
        return (inline, source.is_some(), role);
    }
    let role = inline.is_none().then(|| {
        if stylesheet.is_some() {
            Some("themeVariables.vennSetTextColor")
        } else {
            root_color.map(|_| "themeVariables.textColor")
        }
    });
    (inline, false, role.flatten())
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::families::venn::{VennStyleEntryModel, VennSubsetModel};
    use serde_json::json;

    fn work_meter() -> crate::resources::OperationWorkMeter {
        crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::interactive(),
        )
    }

    fn model() -> VennDiagramModel {
        VennDiagramModel {
            subsets: vec![VennSubsetModel {
                sets: vec!["A".into()],
                size: 1.0,
                label: None,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn color_admission_keeps_svg_none_and_declaration_priority_distinct() {
        assert!(color_declaration("none", true).is_some());
        assert!(color_declaration("none", false).is_none());
        let important = color_declaration("#123456 !important", true).unwrap();
        assert!(important.important);
        assert!(important.inline.is_none());
        for value in [
            "#123456; stroke:red",
            "url(javascript:alert(1))",
            "var(--color); color:red",
        ] {
            assert!(color_declaration(value, true).is_none(), "{value}");
            assert!(color_declaration(value, false).is_none(), "{value}");
        }
    }

    #[test]
    fn only_a_nullish_role_selects_the_primary_intersection_fallback() {
        for (value, fill, role) in [
            (json!(false), Some("#444"), None),
            (json!(7), Some("#444"), None),
            (json!(" "), Some("#444"), None),
            (json!(""), None, None),
            (
                json!(null),
                Some("#abcdef"),
                Some("themeVariables.primaryTextColor"),
            ),
        ] {
            let config = json!({"themeVariables": {
                "vennSetTextColor": value, "primaryTextColor": "#abcdef"
            }});
            let (actual_fill, source_owned, actual_role) =
                intersection_text(&config, None, None, None);
            assert_eq!(actual_fill.as_deref(), fill, "{value}");
            assert_eq!(actual_role, role, "{value}");
            assert!(!source_owned);
        }
    }

    #[test]
    fn source_color_wins_and_avoids_unnecessary_color_interpretation() {
        let mut model = model();
        model.style_entries.push(VennStyleEntryModel {
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
            &json!({"themeVariables": {"vennSetTextColor": "transparent"}}),
            &model,
            &layout,
            Some("#123456"),
            &work_meter(),
        )
        .unwrap();
        assert_eq!(binding.circle(0).fill, "var(--circle)");
        assert_eq!(binding.circle(0).text, "currentColor");
        assert_eq!(binding.title_color, "#123456");
        assert_eq!(binding.set_text_color.as_deref(), Some("transparent"));
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
                &work_meter(),
            ),
            Err(crate::Error::Color(_))
        ));
    }

    #[test]
    fn actual_area_styles_keep_scale_source_order_and_opaque_width() {
        let mut model = model();
        model.style_entries.push(VennStyleEntryModel {
            targets: vec!["A".into()],
            styles: [("stroke-width".into(), "2px".into())].into(),
        });
        model.style_entries.push(VennStyleEntryModel {
            targets: vec!["A".into()],
            styles: [
                ("stroke-width".into(), "calc(1px + 2px)".into()),
                ("fill-opacity".into(), "var(--opacity)".into()),
                ("stroke".into(), "currentColor".into()),
            ]
            .into(),
        });
        model.style_entries.push(VennStyleEntryModel {
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
            VennCssBinding::resolve(&config, &model, &layout, None, &work_meter()).unwrap();
        assert_eq!(binding.areas.len(), layout.areas.len());
        assert!(
            matches!(&binding.areas[0], VennAreaPaintBinding::Intersection {
            fill: None, text, source_text_owned: true, ..
        } if text.as_deref() == Some("var(--label)"))
        );
        assert_eq!(binding.circle(0).stroke_width, "calc(1px + 2px)");
        assert_eq!(binding.circle(0).stroke, "currentColor");
        assert_eq!(binding.circle(0).fill_opacity, "var(--opacity)");
        assert_eq!(binding.hand_drawn_seed, Some(17.0));
        assert!(!binding.is_hand_drawn);

        model.style_entries.clear();
        let defaults =
            VennCssBinding::resolve(&config, &model, &layout, None, &work_meter()).unwrap();
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
            &work_meter(),
        )
        .unwrap();
        assert_eq!(binding.title_color, "#f43f5e");
        assert_eq!(binding.set_text_color.as_deref(), Some("#22c55e"));
        assert_eq!(binding.circle(0).fill, "#123456");

        let defaults =
            VennCssBinding::resolve(&json!({}), &model, &layout, None, &work_meter()).unwrap();
        assert_eq!(defaults.title_color, "#333");
        assert_eq!(defaults.set_text_color, None);
        assert_eq!(defaults.circle(0).fill, "#ECECFF");

        let contrast = VennCssBinding::resolve(
            &json!({"themeVariables": {"primaryColor": "#abc"}}),
            &model,
            &layout,
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
    pub(crate) set_text_color: Option<String>,
    pub(crate) text_node_color: Option<String>,
    pub(crate) inherited_text_fill: Option<String>,
    pub(super) text_node_config_role: Option<&'static str>,
    pub(crate) typed_text_node_fill: bool,
    pub(crate) areas: Box<[VennAreaPaintBinding]>,
    pub(crate) text_node_colors: HashMap<String, String>,
    pub(crate) is_hand_drawn: bool,
    pub(crate) hand_drawn_seed: Option<f64>,
    pub(super) dark_background: bool,
}

impl VennCssBinding {
    pub(super) fn title_color_source(config: &serde_json::Value) -> Option<(&'static str, String)> {
        config_string(config, &["themeVariables", "vennTitleTextColor"])
            .filter(|color| !color.is_empty())
            .map(|color| ("themeVariables.vennTitleTextColor", color))
            .or_else(|| {
                config_string(config, &["themeVariables", "titleColor"])
                    .map(|color| ("themeVariables.titleColor", color))
            })
    }

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
        model: &VennDiagramModel,
        layout: &VennDiagramLayout,
        title_fill: Option<&str>,
        work_meter: &crate::resources::OperationWorkMeter,
    ) -> crate::Result<Self> {
        let option = |key: &str| config_string(config, &["themeVariables", key]);
        let dark = is_dark(&option("background").unwrap_or_else(|| "#f4f4f4".into()))?;
        let palette = (1..=8)
            .filter_map(|index| {
                option(&format!("venn{index}"))
                    .filter(|color| !color.is_empty())
                    .map(|color| (index, color))
            })
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
        let local_text = option("vennSetTextColor");
        let set_text_color = local_text
            .as_deref()
            .and_then(|color| color_declaration(color, true));
        let text_node_color = local_text
            .as_deref()
            .and_then(|color| color_declaration(color, false));
        let root_color = option("textColor")
            .as_deref()
            .and_then(|color| color_declaration(color, true));
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
                    let source_text = source.and_then(|style| style.get("color"));
                    let (text, source_text_owned, config_text_role) = intersection_text(
                        config,
                        source_text.map(String::as_str),
                        set_text_color.as_ref(),
                        root_color.as_ref(),
                    );
                    return Ok(VennAreaPaintBinding::Intersection {
                        fill: value("fill"),
                        source_text_owned,
                        config_text_role,
                        text,
                        typed_text_fill: false,
                    });
                }
                work_meter.charge(1)?;
                let fill = value("fill").unwrap_or_else(|| {
                    palette
                        .get(circle_index % palette.len().max(1))
                        .map(|(_, color)| color.clone())
                        .unwrap_or_else(|| primary.clone())
                });
                let text = match value("color") {
                    Some(text) => text,
                    None => circle_text_color(&fill, dark)?,
                };
                let fill_config_role = palette
                    .get(circle_index % palette.len().max(1))
                    .map(|(index, _)| *index);
                circle_index += 1;
                Ok(VennAreaPaintBinding::Circle(VennCirclePaintBinding {
                    source_fill_owned: value("fill").is_some(),
                    source_stroke_owned: value("stroke").is_some(),
                    source_text_owned: value("color").is_some(),
                    fill_config_role,
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
                    .and_then(|color| color_declaration(color, false))
                    .and_then(|color| color.inline)
                    .map(|color| (node.id.clone(), color))
            })
            .collect();
        let needs_inherited_fill = set_text_color.is_none()
            || areas.iter().any(|area| match area {
                VennAreaPaintBinding::Intersection { text, .. } => text
                    .as_deref()
                    .is_none_or(|color| !crate::mermaid_style::is_supported_css_color_value(color)),
                VennAreaPaintBinding::Circle(_) => false,
            });
        Ok(Self {
            title_color: title_fill
                .map(str::to_owned)
                .or_else(|| Self::title_color_source(config).map(|(_, color)| color))
                .unwrap_or_else(|| "#333".into()),
            set_text_color: set_text_color.map(|color| color.css),
            inherited_text_fill: needs_inherited_fill
                .then(|| root_color.map(|color| color.css))
                .flatten(),
            text_node_config_role: text_node_color
                .as_ref()
                .map(|_| "themeVariables.vennSetTextColor"),
            text_node_color: text_node_color.map(|color| color.css),
            typed_text_node_fill: false,
            areas,
            text_node_colors,
            is_hand_drawn: crate::config::config_diagram_look(config).as_str() == "handDrawn",
            hand_drawn_seed: config
                .get("handDrawnSeed")
                .and_then(serde_json::Value::as_f64),
            dark_background: dark,
        })
    }
}
