#![no_main]

use libfuzzer_sys::fuzz_target;
use merman::{Engine, ParseOptions};
use merman_fuzz::{MAX_PARSE_INPUT_BYTES, bounded_utf8, deterministic_engine};

thread_local! {
    static ENGINE: Engine = deterministic_engine();
}

fuzz_target!(|data: &[u8]| {
    let Some(source) = bounded_utf8(data, MAX_PARSE_INPUT_BYTES) else {
        return;
    };

    ENGINE.with(|engine| {
        let semantic = engine.parse_diagram_sync(source, ParseOptions::strict());
        let render_model =
            engine.parse_diagram_for_render_model_sync(source, ParseOptions::strict());

        if let (Ok(Some(semantic)), Ok(Some(render_model))) = (&semantic, &render_model) {
            assert_eq!(
                semantic.meta.diagram_type, render_model.meta.diagram_type,
                "semantic and render-model parsers selected different diagram types"
            );
            assert_eq!(
                semantic.meta.title, render_model.meta.title,
                "semantic and render-model parsers retained different titles"
            );
        }

        let _ = engine.parse_diagram_sync(source, ParseOptions::lenient());
    });
});
