#include "merman.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

typedef MermanResult (*MermanCall)(const uint8_t*, size_t, const uint8_t*, size_t);
typedef void (*MermanFree)(MermanBuffer);

typedef struct MermanApi {
    MermanCall render_svg;
    MermanCall parse_json;
    MermanCall layout_json;
    MermanFree buffer_free;
} MermanApi;

static int buffer_contains(MermanBuffer buffer, const char* needle) {
    size_t needle_len = strlen(needle);
    if (needle_len == 0) {
        return 1;
    }
    if (buffer.data == NULL || buffer.len < needle_len) {
        return 0;
    }
    for (size_t i = 0; i <= buffer.len - needle_len; i++) {
        if (memcmp(buffer.data + i, needle, needle_len) == 0) {
            return 1;
        }
    }
    return 0;
}

static int expect_ok_with(MermanResult result, MermanFree free_buffer, const char* needle) {
    if (result.code != MERMAN_OK) {
        if (result.data.data != NULL || result.data.len != 0) {
            free_buffer(result.data);
        }
        return 10 + result.code;
    }
    if (!buffer_contains(result.data, needle)) {
        free_buffer(result.data);
        return 20;
    }
    free_buffer(result.data);
    return 0;
}

static int expect_error_with(
    MermanResult result,
    MermanFree free_buffer,
    int expected_code,
    const char* code_name
) {
    if (result.code != expected_code) {
        if (result.data.data != NULL || result.data.len != 0) {
            free_buffer(result.data);
        }
        return 30 + result.code;
    }
    if (!buffer_contains(result.data, code_name)) {
        free_buffer(result.data);
        return 40;
    }
    free_buffer(result.data);
    return 0;
}

#if defined(_WIN32)
__declspec(dllexport)
#else
__attribute__((visibility("default")))
#endif
int merman_c_consumer_smoke(MermanApi api) {
    static const uint8_t source[] = "flowchart TD\nA[Hello] --> B[World]";
    int rc = 0;

    if (
        api.render_svg == NULL ||
        api.parse_json == NULL ||
        api.layout_json == NULL ||
        api.buffer_free == NULL
    ) {
        return 1;
    }

    rc = expect_ok_with(
        api.render_svg(source, sizeof(source) - 1, NULL, 0),
        api.buffer_free,
        "<svg"
    );
    if (rc != 0) {
        return rc;
    }

    rc = expect_ok_with(
        api.parse_json(source, sizeof(source) - 1, NULL, 0),
        api.buffer_free,
        "flowchart-v2"
    );
    if (rc != 0) {
        return rc;
    }

    rc = expect_ok_with(
        api.layout_json(source, sizeof(source) - 1, NULL, 0),
        api.buffer_free,
        "layout"
    );
    if (rc != 0) {
        return rc;
    }

    return expect_error_with(
        api.render_svg(NULL, 1, NULL, 0),
        api.buffer_free,
        MERMAN_INVALID_ARGUMENT,
        "MERMAN_INVALID_ARGUMENT"
    );
}
