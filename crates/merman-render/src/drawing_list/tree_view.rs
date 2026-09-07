//! Renderer-neutral TreeView adapter.
//!
//! TreeView layout already owns row geometry, connector endpoints, text bounds, icon placement,
//! and highlight width growth. The public document expands its two built-in icons to paths and
//! rejects arbitrary registry SVG rather than hiding or approximating it.

use super::{
    RenderDocument, SvgStructureBody, SvgStructureSidecar, TreeViewSvgBody,
    parse_font_families_for, parse_svg_path,
};
use crate::config::config_font_family_css_raw;
use crate::drawing_list::builder::DrawingListBuilder;
use crate::drawing_list::flowchart::{polygon_path, rounded_rect_path};
use crate::drawing_list::support::{PortableStyleResolver, stroke, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, TreeViewDiagramLayout, TreeViewLineLayout, TreeViewNodeLayout};
use crate::theme::PresentationTheme;
use crate::tree_view::{
    TREE_VIEW_ICON_SIZE, TreeViewBuiltinIcon, TreeViewHighlightLayout, TreeViewRenderItem,
    tree_view_builtin_icon, tree_view_node_is_directory, tree_view_render_items,
};
use crate::{Error, Result};
use merman_core::diagrams::tree_view::TreeViewDiagramRenderModel;
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, DrawingCommand, DrawingListLimits, DrawingListPolicy, FillRule, FontDescriptor,
    FontStyle, Paint, PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation,
    SemanticRole, TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun, TextStyle,
    Transform, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type TreeViewPair = FamilyPair<TreeViewDiagramRenderModel, TreeViewDiagramLayout>;

const DIRECTORY_CLASS: &str = "treeView-node-dir";
const DESCRIPTION_CLASS: &str = "treeView-node-description";
const LINE_CLASS: &str = "treeView-node-line";
const HIGHLIGHT_BACKGROUND_CLASS: &str = "treeView-highlight-bg";

pub(crate) fn build_tree_view_document(
    pair: &TreeViewPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: DrawingListLimits,
    session: &RenderSession,
) -> Result<RenderDocument> {
    TreeViewBuilder::new(pair, metadata, policy, limits, session)?.build()
}

struct TreeViewBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    model: &'a TreeViewDiagramRenderModel,
    layout: &'a TreeViewDiagramLayout,
    text_obligation: TextObligation,
    normal_font: FontDescriptor,
    directory_font: FontDescriptor,
    description_font: FontDescriptor,
    label_font_size: f64,
    label_color: Option<Color>,
    line_color: Option<Color>,
    icon_color: Option<Color>,
    description_color: Option<Color>,
    highlight_fill: Option<Color>,
    highlight_stroke: Option<Color>,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    output: DrawingListBuilder<'a>,
}

impl<'a> TreeViewBuilder<'a> {
    fn new(
        pair: &'a TreeViewPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        limits: DrawingListLimits,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout)?;
        validate_node_effects(layout)?;

        let theme = PresentationTheme::new(config).tree_view();
        let styles = PortableStyleResolver::new("treeView");
        let label_font_size =
            styles.positive_length("treeView.labelFontSize", theme.label_font_size_css.as_str())?;
        if (label_font_size - layout.label_font_size).abs() > 1e-9 {
            return Err(unavailable(format!(
                "treeView.labelFontSize resolves to {label_font_size}px but layout used {}px",
                layout.label_font_size
            )));
        }

        let has_icons = layout.nodes.iter().any(|node| node.resolved_icon.is_some());
        let has_descriptions = layout
            .nodes
            .iter()
            .any(|node| node.description.is_some() || node_has_class(node, DESCRIPTION_CLASS));
        let has_highlights = layout
            .nodes
            .iter()
            .any(|node| crate::tree_view::is_tree_view_highlight_class(node.css_class.as_deref()));
        let font_families = parse_font_families_for(
            config_font_family_css_raw(config),
            RenderFamilyKind::TreeView,
        )?;
        let normal_font = font(font_families.clone(), 400, FontStyle::Normal);
        let directory_font = font(font_families.clone(), 700, FontStyle::Normal);
        let description_font = font(font_families, 400, FontStyle::Italic);

        let mut output = DrawingListBuilder::new(policy, limits, session);
        output.push_control(DrawingCommand::Save)?;
        output.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "treeView.document".to_string(),
        })?;

        Ok(Self {
            metadata,
            session,
            model,
            layout,
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            normal_font,
            directory_font,
            description_font,
            label_font_size,
            label_color: styles.optional_color("treeView.labelColor", &theme.label_color)?,
            line_color: styles.optional_color("treeView.lineColor", &theme.line_color)?,
            icon_color: has_icons
                .then(|| styles.color("treeView.iconColor", &theme.icon_color))
                .transpose()?,
            description_color: has_descriptions
                .then(|| {
                    styles.optional_color("treeView.descriptionColor", &theme.description_color)
                })
                .transpose()?
                .flatten(),
            highlight_fill: has_highlights
                .then(|| styles.optional_color("treeView.highlightBg", &theme.highlight_bg))
                .transpose()?
                .flatten(),
            highlight_stroke: has_highlights
                .then(|| styles.optional_color("treeView.highlightStroke", &theme.highlight_stroke))
                .transpose()?
                .flatten(),
            semantic_classes: BTreeMap::from([(
                "treeView.document".to_string(),
                "tree-view".to_string(),
            )]),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            output,
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.output.push_semantic(SemanticAnnotation {
            id: "treeView.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| self.model.title.clone())
                .or_else(|| self.metadata.title.clone())
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        })?;
        self.emit_background()?;

        for item in tree_view_render_items(self.layout) {
            self.session.checkpoint(OperationPhase::Emit)?;
            match item {
                TreeViewRenderItem::Node {
                    node_index,
                    highlight,
                } => self.emit_node(node_index, highlight.as_ref())?,
                TreeViewRenderItem::Line { line_index } => self.emit_line(line_index)?,
            }
        }

        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_control(DrawingCommand::Restore)?;
        let document = self.output.finish(
            Viewport::new(Rect::new(
                -self.layout.line_thickness / 2.0,
                0.0,
                self.layout.total_width,
                self.layout.total_height,
            )),
            BTreeMap::from([(
                "x-merman-tree-view".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "icons": "builtin_paths_only",
                    "text_mode": "layout_measured_host_text",
                    "use_max_width": self.layout.use_max_width,
                }),
            )]),
        )?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::TreeView,
                body: SvgStructureBody::TreeView(TreeViewSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.layout.use_max_width,
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                }),
            },
        })
    }

    fn emit_background(&mut self) -> Result<()> {
        let left = -self.layout.line_thickness / 2.0;
        self.path_classes.insert(
            "treeView.background".to_string(),
            "treeView-background".to_string(),
        );
        self.add_path(
            "treeView.background".to_string(),
            polygon_path(&[
                Point::new(left, 0.0),
                Point::new(left + self.layout.total_width, 0.0),
                Point::new(left + self.layout.total_width, self.layout.total_height),
                Point::new(left, self.layout.total_height),
            ]),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(Color::rgba(255, 255, 255, 255))),
                stroke: None,
            },
        )
    }

    fn emit_node(
        &mut self,
        node_index: usize,
        highlight: Option<&TreeViewHighlightLayout>,
    ) -> Result<()> {
        let node = self
            .layout
            .nodes
            .get(node_index)
            .ok_or_else(|| invalid(format!("missing TreeView node {node_index}")))?
            .clone();
        let semantic_id = format!("treeView.node.{}", node.id);
        self.semantic_classes
            .insert(semantic_id.clone(), "treeView-node".to_string());
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;

        if let Some(highlight) = highlight {
            let highlight_style = PathStyle {
                fill_rule: FillRule::NonZero,
                fill: self.highlight_fill.map(Paint::solid),
                stroke: self.highlight_stroke.map(|color| stroke(color, 1.0)),
            };
            if highlight_style.fill.is_some() || highlight_style.stroke.is_some() {
                self.path_classes.insert(
                    format!("{semantic_id}.highlight"),
                    HIGHLIGHT_BACKGROUND_CLASS.to_string(),
                );
                self.add_path(
                    format!("{semantic_id}.highlight"),
                    rounded_rect_path(
                        highlight.x + highlight.width / 2.0,
                        highlight.y + highlight.height / 2.0,
                        highlight.width,
                        highlight.height,
                        3.0,
                    ),
                    highlight_style,
                )?;
            }
        }
        if let Some(icon_name) = node.resolved_icon.as_deref() {
            let icon = tree_view_builtin_icon(icon_name).ok_or_else(|| {
                unavailable(format!(
                    "TreeView node `{}` uses non-builtin icon `{icon_name}`",
                    node.name
                ))
            })?;
            self.emit_icon(&semantic_id, &node, icon)?;
        }
        self.emit_node_label(&node)?;
        self.emit_description(&node)?;

        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: Some(node.name),
            description: node.description,
            link: None,
        })?;
        Ok(())
    }

    fn emit_icon(
        &mut self,
        semantic_id: &str,
        node: &TreeViewNodeLayout,
        icon: TreeViewBuiltinIcon,
    ) -> Result<()> {
        let Some(fill) = self.icon_color else {
            return Err(invalid("TreeView icon color was not resolved"));
        };
        let id = ResourceId::new(format!("{semantic_id}.icon"));
        self.path_classes
            .insert(id.as_str().to_string(), "treeView-node-icon".to_string());
        let segments = parse_svg_path(icon.path_data)?;
        self.output.push_control(DrawingCommand::Save)?;
        self.output.push_control(DrawingCommand::ConcatTransform {
            transform: translate(
                node.x + self.layout.padding_x,
                node.y + self.layout.padding_y,
            ),
        })?;
        let scale = TREE_VIEW_ICON_SIZE / 24.0;
        self.output.push_control(DrawingCommand::ConcatTransform {
            transform: Transform {
                a: scale,
                b: 0.0,
                c: 0.0,
                d: scale,
                e: 0.0,
                f: 0.0,
            },
        })?;
        self.output.draw_path(
            id,
            segments,
            PathStyle {
                fill_rule: if icon.even_odd {
                    FillRule::EvenOdd
                } else {
                    FillRule::NonZero
                },
                fill: Some(Paint::solid(fill)),
                stroke: None,
            },
        )?;
        self.output.push_control(DrawingCommand::Restore)?;
        Ok(())
    }

    fn emit_node_label(&mut self, node: &TreeViewNodeLayout) -> Result<()> {
        let use_description_style = node_has_class(node, DESCRIPTION_CLASS);
        let fill = if use_description_style {
            self.description_color
        } else {
            self.label_color
        };
        let Some(fill) = fill else {
            return Ok(());
        };
        let directory = tree_view_node_is_directory(node) || node_has_class(node, DIRECTORY_CLASS);
        let font = if use_description_style {
            if directory {
                FontDescriptor {
                    weight: 700,
                    ..self.description_font.clone()
                }
            } else {
                self.description_font.clone()
            }
        } else if directory {
            self.directory_font.clone()
        } else {
            self.normal_font.clone()
        };
        let semantic_id = format!("treeView.node.{}.label", node.id);
        self.semantic_classes
            .insert(semantic_id.clone(), "treeView-node-label-group".to_string());
        self.text_classes
            .insert(semantic_id.clone(), tree_view_label_classes(node));
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        self.emit_text(
            &node.name,
            Point::new(node.label_x, node.label_y),
            Rect::new(
                node.label_x,
                node.label_y - node.label_height / 2.0,
                node.label_width,
                node.label_height,
            ),
            font,
            fill,
        )?;
        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Label,
            title: Some(node.name.clone()),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_description(&mut self, node: &TreeViewNodeLayout) -> Result<()> {
        let Some(description) = node.description.as_deref() else {
            return Ok(());
        };
        let Some(fill) = self.description_color else {
            return Ok(());
        };
        let (Some(x), Some(width)) = (node.description_x, node.description_width) else {
            return Err(invalid(format!(
                "TreeView node `{}` has no description bounds",
                node.name
            )));
        };
        let semantic_id = format!("treeView.node.{}.description", node.id);
        self.semantic_classes.insert(
            semantic_id.clone(),
            "treeView-node-description-group".to_string(),
        );
        self.text_classes
            .insert(semantic_id.clone(), DESCRIPTION_CLASS.to_string());
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        self.emit_text(
            description,
            Point::new(x, node.label_y),
            Rect::new(
                x,
                node.label_y - node.label_height / 2.0,
                width,
                node.label_height,
            ),
            self.description_font.clone(),
            fill,
        )?;
        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Label,
            title: Some(description.to_string()),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_text(
        &mut self,
        text: &str,
        origin: Point,
        bounds: Rect,
        font: FontDescriptor,
        fill: Color,
    ) -> Result<()> {
        let text_obligation = self.text_obligation.clone();
        self.output.draw_host_text(text, |text| TextRun {
            text,
            origin,
            bounds,
            style: TextStyle {
                font,
                font_size: self.label_font_size,
                letter_spacing: 0.0,
                line_height: self.label_font_size,
                fill: Paint::solid(fill),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor: TextAnchor::Start,
            baseline: TextBaseline::Middle,
            direction: TextDirection::Auto,
            language: None,
            obligation: text_obligation,
        })
    }

    fn emit_line(&mut self, line_index: usize) -> Result<()> {
        let line = self
            .layout
            .lines
            .get(line_index)
            .ok_or_else(|| invalid(format!("missing TreeView line {line_index}")))?
            .clone();
        let semantic_id = format!("treeView.line.{line_index}");
        self.semantic_classes
            .insert(semantic_id.clone(), LINE_CLASS.to_string());
        self.path_classes
            .insert(format!("{semantic_id}.path"), LINE_CLASS.to_string());
        self.output
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        if line.stroke_width > 0.0
            && let Some(color) = self.line_color
        {
            self.add_path(
                format!("{semantic_id}.path"),
                line_path(&line),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(color, line.stroke_width)),
                },
            )?;
        }
        self.output.push_control(DrawingCommand::EndSemanticGroup)?;
        self.output.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Edge,
            title: None,
            description: Some(format!("{} tree connector", line.kind)),
            link: None,
        })?;
        Ok(())
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() || (style.fill.is_none() && style.stroke.is_none()) {
            return Ok(());
        }
        self.output.draw_path(ResourceId::new(id), segments, style)
    }
}

fn validate_node_effects(layout: &TreeViewDiagramLayout) -> Result<()> {
    for node in &layout.nodes {
        if let Some(icon) = node.resolved_icon.as_deref()
            && tree_view_builtin_icon(icon).is_none()
        {
            return Err(unavailable(format!(
                "TreeView node `{}` uses non-builtin icon `{icon}` whose SVG subtree is not portable",
                node.name
            )));
        }
        if node_has_class(node, LINE_CLASS) || node_has_class(node, HIGHLIGHT_BACKGROUND_CLASS) {
            return Err(unavailable(format!(
                "TreeView node `{}` applies a built-in class that requires text stroke semantics",
                node.name
            )));
        }
    }
    Ok(())
}

fn validate_layout(layout: &TreeViewDiagramLayout) -> Result<()> {
    if ![
        layout.total_width,
        layout.total_height,
        layout.row_indent,
        layout.padding_x,
        layout.padding_y,
        layout.line_thickness,
        layout.label_font_size,
    ]
    .into_iter()
    .all(f64::is_finite)
        || layout.total_width < 0.0
        || layout.total_height < 0.0
        || layout.row_indent < 0.0
        || layout.padding_x < 0.0
        || layout.padding_y < 0.0
        || layout.line_thickness < 0.0
        || layout.label_font_size <= 0.0
    {
        return Err(invalid("TreeView root geometry is invalid"));
    }
    if let Some(bounds) = &layout.bounds {
        validate_bounds(bounds)?;
    }
    for node in &layout.nodes {
        if ![
            node.x,
            node.y,
            node.width,
            node.height,
            node.label_x,
            node.label_y,
            node.label_width,
            node.label_height,
        ]
        .into_iter()
        .all(f64::is_finite)
            || node.width < 0.0
            || node.height < 0.0
            || node.label_width < 0.0
            || node.label_height < 0.0
            || node.description_x.is_some_and(|value| !value.is_finite())
            || node
                .description_width
                .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(invalid("TreeView node geometry is invalid"));
        }
    }
    for line in &layout.lines {
        if ![line.x1, line.y1, line.x2, line.y2, line.stroke_width]
            .into_iter()
            .all(f64::is_finite)
            || line.stroke_width < 0.0
        {
            return Err(invalid("TreeView line geometry is invalid"));
        }
    }
    Ok(())
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x < bounds.min_x
        || bounds.max_y < bounds.min_y
    {
        return Err(invalid("TreeView root bounds are invalid"));
    }
    Ok(())
}

fn node_has_class(node: &TreeViewNodeLayout, expected: &str) -> bool {
    node.css_class
        .as_deref()
        .is_some_and(|classes| classes.split_whitespace().any(|class| class == expected))
}

fn tree_view_label_classes(node: &TreeViewNodeLayout) -> String {
    let mut classes = vec!["treeView-node-label".to_string()];
    if tree_view_node_is_directory(node) {
        classes.push(DIRECTORY_CLASS.to_string());
    }
    if let Some(css_class) = node.css_class.as_deref() {
        classes.extend(
            css_class
                .split_whitespace()
                .filter(|class| !class.is_empty())
                .map(str::to_string),
        );
    }
    classes.join(" ")
}

fn line_path(line: &TreeViewLineLayout) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo {
            to: Point::new(line.x1, line.y1),
        },
        PathSegment::LineTo {
            to: Point::new(line.x2, line.y2),
        },
    ]
}

fn font(families: Vec<String>, weight: u16, style: FontStyle) -> FontDescriptor {
    FontDescriptor {
        families,
        weight,
        style,
        postscript_name: None,
        resource: None,
    }
}

fn translate(x: f64, y: f64) -> Transform {
    Transform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: x,
        f: y,
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "treeView".to_string(),
        reason: message.into(),
    }
}
