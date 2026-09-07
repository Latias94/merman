//! Renderer-neutral Railroad adapter.
//!
//! Railroad already exposes a typed render tree with ordered elements, connector paths, and
//! nested translations. This adapter projects that tree directly and keeps SVG-only selector and
//! group structure out of the public document.

use super::{
    RailroadSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
    parse_svg_path,
};
use crate::drawing_list::flowchart::{ellipse_path, polygon_path, rounded_rect_path};
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{
    Bounds, RailroadDiagramLayout, RailroadElementLayout, RailroadPathLayout, RailroadRuleLayout,
};
use crate::railroad::{RailroadRenderNode, RailroadStyle, railroad_render_node, railroad_style};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::{Error, Result};
use merman_core::diagrams::railroad::{RailroadDiagramRenderModel, RailroadRuleModel};
use merman_core::{OperationPhase, ParseMetadata};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, Paint, PathResource,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle,
    TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Transform,
    Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type RailroadPair = FamilyPair<RailroadDiagramRenderModel, RailroadDiagramLayout>;

pub(crate) fn build_railroad_document(
    pair: &RailroadPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    RailroadBuilder::new(pair, metadata, policy, session)?.build()
}

struct RailroadBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a RailroadDiagramRenderModel,
    layout: &'a RailroadDiagramLayout,
    style: RailroadStyle,
    font: FontDescriptor,
    text_obligation: TextObligation,
    terminal_fill: Option<Color>,
    terminal_stroke: Option<Color>,
    terminal_text: Option<Color>,
    nonterminal_fill: Option<Color>,
    nonterminal_stroke: Option<Color>,
    nonterminal_text: Option<Color>,
    special_fill: Option<Color>,
    special_stroke: Option<Color>,
    line_color: Option<Color>,
    marker_fill: Option<Color>,
    rule_name_color: Option<Color>,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

impl<'a> RailroadBuilder<'a> {
    fn new(
        pair: &'a RailroadPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout, model)?;
        let style = railroad_style(config);
        validate_style(&style)?;
        let styles = PortableStyleResolver::new("railroad");
        let font = FontDescriptor {
            families: parse_font_families(style.font_family.clone()),
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
            terminal_fill: styles.optional_color("terminalFill", &style.terminal_fill)?,
            terminal_stroke: styles.optional_color("terminalStroke", &style.terminal_stroke)?,
            terminal_text: styles
                .optional_color("terminalTextColor", &style.terminal_text_color)?,
            nonterminal_fill: styles.optional_color("nonTerminalFill", &style.non_terminal_fill)?,
            nonterminal_stroke: styles
                .optional_color("nonTerminalStroke", &style.non_terminal_stroke)?,
            nonterminal_text: styles
                .optional_color("nonTerminalTextColor", &style.non_terminal_text_color)?,
            special_fill: styles.optional_color("specialFill", &style.special_fill)?,
            special_stroke: styles.optional_color("specialStroke", &style.special_stroke)?,
            line_color: styles.optional_color("lineColor", &style.line_color)?,
            marker_fill: styles.optional_color("markerFill", &style.marker_fill)?,
            rule_name_color: styles.optional_color("ruleNameColor", &style.rule_name_color)?,
            style,
            font,
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            semantic_classes: BTreeMap::new(),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "railroad.document".to_string(),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.semantics.push(SemanticAnnotation {
            id: "railroad.document".to_string(),
            role: SemanticRole::Document,
            title: self.model.acc_title.clone(),
            description: self.model.acc_descr.clone(),
            link: None,
        });

        self.emit_background()?;
        for rule_index in 0..self.layout.rules.len() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let layout_rule = self.layout.rules[rule_index].clone();
            let model_rule = self.model.rules[rule_index].clone();
            self.emit_rule(rule_index, &layout_rule, &model_rule)?;
        }

        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(0.0, 0.0, self.layout.width, self.layout.height)),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-railroad".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "syntax": self.layout.diagram_type,
                    "text_mode": "plain_host_text",
                    "use_max_width": self.layout.use_max_width,
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Railroad,
                body: SvgStructureBody::Railroad(RailroadSvgBody {
                    diagram_type: self.layout.diagram_type.clone(),
                    use_max_width: self.layout.use_max_width,
                    semantic_classes: self.semantic_classes,
                    path_classes: self.path_classes,
                    text_classes: self.text_classes,
                }),
            },
        })
    }

    fn emit_background(&mut self) -> Result<()> {
        if self.layout.width == 0.0 || self.layout.height == 0.0 {
            return Ok(());
        }
        self.add_path(
            "railroad.background".to_string(),
            polygon_path(&[
                Point::new(0.0, 0.0),
                Point::new(self.layout.width, 0.0),
                Point::new(self.layout.width, self.layout.height),
                Point::new(0.0, self.layout.height),
            ]),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(Color::rgba(255, 255, 255, 255))),
                stroke: None,
            },
        )
    }

    fn emit_rule(
        &mut self,
        rule_index: usize,
        layout_rule: &RailroadRuleLayout,
        model_rule: &RailroadRuleModel,
    ) -> Result<()> {
        let semantic_id = format!("railroad.rule.{rule_index}");
        self.semantic_classes
            .insert(semantic_id.clone(), "railroad-rule".to_string());
        let (render_node, definition_up) = {
            let measurer = self
                .session
                .controlled_text_measurer(TextMeasurementPhase::Layout, OperationPhase::Emit);
            railroad_render_node(&model_rule.definition, &self.style, &measurer)
        };
        self.session.checkpoint(OperationPhase::Emit)?;

        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        self.commands.push(DrawingCommand::Save);
        self.commands.push(DrawingCommand::ConcatTransform {
            transform: translate(layout_rule.x, layout_rule.y),
        });

        self.commands.push(DrawingCommand::Save);
        self.commands.push(DrawingCommand::ConcatTransform {
            transform: translate(
                layout_rule.definition_x,
                layout_rule.baseline_y - definition_up,
            ),
        });
        let mut counters = RuleCounters::default();
        self.emit_render_node(&semantic_id, &render_node, &mut counters)?;
        self.commands.push(DrawingCommand::Restore);

        self.emit_rule_name(&semantic_id, layout_rule)?;
        self.emit_marker(
            &format!("{semantic_id}.start"),
            "Start",
            layout_rule.start_marker_x,
            layout_rule.baseline_y,
            layout_rule.marker_radius,
        )?;
        self.emit_marker(
            &format!("{semantic_id}.end"),
            "End",
            layout_rule.end_marker_x,
            layout_rule.baseline_y,
            layout_rule.marker_radius,
        )?;

        let connector_start = layout_rule
            .paths
            .get(layout_rule.paths.len() - 2)
            .ok_or_else(|| invalid("Railroad rule is missing its start connector"))?
            .clone();
        let connector_end = layout_rule
            .paths
            .last()
            .ok_or_else(|| invalid("Railroad rule is missing its end connector"))?
            .clone();
        self.emit_connector(
            &format!("{semantic_id}.connector.start"),
            &connector_start,
            Some("Start connector".to_string()),
        )?;
        self.emit_connector(
            &format!("{semantic_id}.connector.end"),
            &connector_end,
            Some("End connector".to_string()),
        )?;

        self.commands.push(DrawingCommand::Restore);
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Group,
            title: visible_text(&layout_rule.name),
            description: Some("Railroad grammar rule".to_string()),
            link: None,
        });
        Ok(())
    }

    fn emit_render_node(
        &mut self,
        rule_id: &str,
        node: &RailroadRenderNode,
        counters: &mut RuleCounters,
    ) -> Result<()> {
        match node {
            RailroadRenderNode::Group {
                class,
                transform,
                children,
            } => {
                validate_group_class(class)?;
                if let Some((x, y)) = transform {
                    validate_translation(*x, *y)?;
                    self.commands.push(DrawingCommand::Save);
                    self.commands.push(DrawingCommand::ConcatTransform {
                        transform: translate(*x, *y),
                    });
                }
                for child in children {
                    self.session.checkpoint(OperationPhase::Emit)?;
                    self.emit_render_node(rule_id, child, counters)?;
                }
                if transform.is_some() {
                    self.commands.push(DrawingCommand::Restore);
                }
            }
            RailroadRenderNode::Element { layout, transform } => {
                let element_index = counters.element;
                counters.element += 1;
                self.emit_element(
                    &format!("{rule_id}.element.{element_index}"),
                    layout,
                    *transform,
                )?;
            }
            RailroadRenderNode::Path(path) => {
                let path_index = counters.path;
                counters.path += 1;
                self.emit_connector(
                    &format!("{rule_id}.path.{path_index}"),
                    path,
                    Some("Grammar connector".to_string()),
                )?;
            }
        }
        Ok(())
    }

    fn emit_element(
        &mut self,
        semantic_id: &str,
        element: &RailroadElementLayout,
        transform: Option<(f64, f64)>,
    ) -> Result<()> {
        validate_element(element)?;
        let (fill, stroke_color, text_color, rounded, dashed, kind) = match element.kind.as_str() {
            "terminal" => (
                self.terminal_fill,
                self.terminal_stroke,
                self.terminal_text,
                true,
                false,
                "terminal",
            ),
            "nonterminal" => (
                self.nonterminal_fill,
                self.nonterminal_stroke,
                self.nonterminal_text,
                false,
                false,
                "nonterminal",
            ),
            "special" => (
                self.special_fill,
                self.special_stroke,
                self.nonterminal_text,
                false,
                true,
                "special",
            ),
            other => {
                return Err(invalid(format!(
                    "Railroad element kind `{other}` has no portable presentation"
                )));
            }
        };

        self.semantic_classes
            .insert(semantic_id.to_string(), format!("railroad-{kind}"));
        self.text_classes
            .insert(semantic_id.to_string(), "railroad-label".to_string());

        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.to_string(),
        });
        if let Some((x, y)) = transform {
            validate_translation(x, y)?;
            self.commands.push(DrawingCommand::Save);
            self.commands.push(DrawingCommand::ConcatTransform {
                transform: translate(x, y),
            });
        }

        if element.width > 0.0 && element.height > 0.0 {
            let stroke = stroke_color.and_then(|color| self.shape_stroke(color, dashed));
            if fill.is_some() || stroke.is_some() {
                let shape = if rounded {
                    rounded_rect_path(
                        element.width / 2.0,
                        element.height / 2.0,
                        element.width,
                        element.height,
                        10.0,
                    )
                } else {
                    polygon_path(&[
                        Point::new(0.0, 0.0),
                        Point::new(element.width, 0.0),
                        Point::new(element.width, element.height),
                        Point::new(0.0, element.height),
                    ])
                };
                self.add_path(
                    format!("{semantic_id}.shape"),
                    shape,
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: fill.map(Paint::solid),
                        stroke,
                    },
                )?;
            }
        }
        self.emit_element_text(element, text_color)?;

        if transform.is_some() {
            self.commands.push(DrawingCommand::Restore);
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Node,
            title: visible_text(&element.label),
            description: Some(format!("Railroad {kind}")),
            link: None,
        });
        Ok(())
    }

    fn emit_element_text(
        &mut self,
        element: &RailroadElementLayout,
        color: Option<Color>,
    ) -> Result<()> {
        let Some(color) = color else {
            return Ok(());
        };
        let text = svg_plain_text(&element.label);
        if text.is_empty() || self.style.font_size == 0.0 {
            return Ok(());
        }
        self.commands.push(DrawingCommand::DrawText {
            run: TextRun {
                text,
                origin: Point::new(element.text_x, element.text_y),
                bounds: Rect::new(0.0, 0.0, element.width, element.height),
                style: TextStyle {
                    font: self.font.clone(),
                    font_size: self.style.font_size,
                    letter_spacing: 0.0,
                    line_height: self.style.font_size,
                    fill: Paint::solid(color),
                    stroke: None,
                    paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                },
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Middle,
                direction: TextDirection::Auto,
                language: None,
                obligation: self.text_obligation.clone(),
            },
        });
        Ok(())
    }

    fn emit_rule_name(&mut self, rule_id: &str, rule: &RailroadRuleLayout) -> Result<()> {
        let semantic_id = format!("{rule_id}.name");
        self.semantic_classes
            .insert(semantic_id.clone(), "railroad-rule-name-group".to_string());
        self.text_classes
            .insert(semantic_id.clone(), "railroad-rule-name".to_string());
        let text = svg_plain_text(&format!("{} =", rule.name));
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.clone(),
        });
        if let Some(color) = self.rule_name_color
            && !text.is_empty()
            && self.style.font_size > 0.0
        {
            let origin = Point::new(0.0, rule.baseline_y);
            let bounds = self.measure_text_bounds(
                &text,
                origin,
                700,
                TextAnchor::Start,
                TextBaseline::Alphabetic,
            )?;
            let mut font = self.font.clone();
            font.weight = 700;
            self.commands.push(DrawingCommand::DrawText {
                run: TextRun {
                    text,
                    origin,
                    bounds,
                    style: TextStyle {
                        font,
                        font_size: self.style.font_size,
                        letter_spacing: 0.0,
                        line_height: self.style.font_size,
                        fill: Paint::solid(color),
                        stroke: None,
                        paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
                    },
                    anchor: TextAnchor::Start,
                    baseline: TextBaseline::Alphabetic,
                    direction: TextDirection::Auto,
                    language: None,
                    obligation: self.text_obligation.clone(),
                },
            });
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Label,
            title: visible_text(&rule.name),
            description: Some("Railroad rule name".to_string()),
            link: None,
        });
        Ok(())
    }

    fn emit_marker(
        &mut self,
        semantic_id: &str,
        title: &str,
        x: f64,
        y: f64,
        radius: f64,
    ) -> Result<()> {
        validate_marker(x, y, radius)?;
        self.semantic_classes.insert(
            semantic_id.to_string(),
            if semantic_id.ends_with(".start") {
                "railroad-start".to_string()
            } else {
                "railroad-end".to_string()
            },
        );
        self.path_classes.insert(
            format!("{semantic_id}.shape"),
            if semantic_id.ends_with(".start") {
                "railroad-start".to_string()
            } else {
                "railroad-end".to_string()
            },
        );
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.to_string(),
        });
        if let Some(fill) = self.marker_fill
            && radius > 0.0
        {
            self.add_path(
                format!("{semantic_id}.shape"),
                ellipse_path(x, y, radius, radius),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill)),
                    stroke: None,
                },
            )?;
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Node,
            title: Some(title.to_string()),
            description: Some("Railroad boundary marker".to_string()),
            link: None,
        });
        Ok(())
    }

    fn emit_connector(
        &mut self,
        semantic_id: &str,
        path: &RailroadPathLayout,
        title: Option<String>,
    ) -> Result<()> {
        validate_path(path)?;
        let segments = parse_svg_path(&path.d)?;
        if segments.is_empty() {
            return Err(invalid(format!(
                "Railroad connector `{semantic_id}` has no geometry"
            )));
        }
        self.path_classes
            .insert(format!("{semantic_id}.path"), "railroad-line".to_string());
        self.semantic_classes
            .insert(semantic_id.to_string(), "railroad-line".to_string());
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.to_string(),
        });
        if let Some(color) = self.line_color
            && self.style.stroke_width > 0.0
        {
            if path.x != 0.0 || path.y != 0.0 {
                self.commands.push(DrawingCommand::Save);
                self.commands.push(DrawingCommand::ConcatTransform {
                    transform: translate(path.x, path.y),
                });
            }
            self.add_path(
                format!("{semantic_id}.path"),
                segments,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(color, self.style.stroke_width)),
                },
            )?;
            if path.x != 0.0 || path.y != 0.0 {
                self.commands.push(DrawingCommand::Restore);
            }
        }
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Edge,
            title,
            description: None,
            link: None,
        });
        Ok(())
    }

    fn shape_stroke(&self, color: Color, dashed: bool) -> Option<StrokeStyle> {
        if self.style.stroke_width == 0.0 {
            return None;
        }
        let mut style = stroke(color, self.style.stroke_width);
        if dashed {
            style.dash_array = vec![5.0, 3.0];
        }
        Some(style)
    }

    fn measure_text_bounds(
        &self,
        text: &str,
        origin: Point,
        font_weight: u16,
        anchor: TextAnchor,
        baseline: TextBaseline,
    ) -> Result<Rect> {
        let measurement_style = MeasurementTextStyle {
            font_family: Some(self.style.font_family.clone()),
            font_size: self.style.font_size,
            font_weight: (font_weight != 400).then(|| font_weight.to_string()),
            font_style: None,
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::Layout, OperationPhase::Emit);
        let width = measurer.measure_svg_raw_text_bbox_width_px(text, &measurement_style);
        let height = measurer.measure_svg_simple_text_bbox_height_px(text, &measurement_style);
        self.session.checkpoint(OperationPhase::Emit)?;
        if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
            return Err(invalid("Railroad text measurement returned invalid bounds"));
        }
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
        Ok(Rect::new(x, y, width, height))
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid(format!("Railroad path `{id}` has no geometry")));
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

#[derive(Default)]
struct RuleCounters {
    element: usize,
    path: usize,
}

fn validate_layout(
    layout: &RailroadDiagramLayout,
    model: &RailroadDiagramRenderModel,
) -> Result<()> {
    if ![layout.width, layout.height]
        .into_iter()
        .all(f64::is_finite)
        || layout.width < 0.0
        || layout.height < 0.0
        || !matches!(
            layout.diagram_type.as_str(),
            "railroad" | "railroadEbnf" | "railroadAbnf" | "railroadPeg"
        )
    {
        return Err(invalid(
            "Railroad root geometry or syntax identity is invalid",
        ));
    }
    if let Some(bounds) = &layout.bounds {
        validate_bounds(bounds)?;
    }
    if layout.rules.len() != model.rules.len() {
        return Err(invalid(
            "Railroad semantic and visual rule counts do not agree",
        ));
    }
    for (layout_rule, model_rule) in layout.rules.iter().zip(&model.rules) {
        if layout_rule.name != model_rule.name {
            return Err(invalid(
                "Railroad semantic and visual rule names do not agree",
            ));
        }
        if ![
            layout_rule.x,
            layout_rule.y,
            layout_rule.width,
            layout_rule.height,
            layout_rule.baseline_y,
            layout_rule.name_width,
            layout_rule.definition_x,
            layout_rule.start_marker_x,
            layout_rule.end_marker_x,
            layout_rule.marker_radius,
        ]
        .into_iter()
        .all(f64::is_finite)
            || layout_rule.width < 0.0
            || layout_rule.height < 0.0
            || layout_rule.name_width < 0.0
            || layout_rule.marker_radius < 0.0
            || layout_rule.paths.len() < 2
        {
            return Err(invalid("Railroad rule geometry is invalid"));
        }
        validate_path(&layout_rule.paths[layout_rule.paths.len() - 2])?;
        validate_path(layout_rule.paths.last().expect("length was checked above"))?;
    }
    Ok(())
}

fn validate_style(style: &RailroadStyle) -> Result<()> {
    if ![
        style.padding,
        style.vertical_separation,
        style.horizontal_separation,
        style.arc_radius,
        style.font_size,
        style.stroke_width,
        style.marker_radius,
    ]
    .into_iter()
    .all(f64::is_finite)
        || style.padding < 0.0
        || style.vertical_separation < 0.0
        || style.horizontal_separation < 0.0
        || style.arc_radius < 0.0
        || style.font_size < 0.0
        || style.stroke_width < 0.0
        || style.marker_radius < 0.0
    {
        return Err(invalid("Railroad presentation contains invalid lengths"));
    }
    Ok(())
}

fn validate_group_class(class: &str) -> Result<()> {
    if matches!(
        class,
        "railroad-group"
            | "railroad-sequence"
            | "railroad-choice"
            | "railroad-optional"
            | "railroad-repetition"
    ) {
        Ok(())
    } else {
        Err(invalid(format!(
            "Railroad group class `{class}` has no portable behavior"
        )))
    }
}

fn validate_element(element: &RailroadElementLayout) -> Result<()> {
    if ![
        element.x,
        element.y,
        element.width,
        element.height,
        element.text_x,
        element.text_y,
    ]
    .into_iter()
    .all(f64::is_finite)
        || element.width < 0.0
        || element.height < 0.0
    {
        return Err(invalid("Railroad element geometry is invalid"));
    }
    Ok(())
}

fn validate_path(path: &RailroadPathLayout) -> Result<()> {
    if !path.x.is_finite() || !path.y.is_finite() || path.d.trim().is_empty() {
        return Err(invalid("Railroad connector geometry is invalid"));
    }
    Ok(())
}

fn validate_marker(x: f64, y: f64, radius: f64) -> Result<()> {
    if !x.is_finite() || !y.is_finite() || !radius.is_finite() || radius < 0.0 {
        return Err(invalid("Railroad marker geometry is invalid"));
    }
    Ok(())
}

fn validate_translation(x: f64, y: f64) -> Result<()> {
    if !x.is_finite() || !y.is_finite() {
        return Err(invalid("Railroad transform is invalid"));
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
        return Err(invalid("Railroad root bounds are invalid"));
    }
    Ok(())
}

fn visible_text(value: &str) -> Option<String> {
    let value = svg_plain_text(value);
    (!value.is_empty()).then_some(value)
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
