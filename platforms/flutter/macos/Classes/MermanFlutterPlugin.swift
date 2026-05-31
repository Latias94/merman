import FlutterMacOS
import Foundation

/// Minimal Flutter plugin registration for merman on macOS.
///
/// Rendering is handled by Dart FFI. The vendored dylib is linked into the app
/// process by CocoaPods, so no method channels are needed.
public class MermanFlutterPlugin: NSObject, FlutterPlugin {
    public static func register(with registrar: FlutterPluginRegistrar) {
        // FFI-only plugin: no channels to register.
    }
}
