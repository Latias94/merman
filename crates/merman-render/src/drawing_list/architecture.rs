//! Renderer-neutral Architecture diagram adapter.
//!
//! Architecture layout is owned by the Cytoscape/FCoSE adapter.  This module only projects that
//! typed result into portable paths and text.  It deliberately admits the built-in Mermaid icon
//! set, while external Iconify content and HTML `iconText` fail closed because neither has a
//! renderer-neutral representation in DrawingList v1.

use super::{
    RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families_for, theme_color,
};
use crate::architecture_metrics::{
    ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX, ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX,
    architecture_create_text_middle_bbox_y_range_px, architecture_svg_group_bbox_padding_px,
};
use crate::config::{
    config_diagram_look, config_f64, config_string, config_theme_font_size_css_or_root_number_px,
};
use crate::drawing_list::flowchart::{ellipse_path, polygon_path, rounded_rect_path};
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{ArchitectureDiagramLayout, Bounds, LayoutEdge, LayoutNode};
use crate::text::{TextMeasurer, TextStyle as MeasurementTextStyle};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::architecture::{
    ArchitectureDiagramRenderModel, ArchitectureRenderEdge, ArchitectureRenderGroup,
    ArchitectureRenderNode, ArchitectureRenderNodeType,
};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Transform, Viewport,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

type ArchitecturePair = FamilyPair<ArchitectureDiagramRenderModel, ArchitectureDiagramLayout>;

/// SVG-only metadata retained beside the renderer-neutral Architecture document.
#[derive(Debug, Clone)]
pub(crate) struct ArchitectureSvgBody {
    pub(crate) diagram_type: String,
    pub(crate) use_max_width: bool,
    pub(crate) acc_title: Option<String>,
    pub(crate) acc_description: Option<String>,
    pub(crate) semantic_classes: BTreeMap<String, String>,
    pub(crate) path_classes: BTreeMap<String, String>,
    pub(crate) text_classes: BTreeMap<String, String>,
    pub(crate) dom_ids: BTreeMap<String, String>,
}

pub(crate) fn build_architecture_document(
    pair: &ArchitecturePair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    ArchitectureBuilder::new(pair, metadata, policy, session)?.build()
}

struct ArchitectureBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a ArchitectureDiagramRenderModel,
    layout: &'a ArchitectureDiagramLayout,
    icon_size: f64,
    padding: f64,
    arch_font_size: f64,
    font_size: f64,
    font: FontDescriptor,
    text_obligation: TextObligation,
    text_color: Color,
    edge_color: Color,
    arrow_color: Color,
    group_color: Color,
    edge_width: f64,
    group_width: f64,
    nodes_by_id: HashMap<&'a str, &'a LayoutNode>,
    model_nodes_by_id: HashMap<&'a str, &'a ArchitectureRenderNode>,
    group_bounds: HashMap<&'a str, Bounds>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    dom_ids: BTreeMap<String, String>,
    content_bounds: Option<Bounds>,
}

struct MultilineTextSpec {
    origin: Point,
    fill: Color,
    anchor: TextAnchor,
    baseline: TextBaseline,
    max_width: f64,
    include_in_root_bounds: bool,
}

impl<'a> ArchitectureBuilder<'a> {
    fn new(
        pair: &'a ArchitecturePair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        if config_diagram_look(config)
            .as_str()
            .eq_ignore_ascii_case("handDrawn")
        {
            return Err(unavailable(
                "hand-drawn Architecture output has no stable vector equivalent in DrawingList v1",
            ));
        }
        let model = pair.semantic();
        let layout = pair.layout();
        let bounds = layout
            .bounds
            .as_ref()
            .ok_or_else(|| invalid("Architecture layout did not provide root bounds"))?;
        validate_bounds(bounds)?;
        if layout.edges.len() != model.edges.len() {
            return Err(invalid(format!(
                "Architecture layout has {} edges for {} semantic edges",
                layout.edges.len(),
                model.edges.len()
            )));
        }

        let icon_size = config_f64(config, &["architecture", "iconSize"])
            .unwrap_or(80.0)
            .max(1.0);
        let padding = config_f64(config, &["architecture", "padding"])
            .unwrap_or(40.0)
            .max(0.0);
        let arch_font_size = config_f64(config, &["architecture", "fontSize"])
            .unwrap_or(16.0)
            .max(1.0);
        let font_size = config_theme_font_size_css_or_root_number_px(config, 16.0).max(1.0);
        let font_family = config_string(config, &["themeVariables", "fontFamily"])
            .or_else(|| config_string(config, &["fontFamily"]))
            .unwrap_or_else(|| "Arial, sans-serif".to_string());
        let font = FontDescriptor {
            families: parse_font_families_for(font_family, RenderFamilyKind::Architecture)?,
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };
        let text_color = theme_color(config, "textColor", "#333333")?;
        let edge_color = theme_color(config, "archEdgeColor", "#333333")?;
        let arrow_color = theme_color(config, "archEdgeArrowColor", "#333333")?;
        let group_color = theme_color(config, "archGroupBorderColor", "#dadaf3")?;
        let resolver = PortableStyleResolver::new("architecture");
        let edge_width = config_css_length(config, &["themeVariables", "archEdgeWidth"])
            .map(|value| resolver.positive_length("archEdgeWidth", &value))
            .transpose()?
            .unwrap_or(3.0);
        let group_width = config_css_length(config, &["themeVariables", "archGroupBorderWidth"])
            .map(|value| resolver.positive_length("archGroupBorderWidth", &value))
            .transpose()?
            .unwrap_or(2.0);

        let nodes_by_id = layout
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let model_nodes_by_id = model
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node))
            .collect::<HashMap<_, _>>();
        let groups_by_id = model
            .groups
            .iter()
            .map(|group| (group.id.as_str(), group))
            .collect::<HashMap<_, _>>();
        validate_model(model, layout, &nodes_by_id, &groups_by_id)?;
        validate_portable_labels(model)?;
        let has_icons = model.nodes.iter().any(|node| {
            node.icon
                .as_deref()
                .is_some_and(|icon| !icon.trim().is_empty())
        }) || model.groups.iter().any(|group| {
            group
                .icon
                .as_deref()
                .is_some_and(|icon| !icon.trim().is_empty())
        });
        if has_icons
            && session
                .icon_registry()
                .is_some_and(|registry| !registry.is_empty())
        {
            return Err(unavailable(
                "custom icon registries can override Architecture icon geometry, which DrawingList v1 cannot yet project without parsing registry SVG",
            ));
        }
        let group_bounds = compute_group_bounds(model, layout, &nodes_by_id, icon_size, padding)?;
        let semantics = vec![
            SemanticAnnotation {
                id: "architecture.document".to_string(),
                role: SemanticRole::Document,
                title: model
                    .acc_title
                    .clone()
                    .or_else(|| model.title.clone())
                    .or_else(|| metadata.title.clone())
                    .or_else(|| Some(metadata.diagram_type.clone())),
                description: model.acc_descr.clone(),
                link: None,
            },
            architecture_container_semantic("architecture.edges", "Architecture edges"),
            architecture_container_semantic("architecture.services", "Architecture services"),
            architecture_container_semantic("architecture.groups", "Architecture groups"),
        ];
        let semantic_classes = BTreeMap::from([
            (
                "architecture.edges".to_string(),
                "architecture-edges".to_string(),
            ),
            (
                "architecture.services".to_string(),
                "architecture-services".to_string(),
            ),
            (
                "architecture.groups".to_string(),
                "architecture-groups".to_string(),
            ),
        ]);

        Ok(Self {
            metadata,
            session,
            policy,
            model,
            layout,
            icon_size,
            padding,
            arch_font_size,
            font_size,
            font,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            text_color,
            edge_color,
            arrow_color,
            group_color,
            edge_width,
            group_width,
            nodes_by_id,
            model_nodes_by_id,
            group_bounds,
            resources: Vec::new(),
            commands: vec![DrawingCommand::Save],
            semantics,
            semantic_classes,
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            dom_ids: BTreeMap::new(),
            content_bounds: None,
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: "architecture.edges".to_string(),
        });
        for (index, edge) in self.model.edges.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_edge(index, edge, &self.layout.edges[index])?;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);

        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: "architecture.services".to_string(),
        });
        for (index, node) in self.model.nodes.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_node(index, node)?;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);

        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: "architecture.groups".to_string(),
        });
        for (index, group) in self.model.groups.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_group(index, group)?;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);

        let bounds = self.content_bounds.unwrap_or(Bounds {
            min_x: -self.icon_size / 2.0,
            min_y: -self.icon_size / 2.0,
            max_x: self.icon_size / 2.0,
            max_y: self.icon_size / 2.0,
        });
        let viewport = expand_bounds(&bounds, self.padding);
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(
                viewport.min_x,
                viewport.min_y,
                (viewport.max_x - viewport.min_x).max(1.0),
                (viewport.max_y - viewport.min_y).max(1.0),
            )),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-architecture".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "icon_mode": "built_in_vector_only",
                    "edge_mode": "typed_polyline_with_explicit_arrows",
                    "group_bounds": "fcose_or_child_union",
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;
        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Architecture,
                body: SvgStructureBody::Architecture(ArchitectureSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self
                        .metadata
                        .effective_config
                        .as_value()
                        .get("architecture")
                        .and_then(|value| value.get("useMaxWidth"))
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                    acc_title: self
                        .model
                        .acc_title
                        .as_deref()
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned),
                    acc_description: self
                        .model
                        .acc_descr
                        .as_deref()
                        .map(|value| value.trim_end_matches('\n'))
                        .filter(|value| !value.trim().is_empty())
                        .map(ToOwned::to_owned),
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                    dom_ids: self.dom_ids,
                }),
            },
        })
    }

    fn emit_node(&mut self, index: usize, node: &ArchitectureRenderNode) -> Result<()> {
        let layout = self
            .nodes_by_id
            .get(node.id.as_str())
            .copied()
            .ok_or_else(|| invalid(format!("Architecture node `{}` has no layout", node.id)))?;
        validate_box(layout.x, layout.y, layout.width, layout.height, &node.id)?;
        let semantic_id = format!("architecture.node.{index}");
        self.semantic_classes.insert(
            semantic_id.clone(),
            match node.node_type {
                ArchitectureRenderNodeType::Service => "architecture-service",
                ArchitectureRenderNodeType::Junction => "architecture-junction",
            }
            .to_string(),
        );
        if node.node_type == ArchitectureRenderNodeType::Service {
            self.dom_ids
                .insert(semantic_id.clone(), format!("service-{}", node.id));
        }
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        match node.node_type {
            ArchitectureRenderNodeType::Junction => {
                let path_id = format!("{semantic_id}.junction");
                self.add_svg_path(
                    path_id,
                    rect_path(layout.x, layout.y, self.icon_size, self.icon_size),
                    fill_style(Color::rgba(0, 0, 0, 0)),
                    None,
                    Some(format!("node-{}", node.id)),
                )?;
                self.extend_content_rect(Rect::new(
                    layout.x,
                    layout.y,
                    self.icon_size,
                    self.icon_size,
                ));
            }
            ArchitectureRenderNodeType::Service => {
                if node
                    .icon_text
                    .as_deref()
                    .is_some_and(|text| !text.trim().is_empty())
                {
                    return Err(unavailable(format!(
                        "Architecture service `{}` uses iconText/foreignObject, which DrawingList v1 cannot represent as vector text",
                        node.id
                    )));
                }
                self.text_classes.insert(
                    semantic_id.clone(),
                    "architecture-service-label".to_string(),
                );
                if let Some(title) = node
                    .title
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    let title = plain_architecture_label(title, "service title")?;
                    self.emit_multiline_text(
                        &format!("{semantic_id}.title"),
                        &title,
                        MultilineTextSpec {
                            origin: Point::new(
                                layout.x + self.icon_size / 2.0,
                                layout.y + self.icon_size,
                            ),
                            fill: self.text_color,
                            anchor: TextAnchor::Middle,
                            baseline: TextBaseline::Middle,
                            max_width: self.icon_size * 1.5,
                            include_in_root_bounds: node.in_group.is_none(),
                        },
                    )?;
                }
                if let Some(icon) = node
                    .icon
                    .as_deref()
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    self.emit_architecture_icon(
                        &format!("{semantic_id}.icon"),
                        icon,
                        Point::new(layout.x, layout.y),
                        self.icon_size,
                    )?;
                } else {
                    self.add_svg_path(
                        format!("{semantic_id}.shape"),
                        architecture_service_path(layout.x, layout.y, self.icon_size),
                        PathStyle {
                            fill_rule: FillRule::NonZero,
                            fill: None,
                            stroke: Some(dashed_stroke(self.group_color, self.group_width)),
                        },
                        Some("node-bkg"),
                        Some(format!("node-{}", node.id)),
                    )?;
                }
                self.extend_content_rect(Rect::new(
                    layout.x,
                    layout.y,
                    self.icon_size,
                    self.icon_size,
                ));
            }
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: node.title.clone().or_else(|| Some(node.id.clone())),
            description: Some(match node.node_type {
                ArchitectureRenderNodeType::Service => "Architecture service".to_string(),
                ArchitectureRenderNodeType::Junction => "Architecture junction".to_string(),
            }),
            link: None,
        });
        Ok(())
    }

    fn emit_group(&mut self, index: usize, group: &ArchitectureRenderGroup) -> Result<()> {
        let bounds = self
            .group_bounds
            .get(group.id.as_str())
            .cloned()
            .ok_or_else(|| invalid(format!("Architecture group `{}` has no bounds", group.id)))?;
        let semantic_id = format!("architecture.group.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.add_svg_path(
            format!("{semantic_id}.outline"),
            rect_path(
                bounds.min_x,
                bounds.min_y,
                bounds.max_x - bounds.min_x,
                bounds.max_y - bounds.min_y,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(dashed_stroke(self.group_color, self.group_width)),
            },
            Some("node-bkg"),
            Some(format!("group-{}", group.id)),
        )?;
        self.extend_content_bounds(bounds.clone());
        if let Some(icon) = group
            .icon
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            self.emit_architecture_icon(
                &format!("{semantic_id}.icon"),
                icon,
                Point::new(bounds.min_x + 1.0, bounds.min_y + 1.0),
                (self.padding * 0.75).max(1.0),
            )?;
        }
        if let Some(title) = group
            .title
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            let title = plain_architecture_label(title, "group title")?;
            self.text_classes.insert(
                semantic_id.clone(),
                "architecture-service-label".to_string(),
            );
            let group_icon_size = (self.padding * 0.75).max(1.0);
            let has_icon = group
                .icon
                .as_deref()
                .is_some_and(|icon| !icon.trim().is_empty());
            self.emit_multiline_text(
                &format!("{semantic_id}.title"),
                &title,
                MultilineTextSpec {
                    origin: Point::new(
                        bounds.min_x + 4.0 + if has_icon { group_icon_size } else { 0.0 },
                        bounds.min_y
                            + 2.0
                            + if has_icon {
                                self.arch_font_size / 2.0 - 3.0
                            } else {
                                0.0
                            },
                    ),
                    fill: self.text_color,
                    anchor: TextAnchor::Start,
                    baseline: TextBaseline::Hanging,
                    max_width: (bounds.max_x - bounds.min_x).max(self.font_size),
                    include_in_root_bounds: true,
                },
            )?;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: group.title.clone().or_else(|| Some(group.id.clone())),
            description: Some("Architecture group".to_string()),
            link: None,
        });
        Ok(())
    }

    fn emit_edge(
        &mut self,
        index: usize,
        edge: &ArchitectureRenderEdge,
        layout: &LayoutEdge,
    ) -> Result<()> {
        if layout.points.len() < 2 {
            return Err(invalid(format!(
                "Architecture edge {index} has fewer than two route points"
            )));
        }
        let mut points = layout
            .points
            .iter()
            .map(|point| Point::new(point.x, point.y))
            .collect::<Vec<_>>();
        if points
            .iter()
            .any(|point| !point.x.is_finite() || !point.y.is_finite())
        {
            return Err(invalid(format!(
                "Architecture edge {index} has non-finite route points"
            )));
        }
        apply_endpoint_shift(
            &mut points[0],
            edge.lhs_dir,
            edge.lhs_group.unwrap_or(false),
            is_junction(self.model_nodes_by_id.get(edge.lhs_id.as_str())),
            self.icon_size,
            self.padding,
        );
        let last = points.len() - 1;
        apply_endpoint_shift(
            &mut points[last],
            edge.rhs_dir,
            edge.rhs_group.unwrap_or(false),
            is_junction(self.model_nodes_by_id.get(edge.rhs_id.as_str())),
            self.icon_size,
            self.padding,
        );
        let semantic_id = format!("architecture.edge.{index}");
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.add_svg_path(
            format!("{semantic_id}.route"),
            polyline_path(&points),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.edge_color, self.edge_width)),
            },
            Some("edge"),
            Some(format!("L_{}_{}_0", edge.lhs_id, edge.rhs_id)),
        )?;
        self.extend_content_points(&points);
        let arrow_size = (self.icon_size / 6.0).max(1.0);
        if edge.lhs_into == Some(true) {
            let arrow = architecture_arrow_path(points[0], points[1], edge.lhs_dir, arrow_size);
            self.extend_content_segments(&arrow);
            self.add_svg_path(
                format!("{semantic_id}.arrow.start"),
                arrow,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.arrow_color)),
                    stroke: None,
                },
                Some("arrow"),
                None,
            )?;
        }
        if edge.rhs_into == Some(true) {
            let arrow =
                architecture_arrow_path(points[last], points[last - 1], edge.rhs_dir, arrow_size);
            self.extend_content_segments(&arrow);
            self.add_svg_path(
                format!("{semantic_id}.arrow.end"),
                arrow,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.arrow_color)),
                    stroke: None,
                },
                Some("arrow"),
                None,
            )?;
        }
        if let Some(title) = edge
            .title
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
        {
            let title = plain_architecture_label(title, "edge label")?;
            self.text_classes.insert(
                semantic_id.clone(),
                "architecture-service-label".to_string(),
            );
            self.emit_edge_label(&semantic_id, &title, &points, edge.lhs_dir, edge.rhs_dir)?;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Edge,
            title: edge.title.clone(),
            description: Some(format!("{} → {}", edge.lhs_id, edge.rhs_id)),
            link: None,
        });
        Ok(())
    }

    fn emit_edge_label(
        &mut self,
        semantic_id: &str,
        text: &str,
        points: &[Point],
        lhs_dir: char,
        rhs_dir: char,
    ) -> Result<()> {
        let middle = points[points.len() / 2];
        let style = self.architecture_text_style();
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let axis = architecture_edge_axis(lhs_dir, rhs_dir);
        let wrap_width = match axis {
            ArchitectureEdgeAxis::Horizontal => (points[0].x - points[points.len() - 1].x).abs(),
            ArchitectureEdgeAxis::Vertical => {
                (points[0].y - points[points.len() - 1].y).abs() / 1.5
            }
            ArchitectureEdgeAxis::Mixed => (points[0].x - points[points.len() - 1].x).abs() / 2.0,
        };
        let lines = wrap_architecture_plain_lines(text, wrap_width, &measurer, &style);
        let first_line = lines
            .iter()
            .find(|line| !line.is_empty())
            .map(String::as_str)
            .unwrap_or(text);
        let width = lines
            .iter()
            .map(|line| architecture_line_bbox_width(line, &measurer, &style))
            .fold(0.0_f64, f64::max)
            .max(1.0);
        let (bbox_y_min, bbox_y_max) = architecture_create_text_middle_bbox_y_range_px(
            first_line,
            &style,
            lines.len(),
            &measurer,
        );
        let height = (bbox_y_max - bbox_y_min).max(self.font_size);
        let rotation = architecture_edge_label_rotation(lhs_dir, rhs_dir);
        let local_bounds = Rect::new(-width / 2.0, bbox_y_min, width, height);
        let transform = architecture_edge_label_transform(
            axis, lhs_dir, rhs_dir, middle, width, height, rotation,
        );
        self.extend_content_rect(transformed_rect_bounds(local_bounds, transform));
        self.commands.push(DrawingCommand::Save);
        self.commands
            .push(DrawingCommand::ConcatTransform { transform });
        self.commands.push(DrawingCommand::draw_text(TextRun {
            text: lines.join("\n"),
            origin: Point::new(0.0, 0.0),
            bounds: local_bounds,
            style: TextStyle {
                font: self.font.clone(),
                font_size: self.font_size,
                letter_spacing: 0.0,
                line_height: self.font_size * 1.1,
                fill: Paint::solid(self.text_color),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor: TextAnchor::Middle,
            baseline: TextBaseline::Middle,
            direction: TextDirection::Auto,
            language: None,
            obligation: self.text_obligation.clone(),
        }));
        self.commands.push(DrawingCommand::Restore);
        self.semantics.push(SemanticAnnotation {
            id: format!("{semantic_id}.label"),
            role: SemanticRole::Label,
            title: Some(text.to_string()),
            description: Some("Architecture edge label".to_string()),
            link: None,
        });
        Ok(())
    }

    fn emit_multiline_text(&mut self, id: &str, text: &str, spec: MultilineTextSpec) -> Result<()> {
        let MultilineTextSpec {
            origin,
            fill,
            anchor,
            baseline,
            max_width,
            include_in_root_bounds,
        } = spec;
        let style = self.architecture_text_style();
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let lines = wrap_architecture_plain_lines(text, max_width, &measurer, &style);
        if lines.iter().all(|line| line.is_empty()) {
            return Err(invalid(format!("Architecture text `{id}` is empty")));
        }
        let first_line = lines
            .iter()
            .find(|line| !line.is_empty())
            .map(String::as_str)
            .unwrap_or(text);
        let width = lines
            .iter()
            .map(|line| architecture_line_bbox_width(line, &measurer, &style))
            .fold(0.0_f64, f64::max)
            .max(1.0);
        let (bbox_y_min, bbox_y_max) = architecture_create_text_middle_bbox_y_range_px(
            first_line,
            &style,
            lines.len(),
            &measurer,
        );
        let height = (bbox_y_max - bbox_y_min).max(self.font_size);
        let x = match anchor {
            TextAnchor::Start => origin.x,
            TextAnchor::Middle => origin.x - width / 2.0,
            TextAnchor::End => origin.x - width,
        };
        let y = match baseline {
            TextBaseline::Middle | TextBaseline::Central => origin.y - height / 2.0,
            TextBaseline::Hanging => origin.y,
            _ => origin.y - height,
        };
        let bounds = Rect::new(x, y, width, height);
        if include_in_root_bounds {
            self.extend_content_rect(bounds);
        }
        self.commands.push(DrawingCommand::draw_text(TextRun {
            text: lines.join("\n"),
            origin,
            bounds,
            style: TextStyle {
                font: self.font.clone(),
                font_size: self.font_size,
                letter_spacing: 0.0,
                line_height: self.font_size * 1.1,
                fill: Paint::solid(fill),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor,
            baseline,
            direction: TextDirection::Auto,
            language: None,
            obligation: self.text_obligation.clone(),
        }));
        self.semantics.push(SemanticAnnotation {
            id: id.to_string(),
            role: SemanticRole::Label,
            title: Some(text.to_string()),
            description: Some("Architecture label".to_string()),
            link: None,
        });
        Ok(())
    }

    fn architecture_text_style(&self) -> MeasurementTextStyle {
        MeasurementTextStyle {
            font_family: Some(self.font.families.join(", ")),
            font_size: self.font_size,
            font_weight: None,
            font_style: None,
        }
    }

    fn emit_architecture_icon(
        &mut self,
        id: &str,
        icon: &str,
        origin: Point,
        size: f64,
    ) -> Result<()> {
        let icon = match icon {
            "database" | "server" | "disk" | "internet" | "cloud" | "blank" => icon,
            _ => "unknown",
        };
        let scale = size / 80.0;
        let blue = Color::rgba(8, 126, 191, 255);
        let white = Color::rgba(255, 255, 255, 255);
        self.add_path(
            format!("{id}.background"),
            rect_path(origin.x, origin.y, size, size),
            fill_style(blue),
        )?;
        match icon {
            "database" => {
                for (index, data) in [
                    "m20,57.86c0,3.94,8.95,7.14,20,7.14s20-3.2,20-7.14",
                    "m20,45.95c0,3.94,8.95,7.14,20,7.14s20-3.2,20-7.14",
                    "m20,34.05c0,3.94,8.95,7.14,20,7.14s20-3.2,20-7.14",
                ]
                .into_iter()
                .enumerate()
                {
                    self.add_icon_data(
                        &format!("{id}.ring.{index}"),
                        data,
                        origin,
                        scale,
                        stroke_style(white, 2.0 * scale),
                    )?;
                }
                self.add_icon_ellipse(
                    &format!("{id}.top"),
                    origin,
                    scale,
                    40.0,
                    22.14,
                    20.0,
                    7.14,
                    stroke_style(white, 2.0 * scale),
                )?;
                for (index, x) in [20.0, 60.0].into_iter().enumerate() {
                    self.add_icon_line(
                        &format!("{id}.side.{index}"),
                        origin,
                        scale,
                        Point::new(x, 57.86),
                        Point::new(x, 22.14),
                        stroke_style(white, 2.0 * scale),
                    )?;
                }
            }
            "server" => {
                self.add_icon_rect(
                    &format!("{id}.case"),
                    origin,
                    scale,
                    Rect::new(17.5, 17.5, 45.0, 45.0),
                    2.0,
                    stroke_style(white, 2.0 * scale),
                )?;
                for (index, y) in [32.5, 47.5].into_iter().enumerate() {
                    self.add_icon_line(
                        &format!("{id}.divider.{index}"),
                        origin,
                        scale,
                        Point::new(17.5, y),
                        Point::new(62.5, y),
                        stroke_style(white, 2.0 * scale),
                    )?;
                }
                for (row, y) in [25.0, 40.0, 55.0].into_iter().enumerate() {
                    self.add_icon_data(
                        &format!("{id}.slot.{row}"),
                        &format!(
                            "m56.25,{y}c0,.27-.45,.5-1,.5h-10.5c-.55,0-1-.23-1-.5s.45-.5,1-.5h10.5c.55,0,1,.23,1,.5Z"
                        ),
                        origin,
                        scale,
                        fill_stroke_style(white, white, scale),
                    )?;
                    for (column, x) in [32.5, 27.5, 22.5].into_iter().enumerate() {
                        self.add_icon_ellipse(
                            &format!("{id}.light.{row}.{column}"),
                            origin,
                            scale,
                            x,
                            y,
                            0.75,
                            0.75,
                            fill_stroke_style(white, white, scale),
                        )?;
                    }
                }
            }
            "disk" => {
                self.add_icon_rect(
                    &format!("{id}.case"),
                    origin,
                    scale,
                    Rect::new(20.0, 15.0, 40.0, 50.0),
                    1.0,
                    stroke_style(white, 2.0 * scale),
                )?;
                for (index, (x, y)) in [(24.0, 19.17), (56.0, 19.17), (24.0, 60.83), (56.0, 60.83)]
                    .into_iter()
                    .enumerate()
                {
                    self.add_icon_ellipse(
                        &format!("{id}.screw.{index}"),
                        origin,
                        scale,
                        x,
                        y,
                        0.8,
                        0.83,
                        stroke_style(white, 2.0 * scale),
                    )?;
                }
                self.add_icon_ellipse(
                    &format!("{id}.platter"),
                    origin,
                    scale,
                    40.0,
                    33.75,
                    14.0,
                    14.58,
                    stroke_style(white, 2.0 * scale),
                )?;
                self.add_icon_ellipse(
                    &format!("{id}.hub"),
                    origin,
                    scale,
                    40.0,
                    33.75,
                    4.0,
                    4.17,
                    fill_stroke_style(white, white, 2.0 * scale),
                )?;
                self.add_icon_data(
                    &format!("{id}.arm"),
                    "m37.51,42.52l-4.83,13.22c-.26,.71-1.1,1.02-1.76,.64l-4.18-2.42c-.66-.38-.81-1.26-.33-1.84l9.01-10.8c.88-1.05,2.56-.08,2.09,1.2Z",
                    origin,
                    scale,
                    fill_style(white),
                )?;
            }
            "internet" => {
                self.add_icon_ellipse(
                    &format!("{id}.globe"),
                    origin,
                    scale,
                    40.0,
                    40.0,
                    22.5,
                    22.5,
                    stroke_style(white, 2.0 * scale),
                )?;
                for (index, (from, to)) in [
                    (Point::new(40.0, 17.5), Point::new(40.0, 62.5)),
                    (Point::new(17.5, 40.0), Point::new(62.5, 40.0)),
                    (Point::new(19.75, 30.1), Point::new(60.25, 30.1)),
                    (Point::new(19.75, 49.9), Point::new(60.25, 49.9)),
                ]
                .into_iter()
                .enumerate()
                {
                    self.add_icon_line(
                        &format!("{id}.axis.{index}"),
                        origin,
                        scale,
                        from,
                        to,
                        stroke_style(white, 2.0 * scale),
                    )?;
                }
                for (index, data) in [
                    "m39.99,17.51c-15.28,11.1-15.28,33.88,0,44.98",
                    "m40.01,17.51c15.28,11.1,15.28,33.88,0,44.98",
                ]
                .into_iter()
                .enumerate()
                {
                    self.add_icon_data(
                        &format!("{id}.longitude.{index}"),
                        data,
                        origin,
                        scale,
                        stroke_style(white, 2.0 * scale),
                    )?;
                }
            }
            "cloud" => {
                self.add_icon_data(
                    &format!("{id}.cloud"),
                    "m65,47.5c0,2.76-2.24,5-5,5H20c-2.76,0-5-2.24-5-5,0-1.87,1.03-3.51,2.56-4.36-.04-.21-.06-.42-.06-.64,0-2.6,2.48-4.74,5.65-4.97,1.65-4.51,6.34-7.76,11.85-7.76.86,0,1.69.08,2.5.23,2.09-1.57,4.69-2.5,7.5-2.5,6.1,0,11.19,4.38,12.28,10.17,2.14.56,3.72,2.51,3.72,4.83,0,.03,0,.07-.01.1,2.29.46,4.01,2.48,4.01,4.9Z",
                    origin,
                    scale,
                    stroke_style(white, 2.0 * scale),
                )?;
            }
            "unknown" => {
                self.commands.push(DrawingCommand::draw_text(TextRun {
                    text: "?".to_string(),
                    origin: Point::new(origin.x + size / 2.0, origin.y + size * 0.82),
                    bounds: Rect::new(origin.x, origin.y, size, size),
                    style: TextStyle {
                        font: self.font.clone(),
                        font_size: size * 0.7,
                        letter_spacing: 0.0,
                        line_height: size,
                        fill: Paint::solid(white),
                        stroke: None,
                        paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                    },
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Alphabetic,
                    direction: TextDirection::Auto,
                    language: None,
                    obligation: self.text_obligation.clone(),
                }));
            }
            "blank" => {}
            // `icon` is normalized to the built-in set above. Keep this arm defensive rather
            // than panicking if a future built-in is added without a matching vector mapping.
            _ => {}
        }
        Ok(())
    }

    fn add_icon_data(
        &mut self,
        id: &str,
        data: &str,
        origin: Point,
        scale: f64,
        style: PathStyle,
    ) -> Result<()> {
        let segments = transform_segments(super::parse_svg_path(data)?, origin, scale);
        self.add_path(id.to_string(), segments, style)
    }

    #[allow(clippy::too_many_arguments)]
    fn add_icon_ellipse(
        &mut self,
        id: &str,
        origin: Point,
        scale: f64,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        style: PathStyle,
    ) -> Result<()> {
        self.add_path(
            id.to_string(),
            ellipse_path(
                origin.x + cx * scale,
                origin.y + cy * scale,
                rx * scale,
                ry * scale,
            ),
            style,
        )
    }

    fn add_icon_line(
        &mut self,
        id: &str,
        origin: Point,
        scale: f64,
        from: Point,
        to: Point,
        style: PathStyle,
    ) -> Result<()> {
        self.add_path(
            id.to_string(),
            vec![
                PathSegment::MoveTo {
                    to: transform_point(from, origin, scale),
                },
                PathSegment::LineTo {
                    to: transform_point(to, origin, scale),
                },
            ],
            style,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn add_icon_rect(
        &mut self,
        id: &str,
        origin: Point,
        scale: f64,
        rect: Rect,
        radius: f64,
        style: PathStyle,
    ) -> Result<()> {
        self.add_path(
            id.to_string(),
            rounded_rect_path(
                origin.x + (rect.x + rect.width / 2.0) * scale,
                origin.y + (rect.y + rect.height / 2.0) * scale,
                rect.width * scale,
                rect.height * scale,
                radius * scale,
            ),
            style,
        )
    }

    fn add_svg_path(
        &mut self,
        id: String,
        segments: Vec<PathSegment>,
        style: PathStyle,
        class: Option<&str>,
        dom_id: Option<String>,
    ) -> Result<()> {
        if let Some(class) = class {
            self.path_classes.insert(id.clone(), class.to_string());
        }
        if let Some(dom_id) = dom_id {
            self.dom_ids.insert(id.clone(), dom_id);
        }
        self.add_path(id, segments, style)
    }

    fn extend_content_bounds(&mut self, bounds: Bounds) {
        extend_bounds(&mut self.content_bounds, bounds);
    }

    fn extend_content_rect(&mut self, rect: Rect) {
        self.extend_content_bounds(Bounds {
            min_x: rect.x,
            min_y: rect.y,
            max_x: rect.x + rect.width,
            max_y: rect.y + rect.height,
        });
    }

    fn extend_content_points(&mut self, points: &[Point]) {
        if let Some(bounds) = points_bounds(points) {
            self.extend_content_bounds(bounds);
        }
    }

    fn extend_content_segments(&mut self, segments: &[PathSegment]) {
        let mut points = Vec::with_capacity(segments.len() * 3);
        for segment in segments {
            match segment {
                PathSegment::MoveTo { to }
                | PathSegment::LineTo { to }
                | PathSegment::ArcTo { to, .. } => points.push(*to),
                PathSegment::QuadTo { control, to } => {
                    points.push(*control);
                    points.push(*to);
                }
                PathSegment::CubicTo {
                    control1,
                    control2,
                    to,
                } => {
                    points.push(*control1);
                    points.push(*control2);
                    points.push(*to);
                }
                PathSegment::Close => {}
            }
        }
        self.extend_content_points(&points);
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        self.resources.push(DrawingResource::Path(PathResource {
            id: ResourceId::new(id.clone()),
            segments,
        }));
        self.commands.push(DrawingCommand::DrawPath {
            path: ResourceId::new(id),
            style,
        });
        Ok(())
    }
}

fn architecture_container_semantic(id: &str, description: &str) -> SemanticAnnotation {
    SemanticAnnotation {
        id: id.to_string(),
        role: SemanticRole::Group,
        title: None,
        description: Some(description.to_string()),
        link: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ArchitectureEdgeAxis {
    Horizontal,
    Vertical,
    Mixed,
}

fn architecture_edge_axis(lhs_dir: char, rhs_dir: char) -> ArchitectureEdgeAxis {
    match (lhs_dir, rhs_dir) {
        ('L' | 'R', 'L' | 'R') => ArchitectureEdgeAxis::Horizontal,
        ('T' | 'B', 'T' | 'B') => ArchitectureEdgeAxis::Vertical,
        _ => ArchitectureEdgeAxis::Mixed,
    }
}

fn wrap_architecture_plain_lines(
    text: &str,
    max_width_px: f64,
    measurer: &dyn TextMeasurer,
    style: &MeasurementTextStyle,
) -> Vec<String> {
    let max_width_px = if max_width_px.is_finite() && max_width_px > 0.0 {
        max_width_px
    } else {
        ARCHITECTURE_CREATE_TEXT_DEFAULT_WRAP_WIDTH_PX
    };
    let mut measured_style = style.clone();
    measured_style.font_weight = Some("normal".to_string());
    measured_style.font_style = Some("normal".to_string());

    let word_width = |word: &str, first: bool| {
        let measured = if first {
            word.to_string()
        } else {
            format!(" {word}")
        };
        measurer.measure_svg_text_computed_length_px(&measured, &measured_style)
    };

    let mut output = Vec::new();
    for source_line in text.split('\n') {
        let mut current = String::new();
        let mut current_width = 0.0;
        for word in source_line.split(' ').filter(|word| !word.is_empty()) {
            let width = word_width(word, current.is_empty());
            if !current.is_empty() && current_width + width > max_width_px {
                output.push(std::mem::take(&mut current));
                current_width = 0.0;
            }

            if current.is_empty() && word_width(word, true) > max_width_px {
                let mut chunk = String::new();
                for ch in word.chars() {
                    let mut candidate = chunk.clone();
                    candidate.push(ch);
                    if !chunk.is_empty()
                        && measurer.measure_svg_text_computed_length_px(&candidate, &measured_style)
                            > max_width_px
                    {
                        output.push(std::mem::take(&mut chunk));
                    }
                    chunk.push(ch);
                }
                current = chunk;
                current_width =
                    measurer.measure_svg_text_computed_length_px(&current, &measured_style);
                continue;
            }

            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
            current_width += width;
        }
        if !current.is_empty() {
            output.push(current);
        } else if source_line.is_empty() {
            output.push(String::new());
        }
    }

    if output.is_empty() {
        vec![String::new()]
    } else {
        output
    }
}

fn architecture_line_bbox_width(
    line: &str,
    measurer: &dyn TextMeasurer,
    style: &MeasurementTextStyle,
) -> f64 {
    let mut measured_style = style.clone();
    measured_style.font_weight = Some("normal".to_string());
    measured_style.font_style = Some("normal".to_string());
    let (left, right) = measurer.measure_svg_text_bbox_x(line, &measured_style);
    (left + right).max(0.0)
}

fn points_bounds(points: &[Point]) -> Option<Bounds> {
    let first = points.first()?;
    let mut bounds = Bounds {
        min_x: first.x,
        min_y: first.y,
        max_x: first.x,
        max_y: first.y,
    };
    for point in &points[1..] {
        bounds.min_x = bounds.min_x.min(point.x);
        bounds.min_y = bounds.min_y.min(point.y);
        bounds.max_x = bounds.max_x.max(point.x);
        bounds.max_y = bounds.max_y.max(point.y);
    }
    Some(bounds)
}

fn config_css_length(config: &Value, path: &[&str]) -> Option<String> {
    let value = path.iter().try_fold(config, |value, key| value.get(*key))?;
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn validate_portable_labels(model: &ArchitectureDiagramRenderModel) -> Result<()> {
    for node in &model.nodes {
        if let Some(title) = node
            .title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
        {
            let _ = plain_architecture_label(title, &format!("service `{}` title", node.id))?;
        }
    }
    for group in &model.groups {
        if let Some(title) = group
            .title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
        {
            let _ = plain_architecture_label(title, &format!("group `{}` title", group.id))?;
        }
    }
    for (index, edge) in model.edges.iter().enumerate() {
        if let Some(title) = edge
            .title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
        {
            let _ = plain_architecture_label(title, &format!("edge {index} label"))?;
        }
    }
    Ok(())
}

fn plain_architecture_label(value: &str, context: &str) -> Result<String> {
    let normalized = normalize_architecture_breaks(value);
    let analysis = crate::text::analyze_mermaid_markdown(&normalized, true);
    if analysis.has_styled_runs
        || crate::text::mermaid_markdown_contains_raw_blocks(&normalized)
        || crate::text::mermaid_markdown_contains_html_tags(&normalized)
        || normalized.contains('`')
        || normalized.contains("](")
        || normalized.contains("![")
    {
        return Err(unavailable(format!(
            "Architecture {context} uses Markdown styling that DrawingList v1 cannot preserve as a single host text run"
        )));
    }
    let fragment = crate::text::mermaid_markdown_to_xhtml_label_fragment(&normalized, true);
    let plain = crate::text::mermaid_xhtml_label_plain_text(&fragment).ok_or_else(|| {
        unavailable(format!(
            "Architecture {context} uses HTML structure that DrawingList v1 cannot preserve"
        ))
    })?;
    Ok(svg_plain_text(&plain))
}

fn normalize_architecture_breaks(value: &str) -> String {
    // Architecture labels use Mermaid's `<br>` line-break spelling. Normalize those explicit
    // breaks before rejecting all other HTML so line breaks remain representable as separate
    // host text runs instead of being silently flattened.
    crate::text::split_html_br_lines(value).join("\n")
}

fn validate_model(
    model: &ArchitectureDiagramRenderModel,
    layout: &ArchitectureDiagramLayout,
    nodes_by_id: &HashMap<&str, &LayoutNode>,
    groups_by_id: &HashMap<&str, &ArchitectureRenderGroup>,
) -> Result<()> {
    if nodes_by_id.len() != model.nodes.len() {
        return Err(invalid("Architecture model contains duplicate node IDs"));
    }
    if groups_by_id.len() != model.groups.len() {
        return Err(invalid("Architecture model contains duplicate group IDs"));
    }
    if model
        .nodes
        .iter()
        .any(|node| groups_by_id.contains_key(node.id.as_str()))
    {
        return Err(invalid(
            "Architecture node and group identifiers must be disjoint",
        ));
    }
    let layout_node_ids = layout
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    if layout_node_ids.len() != layout.nodes.len() {
        return Err(invalid("Architecture layout contains duplicate node IDs"));
    }
    for node in &model.nodes {
        let layout = nodes_by_id
            .get(node.id.as_str())
            .ok_or_else(|| invalid(format!("Architecture node `{}` has no layout", node.id)))?;
        validate_box(layout.x, layout.y, layout.width, layout.height, &node.id)?;
        if let Some(group) = node.in_group.as_deref()
            && !groups_by_id.contains_key(group)
        {
            return Err(invalid(format!(
                "Architecture node `{}` references missing group `{group}`",
                node.id
            )));
        }
    }
    for group in &model.groups {
        if let Some(parent) = group.in_group.as_deref()
            && !groups_by_id.contains_key(parent)
        {
            return Err(invalid(format!(
                "Architecture group `{}` references missing parent group `{parent}`",
                group.id
            )));
        }
    }
    for (index, edge) in model.edges.iter().enumerate() {
        if !matches!(edge.lhs_dir, 'L' | 'R' | 'T' | 'B')
            || !matches!(edge.rhs_dir, 'L' | 'R' | 'T' | 'B')
        {
            return Err(invalid(format!(
                "Architecture edge {index} has an invalid port direction"
            )));
        }
        if !nodes_by_id.contains_key(edge.lhs_id.as_str())
            && !groups_by_id.contains_key(edge.lhs_id.as_str())
        {
            return Err(invalid(format!(
                "Architecture edge references missing lhs `{}`",
                edge.lhs_id
            )));
        }
        if !nodes_by_id.contains_key(edge.rhs_id.as_str())
            && !groups_by_id.contains_key(edge.rhs_id.as_str())
        {
            return Err(invalid(format!(
                "Architecture edge references missing rhs `{}`",
                edge.rhs_id
            )));
        }
        let layout_edge = layout
            .edges
            .get(index)
            .ok_or_else(|| invalid(format!("Architecture edge {index} has no layout record")))?;
        if layout_edge.from != edge.lhs_id || layout_edge.to != edge.rhs_id {
            return Err(invalid(format!(
                "Architecture edge {index} layout endpoints do not match semantic endpoints"
            )));
        }
    }
    if layout.nodes.len() != model.nodes.len()
        || layout_node_ids
            .iter()
            .any(|id| !nodes_by_id.contains_key(id))
    {
        return Err(invalid(
            "Architecture semantic nodes and layout nodes are out of sync",
        ));
    }
    Ok(())
}

fn compute_group_bounds<'a>(
    model: &'a ArchitectureDiagramRenderModel,
    layout: &'a ArchitectureDiagramLayout,
    nodes_by_id: &HashMap<&'a str, &'a LayoutNode>,
    icon_size: f64,
    padding: f64,
) -> Result<HashMap<&'a str, Bounds>> {
    let cached_service_bounds = layout
        .cytoscape_service_bounds
        .iter()
        .map(|bounds| (bounds.id.as_str(), bounds))
        .collect::<HashMap<_, _>>();
    let mut service_bounds = HashMap::new();
    let mut junction_bounds = HashMap::new();
    let mut services_in_group: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut junctions_in_group: HashMap<&str, Vec<&str>> = HashMap::new();

    for node in &model.nodes {
        let Some(parent) = node.in_group.as_deref() else {
            continue;
        };
        let node_layout = nodes_by_id
            .get(node.id.as_str())
            .copied()
            .ok_or_else(|| invalid(format!("Architecture node `{}` has no layout", node.id)))?;
        match node.node_type {
            ArchitectureRenderNodeType::Service => {
                let cached = cached_service_bounds
                    .get(node.id.as_str())
                    .copied()
                    .filter(|bounds| bounds.in_group.as_deref() == Some(parent))
                    .ok_or_else(|| {
                        invalid(format!(
                            "Architecture grouped service `{}` has no matching Cytoscape child bounds",
                            node.id
                        ))
                    })?;
                validate_bounds(&cached.body_bounds)?;
                validate_bounds(&cached.union_bounds)?;
                if !bounds_match_rect(&cached.body_bounds, node_layout.x, node_layout.y, icon_size)
                {
                    return Err(invalid(format!(
                        "Architecture grouped service `{}` has stale Cytoscape child bounds",
                        node.id
                    )));
                }
                service_bounds.insert(node.id.as_str(), cached.union_bounds.clone());
                services_in_group
                    .entry(parent)
                    .or_default()
                    .push(node.id.as_str());
            }
            ArchitectureRenderNodeType::Junction => {
                let bounds = Bounds {
                    min_x: node_layout.x,
                    min_y: node_layout.y,
                    max_x: node_layout.x + icon_size,
                    max_y: node_layout.y + icon_size,
                };
                validate_bounds(&bounds)?;
                junction_bounds.insert(node.id.as_str(), bounds);
                junctions_in_group
                    .entry(parent)
                    .or_default()
                    .push(node.id.as_str());
            }
        }
    }

    let mut child_groups: HashMap<&str, Vec<&str>> = HashMap::new();
    for group in &model.groups {
        if let Some(parent) = group.in_group.as_deref() {
            child_groups
                .entry(parent)
                .or_default()
                .push(group.id.as_str());
        }
    }
    for children in child_groups.values_mut() {
        children.sort_unstable();
    }

    enum Step<'a> {
        Enter(&'a str),
        Exit(&'a str),
    }

    // Mermaid draws SVG group rectangles from Cytoscape's final `node.boundingBox()` phase,
    // which includes the service label contribution cached above. The raw FCoSE compound
    // rectangles intentionally represent an earlier layout-engine phase and are therefore not a
    // valid SVG geometry source here.
    let mut output: HashMap<&'a str, Bounds> = HashMap::new();
    let mut visiting = HashSet::new();
    for group in &model.groups {
        let mut stack = vec![Step::Enter(group.id.as_str())];
        while let Some(step) = stack.pop() {
            match step {
                Step::Enter(id) => {
                    if output.contains_key(id) {
                        continue;
                    }
                    if !visiting.insert(id) {
                        return Err(invalid(format!(
                            "Architecture group hierarchy contains a cycle at `{id}`"
                        )));
                    }
                    stack.push(Step::Exit(id));
                    if let Some(children) = child_groups.get(id) {
                        for child in children.iter().rev() {
                            if !output.contains_key(child) {
                                stack.push(Step::Enter(child));
                            }
                        }
                    }
                }
                Step::Exit(id) => {
                    let mut content = None;
                    if let Some(services) = services_in_group.get(id) {
                        for service in services {
                            let bounds = service_bounds.get(service).ok_or_else(|| {
                                invalid(format!(
                                    "Architecture service `{service}` has no compound bounds"
                                ))
                            })?;
                            extend_bounds(&mut content, bounds.clone());
                        }
                    }
                    if let Some(junctions) = junctions_in_group.get(id) {
                        for junction in junctions {
                            let bounds = junction_bounds.get(junction).ok_or_else(|| {
                                invalid(format!(
                                    "Architecture junction `{junction}` has no compound bounds"
                                ))
                            })?;
                            extend_bounds(&mut content, bounds.clone());
                        }
                    }
                    if let Some(children) = child_groups.get(id) {
                        for child in children {
                            let bounds = output.get(child).ok_or_else(|| {
                                invalid(format!(
                                    "Architecture child group `{child}` has no computed bounds"
                                ))
                            })?;
                            extend_bounds(&mut content, bounds.clone());
                        }
                    }
                    let bounds = match content {
                        Some(content) => {
                            expand_bounds(&content, architecture_svg_group_bbox_padding_px(padding))
                        }
                        None => Bounds {
                            min_x: 0.0,
                            min_y: 0.0,
                            max_x: icon_size,
                            max_y: icon_size,
                        },
                    };
                    validate_bounds(&bounds)?;
                    output.insert(id, bounds);
                    visiting.remove(id);
                }
            }
        }
    }
    Ok(output)
}

fn bounds_match_rect(bounds: &Bounds, x: f64, y: f64, size: f64) -> bool {
    const EPSILON: f64 = 1e-6;
    (bounds.min_x - x).abs() <= EPSILON
        && (bounds.min_y - y).abs() <= EPSILON
        && (bounds.max_x - (x + size)).abs() <= EPSILON
        && (bounds.max_y - (y + size)).abs() <= EPSILON
}

fn fill_style(color: Color) -> PathStyle {
    PathStyle {
        fill_rule: FillRule::NonZero,
        fill: Some(Paint::solid(color)),
        stroke: None,
    }
}

fn stroke_style(color: Color, width: f64) -> PathStyle {
    PathStyle {
        fill_rule: FillRule::NonZero,
        fill: None,
        stroke: Some(stroke(color, width)),
    }
}

fn fill_stroke_style(fill: Color, stroke_color: Color, width: f64) -> PathStyle {
    PathStyle {
        fill_rule: FillRule::NonZero,
        fill: Some(Paint::solid(fill)),
        stroke: Some(stroke(stroke_color, width)),
    }
}

fn transform_segments(segments: Vec<PathSegment>, origin: Point, scale: f64) -> Vec<PathSegment> {
    segments
        .into_iter()
        .map(|segment| match segment {
            PathSegment::MoveTo { to } => PathSegment::MoveTo {
                to: transform_point(to, origin, scale),
            },
            PathSegment::LineTo { to } => PathSegment::LineTo {
                to: transform_point(to, origin, scale),
            },
            PathSegment::QuadTo { control, to } => PathSegment::QuadTo {
                control: transform_point(control, origin, scale),
                to: transform_point(to, origin, scale),
            },
            PathSegment::CubicTo {
                control1,
                control2,
                to,
            } => PathSegment::CubicTo {
                control1: transform_point(control1, origin, scale),
                control2: transform_point(control2, origin, scale),
                to: transform_point(to, origin, scale),
            },
            PathSegment::ArcTo {
                radius_x,
                radius_y,
                x_axis_rotation_degrees,
                large_arc,
                sweep_clockwise,
                to,
            } => PathSegment::ArcTo {
                radius_x: radius_x.abs() * scale,
                radius_y: radius_y.abs() * scale,
                x_axis_rotation_degrees,
                large_arc,
                sweep_clockwise,
                to: transform_point(to, origin, scale),
            },
            PathSegment::Close => PathSegment::Close,
        })
        .collect()
}

fn transform_point(point: Point, origin: Point, scale: f64) -> Point {
    Point::new(origin.x + point.x * scale, origin.y + point.y * scale)
}

fn architecture_service_path(x: f64, y: f64, size: f64) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo {
            to: Point::new(x, y + size),
        },
        PathSegment::LineTo {
            to: Point::new(x, y + 5.0),
        },
        PathSegment::QuadTo {
            control: Point::new(x, y),
            to: Point::new(x + 5.0, y),
        },
        PathSegment::LineTo {
            to: Point::new(x + size - 5.0, y),
        },
        PathSegment::QuadTo {
            control: Point::new(x + size, y),
            to: Point::new(x + size, y + 5.0),
        },
        PathSegment::LineTo {
            to: Point::new(x + size, y + size),
        },
        PathSegment::Close,
    ]
}

fn polyline_path(points: &[Point]) -> Vec<PathSegment> {
    let mut segments = Vec::with_capacity(points.len());
    segments.push(PathSegment::MoveTo { to: points[0] });
    segments.extend(
        points
            .iter()
            .skip(1)
            .copied()
            .map(|to| PathSegment::LineTo { to }),
    );
    segments
}

fn architecture_arrow_path(
    anchor: Point,
    adjacent: Point,
    dir: char,
    size: f64,
) -> Vec<PathSegment> {
    let dx = anchor.x - adjacent.x;
    let dy = anchor.y - adjacent.y;
    let length = dx.hypot(dy);
    let (ux, uy) = if length > f64::EPSILON {
        (dx / length, dy / length)
    } else {
        match dir {
            'L' => (-1.0, 0.0),
            'R' => (1.0, 0.0),
            'T' => (0.0, -1.0),
            'B' => (0.0, 1.0),
            _ => (1.0, 0.0),
        }
    };

    if ux.abs() < 1e-6 || uy.abs() < 1e-6 {
        let half = size / 2.0;
        let (x, y, points) = match dir {
            'L' => (
                anchor.x - size + 2.0,
                anchor.y - half,
                [(size, half), (0.0, size), (0.0, 0.0)],
            ),
            'R' => (
                anchor.x - 2.0,
                anchor.y - half,
                [(0.0, half), (size, 0.0), (size, size)],
            ),
            'T' => (
                anchor.x - half,
                anchor.y - size + 2.0,
                [(0.0, 0.0), (size, 0.0), (half, size)],
            ),
            'B' => (
                anchor.x - half,
                anchor.y - 2.0,
                [(half, 0.0), (size, size), (0.0, size)],
            ),
            _ => (
                anchor.x - 2.0,
                anchor.y - half,
                [(0.0, half), (size, 0.0), (size, size)],
            ),
        };
        return polygon_path(&points.map(|(px, py)| Point::new(x + px, y + py)));
    }

    let tip = Point::new(anchor.x + 2.0 * ux, anchor.y + 2.0 * uy);
    let base = Point::new(tip.x - ux * size, tip.y - uy * size);
    let half = size / 2.0;
    let left = Point::new(base.x - uy * half, base.y + ux * half);
    let right = Point::new(base.x + uy * half, base.y - ux * half);
    polygon_path(&[left, right, tip])
}

fn apply_endpoint_shift(
    point: &mut Point,
    dir: char,
    group: bool,
    junction: bool,
    icon_size: f64,
    padding: f64,
) {
    let shift = if group {
        padding + 4.0
    } else if junction {
        icon_size / 2.0
    } else {
        0.0
    };
    if shift == 0.0 {
        return;
    }
    match (group, dir) {
        (true, 'L') => point.x -= shift,
        (true, 'R') => point.x += shift,
        (true, 'T') => point.y -= shift,
        (true, 'B') => {
            point.y += shift + ARCHITECTURE_SERVICE_LABEL_BOTTOM_EXTENSION_PX;
        }
        (false, 'L') => point.x += shift,
        (false, 'R') => point.x -= shift,
        (false, 'T') => point.y += shift,
        (false, 'B') => point.y -= shift,
        (_, _) => {}
    }
}

fn is_junction(node: Option<&&ArchitectureRenderNode>) -> bool {
    node.is_some_and(|node| node.node_type == ArchitectureRenderNodeType::Junction)
}

fn dashed_stroke(color: Color, width: f64) -> merman_display_list::StrokeStyle {
    let mut result = stroke(color, width);
    result.dash_array = vec![8.0, 8.0];
    result
}

fn rect_path(x: f64, y: f64, width: f64, height: f64) -> Vec<PathSegment> {
    polygon_path(&[
        Point::new(x, y),
        Point::new(x + width, y),
        Point::new(x + width, y + height),
        Point::new(x, y + height),
    ])
}

fn rotation_transform(degrees: f64, cx: f64, cy: f64) -> Transform {
    let radians = degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    Transform {
        a: cos,
        b: sin,
        c: -sin,
        d: cos,
        e: cx - cos * cx + sin * cy,
        f: cy - sin * cx - cos * cy,
    }
}

fn architecture_edge_label_rotation(lhs_dir: char, rhs_dir: char) -> f64 {
    match (lhs_dir, rhs_dir) {
        ('T' | 'B', 'T' | 'B') => -90.0,
        ('L' | 'R', 'L' | 'R') => 0.0,
        (lhs, rhs) => {
            let (xf, yf) = match (lhs, rhs) {
                ('L', 'T') | ('T', 'L') => (1.0, 1.0),
                ('B', 'L') | ('L', 'B') => (1.0, -1.0),
                ('B', 'R') | ('R', 'B') => (-1.0, -1.0),
                _ => (-1.0, 1.0),
            };
            -xf * yf * 45.0
        }
    }
}

fn architecture_edge_label_transform(
    axis: ArchitectureEdgeAxis,
    lhs_dir: char,
    rhs_dir: char,
    middle: Point,
    width: f64,
    height: f64,
    rotation: f64,
) -> Transform {
    match axis {
        ArchitectureEdgeAxis::Horizontal => translation_transform(middle.x, middle.y),
        ArchitectureEdgeAxis::Vertical => compose_transform(
            translation_transform(middle.x, middle.y),
            rotation_transform(rotation, 0.0, 0.0),
        ),
        ArchitectureEdgeAxis::Mixed => {
            let (x_factor, y_factor) = architecture_edge_xy_factors(lhs_dir, rhs_dir);
            let diagonal = (width + height) * std::f64::consts::FRAC_1_SQRT_2;
            compose_transform(
                compose_transform(
                    translation_transform(middle.x, middle.y - height / 2.0),
                    translation_transform(x_factor * diagonal / 2.0, y_factor * diagonal / 2.0),
                ),
                rotation_transform(rotation, 0.0, height / 2.0),
            )
        }
    }
}

fn architecture_edge_xy_factors(lhs_dir: char, rhs_dir: char) -> (f64, f64) {
    match (lhs_dir, rhs_dir) {
        ('L', 'T') | ('T', 'L') => (1.0, 1.0),
        ('B', 'L') | ('L', 'B') => (1.0, -1.0),
        ('B', 'R') | ('R', 'B') => (-1.0, -1.0),
        _ => (-1.0, 1.0),
    }
}

fn translation_transform(x: f64, y: f64) -> Transform {
    Transform {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: x,
        f: y,
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

fn transformed_rect_bounds(rect: Rect, transform: Transform) -> Rect {
    let points = [
        Point::new(rect.x, rect.y),
        Point::new(rect.x + rect.width, rect.y),
        Point::new(rect.x + rect.width, rect.y + rect.height),
        Point::new(rect.x, rect.y + rect.height),
    ]
    .map(|point| apply_transform(transform, point));
    let bounds = points_bounds(&points).expect("a rectangle always has four corners");
    Rect::new(
        bounds.min_x,
        bounds.min_y,
        bounds.max_x - bounds.min_x,
        bounds.max_y - bounds.min_y,
    )
}

fn apply_transform(transform: Transform, point: Point) -> Point {
    Point::new(
        transform.a * point.x + transform.c * point.y + transform.e,
        transform.b * point.x + transform.d * point.y + transform.f,
    )
}

fn expand_bounds(bounds: &Bounds, padding: f64) -> Bounds {
    Bounds {
        min_x: bounds.min_x - padding,
        min_y: bounds.min_y - padding,
        max_x: bounds.max_x + padding,
        max_y: bounds.max_y + padding,
    }
}

fn extend_bounds(target: &mut Option<Bounds>, other: Bounds) {
    if let Some(existing) = target {
        existing.min_x = existing.min_x.min(other.min_x);
        existing.min_y = existing.min_y.min(other.min_y);
        existing.max_x = existing.max_x.max(other.max_x);
        existing.max_y = existing.max_y.max(other.max_y);
    } else {
        *target = Some(other);
    }
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .iter()
        .any(|value| !value.is_finite())
        || bounds.max_x <= bounds.min_x
        || bounds.max_y <= bounds.min_y
    {
        return Err(invalid("Architecture bounds are invalid"));
    }
    Ok(())
}

fn validate_box(x: f64, y: f64, width: f64, height: f64, id: &str) -> Result<()> {
    if [x, y, width, height].iter().any(|value| !value.is_finite()) || width <= 0.0 || height <= 0.0
    {
        return Err(invalid(format!(
            "Architecture geometry for `{id}` is invalid"
        )));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "architecture".to_string(),
        reason: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::architecture_edge_label_rotation;

    #[test]
    fn edge_label_rotation_matches_mermaid_axis_and_diagonal_rules() {
        for (lhs, rhs, expected) in [
            ('L', 'R', 0.0),
            ('R', 'L', 0.0),
            ('T', 'B', -90.0),
            ('B', 'T', -90.0),
            ('L', 'T', -45.0),
            ('T', 'L', -45.0),
            ('B', 'L', 45.0),
            ('L', 'B', 45.0),
            ('B', 'R', -45.0),
            ('R', 'B', -45.0),
            ('R', 'T', 45.0),
            ('T', 'R', 45.0),
        ] {
            assert_eq!(
                architecture_edge_label_rotation(lhs, rhs),
                expected,
                "unexpected rotation for {lhs}->{rhs}"
            );
        }
    }
}
