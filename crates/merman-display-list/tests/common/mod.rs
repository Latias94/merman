use merman_display_list::{
    BlendMode, Color, CoordinateSystem, DrawingCommand, DrawingListDocument, DrawingListPolicy,
    DrawingResource, FontDescriptor, FontStyle, GradientStop, LineCap, LineJoin,
    LinearGradientResource, MeasurementProvenance, Paint, PathResource, PathSegment, PathStyle,
    Point, Rect, ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle, TextObligation,
    TextRun, TextStyle, Viewport,
};
use std::collections::BTreeMap;

pub fn sample_document() -> DrawingListDocument {
    let node_path = ResourceId::new("path.node-a");
    let node_fill = ResourceId::new("paint.node-fill");

    DrawingListDocument {
        version: 1,
        coordinate_system: CoordinateSystem::LogicalPixelsYDown,
        viewport: Viewport::new(Rect::new(0.0, 0.0, 120.0, 80.0)),
        policy: DrawingListPolicy::AllowRasterSubtree,
        resources: vec![
            DrawingResource::Path(PathResource {
                id: node_path.clone(),
                segments: vec![
                    PathSegment::MoveTo {
                        to: Point::new(8.0, 8.0),
                    },
                    PathSegment::LineTo {
                        to: Point::new(112.0, 8.0),
                    },
                    PathSegment::LineTo {
                        to: Point::new(112.0, 72.0),
                    },
                    PathSegment::LineTo {
                        to: Point::new(8.0, 72.0),
                    },
                    PathSegment::Close,
                ],
            }),
            DrawingResource::LinearGradient(LinearGradientResource {
                id: node_fill.clone(),
                start: Point::new(8.0, 8.0),
                end: Point::new(112.0, 72.0),
                stops: vec![
                    GradientStop::new(0.0, Color::rgba(236, 248, 255, 255)),
                    GradientStop::new(1.0, Color::rgba(129, 199, 212, 255)),
                ],
            }),
        ],
        commands: vec![
            DrawingCommand::Save,
            DrawingCommand::SetOpacity { opacity: 1.0 },
            DrawingCommand::SetBlendMode {
                blend_mode: BlendMode::Normal,
            },
            DrawingCommand::DrawPath {
                path: node_path,
                style: PathStyle {
                    fill: Some(Paint::resource(node_fill)),
                    stroke: Some(StrokeStyle {
                        paint: Paint::solid(Color::rgba(21, 52, 80, 255)),
                        width: 1.5,
                        dash_array: Vec::new(),
                        line_cap: LineCap::Round,
                        line_join: LineJoin::Round,
                        miter_limit: 4.0,
                    }),
                },
            },
            DrawingCommand::BeginSemanticGroup {
                semantic_id: "node.a".into(),
            },
            DrawingCommand::DrawText {
                run: TextRun {
                    text: "A node".into(),
                    origin: Point::new(60.0, 42.0),
                    bounds: Rect::new(24.0, 28.0, 72.0, 24.0),
                    style: TextStyle {
                        font: FontDescriptor {
                            families: vec!["Arial".into(), "sans-serif".into()],
                            weight: 400,
                            style: FontStyle::Normal,
                        },
                        font_size: 16.0,
                        letter_spacing: 0.0,
                        line_height: 19.2,
                        fill: Paint::solid(Color::rgba(21, 52, 80, 255)),
                    },
                    obligation: TextObligation::HostText {
                        measurement: MeasurementProvenance::HostCallback {
                            profile: "fixture".into(),
                        },
                    },
                },
            },
            DrawingCommand::EndSemanticGroup,
            DrawingCommand::Restore,
        ],
        semantics: vec![SemanticAnnotation {
            id: "node.a".into(),
            role: SemanticRole::Node,
            title: Some("A node".into()),
            description: None,
            link: Some("https://example.invalid/node-a".into()),
        }],
        fallbacks: Vec::new(),
        extensions: BTreeMap::new(),
    }
}
