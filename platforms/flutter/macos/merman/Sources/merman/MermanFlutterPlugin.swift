import FlutterMacOS
import Foundation

/// Minimal Flutter plugin registration for merman on macOS.
///
/// Rendering is handled by Dart FFI. The vendored XCFramework links the native
/// library into the app process, so no method channels are needed.
public class MermanFlutterPlugin: NSObject, FlutterPlugin {
    public static func register(with registrar: FlutterPluginRegistrar) {
        // FFI-only plugin: no channels to register.
    }
}
