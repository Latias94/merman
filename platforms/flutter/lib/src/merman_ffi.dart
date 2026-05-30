import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

const int mermanAbiVersion = 1;

enum MermanStatus {
  ok(0, 'MERMAN_OK'),
  invalidArgument(1, 'MERMAN_INVALID_ARGUMENT'),
  utf8Error(2, 'MERMAN_UTF8_ERROR'),
  optionsJsonError(3, 'MERMAN_OPTIONS_JSON_ERROR'),
  noDiagram(4, 'MERMAN_NO_DIAGRAM'),
  parseError(5, 'MERMAN_PARSE_ERROR'),
  renderError(6, 'MERMAN_RENDER_ERROR'),
  unsupportedFormat(7, 'MERMAN_UNSUPPORTED_FORMAT'),
  panic(8, 'MERMAN_PANIC'),
  internalError(9, 'MERMAN_INTERNAL_ERROR');

  const MermanStatus(this.code, this.codeName);

  final int code;
  final String codeName;

  static MermanStatus? fromCode(int code) {
    for (final status in values) {
      if (status.code == code) {
        return status;
      }
    }
    return null;
  }
}

final class NativeMermanBuffer extends Struct {
  external Pointer<Uint8> data;

  @UintPtr()
  external int len;
}

final class NativeMermanResult extends Struct {
  @Int32()
  external int code;

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

typedef _BufferFreeC = Void Function(NativeMermanBuffer);
typedef _BufferFreeDart = void Function(NativeMermanBuffer);

DynamicLibrary openMermanLibrary([String? path]) {
  if (path != null) {
    return DynamicLibrary.open(path);
  }
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

class MermanException implements Exception {
  const MermanException({
    required this.code,
    required this.codeName,
    required this.message,
  });

  final int code;
  final String codeName;
  final String message;

  @override
  String toString() => 'MermanException($codeName): $message';
}

class Merman {
  Merman(DynamicLibrary library) : _bindings = _MermanBindings(library) {
    _bindings.checkAbi();
  }

  factory Merman.open([String? path]) => Merman(openMermanLibrary(path));

  final _MermanBindings _bindings;

  String get packageVersion => _bindings.packageVersion();

  String renderSvg(String source, {String? optionsJson}) {
    return _decodeText(
      _bindings.call(_bindings.renderSvg, source, optionsJson),
    );
  }

  String parseJsonRaw(String source, {String? optionsJson}) {
    return _decodeText(
      _bindings.call(_bindings.parseJson, source, optionsJson),
    );
  }

  Map<String, Object?> parseJson(String source, {String? optionsJson}) {
    return _decodeJsonMap(parseJsonRaw(source, optionsJson: optionsJson));
  }

  String layoutJsonRaw(String source, {String? optionsJson}) {
    return _decodeText(
      _bindings.call(_bindings.layoutJson, source, optionsJson),
    );
  }

  Map<String, Object?> layoutJson(String source, {String? optionsJson}) {
    return _decodeJsonMap(layoutJsonRaw(source, optionsJson: optionsJson));
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
        parseJson = library.lookupFunction<_MermanCallC, _MermanCallDart>(
          'merman_parse_json',
        ),
        layoutJson = library.lookupFunction<_MermanCallC, _MermanCallDart>(
          'merman_layout_json',
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
  final _MermanCallDart parseJson;
  final _MermanCallDart layoutJson;

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
