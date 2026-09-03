//! Renderer-neutral Info adapter.
//!
//! Mermaid's Info diagram is intentionally small: it emits one version label on a fixed canvas.
//! The adapter keeps that source-backed geometry typed and preserves the configured font and
//! text color without introducing an SVG parsing dependency.

use super::{
    InfoSvgBody, RenderDocument, SvgStructureBody, SvgStructureSidecar, parse_font_families,
    theme_color,
};
use crate::config::config_font_family_css;
use crate::drawing_list::support::text_obligation;
use crate::environment::{RenderSession, TextMeasurementPhase};
use crate::family::{FamilyPair, RenderFamilyKind};
use crate::model::{Bounds, InfoDiagramLayout};
use crate::{Error, Result};
use merman_core::OperationPhase;
use merman_core::ParseMetadata;
use merman_core::diagrams::info::InfoDiagramRenderModel;
use merman_display_list::{
    CoordinateSystem, DRAWING_LIST_VERSION, DrawingCommand, DrawingListDocument, DrawingListPolicy,
    FontDescriptor, FontStyle, Paint, Point, Rect, SemanticAnnotation, SemanticRole, TextAnchor,
    TextBaseline, TextDirection, TextRun, TextStyle, Viewport,
};
use serde_json::json;
use std::collections::BTreeMap;

type InfoPair = FamilyPair<InfoDiagramRenderModel, InfoDiagramLayout>;

const VERSION_ORIGIN: Point = Point { x: 100.0, y: 40.0 };
const VERSION_FONT_SIZE: f64 = 32.0;
const VERSION_BOUNDS: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 200.0,
    height: 50.0,
};

pub(crate) fn build_info_document(
    pair: &InfoPair,
    metadata: &ParseMetadata,
    policy: DrawingListPolicy,
    session: &RenderSession,
) -> Result<RenderDocument> {
    session.checkpoint(OperationPhase::Emit)?;
    let layout = pair.layout();
    let bounds = layout
        .bounds
        .as_ref()
        .ok_or_else(|| invalid("Info layout did not provide canvas bounds"))?;
    validate_bounds(bounds)?;
    if layout.version.trim().is_empty() {
        return Err(invalid("Info layout did not provide a version label"));
    }

    let config = metadata.effective_config.as_value();
    let font = FontDescriptor {
        families: parse_font_families(config_font_family_css(config)),
        weight: 400,
        style: FontStyle::Normal,
        postscript_name: None,
        resource: None,
    };
    let text_color = theme_color(config, "textColor", "#333")?;
    let text_obligation = text_obligation(session, TextMeasurementPhase::Layout);

    let mut commands = vec![
        DrawingCommand::Save,
        DrawingCommand::BeginSemanticGroup {
            semantic_id: "info.document".to_string(),
        },
        DrawingCommand::BeginSemanticGroup {
            semantic_id: "info.version".to_string(),
        },
        DrawingCommand::DrawText {
            run: TextRun {
                text: layout.version.clone(),
                origin: VERSION_ORIGIN,
                bounds: VERSION_BOUNDS,
                style: TextStyle {
                    font,
                    font_size: VERSION_FONT_SIZE,
                    letter_spacing: 0.0,
                    line_height: VERSION_FONT_SIZE,
                    fill: Paint::solid(text_color),
                },
                anchor: TextAnchor::Middle,
                baseline: TextBaseline::Alphabetic,
                direction: TextDirection::Auto,
                language: None,
                obligation: text_obligation,
            },
        },
        DrawingCommand::EndSemanticGroup,
        DrawingCommand::EndSemanticGroup,
        DrawingCommand::Restore,
    ];

    let document = DrawingListDocument {
        version: DRAWING_LIST_VERSION,
        coordinate_system: CoordinateSystem::LogicalPixelsYDown,
        viewport: Viewport::new(Rect::new(
            bounds.min_x,
            bounds.min_y,
            bounds.max_x - bounds.min_x,
            bounds.max_y - bounds.min_y,
        )),
        policy,
        resources: Vec::new(),
        commands: std::mem::take(&mut commands),
        semantics: vec![
            SemanticAnnotation {
                id: "info.document".to_string(),
                role: SemanticRole::Document,
                title: Some(metadata.diagram_type.clone()),
                description: None,
                link: None,
            },
            SemanticAnnotation {
                id: "info.version".to_string(),
                role: SemanticRole::Label,
                title: Some(layout.version.clone()),
                description: None,
                link: None,
            },
        ],
        fallbacks: Vec::new(),
        extensions: BTreeMap::from([(
            "x-merman-info".to_string(),
            json!({
                "diagram_type": metadata.diagram_type,
                "label_mode": "plain_host_text",
            }),
        )]),
    };
    document.validate().map_err(Error::DrawingListContract)?;

    Ok(RenderDocument {
        public: document,
        svg: SvgStructureSidecar {
            family: RenderFamilyKind::Info,
            body: SvgStructureBody::Info(InfoSvgBody {
                version: layout.version.clone(),
            }),
        },
    })
}

fn validate_bounds(bounds: &Bounds) -> Result<()> {
    let values = [bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y];
    if values.iter().any(|value| !value.is_finite())
        || bounds.max_x <= bounds.min_x
        || bounds.max_y <= bounds.min_y
    {
        return Err(invalid("Info layout bounds are invalid"));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}
