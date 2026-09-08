//! Renderer-neutral Packet diagram adapter.
//!
//! Packet diagrams are already fully described by typed block rectangles and text anchors. The
//! adapter resolves the family-local style options and preserves SVG text whitespace behavior;
//! no SVG DOM reconstruction or raster fallback is required.

use super::{
    PacketSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families_for,
};
use crate::drawing_list::builder::DrawingListBuilder;
use crate::drawing_list::flowchart::polygon_path;
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, PacketBlockLayout, PacketDiagramLayout};
use crate::packet::{PACKET_FONT_FAMILY_CSS, PacketConfigView, packet_title, packet_title_y};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::packet::PacketDiagramRenderModel;
use merman_display_list::{
    Color, DrawingCommand, DrawingListPolicy, FillRule, FontDescriptor, FontStyle, Paint,
    PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type PacketPair = FamilyPair<PacketDiagramRenderModel, PacketDiagramLayout>;

pub(crate) fn build_packet_document(
    pair: &PacketPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    limits: impl Into<super::DocumentBudget>,
    session: &RenderSession,
) -> Result<RenderDocument> {
    PacketBuilder::new(pair, metadata, policy, limits, session)?.build()
}

struct PacketBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    document: DrawingListBuilder<'a>,
    model: &'a PacketDiagramRenderModel,
    layout: &'a PacketDiagramLayout,
    title: Option<String>,
    font: FontDescriptor,
    text_obligation: TextObligation,
    byte_font_size: f64,
    start_byte_color: Color,
    end_byte_color: Color,
    label_color: Color,
    label_font_size: f64,
    title_color: Color,
    title_font_size: f64,
    block_stroke_color: Option<Color>,
    block_stroke_width: f64,
    block_fill_color: Option<Color>,
}

impl<'a> PacketBuilder<'a> {
    fn new(
        pair: &'a PacketPair,
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
        let style = PacketConfigView::new(config).style_settings();
        let styles = PortableStyleResolver::new("packet");
        let title = packet_title(model.title.as_deref(), metadata.title.as_deref())
            .map(svg_plain_text)
            .filter(|title| !title.is_empty());
        let mut document = DrawingListBuilder::new(policy, limits, session);
        document.push_control(DrawingCommand::Save)?;
        document.push_control(DrawingCommand::BeginSemanticGroup {
            semantic_id: "packet.document".to_string(),
        })?;

        Ok(Self {
            metadata,
            session,
            document,
            model,
            layout,
            title,
            font: FontDescriptor {
                families: parse_font_families_for(
                    PACKET_FONT_FAMILY_CSS,
                    RenderFamilyKind::Packet,
                )?,
                weight: 400,
                style: FontStyle::Normal,
                postscript_name: None,
                resource: None,
            },
            text_obligation: text_obligation(session, TextMeasurementPhase::Layout),
            byte_font_size: styles.positive_length("byteFontSize", &style.byte_font_size)?,
            start_byte_color: styles.color("startByteColor", &style.start_byte_color)?,
            end_byte_color: styles.color("endByteColor", &style.end_byte_color)?,
            label_color: styles.color("labelColor", &style.label_color)?,
            label_font_size: styles.positive_length("labelFontSize", &style.label_font_size)?,
            title_color: styles.color("titleColor", &style.title_color)?,
            title_font_size: styles.positive_length("titleFontSize", &style.title_font_size)?,
            block_stroke_color: styles
                .optional_color("blockStrokeColor", &style.block_stroke_color)?,
            block_stroke_width: styles.length("blockStrokeWidth", &style.block_stroke_width)?,
            block_fill_color: styles.optional_color("blockFillColor", &style.block_fill_color)?,
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .expect("validated in constructor");
        // The SVG root's white background is real paint, so native hosts must receive it too.
        self.add_path(
            "packet.background".to_string(),
            polygon_path(&[
                Point::new(bounds.min_x, bounds.min_y),
                Point::new(bounds.max_x, bounds.min_y),
                Point::new(bounds.max_x, bounds.max_y),
                Point::new(bounds.min_x, bounds.max_y),
            ]),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(Color::rgba(255, 255, 255, 255))),
                stroke: None,
            },
        )?;
        self.document.push_semantic(SemanticAnnotation {
            id: "packet.document".to_string(),
            role: SemanticRole::Document,
            title: self
                .model
                .acc_title
                .clone()
                .or_else(|| self.title.clone())
                .or_else(|| Some(self.metadata.diagram_type.clone())),
            description: self.model.acc_descr.clone(),
            link: None,
        })?;

        for (word_index, word) in self.layout.words.iter().enumerate() {
            let word_id = format!("packet.word.{word_index}");
            self.document.push_semantic(SemanticAnnotation {
                id: word_id.clone(),
                role: SemanticRole::Group,
                title: None,
                description: None,
                link: None,
            })?;
            self.document
                .push_control(DrawingCommand::BeginSemanticGroup {
                    semantic_id: word_id,
                })?;
            for (block_index, block) in word.blocks.iter().enumerate() {
                self.session.checkpoint(OperationPhase::Emit)?;
                self.emit_block(word_index, block_index, block)?;
            }
            self.document
                .push_control(DrawingCommand::EndSemanticGroup)?;
        }
        self.emit_title()?;

        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_control(DrawingCommand::Restore)?;
        let bounds = self
            .layout
            .bounds
            .as_ref()
            .expect("validated in constructor");
        let document = self.document.finish(
            Viewport::new(Rect::new(
                bounds.min_x,
                bounds.min_y,
                bounds.max_x - bounds.min_x,
                bounds.max_y - bounds.min_y,
            )),
            BTreeMap::from([(
                "x-merman-packet".to_string(),
                json!({
                    "bits_per_row": self.layout.bits_per_row,
                    "diagram_type": self.metadata.diagram_type,
                    "show_bits": self.layout.show_bits,
                    "text_mode": "plain_host_text",
                }),
            )]),
        )?;

        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Packet,
                body: SvgStructureBody::Packet(PacketSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    expose_accessibility_title: self.model.acc_title.is_some(),
                }),
            },
        })
    }

    fn emit_block(
        &mut self,
        word_index: usize,
        block_index: usize,
        block: &PacketBlockLayout,
    ) -> Result<()> {
        let semantic_id = format!("packet.block.{word_index}.{block_index}");
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        self.add_path(
            format!("{semantic_id}.shape"),
            polygon_path(&[
                Point::new(block.x, block.y),
                Point::new(block.x + block.width, block.y),
                Point::new(block.x + block.width, block.y + block.height),
                Point::new(block.x, block.y + block.height),
            ]),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: self.block_fill_color.map(Paint::solid),
                stroke: self
                    .block_stroke_color
                    .map(|color| stroke(color, self.block_stroke_width)),
            },
        )?;

        let label = svg_plain_text(&block.label);
        let semantic_title = (!label.is_empty()).then(|| label.clone());
        let font = &self.font;
        let text_obligation = &self.text_obligation;
        self.document.draw_host_text(&label, |text| {
            packet_text_run(
                text,
                Point::new(block.x + block.width / 2.0, block.y + block.height / 2.0),
                Rect::new(block.x, block.y, block.width, block.height),
                self.label_font_size,
                self.label_color,
                TextAnchor::Middle,
                TextBaseline::Middle,
                font,
                text_obligation,
            )
        })?;

        if self.layout.show_bits {
            self.emit_bit_numbers(block)?;
        }
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Node,
            title: semantic_title,
            description: Some(if block.start == block.end {
                format!("Bit {}", block.start)
            } else {
                format!("Bits {}–{}", block.start, block.end)
            }),
            link: None,
        })?;
        Ok(())
    }

    fn emit_bit_numbers(&mut self, block: &PacketBlockLayout) -> Result<()> {
        let is_single = block.start == block.end;
        let y = block.y - 2.0;
        let start_anchor = if is_single {
            TextAnchor::Middle
        } else {
            TextAnchor::Start
        };
        let start_x = if is_single {
            block.x + block.width / 2.0
        } else {
            block.x
        };
        let bounds = Rect::new(
            block.x,
            y - self.byte_font_size,
            block.width,
            self.byte_font_size,
        );
        let start = block.start.to_string();
        let font = &self.font;
        let text_obligation = &self.text_obligation;
        self.document.draw_host_text(&start, |text| {
            packet_text_run(
                text,
                Point::new(start_x, y),
                bounds,
                self.byte_font_size,
                self.start_byte_color,
                start_anchor,
                TextBaseline::Alphabetic,
                font,
                text_obligation,
            )
        })?;
        if !is_single {
            let end = block.end.to_string();
            self.document.draw_host_text(&end, |text| {
                packet_text_run(
                    text,
                    Point::new(block.x + block.width, y),
                    bounds,
                    self.byte_font_size,
                    self.end_byte_color,
                    TextAnchor::End,
                    TextBaseline::Alphabetic,
                    font,
                    text_obligation,
                )
            })?;
        }
        Ok(())
    }

    fn emit_title(&mut self) -> Result<()> {
        let title = self.title.take().unwrap_or_default();
        self.session.checkpoint(OperationPhase::Emit)?;
        let semantic_id = "packet.title".to_string();
        let total_row_height = self.layout.row_height + self.layout.padding_y;
        let y = packet_title_y(self.layout);
        self.document
            .push_control(DrawingCommand::BeginSemanticGroup {
                semantic_id: semantic_id.clone(),
            })?;
        let font = &self.font;
        let text_obligation = &self.text_obligation;
        self.document.draw_host_text(&title, |text| {
            packet_text_run(
                text,
                Point::new(self.layout.width / 2.0, y),
                Rect::new(
                    0.0,
                    y - total_row_height / 2.0,
                    self.layout.width,
                    total_row_height,
                ),
                self.title_font_size,
                self.title_color,
                TextAnchor::Middle,
                TextBaseline::Middle,
                font,
                text_obligation,
            )
        })?;
        self.document
            .push_control(DrawingCommand::EndSemanticGroup)?;
        self.document.push_semantic(SemanticAnnotation {
            id: semantic_id,
            role: SemanticRole::Label,
            title: Some(title),
            description: None,
            link: None,
        })?;
        Ok(())
    }

    fn add_path(&mut self, id: String, segments: Vec<PathSegment>, style: PathStyle) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid(format!("Packet path `{id}` has no geometry")));
        }
        self.document
            .draw_path(ResourceId::new(id), segments, style)
    }
}

#[allow(clippy::too_many_arguments)]
fn packet_text_run(
    text: String,
    origin: Point,
    bounds: Rect,
    font_size: f64,
    color: Color,
    anchor: TextAnchor,
    baseline: TextBaseline,
    font: &FontDescriptor,
    text_obligation: &TextObligation,
) -> TextRun {
    TextRun {
        text,
        origin,
        bounds,
        style: TextStyle {
            font: font.clone(),
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
        obligation: text_obligation.clone(),
    }
}

fn validate_layout(layout: &PacketDiagramLayout) -> Result<()> {
    let bounds = layout
        .bounds
        .as_ref()
        .ok_or_else(|| invalid("Packet layout did not provide root bounds"))?;
    validate_bounds(bounds)?;
    if ![
        layout.width,
        layout.height,
        layout.row_height,
        layout.padding_x,
        layout.padding_y,
        layout.bit_width,
    ]
    .into_iter()
    .all(f64::is_finite)
        || layout.width <= 0.0
        || layout.height <= 0.0
        || layout.row_height <= 0.0
        || layout.padding_x < 0.0
        || layout.padding_y < 0.0
        || layout.bit_width <= 0.0
        || layout.bits_per_row < 1
    {
        return Err(invalid("Packet layout has invalid root geometry"));
    }
    for block in layout.words.iter().flat_map(|word| &word.blocks) {
        if ![block.x, block.y, block.width, block.height]
            .into_iter()
            .all(f64::is_finite)
            || block.width <= 0.0
            || block.height <= 0.0
            || block.start < 0
            || block.end < block.start
        {
            return Err(invalid(format!(
                "Packet block {}–{} has invalid geometry",
                block.start, block.end
            )));
        }
    }
    Ok(())
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x <= bounds.min_x
        || bounds.max_y <= bounds.min_y
    {
        return Err(invalid("Packet root bounds are invalid"));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}
