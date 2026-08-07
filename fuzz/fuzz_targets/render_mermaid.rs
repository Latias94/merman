#![no_main]

use libfuzzer_sys::fuzz_target;
use merman::svg::HeadlessRenderer;
use merman_fuzz::{MAX_RENDER_INPUT_BYTES, assert_resvg_safe_svg, bounded_renderer, bounded_utf8};

thread_local! {
    static RENDERER: HeadlessRenderer = bounded_renderer();
}

fuzz_target!(|data: &[u8]| {
    let Some(source) = bounded_utf8(data, MAX_RENDER_INPUT_BYTES) else {
        return;
    };

    RENDERER.with(|renderer| {
        if let Ok(Some(svg)) = renderer.render_resvg_compatible_svg_sync(source) {
            assert_resvg_safe_svg(svg.as_ref());
        }
    });
});
