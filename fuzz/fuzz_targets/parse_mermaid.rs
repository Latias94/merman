#![no_main]

use libfuzzer_sys::fuzz_target;
use merman::Engine;
use merman_fuzz::{MAX_PARSE_INPUT_BYTES, assert_parse_paths, bounded_utf8, deterministic_engine};

thread_local! {
    static ENGINE: Engine = deterministic_engine();
}

fuzz_target!(|data: &[u8]| {
    let Some(source) = bounded_utf8(data, MAX_PARSE_INPUT_BYTES) else {
        return;
    };

    ENGINE.with(|engine| assert_parse_paths(engine, source));
});
