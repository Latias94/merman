#![cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]

#[test]
fn legacy_codecs_preserve_native_system_fonts_without_the_native_fonts_feature() {
    assert!(cfg!(feature = "system-fonts"));
    assert_eq!(
        merman_export::system_font_discovery_available(),
        cfg!(not(target_family = "wasm"))
    );
    for output in [
        #[cfg(feature = "png")]
        "png",
        #[cfg(feature = "jpeg")]
        "jpeg",
        #[cfg(feature = "pdf")]
        "pdf",
    ] {
        let contract = merman_export::output_environment_contract(output).unwrap();
        assert_eq!(
            contract.system_fonts.is_some(),
            cfg!(not(target_family = "wasm"))
        );
    }
}
