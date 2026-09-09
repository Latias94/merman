mod common;

use common::{apng_1x1, extended_document, png_1x1, png_64x64, png_alpha_1x1, sample_document};
use merman_display_list::{
    AlphaMode, DrawingCommand, DrawingListError, DrawingListLimits, DrawingResource, EncodedAsset,
    EncodedImage, FallbackReason, FillRule, FontResource, ImageResource, Paint, PathSegment, Point,
    RasterFallback, RasterFormat, Rect, ResourceId, StrokeStyle, TextPaintOrder, ValidationEvent,
    VisualSource,
};

#[derive(Debug, PartialEq, Eq)]
enum ControlledValidationError {
    Document(DrawingListError),
    Stopped,
}

impl From<DrawingListError> for ControlledValidationError {
    fn from(error: DrawingListError) -> Self {
        Self::Document(error)
    }
}

#[test]
fn invalid_paint_opacity_is_rejected_for_every_path_and_text_paint() {
    for opacity in [-0.1, 1.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for paint in [
            Paint::solid(merman_display_list::Color::rgba(255, 0, 0, 128)),
            Paint::resource(ResourceId::new("paint.node-fill")),
        ] {
            for slot in 0..4 {
                let mut document = sample_document();
                let DrawingCommand::DrawPath { style, .. } = &document.commands[3] else {
                    panic!("fixture path command");
                };
                let text_stroke = style.stroke.clone();
                let DrawingCommand::DrawText { run } = &mut document.commands[5] else {
                    panic!("fixture text command");
                };
                run.style.stroke = text_stroke;
                let invalid = paint.clone().with_opacity(opacity);
                if slot < 2 {
                    let DrawingCommand::DrawPath { style, .. } = &mut document.commands[3] else {
                        unreachable!();
                    };
                    if slot == 0 {
                        style.fill = Some(invalid);
                    } else {
                        style.stroke.as_mut().unwrap().paint = invalid;
                    }
                } else {
                    let DrawingCommand::DrawText { run } = &mut document.commands[5] else {
                        unreachable!();
                    };
                    if slot == 2 {
                        run.style.fill = invalid;
                    } else {
                        run.style.stroke.as_mut().unwrap().paint = invalid;
                    }
                }
                assert!(
                    matches!(document.validate(), Err(DrawingListError::InvalidDocument(message)) if message.contains("paint opacity")),
                    "slot {slot}, opacity {opacity}"
                );
                assert!(document.canonical_json_bytes().is_err());
            }
        }
    }
}

#[test]
fn controlled_validation_separates_quantity_policy_from_correctness() {
    let mut document = sample_document();
    let depth = DrawingListLimits::default().max_nesting_depth + 1;
    document.commands = vec![DrawingCommand::Save; depth];
    document
        .commands
        .extend(vec![DrawingCommand::Restore; depth]);
    assert!(matches!(
        document.validate(),
        Err(DrawingListError::ResourceLimit {
            resource: "nesting_depth",
            ..
        })
    ));
    document
        .validate_with_control(|_| Ok::<_, DrawingListError>(()))
        .unwrap();

    // The ordinary decoder keeps its default policy even when another consumer admits the model.
    let bytes = serde_json::to_vec(&document).unwrap();
    assert!(matches!(
        merman_display_list::DrawingListDocument::from_json_bytes(&bytes),
        Err(DrawingListError::ResourceLimit {
            resource: "nesting_depth",
            ..
        })
    ));
    document.commands.push(DrawingCommand::Restore);
    assert!(matches!(
        document.validate_with_control(|_| Ok::<_, ControlledValidationError>(())),
        Err(ControlledValidationError::Document(
            DrawingListError::InvalidDocument(_)
        ))
    ));
}

#[test]
fn controlled_image_admission_precedes_pixel_decode_and_remains_cancellable() {
    let mut document = sample_document();
    document
        .resources
        .push(DrawingResource::Image(ImageResource {
            id: ResourceId::new("controlled-image"),
            image: EncodedAsset::new("image/png", corrupt_png_deflate_with_valid_crc(png_64x64())),
            pixel_width: 64,
            pixel_height: 64,
            has_alpha: false,
        }));
    let result = document.validate_with_control(|event| {
        if let ValidationEvent::ResourceCount {
            resource: "image_pixels",
            actual,
        } = event
        {
            assert_eq!(actual, 4096);
            return Err(ControlledValidationError::Stopped);
        }
        Ok(())
    });
    assert_eq!(result, Err(ControlledValidationError::Stopped));
    assert!(matches!(
        document.validate_with_control(|_| Ok::<_, ControlledValidationError>(())),
        Err(ControlledValidationError::Document(
            DrawingListError::InvalidDocument(_)
        ))
    ));

    let DrawingResource::Image(image) = document.resources.last_mut().unwrap() else {
        unreachable!();
    };
    image.image.data = png_64x64();
    let mut pixels_admitted = false;
    let mut row_checkpoints = 0;
    let result = document.validate_with_control(|event| {
        match event {
            ValidationEvent::ResourceCount {
                resource: "image_pixels",
                ..
            } => pixels_admitted = true,
            ValidationEvent::Checkpoint if pixels_admitted => {
                row_checkpoints += 1;
                if row_checkpoints == 3 {
                    return Err(ControlledValidationError::Stopped);
                }
            }
            _ => {}
        }
        Ok(())
    });
    assert_eq!(result, Err(ControlledValidationError::Stopped));
    assert_eq!(row_checkpoints, 3);
    document.validate().unwrap();
}

fn png_crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let polynomial = 0xedb8_8320 & 0_u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ polynomial;
        }
    }
    !crc
}

fn corrupt_png_deflate_with_valid_crc(mut data: Vec<u8>) -> Vec<u8> {
    let mut chunk_start = 8;
    while chunk_start + 12 <= data.len() {
        let data_len = u32::from_be_bytes(
            data[chunk_start..chunk_start + 4]
                .try_into()
                .expect("PNG chunk length has four bytes"),
        ) as usize;
        let data_start = chunk_start + 8;
        let data_end = data_start
            .checked_add(data_len)
            .expect("PNG fixture chunk length does not overflow");
        let chunk_end = data_end
            .checked_add(4)
            .expect("PNG fixture CRC offset does not overflow");
        assert!(chunk_end <= data.len(), "PNG fixture chunk is complete");

        if &data[chunk_start + 4..data_start] == b"IDAT" {
            assert!(data_start < data_end, "PNG fixture has compressed data");
            data[data_start] ^= 0xff;
            let crc = png_crc32(&data[chunk_start + 4..data_end]);
            data[data_end..chunk_end].copy_from_slice(&crc.to_be_bytes());
            return data;
        }
        chunk_start = chunk_end;
    }
    panic!("PNG fixture has an IDAT chunk");
}

fn png_with_ihdr_dimensions(width: u32, height: u32) -> Vec<u8> {
    let mut data = png_1x1();
    assert_eq!(&data[12..16], b"IHDR", "PNG fixture begins with IHDR");
    data[16..20].copy_from_slice(&width.to_be_bytes());
    data[20..24].copy_from_slice(&height.to_be_bytes());
    let crc = png_crc32(&data[12..29]);
    data[29..33].copy_from_slice(&crc.to_be_bytes());
    data
}

#[test]
fn unpainted_paths_preserve_geometry_and_round_trip() {
    let mut document = sample_document();
    let style = document
        .commands
        .iter_mut()
        .find_map(|command| match command {
            DrawingCommand::DrawPath { style, .. } => Some(style),
            _ => None,
        })
        .unwrap();
    style.fill = None;
    style.stroke = None;
    document
        .validate()
        .expect("explicitly disabled paint is a valid retained path");
    let encoded = document.canonical_json_bytes().unwrap();
    let decoded = merman_display_list::DrawingListDocument::from_json_bytes(&encoded).unwrap();
    assert_eq!(decoded.canonical_json_bytes().unwrap(), encoded);
}

#[test]
fn validator_rejects_unbalanced_state_and_wrong_resource_kind() {
    let mut document = sample_document();
    document.commands = vec![DrawingCommand::Restore];
    assert!(document.validate().is_err());

    let mut document = sample_document();
    let path = document
        .resources
        .iter_mut()
        .find_map(|resource| resource.path_mut())
        .expect("fixture has a path resource");
    path.segments[0] = PathSegment::LineTo {
        to: Point::new(1.0, 1.0),
    };
    assert!(document.validate().is_err());

    let mut document = sample_document();
    let path_id = ResourceId::new("path.node-a");
    document.commands[3] = DrawingCommand::DrawPath {
        path: path_id,
        style: merman_display_list::PathStyle {
            fill_rule: FillRule::NonZero,
            fill: Some(Paint::resource(ResourceId::new("path.node-a"))),
            stroke: None,
        },
    };
    assert!(document.validate().is_err());
}

#[test]
fn validator_checks_text_stroke_style_and_paint_resources() {
    let mut document = sample_document();
    let DrawingCommand::DrawText { run } = &mut document.commands[5] else {
        panic!("sample document command 5 is text");
    };
    run.style.stroke = Some(StrokeStyle {
        paint: Paint::resource(ResourceId::new("paint.node-fill")),
        width: 1.5,
        dash_array: Vec::new(),
        dash_offset: 0.0,
        line_cap: merman_display_list::LineCap::Round,
        line_join: merman_display_list::LineJoin::Round,
        miter_limit: 4.0,
    });
    run.style.paint_order = TextPaintOrder::StrokeThenFill;
    document
        .validate()
        .expect("text stroke may reference a known paint resource");

    let DrawingCommand::DrawText { run } = &mut document.commands[5] else {
        unreachable!("sample document command 5 remains text");
    };
    run.style
        .stroke
        .as_mut()
        .expect("test installed a text stroke")
        .paint = Paint::resource(ResourceId::new("paint.missing"));
    assert!(document.validate().is_err());

    let DrawingCommand::DrawText { run } = &mut document.commands[5] else {
        unreachable!("sample document command 5 remains text");
    };
    let stroke = run
        .style
        .stroke
        .as_mut()
        .expect("test installed a text stroke");
    stroke.paint = Paint::solid(merman_display_list::Color::rgba(0, 0, 0, 255));
    stroke.width = f64::NAN;
    assert!(document.validate().is_err());
}

#[test]
fn validator_enforces_save_scopes_for_clips_and_groups() {
    let mut document = sample_document();
    let path_id = ResourceId::new("path.node-a");

    let restore = document
        .commands
        .pop()
        .expect("sample document has a final restore");
    document.commands.insert(
        0,
        DrawingCommand::ClipPath {
            path: path_id.clone(),
            fill_rule: FillRule::NonZero,
        },
    );
    document.commands.push(restore);
    assert!(document.validate().is_err(), "clip paths need a save scope");

    let mut document = sample_document();
    document.commands.insert(
        1,
        DrawingCommand::BeginSemanticGroup {
            semantic_id: "node.a".into(),
        },
    );
    assert!(
        document.validate().is_err(),
        "a restore must not cross an open semantic group"
    );

    let mut document = sample_document();
    let restore = document
        .commands
        .pop()
        .expect("sample document has a final restore");
    document.commands.insert(
        3,
        DrawingCommand::ClipPath {
            path: path_id,
            fill_rule: FillRule::EvenOdd,
        },
    );
    document.commands.push(restore);
    document
        .validate()
        .expect("a clip path inside the sample save scope is valid");
}

#[test]
fn validator_rejects_non_lifo_group_endings() {
    let mut document = sample_document();
    document.commands = vec![
        DrawingCommand::Save,
        DrawingCommand::BeginLayer {
            bounds: Rect::new(0.0, 0.0, 120.0, 80.0),
            opacity: 1.0,
            blend_mode: merman_display_list::BlendMode::Normal,
        },
        DrawingCommand::BeginSemanticGroup {
            semantic_id: "node.a".into(),
        },
        DrawingCommand::EndLayer,
        DrawingCommand::EndSemanticGroup,
        DrawingCommand::Restore,
    ];

    assert!(document.validate().is_err());
    assert!(document.footprint().is_err());
}

#[test]
fn validator_rejects_replaced_group_at_the_same_depth() {
    let mut document = sample_document();
    document.commands = vec![
        DrawingCommand::BeginLayer {
            bounds: Rect::new(0.0, 0.0, 120.0, 80.0),
            opacity: 1.0,
            blend_mode: merman_display_list::BlendMode::Normal,
        },
        DrawingCommand::Save,
        DrawingCommand::EndLayer,
        DrawingCommand::BeginLayer {
            bounds: Rect::new(0.0, 0.0, 120.0, 80.0),
            opacity: 1.0,
            blend_mode: merman_display_list::BlendMode::Normal,
        },
        DrawingCommand::Restore,
        DrawingCommand::EndLayer,
    ];

    assert!(document.validate().is_err());
    assert!(document.footprint().is_err());
}

#[test]
fn validator_enforces_exact_command_and_numeric_boundaries() {
    let document = sample_document();
    let exact = DrawingListLimits {
        max_commands: document.commands.len(),
        ..DrawingListLimits::default()
    };
    document
        .validate_with_limits(&exact)
        .expect("exact command limit is accepted");

    let too_small = DrawingListLimits {
        max_commands: document.commands.len() - 1,
        ..exact
    };
    assert!(document.validate_with_limits(&too_small).is_err());

    let mut non_finite = document;
    non_finite.viewport.bounds.width = f64::NAN;
    assert!(non_finite.validate().is_err());

    let document = sample_document();
    let encoded = document.canonical_json_bytes().unwrap();
    let too_small = DrawingListLimits {
        max_serialized_bytes: encoded.len() - 1,
        ..DrawingListLimits::default()
    };
    assert!(
        document
            .canonical_json_bytes_with_limits(&too_small)
            .is_err()
    );
    assert!(
        merman_display_list::DrawingListDocument::from_json_bytes_with_limits(&encoded, &too_small)
            .is_err()
    );
}

#[test]
fn validator_enforces_cumulative_nesting_across_scope_kinds() {
    let document = common::extended_document();
    let limits = DrawingListLimits {
        max_nesting_depth: 3,
        ..DrawingListLimits::default()
    };

    assert!(document.validate_with_limits(&limits).is_err());
}

#[test]
fn document_footprint_reports_cumulative_geometry_and_asset_cost() {
    let mut document = common::extended_document();
    let baseline_work_units = document
        .footprint()
        .expect("fixture scopes are balanced")
        .work_units()
        .expect("fixture work units fit usize");
    let style = document
        .commands
        .iter_mut()
        .find_map(|command| match command {
            DrawingCommand::DrawPath { style, .. } => Some(style),
            _ => None,
        })
        .expect("extended fixture has a path command");
    style
        .stroke
        .as_mut()
        .expect("extended fixture path has a stroke")
        .dash_array = vec![3.0, 1.0];
    let footprint = document.footprint().expect("fixture scopes are balanced");
    let image = document
        .resources
        .iter()
        .find_map(|resource| match resource {
            DrawingResource::Image(image) => Some(image),
            _ => None,
        })
        .expect("extended fixture has an image");

    assert_eq!(footprint.commands, document.commands.len());
    assert_eq!(footprint.resources, document.resources.len());
    assert_eq!(footprint.semantics, document.semantics.len());
    assert_eq!(footprint.path_segments, 5);
    assert_eq!(footprint.stroke_dash_entries, 2);
    assert_eq!(footprint.gradient_stops, 4);
    assert_eq!(footprint.image_bytes, image.image.data.len());
    assert_eq!(footprint.image_pixels, 1);
    assert_eq!(footprint.font_bytes, 4);
    assert_eq!(footprint.text_bytes, "A node".len());
    assert_eq!(footprint.glyphs, 1);
    assert_eq!(footprint.max_nesting_depth, 5);
    assert_eq!(
        footprint.work_units().unwrap(),
        baseline_work_units + footprint.stroke_dash_entries
    );
}

#[test]
fn footprint_checks_cumulative_limits_before_serialization() {
    let document = common::extended_document();
    let footprint = document.footprint().expect("fixture scopes are balanced");
    let exact = DrawingListLimits {
        max_path_segments: footprint.path_segments,
        max_stroke_dash_entries: footprint.stroke_dash_entries,
        max_image_bytes: footprint.image_bytes,
        max_image_pixels: footprint.image_pixels,
        max_fallback_pixels: footprint.fallback_pixels,
        max_font_bytes: footprint.font_bytes,
        max_nesting_depth: footprint.max_nesting_depth,
        max_text_bytes: footprint.text_bytes,
        max_glyphs: footprint.glyphs,
        ..DrawingListLimits::default()
    };
    footprint
        .check_limits(&exact)
        .expect("exact cumulative limits are accepted");

    let too_small = DrawingListLimits {
        max_path_segments: footprint.path_segments - 1,
        ..exact
    };
    let error = footprint
        .check_limits(&too_small)
        .expect_err("a smaller cumulative path budget must reject");
    assert!(matches!(
        error,
        merman_display_list::DrawingListError::ResourceLimit {
            resource: "path_segments",
            actual,
            maximum,
        } if actual == footprint.path_segments && maximum + 1 == actual
    ));
}

#[test]
fn document_footprint_rejects_unbalanced_scopes_before_accounting() {
    let mut document = sample_document();
    document.commands.pop();
    assert!(document.footprint().is_err());
}

#[test]
fn validator_enforces_document_wide_path_segment_budget() {
    let mut document = sample_document();
    document
        .resources
        .push(DrawingResource::Path(merman_display_list::PathResource {
            id: ResourceId::new("path.unreferenced"),
            segments: vec![
                PathSegment::MoveTo {
                    to: Point::new(0.0, 0.0),
                },
                PathSegment::LineTo {
                    to: Point::new(1.0, 1.0),
                },
            ],
        }));

    let limits = DrawingListLimits {
        max_path_segments: 5,
        ..DrawingListLimits::default()
    };
    assert!(document.validate_with_limits(&limits).is_err());
}

#[test]
fn validator_enforces_document_wide_stroke_dash_budget() {
    let mut document = sample_document();
    let DrawingCommand::DrawPath { style, .. } = &mut document.commands[3] else {
        panic!("sample document command 3 is a path");
    };
    style
        .stroke
        .as_mut()
        .expect("sample document path has a stroke")
        .dash_array = vec![3.0, 1.0];
    let footprint = document.footprint().expect("fixture scopes are balanced");
    assert_eq!(footprint.path_segments, 5);
    assert_eq!(footprint.stroke_dash_entries, 2);

    let limits = DrawingListLimits {
        max_stroke_dash_entries: 1,
        ..DrawingListLimits::default()
    };
    assert!(matches!(
        document.validate_with_limits(&limits),
        Err(DrawingListError::ResourceLimit {
            resource: "stroke_dash_entries",
            actual: 2,
            maximum: 1,
        })
    ));
}

#[test]
fn extended_visual_vocabulary_validates_as_one_document() {
    let document = extended_document();
    document
        .validate()
        .expect("extended visual vocabulary is valid");
    let bytes = document
        .canonical_json_bytes()
        .expect("extended document serializes canonically");
    let decoded = merman_display_list::DrawingListDocument::from_json_bytes(&bytes)
        .expect("extended document round-trips");
    assert_eq!(decoded.canonical_json_bytes().unwrap(), bytes);
}

#[test]
fn fallback_validation_uses_indexed_images_for_large_resource_tables() {
    let mut document = sample_document();
    let fallback_count = 2_048;
    let image_data = png_1x1();
    let mut fallback_commands = Vec::with_capacity(fallback_count);
    for index in 0..fallback_count {
        let image = ResourceId::new(format!("image.fallback.{index:04}"));
        let fallback_id = format!("fallback.{index:04}");
        document
            .resources
            .push(DrawingResource::Image(ImageResource {
                id: image.clone(),
                image: EncodedImage::new("image/png", image_data.clone()),
                pixel_width: 1,
                pixel_height: 1,
                has_alpha: false,
            }));
        document.fallbacks.push(RasterFallback {
            id: fallback_id.clone(),
            image,
            bounds: Rect::new(0.0, 0.0, 1.0, 1.0),
            pixel_width: 1,
            pixel_height: 1,
            scale: 1.0,
            format: RasterFormat::Png,
            alpha: AlphaMode::Opaque,
            reason: FallbackReason::UnsupportedEffect,
            source: VisualSource {
                family: "fixture".into(),
                element_id: Some(fallback_id.clone()),
                effect: "fixture_effect".into(),
            },
        });
        fallback_commands.push(DrawingCommand::DrawRasterSubtree { fallback_id });
    }
    let restore = document
        .commands
        .pop()
        .expect("sample document has a final restore");
    document.commands.extend(fallback_commands);
    document.commands.push(restore);

    document
        .validate()
        .expect("large fallback/resource table remains bounded and valid");
}

#[test]
fn media_type_validation_is_case_insensitive_and_parameter_tolerant() {
    let mut document = extended_document();
    document
        .resources
        .iter_mut()
        .find_map(|resource| match resource {
            DrawingResource::Image(image) => Some(image),
            _ => None,
        })
        .expect("extended fixture has an image")
        .image
        .media_type = "\t IMAGE/PNG \t; charset=binary\r\n".into();
    document
        .validate()
        .expect("image media type comparison should ignore case and parameters");

    document
        .resources
        .iter_mut()
        .find_map(|resource| match resource {
            DrawingResource::Image(image) => Some(image),
            _ => None,
        })
        .expect("extended fixture has an image")
        .image
        .media_type = "text/plain".into();
    assert!(document.validate().is_err());

    for media_type in [
        "IMAGE /PNG",
        "IMAGE/ PNG",
        "\u{a0}IMAGE/PNG",
        "IMAGE/PNG\u{a0}",
        "IMAGE/PNG\u{a0}; charset=binary",
    ] {
        let mut document = extended_document();
        document
            .resources
            .iter_mut()
            .find_map(|resource| match resource {
                DrawingResource::Image(image) => Some(image),
                _ => None,
            })
            .expect("extended fixture has an image")
            .image
            .media_type = media_type.into();
        assert!(
            document.validate().is_err(),
            "media type whitespace adjacent to '/' must be rejected: {media_type}"
        );
    }
}

#[test]
fn image_pixel_budget_uses_u64_before_pointer_width_conversion() {
    const DIMENSION: u32 = 65_536;

    let mut document = sample_document();
    document
        .resources
        .push(DrawingResource::Image(ImageResource {
            id: ResourceId::new("image.pointer-width-boundary"),
            image: EncodedImage::new("image/png", png_with_ihdr_dimensions(DIMENSION, DIMENSION)),
            pixel_width: DIMENSION,
            pixel_height: DIMENSION,
            has_alpha: false,
        }));
    let limits = DrawingListLimits {
        max_image_pixels: 1,
        ..DrawingListLimits::default()
    };
    let expected_actual =
        usize::try_from(u64::from(DIMENSION) * u64::from(DIMENSION)).unwrap_or(usize::MAX);

    assert!(matches!(
        document.validate_with_limits(&limits),
        Err(DrawingListError::ResourceLimit {
            resource: "image_pixels",
            actual,
            maximum: 1,
        }) if actual == expected_actual
    ));
}

#[test]
fn image_validation_checks_payload_format_dimensions_and_alpha_contract() {
    for (index, data, has_alpha) in [(0, png_1x1(), false), (1, png_alpha_1x1(), true)] {
        let mut document = sample_document();
        document
            .resources
            .push(DrawingResource::Image(ImageResource {
                id: ResourceId::new(format!("image.supported.{index}")),
                image: EncodedImage::new("image/png", data),
                pixel_width: 1,
                pixel_height: 1,
                has_alpha,
            }));
        document
            .validate()
            .expect("supported image metadata matches its payload");
    }

    let mut unsupported_format = sample_document();
    unsupported_format
        .resources
        .push(DrawingResource::Image(ImageResource {
            id: ResourceId::new("image.jpeg"),
            image: EncodedImage::new("image/jpeg", png_1x1()),
            pixel_width: 1,
            pixel_height: 1,
            has_alpha: false,
        }));
    assert!(unsupported_format.validate().is_err());

    let mut mismatched_alpha = sample_document();
    mismatched_alpha
        .resources
        .push(DrawingResource::Image(ImageResource {
            id: ResourceId::new("image.mismatched-alpha"),
            image: EncodedImage::new("image/png", png_1x1()),
            pixel_width: 1,
            pixel_height: 1,
            has_alpha: true,
        }));
    assert!(mismatched_alpha.validate().is_err());

    let mut corrupt_crc = png_1x1();
    let idat = corrupt_crc
        .windows(4)
        .position(|window| window == b"IDAT")
        .expect("PNG fixture has IDAT data");
    corrupt_crc[idat + 4] ^= 0x01;
    let corrupt_deflate = corrupt_png_deflate_with_valid_crc(png_1x1());
    let mut trailing_data = png_1x1();
    trailing_data.extend_from_slice(b"trailing data");
    trailing_data.extend_from_slice(b"\0\0\0\0IEND\xaeB`\x82");
    for (name, data) in [
        ("truncated", png_1x1()[..24].to_vec()),
        ("corrupt-crc", corrupt_crc),
        ("corrupt-deflate", corrupt_deflate),
        ("trailing-data", trailing_data),
        ("animated", apng_1x1()),
    ] {
        let mut document = sample_document();
        document
            .resources
            .push(DrawingResource::Image(ImageResource {
                id: ResourceId::new(format!("image.invalid.{name}")),
                image: EncodedImage::new("image/png", data),
                pixel_width: 1,
                pixel_height: 1,
                has_alpha: false,
            }));
        assert!(
            document.validate().is_err(),
            "{name} PNG payload must be rejected"
        );
    }

    let mut mismatched_dimensions = sample_document();
    mismatched_dimensions
        .resources
        .push(DrawingResource::Image(ImageResource {
            id: ResourceId::new("image.mismatched-dimensions"),
            image: EncodedImage::new("image/png", png_64x64()),
            pixel_width: 1,
            pixel_height: 1,
            has_alpha: false,
        }));
    let error = mismatched_dimensions
        .validate()
        .expect_err("intrinsic dimensions must match declared dimensions");
    assert!(error.to_string().contains("contains 64x64 pixels"));
    let limits = DrawingListLimits {
        max_image_pixels: 1,
        ..DrawingListLimits::default()
    };
    let error = mismatched_dimensions
        .validate_with_limits(&limits)
        .expect_err("intrinsic dimensions must be charged before PNG decoding");
    assert!(matches!(
        error,
        DrawingListError::ResourceLimit {
            resource: "image_pixels",
            actual: 4096,
            maximum: 1
        }
    ));
}

#[test]
fn cumulative_image_pixel_budget_stops_before_decoding_the_next_payload() {
    let mut document = sample_document();
    document
        .resources
        .push(DrawingResource::Image(ImageResource {
            id: ResourceId::new("image.first"),
            image: EncodedImage::new("image/png", png_1x1()),
            pixel_width: 1,
            pixel_height: 1,
            has_alpha: false,
        }));
    document
        .resources
        .push(DrawingResource::Image(ImageResource {
            id: ResourceId::new("image.second"),
            image: EncodedImage::new("image/png", corrupt_png_deflate_with_valid_crc(png_1x1())),
            pixel_width: 1,
            pixel_height: 1,
            has_alpha: false,
        }));

    let limits = DrawingListLimits {
        max_image_pixels: 1,
        ..DrawingListLimits::default()
    };
    assert!(matches!(
        document.validate_with_limits(&limits),
        Err(DrawingListError::ResourceLimit {
            resource: "image_pixels",
            actual: 2,
            maximum: 1,
        })
    ));
}

#[test]
fn raster_fallback_alpha_mode_matches_the_validated_png_resource() {
    let mut document = extended_document();
    document.fallbacks.push(RasterFallback {
        id: "fallback.alpha".into(),
        image: ResourceId::new("image.pattern-tile"),
        bounds: Rect::new(0.0, 0.0, 1.0, 1.0),
        pixel_width: 1,
        pixel_height: 1,
        scale: 1.0,
        format: RasterFormat::Png,
        alpha: AlphaMode::Straight,
        reason: FallbackReason::UnsupportedEffect,
        source: VisualSource {
            family: "fixture".into(),
            element_id: None,
            effect: "alpha".into(),
        },
    });
    document.commands.insert(
        0,
        DrawingCommand::DrawRasterSubtree {
            fallback_id: "fallback.alpha".into(),
        },
    );
    assert!(
        document.validate().is_err(),
        "an opaque PNG cannot declare straight fallback alpha"
    );

    let image = document
        .resources
        .iter_mut()
        .find_map(|resource| match resource {
            DrawingResource::Image(image) if image.id.as_str() == "image.pattern-tile" => {
                Some(image)
            }
            _ => None,
        })
        .expect("extended fixture image");
    image.image.data = png_alpha_1x1();
    image.has_alpha = true;
    document.fallbacks[0].alpha = AlphaMode::Opaque;
    assert!(
        document.validate().is_err(),
        "a PNG with alpha cannot declare an opaque fallback"
    );
    document.fallbacks[0].alpha = AlphaMode::Straight;
    document
        .validate()
        .expect("matching PNG and fallback alpha metadata");
}

#[test]
fn decoder_charges_encoded_assets_before_materializing_base64_payloads() {
    let document = extended_document();
    let image_bytes = document
        .resources
        .iter()
        .filter_map(|resource| match resource {
            DrawingResource::Image(image) => Some(image.image.data.len()),
            _ => None,
        })
        .sum();
    let font_bytes = document
        .resources
        .iter()
        .filter_map(|resource| match resource {
            DrawingResource::Font(font) => Some(font.font.data.len()),
            _ => None,
        })
        .sum();
    let bytes = serde_json::to_vec(&document).expect("fixture JSON");
    let exact = DrawingListLimits {
        max_image_bytes: image_bytes,
        max_font_bytes: font_bytes,
        ..DrawingListLimits::default()
    };
    merman_display_list::DrawingListDocument::from_json_bytes_with_limits(&bytes, &exact)
        .expect("exact encoded asset budgets are accepted");

    for (resource, limits) in [
        (
            "image_bytes",
            DrawingListLimits {
                max_image_bytes: image_bytes - 1,
                ..exact
            },
        ),
        (
            "font_bytes",
            DrawingListLimits {
                max_font_bytes: font_bytes - 1,
                ..exact
            },
        ),
    ] {
        assert!(matches!(
            merman_display_list::DrawingListDocument::from_json_bytes_with_limits(&bytes, &limits),
            Err(DrawingListError::ResourceLimit {
                resource: actual,
                ..
            }) if actual == resource
        ));
    }

    let escaped = String::from_utf8(bytes)
        .expect("fixture JSON is UTF-8")
        .replacen("iVBOR", "\\u0069VBOR", 1);
    assert!(matches!(
        merman_display_list::DrawingListDocument::from_json_bytes_with_limits(
            escaped.as_bytes(),
            &exact
        ),
        Err(DrawingListError::JsonDecode(message))
            if message.contains("must not use JSON escapes")
    ));
}

#[test]
fn decoder_stops_at_wire_collection_and_nested_sequence_limits() {
    let cases = [
        (
            "resources",
            br#"{"resources":[{"kind":"pattern"},{"kind":"pattern"},{"broken":}]}"#
                .as_slice(),
            DrawingListLimits {
                max_resources: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "commands",
            br#"{"commands":[{"kind":"save"},{"kind":"save"},{"broken":}]}"#.as_slice(),
            DrawingListLimits {
                max_commands: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "fallbacks",
            br#"{"fallbacks":[{}, {}, {"broken":}]}"#.as_slice(),
            DrawingListLimits {
                max_fallbacks: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "path_segments",
            br#"{"resources":[{"kind":"path","segments":[{},{}]}]}"#.as_slice(),
            DrawingListLimits {
                max_path_segments: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "stroke_dash_entries",
            br#"{"commands":[{"kind":"draw_path","style":{"stroke":{"dash_array":[1]}}},{"kind":"draw_text","run":{"text":"","style":{"stroke":{"dash_array":[2]}}}},{"broken":}]}"#
                .as_slice(),
            DrawingListLimits {
                max_stroke_dash_entries: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "glyphs",
            br#"{"commands":[{"kind":"draw_text","run":{"text":"","obligation":{"kind":"glyph_run","glyphs":[{},{}]}}}]}"#
                .as_slice(),
            DrawingListLimits {
                max_glyphs: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "text_bytes",
            br#"{"commands":[{"kind":"draw_text","run":{"text":"a"}},{"kind":"draw_text","run":{"text":"b"}},{"broken":}]}"#
                .as_slice(),
            DrawingListLimits {
                max_text_bytes: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "stroke_dash_entries",
            br#"{"commands":[["draw_text",{"text":"","style":{"stroke":{"dash_array":[1]}}}],["draw_text",{"text":"","style":{"stroke":{"dash_array":[2]}}}],{"broken":}]}"#
                .as_slice(),
            DrawingListLimits {
                max_stroke_dash_entries: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "glyphs",
            br#"{"commands":[["draw_text",{"text":"","obligation":{"kind":"glyph_run","glyphs":[{},{}]}}],{"broken":}]}"#
                .as_slice(),
            DrawingListLimits {
                max_glyphs: 1,
                ..DrawingListLimits::default()
            },
        ),
        (
            "text_bytes",
            br#"{"commands":[["draw_text",{"text":"a"}],["draw_text",{"text":"b"}],{"broken":}]}"#
                .as_slice(),
            DrawingListLimits {
                max_text_bytes: 1,
                ..DrawingListLimits::default()
            },
        ),
    ];

    for (resource, bytes, limits) in cases {
        assert!(matches!(
            merman_display_list::DrawingListDocument::from_json_bytes_with_limits(bytes, &limits),
            Err(DrawingListError::ResourceLimit {
                resource: actual_resource,
                actual: 2,
                maximum: 1,
            }) if actual_resource == resource
        ));
    }
}

#[test]
fn decoder_charges_json_escaped_text_before_owned_document_decode() {
    let mut bytes = String::from(r#"{"commands":[{"kind":"draw_text","run":{"text":""#);
    bytes.push_str(&r"\u0061".repeat(4096));
    bytes.push_str(r#""}},{"broken":}]}"#);
    let limits = DrawingListLimits {
        max_text_bytes: 4095,
        ..DrawingListLimits::default()
    };

    assert!(matches!(
        merman_display_list::DrawingListDocument::from_json_bytes_with_limits(
            bytes.as_bytes(),
            &limits
        ),
        Err(DrawingListError::ResourceLimit {
            resource: "text_bytes",
            actual: 4096,
            maximum: 4095,
        })
    ));
}

#[test]
fn decoder_accepts_json_escaped_text_at_its_decoded_utf8_budget() {
    let mut document = sample_document();
    let DrawingCommand::DrawText { run } = &mut document.commands[5] else {
        panic!("sample document command 5 is text");
    };
    run.text = "a😀".to_string();
    let bytes = serde_json::to_string(&document)
        .expect("fixture JSON")
        .replacen("a😀", r"\u0061\uD83D\uDE00", 1);
    let limits = DrawingListLimits {
        max_text_bytes: 5,
        ..DrawingListLimits::default()
    };

    let decoded = merman_display_list::DrawingListDocument::from_json_bytes_with_limits(
        bytes.as_bytes(),
        &limits,
    )
    .expect("escaped text fits its decoded UTF-8 budget");
    let DrawingCommand::DrawText { run } = &decoded.commands[5] else {
        panic!("decoded command 5 is text");
    };
    assert_eq!(run.text, "a😀");
}

#[test]
fn encoded_assets_require_canonical_base64_pad_bits() {
    let canonical: EncodedAsset =
        serde_json::from_str(r#"{"media_type":"font/woff2","data":"Zg=="}"#)
            .expect("canonical Base64 decodes");
    assert_eq!(canonical.data, b"f");
    assert_eq!(
        serde_json::to_string(&canonical).expect("canonical Base64 serializes"),
        r#"{"media_type":"font/woff2","data":"Zg=="}"#
    );

    for non_canonical in ["Zh==", "Zm9="] {
        let json = format!(r#"{{"media_type":"font/woff2","data":"{non_canonical}"}}"#);
        let error = serde_json::from_str::<EncodedAsset>(&json)
            .expect_err("non-zero Base64 pad bits must be rejected");
        assert!(error.to_string().contains("not canonical Base64"));
    }
}

#[test]
fn validator_enforces_cumulative_encoded_asset_byte_budgets() {
    let mut document = extended_document();
    let image_data = png_1x1();
    document
        .resources
        .push(DrawingResource::Image(ImageResource {
            id: ResourceId::new("image.pattern-tile-2"),
            image: EncodedImage::new("image/png", image_data.clone()),
            pixel_width: 1,
            pixel_height: 1,
            has_alpha: false,
        }));
    document.resources.push(DrawingResource::Font(FontResource {
        id: ResourceId::new("font.fixture-2"),
        font: EncodedAsset::new("font/woff2", vec![0x77, 0x4f, 0x46, 0x32]),
    }));

    let limits = DrawingListLimits {
        max_image_bytes: image_data.len() + 1,
        max_font_bytes: 5,
        ..DrawingListLimits::default()
    };
    let error = document
        .validate_with_limits(&limits)
        .expect_err("two individually valid assets must not bypass the document budget");
    assert!(error.to_string().contains("image_bytes"));

    let mut font_only = sample_document();
    font_only
        .resources
        .push(DrawingResource::Font(FontResource {
            id: ResourceId::new("font.fixture-1"),
            font: EncodedAsset::new("font/woff2", vec![0x77, 0x4f, 0x46, 0x32]),
        }));
    font_only
        .resources
        .push(DrawingResource::Font(FontResource {
            id: ResourceId::new("font.fixture-2"),
            font: EncodedAsset::new("font/woff2", vec![0x77, 0x4f, 0x46, 0x32]),
        }));
    let error = font_only
        .validate_with_limits(&limits)
        .expect_err("font resources must share one document-wide byte budget");
    assert!(error.to_string().contains("font_bytes"));
}
