#include <napi.h>

typedef struct TSLanguage TSLanguage;

extern "C" const TSLanguage *tree_sitter_mermaid();

namespace {

const napi_type_tag LANGUAGE_TYPE_TAG = {
    0x8AF2E5212AD58ABF,
    0xD5006CAD83ABBA16,
};

Napi::Object Initialize(Napi::Env environment, Napi::Object exports) {
  exports["name"] = Napi::String::New(environment, "mermaid");
  auto language = Napi::External<TSLanguage>::New(
      environment, const_cast<TSLanguage *>(tree_sitter_mermaid()));
  language.TypeTag(&LANGUAGE_TYPE_TAG);
  exports["language"] = language;
  return exports;
}

}  // namespace

NODE_API_MODULE(tree_sitter_mermaid_binding, Initialize)
