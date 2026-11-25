use super::*;

fn sealed_svg(text: &str) -> ResvgCompatibleSvg {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .with_font_source_policy(merman_render::text::FontSourcePolicy::embedded_only())
        .begin_session()
        .unwrap();
    let source = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40"><defs><g id="space"> </g></defs><text x="4" y="24" text-decoration="underline">{text}</text></svg>"#
    );
    merman_render::svg::finalize_resvg_svg(&source, &session).unwrap()
}

const TRIMMED: &str = r##"<tref xml:space="preserve" href="#space"/><tref href="#space"/>"##;
const SURVIVING: &str = r##"<tref href="#space"/><tref href="#space"/><tspan><tref href="#space"/></tspan><tref href="#space"/>"##;

#[test]
fn retained_policy_only_requires_fonts_for_converted_svg_text() {
    let blank = sealed_svg("");
    let trimmed = sealed_svg(TRIMMED);
    let surviving = sealed_svg(SURVIVING);
    assert_eq!(trimmed.finalization_report().text_element_count(), 1);
    assert_eq!(surviving.finalization_report().text_element_count(), 1);
    assert!(
        !merman_render::svg::StandaloneSvgArtifact::from(trimmed.clone())
            .text_fonts_are_self_contained()
    );

    type Encode = fn(&ResvgCompatibleSvg) -> Result<Vec<u8>>;
    let encoders: &[(&str, Encode)] = &[
        #[cfg(feature = "png")]
        ("png", |svg| svg_to_png(svg, &RasterOptions::default())),
        #[cfg(feature = "jpeg")]
        ("jpeg", |svg| svg_to_jpeg(svg, &RasterOptions::default())),
        #[cfg(feature = "pdf")]
        ("pdf", svg_to_pdf),
    ];
    for (format, encode) in encoders {
        assert_eq!(
            encode(&trimmed).unwrap(),
            encode(&blank).unwrap(),
            "{format}"
        );
        assert!(
            matches!(encode(&surviving), Err(ExportError::FontSourceUnavailable)),
            "{format}"
        );
    }
}

#[cfg(all(feature = "native-fonts", any(feature = "png", feature = "jpeg")))]
#[test]
fn cancelling_at_the_deferred_svg_font_loader_prevents_preparation() {
    let control = OperationControl::new();
    let result = prepare_raster_with_font_loader_controlled(
        &sealed_svg(SURVIVING),
        &RasterOptions::default(),
        control.clone(),
        |needs_fonts, control| {
            assert!(needs_fonts);
            control.cancel();
            Ok(None)
        },
    );
    assert!(matches!(result, Err(ExportError::Cancelled(_))));
}
