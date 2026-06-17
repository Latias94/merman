/*
 * merman.h - C ABI for merman headless Mermaid rendering.
 * Project: https://github.com/Latias94/merman
 *
 * All strings are UTF-8 byte buffers. Every non-empty MermanResult.data buffer returned by Rust
 * must be released with merman_buffer_free.
 */

#ifndef MERMAN_H
#define MERMAN_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define MERMAN_ABI_VERSION 2

enum {
    MERMAN_OK = 0,
    MERMAN_INVALID_ARGUMENT = 1,
    MERMAN_UTF8_ERROR = 2,
    MERMAN_OPTIONS_JSON_ERROR = 3,
    MERMAN_NO_DIAGRAM = 4,
    MERMAN_PARSE_ERROR = 5,
    MERMAN_RENDER_ERROR = 6,
    MERMAN_UNSUPPORTED_FORMAT = 7,
    MERMAN_PANIC = 8,
    MERMAN_INTERNAL_ERROR = 9
};

typedef struct MermanBuffer {
    uint8_t* data;
    size_t len;
} MermanBuffer;

typedef struct MermanResult {
    int32_t code;
    MermanBuffer data;
} MermanResult;

typedef struct MermanEngine MermanEngine;

typedef struct MermanEngineResult {
    int32_t code;
    MermanEngine* engine;
    MermanBuffer data;
} MermanEngineResult;

enum {
    MERMAN_WRAP_MODE_SVG_LIKE = 0,
    MERMAN_WRAP_MODE_SVG_LIKE_SINGLE_RUN = 1,
    MERMAN_WRAP_MODE_HTML_LIKE = 2
};

enum {
    MERMAN_TEXT_DIRECTION_AUTO = 0,
    MERMAN_TEXT_DIRECTION_LTR = 1,
    MERMAN_TEXT_DIRECTION_RTL = 2
};

enum {
    MERMAN_TEXT_WHITE_SPACE_NORMAL = 0,
    MERMAN_TEXT_WHITE_SPACE_NOWRAP = 1,
    MERMAN_TEXT_WHITE_SPACE_BREAK_SPACES = 2,
    MERMAN_TEXT_WHITE_SPACE_PRE_WRAP = 3
};

typedef struct MermanHostTextMeasureRequest {
    const uint8_t* text;
    size_t text_len;
    const uint8_t* font_family;
    size_t font_family_len;
    double font_size;
    const uint8_t* font_weight;
    size_t font_weight_len;
    const uint8_t* font_style;
    size_t font_style_len;
    double max_width;
    double line_height;
    double letter_spacing;
    double word_spacing;
    int32_t wrap_mode;
    int32_t direction;
    int32_t white_space;
    uint8_t has_max_width;
} MermanHostTextMeasureRequest;

typedef struct MermanHostTextMeasureResult {
    uint8_t handled;
    double width;
    double height;
    size_t line_count;
} MermanHostTextMeasureResult;

typedef MermanHostTextMeasureResult (*MermanHostTextMeasureCallback)(
    MermanHostTextMeasureRequest request,
    void* user_data
);

/*
 * Return the C ABI protocol version implemented by this library.
 *
 * Hosts should compare this with MERMAN_ABI_VERSION before calling render functions.
 */
uint32_t merman_abi_version(void);

/*
 * Return the merman-ffi crate package version as a static null-terminated string.
 *
 * The returned pointer is owned by Rust and must not be freed.
 */
const char* merman_package_version(void);

/*
 * Return the Rust-side struct sizes for runtime host compatibility checks.
 */
size_t merman_buffer_struct_size(void);
size_t merman_result_struct_size(void);
size_t merman_engine_result_struct_size(void);
size_t merman_host_text_measure_request_struct_size(void);
size_t merman_host_text_measure_result_struct_size(void);

/*
 * Create and free a reusable engine for repeated calls with the same options_json.
 *
 * code == MERMAN_OK:
 *   engine contains an opaque handle for merman_engine_* calls and data is empty.
 * code != MERMAN_OK:
 *   engine is NULL and data contains UTF-8 JSON error bytes.
 *
 * The caller must release a non-null engine with merman_engine_free.
 * The caller must not free an engine while another thread is using it.
 */
MermanEngineResult merman_engine_new(
    const uint8_t* options_json,
    size_t options_len
);
void merman_engine_free(MermanEngine* engine);

/*
 * Install a host-provided text measurer on a reusable engine.
 *
 * The callback is used by future layout/render calls made through this engine. Return
 * handled=0 for any request the host does not support; merman will fall back to its vendored
 * measurer for that request. Passing callback=NULL resets the engine to the text measurer selected
 * by merman_engine_new options.
 *
 * Request strings are UTF-8 byte slices valid only for the duration of the callback. The callback
 * must not store those pointers. If the same engine is used concurrently, the callback and
 * user_data must be thread-safe.
 */
MermanResult merman_engine_set_text_measure_callback(
    MermanEngine* engine,
    MermanHostTextMeasureCallback callback,
    void* user_data
);

/*
 * Reusable-engine variants of the stateless entry points.
 *
 * These functions use the options captured by merman_engine_new. They are intended for hosts that
 * render many diagrams with the same layout/SVG/parse settings.
 */
MermanResult merman_engine_render_svg(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);
MermanResult merman_engine_render_ascii(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);
MermanResult merman_engine_parse_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);
MermanResult merman_engine_layout_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);
MermanResult merman_engine_validate_json(
    const MermanEngine* engine,
    const uint8_t* source,
    size_t source_len
);

/*
 * Render Mermaid source to SVG.
 *
 * source:
 *   UTF-8 Mermaid source bytes. May be NULL only when source_len == 0.
 *
 * options_json:
 *   Optional UTF-8 JSON options. Pass NULL/0 for defaults.
 *
 * Result:
 *   code == MERMAN_OK:
 *     data contains UTF-8 SVG bytes.
 *   code != MERMAN_OK:
 *     data contains UTF-8 JSON error bytes:
 *       {"version":1,"ok":false,"code":6,"code_name":"MERMAN_RENDER_ERROR","message":"..."}
 *
 * If the library was built without render support, this returns MERMAN_UNSUPPORTED_FORMAT.
 *
 * The caller must free every non-empty data buffer with merman_buffer_free.
 */
MermanResult merman_render_svg(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);

/*
 * Render Mermaid source to Unicode ASCII-art text.
 *
 * Success and error ownership rules are identical to merman_render_svg.
 * If the library was built without ASCII support, this returns MERMAN_UNSUPPORTED_FORMAT.
 */
MermanResult merman_render_ascii(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);

/*
 * Parse Mermaid source to semantic JSON.
 *
 * Success and error ownership rules are identical to merman_render_svg.
 * If the library was built without render support, this returns MERMAN_UNSUPPORTED_FORMAT.
 */
MermanResult merman_parse_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);

/*
 * Layout Mermaid source to layout JSON.
 *
 * Success and error ownership rules are identical to merman_render_svg.
 * If the library was built without render support, this returns MERMAN_UNSUPPORTED_FORMAT.
 */
MermanResult merman_layout_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);

/*
 * Validate Mermaid source and return a UTF-8 JSON validation payload.
 *
 * This function returns MERMAN_OK when the validation payload itself was produced. Invalid Mermaid
 * source is represented inside data:
 *   {"valid":false,"error":"...","code":5,"code_name":"MERMAN_PARSE_ERROR"}
 *
 * If the library was built without render support, this still returns MERMAN_OK with
 * MERMAN_UNSUPPORTED_FORMAT represented inside the validation payload.
 */
MermanResult merman_validate_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);

/*
 * Return UTF-8 JSON string arrays describing binding metadata.
 *
 * Success and error ownership rules are identical to merman_render_svg.
 */
MermanResult merman_supported_diagrams_json(void);
MermanResult merman_ascii_supported_diagrams_json(void);
MermanResult merman_supported_themes_json(void);
MermanResult merman_supported_host_theme_presets_json(void);

/*
 * Free a buffer returned by merman.
 *
 * Passing {NULL, 0} is a no-op. Passing the same non-null buffer twice is caller misuse.
 */
void merman_buffer_free(MermanBuffer buffer);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* MERMAN_H */
