#include "merman.h"

#include <stdint.h>
#include <stdio.h>
#include <string.h>

static MermanNativeSlice borrowed_slice(const uint8_t *data, size_t len) {
    MermanNativeSlice slice;
    slice.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice);
    slice.data = data;
    slice.len = len;
    return slice;
}

static void print_failure(const char *operation, const MermanNativeResult *result) {
    fprintf(stderr, "%s failed (status %d): ", operation, result->status);
    if (result->metadata_or_error_json.data != NULL) {
        fwrite(
            result->metadata_or_error_json.data,
            1,
            result->metadata_or_error_json.len,
            stderr
        );
    }
    fputc('\n', stderr);
}

int main(void) {
    static const uint8_t source[] = "flowchart TD\nA[Hello] --> B[World]";
    static const uint8_t request_options[] =
        "{\"svg\":{\"diagram_id\":\"c-request\"}}";
    MermanNativeApiRequest discovery;
    MermanNativeApi api;
    MermanNativeEngineConfig config;
    MermanNativeOperationRequest request;
    MermanNativeResult result;
    MermanNativeEngineToken engine = 0;

    memset(&discovery, 0, sizeof(discovery));
    discovery.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApiRequest);
    discovery.expected_abi_version = MERMAN_NATIVE_ABI_VERSION;
    discovery.expected_minimum_prefix_layout_digest = borrowed_slice(
        (const uint8_t *)MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST,
        strlen(MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST)
    );
    memset(&api, 0, sizeof(api));
    api.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi);
    if (merman_get_native_api(&discovery, &api) != MERMAN_NATIVE_STATUS_OK) {
        fputs("Merman ABI 3 discovery failed\n", stderr);
        return 1;
    }

    memset(&config, 0, sizeof(config));
    config.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeEngineConfig);
    config.options_json = borrowed_slice(NULL, 0);
    result = (MermanNativeResult)MERMAN_NATIVE_RESULT_INIT;
    if (api.engine_new(&config, &engine, &result) != MERMAN_NATIVE_STATUS_OK) {
        print_failure("engine creation", &result);
        api.result_free(&result);
        return 1;
    }
    api.result_free(&result);

    memset(&request, 0, sizeof(request));
    request.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeOperationRequest);
    request.operation = MERMAN_NATIVE_OPERATION_SVG;
    request.source = borrowed_slice(source, sizeof(source) - 1);
    request.uri = borrowed_slice(NULL, 0);
    request.options_json = borrowed_slice(
        request_options,
        sizeof(request_options) - 1
    );
    result = (MermanNativeResult)MERMAN_NATIVE_RESULT_INIT;
    if (api.execute_collect(engine, &request, &result) != MERMAN_NATIVE_STATUS_OK) {
        print_failure("SVG render", &result);
        api.result_free(&result);
        api.engine_try_close(engine);
        return 1;
    }

    fwrite(result.data.data, 1, result.data.len, stdout);
    fputc('\n', stdout);
    api.result_free(&result);
    return api.engine_try_close(engine) == MERMAN_NATIVE_STATUS_OK ? 0 : 1;
}
