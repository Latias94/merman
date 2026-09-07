//! Renderer-neutral GitGraph adapter.
//!
//! GitGraph's layout already contains branch lanes, commit positions, routed parent arrows, and
//! the measurements needed by its labels. This adapter expands the remaining SVG conveniences
//! (CSS classes, marker-like commit symbols, and label polygons) into ordinary DrawingList
//! resources and commands. Unsupported CSS effects fail closed instead of disappearing.

use super::{
    GitGraphSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
    parse_svg_path,
};
use crate::config::{
    config_bool, config_css_number_or_string, config_diagram_look, config_f64, config_f64_css_px,
    config_font_family_css, config_string, config_string_vec,
};
use crate::drawing_list::flowchart::{polygon_path, rounded_rect_path};
use crate::drawing_list::support::{PortableStyleResolver, stroke, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::gitgraph::gitgraph_theme_is_redux_geometry;
use crate::model::{Bounds, GitGraphBranchLayout, GitGraphCommitLayout, GitGraphDiagramLayout};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::{Error, Result};
use merman_core::diagrams::git_graph::GitGraphRenderModel;
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, GradientSpread,
    GradientStop, LineCap, LineJoin, Paint, PathResource, PathSegment, PathStyle, Point, Rect,
    ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle, TextAnchor, TextBaseline,
    TextDirection, TextObligation, TextRun, TextStyle, Transform, Viewport,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

type GitGraphPair = FamilyPair<GitGraphRenderModel, GitGraphDiagramLayout>;

const GITGRAPH_NAMED_COLOR_COUNT: usize = 8;
const DEFAULT_COMMIT_LABEL_FONT_SIZE: f64 = 10.0;
const DEFAULT_TAG_LABEL_FONT_SIZE: f64 = 10.0;
const TITLE_FONT_SIZE: f64 = 18.0;
const VIEWBOX_PADDING: f64 = 8.0;

pub(crate) fn build_gitgraph_document(
    pair: &GitGraphPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    GitGraphBuilder::new(pair, metadata, policy, session)?.build()
}

#[derive(Debug, Clone)]
struct GitGraphTheme {
    theme_name: String,
    use_color_theme: bool,
    use_color_generation: bool,
    use_neo: bool,
    use_dark: bool,
    use_redux_geometry: bool,
    branch_stroke: Color,
    branch_width: f64,
    branch_dash: Vec<f64>,
    arrow_width: f64,
    commit_label_color: Color,
    commit_label_background: Color,
    commit_label_background_opacity: f64,
    tag_label_color: Color,
    tag_label_background: Color,
    tag_label_border: Color,
    text_color: Color,
    main_background: Color,
    node_border: Color,
    state_fill: Color,
    reverse_width: f64,
    note_weight: u16,
    font_size: f64,
    commit_label_font_size: f64,
    tag_label_font_size: f64,
    use_gradient: bool,
    gradient_start: Color,
    gradient_stop: Color,
    branch_colors: Vec<Color>,
    branch_label_colors: Vec<Color>,
    inverse_colors: Vec<Color>,
    border_colors: Vec<Color>,
}

impl GitGraphTheme {
    fn from_config(config: &Value) -> Result<Self> {
        let styles = PortableStyleResolver::new("gitGraph");
        let theme_name = config_string(config, &["theme"]).unwrap_or_else(|| "default".into());
        let use_color_theme = matches!(theme_name.as_str(), "redux-color" | "redux-dark-color");
        let use_neo = matches!(theme_name.as_str(), "neo" | "neo-dark");
        let use_dark = matches!(
            theme_name.as_str(),
            "dark" | "redux-dark" | "redux-dark-color" | "neo-dark"
        );
        let use_color_generation = matches!(
            theme_name.as_str(),
            "redux" | "redux-dark" | "redux-color" | "redux-dark-color" | "neo" | "neo-dark"
        );
        let use_redux_geometry = gitgraph_theme_is_redux_geometry(&theme_name);

        if config_diagram_look(config).is_neo() {
            let drop_shadow =
                config_css_number_or_string(config, &["themeVariables", "dropShadow"])
                    .unwrap_or_else(|| "none".into());
            if !drop_shadow.trim().is_empty() && !drop_shadow.eq_ignore_ascii_case("none") {
                return Err(unavailable(
                    "GitGraph neo drop shadows require an explicit filter or raster fallback",
                ));
            }
        }

        let color =
            |key: &str, fallback: &str| styles.color(key, &theme_token(config, key, fallback));
        let branch_stroke = color(
            "commitLineColor",
            &theme_token(config, "lineColor", "#333333"),
        )?;
        let branch_width = css_length(
            &styles,
            "strokeWidth",
            &config_css_number_or_string(config, &["themeVariables", "strokeWidth"])
                .unwrap_or_else(|| "1".into()),
        )?;
        let arrow_width = if use_redux_geometry {
            branch_width
        } else {
            8.0
        };
        let reverse_width = if use_color_generation {
            branch_width
        } else {
            3.0
        };
        let branch_dash = if use_color_generation {
            vec![4.0, 2.0]
        } else {
            vec![2.0]
        };
        let note_weight = parse_font_weight(
            &config_css_number_or_string(config, &["themeVariables", "noteFontWeight"])
                .unwrap_or_else(|| "normal".into()),
        );
        let font_size = config_f64_css_px(config, &["themeVariables", "fontSize"])
            .unwrap_or(16.0)
            .max(1.0);
        let commit_label_font_size = css_font_size(
            config_string(config, &["themeVariables", "commitLabelFontSize"]),
            DEFAULT_COMMIT_LABEL_FONT_SIZE,
        );
        let tag_label_font_size = css_font_size(
            config_string(config, &["themeVariables", "tagLabelFontSize"]),
            DEFAULT_TAG_LABEL_FONT_SIZE,
        );
        let use_gradient = config_bool(config, &["themeVariables", "useGradient"]).unwrap_or(false);
        let gradient_start = color(
            "gradientStart",
            &theme_token(
                config,
                "primaryBorderColor",
                &theme_token(config, "nodeBorder", "#9370DB"),
            ),
        )?;
        let gradient_stop = color(
            "gradientStop",
            &theme_token(
                config,
                "secondaryBorderColor",
                &theme_token(config, "nodeBorder", "#9370DB"),
            ),
        )?;

        let mut branch_colors = Vec::with_capacity(GITGRAPH_NAMED_COLOR_COUNT);
        let mut branch_label_colors = Vec::with_capacity(GITGRAPH_NAMED_COLOR_COUNT);
        let mut inverse_colors = Vec::with_capacity(GITGRAPH_NAMED_COLOR_COUNT);
        for index in 0..GITGRAPH_NAMED_COLOR_COUNT {
            let ci = index % GITGRAPH_NAMED_COLOR_COUNT;
            branch_colors.push(styles.color(
                &format!("git{ci}"),
                &theme_token(config, &format!("git{ci}"), default_git_color(ci)),
            )?);
            branch_label_colors.push(styles.color(
                &format!("gitBranchLabel{ci}"),
                &theme_token(
                    config,
                    &format!("gitBranchLabel{ci}"),
                    default_git_branch_label(ci),
                ),
            )?);
            inverse_colors.push(styles.color(
                &format!("gitInv{ci}"),
                &theme_token(config, &format!("gitInv{ci}"), default_git_inverse(ci)),
            )?);
        }

        let border_colors = config_string_vec(config, &["themeVariables", "borderColorArray"])
            .iter()
            .enumerate()
            .map(|(index, value)| styles.color(&format!("borderColorArray[{index}]"), value))
            .collect::<Result<Vec<_>>>()?;

        let text_color = color("textColor", "#333")?;
        let main_background = color("mainBkg", "#ECECFF")?;
        let primary_color = color("primaryColor", "#ECECFF")?;
        let node_border = color("nodeBorder", "#9370DB")?;
        let state_fill = if use_color_generation {
            main_background
        } else {
            primary_color
        };

        Ok(Self {
            theme_name,
            use_color_theme,
            use_color_generation,
            use_neo,
            use_dark,
            use_redux_geometry,
            branch_stroke,
            branch_width,
            branch_dash,
            arrow_width,
            commit_label_color: color("commitLabelColor", "#000021")?,
            commit_label_background: color("commitLabelBackground", "#ffffde")?,
            commit_label_background_opacity: if use_color_generation { 0.0 } else { 0.5 },
            tag_label_color: color("tagLabelColor", "#131300")?,
            tag_label_background: if use_color_generation {
                main_background
            } else {
                color("tagLabelBackground", "#ECECFF")?
            },
            tag_label_border: if use_color_generation {
                node_border
            } else {
                color("tagLabelBorder", "hsl(240, 60%, 86.2745098039%)")?
            },
            text_color,
            main_background,
            node_border,
            state_fill,
            reverse_width,
            note_weight,
            font_size,
            commit_label_font_size,
            tag_label_font_size,
            use_gradient,
            gradient_start,
            gradient_stop,
            branch_colors,
            branch_label_colors,
            inverse_colors,
            border_colors,
        })
    }

    fn branch_index(&self, raw: i64) -> usize {
        let raw = raw.rem_euclid(GITGRAPH_NAMED_COLOR_COUNT as i64) as usize;
        if self.use_color_theme && raw > 0 {
            ((raw - 1) % (GITGRAPH_NAMED_COLOR_COUNT - 1)) + 1
        } else {
            raw
        }
    }

    fn branch_color(&self, raw: i64) -> Color {
        self.branch_colors[self.branch_index(raw)]
    }

    fn branch_label_color(&self, raw: i64) -> Color {
        let index = self.branch_index(raw);
        if !self.use_color_generation {
            return self.branch_label_colors[index];
        }
        if self.use_neo && !self.use_color_theme && index > 0 {
            self.branch_label_colors[index]
        } else {
            self.node_border
        }
    }

    fn branch_background(&self, raw: i64) -> (Color, Color, f64) {
        if self.use_color_generation {
            (self.main_background, self.node_border, self.branch_width)
        } else {
            (self.branch_color(raw), Color::rgba(0, 0, 0, 0), 0.0)
        }
    }

    fn highlight_outer(&self, raw: i64) -> Color {
        let index = self.branch_index(raw);
        if !self.use_color_generation {
            return self.inverse_colors[index];
        }
        if self.use_neo {
            if index == 0 {
                self.node_border
            } else {
                self.inverse_colors[index]
            }
        } else if self.use_color_theme && index > 0 {
            self.border_colors
                .get(index % self.border_colors.len().max(1))
                .copied()
                .unwrap_or(self.node_border)
        } else {
            self.node_border
        }
    }

    fn commit_color(&self, raw: i64) -> Color {
        let index = self.branch_index(raw);
        if !self.use_color_generation {
            return self.branch_colors[index];
        }
        if self.use_neo {
            if index == 0 {
                self.node_border
            } else {
                self.branch_colors[index]
            }
        } else if self.use_color_theme && index > 0 {
            self.border_colors
                .get(index % self.border_colors.len().max(1))
                .copied()
                .unwrap_or(self.node_border)
        } else {
            self.node_border
        }
    }
}

struct GitGraphBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a GitGraphRenderModel,
    layout: &'a GitGraphDiagramLayout,
    theme: GitGraphTheme,
    font: FontDescriptor,
    text_obligation: TextObligation,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    dom_ids: BTreeMap<String, String>,
}

#[derive(Clone, Copy)]
struct TextEmitSpec {
    origin: Point,
    font_size: f64,
    bounds_height: f64,
    color: Color,
    weight: u16,
    anchor: TextAnchor,
    baseline: TextBaseline,
    rotation: Option<Transform>,
}

impl<'a> GitGraphBuilder<'a> {
    fn new(
        pair: &'a GitGraphPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout)?;
        let theme = GitGraphTheme::from_config(config)?;
        let font = FontDescriptor {
            families: parse_font_families(config_font_family_css(config)),
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };
        Ok(Self {
            metadata,
            session,
            policy,
            model,
            layout,
            theme,
            font,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "gitgraph.document".into(),
                },
            ],
            semantics: Vec::new(),
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            dom_ids: BTreeMap::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let title = self
            .model
            .title
            .clone()
            .or_else(|| self.metadata.title.clone());
        self.semantics.push(SemanticAnnotation {
            id: "gitgraph.document".into(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| title.clone())
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        });

        self.emit_branches()?;
        self.emit_arrows()?;
        self.emit_commits()?;
        if let Some(title) = title.as_deref().filter(|title| !title.trim().is_empty()) {
            self.emit_title(title)?;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);

        let viewport_bounds = self.viewport_bounds(title.as_ref())?;
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(viewport_bounds),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-gitgraph".into(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "direction": self.layout.direction,
                    "text_mode": "plain_host_text",
                    "rotate_commit_label": self.layout.rotate_commit_label,
                    "show_branches": self.layout.show_branches,
                    "show_commit_label": self.layout.show_commit_label,
                    "parallel_commits": self.layout.parallel_commits,
                    "theme": self.theme.theme_name,
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::GitGraph,
                body: SvgStructureBody::GitGraph(GitGraphSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                    dom_ids: self.dom_ids,
                }),
            },
        })
    }

    fn branch_label_weight(&self) -> u16 {
        if self.theme.use_redux_geometry {
            self.theme.note_weight
        } else {
            400
        }
    }

    fn content_bounds(&self) -> Result<Bounds> {
        let root = self
            .layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("GitGraph layout did not provide root bounds"))?;
        let mut bounds = root.clone();

        if self.layout.show_branches {
            for branch in &self.layout.branches {
                let line = branch_line(self.layout, branch, self.theme.use_redux_geometry);
                include_line_bounds(&mut bounds, line, self.theme.branch_width / 2.0);

                let (rect, text_origin) = branch_label_geometry(
                    self.layout,
                    branch,
                    self.theme.use_redux_geometry,
                    self.theme.font_size,
                );
                let (_, _, border_width) = self.theme.branch_background(branch.index);
                include_rect_bounds(&mut bounds, rect, None, border_width / 2.0);
                let style = MeasurementTextStyle {
                    font_family: Some(self.font.families.join(", ")),
                    font_size: self.theme.font_size,
                    font_weight: Some(self.branch_label_weight().to_string()),
                    font_style: None,
                };
                let measurer = self
                    .session
                    .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
                let width = measurer
                    .measure_svg_raw_text_bbox_width_px(&branch.name, &style)
                    .max(1.0);
                include_rect_bounds(
                    &mut bounds,
                    text_bounds(
                        text_origin,
                        width,
                        branch.bbox_height.max(1.0),
                        TextAnchor::Start,
                        TextBaseline::Alphabetic,
                    ),
                    None,
                    0.0,
                );
            }
        }

        for arrow in &self.layout.arrows {
            let segments = parse_svg_path(&arrow.d)?;
            include_path_bounds(&mut bounds, &segments, None, self.theme.arrow_width / 2.0);
        }

        for commit in &self.layout.commits {
            include_commit_symbol_bounds(&mut bounds, &self.theme, commit);

            if should_show_commit_label(commit, self.layout.show_commit_label) {
                let style = MeasurementTextStyle {
                    font_family: Some(self.font.families.join(", ")),
                    font_size: self.theme.commit_label_font_size,
                    font_weight: Some(
                        if self.theme.use_color_generation {
                            self.theme.note_weight
                        } else {
                            400
                        }
                        .to_string(),
                    ),
                    font_style: None,
                };
                let measurer = self
                    .session
                    .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
                let width = measurer
                    .measure_svg_raw_text_bbox_width_px(&commit.id, &style)
                    .max(1.0);
                let height = measurer
                    .measure_svg_simple_text_bbox_height_px(&commit.id, &style)
                    .max(1.0);
                let (rect, origin, rotation) =
                    commit_label_geometry(self.layout, commit, width, height);
                include_rect_bounds(&mut bounds, rect, rotation, 0.0);
                include_rect_bounds(
                    &mut bounds,
                    text_bounds_transformed(
                        origin,
                        width,
                        height,
                        TextAnchor::Start,
                        TextBaseline::Alphabetic,
                        rotation,
                    ),
                    None,
                    0.0,
                );
            }

            if !commit.tags.is_empty() {
                let style = MeasurementTextStyle {
                    font_family: Some(self.font.families.join(", ")),
                    font_size: self.theme.tag_label_font_size,
                    font_weight: None,
                    font_style: None,
                };
                let measurer = self
                    .session
                    .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
                let mut tags = commit.tags.clone();
                tags.reverse();
                let mut max_width: f64 = 0.0;
                let mut max_height: f64 = 0.0;
                for tag in &tags {
                    max_width = max_width.max(
                        measurer
                            .measure_svg_raw_text_bbox_width_px(tag, &style)
                            .max(0.0),
                    );
                    let height = if self.theme.tag_label_font_size <= 10.0 {
                        measurer.measure_svg_simple_text_bbox_height_px(tag, &style)
                    } else {
                        crate::text::svg_wrapped_first_line_bbox_height_px(&style)
                    };
                    max_height = max_height.max(height.max(0.0));
                }
                for (index, tag) in tags.iter().enumerate() {
                    let geometry = tag_geometry(
                        self.layout,
                        commit,
                        max_width,
                        max_height,
                        index as f64 * 20.0,
                    );
                    include_path_bounds(
                        &mut bounds,
                        &polygon_path(&geometry.polygon),
                        geometry.shape_transform,
                        0.5,
                    );
                    include_circle_bounds(
                        &mut bounds,
                        geometry.hole,
                        1.5,
                        geometry.shape_transform,
                        0.0,
                    );
                    let width = measurer
                        .measure_svg_raw_text_bbox_width_px(tag, &style)
                        .max(1.0);
                    include_rect_bounds(
                        &mut bounds,
                        text_bounds_transformed(
                            geometry.origin,
                            width,
                            max_height.max(1.0),
                            TextAnchor::Start,
                            TextBaseline::Alphabetic,
                            geometry.text_transform,
                        ),
                        None,
                        0.0,
                    );
                }
            }
        }

        Ok(bounds)
    }

    fn viewport_bounds(&self, title: Option<&String>) -> Result<Rect> {
        let content = self.content_bounds()?;
        let mut min_x = content.min_x;
        let mut min_y = content.min_y;
        let mut max_x = content.max_x;
        let mut max_y = content.max_y;
        if let Some(title) = title.filter(|title| !title.trim().is_empty()) {
            let style = MeasurementTextStyle {
                font_family: Some(self.font.families.join(", ")),
                font_size: TITLE_FONT_SIZE,
                font_weight: None,
                font_style: None,
            };
            let measurer = self
                .session
                .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
            let width = measurer
                .measure_svg_raw_text_bbox_width_px(title, &style)
                .max(1.0);
            let height = measurer
                .measure_svg_simple_text_bbox_height_px(title, &style)
                .max(1.0);
            let center = (content.min_x + content.max_x) / 2.0;
            let title_y = -title_top_margin(self.metadata.effective_config.as_value());
            let title_bounds = text_bounds_transformed(
                Point::new(center, title_y),
                width,
                height,
                TextAnchor::Middle,
                TextBaseline::Alphabetic,
                None,
            );
            min_x = min_x.min(title_bounds.x);
            min_y = min_y.min(title_bounds.y);
            max_x = max_x.max(title_bounds.x + title_bounds.width);
            max_y = max_y.max(title_bounds.y + title_bounds.height);
        }
        Ok(Rect::new(
            min_x - VIEWBOX_PADDING,
            min_y - VIEWBOX_PADDING,
            (max_x - min_x + 2.0 * VIEWBOX_PADDING).max(1.0),
            (max_y - min_y + 2.0 * VIEWBOX_PADDING).max(1.0),
        ))
    }

    fn emit_branches(&mut self) -> Result<()> {
        if !self.layout.show_branches {
            return Ok(());
        }
        for (index, branch) in self.layout.branches.iter().enumerate() {
            let semantic_id = format!("gitgraph.branch.{index}");
            let branch_class_index = theme_class_index(branch.index);
            self.semantic_classes
                .insert(semantic_id.clone(), "branchLabel".to_string());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            let branch_line = branch_line(self.layout, branch, self.theme.use_redux_geometry);
            let branch_line_id = format!("{semantic_id}.line");
            self.path_classes.insert(
                branch_line_id.clone(),
                format!("branch branch{branch_class_index}"),
            );
            self.add_path(
                branch_line_id,
                line_path(branch_line),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(StrokeStyle {
                        paint: Paint::solid(self.theme.branch_stroke),
                        width: self.theme.branch_width,
                        dash_array: self.theme.branch_dash.clone(),
                        dash_offset: 0.0,
                        line_cap: LineCap::Butt,
                        line_join: LineJoin::Miter,
                        miter_limit: 4.0,
                    }),
                },
            )?;

            let (rect, text_origin) = branch_label_geometry(
                self.layout,
                branch,
                self.theme.use_redux_geometry,
                self.theme.font_size,
            );
            let (fill, border, border_width) = self.theme.branch_background(branch.index);
            let branch_path_id = format!("{semantic_id}.label.background");
            self.path_classes.insert(
                branch_path_id.clone(),
                format!("branchLabelBkg label{branch_class_index}"),
            );
            let gradient_id = if self.theme.use_neo && self.theme.use_gradient {
                // Mermaid's GitGraph gradient is objectBoundingBox, so every branch label gets
                // the same color ramp relative to its own box.  The public DrawingList uses
                // document-space gradients; emit one resource per label to preserve that visual
                // meaning for all renderer-neutral hosts instead of reusing the first label's
                // coordinates for every branch.
                let gradient_id = if index == 0 {
                    ResourceId::new("gitgraph.gradient")
                } else {
                    ResourceId::new(format!("gitgraph.gradient.{index}"))
                };
                self.resources
                    .push(DrawingResource::LinearGradient(gradient_resource(
                        gradient_id.clone(),
                        rect,
                        self.theme.gradient_start,
                        self.theme.gradient_stop,
                    )));
                Some(gradient_id)
            } else {
                None
            };
            // Mermaid applies the neo gradient to the label border; the label background remains
            // the resolved theme color.  The SVG serializer intentionally omits the legacy CSS
            // gradient rule for canonical documents, so these inline paints remain authoritative.
            let background_paint = Paint::solid(fill);
            let stroke_style = (border_width > 0.0).then(|| {
                gradient_id.map_or_else(
                    || stroke(border, border_width),
                    |id| StrokeStyle {
                        paint: Paint::resource(id),
                        ..stroke(border, border_width)
                    },
                )
            });
            self.add_path(
                branch_path_id,
                rounded_rect_path(
                    rect.x + rect.width / 2.0,
                    rect.y + rect.height / 2.0,
                    rect.width,
                    rect.height,
                    if self.theme.use_redux_geometry {
                        0.0
                    } else {
                        4.0
                    },
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(background_paint),
                    stroke: stroke_style,
                },
            )?;
            self.emit_text(
                &format!("{semantic_id}.label"),
                &branch.name,
                TextEmitSpec {
                    origin: text_origin,
                    font_size: self.theme.font_size,
                    bounds_height: branch.bbox_height.max(1.0),
                    color: self.theme.branch_label_color(branch.index),
                    weight: self.branch_label_weight(),
                    anchor: TextAnchor::Start,
                    baseline: TextBaseline::Alphabetic,
                    rotation: None,
                },
            )?;
            self.text_classes.insert(
                format!("{semantic_id}.label"),
                format!("label branch-label{branch_class_index}"),
            );
            self.semantic_classes.insert(
                format!("{semantic_id}.label"),
                format!("label branch-label{branch_class_index}"),
            );
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Group,
                title: Some(branch.name.clone()),
                description: Some("Git branch".into()),
                link: None,
            });
        }
        Ok(())
    }

    fn emit_arrows(&mut self) -> Result<()> {
        for (index, arrow) in self.layout.arrows.iter().enumerate() {
            let semantic_id = format!("gitgraph.arrow.{index}");
            let arrow_class_index = theme_class_index(arrow.class_index);
            self.semantic_classes
                .insert(semantic_id.clone(), "commit-arrows".to_string());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            let segments = parse_svg_path(&arrow.d)?;
            let arrow_path_id = format!("{semantic_id}.path");
            self.path_classes.insert(
                arrow_path_id.clone(),
                format!("arrow arrow{arrow_class_index}"),
            );
            self.add_path(
                arrow_path_id,
                segments,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(StrokeStyle {
                        paint: Paint::solid(self.theme.commit_color(arrow.class_index)),
                        width: self.theme.arrow_width,
                        dash_array: Vec::new(),
                        dash_offset: 0.0,
                        line_cap: LineCap::Round,
                        line_join: LineJoin::Round,
                        miter_limit: 4.0,
                    }),
                },
            )?;
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Edge,
                title: Some(format!("{} → {}", arrow.from, arrow.to)),
                description: Some("Git parent relationship".into()),
                link: None,
            });
        }
        Ok(())
    }

    fn emit_commits(&mut self) -> Result<()> {
        for (index, commit) in self.layout.commits.iter().enumerate() {
            let semantic_id = format!("gitgraph.commit.{index}");
            self.semantic_classes
                .insert(semantic_id.clone(), "commit-bullets".to_string());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            self.emit_commit_symbol(&semantic_id, commit)?;
            if should_show_commit_label(commit, self.layout.show_commit_label) {
                self.emit_commit_label(&semantic_id, commit)?;
            }
            self.emit_commit_tags(&semantic_id, commit)?;
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(commit.id.clone()),
                description: (!commit.message.is_empty()).then(|| commit.message.clone()),
                link: None,
            });
        }
        Ok(())
    }

    fn emit_commit_symbol(
        &mut self,
        semantic_id: &str,
        commit: &GitGraphCommitLayout,
    ) -> Result<()> {
        let branch_color = self
            .theme
            .commit_color(branch_index_for_commit(self.layout, commit));
        let symbol_type = commit.custom_type.unwrap_or(commit.commit_type);
        let class_index = theme_class_index(branch_index_for_commit(self.layout, commit));
        let radius = if self.theme.use_redux_geometry {
            7.0
        } else {
            10.0
        };
        match symbol_type {
            2 => {
                let outer = if self.theme.use_redux_geometry {
                    7.0
                } else {
                    10.0
                };
                let inner = if self.theme.use_redux_geometry {
                    4.0
                } else {
                    6.0
                };
                let outer_id = format!("{semantic_id}.highlight.outer");
                self.path_classes.insert(
                    outer_id.clone(),
                    format!("commit commit{class_index} commit-highlight commit-highlight-outer"),
                );
                self.add_rect_path(
                    outer_id,
                    Rect::new(commit.x - outer, commit.y - outer, outer * 2.0, outer * 2.0),
                    Some(
                        self.theme
                            .highlight_outer(branch_index_for_commit(self.layout, commit)),
                    ),
                    Some(stroke(
                        self.theme
                            .highlight_outer(branch_index_for_commit(self.layout, commit)),
                        1.0,
                    )),
                )?;
                let inner_id = format!("{semantic_id}.highlight.inner");
                self.path_classes.insert(
                    inner_id.clone(),
                    format!("commit commit{class_index} commit-highlight commit-highlight-inner"),
                );
                self.add_rect_path(
                    inner_id,
                    Rect::new(commit.x - inner, commit.y - inner, inner * 2.0, inner * 2.0),
                    Some(self.theme.state_fill),
                    Some(stroke(self.theme.state_fill, 1.0)),
                )?;
            }
            1 => {
                let circle_id = format!("{semantic_id}.reverse.circle");
                self.path_classes.insert(
                    circle_id.clone(),
                    format!("commit commit{class_index} commit-reverse"),
                );
                self.add_circle(
                    circle_id,
                    Point::new(commit.x, commit.y),
                    radius,
                    Some(branch_color),
                    Some(stroke(branch_color, 1.0)),
                )?;
                let cross = if self.theme.use_redux_geometry {
                    4.0
                } else {
                    5.0
                };
                let cross_id = format!("{semantic_id}.reverse.cross");
                self.path_classes.insert(
                    cross_id.clone(),
                    format!("commit commit{class_index} commit-reverse"),
                );
                self.add_path(
                    cross_id,
                    vec![
                        PathSegment::MoveTo {
                            to: Point::new(commit.x - cross, commit.y - cross),
                        },
                        PathSegment::LineTo {
                            to: Point::new(commit.x + cross, commit.y + cross),
                        },
                        PathSegment::MoveTo {
                            to: Point::new(commit.x - cross, commit.y + cross),
                        },
                        PathSegment::LineTo {
                            to: Point::new(commit.x + cross, commit.y - cross),
                        },
                    ],
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: None,
                        stroke: Some(StrokeStyle {
                            paint: Paint::solid(self.theme.state_fill),
                            width: self.theme.reverse_width,
                            dash_array: Vec::new(),
                            dash_offset: 0.0,
                            line_cap: LineCap::Butt,
                            line_join: LineJoin::Miter,
                            miter_limit: 4.0,
                        }),
                    },
                )?;
            }
            3 => {
                let outer_id = format!("{semantic_id}.merge.outer");
                self.path_classes.insert(
                    outer_id.clone(),
                    format!("commit commit{class_index} commit-merge"),
                );
                self.add_circle(
                    outer_id,
                    Point::new(commit.x, commit.y),
                    radius,
                    Some(branch_color),
                    Some(stroke(branch_color, 1.0)),
                )?;
                let inner_id = format!("{semantic_id}.merge.inner");
                self.path_classes.insert(
                    inner_id.clone(),
                    format!("commit commit{class_index} commit-merge"),
                );
                self.add_circle(
                    inner_id,
                    Point::new(commit.x, commit.y),
                    if self.theme.use_redux_geometry {
                        5.0
                    } else {
                        6.0
                    },
                    Some(self.theme.state_fill),
                    Some(stroke(self.theme.state_fill, 1.0)),
                )?;
            }
            4 => self.emit_cherry_pick_symbol(semantic_id, commit, branch_color, radius)?,
            _ => {
                let circle_id = format!("{semantic_id}.circle");
                self.path_classes.insert(
                    circle_id.clone(),
                    format!("commit commit{class_index} commit-normal"),
                );
                self.add_circle(
                    circle_id,
                    Point::new(commit.x, commit.y),
                    radius,
                    Some(branch_color),
                    Some(stroke(branch_color, 1.0)),
                )?;
            }
        }
        Ok(())
    }

    fn emit_cherry_pick_symbol(
        &mut self,
        semantic_id: &str,
        commit: &GitGraphCommitLayout,
        branch_color: Color,
        radius: f64,
    ) -> Result<()> {
        let detail = if self.theme.use_dark {
            Color::rgba(0, 0, 0, 255)
        } else {
            Color::rgba(255, 255, 255, 255)
        };
        let class_index = theme_class_index(branch_index_for_commit(self.layout, commit));
        let outer_id = format!("{semantic_id}.cherry-pick.outer");
        self.path_classes.insert(
            outer_id.clone(),
            format!("commit commit{class_index} commit-cherry-pick"),
        );
        self.add_circle(
            outer_id,
            Point::new(commit.x, commit.y),
            radius,
            Some(branch_color),
            Some(stroke(branch_color, 1.0)),
        )?;
        for (suffix, dx, dy) in [("left", -3.0, 2.0), ("right", 3.0, 2.0)] {
            let circle_id = format!("{semantic_id}.cherry-pick.{suffix}");
            self.path_classes.insert(
                circle_id.clone(),
                format!("commit commit{class_index} commit-cherry-pick"),
            );
            self.add_circle(
                circle_id,
                Point::new(commit.x + dx, commit.y + dy),
                if self.theme.use_redux_geometry {
                    2.5
                } else {
                    2.75
                },
                Some(detail),
                None,
            )?;
        }
        for (suffix, x1, y1, x2, y2) in [
            ("left-stem", 3.0, 1.0, 0.0, -5.0),
            ("right-stem", -3.0, 1.0, 0.0, -5.0),
        ] {
            let stem_id = format!("{semantic_id}.cherry-pick.{suffix}");
            self.path_classes.insert(
                stem_id.clone(),
                format!("commit commit{class_index} commit-cherry-pick"),
            );
            self.add_path(
                stem_id,
                line_path(Line {
                    x1: commit.x + x1,
                    y1: commit.y + y1,
                    x2: commit.x + x2,
                    y2: commit.y + y2,
                }),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(detail, 1.0)),
                },
            )?;
        }
        Ok(())
    }

    fn emit_commit_label(
        &mut self,
        semantic_id: &str,
        commit: &GitGraphCommitLayout,
    ) -> Result<()> {
        let style = MeasurementTextStyle {
            font_family: Some(self.font.families.join(", ")),
            font_size: self.theme.commit_label_font_size,
            font_weight: None,
            font_style: None,
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let width = measurer
            .measure_svg_raw_text_bbox_width_px(&commit.id, &style)
            .max(1.0);
        let height = measurer
            .measure_svg_simple_text_bbox_height_px(&commit.id, &style)
            .max(1.0);
        let (rect, origin, rotation) = commit_label_geometry(self.layout, commit, width, height);
        let background_id = format!("{semantic_id}.label.background");
        self.path_classes
            .insert(background_id.clone(), "commit-label-bkg".to_string());
        self.emit_label_background(
            &background_id,
            rect,
            self.theme.commit_label_background,
            self.theme.commit_label_background_opacity,
            rotation,
        )?;
        let text_id = format!("{semantic_id}.label");
        self.text_classes
            .insert(text_id.clone(), "commit-label".to_string());
        self.semantic_classes
            .insert(text_id.clone(), "commit-label".to_string());
        self.emit_text(
            &text_id,
            &commit.id,
            TextEmitSpec {
                origin,
                font_size: self.theme.commit_label_font_size,
                bounds_height: height,
                color: if self.theme.use_color_generation {
                    self.theme.node_border
                } else {
                    self.theme.commit_label_color
                },
                weight: if self.theme.use_color_generation {
                    self.theme.note_weight
                } else {
                    400
                },
                anchor: TextAnchor::Start,
                baseline: TextBaseline::Alphabetic,
                rotation,
            },
        )?;
        Ok(())
    }

    fn emit_commit_tags(&mut self, semantic_id: &str, commit: &GitGraphCommitLayout) -> Result<()> {
        if commit.tags.is_empty() {
            return Ok(());
        }
        let style = MeasurementTextStyle {
            font_family: Some(self.font.families.join(", ")),
            font_size: self.theme.tag_label_font_size,
            font_weight: None,
            font_style: None,
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let mut tags = commit.tags.clone();
        tags.reverse();
        let mut max_width: f64 = 0.0;
        let mut max_height: f64 = 0.0;
        for tag in &tags {
            max_width = max_width.max(
                measurer
                    .measure_svg_raw_text_bbox_width_px(tag, &style)
                    .max(0.0),
            );
            let tag_height = if self.theme.tag_label_font_size <= 10.0 {
                measurer.measure_svg_simple_text_bbox_height_px(tag, &style)
            } else {
                crate::text::svg_wrapped_first_line_bbox_height_px(&style)
            };
            max_height = max_height.max(tag_height.max(0.0));
        }
        for (index, tag) in tags.iter().enumerate() {
            let y_offset = index as f64 * 20.0;
            let geometry = tag_geometry(self.layout, commit, max_width, max_height, y_offset);
            let tag_id = format!("{semantic_id}.tag.{index}");
            self.semantic_classes
                .insert(tag_id.clone(), "tag".to_string());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: tag_id.clone(),
            });
            if let Some(transform) = geometry.shape_transform {
                self.commands.push(DrawingCommand::Save);
                self.commands
                    .push(DrawingCommand::ConcatTransform { transform });
            }
            let background_id = format!("{tag_id}.background");
            self.path_classes
                .insert(background_id.clone(), "tag-label-bkg".to_string());
            self.add_path(
                background_id,
                polygon_path(&geometry.polygon),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.theme.tag_label_background)),
                    stroke: Some(stroke(self.theme.tag_label_border, 1.0)),
                },
            )?;
            let hole_id = format!("{tag_id}.hole");
            self.path_classes
                .insert(hole_id.clone(), "tag-hole".to_string());
            self.add_circle(
                hole_id,
                geometry.hole,
                1.5,
                Some(self.theme.text_color),
                None,
            )?;
            if geometry.shape_transform.is_some() {
                self.commands.push(DrawingCommand::Restore);
            }
            let label_id = format!("{tag_id}.label");
            self.text_classes
                .insert(label_id.clone(), "tag-label".to_string());
            self.emit_text(
                &label_id,
                tag,
                TextEmitSpec {
                    origin: geometry.origin,
                    font_size: self.theme.tag_label_font_size,
                    bounds_height: max_height.max(1.0),
                    color: self.theme.tag_label_color,
                    weight: 400,
                    anchor: TextAnchor::Start,
                    baseline: TextBaseline::Alphabetic,
                    rotation: geometry.text_transform,
                },
            )?;
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: tag_id,
                role: SemanticRole::Label,
                title: Some(tag.clone()),
                description: Some(format!("Tag for {}", commit.id)),
                link: None,
            });
        }
        Ok(())
    }

    fn emit_title(&mut self, title: &str) -> Result<()> {
        let bounds = self.content_bounds()?;
        let origin = Point::new(
            (bounds.min_x + bounds.max_x) / 2.0,
            -title_top_margin(self.metadata.effective_config.as_value()),
        );
        self.text_classes
            .insert("gitgraph.title".to_string(), "gitTitleText".to_string());
        self.semantic_classes
            .insert("gitgraph.title".to_string(), "gitTitleText".to_string());
        self.emit_text(
            "gitgraph.title",
            title,
            TextEmitSpec {
                origin,
                font_size: TITLE_FONT_SIZE,
                bounds_height: TITLE_FONT_SIZE,
                color: self.theme.text_color,
                weight: 400,
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Alphabetic,
                rotation: None,
            },
        )?;
        Ok(())
    }

    fn emit_label_background(
        &mut self,
        id: &str,
        rect: Rect,
        fill: Color,
        opacity: f64,
        rotation: Option<Transform>,
    ) -> Result<()> {
        if opacity <= 0.0 {
            return Ok(());
        }
        self.commands.push(DrawingCommand::Save);
        if let Some(transform) = rotation {
            self.commands
                .push(DrawingCommand::ConcatTransform { transform });
        }
        self.commands.push(DrawingCommand::SetOpacity { opacity });
        self.add_path(
            id.to_string(),
            rounded_rect_path(
                rect.x + rect.width / 2.0,
                rect.y + rect.height / 2.0,
                rect.width,
                rect.height,
                0.0,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(fill)),
                stroke: None,
            },
        )?;
        self.commands.push(DrawingCommand::Restore);
        Ok(())
    }

    fn emit_text(&mut self, id: &str, text: &str, spec: TextEmitSpec) -> Result<()> {
        let TextEmitSpec {
            origin,
            font_size,
            bounds_height,
            color,
            weight,
            anchor,
            baseline,
            rotation,
        } = spec;
        if text.is_empty() {
            return Ok(());
        }
        let style = MeasurementTextStyle {
            font_family: Some(self.font.families.join(", ")),
            font_size: font_size.max(1.0),
            font_weight: Some(weight.to_string()),
            font_style: None,
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let width = measurer
            .measure_svg_raw_text_bbox_width_px(text, &style)
            .max(1.0);
        let bounds = text_bounds(origin, width, bounds_height.max(1.0), anchor, baseline);
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: id.into(),
        });
        if let Some(transform) = rotation {
            self.commands.push(DrawingCommand::Save);
            self.commands
                .push(DrawingCommand::ConcatTransform { transform });
        }
        self.commands.push(DrawingCommand::DrawText {
            run: TextRun {
                text: text.into(),
                origin,
                bounds,
                style: TextStyle {
                    font: FontDescriptor {
                        weight,
                        ..self.font.clone()
                    },
                    font_size: font_size.max(1.0),
                    letter_spacing: 0.0,
                    line_height: font_size.max(1.0),
                    fill: Paint::solid(color),
                    stroke: None,
                    paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                },
                anchor,
                baseline,
                direction: TextDirection::Auto,
                language: None,
                obligation: self.text_obligation.clone(),
            },
        });
        if rotation.is_some() {
            self.commands.push(DrawingCommand::Restore);
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: id.into(),
            role: SemanticRole::Label,
            title: Some(text.into()),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn add_circle(
        &mut self,
        id: String,
        center: Point,
        radius: f64,
        fill: Option<Color>,
        stroke_style: Option<StrokeStyle>,
    ) -> Result<()> {
        self.add_path(
            id,
            ellipse_path(center.x, center.y, radius, radius),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: fill.map(Paint::solid),
                stroke: stroke_style,
            },
        )
    }

    fn add_rect_path(
        &mut self,
        id: String,
        rect: Rect,
        fill: Option<Color>,
        stroke_style: Option<StrokeStyle>,
    ) -> Result<()> {
        self.add_path(
            id,
            rounded_rect_path(
                rect.x + rect.width / 2.0,
                rect.y + rect.height / 2.0,
                rect.width,
                rect.height,
                0.0,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: fill.map(Paint::solid),
                stroke: stroke_style,
            },
        )
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid("GitGraph path has no geometry"));
        }
        let id = ResourceId::new(id);
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        self.commands
            .push(DrawingCommand::DrawPath { path: id, style });
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct Line {
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
}

fn line_path(line: Line) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo {
            to: Point::new(line.x1, line.y1),
        },
        PathSegment::LineTo {
            to: Point::new(line.x2, line.y2),
        },
    ]
}

fn branch_line(layout: &GitGraphDiagramLayout, branch: &GitGraphBranchLayout, redux: bool) -> Line {
    match layout.direction.as_str() {
        "TB" => Line {
            x1: branch.pos,
            y1: 30.0,
            x2: branch.pos,
            y2: layout.max_pos,
        },
        "BT" => Line {
            x1: branch.pos,
            y1: layout.max_pos,
            x2: branch.pos,
            y2: 30.0,
        },
        _ => Line {
            x1: 0.0,
            y1: crate::gitgraph::gitgraph_lr_branch_spine_y(branch.pos, redux),
            x2: layout.max_pos,
            y2: crate::gitgraph::gitgraph_lr_branch_spine_y(branch.pos, redux),
        },
    }
}

fn branch_label_geometry(
    layout: &GitGraphDiagramLayout,
    branch: &GitGraphBranchLayout,
    redux: bool,
    font_size: f64,
) -> (Rect, Point) {
    let bbox_width = branch.bbox_width.max(0.0);
    let bbox_height = branch.bbox_height.max(1.0);
    let rotate_padding = if layout.rotate_commit_label {
        30.0
    } else {
        0.0
    };
    match layout.direction.as_str() {
        "TB" => (
            Rect::new(
                branch.pos - bbox_width / 2.0 - 10.0 - if redux { 11.0 } else { 0.0 },
                if redux { -22.0 } else { 0.0 },
                bbox_width + 18.0 + if redux { 16.0 } else { 0.0 },
                bbox_height + 4.0 + if redux { 12.0 } else { 0.0 },
            ),
            Point::new(
                branch.pos - bbox_width / 2.0 - 5.0,
                (if redux { -17.0 } else { 0.0 }) + font_size,
            ),
        ),
        "BT" => (
            Rect::new(
                branch.pos - bbox_width / 2.0 - 10.0 - if redux { 11.0 } else { 0.0 },
                layout.max_pos + if redux { 22.0 } else { 0.0 },
                bbox_width + 18.0 + if redux { 16.0 } else { 0.0 },
                bbox_height + 4.0 + if redux { 12.0 } else { 0.0 },
            ),
            Point::new(
                branch.pos - bbox_width / 2.0 - 5.0,
                layout.max_pos + (if redux { 28.0 } else { 0.0 }) + font_size,
            ),
        ),
        _ => {
            let spine_y = crate::gitgraph::gitgraph_lr_branch_spine_y(branch.pos, redux);
            let padding_x = if redux { 16.0 } else { 0.0 };
            let padding_y = if redux { 12.0 } else { 0.0 };
            let x = -bbox_width - 4.0 - rotate_padding - 19.0;
            let y = spine_y - bbox_height / 2.0 - 2.0 - padding_y / 2.0;
            (
                Rect::new(
                    x,
                    y,
                    bbox_width + 18.0 + padding_x,
                    bbox_height + 4.0 + padding_y,
                ),
                Point::new(
                    -bbox_width - 14.0 - rotate_padding + padding_x / 2.0,
                    spine_y - bbox_height / 2.0 - 2.0 + font_size,
                ),
            )
        }
    }
}

fn commit_label_geometry(
    layout: &GitGraphDiagramLayout,
    commit: &GitGraphCommitLayout,
    width: f64,
    height: f64,
) -> (Rect, Point, Option<Transform>) {
    let direction_is_axis = matches!(layout.direction.as_str(), "TB" | "BT");
    let (rect, origin) = if direction_is_axis {
        (
            Rect::new(
                commit.x - (width + 21.0),
                commit.y - 12.0,
                width + 4.0,
                height + 4.0,
            ),
            Point::new(commit.x - (width + 16.0), commit.y + height - 12.0),
        )
    } else {
        (
            Rect::new(
                commit.pos_with_offset - width / 2.0 - 2.0,
                commit.y + 13.5,
                width + 4.0,
                height + 4.0,
            ),
            Point::new(commit.pos_with_offset - width / 2.0, commit.y + 25.0),
        )
    };
    let rotation = layout.rotate_commit_label.then(|| {
        if direction_is_axis {
            rotation_transform(-45.0, commit.x, commit.y)
        } else {
            let rx = -7.5 - ((width + 10.0) / 25.0) * 9.5;
            let ry = 10.0 + (width / 25.0) * 8.5;
            compose_transform(
                translation_transform(rx, ry),
                rotation_transform(-45.0, commit.pos, commit.y),
            )
        }
    });
    (rect, origin, rotation)
}

struct TagGeometry {
    polygon: Vec<Point>,
    hole: Point,
    origin: Point,
    shape_transform: Option<Transform>,
    text_transform: Option<Transform>,
}

fn tag_geometry(
    layout: &GitGraphDiagramLayout,
    commit: &GitGraphCommitLayout,
    max_width: f64,
    max_height: f64,
    y_offset: f64,
) -> TagGeometry {
    let h2 = max_height / 2.0;
    if matches!(layout.direction.as_str(), "TB" | "BT") {
        let y_origin = commit.pos + y_offset;
        let points = vec![
            Point::new(commit.x, y_origin + 2.0),
            Point::new(commit.x, y_origin - 2.0),
            Point::new(commit.x + 10.0, y_origin - h2 - 2.0),
            Point::new(commit.x + 10.0 + max_width + 4.0, y_origin - h2 - 2.0),
            Point::new(commit.x + 10.0 + max_width + 4.0, y_origin + h2 + 2.0),
            Point::new(commit.x + 10.0, y_origin + h2 + 2.0),
        ];
        let rotation = compose_transform(
            translation_transform(12.0, 12.0),
            rotation_transform(45.0, commit.x, commit.pos),
        );
        TagGeometry {
            polygon: points,
            hole: Point::new(commit.x + 2.0, y_origin),
            origin: Point::new(commit.x + 5.0, y_origin + 3.0),
            shape_transform: Some(rotation),
            text_transform: Some(compose_transform(
                translation_transform(14.0, 14.0),
                rotation_transform(45.0, commit.x, commit.pos),
            )),
        }
    } else {
        let ly = commit.y - 19.2 - y_offset;
        TagGeometry {
            polygon: vec![
                Point::new(commit.pos - max_width / 2.0 - 2.0, ly + 2.0),
                Point::new(commit.pos - max_width / 2.0 - 2.0, ly - 2.0),
                Point::new(
                    commit.pos_with_offset - max_width / 2.0 - 4.0,
                    ly - h2 - 2.0,
                ),
                Point::new(
                    commit.pos_with_offset + max_width / 2.0 + 4.0,
                    ly - h2 - 2.0,
                ),
                Point::new(
                    commit.pos_with_offset + max_width / 2.0 + 4.0,
                    ly + h2 + 2.0,
                ),
                Point::new(
                    commit.pos_with_offset - max_width / 2.0 - 4.0,
                    ly + h2 + 2.0,
                ),
            ],
            hole: Point::new(commit.pos - max_width / 2.0 + 2.0, ly),
            origin: Point::new(
                commit.pos_with_offset - max_width / 2.0,
                commit.y - 16.0 - y_offset,
            ),
            shape_transform: None,
            text_transform: None,
        }
    }
}

fn include_point_bounds(
    bounds: &mut Bounds,
    point: Point,
    transform: Option<Transform>,
    padding: f64,
) {
    let point = transform.map_or(point, |transform| apply_transform(transform, point));
    bounds.min_x = bounds.min_x.min(point.x - padding);
    bounds.min_y = bounds.min_y.min(point.y - padding);
    bounds.max_x = bounds.max_x.max(point.x + padding);
    bounds.max_y = bounds.max_y.max(point.y + padding);
}

fn include_rect_bounds(
    bounds: &mut Bounds,
    rect: Rect,
    transform: Option<Transform>,
    padding: f64,
) {
    for point in [
        Point::new(rect.x, rect.y),
        Point::new(rect.x + rect.width, rect.y),
        Point::new(rect.x + rect.width, rect.y + rect.height),
        Point::new(rect.x, rect.y + rect.height),
    ] {
        include_point_bounds(bounds, point, transform, padding);
    }
}

fn include_circle_bounds(
    bounds: &mut Bounds,
    center: Point,
    radius: f64,
    transform: Option<Transform>,
    padding: f64,
) {
    include_rect_bounds(
        bounds,
        Rect::new(
            center.x - radius,
            center.y - radius,
            radius * 2.0,
            radius * 2.0,
        ),
        transform,
        padding,
    );
}

fn include_line_bounds(bounds: &mut Bounds, line: Line, padding: f64) {
    include_point_bounds(bounds, Point::new(line.x1, line.y1), None, padding);
    include_point_bounds(bounds, Point::new(line.x2, line.y2), None, padding);
}

fn include_path_bounds(
    bounds: &mut Bounds,
    segments: &[PathSegment],
    transform: Option<Transform>,
    padding: f64,
) {
    let mut current = None;
    let mut subpath_start = None;
    for segment in segments {
        match segment {
            PathSegment::MoveTo { to } => {
                include_point_bounds(bounds, *to, transform, padding);
                current = Some(*to);
                subpath_start = Some(*to);
            }
            PathSegment::LineTo { to } => {
                if let Some(from) = current {
                    include_point_bounds(bounds, from, transform, padding);
                }
                include_point_bounds(bounds, *to, transform, padding);
                current = Some(*to);
            }
            PathSegment::QuadTo { control, to } => {
                if let Some(from) = current {
                    include_point_bounds(bounds, from, transform, padding);
                }
                include_point_bounds(bounds, *control, transform, padding);
                include_point_bounds(bounds, *to, transform, padding);
                current = Some(*to);
            }
            PathSegment::CubicTo {
                control1,
                control2,
                to,
            } => {
                if let Some(from) = current {
                    include_point_bounds(bounds, from, transform, padding);
                }
                include_point_bounds(bounds, *control1, transform, padding);
                include_point_bounds(bounds, *control2, transform, padding);
                include_point_bounds(bounds, *to, transform, padding);
                current = Some(*to);
            }
            PathSegment::ArcTo {
                radius_x,
                radius_y,
                to,
                ..
            } => {
                if let Some(from) = current {
                    include_point_bounds(bounds, from, transform, padding);
                    for point in [
                        Point::new(from.x - radius_x.abs(), from.y - radius_y.abs()),
                        Point::new(from.x + radius_x.abs(), from.y + radius_y.abs()),
                    ] {
                        include_point_bounds(bounds, point, transform, padding);
                    }
                }
                include_point_bounds(bounds, *to, transform, padding);
                for point in [
                    Point::new(to.x - radius_x.abs(), to.y - radius_y.abs()),
                    Point::new(to.x + radius_x.abs(), to.y + radius_y.abs()),
                ] {
                    include_point_bounds(bounds, point, transform, padding);
                }
                current = Some(*to);
            }
            PathSegment::Close => {
                if let Some(start) = subpath_start {
                    include_point_bounds(bounds, start, transform, padding);
                }
                current = subpath_start;
            }
        }
    }
}

fn include_commit_symbol_bounds(
    bounds: &mut Bounds,
    theme: &GitGraphTheme,
    commit: &GitGraphCommitLayout,
) {
    let symbol_type = commit.custom_type.unwrap_or(commit.commit_type);
    let radius = if theme.use_redux_geometry { 7.0 } else { 10.0 };
    match symbol_type {
        2 => {
            let outer = radius;
            let inner = if theme.use_redux_geometry { 4.0 } else { 6.0 };
            include_rect_bounds(
                bounds,
                Rect::new(commit.x - outer, commit.y - outer, outer * 2.0, outer * 2.0),
                None,
                0.5,
            );
            include_rect_bounds(
                bounds,
                Rect::new(commit.x - inner, commit.y - inner, inner * 2.0, inner * 2.0),
                None,
                0.5,
            );
        }
        1 => {
            include_circle_bounds(bounds, Point::new(commit.x, commit.y), radius, None, 0.5);
            let cross = if theme.use_redux_geometry { 4.0 } else { 5.0 };
            include_path_bounds(
                bounds,
                &line_path(Line {
                    x1: commit.x - cross,
                    y1: commit.y - cross,
                    x2: commit.x + cross,
                    y2: commit.y + cross,
                }),
                None,
                theme.reverse_width / 2.0,
            );
            include_path_bounds(
                bounds,
                &line_path(Line {
                    x1: commit.x - cross,
                    y1: commit.y + cross,
                    x2: commit.x + cross,
                    y2: commit.y - cross,
                }),
                None,
                theme.reverse_width / 2.0,
            );
        }
        3 => {
            include_circle_bounds(bounds, Point::new(commit.x, commit.y), radius, None, 0.5);
            include_circle_bounds(
                bounds,
                Point::new(commit.x, commit.y),
                if theme.use_redux_geometry { 5.0 } else { 6.0 },
                None,
                0.5,
            );
        }
        4 => {
            include_circle_bounds(bounds, Point::new(commit.x, commit.y), radius, None, 0.5);
            for (dx, dy) in [(-3.0, 2.0), (3.0, 2.0)] {
                include_circle_bounds(
                    bounds,
                    Point::new(commit.x + dx, commit.y + dy),
                    if theme.use_redux_geometry { 2.5 } else { 2.75 },
                    None,
                    0.0,
                );
            }
            for (x1, y1, x2, y2) in [(3.0, 1.0, 0.0, -5.0), (-3.0, 1.0, 0.0, -5.0)] {
                include_line_bounds(
                    bounds,
                    Line {
                        x1: commit.x + x1,
                        y1: commit.y + y1,
                        x2: commit.x + x2,
                        y2: commit.y + y2,
                    },
                    0.5,
                );
            }
        }
        _ => {
            include_circle_bounds(bounds, Point::new(commit.x, commit.y), radius, None, 0.5);
        }
    }
}

fn text_bounds(
    origin: Point,
    width: f64,
    height: f64,
    anchor: TextAnchor,
    baseline: TextBaseline,
) -> Rect {
    let x = match anchor {
        TextAnchor::Start => origin.x,
        TextAnchor::Middle => origin.x - width / 2.0,
        TextAnchor::End => origin.x - width,
    };
    let y = match baseline {
        TextBaseline::Hanging | TextBaseline::TextBeforeEdge => origin.y,
        TextBaseline::Middle | TextBaseline::Central => origin.y - height / 2.0,
        TextBaseline::Alphabetic | TextBaseline::Ideographic | TextBaseline::TextAfterEdge => {
            origin.y - height
        }
    };
    Rect::new(x, y, width.max(1.0), height.max(1.0))
}

fn text_bounds_transformed(
    origin: Point,
    width: f64,
    height: f64,
    anchor: TextAnchor,
    baseline: TextBaseline,
    transform: Option<Transform>,
) -> Rect {
    let local = text_bounds(origin, width, height, anchor, baseline);
    let points = [
        Point::new(local.x, local.y),
        Point::new(local.x + local.width, local.y),
        Point::new(local.x + local.width, local.y + local.height),
        Point::new(local.x, local.y + local.height),
    ]
    .map(|point| transform.map_or(point, |transform| apply_transform(transform, point)));
    let min_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::INFINITY, f64::min);
    let max_x = points
        .iter()
        .map(|point| point.x)
        .fold(f64::NEG_INFINITY, f64::max);
    let min_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::INFINITY, f64::min);
    let max_y = points
        .iter()
        .map(|point| point.y)
        .fold(f64::NEG_INFINITY, f64::max);
    Rect::new(
        min_x,
        min_y,
        (max_x - min_x).max(1.0),
        (max_y - min_y).max(1.0),
    )
}

fn should_show_commit_label(commit: &GitGraphCommitLayout, show: bool) -> bool {
    show && match commit.commit_type {
        3 => commit.custom_id.unwrap_or(false),
        4 => false,
        _ => true,
    }
}

fn branch_index_for_commit(layout: &GitGraphDiagramLayout, commit: &GitGraphCommitLayout) -> i64 {
    layout
        .branches
        .iter()
        .find(|branch| branch.name == commit.branch)
        .map(|branch| branch.index)
        .unwrap_or(0)
}

fn theme_class_index(raw: i64) -> usize {
    raw.rem_euclid(GITGRAPH_NAMED_COLOR_COUNT as i64) as usize
}

fn gradient_resource(
    id: ResourceId,
    rect: Rect,
    start: Color,
    stop: Color,
) -> merman_display_list::LinearGradientResource {
    merman_display_list::LinearGradientResource {
        id,
        start: Point::new(rect.x, rect.y),
        end: Point::new(rect.x + rect.width, rect.y),
        transform: Transform::IDENTITY,
        spread: GradientSpread::Pad,
        stops: vec![GradientStop::new(0.0, start), GradientStop::new(1.0, stop)],
    }
}

fn translation_transform(x: f64, y: f64) -> Transform {
    Transform {
        e: x,
        f: y,
        ..Transform::IDENTITY
    }
}

fn rotation_transform(degrees: f64, cx: f64, cy: f64) -> Transform {
    let radians = degrees.to_radians();
    let cos = radians.cos();
    let sin = radians.sin();
    Transform {
        a: cos,
        b: sin,
        c: -sin,
        d: cos,
        e: cx - cos * cx + sin * cy,
        f: cy - sin * cx - cos * cy,
    }
}

fn compose_transform(first: Transform, second: Transform) -> Transform {
    Transform {
        a: first.a * second.a + first.c * second.b,
        b: first.b * second.a + first.d * second.b,
        c: first.a * second.c + first.c * second.d,
        d: first.b * second.c + first.d * second.d,
        e: first.a * second.e + first.c * second.f + first.e,
        f: first.b * second.e + first.d * second.f + first.f,
    }
}

fn apply_transform(transform: Transform, point: Point) -> Point {
    Point::new(
        transform.a * point.x + transform.c * point.y + transform.e,
        transform.b * point.x + transform.d * point.y + transform.f,
    )
}

fn validate_layout(layout: &GitGraphDiagramLayout) -> Result<()> {
    let bounds = layout
        .bounds
        .as_ref()
        .ok_or_else(|| invalid("GitGraph layout did not provide root bounds"))?;
    validate_bounds(bounds)?;
    if !matches!(layout.direction.as_str(), "LR" | "TB" | "BT") {
        return Err(invalid(format!(
            "GitGraph direction `{}` is not supported",
            layout.direction
        )));
    }
    if !layout.diagram_padding.is_finite()
        || layout.diagram_padding < 0.0
        || !layout.max_pos.is_finite()
    {
        return Err(invalid("GitGraph root metrics are invalid"));
    }
    for branch in &layout.branches {
        if ![branch.pos, branch.bbox_width, branch.bbox_height]
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0)
        {
            return Err(invalid("GitGraph branch geometry is invalid"));
        }
    }
    for commit in &layout.commits {
        if ![commit.pos, commit.pos_with_offset, commit.x, commit.y]
            .iter()
            .all(|value| value.is_finite())
        {
            return Err(invalid("GitGraph commit geometry is invalid"));
        }
    }
    for arrow in &layout.arrows {
        if arrow.d.trim().is_empty() || parse_svg_path(&arrow.d)?.is_empty() {
            return Err(invalid("GitGraph arrow path is empty"));
        }
    }
    Ok(())
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .iter()
        .all(|value| value.is_finite())
        || bounds.max_x <= bounds.min_x
        || bounds.max_y <= bounds.min_y
    {
        return Err(invalid("GitGraph bounds are invalid"));
    }
    Ok(())
}

fn title_top_margin(config: &Value) -> f64 {
    config_f64(config, &["gitGraph", "titleTopMargin"])
        .unwrap_or(25.0)
        .max(0.0)
}

fn css_length(resolver: &PortableStyleResolver, property: &str, value: &str) -> Result<f64> {
    resolver.length(property, value)
}

fn css_font_size(value: Option<String>, fallback: f64) -> f64 {
    value
        .as_deref()
        .and_then(|value| {
            value
                .trim()
                .trim_end_matches(';')
                .trim_end_matches("!important")
                .trim()
                .strip_suffix("px")
                .unwrap_or(value.trim())
                .trim()
                .parse::<f64>()
                .ok()
        })
        .filter(|value| value.is_finite() && *value > 0.0)
        .unwrap_or(fallback)
}

fn parse_font_weight(value: &str) -> u16 {
    match value.trim().to_ascii_lowercase().as_str() {
        "bold" => 700,
        "normal" => 400,
        raw => raw
            .parse::<u16>()
            .ok()
            .map(|value| value.clamp(100, 900))
            .unwrap_or(400),
    }
}

fn theme_token(config: &Value, key: &str, fallback: &str) -> String {
    config_string(config, &["themeVariables", key]).unwrap_or_else(|| fallback.into())
}

fn default_git_color(index: usize) -> &'static str {
    match index {
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

fn default_git_branch_label(index: usize) -> &'static str {
    match index {
        0 | 3 => "#ffffff",
        _ => "black",
    }
}

fn default_git_inverse(index: usize) -> &'static str {
    match index {
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

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "gitGraph".into(),
        reason: message.into(),
    }
}

fn ellipse_path(x: f64, y: f64, radius_x: f64, radius_y: f64) -> Vec<PathSegment> {
    let kappa = 0.552_284_749_8;
    vec![
        PathSegment::MoveTo {
            to: Point::new(x + radius_x, y),
        },
        PathSegment::CubicTo {
            control1: Point::new(x + radius_x, y + kappa * radius_y),
            control2: Point::new(x + kappa * radius_x, y + radius_y),
            to: Point::new(x, y + radius_y),
        },
        PathSegment::CubicTo {
            control1: Point::new(x - kappa * radius_x, y + radius_y),
            control2: Point::new(x - radius_x, y + kappa * radius_y),
            to: Point::new(x - radius_x, y),
        },
        PathSegment::CubicTo {
            control1: Point::new(x - radius_x, y - kappa * radius_y),
            control2: Point::new(x - kappa * radius_x, y - radius_y),
            to: Point::new(x, y - radius_y),
        },
        PathSegment::CubicTo {
            control1: Point::new(x + kappa * radius_x, y - radius_y),
            control2: Point::new(x + radius_x, y - kappa * radius_y),
            to: Point::new(x + radius_x, y),
        },
        PathSegment::Close,
    ]
}
