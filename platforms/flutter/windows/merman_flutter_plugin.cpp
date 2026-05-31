// Flutter plugin registration for Merman on Windows.
//
// All rendering is done via Dart FFI. The plugin DLL is a registration stub;
// merman_ffi.dll is bundled beside it by CMake.

#include "merman/merman_flutter_plugin.h"

#include <flutter/plugin_registrar_windows.h>

namespace {

class MermanFlutterPlugin : public flutter::Plugin {
 public:
  static void RegisterWithRegistrar(flutter::PluginRegistrarWindows* registrar) {
    // FFI-only plugin: no channels to register.
  }

  MermanFlutterPlugin() = default;
  virtual ~MermanFlutterPlugin() = default;
  MermanFlutterPlugin(const MermanFlutterPlugin&) = delete;
  MermanFlutterPlugin& operator=(const MermanFlutterPlugin&) = delete;
};

}  // namespace

void MermanFlutterPluginRegisterWithRegistrar(
    FlutterDesktopPluginRegistrarRef registrar) {
  MermanFlutterPlugin::RegisterWithRegistrar(
      flutter::PluginRegistrarManager::GetInstance()
          ->GetRegistrar<flutter::PluginRegistrarWindows>(registrar));
}
