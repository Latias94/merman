use crate::config::{config_css_number_or_string, config_diagram_look, config_f64, config_string};
use crate::svg::PreparedCommonCss;

#[derive(Debug)]
pub(crate) struct GitGraphNamedPaint {
    pub(crate) git: String,
    pub(crate) branch_label: String,
    pub(crate) inverse: String,
}

#[derive(Debug)]
pub(crate) struct GitGraphCssBinding {
    pub(crate) common: PreparedCommonCss,
    pub(crate) use_redux_geometry: bool,
    pub(crate) look_is_neo: bool,
    pub(crate) sources: GitGraphTerminalPaletteSources,
    pub(crate) theme_color_limit: usize,
    pub(crate) named: [GitGraphNamedPaint; 8],
    pub(crate) border_color_array: Vec<String>,
    pub(crate) gradient_start: Option<String>,
    pub(crate) gradient_stop: Option<String>,
    pub(crate) secondary_border: Option<String>,
    pub(crate) commit_line_color: Option<String>,
    pub(crate) stroke_width: String,
    pub(crate) note_font_weight: String,
    pub(crate) drop_shadow: String,
    pub(crate) commit_label_color: String,
    pub(crate) commit_label_background: String,
    pub(crate) tag_label_color: String,
    pub(crate) tag_label_background: String,
    pub(crate) tag_label_border: String,
    pub(crate) primary_color: String,
    pub(crate) main_bkg: String,
    pub(crate) node_border: String,
    pub(crate) primary_border: String,
    pub(crate) filter_color: String,
}

impl GitGraphCssBinding {
    pub(super) fn resolve(config: &serde_json::Value, font_family: &str, font_size: &str) -> Self {
        let optional = |key: &str| config_string(config, &["themeVariables", key]);
        let color =
            |key: &str, fallback: &str| optional(key).unwrap_or_else(|| fallback.to_owned());
        let css = |key: &str, fallback: &str| {
            config_css_number_or_string(config, &["themeVariables", key])
                .unwrap_or_else(|| fallback.to_owned())
        };
        let theme = config_string(config, &["theme"]).unwrap_or_else(|| "default".to_owned());
        let border_color_array: Vec<String> = config
            .pointer("/themeVariables/borderColorArray")
            .and_then(serde_json::Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        Self {
            common: PreparedCommonCss::with_resolved_typography(
                config,
                font_family.to_owned(),
                font_size.to_owned(),
            ),
            use_redux_geometry: super::gitgraph_theme_is_redux_geometry(&theme),
            look_is_neo: config_diagram_look(config).is_neo(),
            sources: GitGraphTerminalPaletteSources {
                use_color_theme: matches!(theme.as_str(), "redux-color" | "redux-dark-color"),
                use_neo_theme: matches!(theme.as_str(), "neo" | "neo-dark"),
                use_dark_theme: matches!(
                    theme.as_str(),
                    "dark" | "redux-dark" | "redux-dark-color" | "neo-dark"
                ),
                use_color_gen: super::gitgraph_theme_uses_color_gen(&theme),
                use_gradient: config
                    .pointer("/themeVariables/useGradient")
                    .and_then(crate::config::json_bool)
                    .unwrap_or(false),
                has_border_color_array: !border_color_array.is_empty(),
            },
            theme_color_limit: config_f64(config, &["themeVariables", "THEME_COLOR_LIMIT"])
                .map(|v| {
                    if v.is_nan() {
                        1
                    } else {
                        v.clamp(1.0, 64.0) as usize
                    }
                })
                .unwrap_or(12),
            named: std::array::from_fn(|i| GitGraphNamedPaint {
                git: color(&format!("git{i}"), default_git_color(i)),
                branch_label: color(&format!("gitBranchLabel{i}"), default_git_branch_label(i)),
                inverse: color(&format!("gitInv{i}"), default_git_inv(i)),
            }),
            border_color_array,
            gradient_start: optional("gradientStart"),
            gradient_stop: optional("gradientStop"),
            secondary_border: optional("secondaryBorderColor"),
            commit_line_color: optional("commitLineColor"),
            stroke_width: css("strokeWidth", "1"),
            note_font_weight: css("noteFontWeight", "normal"),
            drop_shadow: css("dropShadow", "none"),
            commit_label_color: color("commitLabelColor", "#000021"),
            commit_label_background: color("commitLabelBackground", "#ffffde"),
            tag_label_color: color("tagLabelColor", "#131300"),
            tag_label_background: color("tagLabelBackground", "#ECECFF"),
            tag_label_border: color("tagLabelBorder", "hsl(240, 60%, 86.2745098039%)"),
            primary_color: color("primaryColor", "#ECECFF"),
            main_bkg: color("mainBkg", "#ECECFF"),
            node_border: color("nodeBorder", "#9370DB"),
            primary_border: color("primaryBorderColor", "#9370DB"),
            filter_color: color("filterColor", "#000000"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GitGraphTerminalPaletteSources {
    pub(crate) use_color_theme: bool,
    pub(crate) use_neo_theme: bool,
    pub(crate) use_dark_theme: bool,
    pub(crate) use_color_gen: bool,
    pub(crate) use_gradient: bool,
    has_border_color_array: bool,
}

impl GitGraphTerminalPaletteSources {
    pub(crate) fn branch_slot(self, slot: usize) -> crate::gitgraph::GitGraphPaletteSource {
        if !self.use_color_gen {
            crate::gitgraph::GitGraphPaletteSource::Git(slot)
        } else if self.use_neo_theme {
            if slot == 0 {
                crate::gitgraph::GitGraphPaletteSource::NodeBorder
            } else {
                crate::gitgraph::GitGraphPaletteSource::Git(slot)
            }
        } else if !self.use_color_theme || slot == 0 || !self.has_border_color_array {
            crate::gitgraph::GitGraphPaletteSource::NodeBorder
        } else {
            crate::gitgraph::GitGraphPaletteSource::BorderColorArray
        }
    }

    pub(crate) fn state(self) -> crate::gitgraph::GitGraphPaletteSource {
        if self.use_color_gen {
            crate::gitgraph::GitGraphPaletteSource::MainBackground
        } else {
            crate::gitgraph::GitGraphPaletteSource::PrimaryColor
        }
    }

    pub(crate) fn branch_label_background(
        self,
        slot: usize,
    ) -> Option<crate::gitgraph::GitGraphPaletteSource> {
        if !self.use_color_gen {
            Some(crate::gitgraph::GitGraphPaletteSource::Git(slot))
        } else if self.use_neo_theme {
            self.use_gradient
                .then_some(crate::gitgraph::GitGraphPaletteSource::MainBackground)
        } else if !self.use_color_theme || slot == 0 || self.use_dark_theme {
            Some(crate::gitgraph::GitGraphPaletteSource::MainBackground)
        } else {
            Some(self.branch_slot(slot))
        }
    }
}

fn default_git_color(i: usize) -> &'static str {
    match i {
        0 => "hsl(240, 100%, 46.2745098039%)",
        1 => "hsl(60, 100%, 43.5294117647%)",
        2 => "hsl(80, 100%, 46.2745098039%)",
        3 => "hsl(210, 100%, 46.2745098039%)",
        4 => "hsl(180, 100%, 46.2745098039%)",
        5 => "hsl(150, 100%, 46.2745098039%)",
        6 => "hsl(300, 100%, 46.2745098039%)",
        _ => "hsl(0, 100%, 46.2745098039%)",
    }
}

fn default_git_branch_label(i: usize) -> &'static str {
    match i {
        0 | 3 => "#ffffff",
        _ => "black",
    }
}

fn default_git_inv(i: usize) -> &'static str {
    match i {
        0 => "hsl(60, 100%, 3.7254901961%)",
        1 => "rgb(0, 0, 160.5)",
        2 => "rgb(48.8333333334, 0, 146.5000000001)",
        3 => "rgb(146.5000000001, 73.2500000001, 0)",
        4 => "rgb(146.5000000001, 0, 0)",
        5 => "rgb(146.5000000001, 0, 73.2500000001)",
        6 => "rgb(0, 146.5000000001, 0)",
        _ => "rgb(0, 146.5000000001, 146.5000000001)",
    }
}
