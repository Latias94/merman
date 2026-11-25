use super::*;
use crate::test_fixtures::fixture_raster_options;
use merman_model::{OperationCancelled, OperationPhase};

fn compatible_svg() -> ResvgCompatibleSvg {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic render session");
    merman_render::svg::finalize_resvg_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="black"/></svg>"#,
            &session,
        )
        .expect("sealed SVG")
}

#[test]
fn translucent_svg_paint_keeps_native_png_channels() {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><rect width="16" height="16" fill="#FF0000" fill-opacity="0.5"/></svg>"##;
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic color render session");
    let svg = merman_render::svg::finalize_resvg_svg(source, &session)
        .expect("seal SVG with translucent fill");
    let bytes = svg_to_png(&svg, &fixture_raster_options()).expect("export SVG as PNG");
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("read color PNG");
    let mut pixels = vec![0; reader.output_buffer_size().expect("PNG output size")];
    let info = reader.next_frame(&mut pixels).expect("decode color PNG");
    assert_eq!((info.width, info.height), (16, 16));
    assert_eq!(info.color_type, png::ColorType::Rgba);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    let center = ((8 * info.width + 8) * 4) as usize;
    assert_eq!(&pixels[center..center + 4], &[255, 0, 0, 128]);
}

#[test]
fn png_leaf_encodes_a_sealed_svg() {
    let bytes = svg_to_png(&compatible_svg(), &fixture_raster_options())
        .expect("PNG export should be callable when its leaf is enabled");

    assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
}

#[test]
fn controlled_png_preserves_successful_bytes() {
    let svg = compatible_svg();
    let legacy = svg_to_png(&svg, &fixture_raster_options()).expect("legacy PNG export");
    let controlled =
        svg_to_png_controlled(&svg, &fixture_raster_options(), OperationControl::new())
            .expect("controlled PNG export");

    assert_eq!(controlled, legacy);
}

#[test]
fn pre_cancelled_png_returns_no_prepared_artifact_or_bytes() {
    let svg = compatible_svg();
    let control = OperationControl::new();
    control.cancel();

    let prepared_error =
        match prepare_raster_controlled(&svg, &fixture_raster_options(), control.clone()) {
            Ok(_) => panic!("pre-cancelled preparation must fail"),
            Err(error) => error,
        };
    assert!(matches!(
        prepared_error,
        ExportError::Cancelled(OperationCancelled {
            phase: OperationPhase::Export,
            ..
        })
    ));
    assert!(prepared_error.resource_limit_details().is_none());

    let bytes_error = svg_to_png_controlled(&svg, &fixture_raster_options(), control)
        .expect_err("pre-cancelled encoding must fail");
    assert!(matches!(
        bytes_error,
        ExportError::Cancelled(OperationCancelled {
            phase: OperationPhase::Export,
            ..
        })
    ));
}

#[test]
fn cancellation_after_preparation_returns_no_png_bytes() {
    let svg = compatible_svg();
    let control = OperationControl::new();
    let prepared = prepare_raster_controlled(&svg, &fixture_raster_options(), control.clone())
        .expect("preparation should succeed before cancellation");
    control.cancel();

    let error = prepared
        .encode_png()
        .expect_err("cancelled encoding must not return bytes");
    assert!(matches!(
        error,
        ExportError::Cancelled(OperationCancelled {
            phase: OperationPhase::Export,
            ..
        })
    ));
    assert!(error.resource_limit_details().is_none());
}
