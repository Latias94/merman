import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

/// C ABI version expected by this Dart binding.
const int mermanAbiVersion = 1;

/// Result status codes returned by the native `merman-ffi` ABI.
enum MermanStatus {
  /// The call completed successfully.
  ok(0, 'MERMAN_OK'),

  /// A pointer, length, or option value was invalid.
  invalidArgument(1, 'MERMAN_INVALID_ARGUMENT'),

  /// Source or options bytes were not valid UTF-8.
  utf8Error(2, 'MERMAN_UTF8_ERROR'),

  /// The `optionsJson` payload could not be parsed.
  optionsJsonError(3, 'MERMAN_OPTIONS_JSON_ERROR'),

  /// No Mermaid diagram was detected in the source.
  noDiagram(4, 'MERMAN_NO_DIAGRAM'),

  /// Mermaid parsing failed.
  parseError(5, 'MERMAN_PARSE_ERROR'),

  /// Layout, SVG rendering, or postprocessing failed.
  renderError(6, 'MERMAN_RENDER_ERROR'),

  /// The requested output is not enabled or not implemented.
  unsupportedFormat(7, 'MERMAN_UNSUPPORTED_FORMAT'),

  /// A Rust panic was caught at the ABI boundary.
  panic(8, 'MERMAN_PANIC'),

  /// An unexpected internal error occurred.
  internalError(9, 'MERMAN_INTERNAL_ERROR');

  const MermanStatus(this.code, this.codeName);

  /// Numeric status code used by the C ABI.
  final int code;

  /// Stable symbolic status name used in JSON error payloads.
  final String codeName;

  /// Returns the matching status for [code], or `null` if the code is unknown.
  static MermanStatus? fromCode(int code) {
    for (final status in values) {
      if (status.code == code) {
        return status;
      }
    }
    return null;
  }
}

/// Native layout of `MermanBuffer`.
///
/// This mirrors the C ABI struct and is exposed for ABI size checks.
final class NativeMermanBuffer extends Struct {
  /// Pointer to the native payload bytes, or null for an empty payload.
  external Pointer<Uint8> data;

  /// Payload length in bytes.
  @UintPtr()
  external int len;
}

/// Native layout of `MermanResult`.
///
/// This mirrors the C ABI struct and is exposed for ABI size checks.
final class NativeMermanResult extends Struct {
  /// Numeric [MermanStatus] code returned by the native call.
  @Int32()
  external int code;

  /// Native output or error payload.
  external NativeMermanBuffer data;
}

typedef _AbiVersionC = Uint32 Function();
typedef _AbiVersionDart = int Function();

typedef _PackageVersionC = Pointer<Utf8> Function();
typedef _PackageVersionDart = Pointer<Utf8> Function();

typedef _StructSizeC = UintPtr Function();
typedef _StructSizeDart = int Function();

typedef _MermanCallC = NativeMermanResult Function(
  Pointer<Uint8>,
  UintPtr,
  Pointer<Uint8>,
  UintPtr,
);
typedef _MermanCallDart = NativeMermanResult Function(
    Pointer<Uint8>, int, Pointer<Uint8>, int);

typedef _MermanMetadataC = NativeMermanResult Function();
typedef _MermanMetadataDart = NativeMermanResult Function();

typedef _BufferFreeC = Void Function(NativeMermanBuffer);
typedef _BufferFreeDart = void Function(NativeMermanBuffer);

/// Opens the bundled native `merman-ffi` library for the current platform.
///
/// Flutter applications normally use [Merman.open], which calls this helper.
DynamicLibrary openMermanLibrary() {
  if (Platform.isAndroid) {
    return DynamicLibrary.open('libmerman_ffi.so');
  }
  if (Platform.isIOS || Platform.isMacOS) {
    return DynamicLibrary.process();
  }
  if (Platform.isWindows) {
    return DynamicLibrary.open('merman_ffi.dll');
  }
  if (Platform.isLinux) {
    return DynamicLibrary.open('libmerman_ffi.so');
  }
  throw UnsupportedError('Unsupported platform: ${Platform.operatingSystem}');
}

/// Opens a native `merman-ffi` library at [path].
///
/// This is useful for local Dart smoke tests outside Flutter packaging.
DynamicLibrary openMermanLibraryFromPath(String path) =>
    DynamicLibrary.open(path);

/// Exception thrown when a native merman call returns an error status.
class MermanException implements Exception {
  /// Creates a merman exception from a native or Dart-side error payload.
  const MermanException({
    required this.code,
    required this.codeName,
    required this.message,
  });

  /// Numeric status code.
  final int code;

  /// Stable symbolic status name.
  final String codeName;

  /// Human-readable error message.
  final String message;

  @override
  String toString() => 'MermanException($codeName): $message';
}

/// Structured result returned by [Merman.validate].
class MermanValidationResult {
  /// Creates a validation result.
  const MermanValidationResult({
    required this.valid,
    required this.error,
    required this.code,
    required this.codeName,
  });

  /// Whether the source is a valid Mermaid diagram for this renderer.
  final bool valid;

  /// Validation error message, or `null` when [valid] is true.
  final String? error;

  /// Numeric merman status code represented by this validation result.
  final int code;

  /// Stable symbolic status name represented by this validation result.
  final String codeName;

  /// Decodes a validation payload produced by the native ABI.
  factory MermanValidationResult.fromJson(Map<String, Object?> json) {
    final valid = json['valid'];
    final code = json['code'];
    final codeName = json['code_name'];
    if (valid is! bool || code is! num || codeName is! String) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_JSON_TYPE_ERROR',
        message: 'expected validation JSON object',
      );
    }
    final error = json['error'];
    return MermanValidationResult(
      valid: valid,
      error: error is String ? error : null,
      code: code.toInt(),
      codeName: codeName,
    );
  }
}

/// High-level Dart wrapper around the native `merman-ffi` ABI.
class Merman {
  /// Creates an engine wrapper from an already-opened [DynamicLibrary].
  ///
  /// The constructor verifies ABI version and native struct sizes immediately.
  Merman.fromDynamicLibrary(DynamicLibrary library)
      : _bindings = _MermanBindings(library) {
    _bindings.checkAbi();
  }

  /// Opens the bundled native library for the current Flutter platform.
  factory Merman.open() => Merman.fromDynamicLibrary(openMermanLibrary());

  /// Opens a native library from [path].
  ///
  /// Use this for local smoke tests or custom native artifact placement.
  factory Merman.openPath(String path) =>
      Merman.fromDynamicLibrary(openMermanLibraryFromPath(path));

  final _MermanBindings _bindings;
  List<String>? _supportedDiagramsCache;
  List<String>? _asciiSupportedDiagramsCache;
  List<String>? _themesCache;
  List<String>? _hostThemePresetsCache;

  /// Native `merman-ffi` package version.
  String get packageVersion => _bindings.packageVersion();

  /// Renders Mermaid [source] to SVG text.
  ///
  /// [optionsJson] follows the shared merman bindings options schema.
  String renderSvg(String source, {String? optionsJson}) {
    return _decodeText(
      _bindings.call(_bindings.renderSvg, source, optionsJson),
    );
  }

  /// Renders Mermaid [source] to Unicode ASCII-art text.
  String renderAscii(String source, {String? optionsJson}) {
    return _decodeText(
      _bindings.call(_bindings.renderAscii, source, optionsJson),
    );
  }

  /// Parses Mermaid [source] and returns raw semantic JSON text.
  String parseJsonRaw(String source, {String? optionsJson}) {
    return _decodeText(
      _bindings.call(_bindings.parseJson, source, optionsJson),
    );
  }

  /// Parses Mermaid [source] and returns the semantic JSON object.
  Map<String, Object?> parseJson(String source, {String? optionsJson}) {
    return _decodeJsonMap(parseJsonRaw(source, optionsJson: optionsJson));
  }

  /// Lays out Mermaid [source] and returns raw layout JSON text.
  String layoutJsonRaw(String source, {String? optionsJson}) {
    return _decodeText(
      _bindings.call(_bindings.layoutJson, source, optionsJson),
    );
  }

  /// Lays out Mermaid [source] and returns the layout JSON object.
  Map<String, Object?> layoutJson(String source, {String? optionsJson}) {
    return _decodeJsonMap(layoutJsonRaw(source, optionsJson: optionsJson));
  }

  /// Validates Mermaid [source] and returns raw validation JSON text.
  String validateJsonRaw(String source, {String? optionsJson}) {
    return _decodeText(
      _bindings.call(_bindings.validateJson, source, optionsJson),
    );
  }

  /// Validates Mermaid [source] without throwing for ordinary parse errors.
  MermanValidationResult validate(String source, {String? optionsJson}) {
    return MermanValidationResult.fromJson(
      _decodeJsonMap(validateJsonRaw(source, optionsJson: optionsJson)),
    );
  }

  /// Returns diagram types exposed by the binding surface.
  List<String> supportedDiagrams() {
    return _supportedDiagramsCache ??= List.unmodifiable(
      _decodeJsonStringList(
        _decodeText(_bindings.metadata(_bindings.supportedDiagramsJson)),
      ),
    );
  }

  /// Returns diagram types currently supported by ASCII rendering.
  List<String> asciiSupportedDiagrams() {
    return _asciiSupportedDiagramsCache ??= List.unmodifiable(
      _decodeJsonStringList(
        _decodeText(_bindings.metadata(_bindings.asciiSupportedDiagramsJson)),
      ),
    );
  }

  /// Returns built-in Mermaid theme names.
  List<String> supportedThemes() {
    return _themesCache ??= List.unmodifiable(
      _decodeJsonStringList(
        _decodeText(_bindings.metadata(_bindings.supportedThemesJson)),
      ),
    );
  }

  /// Returns built-in host/editor theme preset names.
  List<String> supportedHostThemePresets() {
    return _hostThemePresetsCache ??= List.unmodifiable(
      _decodeJsonStringList(
        _decodeText(
          _bindings.metadata(_bindings.supportedHostThemePresetsJson),
        ),
      ),
    );
  }

  static String _decodeText(Uint8List bytes) => utf8.decode(bytes);

  static Map<String, Object?> _decodeJsonMap(String text) {
    final decoded = jsonDecode(text);
    if (decoded is Map<String, Object?>) {
      return decoded;
    }
    throw const MermanException(
      code: -1,
      codeName: 'DART_JSON_TYPE_ERROR',
      message: 'expected JSON object',
    );
  }

  static List<String> _decodeJsonStringList(String text) {
    final decoded = jsonDecode(text);
    if (decoded is List && decoded.every((item) => item is String)) {
      return decoded.cast<String>();
    }
    throw const MermanException(
      code: -1,
      codeName: 'DART_JSON_TYPE_ERROR',
      message: 'expected JSON string array',
    );
  }
}

class _MermanBindings {
  _MermanBindings(DynamicLibrary library)
      : _abiVersion = library.lookupFunction<_AbiVersionC, _AbiVersionDart>(
          'merman_abi_version',
        ),
        _packageVersion =
            library.lookupFunction<_PackageVersionC, _PackageVersionDart>(
          'merman_package_version',
        ),
        _bufferStructSize =
            library.lookupFunction<_StructSizeC, _StructSizeDart>(
          'merman_buffer_struct_size',
        ),
        _resultStructSize =
            library.lookupFunction<_StructSizeC, _StructSizeDart>(
          'merman_result_struct_size',
        ),
        renderSvg = library.lookupFunction<_MermanCallC, _MermanCallDart>(
          'merman_render_svg',
        ),
        renderAscii = library.lookupFunction<_MermanCallC, _MermanCallDart>(
          'merman_render_ascii',
        ),
        parseJson = library.lookupFunction<_MermanCallC, _MermanCallDart>(
          'merman_parse_json',
        ),
        layoutJson = library.lookupFunction<_MermanCallC, _MermanCallDart>(
          'merman_layout_json',
        ),
        validateJson = library.lookupFunction<_MermanCallC, _MermanCallDart>(
          'merman_validate_json',
        ),
        supportedDiagramsJson =
            library.lookupFunction<_MermanMetadataC, _MermanMetadataDart>(
          'merman_supported_diagrams_json',
        ),
        asciiSupportedDiagramsJson =
            library.lookupFunction<_MermanMetadataC, _MermanMetadataDart>(
          'merman_ascii_supported_diagrams_json',
        ),
        supportedThemesJson =
            library.lookupFunction<_MermanMetadataC, _MermanMetadataDart>(
          'merman_supported_themes_json',
        ),
        supportedHostThemePresetsJson =
            library.lookupFunction<_MermanMetadataC, _MermanMetadataDart>(
          'merman_supported_host_theme_presets_json',
        ),
        _bufferFree = library.lookupFunction<_BufferFreeC, _BufferFreeDart>(
          'merman_buffer_free',
        );

  final _AbiVersionDart _abiVersion;
  final _PackageVersionDart _packageVersion;
  final _StructSizeDart _bufferStructSize;
  final _StructSizeDart _resultStructSize;
  final _BufferFreeDart _bufferFree;
  final _MermanCallDart renderSvg;
  final _MermanCallDart renderAscii;
  final _MermanCallDart parseJson;
  final _MermanCallDart layoutJson;
  final _MermanCallDart validateJson;
  final _MermanMetadataDart supportedDiagramsJson;
  final _MermanMetadataDart asciiSupportedDiagramsJson;
  final _MermanMetadataDart supportedThemesJson;
  final _MermanMetadataDart supportedHostThemePresetsJson;

  void checkAbi() {
    final abiVersion = _abiVersion();
    if (abiVersion != mermanAbiVersion) {
      throw MermanException(
        code: -1,
        codeName: 'DART_ABI_VERSION_MISMATCH',
        message: 'expected ABI $mermanAbiVersion, got $abiVersion',
      );
    }

    final bufferSize = _bufferStructSize();
    final resultSize = _resultStructSize();
    if (bufferSize != sizeOf<NativeMermanBuffer>()) {
      throw MermanException(
        code: -1,
        codeName: 'DART_BUFFER_SIZE_MISMATCH',
        message: 'expected ${sizeOf<NativeMermanBuffer>()}, got $bufferSize',
      );
    }
    if (resultSize != sizeOf<NativeMermanResult>()) {
      throw MermanException(
        code: -1,
        codeName: 'DART_RESULT_SIZE_MISMATCH',
        message: 'expected ${sizeOf<NativeMermanResult>()}, got $resultSize',
      );
    }
  }

  String packageVersion() => _packageVersion().toDartString();

  Uint8List call(_MermanCallDart function, String source, String? optionsJson) {
    final sourceBytes = utf8.encode(source);
    final optionsBytes = optionsJson == null ? null : utf8.encode(optionsJson);
    final sourcePtr = _copyBytes(sourceBytes);
    final optionsPtr =
        optionsBytes == null ? nullptr : _copyBytes(optionsBytes);

    try {
      final result = function(
        sourcePtr,
        sourceBytes.length,
        optionsPtr,
        optionsBytes?.length ?? 0,
      );
      final payload = _takeBuffer(result.data);
      if (result.code == MermanStatus.ok.code) {
        return payload;
      }
      throw _exceptionFromPayload(result.code, payload);
    } finally {
      _freeIfAllocated(sourcePtr);
      _freeIfAllocated(optionsPtr);
    }
  }

  Uint8List metadata(_MermanMetadataDart function) {
    final result = function();
    final payload = _takeBuffer(result.data);
    if (result.code == MermanStatus.ok.code) {
      return payload;
    }
    throw _exceptionFromPayload(result.code, payload);
  }

  Pointer<Uint8> _copyBytes(List<int> bytes) {
    if (bytes.isEmpty) {
      return nullptr;
    }
    final pointer = calloc<Uint8>(bytes.length);
    pointer.asTypedList(bytes.length).setAll(0, bytes);
    return pointer;
  }

  Uint8List _takeBuffer(NativeMermanBuffer buffer) {
    if (buffer.data.address == 0 || buffer.len == 0) {
      return Uint8List(0);
    }
    final bytes = Uint8List.fromList(buffer.data.asTypedList(buffer.len));
    _bufferFree(buffer);
    return bytes;
  }

  MermanException _exceptionFromPayload(int code, Uint8List payload) {
    final status = MermanStatus.fromCode(code);
    final text =
        payload.isEmpty ? '' : utf8.decode(payload, allowMalformed: true);
    try {
      final decoded = jsonDecode(text);
      if (decoded is Map<String, Object?>) {
        return MermanException(
          code: code,
          codeName: decoded['code_name'] as String? ??
              status?.codeName ??
              'MERMAN_ERROR',
          message: decoded['message'] as String? ?? text,
        );
      }
    } catch (_) {
      // Fall back to the raw payload below.
    }
    return MermanException(
      code: code,
      codeName: status?.codeName ?? 'MERMAN_ERROR',
      message: text,
    );
  }

  void _freeIfAllocated(Pointer<Uint8> pointer) {
    if (pointer.address != 0) {
      calloc.free(pointer);
    }
  }
}
