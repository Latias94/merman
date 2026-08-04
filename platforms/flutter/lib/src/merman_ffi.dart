import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import 'generated/native_abi.dart' as native;
import 'generated/package_version.dart';
import 'generated/resource_options.dart' show mermanBindingOptionsSchemaVersion;
import 'generated/text_measurement_protocol.dart';

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
  ),
  svgPlanJson(native.MERMAN_NATIVE_OPERATION_SVG_PLAN_JSON);

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
        MermanOperation.svgPlanJson =>
          native.MERMAN_NATIVE_OPERATION_ID_SVG_PLAN_JSON,
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
        MermanOperation.svgPlanJson =>
          native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_SVG_PLAN_JSON != 0,
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
  reentrantCall(native.MERMAN_NATIVE_ERROR_KIND_REENTRANT_CALL),
  busy(native.MERMAN_NATIVE_ERROR_KIND_BUSY);

  const MermanErrorKind(this.wireName);

  final String wireName;

  static MermanErrorKind fromWireName(Object? value) => values.firstWhere(
        (kind) => kind.wireName == value,
        orElse: () => generic,
      );
}

/// Stable resource metadata attached to a native resource-limit failure.
class MermanResourceErrorDetails {
  const MermanResourceErrorDetails({
    required this.limitId,
    required this.phase,
    required this.actual,
    required this.max,
    required this.profile,
  });

  final String limitId;
  final String phase;
  final int actual;
  final int max;
  final String profile;
}

/// Error returned by the native ABI or by a local contract validation failure.
class MermanException implements Exception {
  const MermanException({
    required this.code,
    required this.codeName,
    required this.message,
    this.kind = MermanErrorKind.generic,
    this.capabilityId,
    this.resourceDetails,
  });

  final int code;
  final String codeName;
  final String message;
  final MermanErrorKind kind;
  final String? capabilityId;
  final MermanResourceErrorDetails? resourceDetails;

  factory MermanException.contract(String message) => MermanException(
        code: -1,
        codeName: 'DART_NATIVE_CONTRACT_ERROR',
        message: message,
      );

  factory MermanException.fromNative(int status, Uint8List metadata) {
    var codeName = switch (status) {
      native.MERMAN_NATIVE_STATUS_REENTRANT_CALL => 'reentrant-call',
      native.MERMAN_NATIVE_STATUS_BUSY => 'busy',
      _ => 'native-status-$status',
    };
    var message = 'native ABI operation failed';
    var kind = switch (status) {
      native.MERMAN_NATIVE_STATUS_REENTRANT_CALL =>
        MermanErrorKind.reentrantCall,
      native.MERMAN_NATIVE_STATUS_BUSY => MermanErrorKind.busy,
      _ => MermanErrorKind.generic,
    };
    String? capabilityId;
    MermanResourceErrorDetails? resourceDetails;
    if (metadata.isNotEmpty) {
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
        final details = decoded['details'];
        if (details is Map) {
          final resource = details['resource'];
          if (resource is Map) {
            final limitId = resource['limit_id'];
            final phase = resource['phase'];
            final actual = resource['actual'];
            final max = resource['max'];
            final profile = resource['profile'];
            if (limitId is String &&
                limitId.isNotEmpty &&
                phase is String &&
                phase.isNotEmpty &&
                actual is int &&
                actual >= 0 &&
                max is int &&
                max >= 0 &&
                profile is String &&
                profile.isNotEmpty) {
              resourceDetails = MermanResourceErrorDetails(
                limitId: limitId,
                phase: phase,
                actual: actual,
                max: max,
                profile: profile,
              );
            }
          }
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
    if (status == native.MERMAN_NATIVE_STATUS_REENTRANT_CALL) {
      return MermanReentrantCallException(
        code: status,
        codeName: codeName,
        message: message,
      );
    }
    if (status == native.MERMAN_NATIVE_STATUS_BUSY) {
      return MermanBusyException(
        code: status,
        codeName: codeName,
        message: message,
      );
    }
    return MermanException(
      code: status,
      codeName: codeName,
      message: message,
      kind: kind,
      capabilityId: capabilityId,
      resourceDetails: resourceDetails,
    );
  }

  @override
  String toString() => 'MermanException($codeName, $code): $message';
}

/// The same engine was re-entered or closed while its callback was active.
class MermanReentrantCallException extends MermanException {
  const MermanReentrantCallException({
    required super.code,
    required super.codeName,
    required super.message,
  }) : super(kind: MermanErrorKind.reentrantCall);
}

/// The engine has an active operation and cannot admit or close immediately.
class MermanBusyException extends MermanException {
  const MermanBusyException({
    required super.code,
    required super.codeName,
    required super.message,
  }) : super(kind: MermanErrorKind.busy);
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

/// Evidence attached to an ASCII rendering capability record.
class MermanAsciiCapabilityEvidence {
  const MermanAsciiCapabilityEvidence({
    required this.kind,
    required this.source,
    required this.note,
  });

  final String kind;
  final String source;
  final String note;

  factory MermanAsciiCapabilityEvidence.fromJson(
    Map<String, Object?> json,
  ) =>
      MermanAsciiCapabilityEvidence(
        kind: _requiredString(json, 'kind', 'ASCII capability evidence'),
        source: _requiredString(json, 'source', 'ASCII capability evidence'),
        note: _requiredString(json, 'note', 'ASCII capability evidence'),
      );
}

/// ASCII rendering capability for one Mermaid diagram type.
class MermanAsciiCapability {
  const MermanAsciiCapability({
    required this.diagramType,
    required this.displayName,
    required this.supportLevel,
    required this.summaryFallback,
    required this.supportedSemantics,
    required this.limits,
    required this.evidence,
  });

  final String diagramType;
  final String displayName;
  final String supportLevel;
  final bool summaryFallback;
  final List<String> supportedSemantics;
  final List<String> limits;
  final List<MermanAsciiCapabilityEvidence> evidence;

  factory MermanAsciiCapability.fromJson(Map<String, Object?> json) {
    final rawEvidence = json['evidence'];
    if (rawEvidence is! List) {
      throw MermanException.contract(
        'ASCII capability.evidence must be an array',
      );
    }
    return MermanAsciiCapability(
      diagramType: _requiredString(json, 'diagram_type', 'ASCII capability'),
      displayName: _requiredString(json, 'display_name', 'ASCII capability'),
      supportLevel: _requiredString(json, 'support_level', 'ASCII capability'),
      summaryFallback:
          _requiredBool(json, 'summary_fallback', 'ASCII capability'),
      supportedSemantics: List.unmodifiable(
        _requiredStringList(
          json,
          'supported_semantics',
          'ASCII capability.supported_semantics',
        ),
      ),
      limits: List.unmodifiable(
        _requiredStringList(json, 'limits', 'ASCII capability.limits'),
      ),
      evidence: List.unmodifiable(
        rawEvidence.indexed.map(
          (entry) => MermanAsciiCapabilityEvidence.fromJson(
            _asObject(
              entry.$2,
              'ASCII capability.evidence[${entry.$1}]',
            ),
          ),
        ),
      ),
    );
  }
}

/// Parser/render capability for one Mermaid diagram family.
class MermanDiagramFamilyCapability {
  const MermanDiagramFamilyCapability({
    required this.diagramType,
    required this.logicalFamilyKind,
    required this.metadataId,
    required this.renderModelKind,
    required this.hasDetector,
    required this.hasSemanticParser,
    required this.hasEditorParser,
    required this.hasCombinedParser,
    required this.hasRenderParser,
    required this.hasHeader,
    required this.configNamespace,
  });

  final String diagramType;
  final String logicalFamilyKind;
  final String? metadataId;
  final String? renderModelKind;
  final bool hasDetector;
  final bool hasSemanticParser;
  final bool hasEditorParser;
  final bool hasCombinedParser;
  final bool hasRenderParser;
  final bool hasHeader;
  final String? configNamespace;

  factory MermanDiagramFamilyCapability.fromJson(
    Map<String, Object?> json,
  ) {
    final metadataId = json['metadata_id'];
    if (metadataId != null && metadataId is! String) {
      throw MermanException.contract(
        'diagram family capability.metadata_id must be a string or null',
      );
    }
    final renderModelKind = json['render_model_kind'];
    if (renderModelKind != null && renderModelKind is! String) {
      throw MermanException.contract(
        'diagram family capability.render_model_kind must be a string or null',
      );
    }
    final configNamespace = json['config_namespace'];
    if (configNamespace != null && configNamespace is! String) {
      throw MermanException.contract(
        'diagram family capability.config_namespace must be a string or null',
      );
    }
    return MermanDiagramFamilyCapability(
      diagramType: _requiredString(
        json,
        'diagram_type',
        'diagram family capability',
      ),
      logicalFamilyKind: _requiredString(
        json,
        'logical_family_kind',
        'diagram family capability',
      ),
      metadataId: metadataId as String?,
      renderModelKind: renderModelKind as String?,
      hasDetector: _requiredBool(
        json,
        'has_detector',
        'diagram family capability',
      ),
      hasSemanticParser: _requiredBool(
        json,
        'has_semantic_parser',
        'diagram family capability',
      ),
      hasEditorParser: _requiredBool(
        json,
        'has_editor_parser',
        'diagram family capability',
      ),
      hasCombinedParser: _requiredBool(
        json,
        'has_combined_parser',
        'diagram family capability',
      ),
      hasRenderParser: _requiredBool(
        json,
        'has_render_parser',
        'diagram family capability',
      ),
      hasHeader: _requiredBool(
        json,
        'has_header',
        'diagram family capability',
      ),
      configNamespace: configNamespace as String?,
    );
  }
}

/// Public metadata for one lint rule exposed by the native artifact.
class MermanLintRuleCatalogEntry {
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

  final String id;
  final String description;
  final List<String> evidence;
  final String defaultSeverity;
  final String category;
  final bool defaultEnabled;
  final String defaultProfile;
  final String origin;
  final bool configurable;
  final bool fixable;

  factory MermanLintRuleCatalogEntry.fromJson(Map<String, Object?> json) =>
      MermanLintRuleCatalogEntry(
        id: _requiredString(json, 'id', 'lint rule catalog entry'),
        description: _requiredString(
          json,
          'description',
          'lint rule catalog entry',
        ),
        evidence: List.unmodifiable(
          _requiredStringList(
            json,
            'evidence',
            'lint rule catalog entry.evidence',
          ),
        ),
        defaultSeverity: _requiredString(
          json,
          'default_severity',
          'lint rule catalog entry',
        ),
        category: _requiredString(
          json,
          'category',
          'lint rule catalog entry',
        ),
        defaultEnabled: _requiredBool(
          json,
          'default_enabled',
          'lint rule catalog entry',
        ),
        defaultProfile: _requiredString(
          json,
          'default_profile',
          'lint rule catalog entry',
        ),
        origin: _requiredString(json, 'origin', 'lint rule catalog entry'),
        configurable: _requiredBool(
          json,
          'configurable',
          'lint rule catalog entry',
        ),
        fixable: _requiredBool(json, 'fixable', 'lint rule catalog entry'),
      );
}

/// Artifact-owned presentation theme and profile metadata.
class MermanPresentationCatalog {
  const MermanPresentationCatalog({
    required this.schemaVersion,
    required this.themePresets,
    required this.profiles,
  });

  final int schemaVersion;
  final List<MermanPresentationThemePreset> themePresets;
  final List<MermanPresentationProfile> profiles;

  factory MermanPresentationCatalog.fromJson(Map<String, Object?> json) {
    final schemaVersion = _requiredInt(json, 'schema_version');
    if (schemaVersion != 1) {
      throw MermanException.contract(
        'unsupported presentation catalog schema $schemaVersion',
      );
    }
    final rawThemePresets = json['theme_presets'];
    if (rawThemePresets is! List) {
      throw MermanException.contract(
        'presentation catalog.theme_presets must be an array',
      );
    }
    final rawProfiles = json['profiles'];
    if (rawProfiles is! List) {
      throw MermanException.contract(
        'presentation catalog.profiles must be an array',
      );
    }
    return MermanPresentationCatalog(
      schemaVersion: schemaVersion,
      themePresets: List.unmodifiable(
        rawThemePresets.indexed.map(
          (entry) => MermanPresentationThemePreset.fromJson(
            _asObject(
              entry.$2,
              'presentation catalog.theme_presets[${entry.$1}]',
            ),
          ),
        ),
      ),
      profiles: List.unmodifiable(
        rawProfiles.indexed.map(
          (entry) => MermanPresentationProfile.fromJson(
            _asObject(entry.$2, 'presentation catalog.profiles[${entry.$1}]'),
          ),
        ),
      ),
    );
  }
}

/// One built-in host/editor presentation theme advertised by the artifact.
class MermanPresentationThemePreset {
  const MermanPresentationThemePreset({
    required this.id,
    required this.appearance,
    required this.fullyAvailable,
    required this.missingCapabilityIds,
  });

  final String id;
  final String appearance;
  final bool fullyAvailable;
  final List<String> missingCapabilityIds;

  factory MermanPresentationThemePreset.fromJson(Map<String, Object?> json) =>
      MermanPresentationThemePreset(
        id: _requiredString(json, 'id', 'presentation theme preset'),
        appearance: _requiredString(
          json,
          'appearance',
          'presentation theme preset',
        ),
        fullyAvailable: _requiredBool(
          json,
          'fully_available',
          'presentation theme preset',
        ),
        missingCapabilityIds: List.unmodifiable(
          _requiredStringList(
            json,
            'missing_capability_ids',
            'presentation theme preset.missing_capability_ids',
          ),
        ),
      );
}

/// One Merman-owned presentation profile advertised by the artifact.
class MermanPresentationProfile {
  const MermanPresentationProfile({
    required this.id,
    required this.fullyAvailable,
    required this.missingCapabilityIds,
    required this.aspects,
  });

  final String id;
  final bool fullyAvailable;
  final List<String> missingCapabilityIds;
  final List<MermanPresentationAspect> aspects;

  factory MermanPresentationProfile.fromJson(Map<String, Object?> json) {
    final rawAspects = json['aspects'];
    if (rawAspects is! List) {
      throw MermanException.contract(
        'presentation profile.aspects must be an array',
      );
    }
    return MermanPresentationProfile(
      id: _requiredString(json, 'id', 'presentation profile'),
      fullyAvailable: _requiredBool(
        json,
        'fully_available',
        'presentation profile',
      ),
      missingCapabilityIds: List.unmodifiable(
        _requiredStringList(
          json,
          'missing_capability_ids',
          'presentation profile.missing_capability_ids',
        ),
      ),
      aspects: List.unmodifiable(
        rawAspects.indexed.map(
          (entry) => MermanPresentationAspect.fromJson(
            _asObject(entry.$2, 'presentation profile.aspects[${entry.$1}]'),
          ),
        ),
      ),
    );
  }
}

/// One independently applicable part of a presentation profile.
class MermanPresentationAspect {
  const MermanPresentationAspect({
    required this.id,
    required this.applicability,
    required this.requiredCapabilityId,
    required this.available,
    required this.missingCapabilityIds,
  });

  final String id;
  final MermanPresentationAspectApplicability applicability;
  final String? requiredCapabilityId;
  final bool available;
  final List<String> missingCapabilityIds;

  factory MermanPresentationAspect.fromJson(Map<String, Object?> json) {
    final requiredCapabilityId = json['required_capability_id'];
    if (requiredCapabilityId != null && requiredCapabilityId is! String) {
      throw MermanException.contract(
        'presentation aspect.required_capability_id must be a string or null',
      );
    }
    return MermanPresentationAspect(
      id: _requiredString(json, 'id', 'presentation aspect'),
      applicability: MermanPresentationAspectApplicability.fromJson(
        _requiredObject(json, 'applicability'),
      ),
      requiredCapabilityId: requiredCapabilityId as String?,
      available: _requiredBool(json, 'available', 'presentation aspect'),
      missingCapabilityIds: List.unmodifiable(
        _requiredStringList(
          json,
          'missing_capability_ids',
          'presentation aspect.missing_capability_ids',
        ),
      ),
    );
  }
}

/// Scope in which a presentation aspect applies.
class MermanPresentationAspectApplicability {
  const MermanPresentationAspectApplicability({
    required this.kind,
    required this.familyId,
  });

  final String kind;
  final String? familyId;

  factory MermanPresentationAspectApplicability.fromJson(
    Map<String, Object?> json,
  ) {
    final familyId = json['family_id'];
    if (familyId != null && familyId is! String) {
      throw MermanException.contract(
        'presentation aspect applicability.family_id must be a string or null',
      );
    }
    return MermanPresentationAspectApplicability(
      kind: _requiredString(json, 'kind', 'presentation aspect applicability'),
      familyId: familyId as String?,
    );
  }
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
        wrapMode = MermanTextWrapMode.requireCode(request.wrap_mode),
        direction = MermanTextDirection.requireCode(request.direction),
        whiteSpace = MermanTextWhiteSpace.requireCode(request.white_space),
        phase = MermanTextMeasurementPhase.requireCode(request.phase),
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

  final MermanTextWrapMode wrapMode;
  final MermanTextDirection direction;
  final MermanTextWhiteSpace whiteSpace;
  final MermanTextMeasurementPhase phase;
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
final class MermanResourceLimitDescriptor {
  MermanResourceLimitDescriptor({
    required this.id,
    required this.phase,
    required this.description,
    required this.overridable,
    required this.hardCap,
    required this.minimumValue,
    required List<String> operationIds,
  }) : operationIds = List.unmodifiable(operationIds);

  final String id;
  final String phase;
  final String description;
  final bool overridable;
  final bool hardCap;
  final int minimumValue;
  final List<String> operationIds;
}

/// One independently versioned binding payload advertised by the loaded artifact.
final class MermanRuntimePayloadSchema {
  const MermanRuntimePayloadSchema({
    required this.id,
    required this.version,
  });

  final String id;
  final int version;
}

final class MermanResourceProfileDescriptor {
  const MermanResourceProfileDescriptor({
    required this.id,
    required this.purpose,
    required this.trustAssumption,
    required this.recommendedBindingDefault,
    required this.limits,
  });

  final String id;
  final String purpose;
  final String trustAssumption;
  final bool recommendedBindingDefault;
  final Map<String, int?> limits;
}

/// Runtime behavior contract for one output exposed by the loaded artifact.
final class MermanRuntimeOutputContract {
  const MermanRuntimeOutputContract({
    required this.id,
    required this.mediaType,
    required this.systemFonts,
    required this.embeddedImages,
  });

  final String id;
  final String mediaType;
  final MermanRuntimeSystemFontContract? systemFonts;
  final MermanRuntimeEmbeddedImageContract? embeddedImages;
}

/// System-font behavior used by a native binary-output backend.
final class MermanRuntimeSystemFontContract {
  const MermanRuntimeSystemFontContract({
    required this.sourceId,
    required this.discovery,
    required this.cacheScope,
    required this.hostDependent,
    required this.callerConfigurable,
    required this.resourceBounded,
  });

  final String sourceId;
  final String discovery;
  final String cacheScope;
  final bool hostDependent;
  final bool callerConfigurable;
  final bool resourceBounded;
}

/// Resource limits applied while a native backend decodes embedded images.
final class MermanRuntimeEmbeddedImageLimits {
  const MermanRuntimeEmbeddedImageLimits({
    required this.maxBytesPerImage,
    required this.maxTotalBytes,
    required this.maxPixelsPerImage,
    required this.maxTotalPixels,
  });

  final int? maxBytesPerImage;
  final int? maxTotalBytes;
  final int? maxPixelsPerImage;
  final int? maxTotalPixels;
}

/// Embedded-image behavior used by a native binary-output backend.
final class MermanRuntimeEmbeddedImageContract {
  MermanRuntimeEmbeddedImageContract({
    required List<String> sourceIds,
    required this.filesystemAccess,
    required this.networkAccess,
    required this.callerConfigurable,
    required this.limits,
  }) : sourceIds = List.unmodifiable(sourceIds);

  final List<String> sourceIds;
  final bool filesystemAccess;
  final bool networkAccess;
  final bool callerConfigurable;
  final MermanRuntimeEmbeddedImageLimits limits;
}

class MermanRuntimeCatalog {
  MermanRuntimeCatalog._({
    required this.packageVersion,
    required List<int> optionsSchemaVersions,
    required List<MermanRuntimePayloadSchema> payloadSchemas,
    required List<String> metadataIds,
    required this.capabilityIds,
    required this.outputIds,
    required this.operationIds,
    required this.systemAdapterIds,
    required this.textMeasurementProviderIds,
    required this.diagramFamilyCount,
    required this.generalBindingDefaultProfile,
    required this.cliDefaultProfile,
    required List<MermanRuntimeOutputContract> outputContracts,
    required List<MermanResourceLimitDescriptor> resourceLimits,
    required List<MermanResourceProfileDescriptor> resourceProfiles,
  })  : optionsSchemaVersions = List.unmodifiable(optionsSchemaVersions),
        payloadSchemas = List.unmodifiable(payloadSchemas),
        metadataIds = List.unmodifiable(metadataIds),
        outputContracts = List.unmodifiable(outputContracts),
        resourceLimits = List.unmodifiable(resourceLimits),
        resourceProfiles = List.unmodifiable(resourceProfiles),
        outputContractsById = Map.unmodifiable({
          for (final contract in outputContracts) contract.id: contract,
        }),
        resourceLimitsById = Map.unmodifiable({
          for (final limit in resourceLimits) limit.id: limit,
        }),
        resourceProfilesById = Map.unmodifiable({
          for (final profile in resourceProfiles) profile.id: profile,
        });

  factory MermanRuntimeCatalog.fromJson(Map<String, Object?> catalog) {
    _requireRequiredKeys(
      catalog,
      const {
        'schema_version',
        'transport_api_version',
        'package_version',
        'capabilities',
        'output_contracts',
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
    final optionsSchemaVersions = _optionalSortedUniquePositiveInts(
      catalog,
      'options_schema_versions',
      'runtime options schema versions',
    );
    final payloadSchemas = _parseRuntimePayloadSchemas(
      catalog['payload_schemas'],
    );
    final metadataIds = _optionalSortedUniqueStrings(
      catalog,
      'metadata_ids',
      'runtime metadata IDs',
    );

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
    final outputContracts = _parseRuntimeOutputContracts(
      catalog['output_contracts'],
      outputIds,
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
    final resourceContract = _parseRuntimeResources(resources, operationIds);

    return MermanRuntimeCatalog._(
      packageVersion: packageVersion,
      optionsSchemaVersions: optionsSchemaVersions,
      payloadSchemas: payloadSchemas,
      metadataIds: metadataIds,
      capabilityIds: List.unmodifiable(capabilityIds),
      outputIds: List.unmodifiable(outputIds),
      operationIds: List.unmodifiable(operationIds),
      systemAdapterIds: List.unmodifiable(systemAdapterIds),
      textMeasurementProviderIds: List.unmodifiable(providers),
      diagramFamilyCount: diagramFamilyCount,
      generalBindingDefaultProfile:
          resourceContract.generalBindingDefaultProfile,
      cliDefaultProfile: resourceContract.cliDefaultProfile,
      outputContracts: outputContracts,
      resourceLimits: resourceContract.limits,
      resourceProfiles: resourceContract.profiles,
    );
  }

  final String packageVersion;
  final List<int> optionsSchemaVersions;
  final List<MermanRuntimePayloadSchema> payloadSchemas;
  final List<String> metadataIds;
  final List<String> capabilityIds;
  final List<String> outputIds;
  final List<String> operationIds;
  final List<String> systemAdapterIds;
  final List<String> textMeasurementProviderIds;
  final int diagramFamilyCount;
  final String generalBindingDefaultProfile;
  final String cliDefaultProfile;
  final List<MermanRuntimeOutputContract> outputContracts;
  final List<MermanResourceLimitDescriptor> resourceLimits;
  final List<MermanResourceProfileDescriptor> resourceProfiles;
  final Map<String, MermanRuntimeOutputContract> outputContractsById;
  final Map<String, MermanResourceLimitDescriptor> resourceLimitsById;
  final Map<String, MermanResourceProfileDescriptor> resourceProfilesById;

  bool supportsCapability(String id) => capabilityIds.contains(id);
  bool supportsOutput(String id) => outputIds.contains(id);
  bool supportsOperation(String id) => operationIds.contains(id);
  bool supportsPayloadSchema(String id, int version) => payloadSchemas.any(
        (schema) => schema.id == id && schema.version == version,
      );

  void requireCurrentBindingSchemas() {
    if (!optionsSchemaVersions.contains(mermanBindingOptionsSchemaVersion)) {
      throw MermanException.contract(
        'runtime catalog does not advertise Options JSON schema '
        '$mermanBindingOptionsSchemaVersion',
      );
    }
    if (!supportsPayloadSchema(
      'binding-result',
      native.MERMAN_NATIVE_RESULT_SCHEMA_VERSION,
    )) {
      throw MermanException.contract(
        'runtime catalog does not advertise binding-result schema '
        '${native.MERMAN_NATIVE_RESULT_SCHEMA_VERSION}',
      );
    }
  }

  MermanResourceProfileDescriptor get generalBindingDefaultResourceProfile =>
      resourceProfilesById[generalBindingDefaultProfile]!;

  MermanResourceProfileDescriptor get cliDefaultResourceProfile =>
      resourceProfilesById[cliDefaultProfile]!;
}

/// Native ABI 3 facade for Flutter and standalone Dart hosts.
///
/// [close] is idempotent and must be called when the application is done with
/// this object. A host text measurer is immutable constructor state. Use
/// [reusableEngine] for additional independently configured engines.
class Merman {
  Merman._(this._native, this.runtimeCatalog, this._defaultEngine);

  factory Merman.fromDynamicLibrary(
    ffi.DynamicLibrary library, {
    String? optionsJson,
    MermanTextMeasurer? textMeasurer,
    String? expectedPackageVersion,
  }) =>
      Merman._load(
        library,
        optionsJson: optionsJson,
        textMeasurer: textMeasurer,
        expectedPackageVersion: expectedPackageVersion,
        requireMetadataCollection: false,
        requireCurrentBindingSchemas: false,
      );

  static Merman _load(
    ffi.DynamicLibrary library, {
    required String? optionsJson,
    required MermanTextMeasurer? textMeasurer,
    required String? expectedPackageVersion,
    required bool requireMetadataCollection,
    required bool requireCurrentBindingSchemas,
  }) {
    final nativeApi = _NativeApi.discover(library);
    if (expectedPackageVersion != null &&
        nativeApi.packageVersion != expectedPackageVersion) {
      throw MermanException.contract(
        'native package version `${nativeApi.packageVersion}` does not match '
        'the required `$expectedPackageVersion`',
      );
    }
    if (requireMetadataCollection) {
      nativeApi.requireMetadataCollection();
    }
    final catalog = nativeApi.loadRuntimeCatalog();
    if (requireCurrentBindingSchemas) {
      catalog.requireCurrentBindingSchemas();
    }
    final defaultEngine = nativeApi.createEngine(
      optionsJson: optionsJson,
      textMeasurer: textMeasurer,
    );
    return Merman._(nativeApi, catalog, defaultEngine);
  }

  /// Opens the package-owned library and requires its exact package version.
  factory Merman.open({
    String? optionsJson,
    MermanTextMeasurer? textMeasurer,
  }) =>
      Merman._load(
        openMermanLibrary(),
        optionsJson: optionsJson,
        textMeasurer: textMeasurer,
        expectedPackageVersion: mermanPackageVersion,
        requireMetadataCollection: true,
        requireCurrentBindingSchemas: true,
      );

  /// Opens an ABI-compatible library without pinning its package version.
  factory Merman.openPath(
    String path, {
    String? optionsJson,
    MermanTextMeasurer? textMeasurer,
  }) =>
      Merman.fromDynamicLibrary(
        openMermanLibraryFromPath(path),
        optionsJson: optionsJson,
        textMeasurer: textMeasurer,
      );

  final _NativeApi _native;
  final MermanRuntimeCatalog runtimeCatalog;
  final MermanReusableEngine _defaultEngine;
  List<String>? _supportedDiagramsCache;
  List<MermanAsciiCapability>? _asciiCapabilitiesCache;
  List<MermanDiagramFamilyCapability>? _diagramFamilyCapabilitiesCache;
  List<MermanLintRuleCatalogEntry>? _lintRuleCatalogCache;
  List<String>? _supportedThemesCache;
  MermanPresentationCatalog? _presentationCatalogCache;

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

  /// Returns diagram types exposed by the loaded native artifact.
  List<String> supportedDiagrams() {
    _ensureOpen();
    return _supportedDiagramsCache ??= List.unmodifiable(
      _decodeJsonStringList(
        _native.collectMetadata('supported-diagrams'),
        'supported diagrams',
      ),
    );
  }

  /// Returns ASCII rendering capability records for the loaded artifact.
  List<MermanAsciiCapability> asciiCapabilities() {
    _ensureOpen();
    return _asciiCapabilitiesCache ??= List.unmodifiable(
      _decodeJsonObjectList(
        _native.collectMetadata('ascii-capabilities'),
        'ASCII capabilities',
        MermanAsciiCapability.fromJson,
      ),
    );
  }

  /// Returns parser/render capability records for the loaded artifact.
  List<MermanDiagramFamilyCapability> diagramFamilyCapabilities() {
    _ensureOpen();
    return _diagramFamilyCapabilitiesCache ??= List.unmodifiable(
      _decodeJsonObjectList(
        _native.collectMetadata('diagram-family-capabilities'),
        'diagram family capabilities',
        MermanDiagramFamilyCapability.fromJson,
      ),
    );
  }

  /// Returns governed lint rule metadata for the loaded artifact.
  List<MermanLintRuleCatalogEntry> lintRuleCatalog() {
    _ensureOpen();
    return _lintRuleCatalogCache ??= List.unmodifiable(
      _decodeJsonObjectListFromField(
        _native.collectMetadata('lint-rule-catalog'),
        'lint rule catalog',
        'rules',
        MermanLintRuleCatalogEntry.fromJson,
      ),
    );
  }

  /// Returns built-in Mermaid theme names.
  List<String> supportedThemes() {
    _ensureOpen();
    return _supportedThemesCache ??= List.unmodifiable(
      _decodeJsonStringList(
        _native.collectMetadata('supported-themes'),
        'supported themes',
      ),
    );
  }

  /// Returns artifact-owned presentation theme and profile metadata.
  MermanPresentationCatalog presentationCatalog() {
    _ensureOpen();
    return _presentationCatalogCache ??= MermanPresentationCatalog.fromJson(
      _decodeJsonObject(
        _native.collectMetadata('presentation-catalog'),
        'presentation catalog',
      ),
    );
  }

  /// Tries to close the default engine without waiting.
  ///
  /// BUSY and REENTRANT failures leave this instance open so the caller can
  /// retry after the active operation or callback returns.
  void close() => _defaultEngine.close();

  /// Alias for [close] for APIs that use `dispose` naming.
  void dispose() => close();

  void _ensureOpen() {
    if (_defaultEngine.isDisposed) {
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
  int? _token;
  final _TextMeasurementRegistration? _textMeasurement;
  bool _activeCall = false;

  bool get isDisposed => _token == null;

  MermanOperationResult execute(
    MermanOperation operation,
    String source, {
    String? uri,
    String? optionsJson,
  }) {
    final token = _requireToken();
    if (operation.requiresUri != (uri != null)) {
      throw MermanException.contract(
        'operation `${operation.operationId}` ${operation.requiresUri ? 'requires' : 'does not accept'} a URI',
      );
    }
    return _withNativeCall(
      () => _native.execute(
        token,
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

  /// Tries to close this engine without waiting. Safe to call more than once.
  ///
  /// A BUSY or REENTRANT failure retains the native token and the immutable
  /// callback registration so [close] can be retried.
  void close() {
    final token = _token;
    if (token == null) {
      return;
    }
    if (_activeCall) {
      throw const MermanReentrantCallException(
        code: native.MERMAN_NATIVE_STATUS_REENTRANT_CALL,
        codeName: 'reentrant-call',
        message: 'Merman engine cannot be closed from a native callback',
      );
    }
    _native.tryCloseEngine(token);
    _token = null;
    _textMeasurement?.dispose();
  }

  void dispose() => close();

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
      throw const MermanReentrantCallException(
        code: native.MERMAN_NATIVE_STATUS_REENTRANT_CALL,
        codeName: 'reentrant-call',
        message: 'Merman engine cannot be re-entered from a native callback',
      );
    }
    _activeCall = true;
    try {
      return body();
    } finally {
      _activeCall = false;
    }
  }

  int _requireToken() {
    final token = _token;
    if (token == null) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_ENGINE_CLOSED',
        message: 'Merman reusable engine is disposed',
      );
    }
    return token;
  }
}

class _NativeApi {
  _NativeApi._({
    required this.packageVersion,
    required native.DartMermanNativeRuntimeCatalogFnFunction runtimeCatalog,
    required native.DartMermanNativeEngineNewFnFunction engineNew,
    required native.DartMermanNativeEngineTryCloseFnFunction engineTryClose,
    required native.DartMermanNativeExecuteCollectFnFunction executeCollect,
    required native.DartMermanNativeResultFreeFnFunction resultFree,
    required native.DartMermanNativeMetadataCollectFnFunction? metadataCollect,
  })  : _runtimeCatalog = runtimeCatalog,
        _engineNew = engineNew,
        _engineTryClose = engineTryClose,
        _executeCollect = executeCollect,
        _resultFree = resultFree,
        _metadataCollect = metadataCollect;

  final String packageVersion;
  final native.DartMermanNativeRuntimeCatalogFnFunction _runtimeCatalog;
  final native.DartMermanNativeEngineNewFnFunction _engineNew;
  final native.DartMermanNativeEngineTryCloseFnFunction _engineTryClose;
  final native.DartMermanNativeExecuteCollectFnFunction _executeCollect;
  final native.DartMermanNativeResultFreeFnFunction _resultFree;
  final native.DartMermanNativeMetadataCollectFnFunction? _metadataCollect;

  factory _NativeApi.discover(ffi.DynamicLibrary library) {
    final entry = native.MermanNativeBindings(library);
    final request = calloc<native.MermanNativeApiRequest>();
    final api = calloc<native.MermanNativeApi>();
    final allocations = _NativeAllocationScope();
    try {
      _writeSlice(
        request.ref.expected_minimum_prefix_layout_digest,
        utf8.encode(
          native.MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST,
        ),
        allocations,
      );
      request.ref.struct_size = ffi.sizeOf<native.MermanNativeApiRequest>();
      request.ref.expected_abi_version = native.MERMAN_NATIVE_ABI_VERSION;
      final consumerTableSize = ffi.sizeOf<native.MermanNativeApi>();
      if (consumerTableSize < native.MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE) {
        throw MermanException.contract(
          'generated native API table is smaller than the ABI 3 minimum prefix',
        );
      }
      api.ref.struct_size = consumerTableSize;

      final status = entry.merman_get_native_api(request, api);
      if (status != native.MERMAN_NATIVE_STATUS_OK) {
        throw MermanException(
          code: status,
          codeName: 'DART_ABI_DISCOVERY_FAILED',
          message: 'merman_get_native_api rejected the ABI 3 request',
        );
      }

      final table = api.ref;
      if (table.struct_size < native.MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE) {
        throw MermanException.contract(
          'native API table is smaller than the ABI 3 minimum prefix',
        );
      }
      if (table.abi_version != native.MERMAN_NATIVE_ABI_VERSION) {
        throw MermanException.contract(
          'native API version `${table.abi_version}` is not ABI 3',
        );
      }
      final minimumPrefixLayoutDigest = _utf8FromSlice(
        table.minimum_prefix_layout_digest,
        'minimum-prefix layout digest',
      );
      if (minimumPrefixLayoutDigest !=
          native.MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST) {
        throw MermanException.contract(
          'native API minimum-prefix digest does not match the generated header',
        );
      }
      final fullDescriptorDigest = _utf8FromSlice(
        table.full_descriptor_digest,
        'full descriptor digest',
      );
      final capabilityCatalogDigest = _utf8FromSlice(
        table.capability_catalog_digest,
        'capability catalog digest',
      );
      final packageVersion = _utf8FromSlice(
        table.package_version,
        'package version',
      );
      if (fullDescriptorDigest.isEmpty ||
          capabilityCatalogDigest.isEmpty ||
          packageVersion.isEmpty) {
        throw MermanException.contract(
          'native API provenance fields must not be empty',
        );
      }
      _requireFunctionPointer(table.runtime_catalog, 'runtime_catalog');
      _requireFunctionPointer(table.engine_new, 'engine_new');
      _requireFunctionPointer(table.engine_try_close, 'engine_try_close');
      _requireFunctionPointer(table.execute_collect, 'execute_collect');
      _requireFunctionPointer(table.result_free, 'result_free');
      native.DartMermanNativeMetadataCollectFnFunction? metadataCollect;
      if (_nativeApiHasMetadataCollectSlot(table.struct_size)) {
        _requireFunctionPointer(table.metadata_collect, 'metadata_collect');
        metadataCollect = table.metadata_collect
            .asFunction<native.DartMermanNativeMetadataCollectFnFunction>();
      }

      return _NativeApi._(
        packageVersion: packageVersion,
        runtimeCatalog: table.runtime_catalog
            .asFunction<native.DartMermanNativeRuntimeCatalogFnFunction>(),
        engineNew: table.engine_new
            .asFunction<native.DartMermanNativeEngineNewFnFunction>(),
        engineTryClose: table.engine_try_close
            .asFunction<native.DartMermanNativeEngineTryCloseFnFunction>(),
        executeCollect: table.execute_collect
            .asFunction<native.DartMermanNativeExecuteCollectFnFunction>(),
        resultFree: table.result_free
            .asFunction<native.DartMermanNativeResultFreeFnFunction>(),
        metadataCollect: metadataCollect,
      );
    } finally {
      allocations.dispose();
      calloc.free(request);
      calloc.free(api);
    }
  }

  void requireMetadataCollection() {
    if (_metadataCollect == null) {
      throw MermanException.contract(
        'the exact-version native library does not expose the current '
        '`metadata_collect` ABI 3 slot',
      );
    }
  }

  Uint8List collectMetadata(String metadataId) {
    final collect = _metadataCollect;
    if (collect == null) {
      throw MermanException.contract(
        'the compatible ABI 3 library does not expose the optional '
        '`metadata_collect` slot required for `$metadataId`',
      );
    }

    final id = calloc<native.MermanNativeSlice>();
    final allocations = _NativeAllocationScope();
    final result = _NativeResult.allocate(_resultFree);
    try {
      _writeSlice(id.ref, utf8.encode(metadataId), allocations);
      final status = collect(id.ref, result.pointer);
      result.requireWritten(status);
      final record = result.pointer.ref;
      final metadata = _copyBuffer(record.metadata_or_error_json);
      _ensureResultStatus(status, record.status, metadata);
      if (record.operation != native.MERMAN_NATIVE_OPERATION_NONE ||
          record.data.len != 0) {
        throw MermanException.contract(
          'native metadata collection returned an operation payload',
        );
      }
      return metadata;
    } finally {
      result.dispose();
      allocations.dispose();
      calloc.free(id);
    }
  }

  MermanRuntimeCatalog loadRuntimeCatalog() {
    final result = _NativeResult.allocate(_resultFree);
    try {
      final status = _runtimeCatalog(result.pointer);
      result.requireWritten(status);
      final record = result.pointer.ref;
      final metadata = _copyBuffer(record.metadata_or_error_json);
      _ensureResultStatus(status, record.status, metadata);
      final catalog = MermanRuntimeCatalog.fromJson(
        _decodeJsonObject(metadata, 'runtime catalog'),
      );
      if (catalog.packageVersion != packageVersion) {
        throw MermanException.contract(
          'runtime catalog package version `${catalog.packageVersion}` does '
          'not match the discovered table `$packageVersion`',
        );
      }
      return catalog;
    } finally {
      result.dispose();
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
    final result = _NativeResult.allocate(_resultFree);
    final allocations = _NativeAllocationScope();
    var unownedToken = 0;
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

      final status = _engineNew(config, token, result.pointer);
      unownedToken = token.value;
      result.requireWritten(status);
      final record = result.pointer.ref;
      final metadata = _copyBuffer(record.metadata_or_error_json);
      _ensureResultStatus(status, record.status, metadata);
      if (token.value == 0) {
        throw MermanException.contract(
          'native engine creation succeeded without an engine token',
        );
      }
      final engine = MermanReusableEngine._(
        this,
        token.value,
        registration,
      );
      unownedToken = 0;
      return engine;
    } catch (_) {
      if (unownedToken != 0) {
        _engineTryClose(unownedToken);
      }
      registration?.dispose();
      rethrow;
    } finally {
      allocations.dispose();
      result.dispose();
      calloc.free(config);
      calloc.free(token);
    }
  }

  void tryCloseEngine(int token) {
    final status = _engineTryClose(token);
    switch (status) {
      case native.MERMAN_NATIVE_STATUS_OK:
        return;
      case native.MERMAN_NATIVE_STATUS_BUSY:
      case native.MERMAN_NATIVE_STATUS_REENTRANT_CALL:
        throw MermanException.fromNative(status, Uint8List(0));
      default:
        throw MermanException(
          code: status,
          codeName: 'DART_ENGINE_CLOSE_FAILED',
          message: 'native engine close failed without retiring its token',
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
    final result = _NativeResult.allocate(_resultFree);
    try {
      final status = _executeCollect(
        engine,
        request.pointer,
        result.pointer,
      );
      result.requireWritten(status);
      final record = result.pointer.ref;
      final metadata = _copyBuffer(record.metadata_or_error_json);
      _ensureResultStatus(status, record.status, metadata);
      if (record.operation != operation.nativeCode) {
        throw MermanException.contract(
          'native operation does not match the requested `${operation.operationId}`',
        );
      }
      return MermanOperationResult(
        operation: operation,
        mediaType: _utf8FromSlice(record.media_type, 'result media type'),
        bytes: _copyBuffer(record.data),
        metadata: _decodeJsonObject(metadata, 'result metadata'),
      );
    } finally {
      result.dispose();
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

class _NativeResult {
  _NativeResult._(this.pointer, this._resultFree);

  factory _NativeResult.allocate(
    native.DartMermanNativeResultFreeFnFunction resultFree,
  ) {
    final pointer = calloc<native.MermanNativeResult>();
    pointer.ref.struct_size = ffi.sizeOf<native.MermanNativeResult>();
    return _NativeResult._(pointer, resultFree);
  }

  final ffi.Pointer<native.MermanNativeResult> pointer;
  final native.DartMermanNativeResultFreeFnFunction _resultFree;
  bool _disposed = false;

  void requireWritten(int callStatus) {
    _requireNativeResultWritten(pointer.ref, callStatus);
  }

  void dispose() {
    if (_disposed) {
      return;
    }
    _resultFree(pointer);
    calloc.free(pointer);
    _disposed = true;
  }
}

/// Validates a native result write without requiring a live native library.
///
/// This package-internal test seam exercises the allocation-token exhaustion
/// exception in the frozen ABI 3 ownership contract.
void validateNativeResultForTesting(
  ffi.Pointer<native.MermanNativeResult> pointer,
  int callStatus,
) {
  _requireNativeResultWritten(pointer.ref, callStatus);
}

/// Reports whether a producer-reported ABI table size includes the appended
/// metadata slot without reading bytes outside the producer-written prefix.
bool nativeApiHasMetadataCollectForTesting(int producerTableSize) =>
    _nativeApiHasMetadataCollectSlot(producerTableSize);

bool _nativeApiHasMetadataCollectSlot(int producerTableSize) =>
    producerTableSize >= native.MERMAN_NATIVE_API_METADATA_COLLECT_PREFIX_SIZE;

void _requireNativeResultWritten(
  native.MermanNativeResult result,
  int callStatus,
) {
  if (result.struct_size != ffi.sizeOf<native.MermanNativeResult>()) {
    throw MermanException.contract(
      'native result has an unexpected struct size `${result.struct_size}`',
    );
  }
  if (result.allocation_token != 0) {
    return;
  }
  if (callStatus == native.MERMAN_NATIVE_STATUS_INTERNAL_ERROR &&
      _isCallerInitializedResult(result)) {
    throw MermanException.fromNative(callStatus, Uint8List(0));
  }
  throw MermanException.contract(
    'native producing call returned without an allocation token',
  );
}

bool _isCallerInitializedResult(native.MermanNativeResult result) {
  return result.status == 0 &&
      result.operation == 0 &&
      result.media_type.struct_size == 0 &&
      result.media_type.data.address == 0 &&
      result.media_type.len == 0 &&
      result.data.struct_size == 0 &&
      result.data.data.address == 0 &&
      result.data.len == 0 &&
      result.metadata_or_error_json.struct_size == 0 &&
      result.metadata_or_error_json.data.address == 0 &&
      result.metadata_or_error_json.len == 0;
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

final class _ParsedRuntimeResources {
  const _ParsedRuntimeResources({
    required this.generalBindingDefaultProfile,
    required this.cliDefaultProfile,
    required this.limits,
    required this.profiles,
  });

  final String generalBindingDefaultProfile;
  final String cliDefaultProfile;
  final List<MermanResourceLimitDescriptor> limits;
  final List<MermanResourceProfileDescriptor> profiles;
}

_ParsedRuntimeResources _parseRuntimeResources(
  Map<String, Object?> resources,
  List<String> operationIds,
) {
  final generalBindingDefaultProfile = _requiredNonEmptyString(
    resources,
    'general_binding_default_profile',
    'runtime resources',
  );
  final cliDefaultProfile = _requiredNonEmptyString(
    resources,
    'cli_default_profile',
    'runtime resources',
  );

  final rawLimits = resources['limits'];
  if (rawLimits is! List || rawLimits.isEmpty) {
    throw MermanException.contract(
      'runtime resources.limits must be a non-empty array',
    );
  }
  final limits = <MermanResourceLimitDescriptor>[];
  final limitIds = <String>{};
  for (var index = 0; index < rawLimits.length; index += 1) {
    final label = 'runtime resources.limits[$index]';
    final limit = _asObject(rawLimits[index], label);
    _requireRequiredKeys(
      limit,
      const {
        'id',
        'phase',
        'description',
        'overridable',
        'hard_cap',
      },
      label,
    );
    final id = _requiredNonEmptyString(limit, 'id', label);
    if (!limitIds.add(id)) {
      throw MermanException.contract(
        'runtime resource limit ID `$id` is duplicated',
      );
    }
    final overridable = _requiredBool(limit, 'overridable', label);
    final hardCap = _requiredBool(limit, 'hard_cap', label);
    if (hardCap && overridable) {
      throw MermanException.contract(
        'runtime resource limit `$id` cannot be both a hard cap and overridable',
      );
    }
    final limitOperationIds = _optionalSortedUniqueStrings(
      limit,
      'operation_ids',
      '$label.operation_ids',
    );
    if (!operationIds.toSet().containsAll(limitOperationIds)) {
      throw MermanException.contract(
        '$label.operation_ids must be declared runtime operation IDs',
      );
    }
    limits.add(MermanResourceLimitDescriptor(
      id: id,
      phase: _requiredNonEmptyString(limit, 'phase', label),
      description: _requiredNonEmptyString(limit, 'description', label),
      overridable: overridable,
      hardCap: hardCap,
      minimumValue: _optionalNonNegativeInt(
        limit,
        'minimum_value',
        label,
        legacyDefault: 1,
      ),
      operationIds: limitOperationIds,
    ));
  }

  final rawProfiles = resources['profiles'];
  if (rawProfiles is! List || rawProfiles.isEmpty) {
    throw MermanException.contract(
      'runtime resources.profiles must be a non-empty array',
    );
  }
  final profiles = <MermanResourceProfileDescriptor>[];
  final profileIds = <String>{};
  for (var index = 0; index < rawProfiles.length; index += 1) {
    final label = 'runtime resources.profiles[$index]';
    final profile = _asObject(rawProfiles[index], label);
    _requireRequiredKeys(
      profile,
      const {
        'id',
        'purpose',
        'trust_assumption',
        'recommended_binding_default',
        'limits',
      },
      label,
    );
    final id = _requiredNonEmptyString(profile, 'id', label);
    if (!profileIds.add(id)) {
      throw MermanException.contract(
        'runtime resource profile ID `$id` is duplicated',
      );
    }

    final rawProfileLimits = _asObject(
      profile['limits'],
      '$label.limits',
    );
    final profileLimitIds = rawProfileLimits.keys.toSet();
    final missingLimitIds = limitIds.difference(profileLimitIds);
    final unknownLimitIds = profileLimitIds.difference(limitIds);
    if (missingLimitIds.isNotEmpty || unknownLimitIds.isNotEmpty) {
      throw MermanException.contract(
        '$label.limits must contain exactly the declared resource limit IDs; '
        'missing: ${missingLimitIds.toList()..sort()}, '
        'unknown: ${unknownLimitIds.toList()..sort()}',
      );
    }

    final profileLimits = <String, int?>{};
    for (final limit in limits) {
      final value = rawProfileLimits[limit.id];
      if (value != null && (value is! int || value < limit.minimumValue)) {
        throw MermanException.contract(
          '$label.limits[`${limit.id}`] must be null or at least ${limit.minimumValue}',
        );
      }
      if (limit.hardCap && value == null) {
        throw MermanException.contract(
          '$label.limits[`${limit.id}`] must retain its finite hard cap',
        );
      }
      profileLimits[limit.id] = value as int?;
    }
    profiles.add(MermanResourceProfileDescriptor(
      id: id,
      purpose: _requiredNonEmptyString(profile, 'purpose', label),
      trustAssumption:
          _requiredNonEmptyString(profile, 'trust_assumption', label),
      recommendedBindingDefault:
          _requiredBool(profile, 'recommended_binding_default', label),
      limits: Map.unmodifiable(profileLimits),
    ));
  }

  if (!profileIds.contains(generalBindingDefaultProfile)) {
    throw MermanException.contract(
      'runtime general binding default resource profile '
      '`$generalBindingDefaultProfile` is not declared',
    );
  }
  if (!profileIds.contains(cliDefaultProfile)) {
    throw MermanException.contract(
      'runtime CLI default resource profile `$cliDefaultProfile` is not declared',
    );
  }
  final recommendedProfiles =
      profiles.where((profile) => profile.recommendedBindingDefault).toList();
  if (recommendedProfiles.length != 1 ||
      recommendedProfiles.single.id != generalBindingDefaultProfile) {
    throw MermanException.contract(
      'runtime resources must recommend exactly the general binding default profile',
    );
  }

  return _ParsedRuntimeResources(
    generalBindingDefaultProfile: generalBindingDefaultProfile,
    cliDefaultProfile: cliDefaultProfile,
    limits: limits,
    profiles: profiles,
  );
}

List<MermanRuntimeOutputContract> _parseRuntimeOutputContracts(
  Object? value,
  List<String> outputIds,
) {
  if (value is! List) {
    throw MermanException.contract('runtime output contracts must be an array');
  }

  final contracts = <MermanRuntimeOutputContract>[];
  for (final item in value) {
    final contract = _asObject(item, 'runtime output contract');
    _requireRequiredKeys(
      contract,
      const {'id', 'media_type', 'system_fonts', 'embedded_images'},
      'runtime output contract',
    );
    contracts.add(MermanRuntimeOutputContract(
      id: _requiredNonEmptyString(contract, 'id', 'runtime output contract'),
      mediaType: _requiredNonEmptyString(
        contract,
        'media_type',
        'runtime output contract',
      ),
      systemFonts: _parseRuntimeSystemFontContract(contract['system_fonts']),
      embeddedImages: _parseRuntimeEmbeddedImageContract(
        contract['embedded_images'],
      ),
    ));
  }

  if (contracts.length != outputIds.length) {
    throw MermanException.contract(
      'runtime output contract IDs must exactly match runtime output IDs',
    );
  }
  for (var index = 0; index < outputIds.length; index += 1) {
    if (contracts[index].id != outputIds[index]) {
      throw MermanException.contract(
        'runtime output contract IDs must exactly match runtime output IDs',
      );
    }
  }
  return contracts;
}

MermanRuntimeSystemFontContract? _parseRuntimeSystemFontContract(
  Object? value,
) {
  if (value == null) {
    return null;
  }
  final fonts = _asObject(value, 'runtime system font contract');
  _requireRequiredKeys(
    fonts,
    const {
      'source_id',
      'discovery',
      'cache_scope',
      'host_dependent',
      'caller_configurable',
      'resource_bounded',
    },
    'runtime system font contract',
  );
  return MermanRuntimeSystemFontContract(
    sourceId: _requiredNonEmptyString(
      fonts,
      'source_id',
      'runtime system font contract',
    ),
    discovery: _requiredNonEmptyString(
      fonts,
      'discovery',
      'runtime system font contract',
    ),
    cacheScope: _requiredNonEmptyString(
      fonts,
      'cache_scope',
      'runtime system font contract',
    ),
    hostDependent: _requiredBool(
      fonts,
      'host_dependent',
      'runtime system font contract',
    ),
    callerConfigurable: _requiredBool(
      fonts,
      'caller_configurable',
      'runtime system font contract',
    ),
    resourceBounded: _requiredBool(
      fonts,
      'resource_bounded',
      'runtime system font contract',
    ),
  );
}

MermanRuntimeEmbeddedImageContract? _parseRuntimeEmbeddedImageContract(
  Object? value,
) {
  if (value == null) {
    return null;
  }
  final images = _asObject(value, 'runtime embedded image contract');
  _requireRequiredKeys(
    images,
    const {
      'source_ids',
      'filesystem_access',
      'network_access',
      'caller_configurable',
      'limits',
    },
    'runtime embedded image contract',
  );
  final limits = _asObject(images['limits'], 'runtime embedded image limits');
  _requireRequiredKeys(
    limits,
    const {
      'max_bytes_per_image',
      'max_total_bytes',
      'max_pixels_per_image',
      'max_total_pixels',
    },
    'runtime embedded image limits',
  );
  return MermanRuntimeEmbeddedImageContract(
    sourceIds: _requiredSortedUniqueStrings(
      images,
      'source_ids',
      'runtime embedded image source IDs',
    ),
    filesystemAccess: _requiredBool(
      images,
      'filesystem_access',
      'runtime embedded image contract',
    ),
    networkAccess: _requiredBool(
      images,
      'network_access',
      'runtime embedded image contract',
    ),
    callerConfigurable: _requiredBool(
      images,
      'caller_configurable',
      'runtime embedded image contract',
    ),
    limits: MermanRuntimeEmbeddedImageLimits(
      maxBytesPerImage: _requiredNullablePositiveInt(
        limits,
        'max_bytes_per_image',
        'runtime embedded image limits',
      ),
      maxTotalBytes: _requiredNullablePositiveInt(
        limits,
        'max_total_bytes',
        'runtime embedded image limits',
      ),
      maxPixelsPerImage: _requiredNullablePositiveInt(
        limits,
        'max_pixels_per_image',
        'runtime embedded image limits',
      ),
      maxTotalPixels: _requiredNullablePositiveInt(
        limits,
        'max_total_pixels',
        'runtime embedded image limits',
      ),
    ),
  );
}

Map<String, Object?> _decodeJsonObject(Uint8List bytes, String label) {
  final decoded = jsonDecode(utf8.decode(bytes));
  return _asObject(decoded, label);
}

List<String> _decodeJsonStringList(Uint8List bytes, String label) {
  final decoded = jsonDecode(utf8.decode(bytes));
  if (decoded is! List || !decoded.every((item) => item is String)) {
    throw MermanException.contract('$label must be a JSON string array');
  }
  return decoded.cast<String>();
}

List<T> _decodeJsonObjectList<T>(
  Uint8List bytes,
  String label,
  T Function(Map<String, Object?> json) decode,
) {
  final decoded = jsonDecode(utf8.decode(bytes));
  if (decoded is! List) {
    throw MermanException.contract('$label must be a JSON array');
  }
  return decoded.indexed
      .map((entry) => decode(_asObject(entry.$2, '$label[${entry.$1}]')))
      .toList(growable: false);
}

List<T> _decodeJsonObjectListFromField<T>(
  Uint8List bytes,
  String label,
  String field,
  T Function(Map<String, Object?> json) decode,
) {
  final object = _decodeJsonObject(bytes, label);
  final values = object[field];
  if (values is! List) {
    throw MermanException.contract('$label.$field must be a JSON array');
  }
  return values.indexed
      .map(
        (entry) => decode(
          _asObject(entry.$2, '$label.$field[${entry.$1}]'),
        ),
      )
      .toList(growable: false);
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

int _requiredNonNegativeInt(
  Map<String, Object?> source,
  String key,
  String label,
) {
  final value = source[key];
  if (value is! int || value < 0) {
    throw MermanException.contract(
      '$label.$key must be a non-negative integer',
    );
  }
  return value;
}

int _optionalNonNegativeInt(
  Map<String, Object?> source,
  String key,
  String label, {
  required int legacyDefault,
}) {
  if (!source.containsKey(key)) {
    return legacyDefault;
  }
  return _requiredNonNegativeInt(source, key, label);
}

int? _requiredNullablePositiveInt(
  Map<String, Object?> source,
  String key,
  String label,
) {
  final value = source[key];
  if (value == null) {
    return null;
  }
  if (value is! int || value <= 0) {
    throw MermanException.contract(
      '$label.$key must be a positive integer or null',
    );
  }
  return value;
}

String _requiredNonEmptyString(
  Map<String, Object?> source,
  String key,
  String label,
) {
  final value = source[key];
  if (value is! String || value.isEmpty) {
    throw MermanException.contract(
      '$label.$key must be a non-empty string',
    );
  }
  return value;
}

String _requiredString(
  Map<String, Object?> source,
  String key,
  String label,
) {
  final value = source[key];
  if (value is! String) {
    throw MermanException.contract('$label.$key must be a string');
  }
  return value;
}

bool _requiredBool(
  Map<String, Object?> source,
  String key,
  String label,
) {
  final value = source[key];
  if (value is! bool) {
    throw MermanException.contract('$label.$key must be a boolean');
  }
  return value;
}

List<String> _requiredStringList(
  Map<String, Object?> source,
  String key,
  String label,
) {
  final value = source[key];
  if (value is! List || !value.every((item) => item is String)) {
    throw MermanException.contract('$label must be a string array');
  }
  return value.cast<String>();
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

List<String> _optionalSortedUniqueStrings(
  Map<String, Object?> source,
  String key,
  String label,
) {
  if (!source.containsKey(key)) {
    return const [];
  }
  return _requiredSortedUniqueStrings(source, key, label);
}

List<int> _optionalSortedUniquePositiveInts(
  Map<String, Object?> source,
  String key,
  String label,
) {
  if (!source.containsKey(key)) {
    return const [];
  }
  final value = source[key];
  if (value is! List) {
    throw MermanException.contract('$label must be an array');
  }
  final values = <int>[];
  int? previous;
  for (final item in value) {
    if (item is! int || item <= 0) {
      throw MermanException.contract(
        '$label must contain positive integer versions',
      );
    }
    if (previous != null && previous >= item) {
      throw MermanException.contract('$label must be sorted and unique');
    }
    previous = item;
    values.add(item);
  }
  return values;
}

List<MermanRuntimePayloadSchema> _parseRuntimePayloadSchemas(Object? value) {
  if (value == null) {
    return const [];
  }
  if (value is! List) {
    throw MermanException.contract('runtime payload schemas must be an array');
  }
  final schemas = <MermanRuntimePayloadSchema>[];
  String? previous;
  for (var index = 0; index < value.length; index += 1) {
    final label = 'runtime payload schemas[$index]';
    final schema = _asObject(value[index], label);
    _requireRequiredKeys(schema, const {'id', 'version'}, label);
    final id = _requiredNonEmptyString(schema, 'id', label);
    if (previous != null && previous.compareTo(id) >= 0) {
      throw MermanException.contract(
        'runtime payload schema IDs must be sorted and unique',
      );
    }
    final version = _requiredInt(schema, 'version');
    if (version <= 0) {
      throw MermanException.contract('$label.version must be positive');
    }
    previous = id;
    schemas.add(MermanRuntimePayloadSchema(id: id, version: version));
  }
  return schemas;
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
  if (callStatus != resultStatus) {
    throw MermanException.contract(
      'native call status `$callStatus` does not match result status '
      '`$resultStatus`',
    );
  }
  if (callStatus != native.MERMAN_NATIVE_STATUS_OK) {
    throw MermanException.fromNative(callStatus, metadata);
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
