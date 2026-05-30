/*
 * merman.h - C ABI for merman headless Mermaid rendering.
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
 * The caller must free every non-empty data buffer with merman_buffer_free.
 */
MermanResult merman_render_svg(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);

/*
 * Parse Mermaid source to semantic JSON.
 *
 * Success and error ownership rules are identical to merman_render_svg.
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
 */
MermanResult merman_layout_json(
    const uint8_t* source,
    size_t source_len,
    const uint8_t* options_json,
    size_t options_len
);

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
