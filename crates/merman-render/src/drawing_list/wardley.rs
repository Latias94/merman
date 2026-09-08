//! Renderer-neutral Wardley map adapter.
//!
//! Wardley owns all projected geometry in its typed layout.  This adapter expands SVG markers,
//! circles, rotations, and dashed lines into ordinary DrawingList paths while retaining the
//! source family, node, link, and annotation semantics.

use super::builder::DrawingListBuilder;
use super::{
    RenderDocument, SvgStructureBody, SvgStructureSidecar, WardleySvgBody, parse_font_families_for,
};
use crate::config::config_font_family_css_raw;
use crate::drawing_list::flowchart::{ellipse_path, polygon_path, rounded_rect_path};
use crate::drawing_list::support::{PortableStyleResolver, stroke, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::wardley::{
    WardleyAnnotationLayout, WardleyAnnotationsBoxLayout, WardleyArrowLayout, WardleyCircleLayout,
    WardleyDiagramLayout, WardleyDominantBaseline, WardleyFontWeight, WardleyLineLayout,
    WardleyNodeShapeLayout, WardleySourceOverlayLayout, WardleyTextAnchor, WardleyTextLayout,
    WardleyTheme,
};
use crate::{Error, Result};
use merman_core::diagrams::wardley::WardleyDiagramRenderModel;
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, DrawingCommand, DrawingListPolicy, FillRule, FontDescriptor, FontStyle, Paint,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle,
    TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Transform,
    Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type WardleyPair = FamilyPair<WardleyDiagramRenderModel, WardleyDiagramLayout>;

#[derive(Debug, Clone, Copy)]
enum WardleyMarker {
    LinkStart,
    LinkEnd,
    TrendEnd,
}

pub(crate) fn build_wardley_document(
    pair: &WardleyPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: impl Into<super::DocumentBudget>,
    session: &RenderSession,
) -> Result<RenderDocument> {
    WardleyBuilder::new(pair, metadata, policy, limits, session)?.build()
}

struct WardleyBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    document: DrawingListBuilder<'a>,
    model: &'a WardleyDiagramRenderModel,
    layout: &'a WardleyDiagramLayout,
    font: FontDescriptor,
    text_obligation: TextObligation,
    background: Color,
    axis: Color,
    axis_text: Color,
    grid: Color,
    component_fill: Color,
    component_stroke: Color,
    component_label: Color,
    link_stroke: Color,
    evolution_stroke: Color,
    white: Color,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
}

impl<'a> WardleyBuilder<'a> {
    fn new(
        pair: &'a WardleyPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        limits: impl Into<super::DocumentBudget>,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout)?;
        let theme = WardleyTheme::from_config(config);
        let styles = PortableStyleResolver::new("wardley");
        let font = FontDescriptor {
            families: parse_font_families_for(
                config_font_family_css_raw(config),
                RenderFamilyKind::Wardley,
            )?,
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };

        let mut document = DrawingListBuilder::new(policy, limits, session);
        document.push_control(DrawingCommand::Save)?;
        document.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "wardley.document".to_string(),
        })?;

        Ok(Self {
            metadata,
            session,
            document,
            model,
            layout,
            background: styles.color("wardley.backgroundColor", &theme.background_color)?,
            axis: styles.color("wardley.axisColor", &theme.axis_color)?,
            axis_text: styles.color("wardley.axisTextColor", &theme.axis_text_color)?,
            grid: styles.color("wardley.gridColor", &theme.grid_color)?,
            component_fill: styles.color("wardley.componentFill", &theme.component_fill)?,
            component_stroke: styles.color("wardley.componentStroke", &theme.component_stroke)?,
            component_label: styles
                .color("wardley.componentLabelColor", &theme.component_label_color)?,
            link_stroke: styles.color("wardley.linkStroke", &theme.link_stroke)?,
            evolution_stroke: styles.color("wardley.evolutionStroke", &theme.evolution_stroke)?,
            white: styles.color("wardley.overlayWhite", "white")?,
            font,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            semantic_classes: BTreeMap::from([(
                "wardley.document".to_string(),
                "wardley-map".to_string(),
            )]),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.document.push_semantic(SemanticAnnotation {
            id: "wardley.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| self.layout.title.as_ref().map(|title| title.text.clone()))
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        })?;

        self.emit_background()?;
        self.emit_title()?;
        self.emit_axes()?;
        self.emit_stages()?;
        self.emit_grid()?;
        self.emit_pipelines()?;
        self.emit_links()?;
        self.emit_trends()?;
        self.emit_nodes()?;
        self.emit_annotations()?;
        self.emit_notes()?;
        self.emit_arrows(
            "wardley.accelerators",
            "wardley-accelerators",
            "wardley.accelerator",
            &self.layout.accelerators,
        )?;
        self.emit_arrows(
            "wardley.deaccelerators",
            "wardley-deaccelerators",
            "wardley.deaccelerator",
            &self.layout.deaccelerators,
        )?;

        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_control(DrawingCommand::Restore)?;

        let document = self.document.finish(
            Viewport::new(Rect::new(0.0, 0.0, self.layout.width, self.layout.height)),
            BTreeMap::from([(
                "x-merman-wardley".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "text_mode": "plain_host_text",
                    "markers": "expanded_svg_marker_geometry",
                    "rotated_labels": "transform_commands",
                    "use_max_width": self.layout.use_max_width,
                }),
            )]),
        )?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Wardley,
                body: SvgStructureBody::Wardley(WardleySvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.layout.use_max_width,
                    acc_title: self
                        .model
                        .acc_title
                        .clone()
                        .filter(|value| !value.is_empty()),
                    acc_description: self
                        .model
                        .acc_descr
                        .clone()
                        .filter(|value| !value.is_empty()),
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                }),
            },
        })
    }

    fn emit_background(&mut self) -> Result<()> {
        self.path_classes.insert(
            "wardley.background".to_string(),
            "wardley-background".to_string(),
        );
        self.add_path(
            "wardley.background",
            rect_path(0.0, 0.0, self.layout.width, self.layout.height, 0.0),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(self.background)),
                stroke: None,
            },
        )
    }

    fn emit_title(&mut self) -> Result<()> {
        let Some(title) = self.layout.title.as_ref() else {
            return Ok(());
        };
        self.text_classes
            .insert("wardley.title".to_string(), "wardley-title".to_string());
        self.emit_text_group(
            "wardley.title",
            title,
            self.axis_text,
            true,
            SemanticRole::Label,
            Some(title.text.clone()),
        )
    }

    fn emit_axes(&mut self) -> Result<()> {
        self.begin_container("wardley.axes", "wardley-axes")?;
        self.add_line(
            "wardley.axis.x",
            self.layout.axes.x_axis,
            self.axis,
            1.0,
            &[],
        )?;
        self.add_line(
            "wardley.axis.y",
            self.layout.axes.y_axis,
            self.axis,
            1.0,
            &[],
        )?;
        self.text_classes.insert(
            "wardley.axis.x.label".to_string(),
            "wardley-axis-label wardley-axis-label-x".to_string(),
        );
        self.emit_text_group(
            "wardley.axis.x.label",
            &self.layout.axes.x_label,
            self.axis_text,
            true,
            SemanticRole::Label,
            None,
        )?;
        self.text_classes.insert(
            "wardley.axis.y.label".to_string(),
            "wardley-axis-label wardley-axis-label-y".to_string(),
        );
        self.emit_text_group(
            "wardley.axis.y.label",
            &self.layout.axes.y_label,
            self.axis_text,
            true,
            SemanticRole::Label,
            None,
        )?;
        self.end_container()?;
        Ok(())
    }

    fn emit_stages(&mut self) -> Result<()> {
        if self.layout.stages.is_empty() {
            return Ok(());
        }
        self.begin_container("wardley.stages", "wardley-stages")?;
        for (index, stage) in self.layout.stages.iter().enumerate() {
            if let Some(divider) = stage.divider {
                self.document.push_control(DrawingCommand::Save)?;
                self.document
                    .push_control(DrawingCommand::SetOpacity { opacity: 0.8 })?;
                self.add_line(
                    format!("wardley.stage.{index}.divider"),
                    divider,
                    Color::rgba(0, 0, 0, 255),
                    1.0,
                    &[5.0, 5.0],
                )?;
                self.document.push_control(DrawingCommand::Restore)?;
            }
            self.text_classes.insert(
                format!("wardley.stage.{index}"),
                "wardley-stage-label".to_string(),
            );
            self.emit_text_group(
                &format!("wardley.stage.{index}"),
                &stage.label,
                self.axis_text,
                false,
                SemanticRole::Label,
                Some(stage.name.clone()),
            )?;
        }
        self.end_container()?;
        Ok(())
    }

    fn emit_grid(&mut self) -> Result<()> {
        if self.layout.grid.is_empty() {
            return Ok(());
        }
        self.begin_container("wardley.grid", "wardley-grid")?;
        for (index, grid) in self.layout.grid.iter().enumerate() {
            self.add_line(
                format!("wardley.grid.{index}.vertical"),
                grid.vertical,
                self.grid,
                1.0,
                &[2.0, 6.0],
            )?;
            self.add_line(
                format!("wardley.grid.{index}.horizontal"),
                grid.horizontal,
                self.grid,
                1.0,
                &[2.0, 6.0],
            )?;
        }
        self.end_container()?;
        Ok(())
    }

    fn emit_pipelines(&mut self) -> Result<()> {
        if self.model.pipelines.is_empty() {
            return Ok(());
        }
        self.begin_container("wardley.pipelines", "wardley-pipelines")?;
        for (index, pipeline) in self.layout.pipeline_boxes.iter().enumerate() {
            self.path_classes.insert(
                format!("wardley.pipeline.{index}.box"),
                "wardley-pipeline-box".to_string(),
            );
            self.add_rect(
                format!("wardley.pipeline.{index}.box"),
                pipeline.rect,
                None,
                Some(stroke(self.axis, 1.5)),
            )?;
        }
        self.end_container()?;
        self.begin_container("wardley.pipeline-links", "wardley-pipeline-links")?;
        for (index, link) in self.layout.pipeline_links.iter().enumerate() {
            self.path_classes.insert(
                format!("wardley.pipeline.{index}.link"),
                "wardley-pipeline-evolution-link".to_string(),
            );
            self.add_line(
                format!("wardley.pipeline.{index}.link"),
                link.line,
                self.link_stroke,
                1.0,
                &[4.0, 4.0],
            )?;
        }
        self.end_container()?;
        Ok(())
    }

    fn emit_links(&mut self) -> Result<()> {
        self.begin_container("wardley.links", "wardley-links")?;
        let mut labels = Vec::new();
        for (index, link) in self.layout.links.iter().enumerate() {
            let semantic_id = format!("wardley.link.{index}");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.path_classes.insert(
                format!("{semantic_id}.line"),
                if link.dashed {
                    "wardley-link wardley-link--dashed"
                } else {
                    "wardley-link"
                }
                .to_string(),
            );
            self.add_line(
                format!("{semantic_id}.line"),
                link.line,
                self.link_stroke,
                1.0,
                if link.dashed {
                    [6.0, 6.0].as_slice()
                } else {
                    &[]
                },
            )?;
            if link.markers.start {
                self.add_marker(
                    format!("{semantic_id}.start"),
                    Point::new(link.line.x1, link.line.y1),
                    Point::new(link.line.x2 - link.line.x1, link.line.y2 - link.line.y1),
                    WardleyMarker::LinkStart,
                    self.link_stroke,
                )?;
            }
            if link.markers.end {
                self.add_marker(
                    format!("{semantic_id}.end"),
                    Point::new(link.line.x2, link.line.y2),
                    Point::new(link.line.x2 - link.line.x1, link.line.y2 - link.line.y1),
                    WardleyMarker::LinkEnd,
                    self.link_stroke,
                )?;
            }
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Edge,
                title: link.label.as_ref().map(|label| label.text.clone()),
                description: Some(format!("{} → {}", link.source, link.target)),
                link: None,
            })?;
            if let Some(label) = link.label.as_ref() {
                labels.push((index, label));
            }
        }
        for (index, label) in labels {
            self.text_classes.insert(
                format!("wardley.link.{index}.label"),
                "wardley-link-label".to_string(),
            );
            self.emit_text_group(
                &format!("wardley.link.{index}.label"),
                label,
                self.axis_text,
                false,
                SemanticRole::Label,
                None,
            )?;
        }
        self.end_container()?;
        Ok(())
    }

    fn emit_trends(&mut self) -> Result<()> {
        self.begin_container("wardley.trends", "wardley-trends")?;
        for (index, trend) in self.layout.trends.iter().enumerate() {
            let semantic_id = format!("wardley.trend.{index}");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.path_classes
                .insert(format!("{semantic_id}.line"), "wardley-trend".to_string());
            self.add_line(
                format!("{semantic_id}.line"),
                trend.line,
                self.evolution_stroke,
                1.0,
                &[4.0, 4.0],
            )?;
            self.add_marker(
                format!("{semantic_id}.end"),
                Point::new(trend.line.x2, trend.line.y2),
                Point::new(trend.line.x2 - trend.line.x1, trend.line.y2 - trend.line.y1),
                WardleyMarker::TrendEnd,
                self.evolution_stroke,
            )?;
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Edge,
                title: None,
                description: Some(format!("evolve {}", trend.node_id)),
                link: None,
            })?;
        }
        self.end_container()?;
        Ok(())
    }

    fn emit_nodes(&mut self) -> Result<()> {
        self.begin_container("wardley.nodes", "wardley-nodes")?;
        for (index, node) in self.layout.nodes.iter().enumerate() {
            let semantic_id = format!("wardley.node.{index}");
            let class = node.class_name.as_deref().map_or_else(
                || "wardley-node".to_string(),
                |class_name| format!("wardley-node wardley-node--{class_name}"),
            );
            self.semantic_classes.insert(semantic_id.clone(), class);
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            if let Some(overlay) = node.source_overlay.as_ref() {
                self.emit_source_overlay(&semantic_id, overlay)?;
            }
            match &node.shape {
                WardleyNodeShapeLayout::Circle { circle } => {
                    self.add_circle(
                        format!("{semantic_id}.shape"),
                        *circle,
                        Some(self.component_fill),
                        Some(stroke(self.component_stroke, 1.0)),
                    )?;
                }
                WardleyNodeShapeLayout::PipelineSquare { rect } => {
                    self.add_rect(
                        format!("{semantic_id}.shape"),
                        *rect,
                        Some(self.component_fill),
                        Some(stroke(self.component_stroke, 1.0)),
                    )?;
                }
                WardleyNodeShapeLayout::Anchor | WardleyNodeShapeLayout::None => {}
            }
            if let Some(inertia) = node.inertia {
                self.path_classes.insert(
                    format!("{semantic_id}.inertia"),
                    "wardley-inertia".to_string(),
                );
                self.add_line(
                    format!("{semantic_id}.inertia"),
                    inertia,
                    self.component_stroke,
                    6.0,
                    &[],
                )?;
            }
            let label_color = match node.class_name.as_deref() {
                Some("evolved") => self.evolution_stroke,
                Some("anchor") => Color::rgba(0, 0, 0, 255),
                _ => self.component_label,
            };
            self.text_classes.insert(
                format!("{semantic_id}.label"),
                "wardley-node-label".to_string(),
            );
            self.emit_text_group(
                &format!("{semantic_id}.label"),
                &node.label_layout,
                label_color,
                true,
                SemanticRole::Label,
                None,
            )?;
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(node.label.clone()),
                description: node.class_name.clone(),
                link: None,
            })?;
        }
        self.end_container()?;
        Ok(())
    }

    fn emit_source_overlay(
        &mut self,
        semantic_id: &str,
        overlay: &WardleySourceOverlayLayout,
    ) -> Result<()> {
        match overlay {
            WardleySourceOverlayLayout::Build { circle } => {
                let id = format!("{semantic_id}.overlay.build");
                self.path_classes
                    .insert(id.clone(), "wardley-build-overlay".to_string());
                self.add_circle(
                    id,
                    *circle,
                    Some(parse_color("#eee")?),
                    Some(stroke(Color::rgba(0, 0, 0, 255), 1.0)),
                )
            }
            WardleySourceOverlayLayout::Buy { circle } => {
                let id = format!("{semantic_id}.overlay.buy");
                self.path_classes
                    .insert(id.clone(), "wardley-buy-overlay".to_string());
                self.add_circle(
                    id,
                    *circle,
                    Some(parse_color("#ccc")?),
                    Some(stroke(self.component_stroke, 1.0)),
                )
            }
            WardleySourceOverlayLayout::Outsource { circle } => {
                let id = format!("{semantic_id}.overlay.outsource");
                self.path_classes
                    .insert(id.clone(), "wardley-outsource-overlay".to_string());
                self.add_circle(
                    id,
                    *circle,
                    Some(parse_color("#666")?),
                    Some(stroke(self.component_stroke, 1.0)),
                )
            }
            WardleySourceOverlayLayout::Market {
                outer_circle,
                connectors,
                dots,
            } => {
                let outer_id = format!("{semantic_id}.overlay.market");
                self.path_classes
                    .insert(outer_id.clone(), "wardley-market-overlay".to_string());
                self.add_circle(
                    outer_id,
                    *outer_circle,
                    Some(self.white),
                    Some(stroke(self.component_stroke, 1.0)),
                )?;
                for (index, connector) in connectors.iter().enumerate() {
                    let id = format!("{semantic_id}.overlay.market.connector.{index}");
                    self.path_classes
                        .insert(id.clone(), "wardley-market-line".to_string());
                    self.add_line(id, *connector, self.component_stroke, 1.0, &[])?;
                }
                for (index, dot) in dots.iter().enumerate() {
                    let id = format!("{semantic_id}.overlay.market.dot.{index}");
                    self.path_classes
                        .insert(id.clone(), "wardley-market-dot".to_string());
                    self.add_circle(
                        id,
                        *dot,
                        Some(self.white),
                        Some(stroke(self.component_stroke, 2.0)),
                    )?;
                }
                Ok(())
            }
        }
    }

    fn emit_annotations(&mut self) -> Result<()> {
        if self.layout.annotations.is_empty() {
            return Ok(());
        }
        self.begin_container("wardley.annotations", "wardley-annotations")?;
        for (index, annotation) in self.layout.annotations.iter().enumerate() {
            let semantic_id = format!("wardley.annotation.{index}");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            for (segment_index, segment) in annotation.segments.iter().enumerate() {
                self.path_classes.insert(
                    format!("{semantic_id}.segment.{segment_index}"),
                    "wardley-annotation-line".to_string(),
                );
                self.add_line(
                    format!("{semantic_id}.segment.{segment_index}"),
                    *segment,
                    self.axis,
                    1.5,
                    &[4.0, 4.0],
                )?;
            }
            for (point_index, point) in annotation.points.iter().enumerate() {
                let point_semantic_id = format!("{semantic_id}.point.{point_index}.group");
                self.semantic_classes
                    .insert(point_semantic_id.clone(), "wardley-annotation".to_string());
                self.document
                    .push_control(DrawingCommand::BeginSemanticGroup {
                        semantic_id: point_semantic_id.clone(),
                    })?;
                self.add_circle(
                    format!("{semantic_id}.point.{point_index}"),
                    WardleyCircleLayout {
                        center: point.center,
                        radius: point.radius,
                    },
                    Some(self.white),
                    Some(stroke(self.axis, 1.5)),
                )?;
                self.emit_text(&point.label, self.axis_text, true)?;
                self.document
                    .push_control(DrawingCommand::EndSemanticGroup)?;
                self.document.push_semantic(SemanticAnnotation {
                    id: point_semantic_id,
                    role: SemanticRole::Label,
                    title: Some(point.label.text.clone()),
                    description: None,
                    link: None,
                })?;
            }
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Group,
                title: Some(annotation.number.to_string()),
                description: None,
                link: None,
            })?;
        }
        if let Some(annotations_box) = self.layout.annotations_box.as_ref() {
            self.emit_annotations_box(annotations_box)?;
        }
        self.end_container()?;
        Ok(())
    }

    fn emit_annotations_box(
        &mut self,
        annotations_box: &WardleyAnnotationsBoxLayout,
    ) -> Result<()> {
        let semantic_id = "wardley.annotations-box";
        self.semantic_classes.insert(
            semantic_id.to_string(),
            "wardley-annotations-box".to_string(),
        );
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.to_string(),
            })?;
        if let Some(rect) = annotations_box.rect {
            self.add_rect(
                format!("{semantic_id}.shape"),
                rect,
                Some(self.white),
                Some(stroke(self.axis, 1.5)),
            )?;
        }
        for (index, line) in annotations_box.lines.iter().enumerate() {
            self.emit_text_group(
                &format!("{semantic_id}.line.{index}"),
                line,
                self.axis_text,
                false,
                SemanticRole::Label,
                None,
            )?;
        }
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Group,
            title: Some("Annotations".to_string()),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_notes(&mut self) -> Result<()> {
        if self.layout.notes.is_empty() {
            return Ok(());
        }
        self.begin_container("wardley.notes", "wardley-notes")?;
        for (index, note) in self.layout.notes.iter().enumerate() {
            self.emit_text_group(
                &format!("wardley.note.{index}"),
                &note.text,
                self.axis_text,
                true,
                SemanticRole::Label,
                Some(note.text.text.clone()),
            )?;
        }
        self.end_container()?;
        Ok(())
    }

    fn emit_arrows(
        &mut self,
        section_id: &str,
        section_class: &str,
        prefix: &str,
        arrows: &[WardleyArrowLayout],
    ) -> Result<()> {
        if arrows.is_empty() {
            return Ok(());
        }
        self.begin_container(section_id, section_class)?;
        for (index, arrow) in arrows.iter().enumerate() {
            let semantic_id = format!("{prefix}.{index}");
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: semantic_id.clone(),
                })?;
            self.add_path(
                format!("{semantic_id}.shape"),
                polygon_path(
                    &arrow
                        .path
                        .iter()
                        .map(|point| Point::new(point.x, point.y))
                        .collect::<Vec<_>>(),
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.white)),
                    stroke: Some(stroke(self.component_stroke, 1.0)),
                },
            )?;
            self.emit_text_group(
                &format!("{semantic_id}.label"),
                &arrow.label,
                self.axis_text,
                true,
                SemanticRole::Label,
                Some(arrow.name.clone()),
            )?;
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
            self.document.push_semantic(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(arrow.name.clone()),
                description: Some(format!("{:?}", arrow.direction)),
                link: None,
            })?;
        }
        self.end_container()?;
        Ok(())
    }

    fn begin_container(&mut self, semantic_id: &str, class: &str) -> Result<()> {
        self.semantic_classes
            .insert(semantic_id.to_string(), class.to_string());
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Group,
            title: None,
            description: None,
            link: None,
        })?;
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.to_string(),
            })
    }

    fn end_container(&mut self) -> Result<()> {
        self.document.push_control(DrawingCommand::EndSemanticGroup)
    }

    fn emit_text_group(
        &mut self,
        semantic_id: &str,
        text: &WardleyTextLayout,
        color: Color,
        include_font_weight: bool,
        role: SemanticRole,
        title: Option<String>,
    ) -> Result<()> {
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.to_string(),
            })?;
        self.emit_text(text, color, include_font_weight)?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id.to_string(),
            role,
            title,
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn emit_text(
        &mut self,
        text: &WardleyTextLayout,
        color: Color,
        _include_font_weight: bool,
    ) -> Result<()> {
        if text.text.is_empty() {
            return Ok(());
        }
        let weight = match text.font_weight {
            WardleyFontWeight::Normal => 400,
            WardleyFontWeight::Bold => 700,
        };
        let measurement_style = MeasurementTextStyle {
            font_family: Some(self.theme_font_family()),
            font_size: text.font_size,
            font_weight: Some(weight.to_string()),
            font_style: None,
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let width = measurer
            .measure_svg_simple_text_bbox_width_px(&text.text, &measurement_style)
            .max(1.0);
        let height = measurer
            .measure_svg_simple_text_bbox_height_px(&text.text, &measurement_style)
            .max(1.0);
        let bounds = text_bounds(text, width, height);
        if let Some(rotation) = text.rotation {
            self.document.push_control(DrawingCommand::Save)?;
            self.document
                .push_control(DrawingCommand::ConcatTransform {
                    transform: rotation_transform(rotation.degrees, rotation.cx, rotation.cy),
                })?;
        }
        let font = self.font.clone();
        let obligation = self.text_obligation.clone();
        let origin = Point::new(text.x, text.y);
        let anchor = map_anchor(text.text_anchor);
        let baseline = map_baseline(text.dominant_baseline);
        self.document
            .draw_host_text(&text.text, move |owned| TextRun {
                text: owned,
                origin,
                bounds,
                style: TextStyle {
                    font: FontDescriptor { weight, ..font },
                    font_size: text.font_size,
                    letter_spacing: 0.0,
                    line_height: text.font_size,
                    fill: Paint::solid(color),
                    stroke: None,
                    paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                },
                anchor,
                baseline,
                direction: TextDirection::Auto,
                language: None,
                obligation,
            })?;
        if text.rotation.is_some() {
            self.document.push_control(DrawingCommand::Restore)?;
        }
        Ok(())
    }

    fn theme_font_family(&self) -> String {
        self.font.families.join(", ")
    }

    fn add_line(
        &mut self,
        id: impl Into<String>,
        line: WardleyLineLayout,
        color: Color,
        width: f64,
        dash: &[f64],
    ) -> Result<()> {
        self.add_path(
            id,
            vec![
                PathSegment::MoveTo {
                    to: Point::new(line.x1, line.y1),
                },
                PathSegment::LineTo {
                    to: Point::new(line.x2, line.y2),
                },
            ],
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(StrokeStyle {
                    dash_array: dash.to_vec(),
                    ..stroke(color, width)
                }),
            },
        )
    }

    fn add_circle(
        &mut self,
        id: impl Into<String>,
        circle: WardleyCircleLayout,
        fill: Option<Color>,
        stroke_style: Option<StrokeStyle>,
    ) -> Result<()> {
        self.add_path(
            id,
            ellipse_path(
                circle.center.x,
                circle.center.y,
                circle.radius,
                circle.radius,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: fill.map(Paint::solid),
                stroke: stroke_style,
            },
        )
    }

    fn add_rect(
        &mut self,
        id: impl Into<String>,
        rect: crate::wardley::WardleyRectLayout,
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
                rect.corner_radius,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: fill.map(Paint::solid),
                stroke: stroke_style,
            },
        )
    }

    fn add_marker(
        &mut self,
        id: impl Into<String>,
        origin: Point,
        tangent: Point,
        marker: WardleyMarker,
        color: Color,
    ) -> Result<()> {
        let length = tangent.x.hypot(tangent.y);
        if !length.is_finite() || length <= f64::EPSILON {
            return Err(unavailable(
                "Wardley marker has no non-zero tangent; DrawingList v1 cannot choose a lossless marker orientation",
            ));
        }
        let ux = tangent.x / length;
        let uy = tangent.y / length;
        let (points, reference_x, marker_size) = match marker {
            WardleyMarker::LinkStart => ([(10.0, 0.0), (0.0, 5.0), (10.0, 10.0)], 1.0, 5.0),
            WardleyMarker::LinkEnd => ([(0.0, 0.0), (10.0, 5.0), (0.0, 10.0)], 9.0, 5.0),
            WardleyMarker::TrendEnd => ([(0.0, 0.0), (10.0, 5.0), (0.0, 10.0)], 9.0, 6.0),
        };
        let scale = marker_size / 10.0;
        let transformed = points.map(|(x, y)| {
            let local_x = (x - reference_x) * scale;
            let local_y = (y - 5.0) * scale;
            Point::new(
                origin.x + ux * local_x - uy * local_y,
                origin.y + uy * local_x + ux * local_y,
            )
        });
        self.add_path(
            id,
            polygon_path(&transformed),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(color)),
                stroke: None,
            },
        )
    }

    fn add_path(
        &mut self,
        id: impl Into<String>,
        segments: Vec<PathSegment>,
        style: PathStyle,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid("Wardley path has no geometry"));
        }
        let id = ResourceId::new(id.into());
        self.document.draw_path(id, segments, style)
    }
}

fn map_anchor(anchor: WardleyTextAnchor) -> TextAnchor {
    match anchor {
        WardleyTextAnchor::Start => TextAnchor::Start,
        WardleyTextAnchor::Middle => TextAnchor::Middle,
    }
}

fn map_baseline(baseline: Option<WardleyDominantBaseline>) -> TextBaseline {
    match baseline {
        Some(WardleyDominantBaseline::Middle) => TextBaseline::Middle,
        Some(WardleyDominantBaseline::Central) => TextBaseline::Central,
        Some(WardleyDominantBaseline::Auto) | None => TextBaseline::Alphabetic,
    }
}

fn text_bounds(text: &WardleyTextLayout, width: f64, height: f64) -> Rect {
    let x = match text.text_anchor {
        WardleyTextAnchor::Start => text.x,
        WardleyTextAnchor::Middle => text.x - width / 2.0,
    };
    let local = [
        Point::new(x, text.y - height / 2.0),
        Point::new(x + width, text.y - height / 2.0),
        Point::new(x + width, text.y + height / 2.0),
        Point::new(x, text.y + height / 2.0),
    ];
    let points = text.rotation.map_or(local, |rotation| {
        local.map(|point| {
            let transform = rotation_transform(rotation.degrees, rotation.cx, rotation.cy);
            Point::new(
                transform.a * point.x + transform.c * point.y + transform.e,
                transform.b * point.x + transform.d * point.y + transform.f,
            )
        })
    });
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

fn rect_path(x: f64, y: f64, width: f64, height: f64, radius: f64) -> Vec<PathSegment> {
    rounded_rect_path(x + width / 2.0, y + height / 2.0, width, height, radius)
}

fn parse_color(value: &str) -> Result<Color> {
    let resolver = PortableStyleResolver::new("wardley");
    resolver.color("overlay", value)
}

fn validate_layout(layout: &WardleyDiagramLayout) -> Result<()> {
    if ![
        layout.width,
        layout.height,
        layout.padding,
        layout.chart_width,
        layout.chart_height,
    ]
    .iter()
    .all(|value| value.is_finite())
        || layout.width <= 0.0
        || layout.height <= 0.0
        || layout.padding < 0.0
    {
        return Err(invalid("Wardley root layout is invalid"));
    }
    validate_text(&layout.axes.x_label)?;
    validate_text(&layout.axes.y_label)?;
    validate_line(layout.axes.x_axis)?;
    validate_line(layout.axes.y_axis)?;
    if let Some(title) = &layout.title {
        validate_text(title)?;
    }
    for stage in &layout.stages {
        validate_text(&stage.label)?;
        if !stage.start_x.is_finite() || !stage.end_x.is_finite() {
            return Err(invalid("Wardley stage geometry is invalid"));
        }
        if let Some(divider) = stage.divider {
            validate_line(divider)?;
        }
    }
    for grid in &layout.grid {
        validate_line(grid.vertical)?;
        validate_line(grid.horizontal)?;
    }
    for pipeline in &layout.pipeline_boxes {
        validate_rect(pipeline.rect)?;
    }
    for link in &layout.pipeline_links {
        validate_line(link.line)?;
    }
    for link in &layout.links {
        validate_line(link.line)?;
        if let Some(label) = &link.label {
            validate_text(label)?;
        }
    }
    for trend in &layout.trends {
        validate_point(trend.origin)?;
        validate_point(trend.target)?;
        validate_line(trend.line)?;
    }
    for node in &layout.nodes {
        validate_point(node.position)?;
        validate_text(&node.label_layout)?;
        validate_shape(&node.shape)?;
        if let Some(overlay) = &node.source_overlay {
            validate_overlay(overlay)?;
        }
        if let Some(inertia) = node.inertia {
            validate_line(inertia)?;
        }
    }
    for annotation in &layout.annotations {
        validate_annotation(annotation)?;
    }
    if let Some(annotations_box) = &layout.annotations_box {
        validate_annotations_box(annotations_box)?;
    }
    for note in &layout.notes {
        validate_text(&note.text)?;
    }
    for arrow in layout
        .accelerators
        .iter()
        .chain(layout.deaccelerators.iter())
    {
        validate_arrow(arrow)?;
    }
    Ok(())
}

fn validate_point(point: crate::wardley::WardleyPointLayout) -> Result<()> {
    if !point.x.is_finite() || !point.y.is_finite() {
        return Err(invalid("Wardley point geometry is invalid"));
    }
    Ok(())
}

fn validate_line(line: WardleyLineLayout) -> Result<()> {
    validate_point(crate::wardley::WardleyPointLayout {
        x: line.x1,
        y: line.y1,
    })?;
    validate_point(crate::wardley::WardleyPointLayout {
        x: line.x2,
        y: line.y2,
    })
}

fn validate_circle(circle: WardleyCircleLayout) -> Result<()> {
    validate_point(circle.center)?;
    if !circle.radius.is_finite() || circle.radius < 0.0 {
        return Err(invalid("Wardley circle geometry is invalid"));
    }
    Ok(())
}

fn validate_rect(rect: crate::wardley::WardleyRectLayout) -> Result<()> {
    if [rect.x, rect.y, rect.width, rect.height, rect.corner_radius]
        .iter()
        .any(|value| !value.is_finite())
        || rect.width < 0.0
        || rect.height < 0.0
        || rect.corner_radius < 0.0
    {
        return Err(invalid("Wardley rectangle geometry is invalid"));
    }
    Ok(())
}

fn validate_text(text: &WardleyTextLayout) -> Result<()> {
    if [text.x, text.y, text.font_size]
        .iter()
        .any(|value| !value.is_finite())
        || text.font_size < 0.0
    {
        return Err(invalid("Wardley text geometry is invalid"));
    }
    if let Some(rotation) = text.rotation
        && [rotation.degrees, rotation.cx, rotation.cy]
            .iter()
            .any(|value| !value.is_finite())
    {
        return Err(invalid("Wardley text rotation is invalid"));
    }
    Ok(())
}

fn validate_shape(shape: &WardleyNodeShapeLayout) -> Result<()> {
    match shape {
        WardleyNodeShapeLayout::Circle { circle } => validate_circle(*circle),
        WardleyNodeShapeLayout::PipelineSquare { rect } => validate_rect(*rect),
        WardleyNodeShapeLayout::Anchor | WardleyNodeShapeLayout::None => Ok(()),
    }
}

fn validate_overlay(overlay: &WardleySourceOverlayLayout) -> Result<()> {
    match overlay {
        WardleySourceOverlayLayout::Build { circle }
        | WardleySourceOverlayLayout::Buy { circle }
        | WardleySourceOverlayLayout::Outsource { circle } => validate_circle(*circle),
        WardleySourceOverlayLayout::Market {
            outer_circle,
            connectors,
            dots,
        } => {
            validate_circle(*outer_circle)?;
            for connector in connectors {
                validate_line(*connector)?;
            }
            for dot in dots {
                validate_circle(*dot)?;
            }
            Ok(())
        }
    }
}

fn validate_annotation(annotation: &WardleyAnnotationLayout) -> Result<()> {
    for point in &annotation.points {
        validate_point(point.center)?;
        if !point.radius.is_finite() || point.radius < 0.0 {
            return Err(invalid("Wardley annotation radius is invalid"));
        }
        validate_text(&point.label)?;
    }
    for segment in &annotation.segments {
        validate_line(*segment)?;
    }
    Ok(())
}

fn validate_annotations_box(annotations_box: &WardleyAnnotationsBoxLayout) -> Result<()> {
    if let Some(rect) = annotations_box.rect {
        validate_rect(rect)?;
    }
    for line in &annotations_box.lines {
        validate_text(line)?;
    }
    if [
        annotations_box.padding,
        annotations_box.line_height,
        annotations_box.font_size,
        annotations_box.max_text_width,
        annotations_box.max_text_height,
    ]
    .iter()
    .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(invalid("Wardley annotation box metrics are invalid"));
    }
    Ok(())
}

fn validate_arrow(arrow: &WardleyArrowLayout) -> Result<()> {
    validate_point(arrow.origin)?;
    if [arrow.width, arrow.height, arrow.head_width]
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(invalid("Wardley arrow geometry is invalid"));
    }
    if arrow.path.is_empty() {
        return Err(invalid("Wardley arrow path is empty"));
    }
    for point in &arrow.path {
        validate_point(*point)?;
    }
    validate_text(&arrow.label)
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "wardley".to_string(),
        reason: message.into(),
    }
}
