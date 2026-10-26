#![no_main]

use libfuzzer_sys::fuzz_target;
use merman::{MermaidEngine, diagram_family_capabilities};
use merman_fuzz::{
    MAX_PARSE_INPUT_BYTES, assert_known_type_parse_paths, assert_parse_paths, bounded_utf8,
    deterministic_engine,
};

thread_local! {
    static ENGINE: MermaidEngine = deterministic_engine();
}

fuzz_target!(|data: &[u8]| {
    let Some(source) = bounded_utf8(data, MAX_PARSE_INPUT_BYTES) else {
        return;
    };

    ENGINE.with(|engine| {
        assert_parse_paths(engine, source);
        let families = diagram_family_capabilities();
        if !families.is_empty() {
            let selector = data.iter().take(8).fold(0usize, |value, byte| {
                value.wrapping_mul(31).wrapping_add(usize::from(*byte))
            });
            let family = &families[selector % families.len()];
            assert_known_type_parse_paths(engine, family.diagram_type, source);
        }
    });
});
