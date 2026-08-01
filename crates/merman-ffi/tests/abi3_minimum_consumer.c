#include "merman.h"

#include <stddef.h>
#include <stdint.h>
#include <string.h>

typedef MermanNativeStatus (*MermanGetNativeApiFn)(
    const MermanNativeApiRequest *request,
    MermanNativeApi *out_api
);

typedef struct MermanNativeApiMinimumBuffer {
    /* `merman.h` in this fixture is the published five-slot ABI 3 header. */
    MermanNativeApi api;
    uint8_t trailing_guard[16];
} MermanNativeApiMinimumBuffer;

static MermanNativeSlice borrowed_slice(const uint8_t *data, size_t len) {
    MermanNativeSlice slice;
    slice.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice);
    slice.data = data;
    slice.len = len;
    return slice;
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

#if defined(_WIN32)
__declspec(dllexport)
#else
__attribute__((visibility("default")))
#endif
int merman_abi3_minimum_consumer_smoke(MermanGetNativeApiFn get_native_api) {
    MermanNativeApiRequest request;
    MermanNativeApiMinimumBuffer api_buffer;
    MermanNativeApi *api = &api_buffer.api;
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
        api->struct_size != MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi)
    ) {
        return 2;
    }
    /* The returned size is safe to reuse as the next discovery capacity. */
    if (
        get_native_api(&request, api) != MERMAN_NATIVE_STATUS_OK ||
        api->struct_size != MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi) ||
        !guard_is_intact(api_buffer.trailing_guard, sizeof(api_buffer.trailing_guard))
    ) {
        return 2;
    }
    if (
        api->abi_version != MERMAN_NATIVE_ABI_VERSION ||
        api->minimum_prefix_layout_digest.data == NULL ||
        api->full_descriptor_digest.data == NULL ||
        api->capability_catalog_digest.data == NULL ||
        api->runtime_catalog == NULL ||
        api->engine_new == NULL ||
        api->engine_try_close == NULL ||
        api->execute_collect == NULL ||
        api->result_free == NULL
    ) {
        return 3;
    }

    memset(&result, 0, sizeof(result));
    result.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeResult);
    if (
        api->runtime_catalog(&result) != MERMAN_NATIVE_STATUS_OK ||
        result.status != MERMAN_NATIVE_STATUS_OK ||
        result.operation != MERMAN_NATIVE_OPERATION_NONE ||
        result.allocation_token == 0
    ) {
        api->result_free(&result);
        return 4;
    }
    api->result_free(&result);
    if (result.allocation_token != 0) {
        return 5;
    }

    return 0;
}
