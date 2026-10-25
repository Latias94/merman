#include "merman.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct Bytes {
    unsigned char *data;
    size_t len;
} Bytes;

static MermanNativeSlice slice(const void *data, size_t len) {
    MermanNativeSlice value = {0};
    value.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice);
    value.data = (const uint8_t *)data;
    value.len = len;
    return value;
}

static int read_file(const char *path, Bytes *bytes) {
    FILE *file = fopen(path, "rb");
    long length;
    int ok;
    if (file == NULL) return 0;
    if (fseek(file, 0, SEEK_END) != 0 || (length = ftell(file)) < 0 ||
        length > 64L * 1024L * 1024L || fseek(file, 0, SEEK_SET) != 0) {
        fclose(file);
        return 0;
    }
    bytes->len = (size_t)length;
    bytes->data = (unsigned char *)malloc(bytes->len == 0 ? 1 : bytes->len);
    if (bytes->data == NULL) {
        fclose(file);
        return 0;
    }
    ok = fread(bytes->data, 1, bytes->len, file) == bytes->len;
    if (fclose(file) != 0) ok = 0;
    if (!ok) {
        free(bytes->data);
        bytes->data = NULL;
    }
    return ok;
}

static int write_file(const char *path, const void *data, size_t len) {
    FILE *file = fopen(path, "wb");
    int ok;
    if (file == NULL) return 0;
    ok = len == 0 || fwrite(data, 1, len, file) == len;
    if (fclose(file) != 0) ok = 0;
    return ok;
}

static int read_font(const char *directory, const char *name, Bytes *bytes) {
    size_t length = strlen(directory) + strlen(name) + 2;
    char *path = (char *)malloc(length);
    int ok;
    if (path == NULL) return 0;
    snprintf(path, length, "%s/%s", directory, name);
    ok = read_file(path, bytes);
    free(path);
    return ok;
}

int main(int argc, char **argv) {
    const char *font_names[3] = {"Cousine-Regular.ttf", "Cousine-Bold.ttf", "Cousine-Italic.ttf"};
    Bytes fonts[3] = {{0}, {0}, {0}};
    MermanNativeSlice font_slices[3];
    Bytes source = {0}, options = {0};
    MermanNativeApiRequest discovery = {0};
    MermanNativeApi api = {0};
    MermanNativeFontEngineConfig config = {0};
    MermanNativeOperationRequest request = {0};
    MermanNativeResult result = MERMAN_NATIVE_RESULT_INIT;
    MermanNativeEngineToken engine = 0;
    MermanNativeStatus status;
    int exit_code = 1;
    size_t index;

    if (argc != 8 || (strcmp(argv[4], "svg") != 0 && strcmp(argv[4], "png") != 0)) {
        fprintf(stderr, "usage: merman-sdk-probe FONT_DIRECTORY SOURCE OPTIONS svg|png OUTPUT METADATA CATALOG\n");
        return 2;
    }
    discovery.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApiRequest);
    discovery.expected_abi_version = MERMAN_NATIVE_ABI_VERSION;
    discovery.expected_minimum_prefix_layout_digest = slice(
        MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST,
        strlen(MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST));
    api.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi);
    if (merman_get_native_api(&discovery, &api) != MERMAN_NATIVE_STATUS_OK ||
        api.struct_size < MERMAN_NATIVE_API_ENGINE_NEW_WITH_FONTS_PREFIX_SIZE ||
        api.engine_new_with_fonts == NULL || api.runtime_catalog == NULL ||
        api.result_free == NULL || api.execute_collect == NULL || api.engine_try_close == NULL) {
        fprintf(stderr, "SDK font-constructor ABI discovery failed\n");
        return 1;
    }
    status = api.runtime_catalog(&result);
    if (status != MERMAN_NATIVE_STATUS_OK || result.status != MERMAN_NATIVE_STATUS_OK ||
        !write_file(argv[7], result.metadata_or_error_json.data, result.metadata_or_error_json.len)) {
        goto cleanup;
    }
    api.result_free(&result);
    result = (MermanNativeResult)MERMAN_NATIVE_RESULT_INIT;
    if (!read_file(argv[2], &source) || !read_file(argv[3], &options)) goto cleanup;
    for (index = 0; index < 3; ++index) {
        if (!read_font(argv[1], font_names[index], &fonts[index])) goto cleanup;
        font_slices[index] = slice(fonts[index].data, fonts[index].len);
    }
    config.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeFontEngineConfig);
    config.options_json = slice(options.data, options.len);
    config.fonts = font_slices;
    config.font_count = 3;
    config.use_bundled_fonts = 0;
    status = api.engine_new_with_fonts(&config, &engine, &result);

    /* Construction must copy caller-owned fonts and options. */
    for (index = 0; index < 3; ++index) {
        free(fonts[index].data);
        fonts[index].data = NULL;
    }
    free(options.data);
    options.data = NULL;
    if (status != MERMAN_NATIVE_STATUS_OK || result.status != MERMAN_NATIVE_STATUS_OK) {
        if (write_file(argv[6], result.metadata_or_error_json.data, result.metadata_or_error_json.len)) {
            exit_code = 3;
        }
        goto cleanup;
    }
    api.result_free(&result);
    result = (MermanNativeResult)MERMAN_NATIVE_RESULT_INIT;
    request.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeOperationRequest);
    request.operation = strcmp(argv[4], "svg") == 0 ? MERMAN_NATIVE_OPERATION_SVG : MERMAN_NATIVE_OPERATION_PNG;
    request.source = slice(source.data, source.len);
    request.uri = slice(NULL, 0);
    request.options_json = slice(NULL, 0);
    status = api.execute_collect(engine, &request, &result);
    free(source.data);
    source.data = NULL;

    /* Result bytes remain valid after both the input and engine are released. */
    if (api.engine_try_close(engine) != MERMAN_NATIVE_STATUS_OK) goto cleanup;
    engine = 0;
    if (!write_file(argv[6], result.metadata_or_error_json.data, result.metadata_or_error_json.len)) goto cleanup;
    if (status != MERMAN_NATIVE_STATUS_OK || result.status != MERMAN_NATIVE_STATUS_OK) {
        exit_code = 3;
    } else if (write_file(argv[5], result.data.data, result.data.len)) {
        exit_code = 0;
    }

cleanup:
    api.result_free(&result);
    if (engine != 0) api.engine_try_close(engine);
    free(source.data);
    free(options.data);
    for (index = 0; index < 3; ++index) free(fonts[index].data);
    if (exit_code == 1) fprintf(stderr, "SDK probe failed outside the ordinary result contract\n");
    return exit_code;
}
