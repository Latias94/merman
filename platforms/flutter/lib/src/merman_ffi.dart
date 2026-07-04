import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

/// C ABI version expected by this Dart binding.
const int mermanAbiVersion = 2;

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
  internalError(9, 'MERMAN_INTERNAL_ERROR'),

  /// A source, layout-model, label, or SVG resource budget was exceeded.
  resourceLimitExceeded(10, 'MERMAN_RESOURCE_LIMIT_EXCEEDED');

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

/// Opaque reusable native engine handle.
///
/// Handles are created by `merman_engine_new` and must be freed with
/// `merman_engine_free`.
final class NativeMermanEngine extends Opaque {}

/// Native layout of `MermanEngineResult`.
final class NativeMermanEngineResult extends Struct {
  /// Numeric [MermanStatus] code returned by the native call.
  @Int32()
  external int code;

  /// Native reusable engine handle, or null when [code] is not OK.
  external Pointer<NativeMermanEngine> engine;

  /// Error payload when [code] is not OK.
  external NativeMermanBuffer data;
}

/// Native layout of `MermanHostTextMeasureRequest`.
final class NativeMermanHostTextMeasureRequest extends Struct {
  external Pointer<Uint8> text;

  @UintPtr()
  external int textLen;

  external Pointer<Uint8> fontFamily;

  @UintPtr()
  external int fontFamilyLen;

  @Double()
  external double fontSize;

  external Pointer<Uint8> fontWeight;

  @UintPtr()
  external int fontWeightLen;

  external Pointer<Uint8> fontStyle;

  @UintPtr()
  external int fontStyleLen;

  @Double()
  external double maxWidth;

  @Double()
  external double lineHeight;

  @Double()
  external double letterSpacing;

  @Double()
  external double wordSpacing;

  @Int32()
  external int wrapMode;

  @Int32()
  external int direction;

  @Int32()
  external int whiteSpace;

  @Uint8()
  external int hasMaxWidth;
}

/// Native layout of `MermanHostTextMeasureResult`.
final class NativeMermanHostTextMeasureResult extends Struct {
  @Uint8()
  external int handled;

  @Double()
  external double width;

  @Double()
  external double height;

  @UintPtr()
  external int lineCount;
}

/// Text wrapping mode requested by the native renderer.
enum MermanTextWrapMode {
  /// SVG-like text measurement.
  svgLike(0),

  /// SVG-like single-run text measurement.
  svgLikeSingleRun(1),

  /// HTML-label-like text measurement.
  htmlLike(2);

  const MermanTextWrapMode(this.code);

  /// Numeric C ABI value.
  final int code;

  static MermanTextWrapMode? fromCode(int code) {
    for (final value in values) {
      if (value.code == code) {
        return value;
      }
    }
    return null;
  }
}

/// Text direction requested by the native renderer.
enum MermanTextDirection {
  /// Let the host decide from content and context.
  auto(0),

  /// Left-to-right text.
  ltr(1),

  /// Right-to-left text.
  rtl(2);

  const MermanTextDirection(this.code);

  /// Numeric C ABI value.
  final int code;

  static MermanTextDirection? fromCode(int code) {
    for (final value in values) {
      if (value.code == code) {
        return value;
      }
    }
    return null;
  }
}

/// CSS-like white-space mode requested by the native renderer.
enum MermanTextWhiteSpace {
  /// CSS `normal` behavior.
  normal(0),

  /// CSS `nowrap` behavior.
  nowrap(1),

  /// CSS `break-spaces` behavior.
  breakSpaces(2),

  /// CSS `pre-wrap` behavior.
  preWrap(3);

  const MermanTextWhiteSpace(this.code);

  /// Numeric C ABI value.
  final int code;

  static MermanTextWhiteSpace? fromCode(int code) {
    for (final value in values) {
      if (value.code == code) {
        return value;
      }
    }
    return null;
  }
}

/// Dart representation of a native host text-measurement request.
class MermanTextMeasureRequest {
  MermanTextMeasureRequest._(NativeMermanHostTextMeasureRequest native)
      : text = _utf8Slice(native.text, native.textLen),
        fontFamily = _utf8Slice(native.fontFamily, native.fontFamilyLen),
        fontSize = native.fontSize,
        fontWeight = _utf8Slice(native.fontWeight, native.fontWeightLen),
        fontStyle = _utf8Slice(native.fontStyle, native.fontStyleLen),
        maxWidth = native.hasMaxWidth == 0 ? null : native.maxWidth,
        lineHeight = native.lineHeight,
        letterSpacing = native.letterSpacing,
        wordSpacing = native.wordSpacing,
        wrapMode = MermanTextWrapMode.fromCode(native.wrapMode),
        direction = MermanTextDirection.fromCode(native.direction),
        whiteSpace = MermanTextWhiteSpace.fromCode(native.whiteSpace);

  /// UTF-8 text to measure.
  final String text;

  /// CSS font-family string, or empty when unspecified.
  final String fontFamily;

  /// CSS font-size in pixels.
  final double fontSize;

  /// CSS font-weight string.
  final String fontWeight;

  /// CSS font-style string.
  final String fontStyle;

  /// Optional wrapping width in CSS pixels.
  final double? maxWidth;

  /// CSS line-height in pixels.
  final double lineHeight;

  /// CSS letter-spacing in pixels.
  final double letterSpacing;

  /// CSS word-spacing in pixels.
  final double wordSpacing;

  /// Requested wrapping behavior.
  final MermanTextWrapMode? wrapMode;

  /// Requested text direction.
  final MermanTextDirection? direction;

  /// Requested white-space behavior.
  final MermanTextWhiteSpace? whiteSpace;
}

/// Dart result for a handled text-measurement request.
class MermanTextMeasureResult {
  /// Creates a handled text-measurement result.
  const MermanTextMeasureResult({
    required this.width,
    required this.height,
    required this.lineCount,
  });

  /// Measured width in CSS pixels.
  final double width;

  /// Measured height in CSS pixels.
  final double height;

  /// Number of laid-out lines.
  final int lineCount;
}

/// Host text-measurement callback.
///
/// Return `null` for requests the host does not support; the native engine will
/// fall back to its vendored compatibility metrics for that request.
typedef MermanTextMeasurer = MermanTextMeasureResult? Function(
    MermanTextMeasureRequest request);

typedef _AbiVersionC = Uint32 Function();
typedef _AbiVersionDart = int Function();

typedef _PackageVersionC = Pointer<Utf8> Function();
typedef _PackageVersionDart = Pointer<Utf8> Function();

typedef _StructSizeC = UintPtr Function();
typedef _StructSizeDart = int Function();

typedef _EngineNewC = NativeMermanEngineResult Function(
    Pointer<Uint8>, UintPtr);
typedef _EngineNewDart = NativeMermanEngineResult Function(Pointer<Uint8>, int);

typedef _EngineFreeC = Void Function(Pointer<NativeMermanEngine>);
typedef _EngineFreeDart = void Function(Pointer<NativeMermanEngine>);

typedef _EngineCallC = NativeMermanResult Function(
  Pointer<NativeMermanEngine>,
  Pointer<Uint8>,
  UintPtr,
);
typedef _EngineCallDart = NativeMermanResult Function(
  Pointer<NativeMermanEngine>,
  Pointer<Uint8>,
  int,
);

typedef _EngineDocumentCallC = NativeMermanResult Function(
  Pointer<NativeMermanEngine>,
  Pointer<Uint8>,
  UintPtr,
  Pointer<Uint8>,
  UintPtr,
);
typedef _EngineDocumentCallDart = NativeMermanResult Function(
  Pointer<NativeMermanEngine>,
  Pointer<Uint8>,
  int,
  Pointer<Uint8>,
  int,
);

typedef _HostTextMeasureCallbackC = NativeMermanHostTextMeasureResult Function(
  NativeMermanHostTextMeasureRequest,
  Pointer<Void>,
);

typedef _EngineSetTextMeasureCallbackC = NativeMermanResult Function(
  Pointer<NativeMermanEngine>,
  Pointer<NativeFunction<_HostTextMeasureCallbackC>>,
  Pointer<Void>,
);
typedef _EngineSetTextMeasureCallbackDart = NativeMermanResult Function(
  Pointer<NativeMermanEngine>,
  Pointer<NativeFunction<_HostTextMeasureCallbackC>>,
  Pointer<Void>,
);

typedef _MermanCallC = NativeMermanResult Function(
  Pointer<Uint8>,
  UintPtr,
  Pointer<Uint8>,
  UintPtr,
);
typedef _MermanCallDart = NativeMermanResult Function(
    Pointer<Uint8>, int, Pointer<Uint8>, int);

typedef _MermanDocumentCallC = NativeMermanResult Function(
  Pointer<Uint8>,
  UintPtr,
  Pointer<Uint8>,
  UintPtr,
  Pointer<Uint8>,
  UintPtr,
);
typedef _MermanDocumentCallDart = NativeMermanResult Function(
  Pointer<Uint8>,
  int,
  Pointer<Uint8>,
  int,
  Pointer<Uint8>,
  int,
);

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

/// Evidence attached to an ASCII rendering capability record.
class MermanAsciiCapabilityEvidence {
  /// Creates an ASCII capability evidence record.
  const MermanAsciiCapabilityEvidence({
    required this.kind,
    required this.source,
    required this.note,
  });

  /// Evidence category, such as `local_advantage` or `beautiful_mermaid_prior_art`.
  final String kind;

  /// Source identifier used to trace the capability claim.
  final String source;

  /// Human-readable note for the evidence.
  final String note;

  /// Decodes an evidence object produced by the native ABI.
  factory MermanAsciiCapabilityEvidence.fromJson(Map<String, Object?> json) {
    final kind = json['kind'];
    final source = json['source'];
    final note = json['note'];
    if (kind is! String || source is! String || note is! String) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_JSON_TYPE_ERROR',
        message: 'expected ASCII capability evidence JSON object',
      );
    }
    return MermanAsciiCapabilityEvidence(
      kind: kind,
      source: source,
      note: note,
    );
  }
}

/// ASCII rendering capability for one Mermaid diagram type.
class MermanAsciiCapability {
  /// Creates an ASCII capability record.
  const MermanAsciiCapability({
    required this.diagramType,
    required this.displayName,
    required this.supportLevel,
    required this.summaryFallback,
    required this.supportedSemantics,
    required this.limits,
    required this.evidence,
  });

  /// Mermaid diagram type identifier.
  final String diagramType;

  /// Display name for host UI.
  final String displayName;

  /// Support level, currently `full`, `partial`, `summary`, or `unsupported`.
  final String supportLevel;

  /// Whether rendering may fall back to a structured summary for unsupported semantics.
  final bool summaryFallback;

  /// Semantics implemented by the ASCII renderer.
  final List<String> supportedSemantics;

  /// Known limits for the ASCII renderer.
  final List<String> limits;

  /// Source-backed evidence for this capability record.
  final List<MermanAsciiCapabilityEvidence> evidence;

  /// Decodes a capability object produced by the native ABI.
  factory MermanAsciiCapability.fromJson(Map<String, Object?> json) {
    final diagramType = json['diagram_type'];
    final displayName = json['display_name'];
    final supportLevel = json['support_level'];
    final summaryFallback = json['summary_fallback'];
    final supportedSemantics = json['supported_semantics'];
    final limits = json['limits'];
    final evidence = json['evidence'];
    if (diagramType is! String ||
        displayName is! String ||
        supportLevel is! String ||
        summaryFallback is! bool ||
        supportedSemantics is! List ||
        limits is! List ||
        evidence is! List ||
        !supportedSemantics.every((item) => item is String) ||
        !limits.every((item) => item is String)) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_JSON_TYPE_ERROR',
        message: 'expected ASCII capability JSON object',
      );
    }
    return MermanAsciiCapability(
      diagramType: diagramType,
      displayName: displayName,
      supportLevel: supportLevel,
      summaryFallback: summaryFallback,
      supportedSemantics: supportedSemantics.cast<String>(),
      limits: limits.cast<String>(),
      evidence: evidence.map((item) {
        if (item is Map<String, Object?>) {
          return MermanAsciiCapabilityEvidence.fromJson(item);
        }
        throw const MermanException(
          code: -1,
          codeName: 'DART_JSON_TYPE_ERROR',
          message: 'expected ASCII capability evidence JSON object',
        );
      }).toList(growable: false),
    );
  }
}

/// Parser/render capability for one Mermaid diagram family in the active native artifact.
class MermanDiagramFamilyCapability {
  /// Creates a diagram family capability record.
  const MermanDiagramFamilyCapability({
    required this.diagramType,
    required this.metadataId,
    required this.hasSemanticParser,
    required this.hasRenderParser,
  });

  /// Mermaid parser/detector id, including aliases such as `flowchart-v2`.
  final String diagramType;

  /// Public supported-diagram metadata id, when this family contributes one.
  final String? metadataId;

  /// Whether semantic JSON parsing is registered for [diagramType].
  final bool hasSemanticParser;

  /// Whether typed render-model parsing is registered for [diagramType].
  final bool hasRenderParser;

  /// Decodes a capability object produced by the native ABI.
  factory MermanDiagramFamilyCapability.fromJson(Map<String, Object?> json) {
    final diagramType = json['diagram_type'];
    final metadataId = json['metadata_id'];
    final hasSemanticParser = json['has_semantic_parser'];
    final hasRenderParser = json['has_render_parser'];
    if (diagramType is! String ||
        (metadataId != null && metadataId is! String) ||
        hasSemanticParser is! bool ||
        hasRenderParser is! bool) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_JSON_TYPE_ERROR',
        message: 'expected diagram family capability JSON object',
      );
    }
    return MermanDiagramFamilyCapability(
      diagramType: diagramType,
      metadataId: metadataId is String ? metadataId : null,
      hasSemanticParser: hasSemanticParser,
      hasRenderParser: hasRenderParser,
    );
  }
}

/// Public metadata for one lint rule exposed by the active native artifact.
class MermanLintRuleCatalogEntry {
  /// Creates a lint rule catalog entry.
  const MermanLintRuleCatalogEntry({
    required this.id,
    required this.description,
    required this.evidence,
    required this.defaultSeverity,
    required this.category,
    required this.defaultEnabled,
    required this.defaultProfile,
    required this.origin,
    required this.configurable,
    required this.fixable,
  });

  /// Stable rule id used by diagnostics and rule configuration.
  final String id;

  /// Short human-readable explanation of the rule.
  final String description;

  /// Source, ADR, fixture, or local implementation references backing the rule.
  final List<String> evidence;

  /// Default diagnostic severity for the rule.
  final String defaultSeverity;

  /// Diagnostic category, such as `parse`, `semantic`, or `config`.
  final String category;

  /// Whether the rule is enabled before profile or explicit-rule overrides.
  final bool defaultEnabled;

  /// Minimum profile that includes this rule by default.
  final String defaultProfile;

  /// Governance origin for the rule.
  final String origin;

  /// Whether callers may configure this rule directly.
  final bool configurable;

  /// Whether diagnostics from this rule can expose quick fixes.
  final bool fixable;

  /// Decodes a lint rule catalog entry produced by the native ABI.
  factory MermanLintRuleCatalogEntry.fromJson(Map<String, Object?> json) {
    final id = json['id'];
    final description = json['description'];
    final evidence = json['evidence'];
    final defaultSeverity = json['default_severity'];
    final category = json['category'];
    final defaultEnabled = json['default_enabled'];
    final defaultProfile = json['default_profile'];
    final origin = json['origin'];
    final configurable = json['configurable'];
    final fixable = json['fixable'];
    if (id is! String ||
        description is! String ||
        evidence is! List ||
        !evidence.every((item) => item is String) ||
        defaultSeverity is! String ||
        category is! String ||
        defaultEnabled is! bool ||
        defaultProfile is! String ||
        origin is! String ||
        configurable is! bool ||
        fixable is! bool) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_JSON_TYPE_ERROR',
        message: 'expected lint rule catalog JSON object',
      );
    }
    return MermanLintRuleCatalogEntry(
      id: id,
      description: description,
      evidence: List.unmodifiable(evidence.cast<String>()),
      defaultSeverity: defaultSeverity,
      category: category,
      defaultEnabled: defaultEnabled,
      defaultProfile: defaultProfile,
      origin: origin,
      configurable: configurable,
      fixable: fixable,
    );
  }
}

/// Reusable engine wrapper around the native `merman_engine_*` ABI.
class MermanReusableEngine {
  MermanReusableEngine._(this._bindings, this._engine);

  /// Creates a reusable engine from an already-opened [DynamicLibrary].
  factory MermanReusableEngine.fromDynamicLibrary(
    DynamicLibrary library, {
    String? optionsJson,
  }) {
    final bindings = _MermanBindings(library)..checkAbi();
    return bindings.newReusableEngine(optionsJson);
  }

  /// Opens the bundled native library and creates a reusable engine.
  factory MermanReusableEngine.open({String? optionsJson}) =>
      MermanReusableEngine.fromDynamicLibrary(
        openMermanLibrary(),
        optionsJson: optionsJson,
      );

  /// Opens a native library from [path] and creates a reusable engine.
  factory MermanReusableEngine.openPath(String path, {String? optionsJson}) =>
      MermanReusableEngine.fromDynamicLibrary(
        openMermanLibraryFromPath(path),
        optionsJson: optionsJson,
      );

  final _MermanBindings _bindings;
  Pointer<NativeMermanEngine> _engine;
  NativeCallable<_HostTextMeasureCallbackC>? _textMeasureCallback;
  MermanTextMeasurer? _textMeasurer;

  bool get _isClosed => _engine.address == 0;

  /// Installs or clears a host text-measurement callback.
  ///
  /// Pass `null` to restore the native fallback measurer configured by the
  /// engine options. The callback must be fast and must not call back into the
  /// same [MermanReusableEngine]. If the callback throws, the native engine
  /// falls back to its configured text measurer for that request.
  void setTextMeasurer(MermanTextMeasurer? measurer) {
    _ensureOpen();
    final previousCallback = _textMeasureCallback;
    final previousMeasurer = _textMeasurer;

    if (measurer == null) {
      _bindings.checkResult(
        _bindings.engineSetTextMeasureCallback(_engine, nullptr, nullptr),
      );
      _textMeasureCallback = null;
      _textMeasurer = null;
      previousCallback?.close();
      return;
    }

    final nativeCallback =
        NativeCallable<_HostTextMeasureCallbackC>.isolateLocal(_measureText);
    try {
      _bindings.checkResult(
        _bindings.engineSetTextMeasureCallback(
          _engine,
          nativeCallback.nativeFunction,
          nullptr,
        ),
      );
      _textMeasureCallback = nativeCallback;
      _textMeasurer = measurer;
      previousCallback?.close();
    } catch (_) {
      nativeCallback.close();
      _textMeasureCallback = previousCallback;
      _textMeasurer = previousMeasurer;
      rethrow;
    }
  }

  /// Renders Mermaid [source] to SVG text.
  String renderSvg(String source) {
    return _decodeText(
      _bindings.engineCall(_bindings.engineRenderSvg, _engine, source),
    );
  }

  /// Renders Mermaid [source] to Unicode ASCII-art text.
  String renderAscii(String source) {
    return _decodeText(
      _bindings.engineCall(_bindings.engineRenderAscii, _engine, source),
    );
  }

  /// Parses Mermaid [source] and returns raw semantic JSON text.
  String parseJsonRaw(String source) {
    return _decodeText(
      _bindings.engineCall(_bindings.engineParseJson, _engine, source),
    );
  }

  /// Parses Mermaid [source] and returns the semantic JSON object.
  Map<String, Object?> parseJson(String source) {
    return Merman._decodeJsonMap(parseJsonRaw(source));
  }

  /// Lays out Mermaid [source] and returns raw layout JSON text.
  String layoutJsonRaw(String source) {
    return _decodeText(
      _bindings.engineCall(_bindings.engineLayoutJson, _engine, source),
    );
  }

  /// Lays out Mermaid [source] and returns the layout JSON object.
  Map<String, Object?> layoutJson(String source) {
    return Merman._decodeJsonMap(layoutJsonRaw(source));
  }

  /// Analyzes Mermaid [source] and returns raw diagnostics JSON text.
  String analyzeJsonRaw(String source) {
    return _decodeText(
        _bindings.engineCall(_bindings.engineAnalyzeJson, _engine, source));
  }

  /// Analyzes Mermaid [source] and returns the diagnostics JSON object.
  Map<String, Object?> analyzeJson(String source) {
    return Merman._decodeJsonMap(analyzeJsonRaw(source));
  }

  /// Analyzes Markdown or MDX [source] and returns raw document diagnostics JSON text.
  String analyzeDocumentJsonRaw(String source, {required String uri}) {
    return _decodeText(
      _bindings.engineDocumentCall(
        _bindings.engineAnalyzeDocumentJson,
        _engine,
        source,
        uri,
      ),
    );
  }

  /// Analyzes Markdown or MDX [source] and returns the document diagnostics JSON object.
  Map<String, Object?> analyzeDocumentJson(String source,
      {required String uri}) {
    return Merman._decodeJsonMap(analyzeDocumentJsonRaw(source, uri: uri));
  }

  /// Analyzes Markdown or MDX [source] and returns raw document syntax facts JSON text.
  String analyzeDocumentFactsJsonRaw(String source, {required String uri}) {
    return _decodeText(
      _bindings.engineDocumentCall(
        _bindings.engineAnalyzeDocumentFactsJson,
        _engine,
        source,
        uri,
      ),
    );
  }

  /// Analyzes Markdown or MDX [source] and returns the document syntax facts JSON object.
  Map<String, Object?> analyzeDocumentFactsJson(String source,
      {required String uri}) {
    return Merman._decodeJsonMap(
      analyzeDocumentFactsJsonRaw(source, uri: uri),
    );
  }

  /// Validates Mermaid [source] and returns raw validation JSON text.
  String validateJsonRaw(String source) {
    return _decodeText(
      _bindings.engineCall(_bindings.engineValidateJson, _engine, source),
    );
  }

  /// Validates Mermaid [source] without throwing for ordinary parse errors.
  MermanValidationResult validate(String source) {
    return MermanValidationResult.fromJson(
      Merman._decodeJsonMap(validateJsonRaw(source)),
    );
  }

  /// Frees the native reusable engine.
  void close() {
    if (_isClosed) {
      return;
    }
    final callback = _takeTextMeasureCallback();
    _bindings.engineFree(_engine);
    _engine = nullptr;
    callback?.close();
  }

  NativeMermanHostTextMeasureResult _measureText(
    NativeMermanHostTextMeasureRequest request,
    Pointer<Void> userData,
  ) {
    final nativeResult = Struct.create<NativeMermanHostTextMeasureResult>();
    final measurer = _textMeasurer;
    if (measurer == null) {
      return nativeResult;
    }

    final MermanTextMeasureResult? result;
    try {
      result = measurer(MermanTextMeasureRequest._(request));
    } catch (_) {
      return nativeResult;
    }
    if (result == null ||
        !result.width.isFinite ||
        !result.height.isFinite ||
        result.width < 0 ||
        result.height < 0 ||
        result.lineCount <= 0) {
      return nativeResult;
    }

    nativeResult.handled = 1;
    nativeResult.width = result.width;
    nativeResult.height = result.height;
    nativeResult.lineCount = result.lineCount;
    return nativeResult;
  }

  NativeCallable<_HostTextMeasureCallbackC>? _takeTextMeasureCallback() {
    final callback = _textMeasureCallback;
    _textMeasureCallback = null;
    _textMeasurer = null;
    return callback;
  }

  void _ensureOpen() {
    if (_isClosed) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_ENGINE_CLOSED',
        message: 'Merman reusable engine is closed',
      );
    }
  }

  static String _decodeText(Uint8List bytes) => utf8.decode(bytes);
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
  List<MermanAsciiCapability>? _asciiCapabilitiesCache;
  List<MermanDiagramFamilyCapability>? _diagramFamilyCapabilitiesCache;
  List<MermanLintRuleCatalogEntry>? _lintRuleCatalogCache;
  List<String>? _themesCache;
  List<String>? _hostThemePresetsCache;

  /// Native `merman-ffi` package version.
  String get packageVersion => _bindings.packageVersion();

  /// Creates a reusable engine using the same native library.
  MermanReusableEngine reusableEngine({String? optionsJson}) {
    return _bindings.newReusableEngine(optionsJson);
  }

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

  /// Analyzes Mermaid [source] and returns raw diagnostics JSON text.
  String analyzeJsonRaw(String source, {String? optionsJson}) {
    return _decodeText(
      _bindings.call(_bindings.analyzeJson, source, optionsJson),
    );
  }

  /// Analyzes Mermaid [source] and returns the diagnostics JSON object.
  Map<String, Object?> analyzeJson(String source, {String? optionsJson}) {
    return _decodeJsonMap(analyzeJsonRaw(source, optionsJson: optionsJson));
  }

  /// Analyzes Markdown or MDX [source] and returns raw document diagnostics JSON text.
  String analyzeDocumentJsonRaw(
    String source, {
    required String uri,
    String? optionsJson,
  }) {
    return _decodeText(
      _bindings.callDocument(
        _bindings.analyzeDocumentJson,
        source,
        optionsJson,
        uri,
      ),
    );
  }

  /// Analyzes Markdown or MDX [source] and returns the document diagnostics JSON object.
  Map<String, Object?> analyzeDocumentJson(
    String source, {
    required String uri,
    String? optionsJson,
  }) {
    return _decodeJsonMap(
      analyzeDocumentJsonRaw(
        source,
        uri: uri,
        optionsJson: optionsJson,
      ),
    );
  }

  /// Analyzes Markdown or MDX [source] and returns raw document syntax facts JSON text.
  String analyzeDocumentFactsJsonRaw(
    String source, {
    required String uri,
    String? optionsJson,
  }) {
    return _decodeText(
      _bindings.callDocument(
        _bindings.analyzeDocumentFactsJson,
        source,
        optionsJson,
        uri,
      ),
    );
  }

  /// Analyzes Markdown or MDX [source] and returns the document syntax facts JSON object.
  Map<String, Object?> analyzeDocumentFactsJson(
    String source, {
    required String uri,
    String? optionsJson,
  }) {
    return _decodeJsonMap(
      analyzeDocumentFactsJsonRaw(
        source,
        uri: uri,
        optionsJson: optionsJson,
      ),
    );
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

  /// Returns ASCII rendering capability records for the active native artifact.
  List<MermanAsciiCapability> asciiCapabilities() {
    return _asciiCapabilitiesCache ??= List.unmodifiable(
      _decodeJsonAsciiCapabilityList(
        _decodeText(_bindings.metadata(_bindings.asciiCapabilitiesJson)),
      ),
    );
  }

  /// Returns parser/render capability records for the active native artifact.
  List<MermanDiagramFamilyCapability> diagramFamilyCapabilities() {
    return _diagramFamilyCapabilitiesCache ??= List.unmodifiable(
      _decodeJsonCapabilityList(
        _decodeText(
          _bindings.metadata(_bindings.diagramFamilyCapabilitiesJson),
        ),
      ),
    );
  }

  /// Returns governed lint rule metadata for the active native artifact.
  List<MermanLintRuleCatalogEntry> lintRuleCatalog() {
    return _lintRuleCatalogCache ??= List.unmodifiable(
      _decodeJsonRuleCatalog(
        _decodeText(_bindings.metadata(_bindings.lintRuleCatalogJson)),
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

  static List<MermanDiagramFamilyCapability> _decodeJsonCapabilityList(
    String text,
  ) {
    final decoded = jsonDecode(text);
    if (decoded is List) {
      return decoded.map((item) {
        if (item is Map<String, Object?>) {
          return MermanDiagramFamilyCapability.fromJson(item);
        }
        throw const MermanException(
          code: -1,
          codeName: 'DART_JSON_TYPE_ERROR',
          message: 'expected diagram family capability JSON object',
        );
      }).toList(growable: false);
    }
    throw const MermanException(
      code: -1,
      codeName: 'DART_JSON_TYPE_ERROR',
      message: 'expected diagram family capability JSON array',
    );
  }

  static List<MermanLintRuleCatalogEntry> _decodeJsonRuleCatalog(
    String text,
  ) {
    final decoded = jsonDecode(text);
    if (decoded is List) {
      return decoded.map((item) {
        if (item is Map<String, Object?>) {
          return MermanLintRuleCatalogEntry.fromJson(item);
        }
        throw const MermanException(
          code: -1,
          codeName: 'DART_JSON_TYPE_ERROR',
          message: 'expected lint rule catalog JSON object',
        );
      }).toList(growable: false);
    }
    throw const MermanException(
      code: -1,
      codeName: 'DART_JSON_TYPE_ERROR',
      message: 'expected lint rule catalog JSON array',
    );
  }

  static List<MermanAsciiCapability> _decodeJsonAsciiCapabilityList(
    String text,
  ) {
    final decoded = jsonDecode(text);
    if (decoded is List) {
      return decoded.map((item) {
        if (item is Map<String, Object?>) {
          return MermanAsciiCapability.fromJson(item);
        }
        throw const MermanException(
          code: -1,
          codeName: 'DART_JSON_TYPE_ERROR',
          message: 'expected ASCII capability JSON object',
        );
      }).toList(growable: false);
    }
    throw const MermanException(
      code: -1,
      codeName: 'DART_JSON_TYPE_ERROR',
      message: 'expected ASCII capability JSON array',
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
        _engineResultStructSize =
            library.lookupFunction<_StructSizeC, _StructSizeDart>(
          'merman_engine_result_struct_size',
        ),
        _hostTextMeasureRequestStructSize =
            library.lookupFunction<_StructSizeC, _StructSizeDart>(
          'merman_host_text_measure_request_struct_size',
        ),
        _hostTextMeasureResultStructSize =
            library.lookupFunction<_StructSizeC, _StructSizeDart>(
          'merman_host_text_measure_result_struct_size',
        ),
        _engineNew = library.lookupFunction<_EngineNewC, _EngineNewDart>(
          'merman_engine_new',
        ),
        engineFree = library.lookupFunction<_EngineFreeC, _EngineFreeDart>(
          'merman_engine_free',
        ),
        engineSetTextMeasureCallback = library.lookupFunction<
                _EngineSetTextMeasureCallbackC,
                _EngineSetTextMeasureCallbackDart>(
            'merman_engine_set_text_measure_callback'),
        engineRenderSvg = library.lookupFunction<_EngineCallC, _EngineCallDart>(
          'merman_engine_render_svg',
        ),
        engineRenderAscii =
            library.lookupFunction<_EngineCallC, _EngineCallDart>(
          'merman_engine_render_ascii',
        ),
        engineParseJson = library.lookupFunction<_EngineCallC, _EngineCallDart>(
          'merman_engine_parse_json',
        ),
        engineLayoutJson =
            library.lookupFunction<_EngineCallC, _EngineCallDart>(
          'merman_engine_layout_json',
        ),
        engineAnalyzeJson =
            library.lookupFunction<_EngineCallC, _EngineCallDart>(
          'merman_engine_analyze_json',
        ),
        engineAnalyzeDocumentJson = library.lookupFunction<_EngineDocumentCallC,
            _EngineDocumentCallDart>('merman_engine_analyze_document_json'),
        engineAnalyzeDocumentFactsJson = library
            .lookupFunction<_EngineDocumentCallC, _EngineDocumentCallDart>(
          'merman_engine_analyze_document_facts_json',
        ),
        engineValidateJson =
            library.lookupFunction<_EngineCallC, _EngineCallDart>(
          'merman_engine_validate_json',
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
        analyzeJson = library.lookupFunction<_MermanCallC, _MermanCallDart>(
          'merman_analyze_json',
        ),
        analyzeDocumentJson = library.lookupFunction<_MermanDocumentCallC,
            _MermanDocumentCallDart>('merman_analyze_document_json'),
        analyzeDocumentFactsJson = library.lookupFunction<_MermanDocumentCallC,
            _MermanDocumentCallDart>('merman_analyze_document_facts_json'),
        validateJson = library.lookupFunction<_MermanCallC, _MermanCallDart>(
          'merman_validate_json',
        ),
        supportedDiagramsJson =
            library.lookupFunction<_MermanMetadataC, _MermanMetadataDart>(
          'merman_supported_diagrams_json',
        ),
        asciiCapabilitiesJson =
            library.lookupFunction<_MermanMetadataC, _MermanMetadataDart>(
          'merman_ascii_capabilities_json',
        ),
        diagramFamilyCapabilitiesJson =
            library.lookupFunction<_MermanMetadataC, _MermanMetadataDart>(
          'merman_diagram_family_capabilities_json',
        ),
        lintRuleCatalogJson =
            library.lookupFunction<_MermanMetadataC, _MermanMetadataDart>(
          'merman_lint_rule_catalog_json',
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
  final _StructSizeDart _engineResultStructSize;
  final _StructSizeDart _hostTextMeasureRequestStructSize;
  final _StructSizeDart _hostTextMeasureResultStructSize;
  final _EngineNewDart _engineNew;
  final _EngineFreeDart engineFree;
  final _EngineSetTextMeasureCallbackDart engineSetTextMeasureCallback;
  final _EngineCallDart engineRenderSvg;
  final _EngineCallDart engineRenderAscii;
  final _EngineCallDart engineParseJson;
  final _EngineCallDart engineLayoutJson;
  final _EngineCallDart engineAnalyzeJson;
  final _EngineDocumentCallDart engineAnalyzeDocumentJson;
  final _EngineDocumentCallDart engineAnalyzeDocumentFactsJson;
  final _EngineCallDart engineValidateJson;
  final _BufferFreeDart _bufferFree;
  final _MermanCallDart renderSvg;
  final _MermanCallDart renderAscii;
  final _MermanCallDart parseJson;
  final _MermanCallDart layoutJson;
  final _MermanCallDart analyzeJson;
  final _MermanDocumentCallDart analyzeDocumentJson;
  final _MermanDocumentCallDart analyzeDocumentFactsJson;
  final _MermanCallDart validateJson;
  final _MermanMetadataDart supportedDiagramsJson;
  final _MermanMetadataDart asciiCapabilitiesJson;
  final _MermanMetadataDart diagramFamilyCapabilitiesJson;
  final _MermanMetadataDart lintRuleCatalogJson;
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
    final engineResultSize = _engineResultStructSize();
    final hostTextMeasureRequestSize = _hostTextMeasureRequestStructSize();
    final hostTextMeasureResultSize = _hostTextMeasureResultStructSize();
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
    if (engineResultSize != sizeOf<NativeMermanEngineResult>()) {
      throw MermanException(
        code: -1,
        codeName: 'DART_ENGINE_RESULT_SIZE_MISMATCH',
        message:
            'expected ${sizeOf<NativeMermanEngineResult>()}, got $engineResultSize',
      );
    }
    if (hostTextMeasureRequestSize !=
        sizeOf<NativeMermanHostTextMeasureRequest>()) {
      throw MermanException(
        code: -1,
        codeName: 'DART_TEXT_MEASURE_REQUEST_SIZE_MISMATCH',
        message:
            'expected ${sizeOf<NativeMermanHostTextMeasureRequest>()}, got $hostTextMeasureRequestSize',
      );
    }
    if (hostTextMeasureResultSize !=
        sizeOf<NativeMermanHostTextMeasureResult>()) {
      throw MermanException(
        code: -1,
        codeName: 'DART_TEXT_MEASURE_RESULT_SIZE_MISMATCH',
        message:
            'expected ${sizeOf<NativeMermanHostTextMeasureResult>()}, got $hostTextMeasureResultSize',
      );
    }
  }

  String packageVersion() => _packageVersion().toDartString();

  MermanReusableEngine newReusableEngine(String? optionsJson) {
    final optionsBytes = optionsJson == null ? null : utf8.encode(optionsJson);
    final optionsPtr =
        optionsBytes == null ? nullptr : _copyBytes(optionsBytes);

    try {
      final result = _engineNew(optionsPtr, optionsBytes?.length ?? 0);
      final payload = _takeBuffer(result.data);
      if (result.code == MermanStatus.ok.code && result.engine.address != 0) {
        return MermanReusableEngine._(this, result.engine);
      }
      throw _exceptionFromPayload(result.code, payload);
    } finally {
      _freeIfAllocated(optionsPtr);
    }
  }

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

  Uint8List callDocument(
    _MermanDocumentCallDart function,
    String source,
    String? optionsJson,
    String uri,
  ) {
    final sourceBytes = utf8.encode(source);
    final optionsBytes = optionsJson == null ? null : utf8.encode(optionsJson);
    final uriBytes = utf8.encode(uri);
    final sourcePtr = _copyBytes(sourceBytes);
    final optionsPtr =
        optionsBytes == null ? nullptr : _copyBytes(optionsBytes);
    final uriPtr = _copyBytes(uriBytes);

    try {
      final result = function(
        sourcePtr,
        sourceBytes.length,
        optionsPtr,
        optionsBytes?.length ?? 0,
        uriPtr,
        uriBytes.length,
      );
      final payload = _takeBuffer(result.data);
      if (result.code == MermanStatus.ok.code) {
        return payload;
      }
      throw _exceptionFromPayload(result.code, payload);
    } finally {
      _freeIfAllocated(sourcePtr);
      _freeIfAllocated(optionsPtr);
      _freeIfAllocated(uriPtr);
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

  Uint8List engineCall(
    _EngineCallDart function,
    Pointer<NativeMermanEngine> engine,
    String source,
  ) {
    if (engine.address == 0) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_ENGINE_CLOSED',
        message: 'Merman reusable engine is closed',
      );
    }

    final sourceBytes = utf8.encode(source);
    final sourcePtr = _copyBytes(sourceBytes);

    try {
      final result = function(engine, sourcePtr, sourceBytes.length);
      final payload = _takeBuffer(result.data);
      if (result.code == MermanStatus.ok.code) {
        return payload;
      }
      throw _exceptionFromPayload(result.code, payload);
    } finally {
      _freeIfAllocated(sourcePtr);
    }
  }

  Uint8List engineDocumentCall(
    _EngineDocumentCallDart function,
    Pointer<NativeMermanEngine> engine,
    String source,
    String uri,
  ) {
    if (engine.address == 0) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_ENGINE_CLOSED',
        message: 'Merman reusable engine is closed',
      );
    }

    final sourceBytes = utf8.encode(source);
    final uriBytes = utf8.encode(uri);
    final sourcePtr = _copyBytes(sourceBytes);
    final uriPtr = _copyBytes(uriBytes);

    try {
      final result = function(
        engine,
        sourcePtr,
        sourceBytes.length,
        uriPtr,
        uriBytes.length,
      );
      final payload = _takeBuffer(result.data);
      if (result.code == MermanStatus.ok.code) {
        return payload;
      }
      throw _exceptionFromPayload(result.code, payload);
    } finally {
      _freeIfAllocated(sourcePtr);
      _freeIfAllocated(uriPtr);
    }
  }

  void checkResult(NativeMermanResult result) {
    final payload = _takeBuffer(result.data);
    if (result.code == MermanStatus.ok.code) {
      return;
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

String _utf8Slice(Pointer<Uint8> pointer, int length) {
  if (pointer.address == 0 || length == 0) {
    return '';
  }
  return utf8.decode(pointer.asTypedList(length));
}
