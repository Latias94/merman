#include "merman.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

typedef MermanResult (*MermanCall)(const uint8_t*, size_t, const uint8_t*, size_t);
typedef MermanResult (*MermanEngineCall)(const MermanEngine*, const uint8_t*, size_t);
typedef void (*MermanFree)(MermanBuffer);

typedef struct MermanApi {
    int render_enabled;
    int ascii_enabled;
    uint32_t (*abi_version)(void);
    const char* (*package_version)(void);
    size_t (*buffer_struct_size)(void);
    size_t (*result_struct_size)(void);
    size_t (*engine_result_struct_size)(void);
    MermanEngineResult (*engine_new)(const uint8_t*, size_t);
    void (*engine_free)(MermanEngine*);
    MermanEngineCall engine_render_svg;
    MermanEngineCall engine_render_ascii;
    MermanEngineCall engine_parse_json;
    MermanEngineCall engine_layout_json;
    MermanEngineCall engine_validate_json;
    MermanCall render_svg;
    MermanCall render_ascii;
    MermanCall parse_json;
    MermanCall layout_json;
    MermanCall validate_json;
    MermanResult (*supported_diagrams_json)(void);
    MermanResult (*ascii_supported_diagrams_json)(void);
    MermanResult (*supported_themes_json)(void);
    MermanResult (*supported_host_theme_presets_json)(void);
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
        api.abi_version == NULL ||
        api.package_version == NULL ||
        api.buffer_struct_size == NULL ||
        api.result_struct_size == NULL ||
        api.engine_result_struct_size == NULL ||
        api.engine_new == NULL ||
        api.engine_free == NULL ||
        api.engine_render_svg == NULL ||
        api.engine_render_ascii == NULL ||
        api.engine_parse_json == NULL ||
        api.engine_layout_json == NULL ||
        api.engine_validate_json == NULL ||
        api.render_svg == NULL ||
        api.render_ascii == NULL ||
        api.parse_json == NULL ||
        api.layout_json == NULL ||
        api.validate_json == NULL ||
        api.supported_diagrams_json == NULL ||
        api.ascii_supported_diagrams_json == NULL ||
        api.supported_themes_json == NULL ||
        api.supported_host_theme_presets_json == NULL ||
        api.buffer_free == NULL
    ) {
        return 1;
    }

    if (api.abi_version() != MERMAN_ABI_VERSION) {
        return 2;
    }
    if (api.package_version() == NULL || strlen(api.package_version()) == 0) {
        return 3;
    }
    if (api.buffer_struct_size() != sizeof(MermanBuffer)) {
        return 4;
    }
    if (api.result_struct_size() != sizeof(MermanResult)) {
        return 5;
    }
    if (api.engine_result_struct_size() != sizeof(MermanEngineResult)) {
        return 6;
    }

    rc = api.render_enabled
        ? expect_ok_with(
            api.render_svg(source, sizeof(source) - 1, NULL, 0),
            api.buffer_free,
            "<svg"
        )
        : expect_error_with(
            api.render_svg(source, sizeof(source) - 1, NULL, 0),
            api.buffer_free,
            MERMAN_UNSUPPORTED_FORMAT,
            "MERMAN_UNSUPPORTED_FORMAT"
        );
    if (rc != 0) {
        return rc;
    }

    rc = api.ascii_enabled
        ? expect_ok_with(
            api.render_ascii(source, sizeof(source) - 1, NULL, 0),
            api.buffer_free,
            "Hello"
        )
        : expect_error_with(
            api.render_ascii(source, sizeof(source) - 1, NULL, 0),
            api.buffer_free,
            MERMAN_UNSUPPORTED_FORMAT,
            "MERMAN_UNSUPPORTED_FORMAT"
        );
    if (rc != 0) {
        return rc;
    }

    rc = api.render_enabled
        ? expect_ok_with(
            api.parse_json(source, sizeof(source) - 1, NULL, 0),
            api.buffer_free,
            "flowchart-v2"
        )
        : expect_error_with(
            api.parse_json(source, sizeof(source) - 1, NULL, 0),
            api.buffer_free,
            MERMAN_UNSUPPORTED_FORMAT,
            "MERMAN_UNSUPPORTED_FORMAT"
        );
    if (rc != 0) {
        return rc;
    }

    rc = api.render_enabled
        ? expect_ok_with(
            api.layout_json(source, sizeof(source) - 1, NULL, 0),
            api.buffer_free,
            "layout"
        )
        : expect_error_with(
            api.layout_json(source, sizeof(source) - 1, NULL, 0),
            api.buffer_free,
            MERMAN_UNSUPPORTED_FORMAT,
            "MERMAN_UNSUPPORTED_FORMAT"
        );
    if (rc != 0) {
        return rc;
    }

    rc = expect_ok_with(
        api.validate_json(source, sizeof(source) - 1, NULL, 0),
        api.buffer_free,
        api.render_enabled ? "\"valid\":true" : "MERMAN_UNSUPPORTED_FORMAT"
    );
    if (rc != 0) {
        return rc;
    }

    rc = expect_ok_with(api.supported_diagrams_json(), api.buffer_free, "flowchart");
    if (rc != 0) {
        return rc;
    }

    rc = expect_ok_with(
        api.ascii_supported_diagrams_json(),
        api.buffer_free,
        api.ascii_enabled ? "sequence" : "[]"
    );
    if (rc != 0) {
        return rc;
    }

    rc = expect_ok_with(api.supported_themes_json(), api.buffer_free, "default");
    if (rc != 0) {
        return rc;
    }

    rc = expect_ok_with(
        api.supported_host_theme_presets_json(),
        api.buffer_free,
        api.render_enabled ? "one-dark" : "[]"
    );
    if (rc != 0) {
        return rc;
    }

    MermanEngineResult engine = api.engine_new(NULL, 0);
    if (engine.code != MERMAN_OK || engine.engine == NULL) {
        if (engine.data.data != NULL || engine.data.len != 0) {
            api.buffer_free(engine.data);
        }
        return 50 + engine.code;
    }

    rc = api.render_enabled
        ? expect_ok_with(
            api.engine_render_svg(engine.engine, source, sizeof(source) - 1),
            api.buffer_free,
            "<svg"
        )
        : expect_error_with(
            api.engine_render_svg(engine.engine, source, sizeof(source) - 1),
            api.buffer_free,
            MERMAN_UNSUPPORTED_FORMAT,
            "MERMAN_UNSUPPORTED_FORMAT"
        );
    if (rc != 0) {
        api.engine_free(engine.engine);
        return rc;
    }

    rc = api.ascii_enabled
        ? expect_ok_with(
            api.engine_render_ascii(engine.engine, source, sizeof(source) - 1),
            api.buffer_free,
            "Hello"
        )
        : expect_error_with(
            api.engine_render_ascii(engine.engine, source, sizeof(source) - 1),
            api.buffer_free,
            MERMAN_UNSUPPORTED_FORMAT,
            "MERMAN_UNSUPPORTED_FORMAT"
        );
    if (rc != 0) {
        api.engine_free(engine.engine);
        return rc;
    }

    rc = expect_ok_with(
        api.engine_validate_json(engine.engine, source, sizeof(source) - 1),
        api.buffer_free,
        api.render_enabled ? "\"valid\":true" : "MERMAN_UNSUPPORTED_FORMAT"
    );
    if (rc != 0) {
        api.engine_free(engine.engine);
        return rc;
    }

    api.engine_free(engine.engine);

    return expect_error_with(
        api.render_svg(NULL, 1, NULL, 0),
        api.buffer_free,
        MERMAN_INVALID_ARGUMENT,
        "MERMAN_INVALID_ARGUMENT"
    );
}
