/*
 * This is a deliberately small alpha.5 consumer fixture.
 *
 * Keep these declarations frozen to the alpha.5 MermanNativeApi prefix.  Do
 * not include the current generated header here: the point of this test is to
 * compile a consumer that has never heard of the alpha.6 appended slots.
 */

#include <stddef.h>
#include <stdint.h>

typedef int32_t MermanNativeStatus;
typedef int32_t MermanNativeOperationCode;
typedef uint64_t MermanNativeEngineToken;

enum {
    MERMAN_NATIVE_STATUS_OK = 0,
    MERMAN_NATIVE_OPERATION_SEMANTIC_JSON = 6,
};

typedef struct MermanNativeSlice {
    uint32_t struct_size;
    const uint8_t *data;
    size_t len;
} MermanNativeSlice;

typedef struct MermanNativeBuffer {
    uint32_t struct_size;
    uint8_t *data;
    size_t len;
} MermanNativeBuffer;

typedef struct MermanNativeResult {
    uint32_t struct_size;
    uint64_t allocation_token;
    MermanNativeStatus status;
    MermanNativeOperationCode operation;
    MermanNativeSlice media_type;
    MermanNativeBuffer data;
    MermanNativeBuffer metadata_or_error_json;
} MermanNativeResult;

typedef struct MermanNativeEngineConfig MermanNativeEngineConfig;
typedef struct MermanNativeOperationRequest MermanNativeOperationRequest;
typedef struct MermanNativeEngineServicesConfig MermanNativeEngineServicesConfig;

typedef struct MermanNativeApiRequest {
    uint32_t struct_size;
    uint32_t expected_abi_version;
    MermanNativeSlice expected_minimum_prefix_layout_digest;
} MermanNativeApiRequest;

typedef MermanNativeStatus (*MermanNativeRuntimeCatalogFn)(MermanNativeResult *out_result);
typedef MermanNativeStatus (*MermanNativeEngineNewFn)(
    const MermanNativeEngineConfig *config,
    MermanNativeEngineToken *out_engine,
    MermanNativeResult *out_result
);
typedef MermanNativeStatus (*MermanNativeEngineTryCloseFn)(MermanNativeEngineToken engine);
typedef MermanNativeStatus (*MermanNativeExecuteCollectFn)(
    MermanNativeEngineToken engine,
    const MermanNativeOperationRequest *request,
    MermanNativeResult *out_result
);
typedef void (*MermanNativeResultFreeFn)(MermanNativeResult *result);
typedef MermanNativeStatus (*MermanNativeMetadataCollectFn)(
    MermanNativeSlice metadata_id,
    MermanNativeResult *out_result
);
typedef MermanNativeStatus (*MermanNativeEngineNewWithServicesFn)(
    const MermanNativeEngineServicesConfig *config,
    MermanNativeEngineToken *out_engine,
    MermanNativeResult *out_result
);

typedef struct MermanNativeApi {
    uint32_t struct_size;
    uint32_t abi_version;
    MermanNativeSlice minimum_prefix_layout_digest;
    MermanNativeSlice full_descriptor_digest;
    MermanNativeSlice capability_catalog_digest;
    MermanNativeSlice package_version;
    MermanNativeRuntimeCatalogFn runtime_catalog;
    MermanNativeEngineNewFn engine_new;
    MermanNativeEngineTryCloseFn engine_try_close;
    MermanNativeExecuteCollectFn execute_collect;
    MermanNativeResultFreeFn result_free;
    MermanNativeMetadataCollectFn metadata_collect;
    MermanNativeEngineNewWithServicesFn engine_new_with_services;
} MermanNativeApi;

typedef MermanNativeStatus (*MermanNativeGetApiFn)(
    const MermanNativeApiRequest *request,
    MermanNativeApi *out_api
);

#define MERMAN_NATIVE_ABI_VERSION 3u
#define MERMAN_NATIVE_API_MINIMUM_PREFIX_LAYOUT_DIGEST \
    "sha256:623c099f91282a88bf4d4e9cc7cdf728fc39c3b71a3ae7392007dd74f2b6ab41"

int merman_alpha5_consumer_smoke(MermanNativeGetApiFn discover) {
    static const char digest[] = MERMAN_NATIVE_API_MINIMUM_PREFIX_LAYOUT_DIGEST;
    MermanNativeApiRequest request = {0};
    MermanNativeApi api = {0};
    MermanNativeResult result = {0};

    request.struct_size = (uint32_t)sizeof(request);
    request.expected_abi_version = MERMAN_NATIVE_ABI_VERSION;
    request.expected_minimum_prefix_layout_digest.struct_size =
        (uint32_t)sizeof(MermanNativeSlice);
    request.expected_minimum_prefix_layout_digest.data = (const uint8_t *)digest;
    request.expected_minimum_prefix_layout_digest.len = sizeof(digest) - 1;
    api.struct_size = (uint32_t)sizeof(api);

    if (discover == NULL || discover(&request, &api) != MERMAN_NATIVE_STATUS_OK) {
        return 1;
    }
    if (api.struct_size != (uint32_t)sizeof(api) || api.abi_version != MERMAN_NATIVE_ABI_VERSION) {
        return 2;
    }
    if (api.runtime_catalog == NULL || api.engine_new == NULL ||
        api.engine_try_close == NULL || api.execute_collect == NULL ||
        api.result_free == NULL || api.metadata_collect == NULL ||
        api.engine_new_with_services == NULL) {
        return 3;
    }

    result.struct_size = (uint32_t)sizeof(result);
    if (api.runtime_catalog(&result) != MERMAN_NATIVE_STATUS_OK) {
        return 4;
    }
    if (result.struct_size != (uint32_t)sizeof(result) || result.allocation_token == 0) {
        return 5;
    }
    api.result_free(&result);
    return result.allocation_token == 0 && result.data.data == NULL &&
                   result.metadata_or_error_json.data == NULL
               ? 0
               : 6;
}
