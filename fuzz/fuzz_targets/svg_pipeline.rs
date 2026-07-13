#![no_main]

use libfuzzer_sys::fuzz_target;
use merman::render::svg_resvg_safe;
use merman_fuzz::{MAX_SVG_INPUT_BYTES, assert_resvg_safe_svg, bounded_utf8, is_well_formed_svg};

fuzz_target!(|data: &[u8]| {
    let Some(svg) = bounded_utf8(data, MAX_SVG_INPUT_BYTES) else {
        return;
    };
    let input_is_well_formed_svg = is_well_formed_svg(svg);

    if let Ok(sanitized) = svg_resvg_safe(svg)
        && input_is_well_formed_svg
    {
        assert_resvg_safe_svg(&sanitized);
    }
});
