#![no_main]

use libfuzzer_sys::fuzz_target;
use merman_render::diagram_theme::{
    FontAssetSpec, FontCatalog, FontCatalogSpec, FontSourcePolicy, FontStack, FontStyle,
    ThemeResourcePolicy, ThemeTextStyle,
};
use merman_render::text::{
    NativeTextLayoutBackend, PrepareCatalogRequest, PrepareTextRequest, PreparedTextWrap,
    TextLayoutDirection,
};
use std::sync::OnceLock;

const MAX_INPUT_BYTES: usize = 16 * 1024;
const MAX_TEXT_BYTES: usize = 8 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() < 3 || data.len() > MAX_INPUT_BYTES {
        return;
    }

    let direction = match data[0] % 3 {
        0 => TextLayoutDirection::Auto,
        1 => TextLayoutDirection::LeftToRight,
        _ => TextLayoutDirection::RightToLeft,
    };
    let features = feature_set(data[1]);
    let variation = format!("wght={}", 100 + u16::from(data[2]) * 3);
    let text_bytes = &data[3..data.len().min(3 + MAX_TEXT_BYTES)];
    let text = String::from_utf8_lossy(text_bytes);

    let catalog_request =
        PrepareCatalogRequest::new(catalog().clone(), FontSourcePolicy::embedded_only());
    let backend = NativeTextLayoutBackend::default();

    let typography = ThemeTextStyle::default()
        .with_font_stack(
            FontStack::new(["Excalifont", "Xiaolai"]).expect("fixed fuzz font stack is valid"),
        )
        .with_font_size_px(8.0 + f32::from(data[0] % 64))
        .expect("bounded fuzz font size is valid")
        .with_font_weight(100 + u16::from(data[2]) * 3)
        .expect("bounded fuzz font weight is valid")
        .with_font_style(if data[1] & 0x80 == 0 {
            FontStyle::Normal
        } else {
            FontStyle::Italic
        });
    let request = PrepareTextRequest::for_fuzz_probe(
        text.as_ref(),
        typography,
        direction,
        features,
        [variation],
    )
    .expect("bounded fuzz request is valid");
    let Some(single_run) = backend
        .prepare_text_probe_for_fuzz(
            &catalog_request,
            &request.clone().with_wrap(PreparedTextWrap::SingleRun),
        )
        .expect("bounded single-run request should prepare")
    else {
        return;
    };
    let max_width = 1.0 + f64::from(data[1]) * 4.0;
    let Some(wrapped) = backend
        .prepare_text_probe_for_fuzz(
            &catalog_request,
            &request.with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(max_width),
                break_long_words: true,
            }),
        )
        .expect("bounded wrapped request should prepare")
    else {
        return;
    };

    for value in [
        single_run.metrics().width,
        single_run.metrics().height,
        single_run.bbox_width_px(),
        single_run.bbox_height_px(),
        wrapped.metrics().width,
        wrapped.metrics().height,
        wrapped.bbox_width_px(),
        wrapped.bbox_height_px(),
    ] {
        assert!(value.is_finite() && value >= 0.0);
    }
    assert!(single_run.metrics().line_count >= 1);
    assert!(wrapped.metrics().line_count >= 1);
    assert!(wrapped.metrics().line_count <= text.chars().count().saturating_add(1).max(1));
});

fn feature_set(selector: u8) -> Vec<&'static str> {
    let mut features = Vec::new();
    for (mask, feature) in [
        (0x01, "kern"),
        (0x02, "liga"),
        (0x04, "clig"),
        (0x08, "calt"),
    ] {
        if selector & mask != 0 {
            features.push(feature);
        }
    }
    features
}

fn catalog() -> &'static FontCatalog {
    static CATALOG: OnceLock<FontCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let latin = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let cjk = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
        ));
        FontCatalogSpec::new([
            FontAssetSpec::new("excalifont", latin),
            FontAssetSpec::new("xiaolai", cjk),
        ])
        .with_alias("Xiaolai", "Xiaolai SC")
        .compile(&ThemeResourcePolicy::interactive())
        .expect("committed shaping catalog should compile")
    })
}
