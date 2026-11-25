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
fn jpeg_leaf_encodes_a_sealed_svg() {
    let bytes = svg_to_jpeg(&compatible_svg(), &fixture_raster_options())
        .expect("JPEG export should be callable when its leaf is enabled");

    assert!(bytes.starts_with(b"\xff\xd8\xff"));
}

#[test]
fn pre_cancelled_jpeg_returns_no_bytes() {
    let svg = compatible_svg();
    let control = OperationControl::new();
    control.cancel();

    let error = svg_to_jpeg_controlled(&svg, &fixture_raster_options(), control)
        .expect_err("pre-cancelled JPEG encoding must fail");
    assert!(matches!(
        error,
        ExportError::Cancelled(OperationCancelled {
            phase: OperationPhase::Export,
            ..
        })
    ));
    assert!(error.resource_limit_details().is_none());
}
