#include "merman.h"

#include <filesystem>
#include <fstream>
#include <iostream>
#include <stdexcept>
#include <string>
#include <string_view>

namespace {
MermanNativeSlice slice(std::string_view text) {
    MermanNativeSlice value{};
    value.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeSlice);
    value.data = text.empty() ? nullptr : reinterpret_cast<const uint8_t*>(text.data());
    value.len = text.size();
    return value;
}

void write(const std::filesystem::path& path, const MermanNativeBuffer& bytes) {
    std::ofstream stream(path, std::ios::binary);
    if (bytes.len != 0) {
        stream.write(reinterpret_cast<const char*>(bytes.data),
                     static_cast<std::streamsize>(bytes.len));
    }
    stream.close();
    if (!stream) throw std::runtime_error("cannot write " + path.string());
}

class Result {
public:
    explicit Result(const MermanNativeApi& api) : api_(api) {}
    ~Result() { api_.result_free(&value); }
    Result(const Result&) = delete;
    Result& operator=(const Result&) = delete;

    void check(MermanNativeStatus status) const {
        if (status == MERMAN_NATIVE_STATUS_OK && value.status == MERMAN_NATIVE_STATUS_OK) return;
        const auto& error = value.metadata_or_error_json;
        throw std::runtime_error(error.len == 0 ? "native ABI call failed" :
            std::string(reinterpret_cast<const char*>(error.data), error.len));
    }

    MermanNativeResult value = MERMAN_NATIVE_RESULT_INIT;

private:
    const MermanNativeApi& api_;
};

class Engine {
public:
    Engine() {
        MermanNativeApiRequest discovery{};
        discovery.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApiRequest);
        discovery.expected_abi_version = MERMAN_NATIVE_ABI_VERSION;
        discovery.expected_minimum_prefix_layout_digest =
            slice(MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST);
        api_.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeApi);
        if (merman_get_native_api(&discovery, &api_) != MERMAN_NATIVE_STATUS_OK ||
            api_.abi_version != MERMAN_NATIVE_ABI_VERSION ||
            api_.struct_size < MERMAN_NATIVE_API_ENGINE_NEW_WITH_FONTS_PREFIX_SIZE ||
            !api_.engine_new_with_fonts || !api_.execute_collect || !api_.runtime_catalog ||
            !api_.result_free || !api_.engine_try_close) {
            throw std::runtime_error("SDK does not provide the required native ABI 3 prefix");
        }
        MermanNativeFontEngineConfig config{};
        config.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeFontEngineConfig);
        config.options_json = slice(R"({"resources":{"profile":"interactive"}})");
        config.use_bundled_fonts = 1;
        Result result(api_);
        result.check(api_.engine_new_with_fonts(&config, &engine_, &result.value));
    }

    ~Engine() { if (engine_ != 0) api_.engine_try_close(engine_); }
    Engine(const Engine&) = delete;
    Engine& operator=(const Engine&) = delete;

    void catalog(const std::filesystem::path& path) const {
        Result result(api_);
        result.check(api_.runtime_catalog(&result.value));
        write(path, result.value.metadata_or_error_json);
    }

    void render(MermanNativeOperationCode operation, std::string_view source,
                std::string_view options, const std::filesystem::path& path) const {
        MermanNativeOperationRequest request{};
        request.struct_size = MERMAN_NATIVE_STRUCT_SIZE(MermanNativeOperationRequest);
        request.operation = operation;
        request.source = slice(source);
        request.options_json = slice(options);
        request.uri = slice("");
        Result result(api_);
        result.check(api_.execute_collect(engine_, &request, &result.value));
        write(path, result.value.data);
        write(path.string() + ".metadata.json", result.value.metadata_or_error_json);
    }

    void close() {
        if (api_.engine_try_close(engine_) != MERMAN_NATIVE_STATUS_OK)
            throw std::runtime_error("engine close failed");
        engine_ = 0;
    }

private:
    MermanNativeApi api_{};
    MermanNativeEngineToken engine_ = 0;
};
}  // namespace

int main(int argc, char** argv) {
    if (argc != 2) {
        std::cerr << "usage: merman-render-cpp OUTPUT_DIRECTORY\n";
        return 2;
    }
    try {
        const std::filesystem::path output(argv[1]);
        std::filesystem::create_directories(output);
        Engine engine;
        engine.catalog(output / "runtime-catalog.json");
        const auto source = "---\nconfig:\n  fontFamily: Cousine\n  flowchart:\n"
                            "    htmlLabels: false\n---\nflowchart LR\nA[SDK] --> B[C++]\n";
        engine.render(MERMAN_NATIVE_OPERATION_SVG, source,
                      R"({"svg":{"pipeline":"resvg-safe","diagram_id":"cpp-example"}})",
                      output / "diagram.svg");
        engine.render(MERMAN_NATIVE_OPERATION_PNG, source,
                      R"({"svg":{"pipeline":"resvg-safe","diagram_id":"cpp-example"},"raster":{"scale":2,"matte":"#ffffff"}})",
                      output / "diagram.png");
        engine.close();
        return 0;
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
