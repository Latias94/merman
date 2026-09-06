mod common;

use common::{png_1x1, sample_document};
use merman_display_list::{
    AlphaMode, DrawingCommand, DrawingListDocument, DrawingListPolicy, DrawingResource,
    EncodedImage, FallbackReason, ImageResource, RasterFallback, RasterFormat, Rect, ResourceId,
    VisualSource,
};
use serde_json::json;

#[test]
fn vector_only_rejects_required_raster_fallback_before_output() {
    let mut document = document_with_raster_fallback();
    document.policy = DrawingListPolicy::VectorOnly;
    assert!(document.validate().is_err());
    assert!(document.canonical_json_bytes().is_err());
}

#[test]
fn raster_fallback_requires_complete_accountability_and_a_command_reference() {
    let mut document = document_with_raster_fallback();
    document.fallbacks[0].source.family.clear();
    assert!(document.validate().is_err());

    let mut document = document_with_raster_fallback();
    document
        .commands
        .retain(|command| !matches!(command, DrawingCommand::DrawRasterSubtree { .. }));
    assert!(document.validate().is_err());
}

#[test]
fn unknown_visual_commands_fail_closed_but_metadata_extensions_round_trip() {
    let document = sample_document();
    let mut value = serde_json::to_value(&document).unwrap();
    value["commands"][0] = json!({ "kind": "future_visual_command" });
    assert!(DrawingListDocument::from_json_bytes(&serde_json::to_vec(&value).unwrap()).is_err());

    let mut document = sample_document();
    document.extensions.insert(
        "x-producer-trace".into(),
        json!({ "build": "fixture", "visual": false }),
    );
    let bytes = document.canonical_json_bytes().unwrap();
    let decoded = DrawingListDocument::from_json_bytes(&bytes).unwrap();
    assert_eq!(
        decoded.extensions["x-producer-trace"],
        document.extensions["x-producer-trace"]
    );
}

fn document_with_raster_fallback() -> DrawingListDocument {
    let mut document = sample_document();
    let image = ResourceId::new("image.foreign-object");
    document
        .resources
        .push(DrawingResource::Image(ImageResource {
            id: image.clone(),
            image: EncodedImage::new("image/png", png_1x1()),
            pixel_width: 1,
            pixel_height: 1,
            has_alpha: true,
        }));
    document.fallbacks.push(RasterFallback {
        id: "fallback.foreign-object".into(),
        image,
        bounds: Rect::new(24.0, 28.0, 1.0, 1.0),
        pixel_width: 1,
        pixel_height: 1,
        scale: 1.0,
        format: RasterFormat::Png,
        alpha: AlphaMode::Straight,
        reason: FallbackReason::ForeignObject,
        source: VisualSource {
            family: "flowchart".into(),
            element_id: Some("node.a".into()),
            effect: "foreign_object".into(),
        },
    });
    document.commands.insert(
        5,
        DrawingCommand::DrawRasterSubtree {
            fallback_id: "fallback.foreign-object".into(),
        },
    );
    document
}
