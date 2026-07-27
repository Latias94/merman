#ifndef MERMAN_ABI3_MINIMUM_CONSUMER_H
#define MERMAN_ABI3_MINIMUM_CONSUMER_H

#include <stddef.h>
#include <stdint.h>

#define MERMAN_NATIVE_ABI_VERSION 3u
#define MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST "sha256:26c9571ef2afa173aab5bd2562d1823f2d28c4cff5bbe9f9fdf4e3fc2b894a8d"
#define MERMAN_NATIVE_STRUCT_SIZE(type) ((uint32_t)sizeof(type))

typedef int32_t MermanNativeStatus;
enum {
    MERMAN_NATIVE_STATUS_OK = 0
};

typedef int32_t MermanNativeOperationCode;
enum {
    MERMAN_NATIVE_OPERATION_NONE = 0
};

typedef uint64_t MermanNativeEngineToken;

typedef struct MermanNativeSlice MermanNativeSlice;
typedef struct MermanNativeBuffer MermanNativeBuffer;
typedef struct MermanNativeTextMeasureRequest MermanNativeTextMeasureRequest;
typedef struct MermanNativeTextMeasureResult MermanNativeTextMeasureResult;
typedef struct MermanNativeEngineConfig MermanNativeEngineConfig;
typedef struct MermanNativeOperationRequest MermanNativeOperationRequest;
typedef struct MermanNativeResult MermanNativeResult;
typedef struct MermanNativeApiRequest MermanNativeApiRequest;
typedef struct MermanNativeApi MermanNativeApi;

typedef MermanNativeStatus (*MermanNativeTextMeasureCallback)(
    const MermanNativeTextMeasureRequest *request,
    MermanNativeTextMeasureResult *out_result,
    void *user_data
);
typedef MermanNativeStatus (*MermanNativeRuntimeCatalogFn)(
    MermanNativeResult *out_result
);
typedef MermanNativeStatus (*MermanNativeEngineNewFn)(
    const MermanNativeEngineConfig *config,
    MermanNativeEngineToken *out_engine,
    MermanNativeResult *out_result
);
typedef MermanNativeStatus (*MermanNativeEngineTryCloseFn)(
    MermanNativeEngineToken engine
);
typedef MermanNativeStatus (*MermanNativeExecuteCollectFn)(
    MermanNativeEngineToken engine,
    const MermanNativeOperationRequest *request,
    MermanNativeResult *out_result
);
typedef void (*MermanNativeResultFreeFn)(MermanNativeResult *result);

struct MermanNativeSlice {
    uint32_t struct_size;
    const uint8_t *data;
    size_t len;
};

struct MermanNativeBuffer {
    uint32_t struct_size;
    uint8_t *data;
    size_t len;
};

struct MermanNativeEngineConfig {
    uint32_t struct_size;
    MermanNativeSlice options_json;
    MermanNativeTextMeasureCallback text_measure;
    void *text_measure_user_data;
};

struct MermanNativeOperationRequest {
    uint32_t struct_size;
    MermanNativeOperationCode operation;
    MermanNativeSlice source;
    MermanNativeSlice uri;
    MermanNativeSlice options_json;
};

struct MermanNativeResult {
    uint32_t struct_size;
    uint64_t allocation_token;
    MermanNativeStatus status;
    MermanNativeOperationCode operation;
    MermanNativeSlice media_type;
    MermanNativeBuffer data;
    MermanNativeBuffer metadata_or_error_json;
};

struct MermanNativeApiRequest {
    uint32_t struct_size;
    uint32_t expected_abi_version;
    MermanNativeSlice expected_minimum_prefix_layout_digest;
};

struct MermanNativeApi {
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
};

MermanNativeStatus merman_get_native_api(
    const MermanNativeApiRequest *request,
    MermanNativeApi *out_api
);

#endif
