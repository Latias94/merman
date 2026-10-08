use super::super::*;
use crate::gitgraph::GITGRAPH_PALETTE_SLOT_COUNT;
use std::borrow::Cow;

struct GitGraphCss {
    css: String,
    defs: String,
    font_family_css: String,
    base_font_size_css: String,
    base_typography_emitted: bool,
    commit_label_background_fill: String,
    branch_stroke: String,
    text_colors: Option<[String; 5]>,
    // These facts are set alongside the exact CSS declarations that mask root inheritance.
    root_geometry: [[bool; 8]; 4],
    root_cherry_pick: bool,
    node_paint: Option<GitGraphNodePaintCss>,
}

use crate::gitgraph::GitGraphNodePaintCss;

impl GitGraphCss {
    fn typography_emission(&self) -> crate::gitgraph::GitGraphTypographyCssEmission<'_> {
        crate::gitgraph::GitGraphTypographyCssEmission {
            base_font_family_css: &self.font_family_css,
            branch_label_font_family_css: &self.font_family_css,
            base_font_size_css: &self.base_font_size_css,
            base_css_emitted: self.base_typography_emitted,
        }
    }

    fn commit_label_background_fill(&self) -> &str {
        &self.commit_label_background_fill
    }
}

enum GitGraphBranchLabelStyle<'a> {
    ScopedDropShadow(SvgDiagramId<'a>),
    Configured(String),
    None,
}

impl std::fmt::Display for GitGraphBranchLabelStyle<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ScopedDropShadow(diagram_id) => {
                write!(formatter, "filter:url(#{diagram_id}-drop-shadow)")
            }
            Self::Configured(filter) => {
                write!(formatter, "filter:{}", escape_attr_display(filter))
            }
            Self::None => Ok(()),
        }
    }
}

const GITGRAPH_NAMED_COLOR_COUNT: usize = 8;

fn gitgraph_theme_name(effective_config: &serde_json::Value) -> String {
    config_string(effective_config, &["theme"]).unwrap_or_else(|| "default".to_string())
}

fn gitgraph_theme_is_color(theme: &str) -> bool {
    matches!(theme, "redux-color" | "redux-dark-color")
}

fn gitgraph_theme_is_neo(theme: &str) -> bool {
    matches!(theme, "neo" | "neo-dark")
}

fn gitgraph_theme_is_dark(theme: &str) -> bool {
    matches!(
        theme,
        "dark" | "redux-dark" | "redux-dark-color" | "neo-dark"
    )
}

fn gitgraph_theme_array(effective_config: &serde_json::Value, key: &str) -> Vec<String> {
    effective_config
        .get("themeVariables")
        .and_then(|v| v.get(key))
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn gitgraph_defs<I>(
    diagram_id: I,
    effective_config: &serde_json::Value,
    primary_border: Option<&str>,
) -> String
where
    I: Copy + std::fmt::Display,
{
    let mut out = String::new();

    if config_bool(effective_config, &["themeVariables", "useGradient"]).unwrap_or(false) {
        let gradient_start = config_string(effective_config, &["themeVariables", "gradientStart"])
            .or_else(|| primary_border.map(str::to_owned))
            .or_else(|| config_string(effective_config, &["themeVariables", "primaryBorderColor"]))
            .unwrap_or_else(|| "#9370DB".to_string());
        let gradient_stop = config_string(effective_config, &["themeVariables", "gradientStop"])
            .or_else(|| {
                config_string(
                    effective_config,
                    &["themeVariables", "secondaryBorderColor"],
                )
            })
            .unwrap_or_else(|| gradient_start.clone());

        let _ = write!(
            &mut out,
            r#"<defs><linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient></defs>"#,
            diagram_id,
            escape_xml(&gradient_start),
            escape_xml(&gradient_stop)
        );
    }

    let theme_name = gitgraph_theme_name(effective_config);
    if config_diagram_look(effective_config).is_neo()
        && crate::gitgraph::gitgraph_theme_is_redux_geometry(&theme_name)
    {
        let filter_color = theme_token(effective_config, "filterColor", "#000000");
        let _ = write!(
            &mut out,
            r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
            diagram_id,
            escape_xml(&filter_color)
        );
    }

    out
}

fn gitgraph_css(
    diagram_id: &str,
    effective_config: &serde_json::Value,
    static_paint: &crate::gitgraph::GitGraphStaticPaintPlan,
    typography_theme: &crate::gitgraph::GitGraphTypographyThemePlan,
    text_colors: Option<[Option<&str>; 3]>,
    branch_stroke: Option<&str>,
) -> GitGraphCss {
    let id = crate::svg::escape_css_identifier(diagram_id);
    let fragment_id = escape_xml(diagram_id);
    let node_paint_values = static_paint
        .node_paint()
        .map(|plan| plan.css_values())
        .unwrap_or([None; 6]);
    let css = super::super::css::InfoCssWriter::with_resolved_typography(
        effective_config,
        typography_theme.font_family_css(),
        typography_theme.font_size_css(),
    );
    let css = match text_colors.and_then(|colors| colors[0]) {
        Some(color) => css.with_text_color(color),
        None => css,
    };
    let css = match node_paint_values[4] {
        Some(border) => css.with_node_border(border),
        None => css,
    };
    let parts = css.into_parts(diagram_id);
    let theme_name = gitgraph_theme_name(effective_config);
    let use_redux_geometry = crate::gitgraph::gitgraph_theme_is_redux_geometry(&theme_name);
    let use_color_theme = gitgraph_theme_is_color(&theme_name);
    let use_neo_theme = gitgraph_theme_is_neo(&theme_name);
    let use_dark_theme = gitgraph_theme_is_dark(&theme_name);
    let use_color_gen = crate::gitgraph::gitgraph_theme_uses_color_gen(&theme_name);

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

    let commit_label_font_size = typography_theme.commit_label_font_size_css();
    let tag_label_font_size = typography_theme.tag_label_font_size_css();
    let commit_label_color = text_colors
        .and_then(|colors| colors[2])
        .map(str::to_owned)
        .unwrap_or_else(|| theme_token(effective_config, "commitLabelColor", "#000021"));
    let commit_label_background = static_paint
        .commit_label_background_css()
        .map(str::to_owned)
        .unwrap_or_else(|| theme_token(effective_config, "commitLabelBackground", "#ffffde"));
    let tag_label_color = text_colors
        .and_then(|colors| colors[1])
        .map(str::to_owned)
        .unwrap_or_else(|| theme_token(effective_config, "tagLabelColor", "#131300"));
    let tag_label_background = node_paint_values[2].map_or_else(
        || {
            Cow::Owned(theme_token(
                effective_config,
                "tagLabelBackground",
                "#ECECFF",
            ))
        },
        Cow::Borrowed,
    );
    let tag_label_border = node_paint_values[5].map(str::to_owned).unwrap_or_else(|| {
        theme_token(
            effective_config,
            "tagLabelBorder",
            "hsl(240, 60%, 86.2745098039%)",
        )
    });
    let theme_color_limit = config_f64(effective_config, &["themeVariables", "THEME_COLOR_LIMIT"])
        .map(|value| {
            let value = if value.is_nan() {
                1.0
            } else {
                value.clamp(1.0, 64.0)
            };
            value as usize
        })
        .unwrap_or(12);
    let stroke_width = crate::config::config_css_number_or_string(
        effective_config,
        &["themeVariables", "strokeWidth"],
    )
    .unwrap_or_else(|| "1".to_string());
    let commit_line_color = branch_stroke
        .map(str::to_owned)
        .or_else(|| config_string(effective_config, &["themeVariables", "commitLineColor"]))
        .unwrap_or_else(|| parts.line_color.clone());
    let primary_color = node_paint_values[0].map_or_else(
        || Cow::Owned(theme_token(effective_config, "primaryColor", "#ECECFF")),
        Cow::Borrowed,
    );
    let node_border = node_paint_values[4]
        .map(str::to_owned)
        .unwrap_or_else(|| theme_token(effective_config, "nodeBorder", "#9370DB"));
    let main_bkg = node_paint_values[1].map_or_else(
        || Cow::Owned(theme_token(effective_config, "mainBkg", "#ECECFF")),
        Cow::Borrowed,
    );
    let note_font_weight = crate::config::config_css_number_or_string(
        effective_config,
        &["themeVariables", "noteFontWeight"],
    )
    .unwrap_or_else(|| "normal".to_string());
    let note_font_weight_decl = if use_redux_geometry {
        format!("font-weight:{};", note_font_weight)
    } else {
        String::new()
    };
    let drop_shadow = crate::config::config_css_number_or_string(
        effective_config,
        &["themeVariables", "dropShadow"],
    )
    .unwrap_or_else(|| "none".to_string());
    let use_gradient =
        config_bool(effective_config, &["themeVariables", "useGradient"]).unwrap_or(false);
    let border_color_array = gitgraph_theme_array(effective_config, "borderColorArray");
    // gitGraph owns its draw path instead of using rendering-util/render.ts, so it must append the
    // configured root gradient itself for every theme. Several classic themes enable gradients.
    let defs = gitgraph_defs(diagram_id, effective_config, node_paint_values[3]);
    let mut out = parts.css_prefix;
    let _ = write!(
        &mut out,
        r#"#{} .commit-id,#{} .commit-msg,#{} .branch-label{{fill:lightgrey;color:lightgrey;font-family:'trebuchet ms',verdana,arial,sans-serif;font-family:{};}}"#,
        id, id, id, parts.font_family
    );
    let mut root_geometry = [[true; 8]; 4];
    let mut root_cherry_pick = true;
    let mut node_paint = static_paint
        .node_paint()
        .map(|_| GitGraphNodePaintCss::default());
    for i in 0..theme_color_limit {
        if i < 8 {
            root_geometry[3][i] = false;
        }
        let ci = i % GITGRAPH_NAMED_COLOR_COUNT;
        if use_color_gen {
            // .commit-bullets supplies the inherited fill even for slots beyond THEME_COLOR_LIMIT.
            root_geometry[1] = [false; 8];
            root_geometry[2] = [false; 8];
            root_cherry_pick = false;
            if i < 8 && (!use_neo_theme || use_gradient) {
                root_geometry[0][i] = false;
            }
            if use_neo_theme {
                if i == 0 {
                    let _ = write!(
                        &mut out,
                        r#"#{} .branch-label{}{{fill:{};}}#{} .commit{}{{stroke:{};}}#{} .commit-highlight{}{{stroke:{};fill:{};}}#{} .arrow{}{{stroke:{};}}#{} .commit-bullets{{fill:{};}}#{} .commit-cherry-pick{}{{stroke:{};}}"#,
                        id,
                        i,
                        node_border,
                        id,
                        i,
                        node_border,
                        id,
                        i,
                        node_border,
                        node_border,
                        id,
                        i,
                        node_border,
                        id,
                        node_border,
                        id,
                        i,
                        node_border
                    );
                    if let Some(facts) = node_paint.as_mut() {
                        let border = facts.source(4, &node_border);
                        facts.commits[0] = [border; GITGRAPH_PALETTE_SLOT_COUNT];
                        facts.outers[0] = [border; GITGRAPH_PALETTE_SLOT_COUNT];
                        facts.cherry = border;
                        facts.branches[2][0] = border;
                        facts.commits[1][0] = border;
                        facts.outers[1][0] = border;
                        facts.arrows[0] = border;
                    }
                    if use_gradient {
                        let primary_border = if node_paint.is_some()
                            && config_string(effective_config, &["themeVariables", "gradientStart"])
                                .is_none()
                        {
                            Some(node_paint_values[3].map(Cow::Borrowed).unwrap_or_else(|| {
                                Cow::Owned(theme_token(
                                    effective_config,
                                    "primaryBorderColor",
                                    "#9370DB",
                                ))
                            }))
                        } else {
                            None
                        };
                        for label_i in 0..theme_color_limit {
                            let _ = write!(
                                &mut out,
                                r#"#{} .label{}{{fill:{};stroke:url(#{}-gradient);stroke-width:{};}}"#,
                                id, label_i, main_bkg, fragment_id, stroke_width
                            );
                            if let Some(facts) = node_paint.as_mut()
                                && label_i < GITGRAPH_PALETTE_SLOT_COUNT
                            {
                                facts.branches[0][label_i] = facts.source(1, &main_bkg);
                                if let Some(primary_border) = primary_border.as_deref() {
                                    facts.branches[1][label_i] = facts.source(3, primary_border);
                                }
                            }
                        }
                    }
                } else {
                    let git =
                        theme_token(effective_config, &format!("git{ci}"), default_git_color(ci));
                    let branch_label = theme_token(
                        effective_config,
                        &format!("gitBranchLabel{ci}"),
                        default_git_branch_label(ci),
                    );
                    let git_inv = theme_token(
                        effective_config,
                        &format!("gitInv{ci}"),
                        default_git_inv(ci),
                    );
                    let _ = write!(
                        &mut out,
                        r#"#{} .branch-label{}{{fill:{};}}#{} .commit{}{{stroke:{};fill:{};}}#{} .commit-highlight{}{{stroke:{};fill:{};}}#{} .arrow{}{{stroke:{};}}"#,
                        id, i, branch_label, id, i, git, git, id, i, git_inv, git_inv, id, i, git
                    );
                    if let Some(facts) = node_paint.as_mut()
                        && i < GITGRAPH_PALETTE_SLOT_COUNT
                    {
                        facts.commits[0][i] = 0;
                        facts.outers[0][i] = 0;
                    }
                }
            } else if !use_color_theme {
                let _ = write!(
                    &mut out,
                    r#"#{} .branch-label{}{{fill:{};{}}}#{} .commit{}{{stroke:{};}}#{} .commit-highlight{}{{stroke:{};fill:{};}}#{} .label{}{{fill:{};stroke:{};stroke-width:{};{}}}#{} .arrow{}{{stroke:{};}}#{} .commit-bullets{{fill:{};}}#{} .commit-cherry-pick{}{{stroke:{};}}"#,
                    id,
                    i,
                    node_border,
                    note_font_weight_decl,
                    id,
                    i,
                    node_border,
                    id,
                    i,
                    node_border,
                    node_border,
                    id,
                    i,
                    main_bkg,
                    node_border,
                    stroke_width,
                    note_font_weight_decl,
                    id,
                    i,
                    node_border,
                    id,
                    node_border,
                    id,
                    i,
                    node_border
                );
                if let Some(facts) = node_paint.as_mut() {
                    let border = facts.source(4, &node_border);
                    facts.commits[0] = [border; GITGRAPH_PALETTE_SLOT_COUNT];
                    facts.outers[0] = [border; GITGRAPH_PALETTE_SLOT_COUNT];
                    facts.cherry = border;
                    if i < GITGRAPH_PALETTE_SLOT_COUNT {
                        facts.branches[0][i] = facts.source(1, &main_bkg);
                        facts.branches[1][i] = border;
                        facts.branches[2][i] = border;
                        facts.commits[1][i] = border;
                        facts.outers[1][i] = border;
                        facts.arrows[i] = border;
                    }
                }
            } else if i == 0 {
                let _ = write!(
                    &mut out,
                    r#"#{} .branch-label{}{{fill:{};{}}}#{} .commit{}{{stroke:{};}}#{} .commit-highlight{}{{stroke:{};fill:{};}}#{} .label{}{{fill:{};stroke:{};stroke-width:{};{}}}#{} .arrow{}{{stroke:{};}}#{} .commit-bullets{{fill:{};}}"#,
                    id,
                    i,
                    node_border,
                    note_font_weight_decl,
                    id,
                    i,
                    node_border,
                    id,
                    i,
                    node_border,
                    main_bkg,
                    id,
                    i,
                    main_bkg,
                    node_border,
                    stroke_width,
                    note_font_weight_decl,
                    id,
                    i,
                    node_border,
                    id,
                    node_border
                );
                if let Some(facts) = node_paint.as_mut() {
                    let border = facts.source(4, &node_border);
                    let background = facts.source(1, &main_bkg);
                    facts.commits[0] = [border; GITGRAPH_PALETTE_SLOT_COUNT];
                    facts.outers[0] = [border; GITGRAPH_PALETTE_SLOT_COUNT];
                    facts.cherry = border;
                    facts.branches[0][0] = background;
                    facts.branches[1][0] = border;
                    facts.branches[2][0] = border;
                    facts.outers[0][0] = background;
                    facts.outers[1][0] = border;
                    facts.commits[1][0] = border;
                    facts.arrows[0] = border;
                }
            } else {
                let border_color = border_color_array
                    .get(i % border_color_array.len().max(1))
                    .cloned()
                    .unwrap_or_else(|| node_border.clone());
                let label_fill = if use_dark_theme {
                    main_bkg.as_ref()
                } else {
                    border_color.as_str()
                };
                let _ = write!(
                    &mut out,
                    r#"#{} .branch-label{}{{fill:{};{}}}#{} .commit{}{{stroke:{};fill:{};}}#{} .commit-highlight{}{{stroke:{};fill:{};}}#{} .label{}{{fill:{};stroke:{};stroke-width:{};}}#{} .arrow{}{{stroke:{};}}"#,
                    id,
                    i,
                    node_border,
                    note_font_weight_decl,
                    id,
                    i,
                    border_color,
                    border_color,
                    id,
                    i,
                    border_color,
                    border_color,
                    id,
                    i,
                    label_fill,
                    border_color,
                    stroke_width,
                    id,
                    i,
                    border_color
                );
                if let Some(facts) = node_paint.as_mut()
                    && i < GITGRAPH_PALETTE_SLOT_COUNT
                {
                    let border = facts.source(4, &node_border);
                    let slot_border = if border_color_array.is_empty() {
                        border
                    } else {
                        0
                    };
                    facts.branches[0][i] = if use_dark_theme {
                        facts.source(1, label_fill)
                    } else {
                        slot_border
                    };
                    facts.branches[1][i] = slot_border;
                    facts.branches[2][i] = border;
                    facts.commits[0][i] = slot_border;
                    facts.commits[1][i] = slot_border;
                    facts.outers[0][i] = slot_border;
                    facts.outers[1][i] = slot_border;
                    facts.arrows[i] = slot_border;
                }
            }
        } else {
            if i < 8 {
                for surface in &mut root_geometry {
                    surface[i] = false;
                }
            }
            let git = theme_token(effective_config, &format!("git{ci}"), default_git_color(ci));
            let branch_label = theme_token(
                effective_config,
                &format!("gitBranchLabel{ci}"),
                default_git_branch_label(ci),
            );
            let git_inv = theme_token(
                effective_config,
                &format!("gitInv{ci}"),
                default_git_inv(ci),
            );
            let _ = write!(
                &mut out,
                r#"#{} .branch-label{}{{fill:{};}}#{} .commit{}{{stroke:{};fill:{};}}#{} .commit-highlight{}{{stroke:{};fill:{};}}#{} .label{}{{fill:{};}}#{} .arrow{}{{stroke:{};}}"#,
                id,
                i,
                branch_label,
                id,
                i,
                git,
                git,
                id,
                i,
                git_inv,
                git_inv,
                id,
                i,
                git,
                id,
                i,
                git
            );
        }
    }
    let branch_dasharray = if use_color_gen { "4 2" } else { "2" };
    let commit_label_fill = if use_color_gen {
        node_border.as_str()
    } else {
        commit_label_color.as_str()
    };
    let commit_label_weight = if use_color_gen {
        format!("font-weight:{};", note_font_weight)
    } else {
        String::new()
    };
    let commit_label_bkg_fill = if use_color_gen {
        "transparent"
    } else {
        commit_label_background.as_str()
    };
    let commit_label_bkg_opacity = if use_color_gen { "" } else { "opacity:0.5;" };
    let tag_label_bkg_fill = if use_color_gen {
        main_bkg.as_ref()
    } else {
        tag_label_background.as_ref()
    };
    let tag_label_bkg_stroke = if use_color_gen {
        node_border.as_str()
    } else {
        tag_label_border.as_str()
    };
    let tag_label_bkg_filter = if use_color_gen {
        format!("filter:{};", drop_shadow)
    } else {
        String::new()
    };
    let state_fill = if use_color_gen {
        main_bkg.as_ref()
    } else {
        primary_color.as_ref()
    };
    let reverse_stroke_width = if use_color_gen {
        stroke_width.as_str()
    } else {
        "3"
    };
    let arrow_stroke_width = if use_redux_geometry {
        stroke_width.as_str()
    } else {
        "8"
    };
    let _ = write!(
        &mut out,
        r#"#{} .branch{{stroke-width:{};stroke:{};stroke-dasharray:{};}}#{} .arrow{{stroke-width:{};stroke-linecap:round;fill:none;}}#{} .commit-label{{font-size:{};fill:{};{}}}#{} .commit-label-bkg{{font-size:{};fill:{};{}}}#{} .tag-label{{font-size:{};fill:{};}}#{} .tag-label-bkg{{fill:{};stroke:{};{}}}#{} .tag-hole{{fill:{};}}#{} .commit-merge{{stroke:{};fill:{};}}#{} .commit-reverse{{stroke:{};fill:{};stroke-width:{};}}#{} .commit-highlight-outer{{}}#{} .commit-highlight-inner{{stroke:{};fill:{};}}#{} .gitTitleText{{text-anchor:middle;font-size:{}px;fill:{};}}"#,
        id,
        stroke_width,
        commit_line_color,
        branch_dasharray,
        id,
        arrow_stroke_width,
        id,
        commit_label_font_size,
        commit_label_fill,
        commit_label_weight,
        id,
        commit_label_font_size,
        commit_label_bkg_fill,
        commit_label_bkg_opacity,
        id,
        tag_label_font_size,
        tag_label_color,
        id,
        tag_label_bkg_fill,
        tag_label_bkg_stroke,
        tag_label_bkg_filter,
        id,
        parts.text_color,
        id,
        state_fill,
        state_fill,
        id,
        state_fill,
        state_fill,
        reverse_stroke_width,
        id,
        id,
        state_fill,
        state_fill,
        id,
        fmt(typography_theme.title_font_size_px()),
        parts.text_color
    );
    out.push_str(&parts.root_rule);
    if let Some(facts) = node_paint.as_mut() {
        facts.state = facts.source(usize::from(use_color_gen), state_fill);
        facts.tag = facts.source(if use_color_gen { 1 } else { 2 }, tag_label_bkg_fill)
            | facts.source(if use_color_gen { 4 } else { 5 }, tag_label_bkg_stroke);
        if use_color_gen {
            facts.commit_label = facts.source(4, commit_label_fill);
        }
        facts.complete = true;
    }
    GitGraphCss {
        css: out,
        defs,
        font_family_css: parts.font_family,
        base_font_size_css: parts.font_size_css,
        base_typography_emitted: parts.base_typography_emitted,
        commit_label_background_fill: commit_label_bkg_fill.to_string(),
        branch_stroke: commit_line_color,
        text_colors: text_colors.map(|_| {
            [
                parts.text_color.clone(),
                parts.text_color.clone(),
                parts.text_color,
                tag_label_color,
                commit_label_fill.to_string(),
            ]
        }),
        root_geometry,
        root_cherry_pick,
        node_paint,
    }
}

#[derive(Debug, Clone, Copy)]
struct GitGraphTerminalPaletteSources {
    use_color_theme: bool,
    use_neo_theme: bool,
    use_dark_theme: bool,
    use_color_gen: bool,
    use_gradient: bool,
    has_border_color_array: bool,
}

impl GitGraphTerminalPaletteSources {
    fn from_config(effective_config: &serde_json::Value) -> Self {
        let theme_name = gitgraph_theme_name(effective_config);
        Self {
            use_color_theme: gitgraph_theme_is_color(&theme_name),
            use_neo_theme: gitgraph_theme_is_neo(&theme_name),
            use_dark_theme: gitgraph_theme_is_dark(&theme_name),
            use_color_gen: crate::gitgraph::gitgraph_theme_uses_color_gen(&theme_name),
            use_gradient: config_bool(effective_config, &["themeVariables", "useGradient"])
                .unwrap_or(false),
            has_border_color_array: !gitgraph_theme_array(effective_config, "borderColorArray")
                .is_empty(),
        }
    }

    fn branch_slot(self, slot: usize) -> crate::gitgraph::GitGraphPaletteSource {
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

    fn state(self) -> crate::gitgraph::GitGraphPaletteSource {
        if self.use_color_gen {
            crate::gitgraph::GitGraphPaletteSource::MainBackground
        } else {
            crate::gitgraph::GitGraphPaletteSource::PrimaryColor
        }
    }

    fn branch_label_background(
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

fn gitgraph_node_palette_surface_ownership(
    effective_config: &serde_json::Value,
    node_palette: &crate::gitgraph::GitGraphNodePalettePlan,
) -> crate::gitgraph::GitGraphPaletteSurfaceOwnership {
    let mut ownership = crate::gitgraph::GitGraphPaletteSurfaceOwnership::default();
    if !node_palette.has_palette_assignment() {
        return ownership;
    }
    let sources = GitGraphTerminalPaletteSources::from_config(effective_config);

    for slot in 0..crate::gitgraph::GITGRAPH_PALETTE_SLOT_COUNT {
        if node_palette.surface_is_visible(crate::gitgraph::GitGraphPaletteSurface::Arrow, slot)
            && node_palette.mermaid_source_is_owned(sources.branch_slot(slot))
        {
            ownership.mark_stroke_owned(crate::gitgraph::GitGraphPaletteSurface::Arrow, slot);
        }
        if node_palette.surface_is_visible(
            crate::gitgraph::GitGraphPaletteSurface::BranchLabelBackground,
            slot,
        ) && sources
            .branch_label_background(slot)
            .is_some_and(|source| node_palette.mermaid_source_is_owned(source))
        {
            ownership.mark_fill_owned(
                crate::gitgraph::GitGraphPaletteSurface::BranchLabelBackground,
                slot,
            );
        }
    }

    for (slot, role) in node_palette.commit_palette_elements() {
        let (fill_source, stroke_source) = match role {
            crate::gitgraph::GitGraphCommitPaletteRole::BranchSlot => (
                Some(sources.branch_slot(slot)),
                Some(sources.branch_slot(slot)),
            ),
            crate::gitgraph::GitGraphCommitPaletteRole::State => {
                let source = sources.state();
                (Some(source), Some(source))
            }
        };
        if fill_source.is_some_and(|source| node_palette.mermaid_source_is_owned(source)) {
            // A single `.commitN` selector covers every palette-bearing element in the slot. If
            // any concrete element has a higher-priority Mermaid fill owner, suppress only the
            // shared fill declaration while leaving an independently unowned stroke available.
            ownership.mark_fill_owned(crate::gitgraph::GitGraphPaletteSurface::Commit, slot);
        }
        if stroke_source.is_some_and(|source| node_palette.mermaid_source_is_owned(source)) {
            // Stroke ownership is independent from fill ownership because the two properties can
            // reach the commit through different explicit or inherited terminal declarations.
            ownership.mark_stroke_owned(crate::gitgraph::GitGraphPaletteSurface::Commit, slot);
        }
    }

    ownership
}

fn gitgraph_node_palette_css(
    diagram_id: &str,
    node_palette: &crate::gitgraph::GitGraphNodePalettePlan,
    ownership: crate::gitgraph::GitGraphPaletteSurfaceOwnership,
    receipt: &mut Option<crate::gitgraph::GitGraphNodePaletteReceipt>,
) -> String {
    let id = crate::svg::escape_css_identifier(diagram_id);
    let mut css = String::new();
    for slot in 0..crate::gitgraph::GITGRAPH_PALETTE_SLOT_COUNT {
        for surface in crate::gitgraph::GitGraphPaletteSurface::ALL {
            let fill = node_palette.terminal_fill_css(surface, slot, ownership);
            let stroke = node_palette.terminal_stroke_css(surface, slot, ownership);
            if fill.is_none() && stroke.is_none() {
                continue;
            }
            let class_name = match surface {
                crate::gitgraph::GitGraphPaletteSurface::Commit => "commit",
                crate::gitgraph::GitGraphPaletteSurface::Arrow => "arrow",
                crate::gitgraph::GitGraphPaletteSurface::BranchLabelBackground => "label",
            };
            let _ = write!(&mut css, "#{id} .{class_name}{slot}{{");
            if let Some(stroke) = stroke {
                let _ = write!(&mut css, "stroke:{stroke};");
            }
            if let Some(fill) = fill {
                let _ = write!(&mut css, "fill:{fill};");
            }
            css.push('}');
            if let Some(receipt) = receipt.as_mut() {
                receipt.record_stylesheet_rule(node_palette, surface, slot, fill, stroke);
            }
        }
    }
    css
}

fn gitgraph_commit_tag_label_width_px(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &crate::text::TextStyle,
) -> f64 {
    measurer
        .measure_svg_raw_text_bbox_width_px(text, style)
        .max(0.0)
}

fn gitgraph_commit_tag_label_height_px(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &crate::text::TextStyle,
) -> f64 {
    if text.trim_end().is_empty() {
        return 0.0;
    }
    if style.font_size <= 10.0 {
        return measurer
            .measure_svg_simple_text_bbox_height_px(text, style)
            .max(0.0);
    }
    crate::text::svg_wrapped_first_line_bbox_height_px(style).max(0.0)
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
)]
pub(crate) fn render_gitgraph_diagram_svg_model(
    layout: &crate::model::GitGraphDiagramLayout,
    model: &merman_core::diagrams::git_graph::GitGraphRenderModel,
    node_palette: &crate::gitgraph::GitGraphNodePalettePlan,
    static_paint: &crate::gitgraph::GitGraphStaticPaintPlan,
    typography_theme: &crate::gitgraph::GitGraphTypographyThemePlan,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_title = crate::gitgraph::resolve_gitgraph_title(model, diagram_title);
    let acc_title = model
        .acc_title
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    let acc_descr = model
        .acc_descr
        .as_deref()
        .map(|s| s.trim_end_matches('\n'))
        .filter(|s| !s.is_empty());

    render_gitgraph_diagram_svg_with_accessibility(
        layout,
        acc_title,
        acc_descr,
        node_palette,
        static_paint,
        typography_theme,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes geometry, resolved styles, and terminal evidence separately."
)]
fn render_gitgraph_diagram_svg_with_accessibility(
    layout: &crate::model::GitGraphDiagramLayout,
    acc_title: Option<&str>,
    acc_descr: Option<&str>,
    node_palette: &crate::gitgraph::GitGraphNodePalettePlan,
    static_paint: &crate::gitgraph::GitGraphStaticPaintPlan,
    typography_theme: &crate::gitgraph::GitGraphTypographyThemePlan,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    const PX: f64 = 4.0;
    const PY: f64 = 2.0;
    const VIEWBOX_PADDING_PX: f64 = 8.0;

    fn include_gitgraph_branch_line_bounds(
        bounds: &mut Bounds,
        layout: &crate::model::GitGraphDiagramLayout,
        use_redux_geometry: bool,
    ) {
        if !layout.show_branches {
            return;
        }

        fn include_point(bounds: &mut Bounds, x: f64, y: f64) {
            bounds.min_x = bounds.min_x.min(x);
            bounds.min_y = bounds.min_y.min(y);
            bounds.max_x = bounds.max_x.max(x);
            bounds.max_y = bounds.max_y.max(y);
        }

        for branch in &layout.branches {
            match layout.direction.as_str() {
                "TB" => {
                    include_point(bounds, branch.pos, 30.0);
                    include_point(bounds, branch.pos, layout.max_pos);
                }
                "BT" => {
                    include_point(bounds, branch.pos, layout.max_pos);
                    include_point(bounds, branch.pos, 30.0);
                }
                _ => {
                    let spine_y =
                        crate::gitgraph::gitgraph_lr_branch_spine_y(branch.pos, use_redux_geometry);
                    include_point(bounds, 0.0, spine_y);
                    include_point(bounds, layout.max_pos, spine_y);
                }
            }
        }
    }

    let diagram_id = options.diagram_id_or("merman");

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    let vb_min_x = bounds.min_x;
    let vb_min_y = bounds.min_y;
    let vb_w = (bounds.max_x - bounds.min_x).max(1.0);
    let vb_h = (bounds.max_y - bounds.min_y).max(1.0);

    let aria_title_id = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_desc_id = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let aria_describedby = aria_desc_id.as_deref();
    let aria_labelledby = aria_title_id.as_deref();
    let root_context =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::GIT_GRAPH, diagram_id);
    let root_document = root_context.begin_document(
        &mut out,
        root_svg::DeferredRootSpec::responsive(),
        root_svg::RootChrome {
            aria_labelledby,
            aria_describedby,
            dom: root_svg::RootDomProfile {
                trailing_newline: false,
                ..Default::default()
            },
            ..root_svg::RootChrome::new(diagram_id, "gitGraph")
        },
    )?;
    options.checkpoint_emit()?;

    if let Some(t) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml(t)
        );
    }
    if let Some(d) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml(d)
        );
    }
    out.checkpoint()?;

    let theme_name = gitgraph_theme_name(effective_config);
    let use_redux_geometry = crate::gitgraph::gitgraph_theme_is_redux_geometry(&theme_name);
    let use_dark_theme = gitgraph_theme_is_dark(&theme_name);
    let look = config_diagram_look(effective_config);
    let title = diagram_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let css = gitgraph_css(
        diagram_id.semantic_str(),
        effective_config,
        static_paint,
        typography_theme,
        node_palette
            .text_paint()
            .is_requested()
            .then(|| node_palette.text_paint().css_colors()),
        node_palette.branch_stylesheet_stroke(),
    );
    let node_palette_ownership =
        gitgraph_node_palette_surface_ownership(effective_config, node_palette);
    let mut node_palette_receipt = node_palette.begin_terminal_receipt(node_palette_ownership);
    let mut branch_stroke_receipt: Option<crate::gitgraph::GitGraphBranchStrokeReceipt> =
        node_palette.begin_branch_stroke_receipt();
    let mut typography_receipt = typography_theme.begin_terminal_receipt(layout, title);
    let mut static_paint_receipt = static_paint.begin_terminal_receipt();
    let node_palette_css = gitgraph_node_palette_css(
        diagram_id.semantic_str(),
        node_palette,
        node_palette_ownership,
        &mut node_palette_receipt,
    );
    let _ = write!(
        &mut out,
        r#"<style>{}{}</style>"#,
        css.css, node_palette_css
    );
    out.checkpoint()?;
    if let Some(receipt) = branch_stroke_receipt.as_mut() {
        receipt.record_css(&css.branch_stroke);
    }
    if let Some(receipt) = typography_receipt.as_mut() {
        receipt.record_css_emission(css.typography_emission());
    }
    static_paint.record_css_emission(
        &mut static_paint_receipt,
        Some(css.commit_label_background_fill()),
    );

    out.push_str(r#"<g/>"#);
    out.push_str(&css.defs);
    out.push_str(r#"<g class="commit-bullets"/>"#);
    out.push_str(r#"<g class="commit-labels"/>"#);
    out.checkpoint()?;

    let mut branch_idx: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    for b in &layout.branches {
        branch_idx.insert(b.name.as_str(), b.index);
    }

    let mut node_paint_receipt =
        static_paint
            .node_paint()
            .zip(css.node_paint.as_ref())
            .map(|(plan, facts)| {
                let fill_masked = [
                    crate::gitgraph::GitGraphPaletteSurface::Commit,
                    crate::gitgraph::GitGraphPaletteSurface::BranchLabelBackground,
                ]
                .map(|surface| {
                    std::array::from_fn(|slot| {
                        node_palette
                            .terminal_fill_css(surface, slot, node_palette_ownership)
                            .is_some()
                    })
                });
                let stroke_masked = [
                    crate::gitgraph::GitGraphPaletteSurface::Commit,
                    crate::gitgraph::GitGraphPaletteSurface::Arrow,
                ]
                .map(|surface| {
                    std::array::from_fn(|slot| {
                        node_palette
                            .terminal_stroke_css(surface, slot, node_palette_ownership)
                            .is_some()
                    })
                });
                plan.begin_terminal_receipt(layout, &branch_idx, fill_masked, stroke_masked, facts)
            });

    let text_paint = node_palette.text_paint();
    let mut text_receipt = if text_paint.is_requested() {
        let mut root_geometry = css.root_geometry;
        let [branch_backgrounds, commits, ..] = &mut root_geometry;
        for (slot, (branch_inherited, commit_inherited)) in branch_backgrounds
            .iter_mut()
            .zip(commits.iter_mut())
            .enumerate()
        {
            if node_palette
                .terminal_fill_css(
                    crate::gitgraph::GitGraphPaletteSurface::BranchLabelBackground,
                    slot,
                    node_palette_ownership,
                )
                .is_some()
            {
                *branch_inherited = false;
            }
            if node_palette
                .terminal_fill_css(
                    crate::gitgraph::GitGraphPaletteSurface::Commit,
                    slot,
                    node_palette_ownership,
                )
                .is_some()
            {
                // Highlight outer uses commit-highlightN, so a .commitN override does not own it.
                *commit_inherited = false;
            }
        }
        let mut receipt = text_paint.begin_terminal_receipt(
            layout,
            &branch_idx,
            root_geometry,
            css.root_cherry_pick,
        );
        if let (Some(receipt), Some(colors)) = (receipt.as_mut(), css.text_colors.as_ref()) {
            receipt.record_css(std::array::from_fn(|i| colors[i].as_str()));
        }
        receipt
    } else {
        None
    };
    let direction = layout.direction.as_str();
    let branch_border_radius = if use_redux_geometry { 0.0 } else { 4.0 };
    let branch_label_padding_x = if use_redux_geometry { 16.0 } else { 0.0 };
    let branch_label_padding_y = if use_redux_geometry {
        crate::gitgraph::REDUX_BRANCH_LABEL_PADDING_Y
    } else {
        0.0
    };
    let branch_label_style = if look.is_neo() {
        if use_redux_geometry {
            GitGraphBranchLabelStyle::ScopedDropShadow(diagram_id)
        } else {
            GitGraphBranchLabelStyle::Configured(
                crate::config::config_css_number_or_string(
                    effective_config,
                    &["themeVariables", "dropShadow"],
                )
                .unwrap_or_else(|| "none".to_string()),
            )
        }
    } else {
        GitGraphBranchLabelStyle::None
    };
    let branch_data_look = if look.is_neo() {
        r#" data-look="neo""#
    } else {
        ""
    };
    let branch_stroke_style = node_palette
        .terminal_branch_stroke_css()
        .map(|stroke| format!("stroke:{stroke};"));
    let branch_stroke_attr = branch_stroke_style
        .as_deref()
        .map(|style| format!(r#" style="{}""#, escape_attr(style)))
        .unwrap_or_default();

    if layout.show_branches {
        out.push_str("<g>");
        for (branch_index, b) in layout.branches.iter().enumerate() {
            let idx = crate::gitgraph::palette_slot(b.index);
            let pos = b.pos;
            let branch_class = format!("branch branch{idx}");

            if direction == "TB" {
                let _ = write!(
                    &mut out,
                    r#"<line x1="{x1}" y1="30" x2="{x2}" y2="{y2}" class="{class}"{style}/>"#,
                    x1 = fmt(pos),
                    x2 = fmt(pos),
                    y2 = fmt(layout.max_pos),
                    class = branch_class,
                    style = branch_stroke_attr,
                );
            } else if direction == "BT" {
                let _ = write!(
                    &mut out,
                    r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="30" class="{class}"{style}/>"#,
                    x1 = fmt(pos),
                    y1 = fmt(layout.max_pos),
                    x2 = fmt(pos),
                    class = branch_class,
                    style = branch_stroke_attr,
                );
            } else {
                let spine_y = crate::gitgraph::gitgraph_lr_branch_spine_y(pos, use_redux_geometry);
                let _ = write!(
                    &mut out,
                    r#"<line x1="0" y1="{y1}" x2="{x2}" y2="{y2}" class="{class}"{style}/>"#,
                    y1 = fmt(spine_y),
                    x2 = fmt(layout.max_pos),
                    y2 = fmt(spine_y),
                    class = branch_class,
                    style = branch_stroke_attr,
                );
            }
            node_palette.record_branch_line(
                &mut branch_stroke_receipt,
                &branch_class,
                branch_stroke_style.as_deref(),
            );

            let name = escape_xml(&b.name);
            let bbox_w = b.bbox_width.max(0.0);
            let bbox_h = b.bbox_height.max(0.0);

            let bkg_class = format!(r#"branchLabelBkg label{idx}"#);
            let label_class = format!(r#"label branch-label{idx}"#);

            if direction == "TB" {
                let x = pos - bbox_w / 2.0 - 10.0;
                let bkg_transform = if use_redux_geometry {
                    format!(
                        r#" transform="translate({}, {})""#,
                        fmt(-branch_label_padding_x / 2.0 - 3.0),
                        fmt(-branch_label_padding_y - 10.0)
                    )
                } else {
                    String::new()
                };
                let _ = write!(
                    &mut out,
                    r#"<rect{data_look} class="{cls}" style="{style}" rx="{radius}" ry="{radius}" x="{x}" y="0" width="{w}" height="{h}"{transform}/>"#,
                    data_look = branch_data_look,
                    cls = bkg_class,
                    style = &branch_label_style,
                    radius = fmt(branch_border_radius),
                    x = fmt(x),
                    w = fmt(bbox_w + 18.0 + branch_label_padding_x),
                    h = fmt(bbox_h + 4.0 + branch_label_padding_y),
                    transform = bkg_transform,
                );
                let tx = pos - bbox_w / 2.0 - 5.0;
                let ty = if use_redux_geometry {
                    -branch_label_padding_y * 2.0 + 7.0
                } else {
                    0.0
                };
                let _ = write!(
                    &mut out,
                    r#"<g class="branchLabel"><g class="{cls}" transform="translate({x}, {y})"><text><tspan xml:space="preserve" dy="1em" x="0" class="row">{name}</tspan></text></g></g>"#,
                    cls = label_class,
                    x = fmt(tx),
                    y = fmt(ty),
                    name = name
                );
            } else if direction == "BT" {
                let x = pos - bbox_w / 2.0 - 10.0;
                let bkg_transform = if use_redux_geometry {
                    format!(
                        r#" transform="translate({}, {})""#,
                        fmt(-branch_label_padding_x / 2.0 - 3.0),
                        fmt(branch_label_padding_y + 10.0)
                    )
                } else {
                    String::new()
                };
                let _ = write!(
                    &mut out,
                    r#"<rect{data_look} class="{cls}" style="{style}" rx="{radius}" ry="{radius}" x="{x}" y="{y}" width="{w}" height="{h}"{transform}/>"#,
                    data_look = branch_data_look,
                    cls = bkg_class,
                    style = &branch_label_style,
                    radius = fmt(branch_border_radius),
                    x = fmt(x),
                    y = fmt(layout.max_pos),
                    w = fmt(bbox_w + 18.0 + branch_label_padding_x),
                    h = fmt(bbox_h + 4.0 + branch_label_padding_y),
                    transform = bkg_transform,
                );
                let tx = pos - bbox_w / 2.0 - 5.0;
                let ty = if use_redux_geometry {
                    layout.max_pos + branch_label_padding_y * 2.0 + 4.0
                } else {
                    layout.max_pos
                };
                let _ = write!(
                    &mut out,
                    r#"<g class="branchLabel"><g class="{cls}" transform="translate({x}, {y})"><text><tspan xml:space="preserve" dy="1em" x="0" class="row">{name}</tspan></text></g></g>"#,
                    cls = label_class,
                    x = fmt(tx),
                    y = fmt(ty),
                    name = name
                );
            } else {
                let rotate_pad = if layout.rotate_commit_label {
                    30.0
                } else {
                    0.0
                };
                let x = -bbox_w - 4.0 - rotate_pad;
                let y = -bbox_h / 2.0 + 10.0;
                let spine_y = crate::gitgraph::gitgraph_lr_branch_spine_y(pos, use_redux_geometry);
                let _ = write!(
                    &mut out,
                    r#"<rect{data_look} class="{cls}" style="{style}" rx="{radius}" ry="{radius}" x="{x}" y="{y}" width="{w}" height="{h}" transform="translate(-19, {ty})"/>"#,
                    data_look = branch_data_look,
                    cls = bkg_class,
                    style = &branch_label_style,
                    radius = fmt(branch_border_radius),
                    x = fmt(x),
                    y = fmt(y),
                    w = fmt(bbox_w + 18.0 + branch_label_padding_x),
                    h = fmt(bbox_h + 4.0 + branch_label_padding_y),
                    ty = fmt(spine_y - 12.0 - branch_label_padding_y / 2.0),
                );
                let tx = -bbox_w - 14.0 - rotate_pad + branch_label_padding_x / 2.0;
                let _ = write!(
                    &mut out,
                    r#"<g class="branchLabel"><g class="{cls}" transform="translate({x}, {y})"><text><tspan xml:space="preserve" dy="1em" x="0" class="row">{name}</tspan></text></g></g>"#,
                    cls = label_class,
                    x = fmt(tx),
                    y = fmt(spine_y - bbox_h / 2.0 - 2.0),
                    name = name
                );
            }
            out.checkpoint()?;
            if let Some(receipt) = node_palette_receipt.as_mut() {
                receipt.record_branch_label(node_palette, idx);
            }
            if let Some(receipt) = node_paint_receipt.as_mut() {
                receipt.record_branch(branch_index, idx);
            }
            if let Some(receipt) = typography_receipt.as_mut() {
                receipt.record_branch_label(branch_index, &b.name);
            }
            if let Some(receipt) = text_receipt.as_mut() {
                let root_inherited = css.root_geometry[0][idx]
                    && node_palette
                        .terminal_fill_css(
                            crate::gitgraph::GitGraphPaletteSurface::BranchLabelBackground,
                            idx,
                            node_palette_ownership,
                        )
                        .is_none();
                receipt.record_branch(
                    branch_index,
                    idx,
                    &b.name,
                    root_inherited,
                    css.root_geometry[3][idx],
                );
            }
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }

    out.push_str(r#"<g class="commit-arrows">"#);
    for (arrow_index, a) in layout.arrows.iter().enumerate() {
        let idx = crate::gitgraph::palette_slot(a.class_index);
        let _ = write!(
            &mut out,
            r#"<path d="{d}" class="arrow arrow{idx}"/>"#,
            d = escape_attr(&a.d),
            idx = idx
        );
        out.checkpoint()?;
        if let Some(receipt) = node_palette_receipt.as_mut() {
            receipt.record_arrow(node_palette, idx);
        }
        if let Some(receipt) = node_paint_receipt.as_mut() {
            receipt.record_arrow(arrow_index, idx);
        }
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="commit-bullets">"#);
    for (commit_index, c) in layout.commits.iter().enumerate() {
        let branch_i = branch_idx.get(c.branch.as_str()).copied().unwrap_or(0);
        let commit_kind = crate::gitgraph::GitGraphCommitKind::from_layout(c);
        let type_class = commit_kind.class_name();
        let palette_roles = commit_kind.palette_element_roles();
        let idx = crate::gitgraph::palette_slot(branch_i);
        let id = escape_attr(&c.id);

        if commit_kind == crate::gitgraph::GitGraphCommitKind::Highlight {
            let outer_half_size = if use_redux_geometry { 7.0 } else { 10.0 };
            let inner_half_size = if use_redux_geometry { 4.0 } else { 6.0 };
            let _ = write!(
                &mut out,
                r#"<rect x="{x}" y="{y}" width="{size}" height="{size}" class="commit {id} commit-highlight{idx} {type_class}-outer"/>"#,
                x = fmt(c.x - outer_half_size),
                y = fmt(c.y - outer_half_size),
                size = fmt(outer_half_size * 2.0),
                id = id,
                idx = idx,
                type_class = type_class
            );
            if let Some(receipt) = node_paint_receipt.as_mut() {
                receipt.record_state(commit_index, idx, true);
            }
            let _ = write!(
                &mut out,
                r#"<rect x="{x}" y="{y}" width="{size}" height="{size}" class="commit {id} commit{idx} {type_class}-inner"/>"#,
                x = fmt(c.x - inner_half_size),
                y = fmt(c.y - inner_half_size),
                size = fmt(inner_half_size * 2.0),
                id = id,
                idx = idx,
                type_class = type_class
            );
            if let Some(receipt) = node_paint_receipt.as_mut() {
                receipt.record_state(commit_index, idx, false);
            }
            if let (Some(receipt), Some(role)) = (
                node_palette_receipt.as_mut(),
                palette_roles.first().copied(),
            ) {
                receipt.record_commit_element(node_palette, commit_index, 0, idx, role);
            }
        } else if commit_kind == crate::gitgraph::GitGraphCommitKind::CherryPick {
            let outer_radius = if use_redux_geometry { 7.0 } else { 10.0 };
            let inner_radius = if use_redux_geometry { 2.5 } else { 2.75 };
            let cherry_pick_detail_color = if use_dark_theme { "#000000" } else { "#fff" };
            let _ = write!(
                &mut out,
                r#"<circle cx="{x}" cy="{y}" r="{r}" class="commit {id} {type_class}"/>"#,
                x = fmt(c.x),
                y = fmt(c.y),
                r = fmt(outer_radius),
                id = id,
                type_class = type_class
            );
            let _ = write!(
                &mut out,
                r#"<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}" class="commit {id} {type_class}"/>"#,
                x = fmt(c.x - 3.0),
                y = fmt(c.y + 2.0),
                r = fmt(inner_radius),
                fill = cherry_pick_detail_color,
                id = id,
                type_class = type_class
            );
            let _ = write!(
                &mut out,
                r#"<circle cx="{x}" cy="{y}" r="{r}" fill="{fill}" class="commit {id} {type_class}"/>"#,
                x = fmt(c.x + 3.0),
                y = fmt(c.y + 2.0),
                r = fmt(inner_radius),
                fill = cherry_pick_detail_color,
                id = id,
                type_class = type_class
            );
            let _ = write!(
                &mut out,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" class="commit {id} {type_class}"/>"#,
                x1 = fmt(c.x + 3.0),
                y1 = fmt(c.y + 1.0),
                x2 = fmt(c.x),
                y2 = fmt(c.y - 5.0),
                stroke = cherry_pick_detail_color,
                id = id,
                type_class = type_class
            );
            let _ = write!(
                &mut out,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{stroke}" class="commit {id} {type_class}"/>"#,
                x1 = fmt(c.x - 3.0),
                y1 = fmt(c.y + 1.0),
                x2 = fmt(c.x),
                y2 = fmt(c.y - 5.0),
                stroke = cherry_pick_detail_color,
                id = id,
                type_class = type_class
            );
        } else {
            let r = if use_redux_geometry { 7.0 } else { 10.0 };
            let _ = write!(
                &mut out,
                r#"<circle cx="{x}" cy="{y}" r="{r}" class="commit {id} commit{idx}"/>"#,
                x = fmt(c.x),
                y = fmt(c.y),
                r = fmt(r),
                id = id,
                idx = idx
            );
            if let (Some(receipt), Some(role)) = (
                node_palette_receipt.as_mut(),
                palette_roles.first().copied(),
            ) {
                receipt.record_commit_element(node_palette, commit_index, 0, idx, role);
            }
            if commit_kind == crate::gitgraph::GitGraphCommitKind::Merge {
                let inner_radius = if use_redux_geometry { 5.0 } else { 6.0 };
                let _ = write!(
                    &mut out,
                    r#"<circle cx="{x}" cy="{y}" r="{r}" class="commit {type_class} {id} commit{idx}"/>"#,
                    x = fmt(c.x),
                    y = fmt(c.y),
                    r = fmt(inner_radius),
                    type_class = type_class,
                    id = id,
                    idx = idx
                );
                if let Some(receipt) = node_paint_receipt.as_mut() {
                    receipt.record_state(commit_index, idx, false);
                }
                if let (Some(receipt), Some(role)) =
                    (node_palette_receipt.as_mut(), palette_roles.get(1).copied())
                {
                    receipt.record_commit_element(node_palette, commit_index, 1, idx, role);
                }
            }
            if commit_kind == crate::gitgraph::GitGraphCommitKind::Reverse {
                let cross_offset = if use_redux_geometry { 4.0 } else { 5.0 };
                let d = format!(
                    "M {},{}L{},{}M {},{}L{},{}",
                    fmt(c.x - cross_offset),
                    fmt(c.y - cross_offset),
                    fmt(c.x + cross_offset),
                    fmt(c.y + cross_offset),
                    fmt(c.x - cross_offset),
                    fmt(c.y + cross_offset),
                    fmt(c.x + cross_offset),
                    fmt(c.y - cross_offset)
                );
                let _ = write!(
                    &mut out,
                    r#"<path d="{d}" class="commit {type_class} {id} commit{idx}"/>"#,
                    d = escape_attr(&d),
                    type_class = type_class,
                    id = id,
                    idx = idx
                );
                if let Some(receipt) = node_paint_receipt.as_mut() {
                    receipt.record_state(commit_index, idx, false);
                }
                if let (Some(receipt), Some(role)) =
                    (node_palette_receipt.as_mut(), palette_roles.get(1).copied())
                {
                    receipt.record_commit_element(node_palette, commit_index, 1, idx, role);
                }
            }
        }
        out.checkpoint()?;
        if let Some(receipt) = node_paint_receipt.as_mut() {
            receipt.record_commit(commit_index, idx);
        }
        if let Some(receipt) = text_receipt.as_mut() {
            let root_inherited = match commit_kind {
                crate::gitgraph::GitGraphCommitKind::CherryPick => css.root_cherry_pick,
                crate::gitgraph::GitGraphCommitKind::Highlight => css.root_geometry[2][idx],
                _ => {
                    css.root_geometry[1][idx]
                        && node_palette
                            .terminal_fill_css(
                                crate::gitgraph::GitGraphPaletteSurface::Commit,
                                idx,
                                node_palette_ownership,
                            )
                            .is_none()
                }
            };
            receipt.record_commit(commit_index, idx, commit_kind, root_inherited);
        }
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="commit-labels">"#);
    let commit_label_style = typography_theme.commit_label_style();
    let tag_label_style = typography_theme.tag_label_style();
    for (commit_index, c) in layout.commits.iter().enumerate() {
        if crate::gitgraph::gitgraph_commit_label_is_visible(layout, c) {
            let bbox_w = gitgraph_commit_tag_label_width_px(measurer, &c.id, &commit_label_style);
            let bbox_h = gitgraph_commit_tag_label_height_px(measurer, &c.id, &commit_label_style);

            let mut wrapper_transform: Option<String> = None;
            let mut rect_transform: Option<String> = None;
            let mut text_transform: Option<String> = None;

            let mut rect_x = c.pos_with_offset - bbox_w / 2.0 - PY;
            let mut rect_y = c.y + 13.5;
            let rect_w = bbox_w + 2.0 * PY;
            let rect_h = bbox_h + 2.0 * PY;
            let mut text_x = c.pos_with_offset - bbox_w / 2.0;
            let mut text_y = c.y + 25.0;

            if direction == "TB" || direction == "BT" {
                rect_x = c.x - (bbox_w + 4.0 * PX + 5.0);
                rect_y = c.y - 12.0;
                text_x = c.x - (bbox_w + 4.0 * PX);
                text_y = c.y + bbox_h - 12.0;
            }

            if layout.rotate_commit_label {
                if direction == "TB" || direction == "BT" {
                    let t = format!("rotate(-45, {}, {})", fmt(c.x), fmt(c.y));
                    rect_transform = Some(t.clone());
                    text_transform = Some(t);
                } else {
                    let r_x = -7.5 - ((bbox_w + 10.0) / 25.0) * 9.5;
                    let r_y = 10.0 + (bbox_w / 25.0) * 8.5;
                    wrapper_transform = Some(format!(
                        "translate({}, {}) rotate(-45, {}, {})",
                        fmt(r_x),
                        fmt(r_y),
                        fmt(c.pos),
                        fmt(c.y)
                    ));
                }
            }

            out.push_str("<g");
            if let Some(t) = &wrapper_transform {
                let _ = write!(&mut out, r#" transform="{}""#, escape_attr(t));
            }
            out.push('>');

            out.push_str(r#"<rect class="commit-label-bkg""#);
            let _ = write!(
                &mut out,
                r#" x="{}" y="{}" width="{}" height="{}""#,
                fmt(rect_x),
                fmt(rect_y),
                fmt(rect_w),
                fmt(rect_h)
            );
            if let Some(t) = &rect_transform {
                let _ = write!(&mut out, r#" transform="{}""#, escape_attr(t));
            }
            out.push_str("/>");
            static_paint.record_commit_label_background(&mut static_paint_receipt);

            out.push_str(r#"<text class="commit-label""#);
            let _ = write!(
                &mut out,
                r#" x="{}" y="{}""#,
                fmt_display(text_x),
                fmt_display(text_y)
            );
            if let Some(t) = &text_transform {
                let _ = write!(&mut out, r#" transform="{}""#, escape_attr(t));
            }
            let _ = write!(&mut out, ">{}</text>", escape_xml(&c.id));
            out.push_str("</g>");
            out.checkpoint()?;
            if let Some(receipt) = typography_receipt.as_mut() {
                receipt.record_commit_label(commit_index, &c.id);
            }
            if let Some(receipt) = text_receipt.as_mut() {
                receipt.record_commit_label(commit_index, &c.id);
            }
            if let Some(receipt) = node_paint_receipt.as_mut() {
                receipt.record_commit_label(commit_index);
            }
        }

        if !c.tags.is_empty() {
            let mut max_w: f64 = 0.0;
            let mut max_h: f64 = 0.0;
            for tag_value in crate::gitgraph::gitgraph_tags_in_output_order(c) {
                let bbox_w =
                    gitgraph_commit_tag_label_width_px(measurer, tag_value, &tag_label_style);
                let bbox_h =
                    gitgraph_commit_tag_label_height_px(measurer, tag_value, &tag_label_style);
                max_w = max_w.max(bbox_w.max(0.0));
                max_h = max_h.max(bbox_h.max(0.0));
            }

            for (i, tag_value) in crate::gitgraph::gitgraph_tags_in_output_order(c).enumerate() {
                let y_off = i as f64 * 20.0;
                let h2 = max_h / 2.0;
                let ly = c.y - 19.2 - y_off;

                if direction == "TB" || direction == "BT" {
                    let y_origin = c.pos + y_off;
                    let points = format!(
                        "{} {} {} {} {} {} {} {} {} {} {} {}",
                        fmt(c.x),
                        fmt(y_origin + 2.0),
                        fmt(c.x),
                        fmt(y_origin - 2.0),
                        fmt(c.x + 10.0),
                        fmt(y_origin - h2 - 2.0),
                        fmt(c.x + 10.0 + max_w + 4.0),
                        fmt(y_origin - h2 - 2.0),
                        fmt(c.x + 10.0 + max_w + 4.0),
                        fmt(y_origin + h2 + 2.0),
                        fmt(c.x + 10.0),
                        fmt(y_origin + h2 + 2.0)
                    );
                    let poly_t =
                        format!("translate(12,12) rotate(45, {},{})", fmt(c.x), fmt(c.pos));
                    let hole_t =
                        format!("translate(12,12) rotate(45, {},{})", fmt(c.x), fmt(c.pos));
                    let text_t =
                        format!("translate(14,14) rotate(45, {},{})", fmt(c.x), fmt(c.pos));

                    let _ = write!(
                        &mut out,
                        r#"<polygon class="tag-label-bkg" points="{pts}" transform="{t}"/>"#,
                        pts = escape_attr(&points),
                        t = escape_attr(&poly_t)
                    );
                    let _ = write!(
                        &mut out,
                        r#"<circle cy="{cy}" cx="{cx}" r="1.5" class="tag-hole" transform="{t}"/>"#,
                        cy = fmt(y_origin),
                        cx = fmt(c.x + PX / 2.0),
                        t = escape_attr(&hole_t)
                    );
                    let _ = write!(
                        &mut out,
                        r#"<text y="{y}" class="tag-label" x="{x}" transform="{t}">{txt}</text>"#,
                        y = fmt(y_origin + 3.0),
                        x = fmt(c.x + 5.0),
                        t = escape_attr(&text_t),
                        txt = escape_xml(tag_value)
                    );
                } else {
                    let points = format!(
                        "{} {} {} {} {} {} {} {} {} {} {} {}",
                        fmt(c.pos - max_w / 2.0 - PX / 2.0),
                        fmt(ly + PY),
                        fmt(c.pos - max_w / 2.0 - PX / 2.0),
                        fmt(ly - PY),
                        fmt(c.pos_with_offset - max_w / 2.0 - PX),
                        fmt(ly - h2 - PY),
                        fmt(c.pos_with_offset + max_w / 2.0 + PX),
                        fmt(ly - h2 - PY),
                        fmt(c.pos_with_offset + max_w / 2.0 + PX),
                        fmt(ly + h2 + PY),
                        fmt(c.pos_with_offset - max_w / 2.0 - PX),
                        fmt(ly + h2 + PY)
                    );
                    let _ = write!(
                        &mut out,
                        r#"<polygon class="tag-label-bkg" points="{pts}"/>"#,
                        pts = escape_attr(&points)
                    );
                    let _ = write!(
                        &mut out,
                        r#"<circle cy="{cy}" cx="{cx}" r="1.5" class="tag-hole"/>"#,
                        cy = fmt(ly),
                        cx = fmt(c.pos - max_w / 2.0 + PX / 2.0)
                    );
                    let _ = write!(
                        &mut out,
                        r#"<text y="{y}" class="tag-label" x="{x}">{txt}</text>"#,
                        y = fmt(c.y - 16.0 - y_off),
                        x = fmt(c.pos_with_offset - max_w / 2.0),
                        txt = escape_xml(tag_value)
                    );
                }
                out.checkpoint()?;
                if let Some(receipt) = typography_receipt.as_mut() {
                    receipt.record_tag_label(commit_index, i, tag_value);
                }
                if let Some(receipt) = text_receipt.as_mut() {
                    receipt.record_tag(commit_index, i, tag_value);
                }
                if let Some(receipt) = node_paint_receipt.as_mut() {
                    receipt.record_tag(commit_index, i);
                }
            }
        }
        out.checkpoint()?;
    }
    out.push_str("</g>");

    let title_top_margin = config_f64(effective_config, &["gitGraph", "titleTopMargin"])
        .unwrap_or(25.0)
        .max(0.0);
    let title_y = -title_top_margin;

    out.push_str("</svg>\n");
    out.checkpoint()?;

    // GitGraph renders rotated commit labels (e.g. `rotate(-45, ...)`) that are not represented
    // in the precomputed layout bounds. Mirror Mermaid's `setupGraphViewbox(svg.getBBox() + pad)`
    // by computing a headless SVG bbox and patching the root viewBox/max-width.
    let mut b = svg_emitted_bounds_from_svg_inner(out.as_str(), None).unwrap_or(Bounds {
        min_x: vb_min_x,
        min_y: vb_min_y,
        max_x: vb_min_x + vb_w,
        max_y: vb_min_y + vb_h,
    });
    include_gitgraph_branch_line_bounds(&mut b, layout, use_redux_geometry);
    let title_anchor_x = (b.min_x + b.max_x) / 2.0;
    if let Some(title) = title {
        let title_style = typography_theme.title_style();
        let (title_left, title_right) = measurer.measure_svg_title_bbox_x(title, &title_style);
        let (ascent, descent) = crate::text::svg_title_bbox_vertical_extents_px(&title_style);
        let title_min_x = title_anchor_x - title_left;
        let title_max_x = title_anchor_x + title_right;
        b.min_x = b.min_x.min(title_min_x);
        b.max_x = b.max_x.max(title_max_x);
        b.min_y = b.min_y.min(title_y - ascent);
        b.max_y = b.max_y.max(title_y + descent);
    }

    let root_bounds = root_svg::DiagramBounds::from_extents(
        b.min_x,
        b.min_y,
        b.max_x,
        b.max_y,
        VIEWBOX_PADDING_PX,
    );
    let root_document = root_context.finish_document(
        &mut out,
        root_document,
        root_svg::RootViewportSpec::responsive(root_bounds)
            .with_max_width(root_svg::RootMaxWidth::SvgNumber(root_bounds.width)),
    )?;
    if let Some(title) = title {
        let mut title_element = String::new();
        let _ = write!(
            &mut title_element,
            r#"<text text-anchor="middle" x="{x}" y="{y}" class="gitTitleText" xmlns="http://www.w3.org/2000/svg">{text}</text>"#,
            x = fmt_string(title_anchor_x),
            y = fmt(title_y),
            text = escape_xml(title),
        );
        let close_start =
            out.as_str()
                .rfind("</svg>")
                .ok_or_else(|| crate::Error::InvalidModel {
                    message: "gitGraph SVG is missing its closing root element".to_string(),
                })?;
        out.replace_range(close_start..close_start, &title_element)?;
    }
    let rooted_svg = root_document.complete(out.finish()?)?;
    if let (Some(plan), Some(receipt)) = (static_paint.node_paint(), node_paint_receipt) {
        // A complete empty receipt proves NotApplicable; missing proof remains a residual.
        plan.record_terminal(receipt);
    }
    if let Some(receipt) = text_receipt.as_mut() {
        receipt.record_title(title);
    }
    if text_receipt.is_some_and(|receipt| !text_paint.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "GitGraph Text paint receipt did not match the terminal SVG".to_string(),
        });
    }
    if let Some(receipt) = typography_receipt.as_mut() {
        receipt.record_title(title);
    }
    if typography_receipt.is_some_and(|receipt| !typography_theme.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "GitGraph typography receipt did not match the terminal SVG".to_string(),
        });
    }
    if node_palette_receipt.is_some_and(|receipt| !node_palette.record_terminal(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "GitGraph Node palette receipt did not match the terminal SVG".to_string(),
        });
    }
    if static_paint_receipt.is_some_and(|receipt| !static_paint.record_terminal_receipt(receipt)) {
        return Err(crate::Error::InvalidModel {
            message: "GitGraph static paint receipt did not match the terminal SVG".to_string(),
        });
    }
    if branch_stroke_receipt
        .is_some_and(|receipt| !node_palette.record_branch_stroke_terminal(receipt))
    {
        return Err(crate::Error::InvalidModel {
            message: "GitGraph branch stroke receipt did not match the terminal SVG".to_string(),
        });
    }
    Ok(rooted_svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn static_paint_plan() -> crate::gitgraph::GitGraphStaticPaintPlan {
        crate::gitgraph::GitGraphStaticPaintPlan::baseline()
    }
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };
    use crate::resources::{
        RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
    };
    use merman_core::MermaidConfig;
    use serde_json::json;
    use std::cell::RefCell;

    fn typography_plan(config: &serde_json::Value) -> crate::gitgraph::GitGraphTypographyThemePlan {
        crate::gitgraph::GitGraphTypographyThemePlan::resolve(
            None,
            &MermaidConfig::from_value(config.clone()),
        )
    }

    fn direct_typography_plan(
        config: &serde_json::Value,
    ) -> crate::gitgraph::GitGraphTypographyThemePlan {
        let typography = ThemeTextStyle::default()
            .with_font_stack(
                FontStack::single("GitGraphMeasure").expect("valid GitGraph font stack"),
            )
            .with_font_size_px(23.0)
            .expect("valid GitGraph font size");
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::GIT_GRAPH, typography),
            ))
            .expect("compile GitGraph typography")
            .resolve(DiagramFamilyId::GIT_GRAPH);
        crate::gitgraph::GitGraphTypographyThemePlan::resolve(
            Some(&theme),
            &MermaidConfig::from_value(config.clone()),
        )
    }

    fn lr_merge_layout(commit_y: f64) -> crate::model::GitGraphDiagramLayout {
        crate::model::GitGraphDiagramLayout {
            bounds: Some(Bounds {
                min_x: -100.0,
                min_y: -50.0,
                max_x: 100.0,
                max_y: 50.0,
            }),
            direction: "LR".to_string(),
            rotate_commit_label: true,
            show_branches: true,
            show_commit_label: false,
            parallel_commits: false,
            diagram_padding: 8.0,
            max_pos: 100.0,
            branches: vec![crate::model::GitGraphBranchLayout {
                name: "main".to_string(),
                index: 0,
                pos: 0.0,
                bbox_width: 35.25,
                bbox_height: 19.0,
            }],
            commits: vec![crate::model::GitGraphCommitLayout {
                id: "merge".to_string(),
                message: "merge".to_string(),
                seq: 0,
                commit_type: 3,
                custom_type: None,
                custom_id: Some(false),
                tags: Vec::new(),
                parents: Vec::new(),
                branch: "main".to_string(),
                pos: 0.0,
                pos_with_offset: 10.0,
                x: 10.0,
                y: commit_y,
            }],
            arrows: Vec::new(),
        }
    }

    fn render_geometry_fixture(
        layout: &crate::model::GitGraphDiagramLayout,
        config: &serde_json::Value,
    ) -> String {
        render_geometry_fixture_with_policy(
            layout,
            config,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .expect("render gitGraph SVG")
        .into_string_for(DiagramFamilyId::GIT_GRAPH)
        .expect("gitGraph root provenance")
    }

    fn render_geometry_fixture_with_policy(
        layout: &crate::model::GitGraphDiagramLayout,
        config: &serde_json::Value,
        policy: RenderResourcePolicy,
    ) -> crate::Result<root_svg::RootedSvg> {
        let request = SvgRenderOptions::default();
        let debug = SvgDebugOptions::default();
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .expect("render session");
        let execution =
            SvgExecution::unthemed_for_test(&request, &debug, &session, DiagramFamilyId::GIT_GRAPH)
                .expect("SVG execution");
        let node_palette = crate::gitgraph::GitGraphNodePalettePlan::baseline(layout);
        let static_paint = static_paint_plan();
        let typography_theme = typography_plan(config);
        render_gitgraph_diagram_svg_with_accessibility(
            layout,
            None,
            None,
            &node_palette,
            &static_paint,
            &typography_theme,
            config,
            None,
            &crate::text::DeterministicTextMeasurer::default(),
            &execution,
        )
    }

    #[test]
    fn gitgraph_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
        let layout = lr_merge_layout(-2.0);
        let config = json!({});
        let baseline = render_geometry_fixture_with_policy(
            &layout,
            &config,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .expect("render unbounded gitGraph SVG");
        let exact_bytes = baseline.len();
        assert!(exact_bytes > 1);

        let exact = render_geometry_fixture_with_policy(
            &layout,
            &config,
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
                .expect("valid exact gitGraph SVG ceiling"),
        )
        .expect("exact gitGraph SVG ceiling must succeed");
        assert_eq!(exact.as_bytes(), baseline.as_bytes());

        let below_exact = exact_bytes - 1;
        let error = render_geometry_fixture_with_policy(
            &layout,
            &config,
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
                .expect("valid below-exact gitGraph SVG ceiling"),
        )
        .expect_err("one byte below the gitGraph SVG size must fail");
        let crate::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected gitGraph MaxSvgBytes rejection, got {error}");
        };
        assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
        assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
        assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
        assert_eq!(limit.max, below_exact);
        assert!(limit.actual > limit.max);
    }

    #[test]
    fn commit_and_tag_measurements_route_raw_width_and_simple_height() {
        struct OperationProbe;

        impl crate::text::TextMeasurer for OperationProbe {
            fn measure(
                &self,
                _text: &str,
                _style: &crate::text::TextStyle,
            ) -> crate::text::TextMetrics {
                crate::text::TextMetrics {
                    width: 0.0,
                    height: 0.0,
                    line_count: 1,
                }
            }

            fn measure_svg_raw_text_bbox_width_px(
                &self,
                _text: &str,
                _style: &crate::text::TextStyle,
            ) -> f64 {
                41.25
            }

            fn measure_svg_simple_text_bbox_height_px(
                &self,
                _text: &str,
                _style: &crate::text::TextStyle,
            ) -> f64 {
                7.75
            }
        }

        let style = crate::text::TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 10.0,
            font_weight: None,
            font_style: None,
        };

        for label in ["1-abcdefg", "A", "MyTag"] {
            assert_eq!(
                gitgraph_commit_tag_label_width_px(&OperationProbe, label, &style),
                41.25
            );
            assert_eq!(
                gitgraph_commit_tag_label_height_px(&OperationProbe, label, &style),
                7.75
            );
        }
    }

    #[test]
    fn gitgraph_deferred_root_is_fully_finalized() {
        let svg = render_geometry_fixture(&lr_merge_layout(-2.0), &json!({}));
        let root_open = svg.split_once('>').expect("root SVG opening tag").0;
        let view_box = root_open
            .split_once(r#"viewBox=""#)
            .and_then(|(_, tail)| tail.split_once('"'))
            .map(|(value, _)| value)
            .expect("root viewBox");
        let view_box_width = view_box
            .split_whitespace()
            .nth(2)
            .expect("root viewBox width");

        assert!(root_open.contains(r#"width="100%""#), "{root_open}");
        assert!(
            root_open.contains(&format!("max-width: {view_box_width}px;")),
            "{root_open}"
        );
        assert!(!root_open.contains("__MERMAN_ROOT_"), "{root_open}");
        assert!(!svg.contains("__MERMAID_GITGRAPH_TITLE_X__"), "{svg}");
    }

    #[test]
    fn gitgraph_typed_body_title_renders_and_overrides_metadata_title() {
        let model = merman_core::diagrams::git_graph::GitGraphRenderModel {
            diagram_type: "gitGraph".to_string(),
            commits: Vec::new(),
            branches: Vec::new(),
            current_branch: "main".to_string(),
            direction: "LR".to_string(),
            title: Some("Body & Title".to_string()),
            acc_title: None,
            acc_descr: None,
            warning_facts: Vec::new(),
        };

        let request = SvgRenderOptions::default();
        let layout = lr_merge_layout(-2.0);
        let node_palette = crate::gitgraph::GitGraphNodePalettePlan::baseline(&layout);
        let static_paint = crate::gitgraph::GitGraphStaticPaintPlan::baseline();
        let config = json!({});
        let typography_theme = typography_plan(&config);
        let svg = with_test_svg_execution(DiagramFamilyId::GIT_GRAPH, &request, |options| {
            render_gitgraph_diagram_svg_model(
                &layout,
                &model,
                &node_palette,
                &static_paint,
                &typography_theme,
                &config,
                Some("Metadata Title"),
                &crate::text::DeterministicTextMeasurer::default(),
                options,
            )
        })
        .expect("render typed gitGraph SVG");

        assert!(svg.contains(
            r#"class="gitTitleText" xmlns="http://www.w3.org/2000/svg">Body &amp; Title</text>"#
        ));
        assert!(!svg.contains("Metadata Title"));
    }

    #[test]
    fn gitgraph_commit_tag_and_title_measurements_share_the_resolved_family_plan() {
        #[derive(Default)]
        struct RecordingMeasurer {
            calls: RefCell<Vec<(String, crate::text::TextStyle)>>,
        }

        impl TextMeasurer for RecordingMeasurer {
            fn measure(
                &self,
                text: &str,
                style: &crate::text::TextStyle,
            ) -> crate::text::TextMetrics {
                self.calls
                    .borrow_mut()
                    .push((text.to_string(), style.clone()));
                crate::text::TextMetrics {
                    width: 40.0,
                    height: 12.0,
                    line_count: 1,
                }
            }
        }

        let mut layout = lr_merge_layout(-2.0);
        layout.show_commit_label = true;
        layout.commits[0].id = "1".to_string();
        layout.commits[0].commit_type = 0;
        layout.commits[0].tags = vec!["v1".to_string()];
        let config = json!({
            "themeVariables": {
                "commitLabelFontSize": "inherit",
                "tagLabelFontSize": "150%"
            }
        });
        let typography_theme = direct_typography_plan(&config);
        let static_paint = static_paint_plan();
        let css = gitgraph_css(
            "git-measure",
            &config,
            &static_paint,
            &typography_theme,
            None,
            None,
        )
        .css;
        assert!(css.contains(
            "#git-measure .commit-id,#git-measure .commit-msg,#git-measure .branch-label{fill:lightgrey;color:lightgrey;font-family:'trebuchet ms',verdana,arial,sans-serif;font-family:GitGraphMeasure;}"
        ));
        let node_palette = crate::gitgraph::GitGraphNodePalettePlan::baseline(&layout);
        let measurer = RecordingMeasurer::default();
        let request = SvgRenderOptions::default();

        with_test_svg_execution(DiagramFamilyId::GIT_GRAPH, &request, |options| {
            render_gitgraph_diagram_svg_with_accessibility(
                &layout,
                None,
                None,
                &node_palette,
                &static_paint,
                &typography_theme,
                &config,
                Some("Graph title"),
                &measurer,
                options,
            )
        })
        .expect("render GitGraph measurement fixture");

        let calls = measurer.calls.borrow();
        for (text, expected_size) in [("1", 23.0), ("v1", 34.5), ("Graph title", 18.0)] {
            assert!(
                calls.iter().any(|(measured, style)| {
                    measured == text
                        && style.font_family.as_deref() == Some("GitGraphMeasure")
                        && style.font_size == expected_size
                }),
                "missing exact measurement style for {text}: {calls:?}"
            );
        }
        assert!(calls.iter().all(|(_, style)| {
            style.font_family.as_deref() == Some("GitGraphMeasure")
                && matches!(style.font_size, 18.0 | 23.0 | 34.5)
        }));
    }

    #[test]
    fn gitgraph_css_includes_mermaid_11_15_branch_theme_rules() {
        let config = json!({});
        let typography_theme = typography_plan(&config);
        let static_paint = static_paint_plan();
        let css = gitgraph_css("git", &config, &static_paint, &typography_theme, None, None).css;

        assert!(css.contains(
            "#git .commit-id,#git .commit-msg,#git .branch-label{fill:lightgrey;color:lightgrey;font-family:'trebuchet ms',verdana,arial,sans-serif;font-family:\"trebuchet ms\",verdana,arial,sans-serif;}"
        ));
        assert!(!css.contains("font-family:var(--mermaid-font-family)"));
        assert!(css.contains("#git .branch-label0{fill:#ffffff;}"));
        assert!(css.contains(
            "#git .commit0{stroke:hsl(240, 100%, 46.2745098039%);fill:hsl(240, 100%, 46.2745098039%);}"
        ));
        assert!(css.contains("#git .label0{fill:hsl(240, 100%, 46.2745098039%);}"));
        assert!(css.contains("#git .arrow0{stroke:hsl(240, 100%, 46.2745098039%);}"));
        assert!(css.contains("#git .commit-merge{stroke:#ECECFF;fill:#ECECFF;}"));
        assert!(css.contains("#git .commit-highlight-inner{stroke:#ECECFF;fill:#ECECFF;}"));
    }

    #[test]
    fn gitgraph_css_uses_redux_geometry_theme_rules() {
        let config = json!({
            "theme": "redux",
            "themeVariables": {
                "nodeBorder": "#101010",
                "mainBkg": "#ffffff",
                "strokeWidth": 2,
                "noteFontWeight": 600,
                "commitLineColor": "#202020"
            }
        });
        let typography_theme = typography_plan(&config);
        let static_paint = static_paint_plan();
        let css = gitgraph_css("git", &config, &static_paint, &typography_theme, None, None);

        assert!(css.defs.is_empty());
        assert!(
            css.css
                .contains("#git .branch-label0{fill:#101010;font-weight:600;}")
        );
        assert!(css.css.contains("#git .commit0{stroke:#101010;}"));
        assert!(
            css.css.contains(
                "#git .label0{fill:#ffffff;stroke:#101010;stroke-width:2;font-weight:600;}"
            )
        );
        assert!(
            css.css
                .contains("#git .branch{stroke-width:2;stroke:#202020;stroke-dasharray:4 2;}")
        );
        assert!(
            css.css
                .contains("#git .arrow{stroke-width:2;stroke-linecap:round;fill:none;}")
        );
        assert!(
            css.css
                .contains("#git .commit-label{font-size:10px;fill:#101010;font-weight:600;}")
        );
        assert!(
            css.css
                .contains("#git .commit-label-bkg{font-size:10px;fill:transparent;}")
        );
        assert!(
            css.css
                .contains("#git .commit-merge{stroke:#ffffff;fill:#ffffff;}")
        );
        assert!(
            css.css
                .contains("#git .commit-reverse{stroke:#ffffff;fill:#ffffff;stroke-width:2;}")
        );
    }

    #[test]
    fn gitgraph_css_uses_redux_color_theme_rules() {
        let config = json!({
            "theme": "redux-color",
            "themeVariables": {
                "nodeBorder": "#101010",
                "mainBkg": "#ffffff",
                "strokeWidth": 2,
                "noteFontWeight": 600,
                "borderColorArray": ["#aa0000", "#00aa00"]
            }
        });
        let typography_theme = typography_plan(&config);
        let static_paint = static_paint_plan();
        let css = gitgraph_css("git", &config, &static_paint, &typography_theme, None, None).css;

        assert!(css.contains("#git .commit0{stroke:#101010;}"));
        assert!(css.contains("#git .commit-highlight0{stroke:#101010;fill:#ffffff;}"));
        assert!(
            css.contains(
                "#git .label0{fill:#ffffff;stroke:#101010;stroke-width:2;font-weight:600;}"
            )
        );
        assert!(css.contains("#git .commit1{stroke:#00aa00;fill:#00aa00;}"));
        assert!(css.contains("#git .label1{fill:#00aa00;stroke:#00aa00;stroke-width:2;}"));
        assert!(css.contains("#git .arrow1{stroke:#00aa00;}"));
    }

    #[test]
    fn gitgraph_css_uses_neo_gradient_theme_rules() {
        let config = json!({
            "theme": "neo",
            "themeVariables": {
                "nodeBorder": "#101010",
                "mainBkg": "#ffffff",
                "strokeWidth": 2,
                "useGradient": true,
                "gradientStart": "#112233",
                "gradientStop": "#445566",
                "git1": "#00aa00",
                "gitInv1": "#aa00aa",
                "gitBranchLabel1": "#202020"
            }
        });
        let typography_theme = typography_plan(&config);
        let static_paint = static_paint_plan();
        let css = gitgraph_css("git", &config, &static_paint, &typography_theme, None, None);

        assert!(
            css.defs
                .contains(r#"<defs><linearGradient id="git-gradient""#)
        );
        assert!(css.defs.contains(r##"stop-color="#112233""##));
        assert!(css.defs.contains(r##"stop-color="#445566""##));
        assert!(css.css.contains("#git .branch-label0{fill:#101010;}"));
        assert!(css.css.contains("#git .commit0{stroke:#101010;}"));
        assert!(css.css.contains("#git .commit-bullets{fill:#101010;}"));
        assert!(
            css.css
                .contains("#git .label0{fill:#ffffff;stroke:url(#git-gradient);stroke-width:2;}")
        );
        assert!(
            css.css
                .contains("#git .label11{fill:#ffffff;stroke:url(#git-gradient);stroke-width:2;}")
        );
        assert!(css.css.contains("#git .branch-label1{fill:#202020;}"));
        assert!(
            css.css
                .contains("#git .commit1{stroke:#00aa00;fill:#00aa00;}")
        );
        assert!(
            css.css
                .contains("#git .commit-highlight1{stroke:#aa00aa;fill:#aa00aa;}")
        );
    }

    #[test]
    fn gitgraph_render_uses_mermaid_11_16_lr_spine_and_merge_geometry() {
        let svg = render_geometry_fixture(&lr_merge_layout(-2.0), &json!({}));

        assert!(
            svg.contains(r#"<line x1="0" y1="-2" x2="100" y2="-2" class="branch branch0"/>"#),
            "{svg}"
        );
        assert!(svg.contains(
            r#"<rect class="branchLabelBkg label0" style="" rx="4" ry="4" x="-69.25" y="0.5" width="53.25" height="23" transform="translate(-19, -14)"/>"#
        ));
        assert!(
            svg.contains(r#"<g class="label branch-label0" transform="translate(-79.25, -13.5)">"#)
        );
        assert!(svg.contains(r#"<circle cx="10" cy="-2" r="10" class="commit merge commit0"/>"#));
        assert!(svg.contains(
            r#"<circle cx="10" cy="-2" r="6" class="commit commit-merge merge commit0"/>"#
        ));
    }

    #[test]
    fn gitgraph_render_uses_redux_geometry_and_neo_branch_filter() {
        let svg = render_geometry_fixture(
            &lr_merge_layout(7.0),
            &json!({
                "look": "neo",
                "theme": "redux",
                "themeVariables": {
                    "filterColor": "#123456"
                }
            }),
        );

        assert!(svg.contains(
            r##"<defs><filter id="merman-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="#123456"/></filter></defs>"##
        ));
        assert!(
            svg.contains(r#"<line x1="0" y1="7" x2="100" y2="7" class="branch branch0"/>"#),
            "{svg}"
        );
        assert!(svg.contains(
            r#"<rect data-look="neo" class="branchLabelBkg label0" style="filter:url(#merman-drop-shadow)" rx="0" ry="0" x="-69.25" y="0.5" width="69.25" height="35" transform="translate(-19, -11)"/>"#
        ));
        assert!(
            svg.contains(r#"<g class="label branch-label0" transform="translate(-71.25, -4.5)">"#)
        );
        assert!(svg.contains(r#"<circle cx="10" cy="7" r="7" class="commit merge commit0"/>"#));
        assert!(svg.contains(
            r#"<circle cx="10" cy="7" r="5" class="commit commit-merge merge commit0"/>"#
        ));
    }

    #[test]
    fn gitgraph_root_emits_gradient_for_non_neo_theme_when_enabled() {
        let layout = crate::model::GitGraphDiagramLayout {
            bounds: Some(Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 100.0,
                max_y: 100.0,
            }),
            direction: "LR".to_string(),
            rotate_commit_label: false,
            show_branches: false,
            show_commit_label: false,
            parallel_commits: false,
            diagram_padding: 8.0,
            max_pos: 100.0,
            branches: Vec::new(),
            commits: Vec::new(),
            arrows: Vec::new(),
        };
        let node_palette = crate::gitgraph::GitGraphNodePalettePlan::baseline(&layout);
        let config = json!({
            "theme": "base",
            "themeVariables": {
                "useGradient": true,
                "gradientStart": "#112233",
                "gradientStop": "#445566"
            }
        });
        let typography_theme = typography_plan(&config);
        let static_paint = static_paint_plan();
        let request = SvgRenderOptions::default();
        let svg = with_test_svg_execution(DiagramFamilyId::GIT_GRAPH, &request, |options| {
            render_gitgraph_diagram_svg_with_accessibility(
                &layout,
                None,
                None,
                &node_palette,
                &static_paint,
                &typography_theme,
                &config,
                None,
                &crate::text::DeterministicTextMeasurer::default(),
                options,
            )
        })
        .expect("render gitGraph SVG");

        let initial_group = svg.find("<g/>").expect("initial gitGraph root group");
        let gradient = svg
            .find(r#"<defs><linearGradient id="merman-gradient""#)
            .expect("configured base theme gradient");
        let commit_bullets = svg
            .find(r#"<g class="commit-bullets"/>"#)
            .expect("commit bullets group");
        assert!(
            initial_group < gradient && gradient < commit_bullets,
            "gitGraph should append its gradient after the initial root group and before diagram groups: {svg}"
        );
    }
}
