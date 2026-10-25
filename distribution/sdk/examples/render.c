#include "merman.h"

#include <stdio.h>
#include <string.h>

static MermanNativeSlice text_slice(const char *text) {
    MermanNativeSlice value = {0};
    value.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice);
    value.data = text[0] == '\0' ? NULL : (const uint8_t *)text;
    value.len = strlen(text);
    return value;
}

int main(int argc, char **argv) {
    MermanNativeApiRequest discovery = {0};
    MermanNativeApi api = {0};
    MermanNativeResult result = MERMAN_NATIVE_RESULT_INIT;
    MermanNativeFontEngineConfig config = {0};
    MermanNativeOperationRequest request = {0};
    MermanNativeEngineToken engine = 0;
    MermanNativeStatus status;
    FILE *output;
    int failed = 0;

    if (argc != 1 && (argc != 3 || strcmp(argv[1], "--catalog-out") != 0)) {
        fprintf(stderr, "usage: merman-render [--catalog-out runtime-catalog.json]\n");
        return 2;
    }
    discovery.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApiRequest);
    discovery.expected_abi_version = MERMAN_NATIVE_ABI_VERSION;
    discovery.expected_minimum_prefix_layout_digest = text_slice(
        MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST);
    api.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi);
    if (merman_get_native_api(&discovery, &api) != MERMAN_NATIVE_STATUS_OK ||
        api.abi_version != MERMAN_NATIVE_ABI_VERSION ||
        api.runtime_catalog == NULL || api.engine_new_with_fonts == NULL ||
        api.execute_collect == NULL || api.result_free == NULL ||
        api.engine_try_close == NULL) {
        fprintf(stderr, "C ABI discovery failed\n");
        return 1;
    }
    status = api.runtime_catalog(&result);
    if (status != MERMAN_NATIVE_STATUS_OK || result.status != MERMAN_NATIVE_STATUS_OK ||
        result.metadata_or_error_json.len == 0 || result.metadata_or_error_json.data == NULL) {
        api.result_free(&result);
        return 1;
    }
    if (argc == 3) {
        output = fopen(argv[2], "wb");
        if (output == NULL) {
            api.result_free(&result);
            return 1;
        }
        failed = fwrite(result.metadata_or_error_json.data, 1,
                        result.metadata_or_error_json.len, output) != result.metadata_or_error_json.len;
        if (fclose(output) != 0) failed = 1;
    }
    api.result_free(&result);
    if (failed) return 1;

    config.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeFontEngineConfig);
    config.options_json = text_slice("{}");
    config.use_bundled_fonts = 1;
    result = (MermanNativeResult)MERMAN_NATIVE_RESULT_INIT;
    status = api.engine_new_with_fonts(&config, &engine, &result);
    failed = status != MERMAN_NATIVE_STATUS_OK || result.status != MERMAN_NATIVE_STATUS_OK || engine == 0;
    api.result_free(&result);
    if (failed) return 1;

    request.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeOperationRequest);
    request.operation = MERMAN_NATIVE_OPERATION_SVG;
    request.source = text_slice("flowchart LR\n  A[SDK] --> B[SVG]\n");
    request.uri = text_slice("");
    request.options_json = text_slice("{}");
    result = (MermanNativeResult)MERMAN_NATIVE_RESULT_INIT;
    status = api.execute_collect(engine, &request, &result);
    failed = status != MERMAN_NATIVE_STATUS_OK || result.status != MERMAN_NATIVE_STATUS_OK ||
             result.data.len == 0 || result.data.data == NULL;
    if (!failed) {
        failed = fwrite(result.data.data, 1, result.data.len, stdout) != result.data.len;
    }
    api.result_free(&result);
    status = api.engine_try_close(engine);
    if (status != MERMAN_NATIVE_STATUS_OK) failed = 1;
    return failed ? 1 : 0;
}
