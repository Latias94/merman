use super::*;
use crate::test_fixtures::fixture_pdf_options;

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
fn pdf_leaf_encodes_a_sealed_svg() {
    let bytes = svg_to_pdf_with_options(&compatible_svg(), &fixture_pdf_options())
        .expect("PDF export should be callable when its leaf is enabled");

    assert!(bytes.starts_with(b"%PDF-"));
}

#[test]
fn pre_cancelled_pdf_returns_no_prepared_artifact_or_bytes() {
    let svg = compatible_svg();
    let control = OperationControl::new();
    control.cancel();

    let prepared_error = match prepare_pdf_controlled(&svg, &fixture_pdf_options(), control.clone())
    {
        Ok(_) => panic!("pre-cancelled PDF preparation must fail"),
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

    let bytes_error = svg_to_pdf_controlled(&svg, &fixture_pdf_options(), control)
        .expect_err("pre-cancelled PDF encoding must fail");
    assert!(matches!(
        bytes_error,
        ExportError::Cancelled(OperationCancelled {
            phase: OperationPhase::Export,
            ..
        })
    ));
}
