#include "merman.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

typedef MermanNativeStatus (*MermanGetNativeApiFn)(
    const MermanNativeApiRequest *request,
    MermanNativeApi *out_api
);

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

static int bytes_contain(const uint8_t *data, size_t len, const char *needle) {
    const size_t needle_len = strlen(needle);
    size_t index = 0;
    if (needle_len == 0) {
        return 1;
    }
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

static uint64_t hash_size_t(uint64_t hash, size_t value) {
    const uint8_t *bytes = (const uint8_t *)&value;
    size_t index = 0;
    for (index = 0; index < sizeof(value); index += 1) {
        hash ^= bytes[index];
        hash *= UINT64_C(1099511628211);
    }
    return hash;
}

#define HASH_RECORD(hash, type) \
    do { \
        (hash) = hash_size_t((hash), sizeof(type)); \
        (hash) = hash_size_t((hash), _Alignof(type)); \
    } while (0)

#define HASH_FIELD(hash, type, field) \
    do { \
        (hash) = hash_size_t((hash), offsetof(type, field)); \
    } while (0)

#if defined(_WIN32)
__declspec(dllexport)
#else
__attribute__((visibility("default")))
#endif
uint64_t merman_c_layout_fingerprint(void) {
    uint64_t hash = UINT64_C(14695981039346656037);

    HASH_RECORD(hash, MermanNativeSlice);
    HASH_FIELD(hash, MermanNativeSlice, struct_size);
    HASH_FIELD(hash, MermanNativeSlice, data);
    HASH_FIELD(hash, MermanNativeSlice, len);

    HASH_RECORD(hash, MermanNativeBuffer);
    HASH_FIELD(hash, MermanNativeBuffer, struct_size);
    HASH_FIELD(hash, MermanNativeBuffer, data);
    HASH_FIELD(hash, MermanNativeBuffer, len);

    HASH_RECORD(hash, MermanNativeTextMeasureRequest);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, struct_size);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, text_measurement_protocol_version);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, text);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, font_family);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, font_size);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, font_weight);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, font_style);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, max_width);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, line_height);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, letter_spacing);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, word_spacing);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, wrap_mode);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, direction);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, white_space);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, has_max_width);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, phase);
    HASH_FIELD(hash, MermanNativeTextMeasureRequest, operation);

    HASH_RECORD(hash, MermanNativeTextMeasureResult);
    HASH_FIELD(hash, MermanNativeTextMeasureResult, struct_size);
    HASH_FIELD(hash, MermanNativeTextMeasureResult, handled);
    HASH_FIELD(hash, MermanNativeTextMeasureResult, has_raw_width);
    HASH_FIELD(hash, MermanNativeTextMeasureResult, result_kind);
    HASH_FIELD(hash, MermanNativeTextMeasureResult, width);
    HASH_FIELD(hash, MermanNativeTextMeasureResult, height);
    HASH_FIELD(hash, MermanNativeTextMeasureResult, length);
    HASH_FIELD(hash, MermanNativeTextMeasureResult, bbox_left);
    HASH_FIELD(hash, MermanNativeTextMeasureResult, bbox_right);
    HASH_FIELD(hash, MermanNativeTextMeasureResult, raw_width);
    HASH_FIELD(hash, MermanNativeTextMeasureResult, line_count);

    HASH_RECORD(hash, MermanNativeEngineConfig);
    HASH_FIELD(hash, MermanNativeEngineConfig, struct_size);
    HASH_FIELD(hash, MermanNativeEngineConfig, options_json);
    HASH_FIELD(hash, MermanNativeEngineConfig, text_measure);
    HASH_FIELD(hash, MermanNativeEngineConfig, text_measure_user_data);

    HASH_RECORD(hash, MermanNativeOperationRequest);
    HASH_FIELD(hash, MermanNativeOperationRequest, struct_size);
    HASH_FIELD(hash, MermanNativeOperationRequest, operation);
    HASH_FIELD(hash, MermanNativeOperationRequest, source);
    HASH_FIELD(hash, MermanNativeOperationRequest, uri);
    HASH_FIELD(hash, MermanNativeOperationRequest, options_json);

    HASH_RECORD(hash, MermanNativeResult);
    HASH_FIELD(hash, MermanNativeResult, struct_size);
    HASH_FIELD(hash, MermanNativeResult, status);
    HASH_FIELD(hash, MermanNativeResult, operation);
    HASH_FIELD(hash, MermanNativeResult, media_type);
    HASH_FIELD(hash, MermanNativeResult, data);
    HASH_FIELD(hash, MermanNativeResult, metadata_or_error_json);

    HASH_RECORD(hash, MermanNativeApiRequest);
    HASH_FIELD(hash, MermanNativeApiRequest, struct_size);
    HASH_FIELD(hash, MermanNativeApiRequest, expected_abi_version);
    HASH_FIELD(hash, MermanNativeApiRequest, expected_layout_descriptor_digest);

    HASH_RECORD(hash, MermanNativeApi);
    HASH_FIELD(hash, MermanNativeApi, struct_size);
    HASH_FIELD(hash, MermanNativeApi, abi_version);
    HASH_FIELD(hash, MermanNativeApi, layout_descriptor_digest);
    HASH_FIELD(hash, MermanNativeApi, capability_catalog_digest);
    HASH_FIELD(hash, MermanNativeApi, package_version);
    HASH_FIELD(hash, MermanNativeApi, runtime_catalog);
    HASH_FIELD(hash, MermanNativeApi, engine_new);
    HASH_FIELD(hash, MermanNativeApi, engine_free);
    HASH_FIELD(hash, MermanNativeApi, execute_collect);
    HASH_FIELD(hash, MermanNativeApi, result_free);

    return hash;
}

#if defined(_WIN32)
__declspec(dllexport)
#else
__attribute__((visibility("default")))
#endif
int merman_c_consumer_smoke(
    MermanGetNativeApiFn get_native_api,
    int require_complete_artifact
) {
    static const uint8_t source[] = "flowchart TD\nA --> B";
    static const uint8_t request_options[] =
        "{\"svg\":{\"diagram_id\":\"c-request\"}}";
    MermanNativeApiRequest discovery;
    MermanNativeApi api;
    MermanNativeResult result;
    MermanNativeEngineConfig config;
    MermanNativeEngineToken engine = 0;
    MermanNativeOperationRequest request;
    MermanNativeStatus status;

    if (get_native_api == NULL) {
        return 1;
    }

    memset(&discovery, 0, sizeof(discovery));
    discovery.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApiRequest);
    discovery.expected_abi_version = MERMAN_NATIVE_ABI_VERSION;
    discovery.expected_layout_descriptor_digest = borrowed_slice(
        (const uint8_t *)MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST,
        strlen(MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST)
    );
    memset(&api, 0, sizeof(api));
    api.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi);
    status = get_native_api(&discovery, &api);
    if (
        status != MERMAN_NATIVE_STATUS_OK ||
        api.abi_version != MERMAN_NATIVE_ABI_VERSION ||
        api.runtime_catalog == NULL ||
        api.engine_new == NULL ||
        api.engine_free == NULL ||
        api.execute_collect == NULL ||
        api.result_free == NULL
    ) {
        return 2;
    }

    result = empty_result();
    status = api.runtime_catalog(&result);
    if (
        status != MERMAN_NATIVE_STATUS_OK ||
        result.status != MERMAN_NATIVE_STATUS_OK ||
        result.operation != MERMAN_NATIVE_OPERATION_NONE ||
        !bytes_contain(
            result.metadata_or_error_json.data,
            result.metadata_or_error_json.len,
            "\"transport_api_version\":3"
        ) ||
        !bytes_contain(
            result.metadata_or_error_json.data,
            result.metadata_or_error_json.len,
            "\"capabilities\""
        ) ||
        !bytes_contain(
            result.metadata_or_error_json.data,
            result.metadata_or_error_json.len,
            "\"operation_ids\""
        )
    ) {
        api.result_free(&result);
        return 3;
    }
    api.result_free(&result);

    memset(&config, 0, sizeof(config));
    config.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeEngineConfig);
    config.options_json = borrowed_slice(NULL, 0);
    result = empty_result();
    status = api.engine_new(&config, &engine, &result);
    if (
        status != MERMAN_NATIVE_STATUS_OK ||
        result.status != MERMAN_NATIVE_STATUS_OK ||
        engine == 0
    ) {
        api.result_free(&result);
        return 4;
    }
    api.result_free(&result);

    memset(&request, 0, sizeof(request));
    request.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeOperationRequest);
    request.source = borrowed_slice(source, sizeof(source) - 1);
    request.uri = borrowed_slice(NULL, 0);
    request.options_json = borrowed_slice(NULL, 0);

    if (require_complete_artifact) {
        request.operation = MERMAN_NATIVE_OPERATION_SVG;
        request.options_json = borrowed_slice(
            request_options,
            sizeof(request_options) - 1
        );
        result = empty_result();
        status = api.execute_collect(engine, &request, &result);
        if (
            status != MERMAN_NATIVE_STATUS_OK ||
            result.status != MERMAN_NATIVE_STATUS_OK ||
            result.operation != MERMAN_NATIVE_OPERATION_SVG ||
            !bytes_contain(result.data.data, result.data.len, "<svg") ||
            !bytes_contain(result.data.data, result.data.len, "id=\"c-request\"") ||
            !bytes_contain(
                result.metadata_or_error_json.data,
                result.metadata_or_error_json.len,
                "\"operation_id\":\"svg\""
            ) ||
            !bytes_contain(
                result.metadata_or_error_json.data,
                result.metadata_or_error_json.len,
                "\"runtime_policy\":\"deterministic\""
            )
        ) {
            api.result_free(&result);
            api.engine_free(engine);
            return 5;
        }
        api.result_free(&result);
        request.options_json = borrowed_slice(NULL, 0);
    }

    request.operation = MERMAN_NATIVE_OPERATION_SEMANTIC_JSON;
    result = empty_result();
    status = api.execute_collect(engine, &request, &result);
    if (
        status != MERMAN_NATIVE_STATUS_OK ||
        result.status != MERMAN_NATIVE_STATUS_OK ||
        result.operation != MERMAN_NATIVE_OPERATION_SEMANTIC_JSON ||
        !bytes_contain(result.data.data, result.data.len, "{") ||
        !bytes_contain(
            result.metadata_or_error_json.data,
            result.metadata_or_error_json.len,
            "\"operation_id\":\"semantic-json\""
        )
    ) {
        api.result_free(&result);
        api.engine_free(engine);
        return 6;
    }
    api.result_free(&result);

    request.operation = INT32_MAX;
    result = empty_result();
    status = api.execute_collect(engine, &request, &result);
    if (
        status != MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION ||
        result.status != MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION ||
        result.operation != MERMAN_NATIVE_OPERATION_NONE ||
        !bytes_contain(
            result.metadata_or_error_json.data,
            result.metadata_or_error_json.len,
            "\"version\":1"
        ) ||
        !bytes_contain(
            result.metadata_or_error_json.data,
            result.metadata_or_error_json.len,
            "\"kind\":\"" MERMAN_NATIVE_ERROR_KIND_UNKNOWN_OPERATION "\""
        ) ||
        !bytes_contain(
            result.metadata_or_error_json.data,
            result.metadata_or_error_json.len,
            "\"capability_id\":null"
        )
    ) {
        api.result_free(&result);
        api.engine_free(engine);
        return 7;
    }
    api.result_free(&result);

    if (api.engine_free(engine) != MERMAN_NATIVE_STATUS_OK) {
        return 8;
    }
    if (api.engine_free(engine) != MERMAN_NATIVE_STATUS_INVALID_ENGINE) {
        return 9;
    }
    return 0;
}
