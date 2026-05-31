// Flutter plugin registration for Merman on Linux.
//
// All rendering is done via Dart FFI. The plugin shared object is a
// registration stub; libmerman_ffi.so is bundled beside it by CMake.

#include "merman/merman_flutter_plugin.h"

#include <flutter_linux/flutter_linux.h>

struct _MermanFlutterPlugin {
  GObject parent_instance;
};

G_DEFINE_TYPE(MermanFlutterPlugin, merman_flutter_plugin, g_object_get_type())

static void merman_flutter_plugin_dispose(GObject* object) {
  G_OBJECT_CLASS(merman_flutter_plugin_parent_class)->dispose(object);
}

static void merman_flutter_plugin_class_init(MermanFlutterPluginClass* klass) {
  G_OBJECT_CLASS(klass)->dispose = merman_flutter_plugin_dispose;
}

static void merman_flutter_plugin_init(MermanFlutterPlugin* self) {}

void merman_flutter_plugin_register_with_registrar(FlPluginRegistrar* registrar) {
  MermanFlutterPlugin* plugin = MERMAN_FLUTTER_PLUGIN(
      g_object_new(merman_flutter_plugin_get_type(), nullptr));
  g_object_unref(plugin);
}
