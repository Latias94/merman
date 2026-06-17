#include "merman.h"

#include <stdint.h>
#include <stdio.h>
#include <string.h>

static int print_error(const char* label, int code, MermanBuffer data) {
    fprintf(
        stderr,
        "%s failed (%d): %.*s\n",
        label,
        code,
        (int)data.len,
        data.data == NULL ? "" : (const char*)data.data
    );
    merman_buffer_free(data);
    return 1;
}

static MermanHostTextMeasureResult measure_text(
    MermanHostTextMeasureRequest request,
    void* user_data
) {
    (void)user_data;

    /*
     * Real hosts should measure with the same DOM/canvas/native text stack used for display.
     * This example only demonstrates the callback shape and falls back for most requests.
     */
    if (
        request.text_len == 5 &&
        request.text != NULL &&
        memcmp(request.text, "Hello", 5) == 0 &&
        request.wrap_mode == MERMAN_WRAP_MODE_HTML_LIKE
    ) {
        MermanHostTextMeasureResult result = {1, 40.0, request.line_height, 1};
        return result;
    }

    MermanHostTextMeasureResult fallback = {0, 0.0, 0.0, 0};
    return fallback;
}

int main(void) {
    static const uint8_t source[] = "flowchart TD\nA[Hello] --> B[World]";
    static const uint8_t options[] =
        "{"
        "\"layout\":{\"text_measurer\":\"deterministic\"},"
        "\"svg\":{\"diagram_id\":\"ffi engine example\",\"pipeline\":\"readable\"}"
        "}";

    if (merman_abi_version() != MERMAN_ABI_VERSION) {
        fprintf(stderr, "Merman ABI mismatch\n");
        return 1;
    }

    MermanEngineResult engine =
        merman_engine_new(options, sizeof(options) - 1);
    if (engine.code != MERMAN_OK) {
        return print_error("Merman engine creation", engine.code, engine.data);
    }

    MermanResult callback_result =
        merman_engine_set_text_measure_callback(engine.engine, measure_text, NULL);
    if (callback_result.code != MERMAN_OK) {
        merman_engine_free(engine.engine);
        return print_error("Merman text measurement callback", callback_result.code, callback_result.data);
    }
    merman_buffer_free(callback_result.data);

    MermanResult result =
        merman_engine_render_svg(engine.engine, source, sizeof(source) - 1);
    if (result.code != MERMAN_OK) {
        merman_engine_free(engine.engine);
        return print_error("Merman render", result.code, result.data);
    }

    printf("%.*s\n", (int)result.data.len, (const char*)result.data.data);
    merman_buffer_free(result.data);
    merman_engine_free(engine.engine);
    return 0;
}
