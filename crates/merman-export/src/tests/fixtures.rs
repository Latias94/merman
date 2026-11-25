#[cfg(feature = "pdf")]
use super::PdfOptions;
#[cfg(any(feature = "png", feature = "jpeg"))]
use super::RasterOptions;
#[cfg(feature = "native-fonts")]
use super::{
    NativeFontContext, NativeFontLimits, OperationControl, system_font_discovery_available,
};

#[cfg(feature = "native-fonts")]
fn fixture_native_font_context() -> NativeFontContext {
    NativeFontContext::from_font_bytes(
        [include_bytes!("../../tests/fonts/Cousine-Regular.ttf").to_vec()],
        NativeFontLimits::default(),
        &OperationControl::new(),
    )
    .expect("explicit export test font")
}

#[cfg(any(feature = "png", feature = "jpeg"))]
pub(super) fn fixture_raster_options() -> RasterOptions {
    #[cfg(feature = "native-fonts")]
    if !system_font_discovery_available() {
        return RasterOptions::default().with_font_context(fixture_native_font_context());
    }
    RasterOptions::default()
}

#[cfg(feature = "pdf")]
pub(super) fn fixture_pdf_options() -> PdfOptions {
    #[cfg(feature = "native-fonts")]
    if !system_font_discovery_available() {
        return PdfOptions::default().with_font_context(fixture_native_font_context());
    }
    PdfOptions::default()
}
