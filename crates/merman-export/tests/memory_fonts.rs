#![cfg(feature = "native-fonts")]

use merman_core::OperationControl;
#[cfg(target_family = "wasm")]
use merman_export::NativeFontOptions;
use merman_export::{NativeFontContext, NativeFontLimits};
use merman_render::environment::{
    HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer, TextMeasurementOperation,
    TextMeasurementPhase,
};
use merman_render::text::{TextStyle, WrapMode};

fn context(control: &OperationControl) -> NativeFontContext {
    NativeFontContext::from_font_bytes(
        [include_bytes!("fonts/Cousine-Regular.ttf").to_vec()],
        NativeFontLimits::default(),
        control,
    )
    .expect("owned font data is usable on native and WebAssembly targets")
}

#[test]
fn owned_font_measurement_needs_no_host_acquisition_on_any_target() {
    let fonts = context(&OperationControl::new());
    assert_eq!(fonts.face_count(), 1);
    assert!(!fonts.uses_system_fonts());
    let style = TextStyle {
        font_family: Some("Cousine".to_owned()),
        font_size: 22.0,
        ..Default::default()
    };
    let measured = fonts
        .measure(HostTextMeasurementRequest {
            operation: TextMeasurementOperation::ComputedLength,
            phase: TextMeasurementPhase::SvgBBox,
            text: "Memory fonts",
            style: &style,
            max_width: None,
            wrap_mode: WrapMode::SvgLike,
        })
        .unwrap();
    assert!(matches!(measured, Some(HostTextMeasurement::Length(width)) if width > 0.0));
    assert_eq!(
        merman_export::system_font_discovery_available(),
        cfg!(all(feature = "system-fonts", not(target_family = "wasm")))
    );
}

#[test]
fn operation_and_export_forks_do_not_inherit_loading_cancellation() {
    let loading = OperationControl::new();
    let fonts = context(&loading);
    loading.cancel();
    assert!(fonts.check().is_err());
    fonts
        .for_operation(&OperationControl::new())
        .check()
        .unwrap();
    fonts
        .for_export(&OperationControl::new())
        .unwrap()
        .check()
        .unwrap();
}

#[cfg(target_family = "wasm")]
#[test]
fn wasm_rejects_path_and_system_acquisition_even_with_unified_codec_features() {
    use merman_export::NativeFontError;

    for options in [
        NativeFontOptions::default().with_font_file("not-opened.ttf"),
        NativeFontOptions::default().with_font_dir("not-enumerated"),
    ] {
        let error = options.load(&OperationControl::new()).unwrap_err();
        assert!(matches!(error, NativeFontError::Source { message, .. }
            if message.contains("provide font bytes")));
    }
    assert!(matches!(
        NativeFontOptions::default()
            .with_system_fonts(true)
            .load(&OperationControl::new()),
        Err(NativeFontError::SystemDiscoveryUnavailable)
    ));
    assert!(!merman_export::system_font_discovery_available());
}

#[cfg(feature = "bundled-fonts")]
#[test]
fn bundled_production_fonts_load_on_any_target_without_system_discovery() {
    let fonts = NativeFontContext::from_font_bytes(
        merman_export::bundled_font_data(),
        Default::default(),
        &OperationControl::new(),
    )
    .unwrap();
    assert_eq!(fonts.face_count(), 3);
    assert!(!fonts.uses_system_fonts());
}
