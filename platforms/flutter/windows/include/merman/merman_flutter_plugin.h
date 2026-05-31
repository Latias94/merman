// Public header for the Merman Flutter plugin on Windows.
#ifndef FLUTTER_PLUGIN_MERMAN_FLUTTER_PLUGIN_H_
#define FLUTTER_PLUGIN_MERMAN_FLUTTER_PLUGIN_H_

#include <flutter_plugin_registrar.h>

#ifdef FLUTTER_PLUGIN_IMPL
#define FLUTTER_PLUGIN_EXPORT __declspec(dllexport)
#else
#define FLUTTER_PLUGIN_EXPORT __declspec(dllimport)
#endif

#if defined(__cplusplus)
extern "C" {
#endif

FLUTTER_PLUGIN_EXPORT void MermanFlutterPluginRegisterWithRegistrar(
    FlutterDesktopPluginRegistrarRef registrar);

#if defined(__cplusplus)
}
#endif

#endif  // FLUTTER_PLUGIN_MERMAN_FLUTTER_PLUGIN_H_
