use base64::{Engine as _, engine::general_purpose::STANDARD};
use merman_display_list::{
    BlendMode, Color, CoordinateSystem, DrawingCommand, DrawingListDocument, DrawingListPolicy,
    DrawingResource, EncodedAsset, FillRule, FontDescriptor, FontResource, FontStyle,
    GradientSpread, GradientStop, ImageResource, LineCap, LineJoin, LinearGradientResource,
    MeasurementProvenance, Paint, PathResource, PathSegment, PathStyle, Point, PositionedGlyph,
    RadialGradientResource, Rect, ResourceId, SemanticAnnotation, SemanticRole, StrokeStyle,
    TextAnchor, TextBaseline, TextDirection, TextObligation, TextPaintOrder, TextRun, TextStyle,
    Transform, Viewport,
};
use std::collections::BTreeMap;

const PNG_1X1_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAAAXNSR0IArs4c6QAAAERlWElmTU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6ABAAMAAAABAAEAAKACAAQAAAABAAAAAaADAAQAAAABAAAAAQAAAAD5Ip3+AAAADElEQVQIHWN4cPwqAAUHAn3f06laAAAAAElFTkSuQmCC";
#[allow(dead_code)]
const PNG_ALPHA_1X1_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=";
const PNG_64X64_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAIAAAAlC+aJAAAABGdBTUEAALGPC/xhBQAAACBjSFJNAAB6JgAAgIQAAPoAAACA6AAAdTAAAOpgAAA6mAAAF3CculE8AAAARGVYSWZNTQAqAAAACAABh2kABAAAAAEAAAAaAAAAAAADoAEAAwAAAAEAAQAAoAIABAAAAAEAAABAoAMABAAAAAEAAABAAAAAAEZRQrAAAAHHaVRYdFhNTDpjb20uYWRvYmUueG1wAAAAAAA8eDp4bXBtZXRhIHhtbG5zOng9ImFkb2JlOm5zOm1ldGEvIiB4OnhtcHRrPSJYTVAgQ29yZSA2LjAuMCI+CiAgIDxyZGY6UkRGIHhtbG5zOnJkZj0iaHR0cDovL3d3dy53My5vcmcvMTk5OS8wMi8yMi1yZGYtc3ludGF4LW5zIyI+CiAgICAgIDxyZGY6RGVzY3JpcHRpb24gcmRmOmFib3V0PSIiCiAgICAgICAgICAgIHhtbG5zOmV4aWY9Imh0dHA6Ly9ucy5hZG9iZS5jb20vZXhpZi8xLjAvIj4KICAgICAgICAgPGV4aWY6Q29sb3JTcGFjZT4xPC9leGlmOkNvbG9yU3BhY2U+CiAgICAgICAgIDxleGlmOlBpeGVsWERpbWVuc2lvbj4xPC9leGlmOlBpeGVsWERpbWVuc2lvbj4KICAgICAgICAgPGV4aWY6UGl4ZWxZRGltZW5zaW9uPjE8L2V4aWY6UGl4ZWxZRGltZW5zaW9uPgogICAgICA8L3JkZjpEZXNjcmlwdGlvbj4KICAgPC9yZGY6UkRGPgo8L3g6eG1wbWV0YT4KyVIhLgAAALJJREFUaAXt0rENwCAUxNCQ/ffKBPQZJWKGV0RfMr1PYLPeZ1+Tzz358ufuPeDvghWoABroC6FAxivACnGgAiiQ8QqwQhyoAApkvAKsEAcqgAIZrwArxIEKoEDGK8AKcaACKJDxCrBCHKgACmS8AqwQByqAAhmvACvEgQqgQMYrwApxoAIokPEKsEIcqAAKZLwCrBAHKoACGa8AK8SBCqBAxivACnGgAiiQ8QqwQhwYX+ADIg8C/KMQOR0AAAAASUVORK5CYII=";
const JPEG_1X1_BASE64: &str = "/9j/4AAQSkZJRgABAQAASABIAAD/4QBMRXhpZgAATU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6ABAAMAAAABAAEAAKACAAQAAAABAAAAAaADAAQAAAABAAAAAQAAAAD/wAARCAABAAEDASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9sAQwACAgICAgIDAgIDBQMDAwUGBQUFBQYIBgYGBgYICggICAgICAoKCgoKCgoKDAwMDAwMDg4ODg4PDw8PDw8PDw8P/9sAQwECAgIEBAQHBAQHEAsJCxAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQ/90ABAAB/9oADAMBAAIRAxEAPwD9QKKKK7D6w//Z";
const WEBP_1X1_BASE64: &str = "UklGRh4AAABXRUJQVlA4TBEAAAAvAAAAAAfQ3e52t/+BiOh/AAA=";
const AVIF_2X2_BASE64: &str = "AAAAGGZ0eXBhdmlmAAAAAGF2aWZtaWYxAAABh21ldGEAAAAAAAAAIWhkbHIAAAAAAAAAAHBpY3QAAAAAAAAAAAAAAAAAAAAAJGRpbmYAAAAcZHJlZgAAAAAAAAABAAAADHVybCAAAAABAAAADnBpdG0AAAAAAAEAAAA4aWluZgAAAAAAAgAAABVpbmZlAgAAAAABAABhdjAxAAAAABVpbmZlAgAAAQACAABFeGlmAAAAABppcmVmAAAAAAAAAA5jZHNjAAIAAQABAAAAqmlwcnAAAACIaXBjbwAAABNjb2xybmNseAACAAIABoAAAAAMY2xsaQDLAEAAAAAUaXNwZQAAAAAAAAACAAAAAgAAAChjbGFwAAAAAQAAAAEAAAABAAAAAf/AAAAAgAAA/8AAAACAAAAAAAAJaXJvdAAAAAAQcGl4aQAAAAADCAgIAAAADGF2MUOBAAwAAAAAGmlwbWEAAAAAAAAAAQABB4ECAwaHhIUAAAAsaWxvYwAAAABEAAACAAEAAAABAAAB/QAAACwAAgAAAAEAAAGvAAAATgAAAAFtZGF0AAAAAAAAAIoAAAAGRXhpZgAATU0AKgAAAAgAAYdpAAQAAAABAAAAGgAAAAAAA6ABAAMAAAABAAEAAKACAAQAAAABAAAAAaADAAQAAAABAAAAAQAAAAASAAoMAAAAAAZ//AgQEDQgMhoQAZIACCCCKAN1RUm0q/42apmxmxK3THNmpg==";

fn decode_image_fixture(encoded: &str) -> Vec<u8> {
    STANDARD
        .decode(encoded)
        .expect("embedded image fixture is valid base64")
}

pub fn png_1x1() -> Vec<u8> {
    decode_image_fixture(PNG_1X1_BASE64)
}

#[allow(dead_code)]
pub fn png_alpha_1x1() -> Vec<u8> {
    decode_image_fixture(PNG_ALPHA_1X1_BASE64)
}

#[allow(dead_code)]
pub fn apng_1x1() -> Vec<u8> {
    let mut data = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut data, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .set_animated(1, 0)
            .expect("one-frame APNG metadata is valid");
        let mut writer = encoder.write_header().expect("APNG header");
        writer.write_image_data(&[0, 0, 0, 0]).expect("APNG frame");
    }
    data
}

#[allow(dead_code)]
pub fn png_64x64() -> Vec<u8> {
    decode_image_fixture(PNG_64X64_BASE64)
}

#[allow(dead_code)]
pub fn jpeg_1x1() -> Vec<u8> {
    decode_image_fixture(JPEG_1X1_BASE64)
}

#[allow(dead_code)]
pub fn webp_1x1() -> Vec<u8> {
    decode_image_fixture(WEBP_1X1_BASE64)
}

#[allow(dead_code)]
pub fn avif_2x2() -> Vec<u8> {
    decode_image_fixture(AVIF_2X2_BASE64)
}

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
            DrawingCommand::draw_text(TextRun {
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
                    stroke: None,
                    paint_order: TextPaintOrder::FillThenStroke,
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
            }),
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
            image: EncodedAsset::new("image/png", png_1x1()),
            pixel_width: 1,
            pixel_height: 1,
            has_alpha: false,
        }));
    document.resources.push(DrawingResource::Pattern(
        merman_display_list::PatternResource {
            id: ResourceId::new("paint.pattern"),
            image: image_id,
            tile: Rect::new(0.0, 0.0, 1.0, 1.0),
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
    // Keep the clip inside its own save scope so the layer/semantic/clip/state nesting remains
    // valid and mirrors the SVG serializer's strict LIFO group contract.
    document.commands.insert(6, DrawingCommand::Save);
    document.commands.insert(
        7,
        DrawingCommand::ClipPath {
            path: path_id,
            fill_rule: FillRule::EvenOdd,
        },
    );
    let semantic_end_index = document
        .commands
        .iter()
        .position(|command| matches!(command, DrawingCommand::EndSemanticGroup))
        .expect("fixture has a semantic group end");
    document
        .commands
        .insert(semantic_end_index, DrawingCommand::Restore);
    let restore_index = document
        .commands
        .iter()
        .rposition(|command| matches!(command, DrawingCommand::Restore))
        .expect("fixture has a restore command");
    document
        .commands
        .insert(restore_index, DrawingCommand::EndLayer);
    document
}
