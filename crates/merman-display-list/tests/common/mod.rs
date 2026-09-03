use merman_display_list::{
    BlendMode, Color, CoordinateSystem, DrawingCommand, DrawingListDocument, DrawingListPolicy,
    DrawingResource, EncodedAsset, FillRule, FontDescriptor, FontResource, FontStyle,
    GradientSpread, GradientStop, ImageResource, LineCap, LineJoin, LinearGradientResource,
    MeasurementProvenance, Paint, PathResource, PathSegment, PathStyle, Point, PositionedGlyph,
    RadialGradientResource, Rect, ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle,
    TextAnchor, TextBaseline, TextDirection, TextObligation, TextRun, TextStyle, Transform,
    Viewport,
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
                transform: Transform::IDENTITY,
                spread: GradientSpread::Pad,
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
                    fill_rule: FillRule::NonZero,
                    fill: Some(Paint::resource(node_fill)),
                    stroke: Some(StrokeStyle {
                        paint: Paint::solid(Color::rgba(21, 52, 80, 255)),
                        width: 1.5,
                        dash_array: Vec::new(),
                        dash_offset: 0.0,
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
                            postscript_name: None,
                            resource: None,
                        },
                        font_size: 16.0,
                        letter_spacing: 0.0,
                        line_height: 19.2,
                        fill: Paint::solid(Color::rgba(21, 52, 80, 255)),
                    },
                    anchor: TextAnchor::Middle,
                    baseline: TextBaseline::Middle,
                    direction: TextDirection::Auto,
                    language: None,
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

#[allow(dead_code)]
pub fn extended_document() -> DrawingListDocument {
    let mut document = sample_document();
    let path_id = ResourceId::new("path.node-a");
    let radial_id = ResourceId::new("paint.radial");
    let image_id = ResourceId::new("image.pattern-tile");
    let font_id = ResourceId::new("font.fixture");

    document
        .resources
        .push(DrawingResource::RadialGradient(RadialGradientResource {
            id: radial_id.clone(),
            center: Point::new(60.0, 40.0),
            focal: Point::new(54.0, 36.0),
            radius: 48.0,
            transform: Transform::IDENTITY,
            spread: GradientSpread::Reflect,
            stops: vec![
                GradientStop::new(0.0, Color::rgba(255, 255, 255, 255)),
                GradientStop::new(1.0, Color::rgba(48, 96, 144, 255)),
            ],
        }));
    document
        .resources
        .push(DrawingResource::Image(ImageResource {
            id: image_id.clone(),
            image: EncodedAsset::new("image/png", vec![0x89, b'P', b'N', b'G']),
            pixel_width: 8,
            pixel_height: 8,
            has_alpha: true,
        }));
    document.resources.push(DrawingResource::Pattern(
        merman_display_list::PatternResource {
            id: ResourceId::new("paint.pattern"),
            image: image_id,
            tile: Rect::new(0.0, 0.0, 8.0, 8.0),
            transform: Transform::IDENTITY,
            repetition: merman_display_list::PatternRepeat::Repeat,
        },
    ));
    document.resources.push(DrawingResource::Font(FontResource {
        id: font_id.clone(),
        font: EncodedAsset::new("font/woff2", vec![0x77, 0x4f, 0x46, 0x32]),
    }));

    let path = document
        .resources
        .iter_mut()
        .find_map(|resource| resource.path_mut())
        .expect("fixture has a path resource");
    path.segments[2] = PathSegment::ArcTo {
        radius_x: 12.0,
        radius_y: 8.0,
        x_axis_rotation_degrees: 15.0,
        large_arc: false,
        sweep_clockwise: true,
        to: Point::new(112.0, 72.0),
    };

    for command in &mut document.commands {
        if let DrawingCommand::DrawPath { style, .. } = command {
            style.fill = Some(Paint::resource(radial_id.clone()));
        }
        if let DrawingCommand::DrawText { run } = command {
            run.style.font.resource = Some(font_id.clone());
            run.obligation = TextObligation::GlyphRun {
                glyphs: vec![PositionedGlyph {
                    glyph_id: 42,
                    origin: Point::new(24.0, 28.0),
                    advance: Point::new(8.0, 0.0),
                }],
            };
        }
    }

    document.commands.insert(
        1,
        DrawingCommand::BeginLayer {
            bounds: Rect::new(0.0, 0.0, 120.0, 80.0),
            opacity: 0.95,
            blend_mode: BlendMode::SoftLight,
        },
    );
    document.commands.insert(
        2,
        DrawingCommand::ClipPath {
            path: path_id,
            fill_rule: FillRule::EvenOdd,
        },
    );
    let restore_index = document
        .commands
        .iter()
        .position(|command| matches!(command, DrawingCommand::Restore))
        .expect("fixture has a restore command");
    document
        .commands
        .insert(restore_index, DrawingCommand::EndLayer);
    document
}
