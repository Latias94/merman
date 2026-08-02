#include "merman.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

typedef MermanNativeStatus (*MermanGetNativeApiFn)(
    const MermanNativeApiRequest *request,
    MermanNativeApi *out_api
);

typedef struct MermanNativeApiPublishedSixBuffer {
    /* This fixture is the exact six-slot header from baseline commit 5117c0ae. */
    MermanNativeApi api;
    uint8_t trailing_guard[16];
} MermanNativeApiPublishedSixBuffer;

static MermanNativeSlice borrowed_slice(const uint8_t *data, size_t len) {
    MermanNativeSlice slice;
    slice.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice);
    slice.data = data;
    slice.len = len;
    return slice;
}

static MermanNativeResult empty_result(void) {
    return (MermanNativeResult)MERMAN_NATIVE_RESULT_INIT;
}

static int guard_is_intact(const uint8_t *bytes, size_t len) {
    size_t index;
    for (index = 0; index < len; index += 1) {
        if (bytes[index] != 0xa5u) {
            return 0;
        }
    }
    return 1;
}

static int bytes_contain(const uint8_t *data, size_t len, const char *needle) {
    const size_t needle_len = strlen(needle);
    size_t index;
    if (data == NULL || len < needle_len) {
        return 0;
    }
    for (index = 0; index <= len - needle_len; index += 1) {
        if (memcmp(data + index, needle, needle_len) == 0) {
            return 1;
        }
    }
    return 0;
}

#if defined(_WIN32)
__declspec(dllexport)
#else
__attribute__((visibility("default")))
#endif
int merman_abi3_published_six_consumer_smoke(MermanGetNativeApiFn get_native_api) {
    static const uint8_t source[] = "flowchart TD\nA --> B";
    static const uint8_t metadata_id[] = "supported-diagrams";
    MermanNativeApiRequest request;
    MermanNativeApiPublishedSixBuffer api_buffer;
    MermanNativeApi *api = &api_buffer.api;
    MermanNativeEngineConfig config;
    MermanNativeEngineToken engine = 0;
    MermanNativeOperationRequest operation;
    MermanNativeResult result;

    if (get_native_api == NULL) {
        return 1;
    }

    memset(&request, 0, sizeof(request));
    request.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApiRequest);
    request.expected_abi_version = MERMAN_NATIVE_ABI_VERSION;
    request.expected_minimum_prefix_layout_digest = borrowed_slice(
        (const uint8_t *)MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST,
        strlen(MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST)
    );
    memset(&api_buffer, 0, sizeof(api_buffer));
    memset(api_buffer.trailing_guard, 0xa5, sizeof(api_buffer.trailing_guard));
    api->struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi);
    if (
        get_native_api(&request, api) != MERMAN_NATIVE_STATUS_OK ||
        api->struct_size != MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi) ||
        !guard_is_intact(api_buffer.trailing_guard, sizeof(api_buffer.trailing_guard)) ||
        api->runtime_catalog == NULL ||
        api->engine_new == NULL ||
        api->engine_try_close == NULL ||
        api->execute_collect == NULL ||
        api->result_free == NULL ||
        api->metadata_collect == NULL
    ) {
        return 2;
    }

    result = empty_result();
    if (
        api->runtime_catalog(&result) != MERMAN_NATIVE_STATUS_OK ||
        result.operation != MERMAN_NATIVE_OPERATION_NONE ||
        result.allocation_token == 0
    ) {
        api->result_free(&result);
        return 3;
    }
    api->result_free(&result);

    result = empty_result();
    if (
        api->metadata_collect(
            borrowed_slice(metadata_id, sizeof(metadata_id) - 1),
            &result
        ) != MERMAN_NATIVE_STATUS_OK ||
        result.operation != MERMAN_NATIVE_OPERATION_NONE ||
        result.allocation_token == 0 ||
        result.metadata_or_error_json.len == 0
    ) {
        api->result_free(&result);
        return 4;
    }
    api->result_free(&result);

    memset(&config, 0, sizeof(config));
    config.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeEngineConfig);
    config.options_json = borrowed_slice(NULL, 0);
    result = empty_result();
    if (
        api->engine_new(&config, &engine, &result) != MERMAN_NATIVE_STATUS_OK ||
        engine == 0 ||
        result.allocation_token == 0
    ) {
        api->result_free(&result);
        return 5;
    }
    api->result_free(&result);

    memset(&operation, 0, sizeof(operation));
    operation.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeOperationRequest);
    operation.operation = MERMAN_NATIVE_OPERATION_SEMANTIC_JSON;
    operation.source = borrowed_slice(source, sizeof(source) - 1);
    operation.uri = borrowed_slice(NULL, 0);
    operation.options_json = borrowed_slice(NULL, 0);
    result = empty_result();
    if (
        api->execute_collect(engine, &operation, &result) != MERMAN_NATIVE_STATUS_OK ||
        result.operation != MERMAN_NATIVE_OPERATION_SEMANTIC_JSON ||
        result.allocation_token == 0 ||
        !bytes_contain(result.data.data, result.data.len, "{")
    ) {
        api->result_free(&result);
        api->engine_try_close(engine);
        return 6;
    }
    api->result_free(&result);

    if (api->engine_try_close(engine) != MERMAN_NATIVE_STATUS_OK) {
        return 7;
    }
    if (api->engine_try_close(engine) != MERMAN_NATIVE_STATUS_INVALID_ENGINE) {
        return 8;
    }
    return 0;
}
