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

static MermanNativeStatus measure_text(
    const MermanNativeTextMeasureRequest *request,
    MermanNativeTextMeasureResult *out_result,
    void *user_data
) {
    (void)request;
    (void)user_data;

    if (out_result == NULL) {
        return MERMAN_NATIVE_STATUS_INVALID_ARGUMENT;
    }
    memset(out_result, 0, sizeof(*out_result));
    out_result->struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeTextMeasureResult);

    /*
     * Returning handled = 0 asks Merman to use its deterministic fallback. Real preview hosts should
     * fill the result for only the operations they can answer from their display font stack.
     */
    out_result->handled = 0;
    return MERMAN_NATIVE_STATUS_OK;
}

int main(void) {
    static const uint8_t source[] =
        "flowchart TD\nA@{ icon: \"example:rocket\", label: \"Hello\" } --> B[World]";
    static const uint8_t icon_pack_json[] =
        "{\"prefix\":\"example\",\"icons\":{\"rocket\":{\"body\":"
        "\"<path data-icon=\\\"example-registry\\\" d=\\\"M0 0H16V16H0z\\\"/>\"}}}";
    MermanNativeApiRequest discovery;
    MermanNativeApi api;
    MermanNativeEngineConfig config;
    MermanNativeIconPack icon_pack;
    MermanNativeEngineServicesConfig services_config;
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
    if (
        merman_get_native_api(&discovery, &api) != MERMAN_NATIVE_STATUS_OK ||
        api.engine_new_with_services == NULL
    ) {
        return 1;
    }

    memset(&config, 0, sizeof(config));
    config.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeEngineConfig);
    config.options_json = borrowed_slice(NULL, 0);
    config.text_measure = measure_text;
    config.text_measure_user_data = NULL;
    memset(&icon_pack, 0, sizeof(icon_pack));
    icon_pack.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeIconPack);
    icon_pack.json = borrowed_slice(icon_pack_json, sizeof(icon_pack_json) - 1);
    icon_pack.registration_name = borrowed_slice(NULL, 0);
    memset(&services_config, 0, sizeof(services_config));
    services_config.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeEngineServicesConfig);
    services_config.engine_config = config;
    services_config.icon_packs = &icon_pack;
    services_config.icon_pack_count = 1;
    result = (MermanNativeResult)MERMAN_NATIVE_RESULT_INIT;
    if (
        api.engine_new_with_services(&services_config, &engine, &result) !=
        MERMAN_NATIVE_STATUS_OK
    ) {
        api.result_free(&result);
        return 1;
    }
    api.result_free(&result);

    memset(&request, 0, sizeof(request));
    request.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeOperationRequest);
    request.operation = MERMAN_NATIVE_OPERATION_SVG;
    request.source = borrowed_slice(source, sizeof(source) - 1);
    request.uri = borrowed_slice(NULL, 0);
    request.options_json = borrowed_slice(NULL, 0);
    result = (MermanNativeResult)MERMAN_NATIVE_RESULT_INIT;
    if (api.execute_collect(engine, &request, &result) != MERMAN_NATIVE_STATUS_OK) {
        api.result_free(&result);
        api.engine_try_close(engine);
        return 1;
    }

    fwrite(result.data.data, 1, result.data.len, stdout);
    fputc('\n', stdout);
    api.result_free(&result);
    return api.engine_try_close(engine) == MERMAN_NATIVE_STATUS_OK ? 0 : 1;
}
