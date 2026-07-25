import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import 'generated/native_abi.dart' as native;

const int _runtimeCatalogSchemaVersion = 1;

/// Opens the bundled native `merman-ffi` library for the current platform.
ffi.DynamicLibrary openMermanLibrary() {
  if (Platform.isAndroid) {
    return ffi.DynamicLibrary.open('libmerman_ffi.so');
  }
  if (Platform.isIOS || Platform.isMacOS) {
    return ffi.DynamicLibrary.process();
  }
  if (Platform.isWindows) {
    return ffi.DynamicLibrary.open('merman_ffi.dll');
  }
  if (Platform.isLinux) {
    return ffi.DynamicLibrary.open('libmerman_ffi.so');
  }
  throw UnsupportedError('Unsupported platform: ${Platform.operatingSystem}');
}

/// Opens a native `merman-ffi` library at [path].
///
/// This exists for local Dart smoke tests and non-Flutter host applications.
ffi.DynamicLibrary openMermanLibraryFromPath(String path) =>
    ffi.DynamicLibrary.open(path);

/// Operations declared by the generated native ABI 3 header.
///
/// Numeric values and public operation IDs are generated from `abi/merman-v3.json`
/// through `merman.h`; this enum intentionally contains no handwritten codes.
enum MermanOperation {
  svg(native.MERMAN_NATIVE_OPERATION_SVG),
  png(native.MERMAN_NATIVE_OPERATION_PNG),
  jpeg(native.MERMAN_NATIVE_OPERATION_JPEG),
  pdf(native.MERMAN_NATIVE_OPERATION_PDF),
  ascii(native.MERMAN_NATIVE_OPERATION_ASCII),
  semanticJson(native.MERMAN_NATIVE_OPERATION_SEMANTIC_JSON),
  layoutJson(native.MERMAN_NATIVE_OPERATION_LAYOUT_JSON),
  analysisJson(native.MERMAN_NATIVE_OPERATION_ANALYSIS_JSON),
  analysisFactsJson(native.MERMAN_NATIVE_OPERATION_ANALYSIS_FACTS_JSON),
  validationJson(native.MERMAN_NATIVE_OPERATION_VALIDATION_JSON),
  documentAnalysisJson(native.MERMAN_NATIVE_OPERATION_DOCUMENT_ANALYSIS_JSON),
  documentAnalysisFactsJson(
    native.MERMAN_NATIVE_OPERATION_DOCUMENT_ANALYSIS_FACTS_JSON,
  );

  const MermanOperation(this.nativeCode);

  /// The generated ABI 3 numeric operation code.
  final int nativeCode;

  /// Stable ID consumed by the shared bindings-core operation path.
  String get operationId => switch (this) {
        MermanOperation.svg => native.MERMAN_NATIVE_OPERATION_ID_SVG,
        MermanOperation.png => native.MERMAN_NATIVE_OPERATION_ID_PNG,
        MermanOperation.jpeg => native.MERMAN_NATIVE_OPERATION_ID_JPEG,
        MermanOperation.pdf => native.MERMAN_NATIVE_OPERATION_ID_PDF,
        MermanOperation.ascii => native.MERMAN_NATIVE_OPERATION_ID_ASCII,
        MermanOperation.semanticJson =>
          native.MERMAN_NATIVE_OPERATION_ID_SEMANTIC_JSON,
        MermanOperation.layoutJson =>
          native.MERMAN_NATIVE_OPERATION_ID_LAYOUT_JSON,
        MermanOperation.analysisJson =>
          native.MERMAN_NATIVE_OPERATION_ID_ANALYSIS_JSON,
        MermanOperation.analysisFactsJson =>
          native.MERMAN_NATIVE_OPERATION_ID_ANALYSIS_FACTS_JSON,
        MermanOperation.validationJson =>
          native.MERMAN_NATIVE_OPERATION_ID_VALIDATION_JSON,
        MermanOperation.documentAnalysisJson =>
          native.MERMAN_NATIVE_OPERATION_ID_DOCUMENT_ANALYSIS_JSON,
        MermanOperation.documentAnalysisFactsJson =>
          native.MERMAN_NATIVE_OPERATION_ID_DOCUMENT_ANALYSIS_FACTS_JSON,
      };

  bool get requiresUri => switch (this) {
        MermanOperation.svg =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_SVG != 0,
        MermanOperation.png =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_PNG != 0,
        MermanOperation.jpeg =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_JPEG != 0,
        MermanOperation.pdf =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_PDF != 0,
        MermanOperation.ascii =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_ASCII != 0,
        MermanOperation.semanticJson =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_SEMANTIC_JSON != 0,
        MermanOperation.layoutJson =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_LAYOUT_JSON != 0,
        MermanOperation.analysisJson =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_ANALYSIS_JSON != 0,
        MermanOperation.analysisFactsJson =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_ANALYSIS_FACTS_JSON != 0,
        MermanOperation.validationJson =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_VALIDATION_JSON != 0,
        MermanOperation.documentAnalysisJson =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_DOCUMENT_ANALYSIS_JSON !=
              0,
        MermanOperation.documentAnalysisFactsJson =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_DOCUMENT_ANALYSIS_FACTS_JSON !=
              0,
      };
}

/// A raw generic-operation result returned by the ABI 3 operation table.
class MermanOperationResult {
  const MermanOperationResult({
    required this.operation,
    required this.mediaType,
    required this.bytes,
    required this.metadata,
  });

  final MermanOperation operation;
  final String mediaType;
  final Uint8List bytes;
  final Map<String, Object?> metadata;

  /// Decodes a UTF-8 output such as SVG, ASCII, or JSON.
  String get utf8Text => utf8.decode(bytes);

  /// Decodes a JSON object output.
  Map<String, Object?> get jsonObject => _decodeJsonObject(bytes, 'output');
}

/// Stable machine-readable classification for a binding failure.
enum MermanErrorKind {
  generic(native.MERMAN_NATIVE_ERROR_KIND_GENERIC),
  unknownOperation(native.MERMAN_NATIVE_ERROR_KIND_UNKNOWN_OPERATION),
  missingCapability(native.MERMAN_NATIVE_ERROR_KIND_MISSING_CAPABILITY),
  reentrantCall(native.MERMAN_NATIVE_ERROR_KIND_REENTRANT_CALL);

  const MermanErrorKind(this.wireName);

  final String wireName;

  static MermanErrorKind fromWireName(Object? value) => values.firstWhere(
        (kind) => kind.wireName == value,
        orElse: () => generic,
      );
}

/// Error returned by the native ABI or by a local contract validation failure.
class MermanException implements Exception {
  const MermanException({
    required this.code,
    required this.codeName,
    required this.message,
    this.kind = MermanErrorKind.generic,
    this.capabilityId,
  });

  final int code;
  final String codeName;
  final String message;
  final MermanErrorKind kind;
  final String? capabilityId;

  factory MermanException.contract(String message) => MermanException(
        code: -1,
        codeName: 'DART_NATIVE_CONTRACT_ERROR',
        message: message,
      );

  factory MermanException.fromNative(int status, Uint8List metadata) {
    var codeName = 'native-status-$status';
    var message = 'native ABI operation failed';
    var kind = MermanErrorKind.generic;
    String? capabilityId;
    try {
      final decoded = _decodeJsonObject(metadata, 'native error metadata');
      final decodedVersion = decoded['version'];
      if (decodedVersion != native.MERMAN_NATIVE_RESULT_SCHEMA_VERSION) {
        return MermanException.contract(
          'unsupported native error payload schema `$decodedVersion`; expected '
          '${native.MERMAN_NATIVE_RESULT_SCHEMA_VERSION}',
        );
      }
      final decodedCodeName = decoded['status_name'];
      final decodedMessage = decoded['message'];
      kind = MermanErrorKind.fromWireName(decoded['kind']);
      final decodedCapabilityId = decoded['capability_id'];
      if (decodedCapabilityId is String && decodedCapabilityId.isNotEmpty) {
        capabilityId = decodedCapabilityId;
      }
      if (decodedCodeName is String) {
        codeName = decodedCodeName;
      }
      if (decodedMessage is String) {
        message = decodedMessage;
      }
    } on FormatException {
      // Preserve the native numeric status when a broken library cannot encode
      // its optional error metadata.
    } on MermanException {
      // Preserve the native numeric status when a broken library cannot encode
      // its optional error metadata.
    }
    if (status == native.MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION) {
      if (kind == MermanErrorKind.unknownOperation) {
        return MermanUnknownOperationException(
          code: status,
          codeName: codeName,
          message: message,
        );
      }
      if (kind == MermanErrorKind.missingCapability && capabilityId != null) {
        return MermanMissingCapabilityException(
          code: status,
          codeName: codeName,
          message: message,
          capabilityId: capabilityId,
        );
      }
      return MermanUnsupportedOperationException(
        code: status,
        codeName: codeName,
        message: message,
        kind: kind,
        capabilityId: capabilityId,
      );
    }
    return MermanException(
      code: status,
      codeName: codeName,
      message: message,
      kind: kind,
      capabilityId: capabilityId,
    );
  }

  @override
  String toString() => 'MermanException($codeName, $code): $message';
}

/// A typed native failure indicating an unknown or unavailable operation.
class MermanUnsupportedOperationException extends MermanException {
  const MermanUnsupportedOperationException({
    required super.code,
    required super.codeName,
    required super.message,
    super.kind,
    super.capabilityId,
  });
}

/// The requested operation ID or native code is not defined by the ABI.
class MermanUnknownOperationException
    extends MermanUnsupportedOperationException {
  const MermanUnknownOperationException({
    required int code,
    required String codeName,
    required String message,
  }) : super(
          code: code,
          codeName: codeName,
          message: message,
          kind: MermanErrorKind.unknownOperation,
        );
}

/// A valid request requires a capability absent from the native artifact.
class MermanMissingCapabilityException
    extends MermanUnsupportedOperationException {
  const MermanMissingCapabilityException({
    required int code,
    required String codeName,
    required String message,
    required String capabilityId,
  }) : super(
          code: code,
          codeName: codeName,
          message: message,
          kind: MermanErrorKind.missingCapability,
          capabilityId: capabilityId,
        );
}

/// A concise decoded validation response.
class MermanValidationResult {
  const MermanValidationResult._(this.json);

  final Map<String, Object?> json;

  bool get valid => json['valid'] == true;

  String? get codeName {
    final value = json['code_name'];
    return value is String ? value : null;
  }
}

/// Text-measurement operations exposed by the separately versioned protocol.
///
/// The values below reference ffigen-generated constants from the public C
/// header, so this Dart facade never owns numeric protocol codes.
enum MermanTextMeasurementOperation {
  measure(native.MERMAN_TEXT_MEASUREMENT_OPERATION_MEASURE),
  computedLength(native.MERMAN_TEXT_MEASUREMENT_OPERATION_COMPUTED_LENGTH),
  bboxX(native.MERMAN_TEXT_MEASUREMENT_OPERATION_BBOX_X),
  bboxXWithAsciiOverhang(
    native.MERMAN_TEXT_MEASUREMENT_OPERATION_BBOX_X_WITH_ASCII_OVERHANG,
  ),
  titleBBoxX(native.MERMAN_TEXT_MEASUREMENT_OPERATION_TITLE_BBOX_X),
  simpleBBoxWidth(native.MERMAN_TEXT_MEASUREMENT_OPERATION_SIMPLE_BBOX_WIDTH),
  rawBBoxWidth(native.MERMAN_TEXT_MEASUREMENT_OPERATION_RAW_BBOX_WIDTH),
  tspanBBoxWidth(native.MERMAN_TEXT_MEASUREMENT_OPERATION_TSPAN_BBOX_WIDTH),
  tspanBBoxHeight(native.MERMAN_TEXT_MEASUREMENT_OPERATION_TSPAN_BBOX_HEIGHT),
  wrapProbeBBoxWidth(
    native.MERMAN_TEXT_MEASUREMENT_OPERATION_WRAP_PROBE_BBOX_WIDTH,
  ),
  simpleBBoxHeight(
    native.MERMAN_TEXT_MEASUREMENT_OPERATION_SIMPLE_BBOX_HEIGHT,
  ),
  wrapped(native.MERMAN_TEXT_MEASUREMENT_OPERATION_WRAPPED),
  wrappedWithRawWidth(
    native.MERMAN_TEXT_MEASUREMENT_OPERATION_WRAPPED_WITH_RAW_WIDTH,
  ),
  boundingClientRectWidth(
    native.MERMAN_TEXT_MEASUREMENT_OPERATION_BOUNDING_CLIENT_RECT_WIDTH,
  ),
  createTextBBoxYOffset(
    native.MERMAN_TEXT_MEASUREMENT_OPERATION_CREATE_TEXT_BBOX_Y_OFFSET,
  ),
  mermaidCalculateTextDimensions(
    native.MERMAN_TEXT_MEASUREMENT_OPERATION_MERMAID_CALCULATE_TEXT_DIMENSIONS,
  ),
  canvasMeasureTextWidth(
    native.MERMAN_TEXT_MEASUREMENT_OPERATION_CANVAS_MEASURE_TEXT_WIDTH,
  ),
  createTextMiddleBBoxYOffset(
    native.MERMAN_TEXT_MEASUREMENT_OPERATION_CREATE_TEXT_MIDDLE_BBOX_Y_OFFSET,
  ),
  rawBBoxHeight(native.MERMAN_TEXT_MEASUREMENT_OPERATION_RAW_BBOX_HEIGHT);

  const MermanTextMeasurementOperation(this.code);

  final int code;

  static MermanTextMeasurementOperation? fromCode(int code) {
    for (final operation in values) {
      if (operation.code == code) {
        return operation;
      }
    }
    return null;
  }
}

/// Shapes accepted for handled host text-measurement responses.
enum MermanTextMeasurementResultKind {
  metrics(native.MERMAN_TEXT_MEASUREMENT_RESULT_KIND_METRICS),
  length(native.MERMAN_TEXT_MEASUREMENT_RESULT_KIND_LENGTH),
  horizontalExtents(
    native.MERMAN_TEXT_MEASUREMENT_RESULT_KIND_HORIZONTAL_EXTENTS,
  ),
  wrappedWithRawWidth(
    native.MERMAN_TEXT_MEASUREMENT_RESULT_KIND_WRAPPED_WITH_RAW_WIDTH,
  );

  const MermanTextMeasurementResultKind(this.code);

  final int code;
}

/// A Dart view of one synchronous text-measurement request.
class MermanTextMeasureRequest {
  MermanTextMeasureRequest._(native.MermanNativeTextMeasureRequest request)
      : text = _utf8FromSlice(request.text, 'text measurement text'),
        fontFamily = _utf8FromSlice(
          request.font_family,
          'text measurement font family',
        ),
        fontSize = request.font_size,
        fontWeight = _utf8FromSlice(
          request.font_weight,
          'text measurement font weight',
        ),
        fontStyle = _utf8FromSlice(
          request.font_style,
          'text measurement font style',
        ),
        maxWidth = request.has_max_width == 0 ? null : request.max_width,
        lineHeight = request.line_height,
        letterSpacing = request.letter_spacing,
        wordSpacing = request.word_spacing,
        wrapModeCode = request.wrap_mode,
        directionCode = request.direction,
        whiteSpaceCode = request.white_space,
        phaseCode = request.phase,
        operationCode = request.operation,
        operation = MermanTextMeasurementOperation.fromCode(request.operation) {
    if (request.text_measurement_protocol_version !=
        native.MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION) {
      throw MermanException.contract(
        'unsupported text-measurement protocol '
        '${request.text_measurement_protocol_version}',
      );
    }
  }

  final String text;
  final String fontFamily;
  final double fontSize;
  final String fontWeight;
  final String fontStyle;
  final double? maxWidth;
  final double lineHeight;
  final double letterSpacing;
  final double wordSpacing;

  /// Native CSS wrap-mode code. It remains transport data rather than a
  /// duplicated Flutter enum because it is not a public capability catalog.
  final int wrapModeCode;
  final int directionCode;
  final int whiteSpaceCode;
  final int phaseCode;
  final int operationCode;
  final MermanTextMeasurementOperation? operation;
}

/// A validated handled text-measurement result.
class MermanTextMeasureResult {
  const MermanTextMeasureResult._({
    required this.resultKind,
    this.width = 0,
    this.height = 0,
    this.length = 0,
    this.lineCount = 0,
    this.bboxLeft,
    this.bboxRight,
    this.rawWidth,
  });

  factory MermanTextMeasureResult.metrics({
    required double width,
    required double height,
    required int lineCount,
  }) {
    _requireNonNegativeFinite(width, 'width');
    _requireNonNegativeFinite(height, 'height');
    if (lineCount <= 0) {
      throw RangeError.value(lineCount, 'lineCount', 'must be greater than 0');
    }
    return MermanTextMeasureResult._(
      resultKind: MermanTextMeasurementResultKind.metrics,
      width: width,
      height: height,
      lineCount: lineCount,
    );
  }

  factory MermanTextMeasureResult.length({required double length}) {
    _requireFinite(length, 'length');
    return MermanTextMeasureResult._(
      resultKind: MermanTextMeasurementResultKind.length,
      length: length,
    );
  }

  factory MermanTextMeasureResult.horizontalExtents({
    required double left,
    required double right,
  }) {
    _requireNonNegativeFinite(left, 'left');
    _requireNonNegativeFinite(right, 'right');
    return MermanTextMeasureResult._(
      resultKind: MermanTextMeasurementResultKind.horizontalExtents,
      bboxLeft: left,
      bboxRight: right,
    );
  }

  factory MermanTextMeasureResult.wrappedWithRawWidth({
    required double width,
    required double height,
    required int lineCount,
    double? rawWidth,
  }) {
    _requireNonNegativeFinite(width, 'width');
    _requireNonNegativeFinite(height, 'height');
    if (lineCount <= 0) {
      throw RangeError.value(lineCount, 'lineCount', 'must be greater than 0');
    }
    if (rawWidth != null) {
      _requireNonNegativeFinite(rawWidth, 'rawWidth');
    }
    return MermanTextMeasureResult._(
      resultKind: MermanTextMeasurementResultKind.wrappedWithRawWidth,
      width: width,
      height: height,
      lineCount: lineCount,
      rawWidth: rawWidth,
    );
  }

  final MermanTextMeasurementResultKind resultKind;
  final double width;
  final double height;
  final double length;
  final int lineCount;
  final double? bboxLeft;
  final double? bboxRight;
  final double? rawWidth;
}

/// Host callback invoked synchronously while native rendering measures text.
typedef MermanTextMeasurer = MermanTextMeasureResult? Function(
  MermanTextMeasureRequest request,
);

/// A validated runtime catalog returned by the native ABI 3 table.
class MermanRuntimeCatalog {
  MermanRuntimeCatalog._({
    required this.packageVersion,
    required this.capabilityIds,
    required this.outputIds,
    required this.operationIds,
    required this.systemAdapterIds,
    required this.textMeasurementProviderIds,
    required this.diagramFamilyCount,
    required this.generalBindingDefaultProfile,
    required this.cliDefaultProfile,
  });

  factory MermanRuntimeCatalog.fromJson(Map<String, Object?> catalog) {
    _requireRequiredKeys(
      catalog,
      const {
        'schema_version',
        'transport_api_version',
        'package_version',
        'capabilities',
        'registry',
        'resources',
      },
      'runtime catalog',
    );
    if (_requiredInt(catalog, 'schema_version') !=
        _runtimeCatalogSchemaVersion) {
      throw MermanException.contract(
        'unsupported runtime contract schema '
        '${catalog['schema_version']}',
      );
    }
    if (_requiredInt(catalog, 'transport_api_version') !=
        native.MERMAN_NATIVE_ABI_VERSION) {
      throw MermanException.contract(
        'runtime contract transport API version does not match native ABI 3',
      );
    }
    final packageVersion = catalog['package_version'];
    if (packageVersion is! String || packageVersion.isEmpty) {
      throw MermanException.contract(
        'runtime catalog package_version must be a non-empty string',
      );
    }

    final runtimeCapabilities = _requiredObject(catalog, 'capabilities');
    _requireRequiredKeys(
      runtimeCapabilities,
      const {
        'capability_ids',
        'operation_ids',
        'output_ids',
        'system_adapter_ids',
        'text_measurement',
      },
      'runtime capabilities',
    );
    final capabilityIds = _requiredSortedUniqueStrings(
      runtimeCapabilities,
      'capability_ids',
      'runtime capability IDs',
    );
    final capabilitySet = capabilityIds.toSet();

    final outputIds = _requiredSortedUniqueStrings(
      runtimeCapabilities,
      'output_ids',
      'runtime output IDs',
    );

    final operationIds = _requiredSortedUniqueStrings(
      runtimeCapabilities,
      'operation_ids',
      'runtime operation IDs',
    );
    for (final outputId in outputIds) {
      if (!operationIds.contains(outputId)) {
        throw MermanException.contract(
          'runtime output `$outputId` must also be a callable operation',
        );
      }
    }

    final systemAdapterIds = _requiredSortedUniqueStrings(
      runtimeCapabilities,
      'system_adapter_ids',
      'runtime system adapter IDs',
    );
    if (!capabilitySet.containsAll(systemAdapterIds)) {
      throw MermanException.contract(
        'runtime system adapter IDs must also be capability IDs',
      );
    }

    final textMeasurement = runtimeCapabilities['text_measurement'];
    final hasSvg =
        capabilitySet.contains(native.MERMAN_NATIVE_OPERATION_CAPABILITY_SVG);
    if (hasSvg != (textMeasurement is Map)) {
      throw MermanException.contract(
        'text measurement must be present exactly when SVG is available',
      );
    }
    final providers = <String>[];
    if (textMeasurement is Map) {
      final textMeasurementMap = _asObject(textMeasurement, 'text_measurement');
      _requireRequiredKeys(
        textMeasurementMap,
        const {'protocol_version', 'provider_ids'},
        'runtime text measurement',
      );
      if (_requiredInt(textMeasurementMap, 'protocol_version') !=
          native.MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION) {
        throw MermanException.contract(
          'text measurement protocol version does not match the generated native header',
        );
      }
      providers.addAll(_requiredSortedUniqueStrings(
        textMeasurementMap,
        'provider_ids',
        'runtime text measurement providers',
      ));
      if (!providers.contains('vendored')) {
        throw MermanException.contract(
          'SVG runtime contract must expose the vendored text measurement provider',
        );
      }
    }

    final registry = _requiredObject(catalog, 'registry');
    _requireRequiredKeys(
      registry,
      const {'diagram_family_count'},
      'runtime registry',
    );
    final diagramFamilyCount = _requiredInt(registry, 'diagram_family_count');
    if (diagramFamilyCount < 0) {
      throw MermanException.contract(
        'runtime diagram_family_count must be non-negative',
      );
    }

    final resources = _requiredObject(catalog, 'resources');
    _requireRequiredKeys(
      resources,
      const {
        'general_binding_default_profile',
        'cli_default_profile',
        'limits',
        'profiles',
      },
      'runtime resources',
    );
    final generalBindingDefaultProfile =
        resources['general_binding_default_profile'];
    final cliDefaultProfile = resources['cli_default_profile'];
    if (generalBindingDefaultProfile is! String ||
        generalBindingDefaultProfile.isEmpty ||
        cliDefaultProfile is! String ||
        cliDefaultProfile.isEmpty ||
        resources['limits'] is! List ||
        resources['profiles'] is! List) {
      throw MermanException.contract('runtime resource contract is invalid');
    }

    return MermanRuntimeCatalog._(
      packageVersion: packageVersion,
      capabilityIds: List.unmodifiable(capabilityIds),
      outputIds: List.unmodifiable(outputIds),
      operationIds: List.unmodifiable(operationIds),
      systemAdapterIds: List.unmodifiable(systemAdapterIds),
      textMeasurementProviderIds: List.unmodifiable(providers),
      diagramFamilyCount: diagramFamilyCount,
      generalBindingDefaultProfile: generalBindingDefaultProfile,
      cliDefaultProfile: cliDefaultProfile,
    );
  }

  final String packageVersion;
  final List<String> capabilityIds;
  final List<String> outputIds;
  final List<String> operationIds;
  final List<String> systemAdapterIds;
  final List<String> textMeasurementProviderIds;
  final int diagramFamilyCount;
  final String generalBindingDefaultProfile;
  final String cliDefaultProfile;

  bool supportsCapability(String id) => capabilityIds.contains(id);
  bool supportsOutput(String id) => outputIds.contains(id);
  bool supportsOperation(String id) => operationIds.contains(id);
}

/// Native ABI 3 facade for Flutter and standalone Dart hosts.
///
/// [dispose] is idempotent and must be called when the application is done
/// with this object. Use [reusableEngine] for an independently configured
/// lifecycle, including host text measurement.
class Merman {
  Merman._(this._native, this.runtimeCatalog, this._defaultEngine);

  factory Merman.fromDynamicLibrary(
    ffi.DynamicLibrary library, {
    String? optionsJson,
  }) {
    final nativeApi = _NativeApi.discover(library);
    final catalog = nativeApi.loadRuntimeCatalog();
    final defaultEngine = nativeApi.createEngine(optionsJson: optionsJson);
    return Merman._(nativeApi, catalog, defaultEngine);
  }

  factory Merman.open({String? optionsJson}) => Merman.fromDynamicLibrary(
        openMermanLibrary(),
        optionsJson: optionsJson,
      );

  factory Merman.openPath(String path, {String? optionsJson}) =>
      Merman.fromDynamicLibrary(
        openMermanLibraryFromPath(path),
        optionsJson: optionsJson,
      );

  final _NativeApi _native;
  final MermanRuntimeCatalog runtimeCatalog;
  final MermanReusableEngine _defaultEngine;
  bool _disposed = false;

  /// Native package version reported by the discovered table.
  String get packageVersion => _native.packageVersion;

  MermanReusableEngine reusableEngine({
    String? optionsJson,
    MermanTextMeasurer? textMeasurer,
  }) {
    _ensureOpen();
    return _native.createEngine(
      optionsJson: optionsJson,
      textMeasurer: textMeasurer,
    );
  }

  MermanOperationResult execute(
    MermanOperation operation,
    String source, {
    String? uri,
    String? optionsJson,
  }) {
    _ensureOpen();
    return _defaultEngine.execute(
      operation,
      source,
      uri: uri,
      optionsJson: optionsJson,
    );
  }

  String renderSvg(String source, {String? optionsJson}) =>
      _defaultEngine.renderSvg(source, optionsJson: optionsJson);

  Uint8List renderPng(String source, {String? optionsJson}) =>
      _defaultEngine.renderPng(source, optionsJson: optionsJson);

  Uint8List renderJpeg(String source, {String? optionsJson}) =>
      _defaultEngine.renderJpeg(source, optionsJson: optionsJson);

  Uint8List renderPdf(String source, {String? optionsJson}) =>
      _defaultEngine.renderPdf(source, optionsJson: optionsJson);

  String renderAscii(String source, {String? optionsJson}) =>
      _defaultEngine.renderAscii(source, optionsJson: optionsJson);

  Map<String, Object?> parseJson(String source, {String? optionsJson}) =>
      _defaultEngine.parseJson(source, optionsJson: optionsJson);

  Map<String, Object?> layoutJson(String source, {String? optionsJson}) =>
      _defaultEngine.layoutJson(source, optionsJson: optionsJson);

  Map<String, Object?> analyzeJson(String source, {String? optionsJson}) =>
      _defaultEngine.analyzeJson(source, optionsJson: optionsJson);

  Map<String, Object?> analyzeDocumentJson(
    String source, {
    required String uri,
    String? optionsJson,
  }) =>
      _defaultEngine.analyzeDocumentJson(
        source,
        uri: uri,
        optionsJson: optionsJson,
      );

  Map<String, Object?> analyzeDocumentFactsJson(
    String source, {
    required String uri,
    String? optionsJson,
  }) =>
      _defaultEngine.analyzeDocumentFactsJson(
        source,
        uri: uri,
        optionsJson: optionsJson,
      );

  MermanValidationResult validate(String source, {String? optionsJson}) =>
      _defaultEngine.validate(source, optionsJson: optionsJson);

  /// Releases the default native engine. Safe to call more than once.
  void dispose() {
    if (_disposed) {
      return;
    }
    _disposed = true;
    _defaultEngine.dispose();
  }

  /// Alias for [dispose] for APIs that use `close` naming.
  void close() => dispose();

  void _ensureOpen() {
    if (_disposed) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_ENGINE_CLOSED',
        message: 'Merman instance is disposed',
      );
    }
  }
}

/// An independently configured native engine with deterministic ownership.
class MermanReusableEngine {
  MermanReusableEngine._(this._native, this._token, this._textMeasurement);

  final _NativeApi _native;
  final int _token;
  final _TextMeasurementRegistration? _textMeasurement;
  bool _activeCall = false;
  bool _disposed = false;

  bool get isDisposed => _disposed;

  MermanOperationResult execute(
    MermanOperation operation,
    String source, {
    String? uri,
    String? optionsJson,
  }) {
    _ensureCallable();
    if (operation.requiresUri != (uri != null)) {
      throw MermanException.contract(
        'operation `${operation.operationId}` ${operation.requiresUri ? 'requires' : 'does not accept'} a URI',
      );
    }
    return _withNativeCall(
      () => _native.execute(
        _token,
        operation,
        source,
        uri: uri,
        optionsJson: optionsJson,
      ),
    );
  }

  String renderSvg(String source, {String? optionsJson}) => execute(
        MermanOperation.svg,
        source,
        optionsJson: optionsJson,
      ).utf8Text;

  Uint8List renderPng(String source, {String? optionsJson}) => execute(
        MermanOperation.png,
        source,
        optionsJson: optionsJson,
      ).bytes;

  Uint8List renderJpeg(String source, {String? optionsJson}) => execute(
        MermanOperation.jpeg,
        source,
        optionsJson: optionsJson,
      ).bytes;

  Uint8List renderPdf(String source, {String? optionsJson}) => execute(
        MermanOperation.pdf,
        source,
        optionsJson: optionsJson,
      ).bytes;

  String renderAscii(String source, {String? optionsJson}) => execute(
        MermanOperation.ascii,
        source,
        optionsJson: optionsJson,
      ).utf8Text;

  Map<String, Object?> parseJson(String source, {String? optionsJson}) => _json(
        MermanOperation.semanticJson,
        source,
        optionsJson: optionsJson,
      );

  Map<String, Object?> layoutJson(String source, {String? optionsJson}) =>
      _json(
        MermanOperation.layoutJson,
        source,
        optionsJson: optionsJson,
      );

  Map<String, Object?> analyzeJson(String source, {String? optionsJson}) =>
      _json(
        MermanOperation.analysisJson,
        source,
        optionsJson: optionsJson,
      );

  Map<String, Object?> analyzeDocumentJson(
    String source, {
    required String uri,
    String? optionsJson,
  }) =>
      _json(
        MermanOperation.documentAnalysisJson,
        source,
        uri: uri,
        optionsJson: optionsJson,
      );

  Map<String, Object?> analyzeDocumentFactsJson(
    String source, {
    required String uri,
    String? optionsJson,
  }) =>
      _json(
        MermanOperation.documentAnalysisFactsJson,
        source,
        uri: uri,
        optionsJson: optionsJson,
      );

  MermanValidationResult validate(String source, {String? optionsJson}) =>
      MermanValidationResult._(_json(
        MermanOperation.validationJson,
        source,
        optionsJson: optionsJson,
      ));

  /// Releases the native engine and its optional callback. Safe to call twice.
  void dispose() {
    if (_disposed) {
      return;
    }
    if (_activeCall) {
      throw const MermanException(
        code: native.MERMAN_NATIVE_STATUS_REENTRANT_CALL,
        codeName: 'reentrant-call',
        message: 'Merman engine cannot be disposed from a native callback',
        kind: MermanErrorKind.reentrantCall,
      );
    }
    _disposeNow();
  }

  void close() => dispose();

  Map<String, Object?> _json(
    MermanOperation operation,
    String source, {
    String? uri,
    String? optionsJson,
  }) =>
      execute(
        operation,
        source,
        uri: uri,
        optionsJson: optionsJson,
      ).jsonObject;

  T _withNativeCall<T>(T Function() body) {
    if (_activeCall) {
      throw const MermanException(
        code: native.MERMAN_NATIVE_STATUS_REENTRANT_CALL,
        codeName: 'reentrant-call',
        message: 'Merman engine cannot be re-entered from a native callback',
        kind: MermanErrorKind.reentrantCall,
      );
    }
    _activeCall = true;
    try {
      return body();
    } finally {
      _activeCall = false;
    }
  }

  void _ensureCallable() {
    if (_disposed) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_ENGINE_CLOSED',
        message: 'Merman reusable engine is disposed',
      );
    }
  }

  void _disposeNow() {
    if (_disposed) {
      return;
    }
    _native.freeEngine(_token);
    _disposed = true;
    _textMeasurement?.dispose();
  }
}

class _NativeApi {
  _NativeApi._({
    required this.packageVersion,
    required native.DartMermanNativeRuntimeCatalogFnFunction runtimeCatalog,
    required native.DartMermanNativeEngineNewFnFunction engineNew,
    required native.DartMermanNativeEngineFreeFnFunction engineFree,
    required native.DartMermanNativeExecuteCollectFnFunction executeCollect,
    required native.DartMermanNativeResultFreeFnFunction resultFree,
  })  : _runtimeCatalog = runtimeCatalog,
        _engineNew = engineNew,
        _engineFree = engineFree,
        _executeCollect = executeCollect,
        _resultFree = resultFree;

  final String packageVersion;
  final native.DartMermanNativeRuntimeCatalogFnFunction _runtimeCatalog;
  final native.DartMermanNativeEngineNewFnFunction _engineNew;
  final native.DartMermanNativeEngineFreeFnFunction _engineFree;
  final native.DartMermanNativeExecuteCollectFnFunction _executeCollect;
  final native.DartMermanNativeResultFreeFnFunction _resultFree;

  factory _NativeApi.discover(ffi.DynamicLibrary library) {
    final entry = native.MermanNativeBindings(library);
    final request = calloc<native.MermanNativeApiRequest>();
    final api = calloc<native.MermanNativeApi>();
    final allocations = _NativeAllocationScope();
    try {
      _writeSlice(
        request.ref.expected_layout_descriptor_digest,
        utf8.encode(native.MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST),
        allocations,
      );
      request.ref.struct_size = ffi.sizeOf<native.MermanNativeApiRequest>();
      request.ref.expected_abi_version = native.MERMAN_NATIVE_ABI_VERSION;
      api.ref.struct_size = ffi.sizeOf<native.MermanNativeApi>();

      final status = entry.merman_get_native_api(request, api);
      if (status != native.MERMAN_NATIVE_STATUS_OK) {
        throw MermanException(
          code: status,
          codeName: 'DART_ABI_DISCOVERY_FAILED',
          message: 'merman_get_native_api rejected the ABI 3 request',
        );
      }

      final table = api.ref;
      if (table.struct_size != ffi.sizeOf<native.MermanNativeApi>()) {
        throw MermanException.contract(
          'native API table size does not match ffigen output',
        );
      }
      if (table.abi_version != native.MERMAN_NATIVE_ABI_VERSION) {
        throw MermanException.contract(
          'native API version `${table.abi_version}` is not ABI 3',
        );
      }
      if (_utf8FromSlice(table.layout_descriptor_digest, 'layout digest') !=
          native.MERMAN_NATIVE_ABI_LAYOUT_DESCRIPTOR_DIGEST) {
        throw MermanException.contract(
          'native API layout descriptor digest does not match the generated header',
        );
      }
      _requireFunctionPointer(table.runtime_catalog, 'runtime_catalog');
      _requireFunctionPointer(table.engine_new, 'engine_new');
      _requireFunctionPointer(table.engine_free, 'engine_free');
      _requireFunctionPointer(table.execute_collect, 'execute_collect');
      _requireFunctionPointer(table.result_free, 'result_free');

      return _NativeApi._(
        packageVersion:
            _utf8FromSlice(table.package_version, 'package version'),
        runtimeCatalog: table.runtime_catalog
            .asFunction<native.DartMermanNativeRuntimeCatalogFnFunction>(),
        engineNew: table.engine_new
            .asFunction<native.DartMermanNativeEngineNewFnFunction>(),
        engineFree: table.engine_free
            .asFunction<native.DartMermanNativeEngineFreeFnFunction>(),
        executeCollect: table.execute_collect
            .asFunction<native.DartMermanNativeExecuteCollectFnFunction>(),
        resultFree: table.result_free
            .asFunction<native.DartMermanNativeResultFreeFnFunction>(),
      );
    } finally {
      allocations.dispose();
      calloc.free(request);
      calloc.free(api);
    }
  }

  MermanRuntimeCatalog loadRuntimeCatalog() {
    final result = _newResult();
    try {
      final status = _runtimeCatalog(result);
      final metadata = _copyBuffer(result.ref.metadata_or_error_json);
      _ensureResultStatus(status, result.ref.status, metadata);
      return MermanRuntimeCatalog.fromJson(
        _decodeJsonObject(metadata, 'runtime catalog'),
      );
    } finally {
      _resultFree(result);
      calloc.free(result);
    }
  }

  MermanReusableEngine createEngine({
    String? optionsJson,
    MermanTextMeasurer? textMeasurer,
  }) {
    final registration = textMeasurer == null
        ? null
        : _TextMeasurementRegistration.create(textMeasurer);
    final config = calloc<native.MermanNativeEngineConfig>();
    final token = calloc<native.MermanNativeEngineToken>();
    final result = _newResult();
    final allocations = _NativeAllocationScope();
    try {
      config.ref.struct_size = ffi.sizeOf<native.MermanNativeEngineConfig>();
      _writeSlice(
        config.ref.options_json,
        optionsJson == null ? const <int>[] : utf8.encode(optionsJson),
        allocations,
      );
      config.ref.text_measure = registration?.nativeFunction ??
          ffi.nullptr.cast<
              ffi.NativeFunction<
                  native.MermanNativeTextMeasureCallbackFunction>>();
      config.ref.text_measure_user_data =
          registration?.userData ?? ffi.nullptr.cast<ffi.Void>();

      final status = _engineNew(config, token, result);
      final metadata = _copyBuffer(result.ref.metadata_or_error_json);
      _ensureResultStatus(status, result.ref.status, metadata);
      if (token.value == 0) {
        throw MermanException.contract(
          'native engine creation succeeded without an engine token',
        );
      }
      return MermanReusableEngine._(this, token.value, registration);
    } catch (_) {
      registration?.dispose();
      rethrow;
    } finally {
      allocations.dispose();
      _resultFree(result);
      calloc.free(config);
      calloc.free(token);
      calloc.free(result);
    }
  }

  void freeEngine(int token) {
    final status = _engineFree(token);
    if (status != native.MERMAN_NATIVE_STATUS_OK) {
      throw MermanException(
        code: status,
        codeName: 'DART_ENGINE_FREE_FAILED',
        message: 'native engine disposal failed',
      );
    }
  }

  MermanOperationResult execute(
    int engine,
    MermanOperation operation,
    String source, {
    String? uri,
    String? optionsJson,
  }) {
    final request = _newRequest(
      operation,
      source,
      uri: uri,
      optionsJson: optionsJson,
    );
    final result = _newResult();
    try {
      final status = _executeCollect(engine, request.pointer, result);
      final metadata = _copyBuffer(result.ref.metadata_or_error_json);
      _ensureResultStatus(status, result.ref.status, metadata);
      if (result.ref.operation != operation.nativeCode) {
        throw MermanException.contract(
          'native operation does not match the requested `${operation.operationId}`',
        );
      }
      return MermanOperationResult(
        operation: operation,
        mediaType: _utf8FromSlice(result.ref.media_type, 'result media type'),
        bytes: _copyBuffer(result.ref.data),
        metadata: _decodeJsonObject(metadata, 'result metadata'),
      );
    } finally {
      _resultFree(result);
      calloc.free(result);
      request.dispose();
    }
  }

  _NativeRequest _newRequest(
    MermanOperation operation,
    String source, {
    String? uri,
    String? optionsJson,
  }) {
    final request = calloc<native.MermanNativeOperationRequest>();
    final allocations = _NativeAllocationScope();
    request.ref.struct_size = ffi.sizeOf<native.MermanNativeOperationRequest>();
    request.ref.operation = operation.nativeCode;
    _writeSlice(request.ref.source, utf8.encode(source), allocations);
    _writeSlice(
      request.ref.uri,
      uri == null ? const <int>[] : utf8.encode(uri),
      allocations,
    );
    _writeSlice(
      request.ref.options_json,
      optionsJson == null ? const <int>[] : utf8.encode(optionsJson),
      allocations,
    );
    return _NativeRequest(request, allocations);
  }
}

class _NativeRequest {
  _NativeRequest(this.pointer, this._allocations);

  final ffi.Pointer<native.MermanNativeOperationRequest> pointer;
  final _NativeAllocationScope _allocations;

  void dispose() {
    _allocations.dispose();
    calloc.free(pointer);
  }
}

class _NativeAllocationScope {
  final List<ffi.Pointer<ffi.Uint8>> _bytes = [];

  ffi.Pointer<ffi.Uint8> copy(Uint8List bytes) {
    if (bytes.isEmpty) {
      return ffi.nullptr.cast<ffi.Uint8>();
    }
    final pointer = calloc<ffi.Uint8>(bytes.length);
    pointer.asTypedList(bytes.length).setAll(0, bytes);
    _bytes.add(pointer);
    return pointer;
  }

  void dispose() {
    for (final pointer in _bytes) {
      calloc.free(pointer);
    }
    _bytes.clear();
  }
}

class _TextMeasurementRegistration {
  _TextMeasurementRegistration._(
    this._key,
    this._callback,
  );

  static final Map<int, MermanTextMeasurer> _measurers = {};

  final ffi.Pointer<ffi.Uint8> _key;
  final ffi.NativeCallable<native.MermanNativeTextMeasureCallbackFunction>
      _callback;
  bool _disposed = false;

  ffi.Pointer<ffi.Void> get userData => _key.cast<ffi.Void>();

  ffi.Pointer<
          ffi.NativeFunction<native.MermanNativeTextMeasureCallbackFunction>>
      get nativeFunction => _callback.nativeFunction;

  static _TextMeasurementRegistration create(MermanTextMeasurer measurer) {
    final key = calloc<ffi.Uint8>();
    _measurers[key.address] = measurer;
    try {
      final callback = ffi.NativeCallable<
          native.MermanNativeTextMeasureCallbackFunction>.isolateLocal(
        _invoke,
        exceptionalReturn: native.MERMAN_NATIVE_STATUS_CALLBACK_ERROR,
      );
      return _TextMeasurementRegistration._(key, callback);
    } catch (_) {
      _measurers.remove(key.address);
      calloc.free(key);
      rethrow;
    }
  }

  static int _invoke(
    ffi.Pointer<native.MermanNativeTextMeasureRequest> request,
    ffi.Pointer<native.MermanNativeTextMeasureResult> outResult,
    ffi.Pointer<ffi.Void> userData,
  ) {
    if (request.address == 0 || outResult.address == 0) {
      return native.MERMAN_NATIVE_STATUS_CALLBACK_ERROR;
    }
    final output = outResult.ref;
    output.struct_size = ffi.sizeOf<native.MermanNativeTextMeasureResult>();
    output.handled = 0;
    output.has_raw_width = 0;
    output.result_kind = native.MERMAN_TEXT_MEASUREMENT_RESULT_KIND_METRICS;
    output.width = 0;
    output.height = 0;
    output.length = 0;
    output.bbox_left = 0;
    output.bbox_right = 0;
    output.raw_width = 0;
    output.line_count = 0;

    final measurer = _measurers[userData.address];
    if (measurer == null) {
      return native.MERMAN_NATIVE_STATUS_CALLBACK_ERROR;
    }
    try {
      final result = measurer(MermanTextMeasureRequest._(request.ref));
      if (result == null) {
        return native.MERMAN_NATIVE_STATUS_OK;
      }
      output.handled = 1;
      output.has_raw_width = result.rawWidth == null ? 0 : 1;
      output.result_kind = result.resultKind.code;
      output.width = result.width;
      output.height = result.height;
      output.length = result.length;
      output.bbox_left = result.bboxLeft ?? 0;
      output.bbox_right = result.bboxRight ?? 0;
      output.raw_width = result.rawWidth ?? 0;
      output.line_count = result.lineCount;
      return native.MERMAN_NATIVE_STATUS_OK;
    } catch (_) {
      return native.MERMAN_NATIVE_STATUS_CALLBACK_ERROR;
    }
  }

  void dispose() {
    if (_disposed) {
      return;
    }
    _disposed = true;
    _measurers.remove(_key.address);
    _callback.close();
    calloc.free(_key);
  }
}

ffi.Pointer<native.MermanNativeResult> _newResult() {
  final result = calloc<native.MermanNativeResult>();
  result.ref.struct_size = ffi.sizeOf<native.MermanNativeResult>();
  return result;
}

void _writeSlice(
  native.MermanNativeSlice slice,
  List<int> bytes,
  _NativeAllocationScope allocations,
) {
  final owned = Uint8List.fromList(bytes);
  slice.struct_size = ffi.sizeOf<native.MermanNativeSlice>();
  slice.data = allocations.copy(owned);
  slice.len = owned.length;
}

Uint8List _copyBuffer(native.MermanNativeBuffer buffer) {
  if (buffer.struct_size != ffi.sizeOf<native.MermanNativeBuffer>()) {
    throw MermanException.contract(
      'native buffer has an unexpected struct size `${buffer.struct_size}`',
    );
  }
  if (buffer.len == 0) {
    return Uint8List(0);
  }
  if (buffer.data.address == 0) {
    throw MermanException.contract('native buffer has a null data pointer');
  }
  return Uint8List.fromList(buffer.data.asTypedList(buffer.len));
}

Uint8List _copySlice(native.MermanNativeSlice slice, String label) {
  if (slice.struct_size != ffi.sizeOf<native.MermanNativeSlice>()) {
    throw MermanException.contract(
      '$label has an unexpected struct size `${slice.struct_size}`',
    );
  }
  if (slice.len == 0) {
    return Uint8List(0);
  }
  if (slice.data.address == 0) {
    throw MermanException.contract('$label has a null data pointer');
  }
  return Uint8List.fromList(slice.data.asTypedList(slice.len));
}

String _utf8FromSlice(native.MermanNativeSlice slice, String label) =>
    utf8.decode(_copySlice(slice, label));

Map<String, Object?> _decodeJsonObject(Uint8List bytes, String label) {
  final decoded = jsonDecode(utf8.decode(bytes));
  return _asObject(decoded, label);
}

Map<String, Object?> _asObject(Object? value, String label) {
  if (value is! Map) {
    throw MermanException.contract('$label must be a JSON object');
  }
  final output = <String, Object?>{};
  for (final entry in value.entries) {
    if (entry.key is! String) {
      throw MermanException.contract('$label has a non-string object key');
    }
    output[entry.key as String] = entry.value;
  }
  return output;
}

Map<String, Object?> _requiredObject(Map<String, Object?> source, String key) {
  if (!source.containsKey(key)) {
    throw MermanException.contract('missing required `$key` field');
  }
  return _asObject(source[key], key);
}

void _requireRequiredKeys(
  Map<String, Object?> source,
  Set<String> expected,
  String label,
) {
  final actual = source.keys.toSet();
  final missing = expected.difference(actual);
  if (missing.isNotEmpty) {
    throw MermanException.contract(
      '$label is missing required fields: ${missing.toList()..sort()}',
    );
  }
}

int _requiredInt(Map<String, Object?> source, String key) {
  final value = source[key];
  if (value is! int) {
    throw MermanException.contract('`$key` must be an integer');
  }
  return value;
}

List<String> _requiredSortedUniqueStrings(
  Map<String, Object?> source,
  String key,
  String label,
) {
  final value = source[key];
  if (value is! List) {
    throw MermanException.contract('$label must be an array');
  }
  final values = <String>[];
  String? previous;
  for (final item in value) {
    if (item is! String || item.isEmpty) {
      throw MermanException.contract(
          '$label must contain non-empty string IDs');
    }
    if (previous != null && previous.compareTo(item) >= 0) {
      throw MermanException.contract('$label must be sorted and unique');
    }
    previous = item;
    values.add(item);
  }
  return values;
}

void _requireFunctionPointer<T extends ffi.NativeType>(
  ffi.Pointer<T> pointer,
  String name,
) {
  if (pointer.address == 0) {
    throw MermanException.contract('native API table has no `$name` function');
  }
}

void _ensureResultStatus(int callStatus, int resultStatus, Uint8List metadata) {
  if (callStatus != native.MERMAN_NATIVE_STATUS_OK) {
    throw MermanException.fromNative(callStatus, metadata);
  }
  if (resultStatus != native.MERMAN_NATIVE_STATUS_OK) {
    throw MermanException.contract(
      'native operation returned OK but result status was `$resultStatus`',
    );
  }
}

void _requireFinite(double value, String name) {
  if (!value.isFinite) {
    throw ArgumentError.value(value, name, 'must be finite');
  }
}

void _requireNonNegativeFinite(double value, String name) {
  _requireFinite(value, name);
  if (value < 0) {
    throw RangeError.value(value, name, 'must be non-negative');
  }
}
