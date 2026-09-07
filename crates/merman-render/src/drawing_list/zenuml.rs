//! Renderer-neutral ZenUML diagram adapter.
//!
//! ZenUML already exposes a typed statement model and a final vertical layout.  This module
//! projects those facts directly into the DrawingList contract; it never reparses SVG.  The
//! embedded participant and fragment artwork is accepted only when it is a static SVG subset
//! that can be converted into ordinary paths.  CSS, gradients, HTML, and Markdown that cannot be
//! represented without changing pixels fail closed before a document is returned.

use super::{
    RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families_for, parse_svg_path,
};
use crate::config::{config_diagram_look, config_string};
use crate::drawing_list::flowchart::{ellipse_path, polygon_path, rounded_rect_path};
use crate::drawing_list::support::{
    PortableStyleResolver, stroke, svg_plain_text, text_obligation,
};
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::Bounds;
use crate::text::{TextMeasurer, TextStyle as MeasurementTextStyle, split_html_br_lines};
use crate::zenuml::DEFAULT_STARTER;
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::zenuml::{
    ZenumlDiagramRenderModel, ZenumlStatement, ZenumlStatementKind,
};
use merman_display_list::{
    Color, CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument,
    DrawingListPolicy, DrawingResource, FillRule, FontDescriptor, FontStyle, LineCap, LineJoin,
    Paint, PathResource, PathSegment, PathStyle, Point, Rect, ResourceId, SemanticAnnotation,
    SemanticRole, StrokeStyle, TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun,
    TextStyle, Viewport,
};
use roxmltree::{Document as XmlDocument, Node};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

use crate::zenuml::{
    ZenumlArrowStyle, ZenumlCreationLayout, ZenumlDiagramLayout, ZenumlFragmentLayout,
    ZenumlLayoutFragmentKind, ZenumlMessageLayout, ZenumlParticipantLayout,
    resolve_zenuml_emojis_in_text, zenuml_emoji_unicode, zenuml_participant_icon_key,
};

type ZenumlPair = FamilyPair<ZenumlDiagramRenderModel, ZenumlDiagramLayout>;

const FRAME_HEADER_HEIGHT: f64 = 28.0;
const CONTENT_PADDING: f64 = 10.0;
const PARTICIPANT_ICON_SIZE: f64 = 24.0;
const FRAGMENT_ICON_WIDTH: f64 = 20.0;
const FRAGMENT_ICON_HEIGHT: f64 = 24.0;
const MESSAGE_LABEL_FONT_SIZE: f64 = 14.0;
const SEQUENCE_NUMBER_FONT_SIZE: f64 = 12.0;
const FRAGMENT_LABEL_FONT_SIZE: f64 = 14.0;
const COMMENT_FONT_SIZE: f64 = 14.0;
const PARTICIPANT_FONT_SIZE: f64 = 16.0;
const TITLE_FONT_SIZE: f64 = 16.0;
const LINE_HEIGHT_FACTOR: f64 = 1.2;

/// SVG-only metadata retained beside the renderer-neutral ZenUML document.
#[derive(Debug, Clone)]
pub(crate) struct ZenumlSvgBody {
    pub(crate) diagram_type: String,
    pub(crate) use_max_width: bool,
    pub(crate) semantic_classes: BTreeMap<String, String>,
    pub(crate) path_classes: BTreeMap<String, String>,
    pub(crate) text_classes: BTreeMap<String, String>,
    pub(crate) path_data_icons: BTreeMap<String, String>,
    pub(crate) semantic_data_statements: BTreeMap<String, String>,
}

pub(crate) fn build_zenuml_document(
    pair: &ZenumlPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    ZenUmlBuilder::new(pair, metadata, policy, session)?.build()
}

#[derive(Debug, Clone, Copy)]
struct TextPalette {
    frame: Color,
    participant: Color,
    message: Color,
    sequence_number: Color,
    occurrence_fill: Color,
    occurrence_stroke: Color,
    fragment_header: Color,
    fragment_border: Color,
    fragment_separator: Color,
    divider_fill: Color,
    divider_stroke: Color,
    divider_text: Color,
    comment: Color,
    group_title: Color,
}

#[derive(Debug, Clone, Copy)]
struct TextSpec {
    color: Color,
    size: f64,
    weight: u16,
    style: FontStyle,
    opacity: f64,
}

struct ZenUmlBuilder<'a> {
    metadata: &'a ParseMetadata,
    session: &'a RenderSession,
    policy: DrawingListPolicy,
    model: &'a ZenumlDiagramRenderModel,
    layout: &'a ZenumlDiagramLayout,
    content_left: f64,
    header_y: f64,
    view_width: f64,
    view_height: f64,
    use_max_width: bool,
    font: FontDescriptor,
    text_obligation: TextObligation,
    palette: TextPalette,
    resources: Vec<DrawingResource>,
    commands: Vec<DrawingCommand>,
    semantics: Vec<SemanticAnnotation>,
    participant_by_name: HashMap<&'a str, &'a ZenumlParticipantLayout>,
    semantic_stack: Vec<String>,
    semantic_text_counts: BTreeMap<String, usize>,
    svg_semantic_classes: BTreeMap<String, String>,
    svg_path_classes: BTreeMap<String, String>,
    svg_text_classes: BTreeMap<String, String>,
    svg_path_data_icons: BTreeMap<String, String>,
    svg_semantic_data_statements: BTreeMap<String, String>,
}

impl<'a> ZenUmlBuilder<'a> {
    fn new(
        pair: &'a ZenumlPair,
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
                "hand-drawn ZenUML output has no stable vector equivalent in DrawingList v1",
            ));
        }
        let model = pair.semantic();
        let layout = pair.layout();
        validate_model_and_layout(model, layout)?;

        let participant_by_name = layout
            .participants
            .iter()
            .map(|participant| (participant.name.as_str(), participant))
            .collect::<HashMap<_, _>>();
        let statement_ids = collect_statement_ids(model);
        validate_layout_statement_coverage(layout, &statement_ids)?;
        validate_portable_text(model, layout)?;

        let font_family = config_string(config, &["fontFamily"])
            .filter(|family| !family.trim().is_empty())
            .unwrap_or_else(|| "Helvetica, Verdana, serif".to_string());
        let font = FontDescriptor {
            families: parse_font_families_for(font_family, RenderFamilyKind::Zenuml)?,
            weight: 400,
            style: FontStyle::Normal,
            postscript_name: None,
            resource: None,
        };
        let styles = PortableStyleResolver::new("zenuml");
        let palette = TextPalette {
            frame: styles.color("frame", "#666")?,
            participant: styles.color("participant.text", "#222")?,
            message: styles.color("message.text", "#222")?,
            sequence_number: styles.color("sequence.number", "#6b7280")?,
            occurrence_fill: styles.color("occurrence.fill", "#dedede")?,
            occurrence_stroke: styles.color("occurrence.stroke", "#666")?,
            fragment_header: with_alpha(styles.color("fragment.header", "#dedede")?, 0.498),
            fragment_border: styles.color("fragment.border", "#666")?,
            fragment_separator: styles.color("fragment.separator", "#e5e7eb")?,
            divider_fill: styles.color("divider.fill", "#fff5ad")?,
            divider_stroke: styles.color("divider.stroke", "#aa3")?,
            divider_text: styles.color("divider.text", "#333")?,
            comment: styles.color("comment.text", "#333")?,
            group_title: styles.color("group.title", "#222")?,
        };

        let content_left = 1.0 + CONTENT_PADDING + layout.frame_border_left;
        let header_y = FRAME_HEADER_HEIGHT + 6.0;
        let view_width =
            layout.width + content_left + CONTENT_PADDING + layout.frame_border_right + 1.0;
        let view_height = layout.height + CONTENT_PADDING * 2.0 + FRAME_HEADER_HEIGHT - 1.0;
        let use_max_width = config
            .get("sequence")
            .and_then(|sequence| sequence.get("useMaxWidth"))
            .and_then(Value::as_bool)
            .unwrap_or(true);
        validate_positive_finite(view_width, "ZenUML viewport width")?;
        validate_positive_finite(view_height, "ZenUML viewport height")?;

        Ok(Self {
            metadata,
            session,
            policy,
            model,
            layout,
            content_left,
            header_y,
            view_width,
            view_height,
            use_max_width,
            font,
            text_obligation: text_obligation(session, TextMeasurementPhase::SvgBBox),
            palette,
            resources: Vec::new(),
            commands: vec![
                DrawingCommand::Save,
                DrawingCommand::BeginSemanticGroup {
                    semantic_id: "zenuml.document".to_string(),
                },
            ],
            semantics: vec![SemanticAnnotation {
                id: "zenuml.document".to_string(),
                role: SemanticRole::Document,
                title: model.title.clone().or_else(|| metadata.title.clone()),
                description: Some("ZenUML sequence diagram".to_string()),
                link: None,
            }],
            participant_by_name,
            semantic_stack: vec!["zenuml.document".to_string()],
            semantic_text_counts: BTreeMap::new(),
            svg_semantic_classes: BTreeMap::new(),
            svg_path_classes: BTreeMap::new(),
            svg_text_classes: BTreeMap::new(),
            svg_path_data_icons: BTreeMap::new(),
            svg_semantic_data_statements: BTreeMap::new(),
        })
    }

    fn build(mut self) -> Result<RenderDocument> {
        self.emit_frame()?;
        self.emit_groups()?;
        self.emit_lifelines()?;
        self.emit_participants(false)?;
        self.emit_occurrences()?;
        self.emit_participants(true)?;
        self.emit_messages()?;
        self.emit_self_calls()?;
        self.emit_creations()?;
        self.emit_returns()?;
        self.emit_fragments()?;
        self.emit_dividers()?;
        self.emit_comments()?;

        self.commands.push(DrawingCommand::EndSemanticGroup);
        debug_assert_eq!(
            self.semantic_stack.pop().as_deref(),
            Some("zenuml.document")
        );
        self.commands.push(DrawingCommand::Restore);
        let viewport = Viewport::new(Rect::new(0.0, 0.0, self.view_width, self.view_height));
        let document = DrawingListDocument {
            version: DRAWING_LIST_VERSION,
            coordinate_system: CoordinateSystem::LogicalPixelsYDown,
            viewport,
            policy: self.policy,
            resources: self.resources,
            commands: self.commands,
            semantics: self.semantics,
            fallbacks: Vec::new(),
            extensions: BTreeMap::from([(
                "x-merman-zenuml".to_string(),
                json!({
                    "diagram_type": self.metadata.diagram_type,
                    "paint_order": "zenuml-core-svg",
                    "icons": "static_embedded_svg_paths",
                    "markup": "plain_host_text_with_explicit_breaks",
                }),
            )]),
        };
        document.validate().map_err(Error::DrawingListContract)?;
        Ok(RenderDocument {
            public: document,
            svg: SvgStructureSidecar {
                family: RenderFamilyKind::Zenuml,
                body: SvgStructureBody::Zenuml(ZenumlSvgBody {
                    diagram_type: self.metadata.diagram_type.clone(),
                    use_max_width: self.use_max_width,
                    semantic_classes: self.svg_semantic_classes,
                    path_classes: self.svg_path_classes,
                    text_classes: self.svg_text_classes,
                    path_data_icons: self.svg_path_data_icons,
                    semantic_data_statements: self.svg_semantic_data_statements,
                }),
            },
        })
    }

    fn emit_frame(&mut self) -> Result<()> {
        self.add_path(
            "zenuml.frame.outer",
            rounded_rect_path(
                self.view_width / 2.0,
                self.view_height / 2.0,
                self.view_width,
                self.view_height,
                4.0,
            ),
            fill_style(self.palette.frame),
        )?;
        self.add_path(
            "zenuml.frame.inner",
            rounded_rect_path(
                self.view_width / 2.0,
                self.view_height / 2.0,
                self.view_width - 2.0,
                self.view_height - 2.0,
                3.0,
            ),
            fill_style(Color::rgba(255, 255, 255, 255)),
        )?;
        self.add_path(
            "zenuml.frame.header",
            line_path(
                Point::new(1.0, self.header_y - 0.5),
                Point::new(self.view_width - 1.0, self.header_y - 0.5),
            ),
            stroke_style(self.palette.frame, 1.0),
        )?;
        if let Some(title) = self
            .model
            .title
            .as_deref()
            .filter(|title| !title.trim().is_empty())
            .or_else(|| {
                self.metadata
                    .title
                    .as_deref()
                    .filter(|title| !title.trim().is_empty())
            })
        {
            let title = portable_text(title, "ZenUML frame title")?;
            self.emit_text(
                "zenuml.frame.title",
                &resolve_zenuml_emojis_in_text(&title),
                Point::new(5.0, (self.header_y - 0.5) / 2.0),
                TextSpec {
                    color: self.palette.participant,
                    size: TITLE_FONT_SIZE,
                    weight: 600,
                    style: FontStyle::Normal,
                    opacity: 1.0,
                },
                TextAnchor::Start,
                TextBaseline::Middle,
                None,
            )?;
        }
        Ok(())
    }

    fn emit_groups(&mut self) -> Result<()> {
        for (index, group) in self.layout.groups.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let id = format!("zenuml.group.{index}");
            self.begin_semantic(
                &id,
                SemanticRole::Group,
                Some(group.name.clone()),
                "participant group",
            );
            let x = self.x(group.x - 0.5);
            let y = self.y(group.y - 2.0);
            self.add_path(
                format!("{id}.outline"),
                rounded_rect_path(
                    x + (group.width + 1.0) / 2.0,
                    y + (group.height + 1.5) / 2.0,
                    group.width + 1.0,
                    group.height + 1.5,
                    0.0,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(dashed_stroke(self.palette.frame, 1.0, vec![4.0, 3.0])),
                },
            )?;
            if !group.name.trim().is_empty() {
                self.add_path(
                    format!("{id}.title.background"),
                    rounded_rect_path(
                        self.x(group.x + group.width / 2.0),
                        self.y(group.y + 10.0),
                        (group.width - 1.0).max(1.0),
                        19.5,
                        0.0,
                    ),
                    fill_style(Color::rgba(255, 255, 255, 255)),
                )?;
                self.emit_text(
                    format!("{id}.title"),
                    &portable_text(&group.name, "ZenUML group title")?,
                    Point::new(self.x(group.x + group.width / 2.0), self.y(group.y + 10.0)),
                    TextSpec {
                        color: self.palette.group_title,
                        size: 13.0,
                        weight: 400,
                        style: FontStyle::Normal,
                        opacity: 1.0,
                    },
                    TextAnchor::Middle,
                    TextBaseline::Middle,
                    None,
                )?;
            }
            self.end_semantic();
        }
        Ok(())
    }

    fn emit_lifelines(&mut self) -> Result<()> {
        for (index, lifeline) in self.layout.lifelines.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.require_participant(&lifeline.participant_name)?;
            let id = format!("zenuml.lifeline.{index}");
            self.begin_semantic(
                &id,
                SemanticRole::Node,
                Some(lifeline.participant_name.clone()),
                "ZenUML lifeline",
            );
            self.add_path(
                format!("{id}.line"),
                line_path(
                    Point::new(self.x(lifeline.x + 0.5), self.y(lifeline.top_y)),
                    Point::new(self.x(lifeline.x + 0.5), self.y(lifeline.bottom_y)),
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: None,
                    stroke: Some(dashed_stroke(self.palette.frame, 1.0, vec![5.0, 5.0])),
                },
            )?;
            self.end_semantic();
        }
        Ok(())
    }

    fn emit_participants(&mut self, creations_only: bool) -> Result<()> {
        let creation_names = self
            .layout
            .creations
            .iter()
            .map(|creation| creation.participant.name.as_str())
            .collect::<HashSet<_>>();
        for (index, participant) in self.layout.participants.iter().enumerate() {
            if participant.is_starter && !creations_only {
                // The starter is a real visual participant in the source SVG and must remain in
                // the command stream; it is not an invisible routing sentinel.
            }
            let is_creation = creation_names.contains(participant.name.as_str());
            if is_creation != creations_only {
                continue;
            }
            if creations_only && !is_creation {
                continue;
            }
            self.session.checkpoint(OperationPhase::Emit)?;
            let id = format!(
                "zenuml.participant.{index}.{}",
                if creations_only { "creation" } else { "base" }
            );
            let title = (!participant.label.is_empty()).then(|| participant.label.clone());
            self.begin_semantic(&id, SemanticRole::Node, title, "ZenUML participant");
            self.emit_participant_shape(&id, participant)?;
            self.end_semantic();
        }
        Ok(())
    }

    fn emit_participant_shape(
        &mut self,
        id: &str,
        participant: &ZenumlParticipantLayout,
    ) -> Result<()> {
        let x = self.x(participant.x - participant.width / 2.0 + 1.0);
        let y = self.y(participant.y + 1.0);
        let fill = participant
            .color
            .as_deref()
            .map(|value| {
                let value = value.strip_prefix('#').unwrap_or(value);
                format!("#{value}")
            })
            .map(|value| PortableStyleResolver::new("zenuml").color("participant.fill", &value))
            .transpose()?
            .unwrap_or(Color::rgba(255, 255, 255, 255));
        self.add_path(
            format!("{id}.box"),
            rounded_rect_path(
                x + (participant.width - 2.0) / 2.0,
                y + (participant.height - 2.0) / 2.0,
                (participant.width - 2.0).max(1.0),
                (participant.height - 2.0).max(1.0),
                3.0,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: Some(Paint::solid(fill)),
                stroke: Some(stroke(self.palette.frame, 2.0)),
            },
        )?;
        if participant.is_starter {
            self.emit_asset_icon(
                &format!("{id}.icon"),
                "actor",
                self.x(participant.x - 14.0),
                self.y(participant.y + 8.0),
                PARTICIPANT_ICON_SIZE,
                AssetContext::Participant,
            )?;
            return Ok(());
        }

        let text_y = participant.y + participant.height / 2.0 - 0.25;
        let label_y = if participant.stereotype.is_some() {
            text_y + 8.0
        } else {
            text_y
        };
        let icon = participant
            .participant_type
            .as_deref()
            .and_then(zenuml_participant_icon_key);
        let emoji = participant
            .emoji
            .as_deref()
            .map(|shortcode| zenuml_emoji_unicode(shortcode).unwrap_or(shortcode));
        let mut text_x = participant.x;
        let mut anchor = TextAnchor::Middle;
        if let Some(icon) = icon {
            let emoji_extra = if emoji.is_some() { 20.0 } else { 0.0 };
            let group_width = 24.0 + 4.0 + 16.0 + participant.label_width + emoji_extra;
            let group_x = participant.x - group_width / 2.0;
            self.emit_asset_icon(
                &format!("{id}.icon"),
                icon,
                self.x(group_x + 4.0),
                self.y(participant.y
                    + (participant.height - 24.0) / 2.0
                    + if icon == "boundary" { 2.75 } else { 0.0 }),
                PARTICIPANT_ICON_SIZE,
                AssetContext::Participant,
            )?;
            if let Some(emoji) = emoji {
                self.emit_text(
                    format!("{id}.emoji"),
                    emoji,
                    Point::new(self.x(group_x + 28.0), self.y(label_y)),
                    TextSpec {
                        color: self.palette.participant,
                        size: PARTICIPANT_FONT_SIZE,
                        weight: 400,
                        style: FontStyle::Normal,
                        opacity: 1.0,
                    },
                    TextAnchor::Start,
                    TextBaseline::Middle,
                    None,
                )?;
                text_x = group_x + 52.0;
            } else {
                text_x = group_x + 36.0;
            }
            anchor = TextAnchor::Start;
        } else if let Some(emoji) = emoji {
            let group_width = 28.0 + participant.label_width;
            let inner_width = participant
                .stereotype_width
                .unwrap_or_default()
                .max(group_width);
            let group_x = participant.x - inner_width / 2.0;
            self.emit_text(
                format!("{id}.emoji"),
                emoji,
                Point::new(self.x(group_x), self.y(label_y)),
                TextSpec {
                    color: self.palette.participant,
                    size: PARTICIPANT_FONT_SIZE,
                    weight: 400,
                    style: FontStyle::Normal,
                    opacity: 1.0,
                },
                TextAnchor::Start,
                TextBaseline::Middle,
                None,
            )?;
            text_x = group_x + 24.0;
            anchor = TextAnchor::Start;
        }
        if let Some(stereotype) = participant.stereotype.as_deref() {
            let stereotype_x = if icon.is_some() {
                text_x + participant.label_width / 2.0
            } else {
                participant.x
            };
            self.emit_text(
                format!("{id}.stereotype"),
                &format!("«{}»", portable_text(stereotype, "ZenUML stereotype")?),
                Point::new(self.x(stereotype_x), self.y(text_y - 8.0)),
                TextSpec {
                    color: self.palette.participant,
                    size: PARTICIPANT_FONT_SIZE,
                    weight: 400,
                    style: FontStyle::Normal,
                    opacity: 1.0,
                },
                TextAnchor::Middle,
                TextBaseline::Middle,
                None,
            )?;
        }
        let label = portable_text(&participant.label, "ZenUML participant label")?;
        let letter_spacing = if participant.name.contains(':') {
            let measured =
                self.measure_width(&label, PARTICIPANT_FONT_SIZE, 400, FontStyle::Normal);
            let chars = label.chars().count();
            (participant.label_width - measured) / chars.saturating_sub(1).max(1) as f64
        } else {
            0.0
        };
        self.emit_text(
            format!("{id}.label"),
            &resolve_zenuml_emojis_in_text(&label),
            Point::new(self.x(text_x), self.y(label_y)),
            TextSpec {
                color: self.palette.participant,
                size: PARTICIPANT_FONT_SIZE,
                weight: 400,
                style: FontStyle::Normal,
                opacity: 1.0,
            },
            anchor,
            TextBaseline::Middle,
            Some(letter_spacing),
        )?;
        Ok(())
    }

    fn emit_occurrences(&mut self) -> Result<()> {
        for (index, occurrence) in self.layout.occurrences.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.require_participant(&occurrence.participant_name)?;
            let id = format!("zenuml.occurrence.{index}");
            self.begin_semantic(
                &id,
                SemanticRole::Node,
                Some(occurrence.statement_id.clone()),
                "ZenUML occurrence",
            );
            self.svg_semantic_data_statements
                .insert(id.clone(), occurrence.statement_id.clone());
            self.add_path(
                format!("{id}.bar"),
                rounded_rect_path(
                    self.x(occurrence.x + occurrence.width / 2.0),
                    self.y(occurrence.y + occurrence.height / 2.0),
                    (occurrence.width - 2.0).max(1.0),
                    (occurrence.height - 2.0).max(1.0),
                    1.0,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.palette.occurrence_fill)),
                    stroke: Some(stroke(self.palette.occurrence_stroke, 2.0)),
                },
            )?;
            self.end_semantic();
        }
        Ok(())
    }

    fn emit_messages(&mut self) -> Result<()> {
        for (index, message) in self.layout.messages.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.emit_message(index, message, false)?;
        }
        Ok(())
    }

    fn emit_message(
        &mut self,
        index: usize,
        message: &ZenumlMessageLayout,
        creation: bool,
    ) -> Result<()> {
        self.validate_endpoint(&message.from)?;
        self.validate_endpoint(&message.to)?;
        let id = format!(
            "zenuml.{}message.{index}",
            if creation { "creation-" } else { "" }
        );
        self.begin_semantic(
            &id,
            SemanticRole::Edge,
            Some(portable_text(&message.label, "ZenUML message label")?),
            "ZenUML message",
        );
        self.svg_semantic_data_statements
            .insert(id.clone(), message.statement_id.clone());
        let left_to_right = message.from_x < message.to_x;
        let from_x = if left_to_right {
            message.from_x + 1.0
        } else {
            message.from_x
        };
        let to_x = if left_to_right {
            message.to_x
        } else {
            message.to_x + 1.0
        };
        let line_y = self.y(if creation { message.y } else { message.y - 0.5 });
        let style = message_text_spec(&message.style, self.palette.message, self.session)?;
        self.add_path(
            format!("{id}.line"),
            line_path(
                Point::new(self.x(from_x), line_y),
                Point::new(self.x(to_x), line_y),
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(if message.arrow_style == ZenumlArrowStyle::Dashed {
                    dashed_stroke(Color::rgba(0, 0, 0, 255), 2.0, vec![6.0, 4.0])
                } else {
                    stroke(Color::rgba(0, 0, 0, 255), 2.0)
                }),
            },
        )?;
        let tip_x = if creation {
            self.x(message.to_x)
        } else {
            self.x(to_x)
        };
        self.add_path(
            format!("{id}.arrow"),
            arrow_head_path(tip_x, line_y, message.is_reverse, message.arrow_style),
            arrow_style(message.arrow_style),
        )?;
        let direction = if left_to_right { 1.0 } else { -1.0 };
        let label_x = (message.from_x + message.to_x) / 2.0 - direction * 3.5 + 0.5;
        self.emit_text(
            format!("{id}.label"),
            &resolve_zenuml_emojis_in_text(&portable_text(&message.label, "ZenUML message label")?),
            Point::new(self.x(label_x), self.y(message.y - 3.5)),
            style,
            TextAnchor::Middle,
            TextBaseline::Middle,
            None,
        )?;
        self.emit_text(
            format!("{id}.number"),
            &message.number,
            Point::new(self.x(from_x.min(to_x) - 4.0), self.y(message.y - 3.5)),
            TextSpec {
                color: self.palette.sequence_number,
                size: SEQUENCE_NUMBER_FONT_SIZE,
                weight: 100,
                style: FontStyle::Normal,
                opacity: 1.0,
            },
            TextAnchor::End,
            TextBaseline::Middle,
            None,
        )?;
        self.end_semantic();
        Ok(())
    }

    fn emit_self_calls(&mut self) -> Result<()> {
        for (index, call) in self.layout.self_calls.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.require_participant(&call.participant_name)?;
            let id = format!("zenuml.self-call.{index}");
            self.begin_semantic(
                &id,
                SemanticRole::Edge,
                Some(portable_text(&call.label, "ZenUML self-call label")?),
                "ZenUML self-call",
            );
            self.svg_semantic_data_statements
                .insert(id.clone(), call.statement_id.clone());
            let asynchronous = call.arrow_style == ZenumlArrowStyle::Open;
            let svg_y = call.y + if asynchronous { 20.0 } else { 14.0 };
            let end_x = if asynchronous { 1.0 } else { 14.0 };
            let x = self.x(call.x + 1.0);
            let y = self.y(svg_y);
            let mut route = vec![
                PathSegment::MoveTo {
                    to: Point::new(x, y + 2.0),
                },
                PathSegment::LineTo {
                    to: Point::new(x + 26.0, y + 2.0),
                },
                PathSegment::QuadTo {
                    control: Point::new(x + 28.0, y + 2.0),
                    to: Point::new(x + 28.0, y + 4.0),
                },
                PathSegment::LineTo {
                    to: Point::new(x + 28.0, y + 13.0),
                },
                PathSegment::QuadTo {
                    control: Point::new(x + 28.0, y + 15.0),
                    to: Point::new(x + 26.0, y + 15.0),
                },
                PathSegment::LineTo {
                    to: Point::new(x + end_x, y + 15.0),
                },
            ];
            if route.len() < 2 {
                return Err(invalid("ZenUML self-call route has no geometry"));
            }
            self.add_path(
                format!("{id}.route"),
                std::mem::take(&mut route),
                stroke_style(Color::rgba(0, 0, 0, 255), 2.0),
            )?;
            self.add_path(
                format!("{id}.arrow"),
                left_arrow_path(Point::new(x + end_x, y + 15.0), call.arrow_style),
                arrow_style(call.arrow_style),
            )?;
            let style = message_text_spec(&call.style, self.palette.message, self.session)?;
            self.emit_text(
                format!("{id}.label"),
                &resolve_zenuml_emojis_in_text(&portable_text(
                    call.label.trim(),
                    "ZenUML self-call label",
                )?),
                Point::new(
                    self.x(call.x + 6.0),
                    self.y(call.y + if asynchronous { 15.0 } else { 12.0 }),
                ),
                style,
                TextAnchor::Start,
                TextBaseline::Middle,
                None,
            )?;
            self.emit_text(
                format!("{id}.number"),
                &call.number,
                Point::new(self.x(call.x - 3.0), self.y(call.y + 12.0)),
                TextSpec {
                    color: self.palette.sequence_number,
                    size: SEQUENCE_NUMBER_FONT_SIZE,
                    weight: 100,
                    style: FontStyle::Normal,
                    opacity: 1.0,
                },
                TextAnchor::End,
                TextBaseline::Middle,
                None,
            )?;
            self.end_semantic();
        }
        Ok(())
    }

    fn emit_creations(&mut self) -> Result<()> {
        for (index, creation) in self.layout.creations.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.require_participant(&creation.participant.name)?;
            let id = format!("zenuml.creation.{index}");
            self.begin_semantic(
                &id,
                SemanticRole::Edge,
                Some(portable_text(
                    &creation.message.label,
                    "ZenUML creation label",
                )?),
                "ZenUML creation",
            );
            self.svg_semantic_data_statements
                .insert(id.clone(), creation.statement_id.clone());
            self.emit_creation_geometry(&id, creation)?;
            self.end_semantic();
        }
        Ok(())
    }

    fn emit_creation_geometry(&mut self, id: &str, creation: &ZenumlCreationLayout) -> Result<()> {
        let message = &creation.message;
        let participant = &creation.participant;
        let reverse = message.to_x < message.from_x;
        let to_x = if reverse {
            participant.x + participant.width / 2.0
        } else {
            participant.x - participant.width / 2.0
        };
        let from_x = if reverse {
            message.from_x
        } else {
            message.from_x + 1.0
        };
        let y = self.y(message.y);
        self.add_path(
            format!("{id}.line"),
            line_path(Point::new(self.x(from_x), y), Point::new(self.x(to_x), y)),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(dashed_stroke(
                    Color::rgba(0, 0, 0, 255),
                    2.0,
                    vec![6.0, 4.0],
                )),
            },
        )?;
        self.add_path(
            format!("{id}.arrow"),
            open_arrow_path(Point::new(self.x(to_x), y), reverse),
            stroke_style(Color::rgba(0, 0, 0, 255), 2.0),
        )?;
        let label_x = from_x + (to_x - from_x) / 2.0 + if reverse { 3.5 } else { -3.0 };
        let style = message_text_spec(&message.style, self.palette.message, self.session)?;
        self.emit_text(
            format!("{id}.label"),
            &resolve_zenuml_emojis_in_text(&portable_text(
                &message.label,
                "ZenUML creation label",
            )?),
            Point::new(self.x(label_x), self.y(message.y - 3.0)),
            style,
            TextAnchor::Middle,
            TextBaseline::Middle,
            None,
        )?;
        self.emit_text(
            format!("{id}.number"),
            &message.number,
            Point::new(self.x(from_x.min(to_x) - 4.0), self.y(message.y - 3.0)),
            TextSpec {
                color: self.palette.sequence_number,
                size: SEQUENCE_NUMBER_FONT_SIZE,
                weight: 100,
                style: FontStyle::Normal,
                opacity: 1.0,
            },
            TextAnchor::End,
            TextBaseline::Middle,
            None,
        )?;
        Ok(())
    }

    fn emit_returns(&mut self) -> Result<()> {
        for (index, returned) in self.layout.returns.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            self.validate_endpoint(&returned.from)?;
            self.validate_endpoint(&returned.to)?;
            let id = format!("zenuml.return.{index}");
            self.begin_semantic(
                &id,
                SemanticRole::Edge,
                Some(portable_text(&returned.label, "ZenUML return label")?),
                "ZenUML return",
            );
            self.svg_semantic_data_statements
                .insert(id.clone(), returned.statement_id.clone());
            self.svg_semantic_classes.insert(
                id.clone(),
                if returned.is_self {
                    "return return-self"
                } else {
                    "return"
                }
                .to_string(),
            );
            if returned.is_self {
                let icon_x = returned.from_x + 4.0;
                let icon_y = returned.y - 12.0;
                self.add_path(
                    format!("{id}.icon.circle"),
                    ellipse_path(self.x(icon_x + 8.0), self.y(icon_y + 8.0), 8.0, 8.0),
                    stroke_style(self.palette.participant, 1.0),
                )?;
                self.add_path(
                    format!("{id}.icon.arrow"),
                    left_arrow_path(
                        Point::new(self.x(icon_x + 8.0), self.y(icon_y + 8.0)),
                        ZenumlArrowStyle::Open,
                    ),
                    stroke_style(self.palette.participant, 1.0),
                )?;
                self.emit_text(
                    format!("{id}.label"),
                    &resolve_zenuml_emojis_in_text(&portable_text(
                        &returned.label,
                        "ZenUML return label",
                    )?),
                    Point::new(self.x(icon_x + 16.0), self.y(returned.y - 1.0)),
                    TextSpec {
                        color: self.palette.message,
                        size: MESSAGE_LABEL_FONT_SIZE,
                        weight: 400,
                        style: FontStyle::Normal,
                        opacity: 1.0,
                    },
                    TextAnchor::Start,
                    TextBaseline::Middle,
                    None,
                )?;
            } else {
                let line_y = returned.y.floor();
                self.add_path(
                    format!("{id}.line"),
                    line_path(
                        Point::new(self.x(returned.from_x), self.y(line_y)),
                        Point::new(self.x(returned.to_x), self.y(line_y)),
                    ),
                    PathStyle {
                        fill_rule: FillRule::NonZero,
                        fill: None,
                        stroke: Some(dashed_stroke(
                            Color::rgba(0, 0, 0, 255),
                            2.0,
                            vec![6.0, 4.0],
                        )),
                    },
                )?;
                self.add_path(
                    format!("{id}.arrow"),
                    open_arrow_path(
                        Point::new(self.x(returned.to_x), self.y(line_y)),
                        returned.is_reverse,
                    ),
                    stroke_style(Color::rgba(0, 0, 0, 255), 2.0),
                )?;
                let label_x = returned.from_x.min(returned.to_x)
                    + (returned.to_x - returned.from_x).abs() / 2.0
                    + if returned.is_reverse { 3.5 } else { -3.5 };
                self.emit_text(
                    format!("{id}.label"),
                    &resolve_zenuml_emojis_in_text(&portable_text(
                        &returned.label,
                        "ZenUML return label",
                    )?),
                    Point::new(self.x(label_x), self.y(line_y - 3.0)),
                    TextSpec {
                        color: self.palette.message,
                        size: MESSAGE_LABEL_FONT_SIZE,
                        weight: 400,
                        style: FontStyle::Normal,
                        opacity: 1.0,
                    },
                    TextAnchor::Middle,
                    TextBaseline::Middle,
                    None,
                )?;
                self.emit_text(
                    format!("{id}.number"),
                    &returned.number,
                    Point::new(
                        self.x(returned.from_x.min(returned.to_x) - 4.0),
                        self.y(line_y - 3.0),
                    ),
                    TextSpec {
                        color: self.palette.sequence_number,
                        size: SEQUENCE_NUMBER_FONT_SIZE,
                        weight: 100,
                        style: FontStyle::Normal,
                        opacity: 1.0,
                    },
                    TextAnchor::End,
                    TextBaseline::Middle,
                    None,
                )?;
            }
            self.end_semantic();
        }
        Ok(())
    }

    fn emit_fragments(&mut self) -> Result<()> {
        for (index, fragment) in self.layout.fragments.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let id = format!("zenuml.fragment.{index}");
            self.begin_semantic(
                &id,
                SemanticRole::Group,
                Some(portable_text(&fragment.label, "ZenUML fragment label")?),
                "ZenUML fragment",
            );
            self.svg_semantic_data_statements
                .insert(id.clone(), fragment.statement_id.clone());
            self.svg_semantic_classes.insert(
                id.clone(),
                format!(
                    "fragment fragment-{}",
                    fragment_kind_css_suffix(fragment.kind)
                ),
            );
            self.emit_fragment_geometry(&id, fragment)?;
            self.end_semantic();
        }
        Ok(())
    }

    fn emit_fragment_geometry(&mut self, id: &str, fragment: &ZenumlFragmentLayout) -> Result<()> {
        let x = self.x(fragment.x);
        let y = self.y(fragment.y);
        self.add_path(
            format!("{id}.border"),
            rounded_rect_path(
                x + fragment.width / 2.0,
                y + fragment.height / 2.0,
                (fragment.width - 1.0).max(1.0),
                (fragment.height - 1.0).max(1.0),
                4.0,
            ),
            PathStyle {
                fill_rule: FillRule::NonZero,
                fill: None,
                stroke: Some(stroke(self.palette.fragment_border, 1.0)),
            },
        )?;
        let header_x = self.x(fragment.x + 1.0);
        self.add_path(
            format!("{id}.header"),
            rect_path(
                header_x,
                self.y(fragment.header_y),
                (fragment.width - 2.0).max(1.0),
                25.0,
            ),
            fill_style(self.palette.fragment_header),
        )?;
        self.emit_asset_icon(
            &format!("{id}.icon"),
            fragment_icon_key(fragment.kind),
            self.x(fragment.x + 5.0),
            self.y(fragment.header_y),
            FRAGMENT_ICON_WIDTH.max(FRAGMENT_ICON_HEIGHT),
            AssetContext::Fragment,
        )?;
        self.emit_text(
            format!("{id}.kind"),
            fragment_kind_label(fragment.kind),
            Point::new(self.x(fragment.x + 27.0), self.y(fragment.header_y + 12.0)),
            TextSpec {
                color: Color::rgba(0, 0, 0, 255),
                size: FRAGMENT_LABEL_FONT_SIZE,
                weight: 600,
                style: FontStyle::Normal,
                opacity: 1.0,
            },
            TextAnchor::Start,
            TextBaseline::Middle,
            None,
        )?;
        self.emit_text(
            format!("{id}.number"),
            &fragment.number,
            Point::new(self.x(fragment.x - 3.0), self.y(fragment.header_y + 8.0)),
            TextSpec {
                color: self.palette.sequence_number,
                size: SEQUENCE_NUMBER_FONT_SIZE,
                weight: 100,
                style: FontStyle::Normal,
                opacity: 1.0,
            },
            TextAnchor::End,
            TextBaseline::Middle,
            None,
        )?;
        if !fragment.label.trim().is_empty() {
            self.emit_bracketed_label(
                &format!("{id}.condition"),
                fragment.x + 1.0,
                fragment.header_y + 40.0,
                &fragment.label,
                fragment.label_width,
                "fragment condition",
            )?;
        }
        for (section_index, section) in fragment.sections.iter().enumerate().skip(1) {
            let separator_y = section.y + 0.5;
            let inset = section.content_inset_left.unwrap_or_default();
            self.add_path(
                format!("{id}.section.{section_index}.separator"),
                line_path(
                    Point::new(self.x(fragment.x + 1.0 + inset), self.y(separator_y)),
                    Point::new(
                        self.x(fragment.x + fragment.width - 1.0),
                        self.y(separator_y),
                    ),
                ),
                stroke_style(self.palette.fragment_separator, 1.0),
            )?;
            if let Some(inner) = section.inner_label.as_deref() {
                self.emit_bracketed_label(
                    &format!("{id}.section.{section_index}.label"),
                    fragment.x + 1.0,
                    section.y + 16.0,
                    inner,
                    section.inner_label_width,
                    "fragment section label",
                )?;
            } else if let (Some(keyword), Some(detail)) =
                (section.keyword.as_deref(), section.detail.as_deref())
            {
                let keyword_width = section.keyword_width.unwrap_or_default();
                let detail_width = section.detail_width.unwrap_or_default();
                let section_id = format!("{id}.section.{section_index}");
                self.add_path(
                    format!("{section_id}.background"),
                    rect_path(
                        self.x(fragment.x + 1.0),
                        self.y(section.y + 1.0),
                        keyword_width + detail_width + 16.0,
                        20.0,
                    ),
                    fill_style(Color::rgba(255, 255, 255, 255)),
                )?;
                let color = Color::rgba(0, 0, 0, 255);
                self.emit_text(
                    format!("{section_id}.keyword"),
                    &portable_text(keyword, "ZenUML fragment section keyword")?,
                    Point::new(self.x(fragment.x + 5.0), self.y(section.y + 16.0)),
                    TextSpec {
                        color,
                        size: FRAGMENT_LABEL_FONT_SIZE,
                        weight: 400,
                        style: FontStyle::Normal,
                        opacity: 0.65,
                    },
                    TextAnchor::Start,
                    TextBaseline::Middle,
                    None,
                )?;
                self.emit_text(
                    format!("{section_id}.detail"),
                    &portable_text(detail, "ZenUML fragment section detail")?,
                    Point::new(
                        self.x(fragment.x + 13.0 + keyword_width),
                        self.y(section.y + 16.0),
                    ),
                    TextSpec {
                        color,
                        size: FRAGMENT_LABEL_FONT_SIZE,
                        weight: 400,
                        style: FontStyle::Normal,
                        opacity: 0.65,
                    },
                    TextAnchor::Start,
                    TextBaseline::Middle,
                    None,
                )?;
            } else if !section.label.trim().is_empty() {
                let section_id = format!("{id}.section.{section_index}");
                self.add_path(
                    format!("{section_id}.background"),
                    rect_path(
                        self.x(fragment.x + 1.0),
                        self.y(section.y + 1.0),
                        section.label_width.unwrap_or_default() + 8.0,
                        20.0,
                    ),
                    fill_style(Color::rgba(255, 255, 255, 255)),
                )?;
                self.emit_text(
                    format!("{section_id}.label"),
                    &portable_text(&section.label, "ZenUML fragment section label")?,
                    Point::new(self.x(fragment.x + 5.0), self.y(section.y + 16.0)),
                    TextSpec {
                        color: Color::rgba(0, 0, 0, 255),
                        size: FRAGMENT_LABEL_FONT_SIZE,
                        weight: 400,
                        style: FontStyle::Normal,
                        opacity: 0.65,
                    },
                    TextAnchor::Start,
                    TextBaseline::Middle,
                    None,
                )?;
            }
        }
        Ok(())
    }

    fn emit_bracketed_label(
        &mut self,
        id: &str,
        x: f64,
        y: f64,
        inner: &str,
        inner_width: Option<f64>,
        _description: &str,
    ) -> Result<()> {
        let inner = resolve_zenuml_emojis_in_text(&portable_text(inner, "ZenUML bracket label")?);
        let inner_x = x + 7.89;
        let close_x = inner_x
            + inner_width.unwrap_or_else(|| {
                self.measure_width(&inner, FRAGMENT_LABEL_FONT_SIZE, 400, FontStyle::Normal)
            })
            + 4.0;
        let spec = TextSpec {
            color: Color::rgba(0, 0, 0, 255),
            size: FRAGMENT_LABEL_FONT_SIZE,
            weight: 400,
            style: FontStyle::Normal,
            opacity: 1.0,
        };
        self.emit_text(
            format!("{id}.open"),
            "[",
            Point::new(self.x(x), self.y(y)),
            spec,
            TextAnchor::Start,
            TextBaseline::Alphabetic,
            None,
        )?;
        self.emit_text(
            format!("{id}.inner"),
            &inner,
            Point::new(self.x(inner_x), self.y(y)),
            TextSpec {
                opacity: 0.65,
                ..spec
            },
            TextAnchor::Start,
            TextBaseline::Alphabetic,
            None,
        )?;
        self.emit_text(
            format!("{id}.close"),
            "]",
            Point::new(self.x(close_x), self.y(y)),
            spec,
            TextAnchor::Start,
            TextBaseline::Alphabetic,
            None,
        )?;
        Ok(())
    }

    fn emit_dividers(&mut self) -> Result<()> {
        for (index, divider) in self.layout.dividers.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let id = format!("zenuml.divider.{index}");
            self.begin_semantic(
                &id,
                SemanticRole::Group,
                Some(portable_text(&divider.label, "ZenUML divider label")?),
                "ZenUML divider",
            );
            self.svg_semantic_data_statements
                .insert(id.clone(), divider.statement_id.clone());
            let label = divider
                .label
                .trim()
                .trim_start_matches('=')
                .trim_end_matches('=')
                .trim();
            let label =
                resolve_zenuml_emojis_in_text(&portable_text(label, "ZenUML divider label")?);
            let center_x = divider.width / 2.0;
            let rect_width = divider.label_width + 17.0;
            let rect_height = 27.0;
            let rect_x = center_x - rect_width / 2.0;
            self.add_path(
                format!("{id}.line.left"),
                line_path(
                    Point::new(self.x(0.0), self.y(divider.y)),
                    Point::new(self.x(rect_x - 0.5), self.y(divider.y)),
                ),
                stroke_style(self.palette.divider_stroke, 1.0),
            )?;
            self.add_path(
                format!("{id}.line.right"),
                line_path(
                    Point::new(self.x(rect_x + rect_width + 0.5), self.y(divider.y)),
                    Point::new(self.x(divider.width), self.y(divider.y)),
                ),
                stroke_style(self.palette.divider_stroke, 1.0),
            )?;
            self.add_path(
                format!("{id}.background"),
                rounded_rect_path(
                    self.x(center_x),
                    self.y(divider.y),
                    rect_width,
                    rect_height,
                    2.0,
                ),
                PathStyle {
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::solid(self.palette.divider_fill)),
                    stroke: Some(stroke(self.palette.divider_stroke, 1.0)),
                },
            )?;
            self.emit_text(
                format!("{id}.label"),
                &label,
                Point::new(self.x(center_x), self.y(divider.y)),
                TextSpec {
                    color: self.palette.divider_text,
                    size: MESSAGE_LABEL_FONT_SIZE,
                    weight: 400,
                    style: FontStyle::Normal,
                    opacity: 1.0,
                },
                TextAnchor::Middle,
                TextBaseline::Middle,
                None,
            )?;
            self.end_semantic();
        }
        Ok(())
    }

    fn emit_comments(&mut self) -> Result<()> {
        for (index, comment) in self.layout.comments.iter().enumerate() {
            self.session.checkpoint(OperationPhase::Emit)?;
            let id = format!("zenuml.comment.{index}");
            self.begin_semantic(
                &id,
                SemanticRole::Label,
                Some(portable_text(&comment.text, "ZenUML comment")?),
                "ZenUML comment",
            );
            self.svg_semantic_data_statements
                .insert(id.clone(), comment.statement_id.clone());
            let style = comment_text_spec(&comment.style, self.palette.comment, self.session)?;
            let text =
                resolve_zenuml_emojis_in_text(&portable_text(&comment.text, "ZenUML comment")?);
            for (line_index, line) in split_html_br_lines(&text)
                .into_iter()
                .flat_map(|line| line.split('\n'))
                .enumerate()
            {
                self.emit_text(
                    format!("{id}.line.{line_index}"),
                    line,
                    Point::new(
                        self.x(comment.x),
                        self.y(comment.y + line_index as f64 * 20.0),
                    ),
                    style,
                    TextAnchor::Start,
                    TextBaseline::Alphabetic,
                    None,
                )?;
            }
            self.end_semantic();
        }
        Ok(())
    }

    fn begin_semantic(
        &mut self,
        id: &str,
        role: SemanticRole,
        title: Option<String>,
        description: &str,
    ) {
        if let Some(class) = zenuml_semantic_class(id) {
            self.svg_semantic_classes
                .insert(id.to_string(), class.to_string());
        }
        self.commands.push(DrawingCommand::BeginSemanticGroup {
            semantic_id: id.to_string(),
        });
        self.semantics.push(SemanticAnnotation {
            id: id.to_string(),
            role,
            title,
            description: Some(description.to_string()),
            link: None,
        });
        self.semantic_stack.push(id.to_string());
    }

    fn end_semantic(&mut self) {
        self.commands.push(DrawingCommand::EndSemanticGroup);
        debug_assert!(self.semantic_stack.pop().is_some());
    }

    fn x(&self, value: f64) -> f64 {
        self.content_left + value
    }

    fn y(&self, value: f64) -> f64 {
        self.header_y + value
    }

    fn require_participant(&self, name: &str) -> Result<&ZenumlParticipantLayout> {
        self.participant_by_name.get(name).copied().ok_or_else(|| {
            invalid(format!(
                "ZenUML layout references missing participant `{name}`"
            ))
        })
    }

    fn validate_endpoint(&self, name: &str) -> Result<()> {
        if name == DEFAULT_STARTER {
            return Ok(());
        }
        self.require_participant(name).map(|_| ())
    }

    fn measure_width(&self, text: &str, size: f64, weight: u16, style: FontStyle) -> f64 {
        let measurement_style = MeasurementTextStyle {
            font_family: Some(self.font.families.join(", ")),
            font_size: size,
            font_weight: Some(weight.to_string()),
            font_style: Some(
                match style {
                    FontStyle::Normal => "normal",
                    FontStyle::Italic => "italic",
                    FontStyle::Oblique => "oblique",
                }
                .to_string(),
            ),
        };
        self.session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit)
            .measure(text, &measurement_style)
            .width
            .max(1.0)
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_text(
        &mut self,
        id: impl Into<String>,
        text: &str,
        origin: Point,
        spec: TextSpec,
        anchor: TextAnchor,
        baseline: TextBaseline,
        letter_spacing: Option<f64>,
    ) -> Result<()> {
        let id = id.into();
        let semantic_id =
            self.semantic_stack.last().cloned().ok_or_else(|| {
                invalid(format!("ZenUML text `{id}` is outside a semantic group"))
            })?;
        let text_index = self
            .semantic_text_counts
            .entry(semantic_id.clone())
            .or_default();
        if let Some(class) = zenuml_text_class(&id) {
            self.svg_text_classes
                .insert(format!("{semantic_id}#{text_index}"), class.to_string());
        }
        *text_index = text_index.saturating_add(1);
        if !origin.x.is_finite() || !origin.y.is_finite() {
            return Err(invalid(format!("ZenUML text `{id}` has invalid origin")));
        }
        let measurement_style = MeasurementTextStyle {
            font_family: Some(self.font.families.join(", ")),
            font_size: spec.size,
            font_weight: Some(spec.weight.to_string()),
            font_style: Some(
                match spec.style {
                    FontStyle::Normal => "normal",
                    FontStyle::Italic => "italic",
                    FontStyle::Oblique => "oblique",
                }
                .to_string(),
            ),
        };
        let measurer = self
            .session
            .controlled_text_measurer(TextMeasurementPhase::SvgBBox, OperationPhase::Emit);
        let measured = measurer.measure(text, &measurement_style);
        let width = measured.width.max(1.0);
        let line_height = (spec.size * LINE_HEIGHT_FACTOR).max(1.0);
        let bounds_x = match anchor {
            TextAnchor::Start => origin.x,
            TextAnchor::Middle => origin.x - width / 2.0,
            TextAnchor::End => origin.x - width,
        };
        let bounds_y = match baseline {
            TextBaseline::Middle | TextBaseline::Central => origin.y - line_height / 2.0,
            TextBaseline::Hanging => origin.y,
            _ => origin.y - spec.size,
        };
        self.commands.push(DrawingCommand::draw_text(TextRun {
            text: text.to_string(),
            origin,
            bounds: Rect::new(bounds_x, bounds_y, width, line_height),
            style: TextStyle {
                font: FontDescriptor {
                    weight: spec.weight.max(1),
                    style: spec.style,
                    ..self.font.clone()
                },
                font_size: spec.size,
                letter_spacing: letter_spacing.unwrap_or(0.0),
                line_height,
                fill: Paint::solid(with_alpha(spec.color, spec.opacity)),
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
            id,
            role: SemanticRole::Label,
            title: Some(text.to_string()),
            description: Some("ZenUML text".to_string()),
            link: None,
        });
        Ok(())
    }

    fn emit_asset_icon(
        &mut self,
        id: &str,
        key: &str,
        x: f64,
        y: f64,
        size: f64,
        context: AssetContext,
    ) -> Result<()> {
        let Some(asset) = embedded_asset(key) else {
            return Err(unavailable(format!(
                "ZenUML icon `{key}` has no embedded vector asset"
            )));
        };
        if !size.is_finite() || size <= 0.0 {
            return Err(invalid(format!("ZenUML icon `{key}` has invalid size")));
        }
        let document = XmlDocument::parse(asset.svg).map_err(|error| {
            unavailable(format!(
                "ZenUML embedded icon `{key}` is not valid SVG: {error}"
            ))
        })?;
        if document.descendants().any(|node| {
            node.is_element()
                && matches!(
                    node.tag_name().name(),
                    "linearGradient"
                        | "radialGradient"
                        | "pattern"
                        | "filter"
                        | "mask"
                        | "image"
                        | "use"
                )
        }) {
            return Err(unavailable(format!(
                "ZenUML icon `{key}` uses an SVG effect that DrawingList v1 cannot project"
            )));
        }
        if document
            .descendants()
            .filter(|node| node.is_element())
            .any(|node| node.attribute("transform").is_some())
        {
            return Err(unavailable(format!(
                "ZenUML icon `{key}` uses an SVG transform that DrawingList v1 cannot project"
            )));
        }
        let view_box = parse_view_box(document.root_element().attribute("viewBox"), asset)?;
        let scale = size / view_box.2.max(view_box.3);
        let inherited = AssetStyle::root(context);
        let mut shapes = Vec::new();
        collect_asset_shapes(document.root_element(), inherited, &mut shapes)?;
        if shapes.is_empty() {
            return Err(unavailable(format!(
                "ZenUML icon `{key}` contains no portable vector geometry"
            )));
        }
        for (index, shape) in shapes.into_iter().enumerate() {
            let path_id = format!("{id}.{index}");
            let segments = shape
                .segments
                .into_iter()
                .map(|segment| transform_asset_segment(segment, x, y, scale, view_box))
                .collect::<Vec<_>>();
            if let Some(style) = shape.style.to_path_style(scale)? {
                self.add_path(path_id.clone(), segments, style)?;
                self.svg_path_data_icons.insert(path_id, key.to_string());
            }
        }
        Ok(())
    }

    fn add_path(
        &mut self,
        id: impl Into<String>,
        segments: Vec<PathSegment>,
        style: PathStyle,
    ) -> Result<()> {
        if segments.is_empty() {
            return Err(invalid("ZenUML path has no geometry"));
        }
        let id = ResourceId::new(id.into());
        if let Some(class) = zenuml_path_class(id.as_str()) {
            self.svg_path_classes
                .insert(id.as_str().to_string(), class.to_string());
        }
        self.resources.push(DrawingResource::Path(PathResource {
            id: id.clone(),
            segments,
        }));
        self.commands
            .push(DrawingCommand::DrawPath { path: id, style });
        Ok(())
    }
}

fn validate_model_and_layout(
    model: &ZenumlDiagramRenderModel,
    layout: &ZenumlDiagramLayout,
) -> Result<()> {
    validate_positive_finite(layout.width, "ZenUML layout width")?;
    validate_positive_finite(layout.height, "ZenUML layout height")?;
    validate_bounds(&layout.bounds)?;
    if !layout.frame_border_left.is_finite()
        || !layout.frame_border_right.is_finite()
        || layout.frame_border_left < 0.0
        || layout.frame_border_right < 0.0
    {
        return Err(invalid("ZenUML frame border geometry is invalid"));
    }

    let model_names = model
        .participants
        .iter()
        .map(|participant| participant.name.as_str())
        .collect::<HashSet<_>>();
    if model_names.len() != model.participants.len() {
        return Err(invalid("ZenUML model contains duplicate participant names"));
    }
    let layout_names = layout
        .participants
        .iter()
        .map(|participant| participant.name.as_str())
        .collect::<HashSet<_>>();
    if layout_names.len() != layout.participants.len() || layout_names != model_names {
        return Err(invalid(
            "ZenUML semantic and layout participant sets are out of sync",
        ));
    }
    for participant in &layout.participants {
        validate_box(
            participant.x - participant.width / 2.0,
            participant.y,
            participant.width,
            participant.height,
            &participant.name,
        )?;
        for value in [
            participant.label_width,
            participant.stereotype_width.unwrap_or(0.0),
        ] {
            if !value.is_finite() || value < 0.0 {
                return Err(invalid(format!(
                    "ZenUML participant `{}` text metrics are invalid",
                    participant.name
                )));
            }
        }
    }
    let lifeline_names = layout
        .lifelines
        .iter()
        .map(|lifeline| lifeline.participant_name.as_str())
        .collect::<HashSet<_>>();
    if lifeline_names.len() != layout.lifelines.len() || lifeline_names != layout_names {
        return Err(invalid(
            "ZenUML layout lifelines are out of sync with participants",
        ));
    }
    for lifeline in &layout.lifelines {
        if !lifeline.x.is_finite()
            || !lifeline.top_y.is_finite()
            || !lifeline.bottom_y.is_finite()
            || lifeline.bottom_y < lifeline.top_y
        {
            return Err(invalid(format!(
                "ZenUML lifeline `{}` geometry is invalid",
                lifeline.participant_name
            )));
        }
    }

    let named_groups = model
        .groups
        .iter()
        .filter_map(|group| group.id.as_deref())
        .collect::<HashSet<_>>();
    let layout_groups = layout
        .groups
        .iter()
        .map(|group| group.name.as_str())
        .collect::<HashSet<_>>();
    if layout_groups.len() != layout.groups.len()
        || layout_groups
            .iter()
            .any(|group| !named_groups.contains(group))
    {
        return Err(invalid(
            "ZenUML layout groups are out of sync with semantic groups",
        ));
    }
    for group in &layout.groups {
        validate_box(group.x, group.y, group.width, group.height, &group.name)?;
    }

    let mut statement_ids = HashSet::new();
    collect_statement_ids_into(&model.statements, &mut statement_ids);
    if statement_ids.len() != count_statements(&model.statements) {
        return Err(invalid("ZenUML model contains duplicate statement IDs"));
    }
    for message in &layout.messages {
        validate_message_layout(message, &layout_names)?;
    }
    for call in &layout.self_calls {
        validate_box(call.x, call.y, call.width, call.height, &call.statement_id)?;
        validate_finite_values(
            [call.x, call.y, call.width, call.height],
            "ZenUML self-call geometry",
        )?;
        if !layout_names.contains(call.participant_name.as_str()) {
            return Err(invalid(format!(
                "ZenUML self-call `{}` references missing participant",
                call.statement_id
            )));
        }
    }
    for creation in &layout.creations {
        validate_box(
            creation.participant.x - creation.participant.width / 2.0,
            creation.participant.y,
            creation.participant.width,
            creation.participant.height,
            &creation.participant.name,
        )?;
        validate_message_layout(&creation.message, &layout_names)?;
    }
    for returned in &layout.returns {
        validate_finite_values(
            [returned.from_x, returned.to_x, returned.y],
            "ZenUML return geometry",
        )?;
        if !is_virtual_endpoint(&returned.from, &layout_names)
            || !is_virtual_endpoint(&returned.to, &layout_names)
        {
            return Err(invalid(format!(
                "ZenUML return `{}` references missing participant",
                returned.statement_id
            )));
        }
    }
    for occurrence in &layout.occurrences {
        validate_box(
            occurrence.x,
            occurrence.y,
            occurrence.width,
            occurrence.height,
            &occurrence.statement_id,
        )?;
        if !layout_names.contains(occurrence.participant_name.as_str()) {
            return Err(invalid(format!(
                "ZenUML occurrence `{}` references missing participant",
                occurrence.statement_id
            )));
        }
    }
    for fragment in &layout.fragments {
        validate_box(
            fragment.x,
            fragment.y,
            fragment.width,
            fragment.height,
            &fragment.statement_id,
        )?;
        validate_finite_values([fragment.header_y], "ZenUML fragment header geometry")?;
        for section in &fragment.sections {
            validate_finite_values(
                [section.y, section.height],
                "ZenUML fragment section geometry",
            )?;
        }
    }
    for divider in &layout.dividers {
        validate_finite_values(
            [divider.y, divider.width, divider.label_width],
            "ZenUML divider geometry",
        )?;
        if divider.width <= 0.0 || divider.label_width < 0.0 {
            return Err(invalid(format!(
                "ZenUML divider `{}` geometry is invalid",
                divider.statement_id
            )));
        }
    }
    for comment in &layout.comments {
        validate_finite_values([comment.x, comment.y], "ZenUML comment geometry")?;
    }
    Ok(())
}

fn validate_message_layout(
    message: &ZenumlMessageLayout,
    participant_names: &HashSet<&str>,
) -> Result<()> {
    validate_finite_values(
        [message.from_x, message.to_x, message.y],
        "ZenUML message geometry",
    )?;
    if !is_virtual_endpoint(&message.from, participant_names)
        || !is_virtual_endpoint(&message.to, participant_names)
    {
        return Err(invalid(format!(
            "ZenUML message `{}` references missing participant",
            message.statement_id
        )));
    }
    Ok(())
}

fn is_virtual_endpoint(name: &str, participant_names: &HashSet<&str>) -> bool {
    name == "_STARTER_" || participant_names.contains(name)
}

fn validate_layout_statement_coverage(
    layout: &ZenumlDiagramLayout,
    statement_ids: &HashSet<&str>,
) -> Result<()> {
    let mut represented = HashSet::new();
    for id in layout
        .messages
        .iter()
        .map(|item| item.statement_id.as_str())
        .chain(
            layout
                .self_calls
                .iter()
                .map(|item| item.statement_id.as_str()),
        )
        .chain(
            layout
                .creations
                .iter()
                .map(|item| item.statement_id.as_str()),
        )
        .chain(
            layout
                .fragments
                .iter()
                .map(|item| item.statement_id.as_str()),
        )
        .chain(
            layout
                .dividers
                .iter()
                .map(|item| item.statement_id.as_str()),
        )
        .chain(
            layout
                .comments
                .iter()
                .map(|item| item.statement_id.as_str()),
        )
    {
        represented.insert(id);
        if !statement_ids.contains(id) {
            return Err(invalid(format!(
                "ZenUML layout item references unknown statement `{id}`"
            )));
        }
    }
    for returned in &layout.returns {
        let base = returned
            .statement_id
            .strip_suffix("-assignment-return")
            .unwrap_or(returned.statement_id.as_str());
        if !statement_ids.contains(base) {
            return Err(invalid(format!(
                "ZenUML return layout references unknown statement `{}`",
                returned.statement_id
            )));
        }
        represented.insert(base);
    }
    for statement_id in statement_ids {
        if !represented.contains(statement_id) {
            return Err(invalid(format!(
                "ZenUML statement `{statement_id}` has no renderer geometry"
            )));
        }
    }
    Ok(())
}

fn validate_portable_text(
    model: &ZenumlDiagramRenderModel,
    layout: &ZenumlDiagramLayout,
) -> Result<()> {
    if let Some(title) = model.title.as_deref() {
        portable_text(title, "ZenUML title")?;
    }
    for participant in &model.participants {
        if let Some(label) = participant.label.as_deref() {
            portable_text(label, "ZenUML participant label")?;
        }
        if let Some(stereotype) = participant.stereotype.as_deref() {
            portable_text(stereotype, "ZenUML stereotype")?;
        }
        if let Some(comment) = participant.comment.as_deref() {
            portable_text(comment, "ZenUML participant comment")?;
        }
    }
    for group in &model.groups {
        if let Some(id) = group.id.as_deref() {
            portable_text(id, "ZenUML group name")?;
        }
    }
    for comment in &layout.comments {
        portable_text(&comment.text, "ZenUML comment")?;
        validate_style_keys(&comment.style, "comment")?;
    }
    for message in &layout.messages {
        portable_text(&message.label, "ZenUML message label")?;
        validate_style_keys(&message.style, "message")?;
    }
    for call in &layout.self_calls {
        portable_text(&call.label, "ZenUML self-call label")?;
        validate_style_keys(&call.style, "self-call")?;
    }
    for creation in &layout.creations {
        portable_text(&creation.message.label, "ZenUML creation label")?;
        validate_style_keys(&creation.message.style, "creation")?;
    }
    for returned in &layout.returns {
        portable_text(&returned.label, "ZenUML return label")?;
    }
    for fragment in &layout.fragments {
        portable_text(&fragment.label, "ZenUML fragment label")?;
        for section in &fragment.sections {
            portable_text(&section.label, "ZenUML fragment section label")?;
            if let Some(inner) = section.inner_label.as_deref() {
                portable_text(inner, "ZenUML fragment condition")?;
            }
            if let Some(keyword) = section.keyword.as_deref() {
                portable_text(keyword, "ZenUML fragment keyword")?;
            }
            if let Some(detail) = section.detail.as_deref() {
                portable_text(detail, "ZenUML fragment detail")?;
            }
        }
    }
    for divider in &layout.dividers {
        portable_text(&divider.label, "ZenUML divider label")?;
    }
    Ok(())
}

fn portable_text(value: &str, context: &str) -> Result<String> {
    let normalized = split_html_br_lines(value).join("\n");
    let analysis = crate::text::analyze_mermaid_markdown(&normalized, true);
    if analysis.has_styled_runs
        || crate::text::mermaid_markdown_contains_raw_blocks(&normalized)
        || crate::text::mermaid_markdown_contains_html_tags(&normalized)
        || normalized.contains('`')
        || normalized.contains("](")
        || normalized.contains("![")
    {
        return Err(unavailable(format!(
            "ZenUML {context} uses Markdown or HTML styling that DrawingList v1 cannot preserve"
        )));
    }
    let fragment = crate::text::mermaid_markdown_to_xhtml_label_fragment(&normalized, true);
    let plain = crate::text::mermaid_xhtml_label_plain_text(&fragment).ok_or_else(|| {
        unavailable(format!(
            "ZenUML {context} uses HTML structure that DrawingList v1 cannot represent"
        ))
    })?;
    let lines = split_html_br_lines(&plain)
        .into_iter()
        .map(svg_plain_text)
        .collect::<Vec<_>>();
    Ok(lines.join("\n"))
}

fn validate_style_keys(style: &BTreeMap<String, String>, context: &str) -> Result<()> {
    for (key, value) in style {
        if !matches!(
            key.as_str(),
            "fill" | "color" | "font-size" | "font-weight" | "font-style" | "text-decoration"
        ) {
            return Err(unavailable(format!(
                "ZenUML {context} style property `{key}` is not portable"
            )));
        }
        if value.trim().is_empty() {
            return Err(unavailable(format!(
                "ZenUML {context} style property `{key}` is empty"
            )));
        }
    }
    Ok(())
}

fn collect_statement_ids(model: &ZenumlDiagramRenderModel) -> HashSet<&str> {
    let mut ids = HashSet::new();
    collect_statement_ids_into(&model.statements, &mut ids);
    ids
}

fn collect_statement_ids_into<'a>(statements: &'a [ZenumlStatement], ids: &mut HashSet<&'a str>) {
    for statement in statements {
        ids.insert(statement.id.as_str());
        match &statement.kind {
            ZenumlStatementKind::Message { body, .. }
            | ZenumlStatementKind::Creation { body, .. } => {
                collect_statement_ids_into(body, ids);
            }
            ZenumlStatementKind::Fragment { sections, .. } => {
                for section in sections {
                    collect_statement_ids_into(&section.statements, ids);
                }
            }
            ZenumlStatementKind::Return { .. }
            | ZenumlStatementKind::Reference { .. }
            | ZenumlStatementKind::Divider { .. } => {}
        }
    }
}

fn count_statements(statements: &[ZenumlStatement]) -> usize {
    statements
        .iter()
        .map(|statement| {
            1 + match &statement.kind {
                ZenumlStatementKind::Message { body, .. }
                | ZenumlStatementKind::Creation { body, .. } => count_statements(body),
                ZenumlStatementKind::Fragment { sections, .. } => sections
                    .iter()
                    .map(|section| count_statements(&section.statements))
                    .sum(),
                ZenumlStatementKind::Return { .. }
                | ZenumlStatementKind::Reference { .. }
                | ZenumlStatementKind::Divider { .. } => 0,
            }
        })
        .sum()
}

fn validate_finite_values<const N: usize>(values: [f64; N], context: &str) -> Result<()> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(invalid(format!("{context} contains non-finite values")))
    }
}

fn validate_positive_finite(value: f64, context: &str) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(invalid(format!("{context} is invalid")))
    }
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    if [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .iter()
        .all(|value| value.is_finite())
        && bounds.max_x >= bounds.min_x
        && bounds.max_y >= bounds.min_y
    {
        Ok(())
    } else {
        Err(invalid("ZenUML bounds are invalid"))
    }
}

fn validate_box(x: f64, y: f64, width: f64, height: f64, id: &str) -> Result<()> {
    if [x, y, width, height].iter().all(|value| value.is_finite()) && width > 0.0 && height > 0.0 {
        Ok(())
    } else {
        Err(invalid(format!("ZenUML geometry for `{id}` is invalid")))
    }
}

fn rect_path(x: f64, y: f64, width: f64, height: f64) -> Vec<PathSegment> {
    polygon_path(&[
        Point::new(x, y),
        Point::new(x + width, y),
        Point::new(x + width, y + height),
        Point::new(x, y + height),
    ])
}

fn line_path(start: Point, end: Point) -> Vec<PathSegment> {
    vec![
        PathSegment::MoveTo { to: start },
        PathSegment::LineTo { to: end },
    ]
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

fn dashed_stroke(color: Color, width: f64, dash_array: Vec<f64>) -> StrokeStyle {
    let mut value = stroke(color, width);
    value.dash_array = dash_array;
    value
}

fn arrow_style(style: ZenumlArrowStyle) -> PathStyle {
    let color = Color::rgba(0, 0, 0, 255);
    PathStyle {
        fill_rule: FillRule::NonZero,
        fill: (style == ZenumlArrowStyle::Solid).then(|| Paint::solid(color)),
        stroke: Some(stroke_style_value(color, 2.0)),
    }
}

fn stroke_style_value(color: Color, width: f64) -> StrokeStyle {
    stroke(color, width)
}

fn arrow_head_path(
    tip_x: f64,
    tip_y: f64,
    points_left: bool,
    style: ZenumlArrowStyle,
) -> Vec<PathSegment> {
    let direction = if points_left { 1.0 } else { -1.0 };
    let base_x = tip_x + direction * 6.0;
    let shoulder_x = tip_x + direction * 0.85;
    let points = [
        Point::new(base_x, tip_y - 3.25),
        Point::new(shoulder_x, tip_y),
        Point::new(base_x, tip_y + 3.25),
    ];
    if style == ZenumlArrowStyle::Solid {
        polygon_path(&points)
    } else {
        vec![
            PathSegment::MoveTo { to: points[0] },
            PathSegment::LineTo { to: points[1] },
            PathSegment::LineTo { to: points[2] },
        ]
    }
}

fn left_arrow_path(tip: Point, style: ZenumlArrowStyle) -> Vec<PathSegment> {
    arrow_head_path(tip.x, tip.y, true, style)
}

fn open_arrow_path(tip: Point, points_left: bool) -> Vec<PathSegment> {
    let direction = if points_left { 1.0 } else { -1.0 };
    let base_x = tip.x + direction * 5.15;
    vec![
        PathSegment::MoveTo {
            to: Point::new(base_x, tip.y - 3.25),
        },
        PathSegment::LineTo { to: tip },
        PathSegment::LineTo {
            to: Point::new(base_x, tip.y + 3.25),
        },
    ]
}

fn fragment_kind_label(kind: ZenumlLayoutFragmentKind) -> &'static str {
    match kind {
        ZenumlLayoutFragmentKind::Loop => "Loop",
        ZenumlLayoutFragmentKind::Alternative => "Alt",
        ZenumlLayoutFragmentKind::Parallel => "Par",
        ZenumlLayoutFragmentKind::Optional => "Opt",
        ZenumlLayoutFragmentKind::Critical => "Critical",
        ZenumlLayoutFragmentKind::Section => "Section",
        ZenumlLayoutFragmentKind::TryCatchFinally => "Try",
        ZenumlLayoutFragmentKind::Reference => "Ref",
    }
}

fn fragment_icon_key(kind: ZenumlLayoutFragmentKind) -> &'static str {
    match kind {
        ZenumlLayoutFragmentKind::Loop => "fragment-loop",
        ZenumlLayoutFragmentKind::Alternative => "fragment-alt",
        ZenumlLayoutFragmentKind::Parallel => "fragment-par",
        ZenumlLayoutFragmentKind::Optional => "fragment-opt",
        ZenumlLayoutFragmentKind::Critical => "fragment-critical",
        ZenumlLayoutFragmentKind::Section => "fragment-section",
        ZenumlLayoutFragmentKind::TryCatchFinally => "fragment-tcf",
        ZenumlLayoutFragmentKind::Reference => "fragment-ref",
    }
}

fn zenuml_semantic_class(id: &str) -> Option<&'static str> {
    if id.starts_with("zenuml.group.") {
        Some("participant-group")
    } else if id.starts_with("zenuml.lifeline.") {
        Some("lifeline")
    } else if id.starts_with("zenuml.participant.") {
        Some("participant")
    } else if id.starts_with("zenuml.occurrence.") {
        Some("occurrence")
    } else if id.starts_with("zenuml.message.") {
        Some("message")
    } else if id.starts_with("zenuml.self-call.") {
        Some("message self-call")
    } else if id.starts_with("zenuml.creation.") {
        Some("creation")
    } else if id.starts_with("zenuml.return.") {
        Some("return")
    } else if id.starts_with("zenuml.fragment.") {
        Some("fragment")
    } else if id.starts_with("zenuml.divider.") {
        Some("divider")
    } else if id.starts_with("zenuml.comment.") {
        Some("comment")
    } else {
        None
    }
}

fn fragment_kind_css_suffix(kind: ZenumlLayoutFragmentKind) -> &'static str {
    match kind {
        ZenumlLayoutFragmentKind::Loop => "loop",
        ZenumlLayoutFragmentKind::Alternative => "alt",
        ZenumlLayoutFragmentKind::Parallel => "par",
        ZenumlLayoutFragmentKind::Optional => "opt",
        ZenumlLayoutFragmentKind::Critical => "critical",
        ZenumlLayoutFragmentKind::Section => "section",
        ZenumlLayoutFragmentKind::TryCatchFinally => "tcf",
        ZenumlLayoutFragmentKind::Reference => "ref",
    }
}

fn zenuml_path_class(id: &str) -> Option<&'static str> {
    if id == "zenuml.frame.outer" {
        Some("frame-border-outer")
    } else if id == "zenuml.frame.inner" {
        Some("frame-border-inner")
    } else if id == "zenuml.frame.header" {
        Some("frame-header-line")
    } else if id.contains(".group.") && id.ends_with(".outline") {
        Some("group-outline")
    } else if id.contains(".group.") && id.ends_with(".title.background") {
        Some("group-title-bg")
    } else if id.contains(".lifeline.") && id.ends_with(".line") {
        Some("lifeline")
    } else if id.contains(".participant.") && id.ends_with(".box") {
        Some("participant-box")
    } else if id.contains(".participant.") && id.contains(".icon.") {
        Some("participant-icon")
    } else if id.contains(".occurrence.") && id.ends_with(".bar") {
        Some("occurrence")
    } else if id.contains(".message.") && id.ends_with(".line") {
        Some("message-line")
    } else if id.contains(".message.") && id.ends_with(".arrow") {
        Some("arrow-head")
    } else if id.contains(".self-call.") && id.ends_with(".route") {
        Some("message-line")
    } else if id.contains(".self-call.") && id.ends_with(".arrow") {
        Some("arrow-head")
    } else if id.contains(".creation.") && id.ends_with(".line") {
        Some("message-line")
    } else if id.contains(".creation.") && id.ends_with(".arrow") {
        Some("arrow-open")
    } else if id.contains(".return.") && id.ends_with(".line") {
        Some("return-line")
    } else if id.contains(".return.") && id.ends_with(".arrow") {
        Some("return-arrow")
    } else if id.contains(".return.") && id.contains(".icon.") {
        Some("return-icon")
    } else if id.contains(".fragment.") && id.ends_with(".border") {
        Some("fragment-border")
    } else if id.contains(".fragment.") && id.ends_with(".header") {
        Some("fragment-header")
    } else if id.contains(".fragment.") && id.contains(".icon.") {
        Some("fragment-icon")
    } else if id.contains(".fragment.") && id.ends_with(".separator") {
        Some("fragment-separator")
    } else if id.contains(".fragment.") && id.ends_with(".background") {
        Some("fragment-section-background")
    } else if id.contains(".divider.") && id.ends_with(".background") {
        Some("divider-bg")
    } else if id.contains(".divider.") && id.contains(".line.") {
        Some("divider-line")
    } else {
        None
    }
}

fn zenuml_text_class(id: &str) -> Option<&'static str> {
    if id == "zenuml.frame.title" {
        Some("frame-title")
    } else if id.contains(".group.") && id.ends_with(".title") {
        Some("group-title-text")
    } else if id.ends_with(".emoji") {
        Some("participant-emoji")
    } else if id.ends_with(".stereotype") {
        Some("stereotype-label")
    } else if id.contains(".participant.") && id.ends_with(".label") {
        Some("participant-label")
    } else if (id.contains(".message.") || id.contains(".self-call.") || id.contains(".creation."))
        && id.ends_with(".label")
    {
        Some("message-label")
    } else if (id.contains(".message.")
        || id.contains(".self-call.")
        || id.contains(".creation.")
        || id.contains(".return.")
        || id.contains(".fragment."))
        && id.ends_with(".number")
    {
        Some("seq-number")
    } else if id.contains(".return.") && id.ends_with(".label") {
        Some("return-label")
    } else if id.contains(".fragment.") && id.ends_with(".kind") {
        Some("fragment-label")
    } else if id.contains(".fragment.") && id.ends_with(".condition.open")
        || id.contains(".fragment.") && id.ends_with(".condition.inner")
        || id.contains(".fragment.") && id.ends_with(".condition.close")
    {
        Some("fragment-condition")
    } else if id.contains(".fragment.") && id.contains(".section.") {
        Some("fragment-section-label")
    } else if id.contains(".divider.") && id.ends_with(".label") {
        Some("divider-label")
    } else if id.contains(".comment.") && id.contains(".line.") {
        Some("comment-text")
    } else {
        None
    }
}

fn with_alpha(color: Color, opacity: f64) -> Color {
    let opacity = opacity.clamp(0.0, 1.0);
    Color::rgba(
        color.red,
        color.green,
        color.blue,
        (f64::from(color.alpha) * opacity).round().clamp(0.0, 255.0) as u8,
    )
}

fn message_text_spec(
    style: &BTreeMap<String, String>,
    default_color: Color,
    session: &RenderSession,
) -> Result<TextSpec> {
    text_spec_from_style(
        style,
        TextSpec {
            color: default_color,
            size: MESSAGE_LABEL_FONT_SIZE,
            weight: 400,
            style: FontStyle::Normal,
            opacity: 1.0,
        },
        session,
        "ZenUML message",
    )
}

fn comment_text_spec(
    style: &BTreeMap<String, String>,
    default_color: Color,
    session: &RenderSession,
) -> Result<TextSpec> {
    text_spec_from_style(
        style,
        TextSpec {
            color: default_color,
            size: COMMENT_FONT_SIZE,
            weight: 400,
            style: FontStyle::Normal,
            opacity: 0.5,
        },
        session,
        "ZenUML comment",
    )
}

fn text_spec_from_style(
    style: &BTreeMap<String, String>,
    mut spec: TextSpec,
    _session: &RenderSession,
    context: &str,
) -> Result<TextSpec> {
    let resolver = PortableStyleResolver::new("zenuml");
    for (key, value) in style {
        match key.as_str() {
            "fill" | "color" => {
                spec.color = resolver.color(key, value)?;
            }
            "font-size" => {
                spec.size = resolver.positive_length(key, value)?;
            }
            "font-weight" => {
                spec.weight = parse_font_weight(value)?;
            }
            "font-style" => {
                spec.style = parse_font_style(value)?;
            }
            "text-decoration" => {
                if !value.trim().eq_ignore_ascii_case("none") {
                    return Err(unavailable(format!(
                        "{context} text decoration `{value}` has no DrawingList v1 mapping"
                    )));
                }
            }
            _ => {
                return Err(unavailable(format!(
                    "{context} style property `{key}` has no DrawingList v1 mapping"
                )));
            }
        }
    }
    Ok(spec)
}

fn parse_font_weight(value: &str) -> Result<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(400),
        "bold" | "bolder" => Ok(700),
        "lighter" => Ok(300),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight))
            .ok_or_else(|| unavailable(format!("ZenUML font-weight `{value}` is unsupported"))),
    }
}

fn parse_font_style(value: &str) -> Result<FontStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Ok(FontStyle::Normal),
        "italic" => Ok(FontStyle::Italic),
        "oblique" => Ok(FontStyle::Oblique),
        value => Err(unavailable(format!(
            "ZenUML font-style `{value}` is unsupported"
        ))),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssetContext {
    Participant,
    Fragment,
}

#[derive(Debug, Clone, Copy)]
struct EmbeddedAsset {
    svg: &'static str,
    width: f64,
    height: f64,
}

fn embedded_asset(key: &str) -> Option<EmbeddedAsset> {
    let asset = match key {
        "actor" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/actor.svg"),
            width: 24.0,
            height: 24.0,
        },
        "database" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/database.svg"),
            width: 24.0,
            height: 24.0,
        },
        "boundary" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/boundary.svg"),
            width: 101.0,
            height: 78.0,
        },
        "control" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/control.svg"),
            width: 77.0,
            height: 86.0,
        },
        "entity" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/entity.svg"),
            width: 77.0,
            height: 80.0,
        },
        "ec2" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/ec2.svg"),
            width: 48.0,
            height: 48.0,
        },
        "lambda" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/lambda.svg"),
            width: 48.0,
            height: 48.0,
        },
        "sqs" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/sqs.svg"),
            width: 48.0,
            height: 48.0,
        },
        "sns" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/sns.svg"),
            width: 48.0,
            height: 48.0,
        },
        "iam" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/iam.svg"),
            width: 48.0,
            height: 48.0,
        },
        "azurefunction" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/azurefunction.svg"),
            width: 18.0,
            height: 18.0,
        },
        "fragment-alt" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/fragment-alt.svg"),
            width: 24.0,
            height: 24.0,
        },
        "fragment-opt" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/fragment-opt.svg"),
            width: 24.0,
            height: 24.0,
        },
        "fragment-par" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/fragment-par.svg"),
            width: 24.0,
            height: 24.0,
        },
        "fragment-critical" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/fragment-critical.svg"),
            width: 24.0,
            height: 24.0,
        },
        "fragment-loop" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/fragment-loop.svg"),
            width: 1024.0,
            height: 1024.0,
        },
        "fragment-tcf" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/fragment-tcf.svg"),
            width: 76.0,
            height: 76.0,
        },
        "fragment-section" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/fragment-section.svg"),
            width: 15.0,
            height: 15.0,
        },
        "fragment-ref" => EmbeddedAsset {
            svg: include_str!("../../assets/zenuml/fragment-ref.svg"),
            width: 24.0,
            height: 24.0,
        },
        _ => return None,
    };
    Some(asset)
}

#[derive(Debug, Clone, Copy)]
enum AssetPaint {
    None,
    CurrentColor,
    Color(Color),
}

impl AssetPaint {
    fn resolve(self, current_color: Color, opacity: f64) -> Option<Color> {
        match self {
            Self::None => None,
            Self::CurrentColor => Some(with_alpha(current_color, opacity)),
            Self::Color(color) => Some(with_alpha(color, opacity)),
        }
    }
}

#[derive(Debug, Clone)]
struct AssetStyle {
    context: AssetContext,
    current_color: Color,
    fill: AssetPaint,
    stroke: AssetPaint,
    stroke_width: f64,
    dash_array: Vec<f64>,
    dash_offset: f64,
    line_cap: LineCap,
    line_join: LineJoin,
    miter_limit: f64,
    fill_rule: FillRule,
    opacity: f64,
    fill_opacity: f64,
    stroke_opacity: f64,
    visible: bool,
}

impl AssetStyle {
    fn root(context: AssetContext) -> Self {
        Self {
            context,
            current_color: match context {
                AssetContext::Participant => Color::rgba(34, 34, 34, 255),
                AssetContext::Fragment => Color::rgba(0, 0, 0, 255),
            },
            fill: AssetPaint::CurrentColor,
            stroke: AssetPaint::None,
            stroke_width: 1.0,
            dash_array: Vec::new(),
            dash_offset: 0.0,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 4.0,
            fill_rule: FillRule::NonZero,
            opacity: 1.0,
            fill_opacity: 1.0,
            stroke_opacity: 1.0,
            visible: true,
        }
    }

    fn apply_node(&mut self, node: Node<'_, '_>) -> Result<()> {
        for property in [
            "color",
            "fill",
            "stroke",
            "stroke-width",
            "stroke-dasharray",
            "stroke-dashoffset",
            "stroke-linecap",
            "stroke-linejoin",
            "stroke-miterlimit",
            "fill-rule",
            "opacity",
            "fill-opacity",
            "stroke-opacity",
            "display",
            "visibility",
        ] {
            if let Some(value) = node.attribute(property) {
                self.apply_property(property, value)?;
            }
        }
        if let Some(style) = node.attribute("style") {
            for declaration in style
                .split(';')
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                let (property, value) = declaration.split_once(':').ok_or_else(|| {
                    unavailable(format!(
                        "ZenUML embedded SVG style declaration `{declaration}` is malformed"
                    ))
                })?;
                self.apply_property(property.trim(), value.trim())?;
            }
        }
        if self.context == AssetContext::Participant
            && node
                .attribute("fill")
                .is_some_and(|fill| fill.eq_ignore_ascii_case("currentColor"))
            && node.attribute("stroke").is_none()
        {
            self.stroke = AssetPaint::Color(Color::rgba(102, 102, 102, 255));
            self.stroke_width = 1.0;
        }
        Ok(())
    }

    fn apply_property(&mut self, property: &str, value: &str) -> Result<()> {
        let resolver = PortableStyleResolver::new("zenuml");
        match property.trim().to_ascii_lowercase().as_str() {
            "color" => self.current_color = resolver.color("embedded-svg.color", value)?,
            "fill" => self.fill = parse_asset_paint(value, "fill")?,
            "stroke" => self.stroke = parse_asset_paint(value, "stroke")?,
            "stroke-width" => {
                self.stroke_width = resolver.length("embedded-svg.stroke-width", value)?;
            }
            "stroke-dasharray" => self.dash_array = parse_asset_dash_array(value)?,
            "stroke-dashoffset" => {
                self.dash_offset = parse_asset_signed_length(value, "stroke-dashoffset")?;
            }
            "stroke-linecap" => self.line_cap = parse_asset_line_cap(value)?,
            "stroke-linejoin" => self.line_join = parse_asset_line_join(value)?,
            "stroke-miterlimit" => {
                self.miter_limit = parse_asset_positive_number(value, "stroke-miterlimit")?;
            }
            "fill-rule" => self.fill_rule = parse_asset_fill_rule(value)?,
            "opacity" => {
                self.opacity *= resolver.opacity("embedded-svg.opacity", value)?;
            }
            "fill-opacity" => {
                self.fill_opacity = resolver.opacity("embedded-svg.fill-opacity", value)?;
            }
            "stroke-opacity" => {
                self.stroke_opacity = resolver.opacity("embedded-svg.stroke-opacity", value)?;
            }
            "display" => match value.trim().to_ascii_lowercase().as_str() {
                "none" => self.visible = false,
                "inline" | "block" => self.visible = true,
                value => {
                    return Err(unavailable(format!(
                        "ZenUML embedded SVG display value `{value}` is unsupported"
                    )));
                }
            },
            "visibility" => match value.trim().to_ascii_lowercase().as_str() {
                "hidden" | "collapse" => self.visible = false,
                "visible" => self.visible = true,
                value => {
                    return Err(unavailable(format!(
                        "ZenUML embedded SVG visibility value `{value}` is unsupported"
                    )));
                }
            },
            property => {
                return Err(unavailable(format!(
                    "ZenUML embedded SVG style property `{property}` is unsupported"
                )));
            }
        }
        Ok(())
    }

    fn to_path_style(&self, scale: f64) -> Result<Option<PathStyle>> {
        if !self.visible {
            return Ok(None);
        }
        let fill = self
            .fill
            .resolve(self.current_color, self.opacity * self.fill_opacity)
            .map(Paint::solid);
        let stroke_color = self
            .stroke
            .resolve(self.current_color, self.opacity * self.stroke_opacity);
        let stroke = stroke_color.map(|color| StrokeStyle {
            paint: Paint::solid(color),
            width: self.stroke_width * scale,
            dash_array: self.dash_array.iter().map(|value| value * scale).collect(),
            dash_offset: self.dash_offset * scale,
            line_cap: self.line_cap,
            line_join: self.line_join,
            miter_limit: self.miter_limit,
        });
        if fill.is_none() && stroke.is_none() {
            return Ok(None);
        }
        Ok(Some(PathStyle {
            fill_rule: self.fill_rule,
            fill,
            stroke,
        }))
    }
}

#[derive(Debug)]
struct AssetShape {
    segments: Vec<PathSegment>,
    style: AssetStyle,
}

fn parse_asset_paint(value: &str, property: &str) -> Result<AssetPaint> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return Ok(AssetPaint::None);
    }
    if value.eq_ignore_ascii_case("currentColor") {
        return Ok(AssetPaint::CurrentColor);
    }
    if value.starts_with("url(") {
        return Err(unavailable(format!(
            "ZenUML embedded SVG {property} `{value}` requires a paint server"
        )));
    }
    let color =
        PortableStyleResolver::new("zenuml").color(&format!("embedded-svg.{property}"), value)?;
    Ok(AssetPaint::Color(color))
}

fn parse_asset_dash_array(value: &str) -> Result<Vec<f64>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let values = value
        .split(|character: char| character == ',' || character.is_ascii_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| parse_asset_nonnegative_number(part, "stroke-dasharray"))
        .collect::<Result<Vec<_>>>()?;
    if values.is_empty() {
        return Err(unavailable("ZenUML embedded SVG stroke-dasharray is empty"));
    }
    Ok(values)
}

fn parse_asset_signed_length(value: &str, property: &str) -> Result<f64> {
    let value = value.trim();
    let numeric = value
        .strip_suffix("px")
        .or_else(|| value.strip_suffix("pt"))
        .unwrap_or(value)
        .trim();
    let scale = if value.ends_with("pt") {
        4.0 / 3.0
    } else {
        1.0
    };
    let parsed = numeric.parse::<f64>().map_err(|_| {
        unavailable(format!(
            "ZenUML embedded SVG {property} value `{value}` is unsupported"
        ))
    })? * scale;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(unavailable(format!(
            "ZenUML embedded SVG {property} value `{value}` is non-finite"
        )))
    }
}

fn parse_asset_nonnegative_number(value: &str, property: &str) -> Result<f64> {
    let parsed = parse_asset_signed_length(value, property)?;
    if parsed >= 0.0 {
        Ok(parsed)
    } else {
        Err(unavailable(format!(
            "ZenUML embedded SVG {property} value `{value}` is negative"
        )))
    }
}

fn parse_asset_positive_number(value: &str, property: &str) -> Result<f64> {
    let parsed = parse_asset_signed_length(value, property)?;
    if parsed > 0.0 {
        Ok(parsed)
    } else {
        Err(unavailable(format!(
            "ZenUML embedded SVG {property} value `{value}` must be positive"
        )))
    }
}

fn parse_asset_line_cap(value: &str) -> Result<LineCap> {
    match value.trim().to_ascii_lowercase().as_str() {
        "butt" => Ok(LineCap::Butt),
        "round" => Ok(LineCap::Round),
        "square" => Ok(LineCap::Square),
        value => Err(unavailable(format!(
            "ZenUML embedded SVG stroke-linecap `{value}` is unsupported"
        ))),
    }
}

fn parse_asset_line_join(value: &str) -> Result<LineJoin> {
    match value.trim().to_ascii_lowercase().as_str() {
        "miter" => Ok(LineJoin::Miter),
        "round" => Ok(LineJoin::Round),
        "bevel" => Ok(LineJoin::Bevel),
        value => Err(unavailable(format!(
            "ZenUML embedded SVG stroke-linejoin `{value}` is unsupported"
        ))),
    }
}

fn parse_asset_fill_rule(value: &str) -> Result<FillRule> {
    match value.trim().to_ascii_lowercase().as_str() {
        "nonzero" | "non-zero" => Ok(FillRule::NonZero),
        "evenodd" | "even-odd" => Ok(FillRule::EvenOdd),
        value => Err(unavailable(format!(
            "ZenUML embedded SVG fill-rule `{value}` is unsupported"
        ))),
    }
}

fn parse_view_box(value: Option<&str>, asset: EmbeddedAsset) -> Result<(f64, f64, f64, f64)> {
    let values = value
        .map(|value| {
            value
                .split(|character: char| character == ',' || character.is_ascii_whitespace())
                .filter(|part| !part.is_empty())
                .map(|part| {
                    part.parse::<f64>().map_err(|_| {
                        unavailable(format!(
                            "ZenUML embedded SVG viewBox value `{part}` is invalid"
                        ))
                    })
                })
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;
    let values = values.unwrap_or_else(|| vec![0.0, 0.0, asset.width, asset.height]);
    if values.len() != 4 {
        return Err(unavailable(
            "ZenUML embedded SVG viewBox must contain four numbers",
        ));
    }
    let [min_x, min_y, width, height] = [values[0], values[1], values[2], values[3]];
    if [min_x, min_y, width, height]
        .iter()
        .all(|value| value.is_finite())
        && width > 0.0
        && height > 0.0
    {
        Ok((min_x, min_y, width, height))
    } else {
        Err(unavailable(
            "ZenUML embedded SVG viewBox has invalid dimensions",
        ))
    }
}

fn collect_asset_shapes(
    node: Node<'_, '_>,
    inherited: AssetStyle,
    output: &mut Vec<AssetShape>,
) -> Result<()> {
    if node.is_text() || node.is_comment() {
        return Ok(());
    }
    if !node.is_element() {
        return Ok(());
    }
    let name = node.tag_name().name();
    if matches!(name, "title" | "desc" | "metadata") {
        return Ok(());
    }
    if node.attribute("transform").is_some()
        || node.attribute("filter").is_some()
        || node.attribute("mask").is_some()
        || node.attribute("clip-path").is_some()
        || node.attribute("marker-start").is_some()
        || node.attribute("marker-mid").is_some()
        || node.attribute("marker-end").is_some()
    {
        return Err(unavailable(format!(
            "ZenUML embedded SVG element `{name}` uses unsupported vector effects"
        )));
    }
    let mut style = inherited;
    style.apply_node(node)?;
    match name {
        "svg" | "g" => {
            for child in node.children() {
                collect_asset_shapes(child, style.clone(), output)?;
            }
        }
        "path" => {
            let data = node.attribute("d").ok_or_else(|| {
                unavailable("ZenUML embedded SVG path is missing its d attribute")
            })?;
            let segments = parse_svg_path(data)?;
            push_asset_shape(style, segments, output)?;
        }
        "line" => {
            let x1 = asset_number(node, "x1", 0.0)?;
            let y1 = asset_number(node, "y1", 0.0)?;
            let x2 = asset_number(node, "x2", 0.0)?;
            let y2 = asset_number(node, "y2", 0.0)?;
            push_asset_shape(
                style,
                line_path(Point::new(x1, y1), Point::new(x2, y2)),
                output,
            )?;
        }
        "rect" => {
            let x = asset_number(node, "x", 0.0)?;
            let y = asset_number(node, "y", 0.0)?;
            let width = asset_required_positive_number(node, "width")?;
            let height = asset_required_positive_number(node, "height")?;
            let rx = asset_optional_number(node, "rx")?;
            let ry = asset_optional_number(node, "ry")?;
            let segments = asset_rect_path(x, y, width, height, rx, ry)?;
            push_asset_shape(style, segments, output)?;
        }
        "circle" => {
            let cx = asset_number(node, "cx", 0.0)?;
            let cy = asset_number(node, "cy", 0.0)?;
            let radius = asset_required_positive_number(node, "r")?;
            push_asset_shape(style, ellipse_path(cx, cy, radius, radius), output)?;
        }
        "ellipse" => {
            let cx = asset_number(node, "cx", 0.0)?;
            let cy = asset_number(node, "cy", 0.0)?;
            let radius_x = asset_required_positive_number(node, "rx")?;
            let radius_y = asset_required_positive_number(node, "ry")?;
            push_asset_shape(style, ellipse_path(cx, cy, radius_x, radius_y), output)?;
        }
        "polygon" | "polyline" => {
            let points = parse_asset_points(node.attribute("points").ok_or_else(|| {
                unavailable(format!(
                    "ZenUML embedded SVG {name} is missing its points attribute"
                ))
            })?)?;
            if points.len() < 2 {
                return Err(unavailable(format!(
                    "ZenUML embedded SVG {name} has fewer than two points"
                )));
            }
            let mut segments = polygon_path(&points);
            if name == "polyline" {
                segments.pop();
            }
            push_asset_shape(style, segments, output)?;
        }
        other => {
            return Err(unavailable(format!(
                "ZenUML embedded SVG element `{other}` is not in the portable icon subset"
            )));
        }
    }
    Ok(())
}

fn push_asset_shape(
    style: AssetStyle,
    segments: Vec<PathSegment>,
    output: &mut Vec<AssetShape>,
) -> Result<()> {
    if segments.is_empty() {
        return Err(unavailable(
            "ZenUML embedded SVG shape contains no geometry",
        ));
    }
    if style.to_path_style(1.0)?.is_some() {
        output.push(AssetShape { segments, style });
    }
    Ok(())
}

fn asset_number(node: Node<'_, '_>, attribute: &str, default: f64) -> Result<f64> {
    match node.attribute(attribute) {
        Some(value) => parse_asset_signed_length(value, attribute),
        None => Ok(default),
    }
}

fn asset_optional_number(node: Node<'_, '_>, attribute: &str) -> Result<Option<f64>> {
    node.attribute(attribute)
        .map(|value| parse_asset_nonnegative_number(value, attribute))
        .transpose()
}

fn asset_required_positive_number(node: Node<'_, '_>, attribute: &str) -> Result<f64> {
    let value = node.attribute(attribute).ok_or_else(|| {
        unavailable(format!(
            "ZenUML embedded SVG shape is missing `{attribute}`"
        ))
    })?;
    parse_asset_positive_number(value, attribute)
}

fn asset_rect_path(
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    rx: Option<f64>,
    ry: Option<f64>,
) -> Result<Vec<PathSegment>> {
    let Some(mut radius_x) = rx.or(ry) else {
        return Ok(rect_path(x, y, width, height));
    };
    let mut radius_y = ry.or(rx).unwrap_or(radius_x);
    radius_x = radius_x.min(width / 2.0);
    radius_y = radius_y.min(height / 2.0);
    if radius_x <= 0.0 || radius_y <= 0.0 {
        return Ok(rect_path(x, y, width, height));
    }
    let left = x;
    let right = x + width;
    let top = y;
    let bottom = y + height;
    Ok(vec![
        PathSegment::MoveTo {
            to: Point::new(left + radius_x, top),
        },
        PathSegment::LineTo {
            to: Point::new(right - radius_x, top),
        },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(right, top + radius_y),
        },
        PathSegment::LineTo {
            to: Point::new(right, bottom - radius_y),
        },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(right - radius_x, bottom),
        },
        PathSegment::LineTo {
            to: Point::new(left + radius_x, bottom),
        },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(left, bottom - radius_y),
        },
        PathSegment::LineTo {
            to: Point::new(left, top + radius_y),
        },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees: 0.0,
            large_arc: false,
            sweep_clockwise: true,
            to: Point::new(left + radius_x, top),
        },
        PathSegment::Close,
    ])
}

fn parse_asset_points(value: &str) -> Result<Vec<Point>> {
    let numbers = value
        .split(|character: char| character == ',' || character.is_ascii_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f64>().map_err(|_| {
                unavailable(format!(
                    "ZenUML embedded SVG point coordinate `{part}` is invalid"
                ))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    if numbers.len() % 2 != 0 {
        return Err(unavailable(
            "ZenUML embedded SVG points must contain x/y pairs",
        ));
    }
    Ok(numbers
        .chunks_exact(2)
        .map(|pair| Point::new(pair[0], pair[1]))
        .collect())
}

fn transform_asset_segment(
    segment: PathSegment,
    x: f64,
    y: f64,
    scale: f64,
    view_box: (f64, f64, f64, f64),
) -> PathSegment {
    let (min_x, min_y, _, _) = view_box;
    let point =
        |value: Point| Point::new(x + (value.x - min_x) * scale, y + (value.y - min_y) * scale);
    match segment {
        PathSegment::MoveTo { to } => PathSegment::MoveTo { to: point(to) },
        PathSegment::LineTo { to } => PathSegment::LineTo { to: point(to) },
        PathSegment::QuadTo { control, to } => PathSegment::QuadTo {
            control: point(control),
            to: point(to),
        },
        PathSegment::CubicTo {
            control1,
            control2,
            to,
        } => PathSegment::CubicTo {
            control1: point(control1),
            control2: point(control2),
            to: point(to),
        },
        PathSegment::ArcTo {
            radius_x,
            radius_y,
            x_axis_rotation_degrees,
            large_arc,
            sweep_clockwise,
            to,
        } => PathSegment::ArcTo {
            radius_x: radius_x * scale,
            radius_y: radius_y * scale,
            x_axis_rotation_degrees,
            large_arc,
            sweep_clockwise,
            to: point(to),
        },
        PathSegment::Close => PathSegment::Close,
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}

fn unavailable(message: impl Into<String>) -> Error {
    Error::DrawingListUnavailable {
        family: "zenuml".to_string(),
        reason: message.into(),
    }
}
