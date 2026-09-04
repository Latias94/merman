mod common;

use common::{extended_document, sample_document};
use merman_display_list::{
    AlphaMode, DrawingCommand, DrawingListLimits, DrawingResource, EncodedImage, FallbackReason,
    FillRule, ImageResource, Paint, PathSegment, Point, RasterFallback, RasterFormat, Rect,
    ResourceId, VisualSource,
};

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
    let mut fallback_commands = Vec::with_capacity(fallback_count);
    for index in 0..fallback_count {
        let image = ResourceId::new(format!("image.fallback.{index:04}"));
        let fallback_id = format!("fallback.{index:04}");
        document
            .resources
            .push(DrawingResource::Image(ImageResource {
                id: image.clone(),
                image: EncodedImage::new("image/png", vec![0x89, b'P', b'N', b'G']),
                pixel_width: 1,
                pixel_height: 1,
                has_alpha: true,
            }));
        document.fallbacks.push(RasterFallback {
            id: fallback_id.clone(),
            image,
            bounds: Rect::new(0.0, 0.0, 1.0, 1.0),
            pixel_width: 1,
            pixel_height: 1,
            scale: 1.0,
            format: RasterFormat::Png,
            alpha: AlphaMode::Straight,
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
        .media_type = "IMAGE/PNG; charset=binary".into();
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
}
