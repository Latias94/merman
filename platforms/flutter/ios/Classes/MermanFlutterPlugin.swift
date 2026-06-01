import Flutter
import UIKit

/// Minimal Flutter plugin registration for merman.
///
/// Rendering is handled by Dart FFI. The merman-ffi framework is linked through
/// the vendored XCFramework, so no method channels are needed.
public class MermanFlutterPlugin: NSObject, FlutterPlugin {
    public static func register(with registrar: FlutterPluginRegistrar) {
        // FFI-only plugin: no channels to register.
    }
}
