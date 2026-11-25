use super::*;

fn sealed_svg() -> ResvgCompatibleSvg {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    merman_render::svg::finalize_resvg_svg(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40"><text x="4" y="24">No implicit fonts</text></svg>"#,
            &session,
        )
        .unwrap()
}

#[test]
fn catalogs_do_not_advertise_uncompiled_system_discovery() {
    assert!(!system_font_discovery_available());
    for format in ["png", "jpeg", "pdf"] {
        if let Some(contract) = output_environment_contract(format) {
            assert!(contract.system_fonts.is_none());
        }
    }
}

#[cfg(feature = "png")]
#[test]
fn png_without_context_returns_font_source_unavailable() {
    assert!(matches!(
        svg_to_png(&sealed_svg(), &RasterOptions::default()),
        Err(ExportError::FontSourceUnavailable)
    ));
}

#[cfg(feature = "jpeg")]
#[test]
fn jpeg_without_context_returns_font_source_unavailable() {
    assert!(matches!(
        svg_to_jpeg(&sealed_svg(), &RasterOptions::default()),
        Err(ExportError::FontSourceUnavailable)
    ));
}

#[cfg(feature = "pdf")]
#[test]
fn pdf_without_context_returns_font_source_unavailable() {
    assert!(matches!(
        svg_to_pdf(&sealed_svg()),
        Err(ExportError::FontSourceUnavailable)
    ));
}
