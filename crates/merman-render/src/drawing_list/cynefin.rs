//! Renderer-neutral Cynefin adapter.
//!
//! Cynefin owns deterministic domain geometry, boundary paths, item placement, and transition
//! control points in its typed layout.  This adapter expands SVG markers and generated paths into
//! ordinary vector resources while retaining opacity, dash, text, and accessibility semantics.

use super::{
    CynefinSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
    parse_svg_path,
};
use crate::config::config_font_family_css;
use crate::cynefin::{
    CynefinTheme, domain_fill, domain_model_and_practice, domain_title, generate_cliff_path,
    generate_confusion_path, generate_fold_path, generate_horizontal_boundary, quadrant_domains,
    resolve_seed,
};
use crate::drawing_list::flowchart::{polygon_path, rounded_rect_path};
use crate::drawing_list::support::{PortableStyleResolver, stroke, text_obligation};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{CynefinDiagramLayout, CynefinDomainLayout};
use crate::text::{TextMeasurer as _, TextStyle as MeasurementTextStyle};
use crate::{Error, Result};
use merman_core::diagrams::cynefin::CynefinDiagramRenderModel;
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

type CynefinPair = FamilyPair<CynefinDiagramRenderModel, CynefinDiagramLayout>;

pub(crate) fn build_cynefin_document(
    pair: &CynefinPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    CynefinBuilder::new(pair, metadata, policy, session)?.build()
}

struct CynefinBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a CynefinDiagramRenderModel,
    layout: &'a CynefinDiagramLayout,
    theme: CynefinTheme,
    font: FontDescriptor,
    text_obligation: TextObligation,
    domain_fills: BTreeMap<String, Color>,
    label_color: Color,
    text_color: Color,
    boundary_color: Color,
    cliff_color: Color,
    arrow_color: Color,
    semantic_classes: BTreeMap<String, String>,
    path_classes: BTreeMap<String, String>,
    text_classes: BTreeMap<String, String>,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
}

#[derive(Clone, Copy)]
struct TextEmitSpec {
    origin: Point,
    font_size: f64,
    weight: u16,
    color: Color,
    anchor: TextAnchor,
    baseline: TextBaseline,
}

impl<'a> CynefinBuilder<'a> {
    fn new(
        pair: &'a CynefinPair,
        metadata: &'a ParseMetadata,
        policy: DrawingListPolicy,
        session: &'a RenderSession,
    ) -> Result<Self> {
        session.checkpoint(OperationPhase::Emit)?;
        let config = metadata.effective_config.as_value();
        let model = pair.semantic();
        let layout = pair.layout();
        validate_layout(layout)?;
        let theme = crate::cynefin::cynefin_theme(config);
        let styles = PortableStyleResolver::new("cynefin");
        let domain_fills = ["complex", "complicated", "chaotic", "clear", "confusion"]
            .into_iter()
            .map(|domain| {
                Ok((
                    domain.to_string(),
                    styles.color(&format!("cynefin.{domain}Bg"), domain_fill(&theme, domain))?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
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
            label_color: styles.color("cynefin.labelColor", &theme.label_color)?,
            text_color: styles.color("cynefin.textColor", &theme.text_color)?,
            boundary_color: styles.color("cynefin.boundaryColor", &theme.boundary_color)?,
            cliff_color: styles.color("cynefin.cliffColor", &theme.cliff_color)?,
            arrow_color: styles.color("cynefin.arrowColor", &theme.arrow_color)?,
            semantic_classes: BTreeMap::from([(
                "cynefin.document".to_string(),
                "cynefin".to_string(),
            )]),
            path_classes: BTreeMap::new(),
            text_classes: BTreeMap::new(),
            theme,
            font,
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            domain_fills,
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "cynefin.document".to_string(),
                },
            ],
            semantics: Vec::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let title = self
            .model
            .title
            .clone()
            .or_else(|| self.metadata.title.clone());
        self.semantics.push(SemanticAnnotation {
            id: "cynefin.document".to_string(),
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
        self.emit_background()?;
        self.commands.push(DrawingCommand::Save);
        self.commands.push(DrawingCommand::ConcatTransform {
            transform: translate(self.layout.padding, self.layout.padding),
        });
        self.emit_domain_backgrounds()?;
        self.emit_boundaries()?;
        self.emit_labels()?;
        if self.layout.show_domain_descriptions {
            self.emit_subtitles()?;
        }
        self.emit_items()?;
        self.emit_transitions()?;
        if let Some(title) = title {
            self.emit_title(&title)?;
        }
        self.commands.push(DrawingCommand::Restore);
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.commands.push(DrawingCommand::Restore);

        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport: Viewport::new(Rect::new(
                0.0,
                0.0,
                self.layout.total_width,
                self.layout.total_height,
            )),
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-cynefin".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "boundary_seed": resolve_seed(self.layout.seed, "cynefin"),
                    "text_mode": "plain_host_text",
                    "svg_marker": "expanded_triangle",
                    "use_max_width": self.layout.use_max_width,
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Cynefin,
                body: SvgStructureBody::Cynefin(CynefinSvgBody {
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
        self.path_classes.insert(
            "cynefin.background".to_string(),
            "cynefinBackground".to_string(),
        );
        self.add_path(
            "cynefin.background",
            rectangle_path(0.0, 0.0, self.layout.total_width, self.layout.total_height),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(Color::rgba(255, 255, 255, 255))),
                stroke: None,
            },
        )
    }

    fn emit_domain_backgrounds(&mut self) -> Result<()> {
        for domain_name in quadrant_domains() {
            let Some(domain) = self.find_domain(domain_name) else {
                continue;
            };
            let (x, y, width, height) = (domain.x, domain.y, domain.width, domain.height);
            let fill = *self
                .domain_fills
                .get(*domain_name)
                .ok_or_else(|| invalid(format!("unknown Cynefin domain `{domain_name}`")))?;
            self.path_classes.insert(
                format!("cynefin.domain.{domain_name}.background"),
                "cynefinDomain".to_string(),
            );
            self.commands.push(DrawingCommand::Save);
            self.commands
                .push(DrawingCommand::SetOpacity { opacity: 0.4 });
            self.add_path(
                format!("cynefin.domain.{domain_name}.background"),
                rectangle_path(x, y, width, height),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(fill)),
                    stroke: None,
                },
            )?;
            self.commands.push(DrawingCommand::Restore);
        }
        Ok(())
    }

    fn emit_boundaries(&mut self) -> Result<()> {
        let seed = resolve_seed(self.layout.seed, "cynefin");
        self.path_classes.insert(
            "cynefin.boundary.fold".to_string(),
            "cynefinBoundary".to_string(),
        );
        self.add_generated_path(
            "cynefin.boundary.fold",
            &generate_fold_path(
                self.layout.width,
                self.layout.height,
                seed,
                Some(self.layout.boundary_amplitude),
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(StrokeStyle {
                    dash_array: vec![6.0, 3.0],
                    ..stroke(self.boundary_color, self.theme.boundary_width)
                }),
            },
        )?;
        self.path_classes.insert(
            "cynefin.boundary.horizontal".to_string(),
            "cynefinBoundary".to_string(),
        );
        self.add_generated_path(
            "cynefin.boundary.horizontal",
            &generate_horizontal_boundary(
                self.layout.width,
                self.layout.height,
                seed + 100.0,
                Some(self.layout.boundary_amplitude),
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(StrokeStyle {
                    dash_array: vec![6.0, 3.0],
                    ..stroke(self.boundary_color, self.theme.boundary_width)
                }),
            },
        )?;
        self.path_classes.insert(
            "cynefin.boundary.cliff".to_string(),
            "cynefinCliff".to_string(),
        );
        self.add_generated_path(
            "cynefin.boundary.cliff",
            &generate_cliff_path(self.layout.width, self.layout.height),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.cliff_color, self.theme.cliff_width)),
            },
        )?;
        self.path_classes.insert(
            "cynefin.domain.confusion.background".to_string(),
            "cynefinConfusion".to_string(),
        );
        self.add_generated_path(
            "cynefin.domain.confusion.background",
            &generate_confusion_path(
                self.layout.width / 2.0,
                self.layout.height / 2.0,
                self.layout.width * 0.15,
                self.layout.height * 0.15,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(with_alpha(
                    self.domain_fills["confusion"],
                    0.5,
                ))),
                stroke: Some(StrokeStyle {
                    dash_array: vec![4.0, 2.0],
                    ..stroke(self.boundary_color, 1.5)
                }),
            },
        )?;
        Ok(())
    }

    fn emit_labels(&mut self) -> Result<()> {
        for domain_name in quadrant_domains() {
            let Some(domain) = self.find_domain(domain_name) else {
                continue;
            };
            let y = if self.layout.show_domain_descriptions {
                domain.cy - 30.0
            } else {
                domain.cy
            };
            self.emit_label_text(
                &format!("cynefin.domain.{domain_name}.label"),
                domain_title(domain_name),
                "cynefinDomainLabel",
                TextEmitSpec {
                    origin: Point::new(domain.cx, y),
                    font_size: self.theme.domain_font_size,
                    weight: 700,
                    color: self.label_color,
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                },
            )?;
        }
        let y = if self.layout.show_domain_descriptions {
            self.layout.height / 2.0 - 10.0
        } else {
            self.layout.height / 2.0
        };
        self.emit_label_text(
            "cynefin.domain.confusion.label",
            domain_title("confusion"),
            "cynefinDomainLabel",
            TextEmitSpec {
                origin: Point::new(self.layout.width / 2.0, y),
                font_size: self.theme.domain_font_size,
                weight: 700,
                color: self.label_color,
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Middle,
            },
        )?;
        Ok(())
    }

    fn emit_subtitles(&mut self) -> Result<()> {
        for domain_name in quadrant_domains() {
            let Some(domain) = self.find_domain(domain_name) else {
                continue;
            };
            let (model, practice) = domain_model_and_practice(domain_name);
            let (cx, cy) = (domain.cx, domain.cy);
            self.emit_label_text(
                &format!("cynefin.domain.{domain_name}.model"),
                model,
                "cynefinSubtitle",
                TextEmitSpec {
                    origin: Point::new(cx, cy - 10.0),
                    font_size: (self.theme.item_font_size - 1.0).max(1.0),
                    weight: 400,
                    color: self.text_color,
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                },
            )?;
            self.emit_label_text(
                &format!("cynefin.domain.{domain_name}.practice"),
                practice,
                "cynefinSubtitle",
                TextEmitSpec {
                    origin: Point::new(cx, cy + 5.0),
                    font_size: (self.theme.item_font_size - 1.0).max(1.0),
                    weight: 400,
                    color: self.text_color,
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                },
            )?;
        }
        self.emit_label_text(
            "cynefin.domain.confusion.subtitle",
            "Disorder",
            "cynefinSubtitle",
            TextEmitSpec {
                origin: Point::new(self.layout.width / 2.0, self.layout.height / 2.0 + 8.0),
                font_size: (self.theme.item_font_size - 1.0).max(1.0),
                weight: 400,
                color: self.text_color,
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Middle,
            },
        )?;
        Ok(())
    }

    fn emit_items(&mut self) -> Result<()> {
        for (index, item) in self.layout.items.iter().enumerate() {
            let semantic_id = format!("cynefin.item.{index}");
            let fill = *self
                .domain_fills
                .get(&item.domain)
                .ok_or_else(|| invalid(format!("unknown Cynefin item domain `{}`", item.domain)))?;
            self.semantic_classes
                .insert(semantic_id.clone(), "cynefin-item".to_string());
            self.path_classes.insert(
                format!("{semantic_id}.shape"),
                if item.overflow {
                    "cynefinItemOverflow"
                } else {
                    "cynefinItem"
                }
                .to_string(),
            );
            self.text_classes
                .insert(semantic_id.clone(), "cynefinItemText".to_string());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            self.add_path(
                format!("{semantic_id}.shape"),
                rounded_rect_path(item.x, item.y, item.width, item.height, 4.0),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(with_alpha(
                        fill,
                        if item.overflow { 0.6 } else { 0.95 },
                    ))),
                    stroke: Some(StrokeStyle {
                        dash_array: if item.overflow {
                            vec![3.0, 2.0]
                        } else {
                            Vec::new()
                        },
                        ..stroke(self.boundary_color, 1.0)
                    }),
                },
            )?;
            self.emit_text(
                &format!("{semantic_id}.label"),
                &item.label,
                TextEmitSpec {
                    origin: Point::new(item.x + item.text_x, item.y + item.text_y),
                    font_size: self.theme.item_font_size,
                    weight: 400,
                    color: self.text_color,
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                },
            )?;
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Node,
                title: Some(item.label.clone()),
                description: Some(format!("{} domain", domain_title(&item.domain))),
                link: None,
            });
        }
        Ok(())
    }

    fn emit_transitions(&mut self) -> Result<()> {
        for (index, transition) in self.layout.transitions.iter().enumerate() {
            let semantic_id = format!("cynefin.transition.{index}");
            self.semantic_classes
                .insert(semantic_id.clone(), "cynefin-transition".to_string());
            self.path_classes.insert(
                format!("{semantic_id}.line"),
                "cynefinArrowLine".to_string(),
            );
            self.path_classes.insert(
                format!("{semantic_id}.arrowhead"),
                "cynefinArrowHead".to_string(),
            );
            self.text_classes
                .insert(semantic_id.clone(), "cynefinArrowLabel".to_string());
            self.commands.push(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            });
            self.add_path(
                format!("{semantic_id}.line"),
                vec![
                    PathSegment::MoveTo {
                        to: Point::new(transition.x1, transition.y1),
                    },
                    PathSegment::QuadTo {
                        control: Point::new(transition.cpx, transition.cpy),
                        to: Point::new(transition.x2, transition.y2),
                    },
                ],
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(stroke(self.arrow_color, self.theme.arrow_width)),
                },
            )?;
            self.add_path(
                format!("{semantic_id}.arrowhead"),
                arrowhead_path(
                    transition.cpx,
                    transition.cpy,
                    transition.x2,
                    transition.y2,
                    self.theme.arrow_width,
                )?,
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.arrow_color)),
                    stroke: None,
                },
            )?;
            if let Some(label) = transition
                .label
                .as_deref()
                .filter(|label| !label.is_empty())
            {
                self.emit_text(
                    &format!("{semantic_id}.label"),
                    label,
                    TextEmitSpec {
                        origin: Point::new(transition.cpx, transition.cpy - 6.0),
                        font_size: (self.theme.item_font_size - 1.0).max(1.0),
                        weight: 400,
                        color: self.text_color,
                        anchor: TextAnchor::Middle,
                        baseline: TextBaseline::Alphabetic,
                    },
                )?;
            }
            self.commands.push(DrawingCommand::EndSemanticGroup);
            self.semantics.push(SemanticAnnotation {
                id: semantic_id,
                role: SemanticRole::Edge,
                title: transition.label.clone(),
                description: Some(format!("{} → {}", transition.from, transition.to)),
                link: None,
            });
        }
        Ok(())
    }

    fn emit_title(&mut self, title: &str) -> Result<()> {
        self.semantic_classes
            .insert("cynefin.title".to_string(), "cynefin-title".to_string());
        self.text_classes
            .insert("cynefin.title".to_string(), "cynefinTitle".to_string());
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: "cynefin.title".to_string(),
        });
        self.emit_text(
            "cynefin.title",
            title,
            TextEmitSpec {
                origin: Point::new(self.layout.width / 2.0, -self.layout.padding / 2.0),
                font_size: self.theme.domain_font_size + 2.0,
                weight: 700,
                color: self.label_color,
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Middle,
            },
        )?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: "cynefin.title".to_string(),
            role: SemanticRole::Label,
            title: Some(title.to_string()),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_label_text(
        &mut self,
        semantic_id: &str,
        text: &str,
        class_name: &str,
        spec: TextEmitSpec,
    ) -> Result<()> {
        self.semantic_classes
            .insert(semantic_id.to_string(), "cynefin-label".to_string());
        self.text_classes
            .insert(semantic_id.to_string(), class_name.to_string());
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: semantic_id.to_string(),
        });
        self.emit_text(semantic_id, text, spec)?;
        self.commands.push(DrawingCommand::EndSemanticGroup);
        self.semantics.push(SemanticAnnotation {
            id: semantic_id.to_string(),
            role: SemanticRole::Label,
            title: Some(text.to_string()),
            description: None,
            link: None,
        });
        Ok(())
    }

    fn emit_text(&mut self, semantic_id: &str, text: &str, spec: TextEmitSpec) -> Result<()> {
        let TextEmitSpec {
            origin,
            font_size,
            weight,
            color,
            anchor,
            baseline,
        } = spec;
        if text.is_empty() {
            return Ok(());
        }
        let measurement_style = MeasurementTextStyle {
            font_size,
            font_family: Some(self.theme.font_family.clone()),
            font_weight: Some(weight.to_string()),
            font_style: (semantic_id.contains(".model")
                || semantic_id.contains(".practice")
                || semantic_id.ends_with(".subtitle"))
            .then(|| "italic".to_string()),
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let width = measurer
            .measure_svg_simple_text_bbox_width_px(text, &measurement_style)
            .max(1.0);
        let height = measurer
            .measure_svg_simple_text_bbox_height_px(text, &measurement_style)
            .max(1.0);
        let bounds = Rect::new(
            match anchor {
                TextAnchor::Start => origin.x,
                TextAnchor::Middle => origin.x - width / 2.0,
                TextAnchor::End => origin.x - width,
            },
            origin.y - height / 2.0,
            width,
            height,
        );
        self.commands.push(DrawingCommand::draw_text(TextRun {
            text: text.to_string(),
            origin,
            bounds,
            style: TextStyle {
                font: FontDescriptor {
                    weight,
                    style: if semantic_id.contains(".model")
                        || semantic_id.contains(".practice")
                        || semantic_id.ends_with(".subtitle")
                    {
                        FontStyle::Italic
                    } else {
                        FontStyle::Normal
                    },
                    ..self.font.clone()
                },
                font_size,
                letter_spacing: 0.0,
                line_height: font_size,
                fill: Paint::solid(color),
                stroke: None,
                paint_order: merman_display_list::TextPaintOrder::FillThenStroke,
            },
            anchor,
            baseline,
            direction: TextDirection::Auto,
            language: None,
            obligation: self.text_obligation.clone(),
        }));
        Ok(())
    }

    fn add_generated_path(&mut self, id: &str, data: &str, style: PathStyle) -> Result<()> {
        self.add_path(id, parse_svg_path(data)?, style)
    }

    fn add_path(
        &mut self,
        id: impl Into<String>,
        segments: Vec<PathSegment>,
        style: PathStyle,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid("Cynefin path has no geometry"));
        }
        let id = ResourceId::new(id.into());
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        self.commands
            .push(DrawingCommand::DrawPath { path: id, style });
        Ok(())
    }

    fn find_domain(&self, name: &str) -> Option<&CynefinDomainLayout> {
        self.layout
            .domain_layouts
            .iter()
            .find(|domain| domain.name == name)
    }
}

fn arrowhead_path(
    control_x: f64,
    control_y: f64,
    end_x: f64,
    end_y: f64,
    stroke_width: f64,
) -> Result<Vec<PathSegment>> {
    let dx = end_x - control_x;
    let dy = end_y - control_y;
    let length = dx.hypot(dy);
    if !length.is_finite() || length <= f64::EPSILON {
        return Err(invalid("Cynefin transition has no arrowhead tangent"));
    }
    let unit_x = dx / length;
    let unit_y = dy / length;
    let normal_x = -unit_y;
    let normal_y = unit_x;
    let arrow_length = 6.0 * stroke_width.max(0.1);
    let half_width = 3.0 * stroke_width.max(0.1);
    let base_x = end_x - unit_x * arrow_length;
    let base_y = end_y - unit_y * arrow_length;
    Ok(polygon_path(&[
        Point::new(end_x, end_y),
        Point::new(
            base_x + normal_x * half_width,
            base_y + normal_y * half_width,
        ),
        Point::new(
            base_x - normal_x * half_width,
            base_y - normal_y * half_width,
        ),
    ]))
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

fn rectangle_path(x: f64, y: f64, width: f64, height: f64) -> Vec<PathSegment> {
    polygon_path(&[
        Point::new(x, y),
        Point::new(x + width, y),
        Point::new(x + width, y + height),
        Point::new(x, y + height),
    ])
}

fn with_alpha(color: Color, opacity: f64) -> Color {
    Color::rgba(
        color.red,
        color.green,
        color.blue,
        (f64::from(color.alpha) * opacity.clamp(0.0, 1.0))
            .round()
            .clamp(0.0, 255.0) as u8,
    )
}

fn validate_layout(layout: &CynefinDiagramLayout) -> Result<()> {
    let values = [
        layout.width,
        layout.height,
        layout.padding,
        layout.total_width,
        layout.total_height,
        layout.boundary_amplitude,
    ];
    if values.iter().any(|value| !value.is_finite())
        || layout.width <= 0.0
        || layout.height <= 0.0
        || layout.total_width <= 0.0
        || layout.total_height <= 0.0
        || layout.padding < 0.0
    {
        return Err(invalid("Cynefin root viewport is invalid"));
    }
    for domain in &layout.domain_layouts {
        validate_rect(domain.x, domain.y, domain.width, domain.height, "domain")?;
        if !domain.cx.is_finite() || !domain.cy.is_finite() {
            return Err(invalid("Cynefin domain center is invalid"));
        }
    }
    for item in &layout.items {
        validate_rect(item.x, item.y, item.width, item.height, "item")?;
        if !item.text_x.is_finite() || !item.text_y.is_finite() {
            return Err(invalid("Cynefin item text position is invalid"));
        }
    }
    for transition in &layout.transitions {
        if [
            transition.x1,
            transition.y1,
            transition.x2,
            transition.y2,
            transition.cpx,
            transition.cpy,
        ]
        .iter()
        .any(|value| !value.is_finite())
        {
            return Err(invalid("Cynefin transition geometry is invalid"));
        }
    }
    Ok(())
}

fn validate_rect(x: f64, y: f64, width: f64, height: f64, kind: &str) -> Result<()> {
    if [x, y, width, height].iter().any(|value| !value.is_finite()) || width <= 0.0 || height <= 0.0
    {
        return Err(invalid(format!("Cynefin {kind} geometry is invalid")));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}
