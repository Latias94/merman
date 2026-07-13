#![no_main]

use libfuzzer_sys::fuzz_target;
use merman_ffi::{
    MermanBuffer, MermanEngine, MermanEngineResult, MermanHostTextMeasureRequest,
    MermanHostTextMeasureResult, MermanResult, merman_abi_version,
    merman_analyze_document_facts_json, merman_analyze_document_json, merman_analyze_json,
    merman_ascii_capabilities_json, merman_buffer_free, merman_buffer_struct_size,
    merman_diagram_family_capabilities_json, merman_engine_analyze_document_facts_json,
    merman_engine_analyze_document_json, merman_engine_analyze_json, merman_engine_free,
    merman_engine_layout_json, merman_engine_new, merman_engine_parse_json,
    merman_engine_render_ascii, merman_engine_render_svg, merman_engine_result_struct_size,
    merman_engine_set_text_measure_callback, merman_engine_validate_json,
    merman_host_text_measure_request_struct_size, merman_host_text_measure_result_struct_size,
    merman_layout_json, merman_lint_rule_catalog_json, merman_package_version, merman_parse_json,
    merman_render_ascii, merman_render_svg, merman_result_struct_size,
    merman_supported_diagrams_json, merman_supported_host_theme_presets_json,
    merman_supported_themes_json, merman_validate_json,
};
use std::ptr;

const MAX_FFI_INPUT_BYTES: usize = 16 * 1024;
const MAX_OPTIONS_BYTES: usize = 256;
const MERMAN_OK: i32 = 0;
const MERMAN_PANIC: i32 = 8;

const FIXED_OPTIONS: &[u8] = br#"{"fixed_today":"2025-01-01","fixed_local_offset_minutes":0,"resources":{"max_source_bytes":16384,"max_svg_bytes":1048576,"max_flowchart_nodes":256,"max_flowchart_edges":512,"max_flowchart_subgraphs":64,"max_class_nodes":256,"max_class_edges":512,"max_class_namespaces":64,"max_label_bytes":65536}}"#;
const MALFORMED_OPTIONS: &[u8] = br#"{"resources":{"max_source_bytes":"bad"}}"#;
const DEFAULT_URI: &[u8] = b"file:///fuzz.mmd";

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_FFI_INPUT_BYTES {
        return;
    }

    let input = decode_input(data);

    unsafe {
        match input.selector % 18 {
            0 => consume_result(merman_parse_json(
                input.source.as_ptr(),
                input.source.len(),
                options_ptr(input.options),
                input.options.len(),
            )),
            1 => consume_result(merman_validate_json(
                input.source.as_ptr(),
                input.source.len(),
                options_ptr(input.options),
                input.options.len(),
            )),
            2 => consume_result(merman_analyze_json(
                input.source.as_ptr(),
                input.source.len(),
                options_ptr(input.options),
                input.options.len(),
            )),
            3 => consume_result(merman_layout_json(
                input.source.as_ptr(),
                input.source.len(),
                options_ptr(input.options),
                input.options.len(),
            )),
            4 => consume_result(merman_render_svg(
                input.source.as_ptr(),
                input.source.len(),
                options_ptr(input.options),
                input.options.len(),
            )),
            5 => consume_result(merman_render_ascii(
                input.source.as_ptr(),
                input.source.len(),
                options_ptr(input.options),
                input.options.len(),
            )),
            6 => consume_result(merman_analyze_document_json(
                input.source.as_ptr(),
                input.source.len(),
                options_ptr(input.options),
                input.options.len(),
                input.uri.as_ptr(),
                input.uri.len(),
            )),
            7 => consume_result(merman_analyze_document_facts_json(
                input.source.as_ptr(),
                input.source.len(),
                options_ptr(input.options),
                input.options.len(),
                input.uri.as_ptr(),
                input.uri.len(),
            )),
            8 => call_engine_source(input.source, input.options, merman_engine_parse_json),
            9 => call_engine_source(input.source, input.options, merman_engine_layout_json),
            10 => call_engine_source(input.source, input.options, merman_engine_analyze_json),
            11 => call_engine_document(
                input.source,
                input.options,
                input.uri,
                merman_engine_analyze_document_json,
            ),
            12 => call_engine_document(
                input.source,
                input.options,
                input.uri,
                merman_engine_analyze_document_facts_json,
            ),
            13 => call_engine_source(input.source, input.options, merman_engine_render_svg),
            14 => call_engine_source(input.source, input.options, merman_engine_render_ascii),
            15 => call_engine_source(input.source, input.options, merman_engine_validate_json),
            16 => call_engine_with_text_callback(input.source, input.options),
            _ => call_metadata_exports(),
        }
    }
});

struct FuzzInput<'a> {
    selector: u8,
    source: &'a [u8],
    options: &'a [u8],
    uri: &'a [u8],
}

fn decode_input(data: &[u8]) -> FuzzInput<'_> {
    if let Some(source) = data.strip_prefix(b"parse\n") {
        return text_seed(0, source, &[]);
    }
    if let Some(source) = data.strip_prefix(b"render-fixed-options\n") {
        return text_seed(4, source, FIXED_OPTIONS);
    }
    if let Some(source) = data.strip_prefix(b"invalid-options\n") {
        return text_seed(0, source, MALFORMED_OPTIONS);
    }

    let Some((&selector, source)) = data.split_first() else {
        return text_seed(17, &[], &[]);
    };

    let Some((&options_len, framed)) = source.split_first() else {
        return text_seed(selector, &[], &[]);
    };
    let options_len = usize::from(options_len)
        .min(MAX_OPTIONS_BYTES)
        .min(framed.len());
    let (options, framed) = framed.split_at(options_len);
    let (uri, source) = split_uri(selector, framed);

    FuzzInput {
        selector,
        source,
        options,
        uri,
    }
}

fn text_seed<'a>(selector: u8, source: &'a [u8], options: &'a [u8]) -> FuzzInput<'a> {
    FuzzInput {
        selector,
        source,
        options,
        uri: DEFAULT_URI,
    }
}

fn split_uri(selector: u8, data: &[u8]) -> (&[u8], &[u8]) {
    if selector & 0b1000_0000 == 0 {
        return (DEFAULT_URI, data);
    }

    let Some((&uri_len, framed)) = data.split_first() else {
        return (&[], &[]);
    };
    let uri_len = usize::from(uri_len)
        .min(MAX_OPTIONS_BYTES)
        .min(framed.len());
    framed.split_at(uri_len)
}

fn options_ptr(options: &[u8]) -> *const u8 {
    if options.is_empty() {
        ptr::null()
    } else {
        options.as_ptr()
    }
}

type EngineSourceCall = unsafe extern "C" fn(*const MermanEngine, *const u8, usize) -> MermanResult;

type EngineDocumentCall =
    unsafe extern "C" fn(*const MermanEngine, *const u8, usize, *const u8, usize) -> MermanResult;

unsafe fn call_engine_source(source: &[u8], options: &[u8], call: EngineSourceCall) {
    if let Some(engine) = unsafe { create_engine(options) } {
        unsafe {
            consume_result(call(engine, source.as_ptr(), source.len()));
            merman_engine_free(engine);
        }
    }
}

unsafe fn call_engine_document(
    source: &[u8],
    options: &[u8],
    uri: &[u8],
    call: EngineDocumentCall,
) {
    if let Some(engine) = unsafe { create_engine(options) } {
        unsafe {
            consume_result(call(
                engine,
                source.as_ptr(),
                source.len(),
                uri.as_ptr(),
                uri.len(),
            ));
            merman_engine_free(engine);
        }
    }
}

unsafe fn call_engine_with_text_callback(source: &[u8], options: &[u8]) {
    if let Some(engine) = unsafe { create_engine(options) } {
        unsafe {
            consume_result(merman_engine_set_text_measure_callback(
                engine,
                Some(fuzz_measure_text),
                ptr::null_mut(),
            ));
            consume_result(merman_engine_render_svg(
                engine,
                source.as_ptr(),
                source.len(),
            ));
            consume_result(merman_engine_set_text_measure_callback(
                engine,
                None,
                ptr::null_mut(),
            ));
            merman_engine_free(engine);
        }
    }
}

unsafe fn create_engine(options: &[u8]) -> Option<*mut MermanEngine> {
    let result = unsafe { merman_engine_new(options_ptr(options), options.len()) };
    assert_ne!(
        result.code, MERMAN_PANIC,
        "FFI engine creation crossed panic boundary"
    );
    unsafe { assert_text_buffer_contract(result.data) };
    unsafe { merman_buffer_free(result.data) };
    if result.code == MERMAN_OK {
        assert!(!result.engine.is_null(), "OK engine result had null engine");
        Some(result.engine)
    } else {
        assert!(
            result.engine.is_null(),
            "error engine result returned engine"
        );
        None
    }
}

unsafe fn call_metadata_exports() {
    assert!(merman_abi_version() > 0);
    assert!(!merman_package_version().is_null());
    assert_eq!(
        merman_buffer_struct_size(),
        std::mem::size_of::<MermanBuffer>()
    );
    assert_eq!(
        merman_result_struct_size(),
        std::mem::size_of::<MermanResult>()
    );
    assert_eq!(
        merman_engine_result_struct_size(),
        std::mem::size_of::<MermanEngineResult>()
    );
    assert_eq!(
        merman_host_text_measure_request_struct_size(),
        std::mem::size_of::<MermanHostTextMeasureRequest>()
    );
    assert_eq!(
        merman_host_text_measure_result_struct_size(),
        std::mem::size_of::<MermanHostTextMeasureResult>()
    );

    unsafe {
        consume_result(merman_supported_diagrams_json());
        consume_result(merman_ascii_capabilities_json());
        consume_result(merman_diagram_family_capabilities_json());
        consume_result(merman_lint_rule_catalog_json());
        consume_result(merman_supported_themes_json());
        consume_result(merman_supported_host_theme_presets_json());
    }
}

unsafe fn consume_result(result: MermanResult) {
    assert_ne!(result.code, MERMAN_PANIC, "FFI call crossed panic boundary");
    unsafe { assert_text_buffer_contract(result.data) };
    unsafe { merman_buffer_free(result.data) };
}

unsafe fn assert_text_buffer_contract(buffer: MermanBuffer) {
    if buffer.len == 0 {
        assert!(buffer.data.is_null(), "empty FFI buffer had non-null data");
    } else {
        assert!(!buffer.data.is_null(), "non-empty FFI buffer had null data");
        let bytes = unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) };
        let text = std::str::from_utf8(bytes)
            .unwrap_or_else(|error| panic!("FFI text buffer was not valid UTF-8: {error}"));
        std::hint::black_box(text);
    }
}

unsafe fn callback_text<'a>(data: *const u8, len: usize, field: &str) -> &'a str {
    if len == 0 {
        return "";
    }
    assert!(!data.is_null(), "non-empty callback {field} had null data");
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    std::str::from_utf8(bytes)
        .unwrap_or_else(|error| panic!("callback {field} was not valid UTF-8: {error}"))
}

unsafe extern "C" fn fuzz_measure_text(
    request: MermanHostTextMeasureRequest,
    _user_data: *mut std::ffi::c_void,
) -> MermanHostTextMeasureResult {
    let text = unsafe { callback_text(request.text, request.text_len, "text") };
    let font_family =
        unsafe { callback_text(request.font_family, request.font_family_len, "font_family") };
    let font_weight =
        unsafe { callback_text(request.font_weight, request.font_weight_len, "font_weight") };
    let font_style =
        unsafe { callback_text(request.font_style, request.font_style_len, "font_style") };
    std::hint::black_box((font_family, font_weight, font_style));

    let font_size = if request.font_size.is_finite() && request.font_size > 0.0 {
        request.font_size.min(128.0)
    } else {
        16.0
    };
    let width = (text.len().min(1024) as f64 * font_size * 0.5).max(font_size);
    let height = font_size * 1.25;

    MermanHostTextMeasureResult {
        handled: 1,
        width,
        height,
        line_count: 1,
    }
}
