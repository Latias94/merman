use super::super::*;
use crate::gitgraph::GitGraphCssBinding;

#[derive(Default)]
struct GitGraphCss {
    css: String,
    defs: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GitGraphCommitKind {
    Normal,
    Reverse,
    Highlight,
    Merge,
    CherryPick,
}

impl GitGraphCommitKind {
    fn from_layout(commit: &crate::model::GitGraphCommitLayout) -> Self {
        match commit.custom_type.unwrap_or(commit.commit_type) {
            1 => Self::Reverse,
            2 => Self::Highlight,
            3 => Self::Merge,
            4 => Self::CherryPick,
            _ => Self::Normal,
        }
    }

    const fn class_name(self) -> &'static str {
        match self {
            Self::Normal => "commit-normal",
            Self::Reverse => "commit-reverse",
            Self::Highlight => "commit-highlight",
            Self::Merge => "commit-merge",
            Self::CherryPick => "commit-cherry-pick",
        }
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

fn gitgraph_defs<I>(
    diagram_id: I,
    binding: &GitGraphCssBinding,
    gradient: Option<(&str, &str)>,
) -> String
where
    I: Copy + std::fmt::Display,
{
    let mut out = String::new();

    if let Some((gradient_start, gradient_stop)) = gradient {
        let _ = write!(
            &mut out,
            r#"<defs><linearGradient id="{}-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%"><stop offset="0%" stop-color="{}" stop-opacity="1"/><stop offset="100%" stop-color="{}" stop-opacity="1"/></linearGradient></defs>"#,
            diagram_id,
            escape_xml(gradient_start),
            escape_xml(gradient_stop)
        );
    }

    if binding.look_is_neo && binding.use_redux_geometry {
        let filter_color = &binding.filter_color;
        let _ = write!(
            &mut out,
            r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
            diagram_id,
            escape_xml(filter_color)
        );
    }

    out
}

fn gitgraph_css(
    diagram_id: &str,
    binding: &GitGraphCssBinding,
    typography_theme: &crate::gitgraph::GitGraphTypographyThemePlan,
    work: &crate::resources::OperationWorkMeter,
) -> Result<GitGraphCss> {
    let id = crate::svg::escape_css_identifier(diagram_id);
    let fragment_id = super::super::css::FragmentDiagramId(diagram_id);
    let mut css_prefix = String::new();
    binding.common.write_prefix(&mut css_prefix, diagram_id)?;
    let mut root_rule = String::new();
    binding
        .common
        .write_root(&mut root_rule, diagram_id, diagram_id)?;
    let use_redux_geometry = binding.use_redux_geometry;
    let use_color_theme = binding.sources.use_color_theme;
    let use_neo_theme = binding.sources.use_neo_theme;
    let use_dark_theme = binding.sources.use_dark_theme;
    let use_color_gen = binding.sources.use_color_gen;

    let commit_label_font_size = typography_theme.commit_label_font_size_css();
    let tag_label_font_size = typography_theme.tag_label_font_size_css();
    let tag_label_color = &binding.tag_label_color;
    let theme_color_limit = binding.theme_color_limit;
    let stroke_width = &binding.stroke_width;
    let commit_line_color = binding
        .commit_line_color
        .as_deref()
        .unwrap_or(binding.common.line_color());
    let node_border = &binding.node_border;
    let main_bkg = &binding.main_bkg;
    let note_font_weight = &binding.note_font_weight;
    let note_font_weight_decl = if use_redux_geometry {
        format!("font-weight:{};", note_font_weight)
    } else {
        String::new()
    };
    let drop_shadow = &binding.drop_shadow;
    let use_gradient = binding.sources.use_gradient;
    // gitGraph owns its draw path instead of using rendering-util/render.ts, so it must append the
    // configured root gradient itself for every theme. Several classic themes enable gradients.
    let gradient = use_gradient.then(|| {
        let start = binding.gradient_start();
        (start, binding.gradient_stop(start))
    });
    let defs = gitgraph_defs(diagram_id, binding, gradient);
    let mut out = BoundedSvgOutput::new(work);
    out.push_str(&css_prefix);
    let _ = write!(
        &mut out,
        r#"#{} .commit-id,#{} .commit-msg,#{} .branch-label{{fill:lightgrey;color:lightgrey;font-family:'trebuchet ms',verdana,arial,sans-serif;font-family:{};}}"#,
        id,
        id,
        id,
        binding.common.font_family()
    );
    out.checkpoint()?;
    for i in 0..theme_color_limit {
        work.charge_emit_work(1)?;
        let ci = i % GITGRAPH_NAMED_COLOR_COUNT;
        if use_color_gen {
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
                    out.checkpoint()?;
                    if use_gradient {
                        for label_i in 0..theme_color_limit {
                            work.charge_emit_work(1)?;
                            let _ = write!(
                                &mut out,
                                r#"#{} .label{}{{fill:{};stroke:url(#{}-gradient);stroke-width:{};}}"#,
                                id, label_i, main_bkg, fragment_id, stroke_width
                            );
                            out.checkpoint()?;
                        }
                    }
                } else {
                    let git = binding.named[ci].git.clone();
                    let branch_label = binding.named[ci].branch_label.clone();
                    let git_inv = binding.named[ci].inverse.clone();
                    let _ = write!(
                        &mut out,
                        r#"#{} .branch-label{}{{fill:{};}}#{} .commit{}{{stroke:{};fill:{};}}#{} .commit-highlight{}{{stroke:{};fill:{};}}#{} .arrow{}{{stroke:{};}}"#,
                        id, i, branch_label, id, i, git, git, id, i, git_inv, git_inv, id, i, git
                    );
                    out.checkpoint()?;
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
                out.checkpoint()?;
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
                out.checkpoint()?;
            } else {
                let border_color = if binding.border_color_array.is_empty() {
                    node_border
                } else {
                    &binding.border_color_array[i % binding.border_color_array.len()]
                };
                let label_fill = if use_dark_theme {
                    main_bkg
                } else {
                    border_color
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
                out.checkpoint()?;
            }
        } else {
            let git = binding.named[ci].git.clone();
            let branch_label = binding.named[ci].branch_label.clone();
            let git_inv = binding.named[ci].inverse.clone();
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
            out.checkpoint()?;
        }
    }
    let branch_dasharray = if use_color_gen { "4 2" } else { "2" };
    let commit_label_fill = if use_color_gen {
        node_border
    } else {
        &binding.commit_label_color
    };
    let commit_label_weight = typography_theme
        .commit_label_font_weight_css()
        .map(|weight| format!("font-weight:{weight};"))
        .unwrap_or_default();
    let (commit_label_bkg_fill, commit_label_bkg_opacity) = if use_color_gen {
        ("transparent", "")
    } else {
        (binding.commit_label_background.as_str(), "opacity:0.5;")
    };
    let tag_label_bkg_fill = if use_color_gen {
        main_bkg
    } else {
        &binding.tag_label_background
    };
    let tag_label_bkg_stroke = if use_color_gen {
        node_border
    } else {
        &binding.tag_label_border
    };
    let tag_label_bkg_filter = if use_color_gen {
        format!("filter:{};", scoped_drop_shadow(fragment_id, drop_shadow))
    } else {
        String::new()
    };
    let state_fill = if use_color_gen {
        main_bkg
    } else {
        &binding.primary_color
    };
    let reverse_stroke_width = if use_color_gen { stroke_width } else { "3" };
    let arrow_stroke_width = if use_redux_geometry {
        stroke_width
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
        binding.common.text_color(),
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
        binding.common.text_color()
    );
    out.push_str(&root_rule);
    Ok(GitGraphCss {
        css: out.finish()?,
        defs,
    })
}

fn write_gitgraph_native_css(
    out: &mut impl SvgOutput,
    diagram_id: SvgDiagramId<'_>,
    typography: &crate::gitgraph::GitGraphTypographyThemePlan,
    theme: &crate::gitgraph::GitGraphNativeStyle,
) -> Result<()> {
    let id = super::super::util::css_selector_diagram_id(diagram_id);
    let _ = write!(
        out,
        r#"#{id}{{font-family:{font};font-size:{size};font-weight:{weight};fill:{primary}}}
#{id} .branch{{stroke-width:{connector_width};stroke-dasharray:2}}
#{id} .arrow{{stroke-width:{connector_width};stroke-linecap:round;fill:none}}
#{id} .commit-label{{font-size:{commit_size};fill:{muted}}}
#{id} .commit-label-bkg{{fill:{node}}}
#{id} .tag-label{{font-size:{tag_size};fill:{callout_text}}}
#{id} .tag-label-bkg{{fill:{callout};stroke:{border};stroke-width:{border_width}}}
#{id} .tag-hole,#{id} .commit-merge,#{id} .commit-highlight-inner{{fill:{node}}}
#{id} .commit-reverse{{stroke:{node};stroke-width:2;fill:none}}
#{id} .gitTitleText{{font-family:{title_font};font-size:{title_size}px;font-weight:{title_weight};fill:{title}}}"#,
        font = theme.body_font,
        size = typography.font_size_css(),
        weight = theme.label_weight,
        primary = theme.primary,
        connector_width = fmt(theme.connector_width),
        border_width = fmt(theme.stroke_width),
        commit_size = typography.commit_label_font_size_css(),
        tag_size = typography.tag_label_font_size_css(),
        muted = theme.muted,
        node = theme.node,
        callout = theme.callout,
        callout_text = theme.callout_text,
        border = theme.border,
        title_font = theme.title_font,
        title_size = fmt(typography.title_font_size_px()),
        title_weight = theme.title_weight,
        title = theme.title,
    );
    for slot in 0..theme.data.ink.len() {
        let _ = write!(
            out,
            r#"#{id} .branch{slot},#{id} .arrow{slot}{{stroke:{ink}}}
#{id} .commit{slot},#{id} .commit-highlight{slot}{{fill:{ink};stroke:{ink}}}
#{id} .commit{slot}.commit-merge,#{id} .commit{slot}.commit-highlight-inner{{fill:{node}}}
#{id} .commit{slot}.commit-reverse{{stroke:{node};fill:none}}
#{id} .branchLabelBkg.label{slot}{{fill:{card}}}
#{id} .branch-label{slot}{{fill:{card_ink}}}"#,
            ink = theme.data.ink(slot),
            node = theme.node,
            card = theme.data.fill(slot),
            card_ink = theme.data.fill_ink(slot),
        );
        out.checkpoint()?;
    }
    out.checkpoint()
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
    measurer
        .measure_svg_tspan_text_bbox_height_px(text, style)
        .max(0.0)
}

fn include_gitgraph_label_ink(
    bounds: &mut Option<crate::geom::Box2>,
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &crate::text::TextStyle,
    origin: crate::geom::Point,
    width: f64,
    transform: crate::geom::Transform,
) {
    if text.trim().is_empty() || style.font_size <= 0.0 {
        return;
    }
    // Backgrounds use fixed baseline offsets and need not contain large text. Mermaid's
    // final getBBox() includes that ink. Profiles without baseline facts retain their
    // compatibility bounds instead of borrowing an unrelated font's ascent.
    let Some(top) = measurer.measure_svg_raw_text_bbox_y_offset_px(text, style) else {
        return;
    };
    let height = measurer
        .measure_svg_raw_text_bbox_height_px(text, style)
        .max(0.0);
    let ink = crate::geom::Box2::from_min_max(
        origin.x,
        origin.y + top,
        origin.x + width,
        origin.y + top + height,
    );
    let ink = crate::geom::Box2(transform.outer_transformed_box(&ink.0));
    if let Some(bounds) = bounds {
        bounds.union(ink);
    } else {
        *bounds = Some(ink);
    }
}

fn gitgraph_label_rotation(
    degrees: f64,
    pivot: crate::geom::Point,
    translation: crate::geom::Vector,
) -> crate::geom::Transform {
    crate::geom::Transform::rotation(euclid::Angle::degrees(degrees))
        .pre_translate(crate::geom::vector(-pivot.x, -pivot.y))
        .then_translate(crate::geom::vector(pivot.x, pivot.y) + translation)
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes layout, resolved styles, and accessibility text."
)]
pub(crate) fn render_gitgraph_diagram_svg_model(
    layout: &crate::model::GitGraphDiagramLayout,
    model: &merman_model::families::git_graph::GitGraphDiagramModel,
    typography_theme: &crate::gitgraph::GitGraphTypographyThemePlan,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_title = crate::gitgraph::resolve_gitgraph_title(model, diagram_title);
    let acc_title = model.acc_title.as_deref().filter(|s| !s.is_empty());
    let acc_descr = model.acc_descr.as_deref().filter(|s| !s.is_empty());

    render_gitgraph_diagram_svg_with_accessibility(
        layout,
        acc_title,
        acc_descr,
        typography_theme,
        effective_config,
        diagram_title,
        measurer,
        options,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "The SVG writer takes layout, resolved styles, and accessibility text."
)]
fn render_gitgraph_diagram_svg_with_accessibility(
    layout: &crate::model::GitGraphDiagramLayout,
    acc_title: Option<&str>,
    acc_descr: Option<&str>,
    typography_theme: &crate::gitgraph::GitGraphTypographyThemePlan,
    effective_config: &serde_json::Value,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    const PX: f64 = 4.0;
    const PY: f64 = 2.0;

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

    let use_max_width = config_bool(effective_config, &["gitGraph", "useMaxWidth"]).unwrap_or(true);
    let diagram_padding =
        config_f64(effective_config, &["gitGraph", "diagramPadding"]).unwrap_or(8.0);
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let aria_describedby = aria_desc_id.as_deref();
    let aria_labelledby = aria_title_id.as_deref();
    let root_context = options.root_viewport(crate::DiagramFamilyId::GIT_GRAPH, diagram_id);
    let root_document = root_context.begin_document(
        &mut out,
        root_svg::DeferredRootSpec::mermaid(use_max_width),
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

    let binding = typography_theme.css_binding();
    let use_redux_geometry = binding.use_redux_geometry;
    let use_dark_theme = binding.sources.use_dark_theme;
    let title = diagram_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let native = typography_theme.native();
    let css = if native.is_some() {
        GitGraphCss::default()
    } else {
        gitgraph_css(
            diagram_id.semantic_str(),
            binding,
            typography_theme,
            options.work_meter(),
        )?
    };
    out.push_str("<style>");
    {
        let mut css_out = super::super::output::XmlTextOutput::new(&mut out);
        if let Some(native) = native {
            write_gitgraph_native_css(&mut css_out, diagram_id, typography_theme, native)?;
        } else {
            css_out.push_str(&css.css);
        }
    }
    out.push_str("</style>");
    out.checkpoint()?;
    out.push_str(r#"<g/>"#);
    out.push_str(&css.defs);
    out.push_str(r#"<g class="commit-bullets"/>"#);
    out.push_str(r#"<g class="commit-labels"/>"#);
    out.checkpoint()?;

    let mut branch_idx: std::collections::HashMap<&str, i64> = std::collections::HashMap::new();
    for b in &layout.branches {
        branch_idx.insert(b.name.as_str(), b.index);
    }

    let direction = layout.direction.as_str();
    let branch_border_radius = if use_redux_geometry { 0.0 } else { 4.0 };
    let branch_label_padding_x = if use_redux_geometry { 16.0 } else { 0.0 };
    let branch_label_padding_y = if use_redux_geometry {
        crate::gitgraph::REDUX_BRANCH_LABEL_PADDING_Y
    } else {
        0.0
    };
    let branch_label_style = if binding.look_is_neo && native.is_none() {
        if use_redux_geometry {
            GitGraphBranchLabelStyle::ScopedDropShadow(diagram_id)
        } else {
            GitGraphBranchLabelStyle::Configured(binding.drop_shadow.clone())
        }
    } else {
        GitGraphBranchLabelStyle::None
    };
    let branch_data_look = if binding.look_is_neo {
        r#" data-look="neo""#
    } else {
        ""
    };
    if layout.show_branches {
        out.push_str("<g>");
        for b in &layout.branches {
            let color_index = native.map_or_else(
                || binding.color_indices.branch_index(b.index),
                |theme| theme.slot(b.index) as f64,
            );
            let idx = if let Some(theme) = native {
                Some(theme.slot(b.index))
            } else {
                binding.color_indices.branch_slot(b.index)
            };
            let pos = b.pos;
            let branch_class = format!("branch branch{color_index}");

            if direction == "TB" {
                let _ = write!(
                    &mut out,
                    r#"<line x1="{x1}" y1="30" x2="{x2}" y2="{y2}" class="{class}"/>"#,
                    x1 = fmt(pos),
                    x2 = fmt(pos),
                    y2 = fmt(layout.max_pos),
                    class = branch_class,
                );
            } else if direction == "BT" {
                let _ = write!(
                    &mut out,
                    r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="30" class="{class}"/>"#,
                    x1 = fmt(pos),
                    y1 = fmt(layout.max_pos),
                    x2 = fmt(pos),
                    class = branch_class,
                );
            } else {
                let spine_y = crate::gitgraph::gitgraph_lr_branch_spine_y(pos, use_redux_geometry);
                let _ = write!(
                    &mut out,
                    r#"<line x1="0" y1="{y1}" x2="{x2}" y2="{y2}" class="{class}"/>"#,
                    y1 = fmt(spine_y),
                    x2 = fmt(layout.max_pos),
                    y2 = fmt(spine_y),
                    class = branch_class,
                );
            }
            let rows = crate::gitgraph::gitgraph_branch_label_rows(&b.name);
            let (bbox_w, bbox_h) = if use_redux_geometry {
                // Numbered branch classes acquire noteFontWeight only after prelayout.
                let style = typography_theme.rendered_branch_label_style(idx);
                let visible_rows = rows.iter().filter(|row| !row.is_empty());
                let row_span = visible_rows.clone().count().saturating_sub(1);
                (
                    visible_rows
                        .clone()
                        .map(|row| {
                            measurer.measure_svg_preserved_tspan_text_bbox_width_px(row, &style)
                        })
                        .fold(0.0_f64, f64::max),
                    visible_rows
                        .map(|row| measurer.measure_svg_tspan_text_bbox_height_px(row, &style))
                        .fold(0.0_f64, f64::max)
                        + row_span as f64 * style.font_size,
                )
            } else {
                (b.bbox_width.max(0.0), b.bbox_height.max(0.0))
            };

            let bkg_class = format!(r#"branchLabelBkg label{color_index}"#);
            let label_class = format!(r#"label branch-label{color_index}"#);

            let (tx, ty) = if direction == "TB" {
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
                    style = branch_label_style,
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
                (tx, ty)
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
                    style = branch_label_style,
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
                (tx, ty)
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
                    style = branch_label_style,
                    radius = fmt(branch_border_radius),
                    x = fmt(x),
                    y = fmt(y),
                    w = fmt(bbox_w + 18.0 + branch_label_padding_x),
                    h = fmt(bbox_h + 4.0 + branch_label_padding_y),
                    ty = fmt(spine_y - 12.0 - branch_label_padding_y / 2.0),
                );
                let tx = -bbox_w - 14.0 - rotate_pad + branch_label_padding_x / 2.0;
                (tx, spine_y - bbox_h / 2.0 - 2.0)
            };
            let _ = write!(
                &mut out,
                r#"<g class="branchLabel"><g class="{cls}" transform="translate({x}, {y})"><text"#,
                cls = label_class,
                x = fmt(tx),
                y = fmt(ty),
            );
            out.push('>');
            for row in rows {
                let _ = write!(
                    &mut out,
                    r#"<tspan xml:space="preserve" dy="1em" x="0" class="row">{}</tspan>"#,
                    escape_xml(row),
                );
                out.checkpoint()?;
            }
            out.push_str("</text></g></g>");
            out.checkpoint()?;
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }

    out.push_str(r#"<g class="commit-arrows">"#);
    for a in &layout.arrows {
        let idx = native.map_or_else(
            || a.class_index.rem_euclid(GITGRAPH_NAMED_COLOR_COUNT as i64) as usize,
            |theme| theme.slot(a.class_index),
        );
        let _ = write!(
            &mut out,
            r#"<path d="{d}" class="arrow arrow{idx}"/>"#,
            d = escape_attr(&a.d),
            idx = idx
        );
        out.checkpoint()?;
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="commit-bullets">"#);
    for c in &layout.commits {
        let branch_i = branch_idx.get(c.branch.as_str()).copied().unwrap_or(0);
        let commit_kind = GitGraphCommitKind::from_layout(c);
        let type_class = commit_kind.class_name();
        let idx = native.map_or_else(
            || binding.color_indices.commit_slot(branch_i),
            |theme| theme.slot(branch_i),
        );
        let id = escape_attr(&c.id);

        if commit_kind == GitGraphCommitKind::Highlight {
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
        } else if commit_kind == GitGraphCommitKind::CherryPick {
            let outer_radius = if use_redux_geometry { 7.0 } else { 10.0 };
            let inner_radius = if use_redux_geometry { 2.5 } else { 2.75 };
            let native_ink = native.map(|theme| theme.data.ink(idx).to_string());
            let native_detail = native.map(|theme| theme.node.to_string());
            let cherry_pick_detail_color =
                native_detail
                    .as_deref()
                    .unwrap_or(if use_dark_theme { "#000000" } else { "#fff" });
            let fill_attr = native_ink
                .as_deref()
                .map(|color| format!(r#" fill="{color}""#))
                .unwrap_or_default();
            let _ = write!(
                &mut out,
                r#"<circle cx="{x}" cy="{y}" r="{r}" class="commit {id} {type_class}"{fill_attr}/>"#,
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
            if commit_kind == GitGraphCommitKind::Merge {
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
            }
            if commit_kind == GitGraphCommitKind::Reverse {
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
            }
        }
        out.checkpoint()?;
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="commit-labels">"#);
    let commit_label_style = typography_theme.commit_label_style();
    let tag_label_style = typography_theme.tag_label_style();
    let mut label_ink_bounds = None;
    for c in &layout.commits {
        if crate::gitgraph::gitgraph_commit_label_is_visible(layout, c) {
            let bbox_w = gitgraph_commit_tag_label_width_px(measurer, &c.id, &commit_label_style);
            let bbox_h = gitgraph_commit_tag_label_height_px(measurer, &c.id, &commit_label_style);

            let mut wrapper_transform: Option<String> = None;
            let mut rect_transform: Option<String> = None;
            let mut text_transform: Option<String> = None;
            let mut ink_transform = crate::geom::Transform::identity();

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
                    ink_transform = gitgraph_label_rotation(
                        -45.0,
                        crate::geom::point(c.x, c.y),
                        crate::geom::vector(0.0, 0.0),
                    );
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
                    ink_transform = gitgraph_label_rotation(
                        -45.0,
                        crate::geom::point(c.pos, c.y),
                        crate::geom::vector(r_x, r_y),
                    );
                }
            }
            include_gitgraph_label_ink(
                &mut label_ink_bounds,
                measurer,
                &c.id,
                &commit_label_style,
                crate::geom::point(text_x, text_y),
                bbox_w,
                ink_transform,
            );

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
        }

        if !c.tags.is_empty() {
            let mut max_w: f64 = 0.0;
            let mut max_h: f64 = 0.0;
            let mut tags = Vec::with_capacity(c.tags.len());
            for tag_value in crate::gitgraph::gitgraph_tags_in_output_order(c) {
                let bbox_w =
                    gitgraph_commit_tag_label_width_px(measurer, tag_value, &tag_label_style);
                let bbox_h =
                    gitgraph_commit_tag_label_height_px(measurer, tag_value, &tag_label_style);
                max_w = max_w.max(bbox_w.max(0.0));
                max_h = max_h.max(bbox_h.max(0.0));
                tags.push((tag_value, bbox_w));
            }

            for (i, (tag_value, bbox_w)) in tags.into_iter().enumerate() {
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
                    include_gitgraph_label_ink(
                        &mut label_ink_bounds,
                        measurer,
                        tag_value,
                        &tag_label_style,
                        crate::geom::point(c.x + 5.0, y_origin + 3.0),
                        bbox_w,
                        gitgraph_label_rotation(
                            45.0,
                            crate::geom::point(c.x, c.pos),
                            crate::geom::vector(14.0, 14.0),
                        ),
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
                        x = fmt(c.pos_with_offset - bbox_w / 2.0),
                        txt = escape_xml(tag_value)
                    );
                    include_gitgraph_label_ink(
                        &mut label_ink_bounds,
                        measurer,
                        tag_value,
                        &tag_label_style,
                        crate::geom::point(c.pos_with_offset - bbox_w / 2.0, c.y - 16.0 - y_off),
                        bbox_w,
                        crate::geom::Transform::identity(),
                    );
                }
                out.checkpoint()?;
            }
        }
        out.checkpoint()?;
    }
    out.push_str("</g>");

    let title_top_margin =
        config_f64(effective_config, &["gitGraph", "titleTopMargin"]).unwrap_or(25.0);
    let title_y = -title_top_margin;

    out.push_str("</svg>\n");
    out.checkpoint()?;

    // GitGraph renders rotated commit labels (e.g. `rotate(-45, ...)`) that are not represented
    // in the precomputed layout bounds. Mirror Mermaid's `setupGraphViewbox(svg.getBBox() + pad)`
    // by computing a headless SVG bbox and patching the root viewBox/max-width.
    let mut b = svg_emitted_bounds_from_svg(out.as_str()).unwrap_or(Bounds {
        min_x: vb_min_x,
        min_y: vb_min_y,
        max_x: vb_min_x + vb_w,
        max_y: vb_min_y + vb_h,
    });
    if let Some(ink) = label_ink_bounds {
        b.min_x = b.min_x.min(ink.min_x());
        b.min_y = b.min_y.min(ink.min_y());
        b.max_x = b.max_x.max(ink.max_x());
        b.max_y = b.max_y.max(ink.max_y());
    }
    include_gitgraph_branch_line_bounds(&mut b, layout, use_redux_geometry);
    let title_anchor_x = (b.min_x + b.max_x) / 2.0;
    if let Some(title) = title {
        let title_style = typography_theme.title_style();
        let (title_left, title_right) = measurer.measure_svg_title_bbox_x(title, &title_style);
        let (ascent, descent) = crate::text::svg_text_vertical_extents_px(
            measurer,
            title,
            &title_style,
            crate::text::svg_title_bbox_vertical_extents_px(&title_style),
        );
        let title_min_x = title_anchor_x - title_left;
        let title_max_x = title_anchor_x + title_right;
        b.min_x = b.min_x.min(title_min_x);
        b.max_x = b.max_x.max(title_max_x);
        b.min_y = b.min_y.min(title_y - ascent);
        b.max_y = b.max_y.max(title_y + descent);
    }

    let root_bounds = root_svg::DiagramBounds::from_view_box(
        b.min_x - diagram_padding,
        b.min_y - diagram_padding,
        b.max_x - b.min_x + 2.0 * diagram_padding,
        b.max_y - b.min_y + 2.0 * diagram_padding,
    );
    let root_document = root_context.finish_document(
        &mut out,
        root_document,
        root_svg::RootViewportSpec::mermaid(root_bounds, use_max_width)
            .with_max_width(root_svg::RootMaxWidth::SvgNumber(root_bounds.width)),
    )?;
    if let Some(title) = title {
        let mut title_element = String::new();
        let _ = write!(
            &mut title_element,
            r#"<text text-anchor="middle" x="{x}" y="{y}" class="gitTitleText" xmlns="http://www.w3.org/2000/svg""#,
            x = fmt_string(title_anchor_x),
            y = fmt(title_y),
        );
        let _ = write!(&mut title_element, ">{}</text>", escape_xml(title));
        let close_start =
            out.as_str()
                .rfind("</svg>")
                .ok_or_else(|| crate::Error::InvalidModel {
                    message: "gitGraph SVG is missing its closing root element".to_string(),
                })?;
        out.replace_range(close_start..close_start, &title_element)?;
    }
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;

    use crate::resources::{
        RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
    };
    use serde_json::json;
    use std::cell::RefCell;

    fn gitgraph_css(
        diagram_id: &str,
        typography: &crate::gitgraph::GitGraphTypographyThemePlan,
    ) -> GitGraphCss {
        super::gitgraph_css(
            diagram_id,
            typography.css_binding(),
            typography,
            &crate::resources::OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .unwrap()
    }

    fn typography_plan(config: &serde_json::Value) -> crate::gitgraph::GitGraphTypographyThemePlan {
        crate::gitgraph::GitGraphTypographyThemePlan::resolve(
            &crate::test_support::presentation_config(config.clone()),
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

    struct FixtureBranchMeasurer;

    impl crate::text::TextMeasurer for FixtureBranchMeasurer {
        fn measure(&self, text: &str, style: &crate::text::TextStyle) -> crate::text::TextMetrics {
            crate::text::TextMeasurer::measure(
                &crate::text::DeterministicTextMeasurer::default(),
                text,
                style,
            )
        }

        fn measure_svg_preserved_tspan_text_bbox_width_px(
            &self,
            text: &str,
            style: &crate::text::TextStyle,
        ) -> f64 {
            if text == "main" {
                35.25
            } else {
                crate::text::TextMeasurer::measure_svg_preserved_tspan_text_bbox_width_px(
                    &crate::text::DeterministicTextMeasurer::default(),
                    text,
                    style,
                )
            }
        }

        fn measure_svg_tspan_text_bbox_height_px(
            &self,
            text: &str,
            style: &crate::text::TextStyle,
        ) -> f64 {
            if text == "main" {
                19.0
            } else {
                crate::text::TextMeasurer::measure_svg_tspan_text_bbox_height_px(
                    &crate::text::DeterministicTextMeasurer::default(),
                    text,
                    style,
                )
            }
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
        let typography_theme = typography_plan(config);
        render_gitgraph_diagram_svg_with_accessibility(
            layout,
            None,
            None,
            &typography_theme,
            config,
            None,
            &FixtureBranchMeasurer,
            &execution,
        )
    }

    #[test]
    fn gitgraph_signed_padding_retains_root_geometry_validation() {
        let layout = lr_merge_layout(-2.0);
        for padding in [f64::MAX, -f64::MAX] {
            let error = match render_geometry_fixture_with_policy(
                &layout,
                &json!({"gitGraph": {"diagramPadding": padding}}),
                RenderResourcePolicy::unbounded_for_trusted_input(),
            ) {
                Ok(_) => panic!("overflowing padding must fail root SVG validation"),
                Err(error) => error,
            };
            assert!(error.to_string().contains("must be finite"), "{error}");
        }
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
    fn gitgraph_palette_css_reports_svg_byte_limit_before_work_limit() {
        let layout = lr_merge_layout(-2.0);
        for (theme, use_gradient) in [("redux-color", false), ("neo", true)] {
            let config = json!({
                "theme": theme,
                "themeVariables": {
                    "THEME_COLOR_LIMIT": 1_000_000_000,
                    "useGradient": use_gradient
                }
            });
            // Both cases are bounded even if CSS generation fails to stop at the byte ceiling.
            // Neo exhausts bytes inside its gradient loop; Redux does so in the outer loop.
            let max_svg_bytes = 16_384;
            let policy = RenderResourcePolicy::interactive()
                .with_limit(ResourceLimitId::MaxSvgBytes, max_svg_bytes)
                .expect("valid GitGraph SVG byte ceiling")
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1_000)
                .expect("valid GitGraph work ceiling");
            let error = render_geometry_fixture_with_policy(&layout, &config, policy)
                .expect_err("enormous GitGraph palette must fail at the SVG byte ceiling");
            let crate::Error::ResourceLimitExceeded(limit) = error else {
                panic!("expected {theme} MaxSvgBytes rejection, got {error}");
            };
            assert_eq!(limit.cause, ResourceLimitCause::Ceiling, "{theme}");
            assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput, "{theme}");
            assert_eq!(
                limit.limit,
                ResourceLimitId::MaxSvgBytes.as_str(),
                "{theme}"
            );
            assert_eq!(limit.max, max_svg_bytes, "{theme}");
            assert!(limit.actual > limit.max, "{theme}");
        }
    }

    #[test]
    fn commit_and_tag_measurements_route_raw_width_and_shape_specific_height() {
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

            fn measure_svg_tspan_text_bbox_height_px(
                &self,
                _text: &str,
                _style: &crate::text::TextStyle,
            ) -> f64 {
                22.5
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
            let larger_style = crate::text::TextStyle {
                font_size: 18.0,
                ..style.clone()
            };
            assert_eq!(
                gitgraph_commit_tag_label_height_px(&OperationProbe, label, &larger_style),
                22.5
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
        let model = merman_model::families::git_graph::GitGraphDiagramModel {
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
        let config = json!({});
        let typography_theme = typography_plan(&config);
        let svg = with_test_svg_execution(DiagramFamilyId::GIT_GRAPH, &request, |options| {
            render_gitgraph_diagram_svg_model(
                &layout,
                &model,
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
                "fontFamily": "GitGraphMeasure",
                "fontSize": "23px",
                "commitLabelFontSize": "inherit",
                "tagLabelFontSize": "150%"
            }
        });
        let typography_theme = typography_plan(&config);
        let css = gitgraph_css("git-measure", &typography_theme).css;
        assert!(css.contains(
            "#git-measure .commit-id,#git-measure .commit-msg,#git-measure .branch-label{fill:lightgrey;color:lightgrey;font-family:'trebuchet ms',verdana,arial,sans-serif;font-family:GitGraphMeasure;}"
        ));
        let measurer = RecordingMeasurer::default();
        let request = SvgRenderOptions::default();

        with_test_svg_execution(DiagramFamilyId::GIT_GRAPH, &request, |options| {
            render_gitgraph_diagram_svg_with_accessibility(
                &layout,
                None,
                None,
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
        let css = gitgraph_css("git", &typography_theme).css;

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
        let css = gitgraph_css("git", &typography_theme);

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
    fn prepared_palette_preserves_selector_range_and_filtered_array_order() {
        let config = json!({
            "theme": "redux-color",
            "themeVariables": {
                "THEME_COLOR_LIMIT": 64.8,
                "borderColorArray": [null, "red", 7, "blue"],
                "nodeBorder": "black"
            }
        });
        let typography = typography_plan(&config);
        let css = gitgraph_css("git", &typography);
        assert!(css.css.contains("#git .commit1{stroke:blue;fill:blue;}"));
        assert!(css.css.contains("#git .commit63{stroke:blue;fill:blue;}"));
        assert!(css.css.contains("#git .commit64{stroke:red;fill:red;}"));
        assert!(!css.css.contains("#git .commit65{"));

        let config = json!({"themeVariables": {"THEME_COLOR_LIMIT": 8.9, "git0": "var(--slot0)"}});
        let typography = typography_plan(&config);
        let css = gitgraph_css("git", &typography);
        assert!(
            css.css
                .contains("#git .commit0{stroke:var(--slot0);fill:var(--slot0);}")
        );
        assert!(css.css.contains("#git .commit7{"));
        assert!(!css.css.contains("#git .commit8{"));
        assert!(!css.css.contains("#git .commit9{"));
    }

    #[test]
    fn prepared_gradient_preserves_missing_and_authored_empty_start() {
        let config = json!({
            "look": "neo", "theme": "redux",
            "themeVariables": {"useGradient": true, "filterColor": "var(--shadow)"}
        });
        let typography = typography_plan(&config);
        let defs = gitgraph_css("git", &typography).defs;
        assert!(defs.contains("stop-color=\"#9370DB\""));
        assert!(defs.contains("flood-color=\"var(--shadow)\""));

        let config = json!({
            "theme": "neo",
            "themeVariables": {"useGradient": true, "gradientStart": "", "gradientStop": "currentColor"}
        });
        let typography = typography_plan(&config);
        let defs = gitgraph_css("git", &typography).defs;
        assert!(defs.contains("stop-color=\"\""));
        assert!(defs.contains("stop-color=\"currentColor\""));
        assert!(!defs.contains("<filter"));
    }

    #[test]
    fn prepared_gradient_preserves_coerced_boolean_defs_and_css() {
        for (value, enabled) in [
            (json!(true), true),
            (json!("true"), true),
            (json!(" YES "), true),
            (json!("on"), true),
            (json!("1"), true),
            (json!(2), true),
            (json!(-1), true),
            (json!(false), false),
            (json!("false"), false),
            (json!(" OFF "), false),
            (json!("no"), false),
            (json!("0"), false),
            (json!(0), false),
            (json!("unknown"), false),
            (json!(1.5), false),
            (json!(null), false),
        ] {
            let config = json!({
                "theme": "neo",
                "themeVariables": {"useGradient": value, "mainBkg": "var(--background)"}
            });
            let typography = typography_plan(&config);
            let css = gitgraph_css("git", &typography);
            assert_eq!(css.defs.contains("<linearGradient"), enabled, "{value}");
            assert_eq!(
                css.css.contains("#git .label11{fill:var(--background);stroke:url(#git-gradient);stroke-width:1;}"),
                enabled,
                "{value}",
            );
        }
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
        let css = gitgraph_css("git", &typography_theme).css;

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
        let css = gitgraph_css("git", &typography_theme);

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
        let config = json!({
            "theme": "base",
            "themeVariables": {
                "useGradient": true,
                "gradientStart": "#112233",
                "gradientStop": "#445566"
            }
        });
        let typography_theme = typography_plan(&config);
        let request = SvgRenderOptions::default();
        let svg = with_test_svg_execution(DiagramFamilyId::GIT_GRAPH, &request, |options| {
            render_gitgraph_diagram_svg_with_accessibility(
                &layout,
                None,
                None,
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
