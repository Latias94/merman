import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import 'generated/binding_contract.dart';
import 'generated/native_abi.dart' as native;
import 'generated/native_operations.dart';
import 'generated/package_version.dart';
import 'generated/resource_options.dart'
    show MermanResourceLimitId, mermanBindingOptionsSchemaVersion;
import 'generated/text_measurement_protocol.dart';
import 'operation_metadata.dart';

export 'generated/native_operations.dart' show MermanOperation;

/// Opens a native `merman-ffi` library at [path].
///
/// This exists for local Dart smoke tests and non-Flutter host applications.
ffi.DynamicLibrary openMermanLibraryFromPath(String path) =>
    ffi.DynamicLibrary.open(path);

/// A binary-safe generic-operation result returned by the ABI 3 table.
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
  final MermanOperationMetadata metadata;

  /// Decodes a UTF-8 output such as SVG, ASCII, or JSON.
  String get utf8Text => utf8.decode(bytes);

  /// Decodes a JSON object output.
  Map<String, Object?> get jsonObject => _decodeJsonObject(bytes, 'output');
}

/// Opaque native control shared with one or more synchronous operations.
///
/// Cancellation is cooperative: an in-flight operation stops at its next
/// checkpoint. [release] retires only the token; an operation that already
/// cloned the control remains safe until that call returns.
final class MermanOperationControl {
  MermanOperationControl._(this._native, this._token);

  final _NativeApi _native;
  int? _token;

  bool get isReleased => _token == null;

  void cancel() => _native.cancelOperationControl(_requireToken());

  void release() {
    final token = _token;
    if (token == null) {
      return;
    }
    _native.releaseOperationControl(token);
    _token = null;
  }

  int _borrowFor(_NativeApi nativeApi) {
    if (!identical(_native, nativeApi)) {
      throw MermanException.contract(
        'operation control belongs to a different native ABI table',
      );
    }
    return _requireToken();
  }

  int _requireToken() {
    final token = _token;
    if (token == null) {
      throw MermanException.contract('operation control has been released');
    }
    return token;
  }
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
    required this.cause,
    required this.limitId,
    required this.phase,
    required this.actual,
    required this.max,
    required this.profile,
  });

  final String cause;
  final MermanResourceLimitId limitId;
  final String phase;
  final int actual;
  final int max;
  final String profile;
}

/// Stable details attached to an icon-registry construction failure.
class MermanIconRegistryErrorDetails {
  const MermanIconRegistryErrorDetails({
    required this.kindId,
    required this.packIndex,
    required this.registrationName,
  });

  final String kindId;
  final int? packIndex;
  final String? registrationName;
}

/// Stable details attached to a cooperative operation cancellation.
class MermanCancellationErrorDetails {
  const MermanCancellationErrorDetails({
    required this.reason,
    required this.phase,
  });

  final String reason;
  final String phase;
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
    this.iconRegistryDetails,
    this.cancellationDetails,
  });

  final int code;
  final String codeName;
  final String message;
  final MermanErrorKind kind;
  final String? capabilityId;
  final MermanResourceErrorDetails? resourceDetails;
  final MermanIconRegistryErrorDetails? iconRegistryDetails;
  final MermanCancellationErrorDetails? cancellationDetails;

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
    MermanIconRegistryErrorDetails? iconRegistryDetails;
    MermanCancellationErrorDetails? cancellationDetails;
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
            final cause = resource['cause'];
            final limitId = resource['limit_id'];
            final phase = resource['phase'];
            final actual = resource['actual'];
            final max = resource['max'];
            final profile = resource['profile'];
            if (cause is String &&
                cause.isNotEmpty &&
                limitId is String &&
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
                cause: cause,
                limitId: MermanResourceLimitId.fromId(limitId),
                phase: phase,
                actual: actual,
                max: max,
                profile: profile,
              );
            }
          }
          final iconRegistry = details['icon_registry'];
          if (iconRegistry is Map) {
            final kindId = iconRegistry['kind_id'];
            final packIndex = iconRegistry['pack_index'];
            final registrationName = iconRegistry['registration_name'];
            if (kindId is String &&
                kindId.isNotEmpty &&
                (packIndex == null || (packIndex is int && packIndex >= 0)) &&
                (registrationName == null || registrationName is String)) {
              iconRegistryDetails = MermanIconRegistryErrorDetails(
                kindId: kindId,
                packIndex: packIndex as int?,
                registrationName: registrationName as String?,
              );
            }
          }
          final cancellation = details['cancellation'];
          if (cancellation is Map) {
            final reason = cancellation['reason'];
            final phase = cancellation['phase'];
            if (reason is String &&
                reason.isNotEmpty &&
                phase is String &&
                phase.isNotEmpty) {
              cancellationDetails = MermanCancellationErrorDetails(
                reason: reason,
                phase: phase,
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
    if (status == native.MERMAN_NATIVE_STATUS_CANCELLED &&
        cancellationDetails != null) {
      return MermanCancelledException(
        code: status,
        codeName: codeName,
        message: message,
        cancellationDetails: cancellationDetails,
      );
    }
    return MermanException(
      code: status,
      codeName: codeName,
      message: message,
      kind: kind,
      capabilityId: capabilityId,
      resourceDetails: resourceDetails,
      iconRegistryDetails: iconRegistryDetails,
      cancellationDetails: cancellationDetails,
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

/// A synchronous operation stopped at a cooperative cancellation checkpoint.
class MermanCancelledException extends MermanException {
  const MermanCancelledException({
    required super.code,
    required super.codeName,
    required super.message,
    required MermanCancellationErrorDetails cancellationDetails,
  }) : super(cancellationDetails: cancellationDetails);
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

  factory MermanAsciiCapabilityEvidence.fromJson(Map<String, Object?> json) =>
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
      summaryFallback: _requiredBool(
        json,
        'summary_fallback',
        'ASCII capability',
      ),
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
            _asObject(entry.$2, 'ASCII capability.evidence[${entry.$1}]'),
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

  factory MermanDiagramFamilyCapability.fromJson(Map<String, Object?> json) {
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
      hasHeader: _requiredBool(json, 'has_header', 'diagram family capability'),
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
        category: _requiredString(json, 'category', 'lint rule catalog entry'),
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
typedef MermanTextMeasurer =
    MermanTextMeasureResult? Function(MermanTextMeasureRequest request);

/// One immutable IconifyJSON collection supplied during engine construction.
final class MermanIconPack {
  MermanIconPack({required this.json, this.registrationName}) {
    if (json.isEmpty) {
      throw ArgumentError.value(json, 'json', 'IconifyJSON must not be empty');
    }
    if (registrationName != null && registrationName!.isEmpty) {
      throw ArgumentError.value(
        registrationName,
        'registrationName',
        'Use null when no registration-name override is required',
      );
    }
  }

  final String json;
  final String? registrationName;
}

/// A reusable immutable collection of icon packs.
///
/// The factory snapshots bounded UTF-8 buffers without retaining the source
/// strings. Native borrows those buffers only during engine construction and
/// the engine owns the parsed registry after construction returns.
final class MermanIconPackSet {
  MermanIconPackSet._(List<_EncodedMermanIconPack> encodedPacks)
    : _encodedPacks = List.unmodifiable(encodedPacks);

  factory MermanIconPackSet.fromPacks(Iterable<MermanIconPack> packs) {
    final maxPacks = _iconRegistryResourceLimit('max_icon_registry_packs');
    final maxPackBytes = _iconRegistryResourceLimit('max_icon_pack_bytes');
    final maxInputBytes = _iconRegistryResourceLimit(
      'max_icon_registry_input_bytes',
    );
    final maxRegistrationNameBytes = _iconRegistryResourceLimit(
      'max_icon_registry_prefix_bytes',
    );
    final encoded = <_EncodedMermanIconPack>[];
    var inputBytes = 0;

    for (final pack in packs) {
      final packIndex = encoded.length;
      if (packIndex >= maxPacks.value) {
        _throwIconRegistryResourceLimit(
          maxPacks,
          actual: packIndex + 1,
          message:
              'icon registry pack count exceeds the fixed registry ceiling',
        );
      }

      final registrationName = pack.registrationName;
      final registrationNameLength = registrationName == null
          ? 0
          : _boundedIconRegistryUtf8Length(
              registrationName,
              byteCeiling: maxRegistrationNameBytes.value,
              packIndex: packIndex,
              field: 'registration name',
            );
      if (registrationNameLength > maxRegistrationNameBytes.value) {
        _throwIconRegistryResourceLimit(
          maxRegistrationNameBytes,
          actual: registrationNameLength,
          packIndex: packIndex,
          message: 'icon registration name exceeds the fixed byte ceiling',
        );
      }

      final jsonLength = _boundedIconRegistryUtf8Length(
        pack.json,
        byteCeiling: maxPackBytes.value,
        packIndex: packIndex,
        field: 'IconifyJSON',
        registrationName: registrationName,
      );
      if (jsonLength > maxPackBytes.value) {
        _throwIconRegistryResourceLimit(
          maxPackBytes,
          actual: jsonLength,
          packIndex: packIndex,
          registrationName: registrationName,
          message: 'icon pack bytes exceed the fixed per-pack ceiling',
        );
      }
      final nextInputBytes = inputBytes + jsonLength;
      if (nextInputBytes > maxInputBytes.value) {
        _throwIconRegistryResourceLimit(
          maxInputBytes,
          actual: nextInputBytes,
          packIndex: packIndex,
          registrationName: registrationName,
          message:
              'aggregate icon pack bytes exceed the fixed registry ceiling',
        );
      }

      final encodedRegistrationName = registrationName == null
          ? Uint8List(0)
          : utf8.encoder.convert(registrationName);
      final encodedJson = utf8.encoder.convert(pack.json);
      inputBytes = nextInputBytes;
      encoded.add(
        _EncodedMermanIconPack(
          json: encodedJson,
          registrationName: encodedRegistrationName,
        ),
      );
    }
    return MermanIconPackSet._(encoded);
  }

  final List<_EncodedMermanIconPack> _encodedPacks;

  int get length => _encodedPacks.length;
  bool get isEmpty => _encodedPacks.isEmpty;
}

final class _EncodedMermanIconPack {
  const _EncodedMermanIconPack({
    required this.json,
    required this.registrationName,
  });

  final Uint8List json;
  final Uint8List registrationName;
}

/// Immutable constructor-owned services for one reusable engine.
final class MermanEngineServices {
  const MermanEngineServices({this.iconPackSet, this.textMeasurer});

  final MermanIconPackSet? iconPackSet;
  final MermanTextMeasurer? textMeasurer;

  bool get hasIconPacks => iconPackSet != null && !iconPackSet!.isEmpty;
  bool get isEmpty => !hasIconPacks && textMeasurer == null;
}

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
    Map<String, Object?> additionalFields = const {},
  }) : operationIds = List.unmodifiable(operationIds),
       additionalFields = _deeplyUnmodifiableJsonObject(
         additionalFields,
         'resource limit additional fields',
       );

  final MermanResourceLimitId id;
  final String phase;
  final String description;
  final bool overridable;
  final bool hardCap;
  final int minimumValue;
  final List<String> operationIds;

  /// Additive schema fields unknown to this SDK version.
  ///
  /// Values are defensively copied and recursively unmodifiable.
  final Map<String, Object?> additionalFields;
}

/// One independently versioned binding payload advertised by the loaded artifact.
final class MermanRuntimePayloadSchema {
  const MermanRuntimePayloadSchema({required this.id, required this.version});

  final String id;
  final int version;
}

final class MermanResourceProfileDescriptor {
  MermanResourceProfileDescriptor({
    required this.id,
    required this.purpose,
    required this.trustAssumption,
    required this.recommendedBindingDefault,
    required Map<MermanResourceLimitId, int?> limits,
    Map<String, Object?> additionalFields = const {},
  }) : limits = Map.unmodifiable(limits),
       additionalFields = _deeplyUnmodifiableJsonObject(
         additionalFields,
         'resource profile additional fields',
       );

  final String id;
  final String purpose;
  final String trustAssumption;
  final bool recommendedBindingDefault;
  final Map<MermanResourceLimitId, int?> limits;

  /// Additive schema fields unknown to this SDK version.
  ///
  /// Values are defensively copied and recursively unmodifiable.
  final Map<String, Object?> additionalFields;
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

/// One fixed constructor-time resource limit for a native service.
final class MermanRuntimeConstructorResourceLimit {
  const MermanRuntimeConstructorResourceLimit({
    required this.id,
    required this.phase,
    required this.unit,
    required this.description,
    required this.value,
  });

  final String id;
  final String phase;
  final String unit;
  final String description;
  final int value;
}

/// Runtime contract for one constructor-owned service.
final class MermanRuntimeConstructorServiceContract {
  MermanRuntimeConstructorServiceContract({
    required this.id,
    required List<String> providedTextMeasurementProviderIds,
    required List<MermanRuntimeConstructorResourceLimit> resourceLimits,
  }) : providedTextMeasurementProviderIds = List.unmodifiable(
         providedTextMeasurementProviderIds,
       ),
       resourceLimits = List.unmodifiable(resourceLimits);

  final String id;
  final List<String> providedTextMeasurementProviderIds;
  final List<MermanRuntimeConstructorResourceLimit> resourceLimits;
}

class MermanRuntimeCatalog {
  MermanRuntimeCatalog._({
    required this.rawJson,
    required this.packageVersion,
    required List<int> optionsSchemaVersions,
    required List<MermanRuntimePayloadSchema> payloadSchemas,
    required List<String> metadataIds,
    required List<String> optionGroupIds,
    required List<String> constructorServiceIds,
    required List<MermanRuntimeConstructorServiceContract>
    constructorServiceContracts,
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
  }) : optionsSchemaVersions = List.unmodifiable(optionsSchemaVersions),
       payloadSchemas = List.unmodifiable(payloadSchemas),
       metadataIds = List.unmodifiable(metadataIds),
       optionGroupIds = List.unmodifiable(optionGroupIds),
       constructorServiceIds = List.unmodifiable(constructorServiceIds),
       constructorServiceContracts = List.unmodifiable(
         constructorServiceContracts,
       ),
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
    _requireRequiredKeys(catalog, const {
      'schema_version',
      'transport_api_version',
      'package_version',
      'options_schema_versions',
      'payload_schemas',
      'metadata_ids',
      'capabilities',
      'output_contracts',
      'registry',
      'resources',
    }, 'runtime catalog');
    if (_requiredInt(catalog, 'schema_version') !=
        mermanRuntimeCatalogSchemaVersion) {
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
    final optionsSchemaVersions = _requiredSortedUniquePositiveInts(
      catalog,
      'options_schema_versions',
      'runtime options schema versions',
    );
    final payloadSchemas = _parseRuntimePayloadSchemas(
      catalog['payload_schemas'],
    );
    final metadataIds = _requiredSortedUniqueStrings(
      catalog,
      'metadata_ids',
      'runtime metadata IDs',
    );

    final runtimeCapabilities = _requiredObject(catalog, 'capabilities');
    _requireRequiredKeys(runtimeCapabilities, const {
      'capability_ids',
      'operation_ids',
      'output_ids',
      'system_adapter_ids',
      'text_measurement',
    }, 'runtime capabilities');
    final capabilityIds = _requiredSortedUniqueStrings(
      runtimeCapabilities,
      'capability_ids',
      'runtime capability IDs',
    );
    final capabilitySet = capabilityIds.toSet();
    _validateRuntimeCapabilityRelations(capabilityIds, capabilitySet);

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
    _validateRuntimeOperationRelations(
      operationIds,
      capabilitySet,
      outputIds.toSet(),
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
    _validateRuntimeMetadataRelations(metadataIds, capabilitySet);

    final textMeasurement = runtimeCapabilities['text_measurement'];
    final hasSvg = capabilitySet.contains(
      native.MERMAN_NATIVE_OPERATION_CAPABILITY_SVG,
    );
    if (hasSvg != (textMeasurement is Map)) {
      throw MermanException.contract(
        'text measurement must be present exactly when SVG is available',
      );
    }
    final providers = <String>[];
    if (textMeasurement is Map) {
      final textMeasurementMap = _asObject(textMeasurement, 'text_measurement');
      _requireRequiredKeys(textMeasurementMap, const {
        'protocol_version',
        'provider_ids',
      }, 'runtime text measurement');
      if (_requiredInt(textMeasurementMap, 'protocol_version') !=
          native.MERMAN_TEXT_MEASUREMENT_PROTOCOL_VERSION) {
        throw MermanException.contract(
          'text measurement protocol version does not match the generated native header',
        );
      }
      providers.addAll(
        _requiredSortedUniqueStrings(
          textMeasurementMap,
          'provider_ids',
          'runtime text measurement providers',
        ),
      );
      if (!providers.contains('vendored')) {
        throw MermanException.contract(
          'SVG runtime contract must expose the vendored text measurement provider',
        );
      }
    }

    final optionGroupIds = catalog.containsKey('option_group_ids')
        ? _requiredSortedUniqueFieldIdentifiers(
            catalog,
            'option_group_ids',
            'runtime option group IDs',
          )
        : const <String>[];
    if (catalog.containsKey('option_group_ids')) {
      _validateRuntimeOptionGroups(
        optionGroupIds,
        capabilitySet,
        usesSvgPipeline: textMeasurement is Map,
      );
    }

    final hasConstructorServiceIds = catalog.containsKey(
      'constructor_service_ids',
    );
    final hasConstructorServiceContracts = catalog.containsKey(
      'constructor_service_contracts',
    );
    if (hasConstructorServiceIds != hasConstructorServiceContracts) {
      throw MermanException.contract(
        'runtime constructor service IDs and contracts must be provided together',
      );
    }
    final constructorServiceIds = hasConstructorServiceIds
        ? _requiredSortedUniqueStrings(
            catalog,
            'constructor_service_ids',
            'runtime constructor service IDs',
          )
        : const <String>[];
    final constructorServiceContracts = hasConstructorServiceContracts
        ? _parseRuntimeConstructorServiceContracts(
            catalog['constructor_service_contracts'],
            constructorServiceIds,
            providers,
          )
        : const <MermanRuntimeConstructorServiceContract>[];
    if (hasConstructorServiceIds) {
      _validateRuntimeConstructorServiceIds(
        constructorServiceIds,
        usesSvgPipeline: textMeasurement is Map,
      );
    }

    final registry = _requiredObject(catalog, 'registry');
    _requireRequiredKeys(registry, const {
      'diagram_family_count',
    }, 'runtime registry');
    final diagramFamilyCount = _requiredInt(registry, 'diagram_family_count');
    if (diagramFamilyCount < 0) {
      throw MermanException.contract(
        'runtime diagram_family_count must be non-negative',
      );
    }

    final resources = _requiredObject(catalog, 'resources');
    _requireRequiredKeys(resources, const {
      'general_binding_default_profile',
      'cli_default_profile',
      'limits',
      'profiles',
    }, 'runtime resources');
    final resourceContract = _parseRuntimeResources(resources, operationIds);

    return MermanRuntimeCatalog._(
      rawJson: _encodePreservedJsonObject(catalog, 'runtime catalog'),
      packageVersion: packageVersion,
      optionsSchemaVersions: optionsSchemaVersions,
      payloadSchemas: payloadSchemas,
      metadataIds: metadataIds,
      optionGroupIds: optionGroupIds,
      constructorServiceIds: constructorServiceIds,
      constructorServiceContracts: constructorServiceContracts,
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

  /// Complete schema-1 catalog, including additive fields unknown to this SDK.
  final String rawJson;
  final String packageVersion;
  final List<int> optionsSchemaVersions;
  final List<MermanRuntimePayloadSchema> payloadSchemas;
  final List<String> metadataIds;
  final List<String> optionGroupIds;
  final List<String> constructorServiceIds;
  final List<MermanRuntimeConstructorServiceContract>
  constructorServiceContracts;
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
  final Map<MermanResourceLimitId, MermanResourceLimitDescriptor>
  resourceLimitsById;
  final Map<String, MermanResourceProfileDescriptor> resourceProfilesById;
  Map<String, Object?>? _jsonObjectCache;

  /// Decodes [rawJson] into an immutable JSON object for forward-compatible use.
  Map<String, Object?> get jsonObject =>
      _jsonObjectCache ??= _deeplyUnmodifiableJsonObject(
        _asObject(jsonDecode(rawJson), 'runtime catalog'),
        'runtime catalog',
      );

  bool supportsCapability(String id) => capabilityIds.contains(id);
  bool supportsOutput(String id) => outputIds.contains(id);
  bool supportsOperation(String id) => operationIds.contains(id);
  bool supportsOptionGroup(String id) => optionGroupIds.contains(id);
  bool supportsConstructorService(String id) =>
      constructorServiceIds.contains(id);
  MermanResourceLimitDescriptor? resourceLimitById(String id) =>
      resourceLimitsById[MermanResourceLimitId.fromId(id)];
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
    final transport = mermanBindingTransportExposureSpecs['native-c']!;
    for (final schemaId in transport.payloadSchemaIds) {
      final version = mermanRequiredPayloadSchemaVersions[schemaId];
      if (version == null || !supportsPayloadSchema(schemaId, version)) {
        throw MermanException.contract(
          'runtime catalog does not advertise required `$schemaId` schema '
          '`${version ?? 'unknown'}`',
        );
      }
    }
    final catalog = jsonObject;
    if (!catalog.containsKey('option_group_ids')) {
      throw MermanException.contract(
        'runtime catalog does not advertise option-group IDs',
      );
    }
    final hasConstructorServiceIds = catalog.containsKey(
      'constructor_service_ids',
    );
    final hasConstructorServiceContracts = catalog.containsKey(
      'constructor_service_contracts',
    );
    if (!hasConstructorServiceIds || !hasConstructorServiceContracts) {
      throw MermanException.contract(
        'runtime catalog does not advertise constructor service contracts',
      );
    }
  }

  void requireEngineServices(MermanEngineServices services) {
    if (services.hasIconPacks &&
        !supportsConstructorService(_iconRegistryServiceId)) {
      throw const MermanMissingCapabilityException(
        code: native.MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
        codeName: 'missing-capability',
        message:
            'the loaded artifact does not expose icon-registry construction',
        capabilityId: 'svg',
      );
    }
    if (services.textMeasurer != null &&
        !supportsConstructorService(_hostTextMeasurementServiceId)) {
      throw const MermanMissingCapabilityException(
        code: native.MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
        codeName: 'missing-capability',
        message:
            'the loaded artifact does not expose host text measurement construction',
        capabilityId: 'svg',
      );
    }
  }

  MermanResourceProfileDescriptor get generalBindingDefaultResourceProfile =>
      resourceProfilesById[generalBindingDefaultProfile]!;

  MermanResourceProfileDescriptor get cliDefaultResourceProfile =>
      resourceProfilesById[cliDefaultProfile]!;
}

final class _LoadedNativeLibrary {
  const _LoadedNativeLibrary(this.native, this.runtimeCatalog);

  final _NativeApi native;
  final MermanRuntimeCatalog runtimeCatalog;
}

typedef _MermanGetNativeApiNative =
    native.MermanNativeStatus Function(
      ffi.Pointer<native.MermanNativeApiRequest>,
      ffi.Pointer<native.MermanNativeApi>,
    );
typedef _MermanGetNativeApiDart =
    int Function(
      ffi.Pointer<native.MermanNativeApiRequest>,
      ffi.Pointer<native.MermanNativeApi>,
    );

_LoadedNativeLibrary _loadNativeEntry(
  _MermanGetNativeApiDart getNativeApi, {
  required String? expectedPackageVersion,
}) {
  final nativeApi = _NativeApi.discover(getNativeApi);
  if (expectedPackageVersion != null &&
      nativeApi.packageVersion != expectedPackageVersion) {
    throw MermanException.contract(
      'native package version `${nativeApi.packageVersion}` does not match '
      'the required `$expectedPackageVersion`',
    );
  }
  final catalog = nativeApi.loadRuntimeCatalog();
  catalog.requireCurrentBindingSchemas();
  return _LoadedNativeLibrary(nativeApi, catalog);
}

_LoadedNativeLibrary _loadDynamicLibrary(
  ffi.DynamicLibrary library, {
  required String? expectedPackageVersion,
}) {
  return _loadNativeEntry(
    library.lookupFunction<_MermanGetNativeApiNative, _MermanGetNativeApiDart>(
      'merman_get_native_api',
    ),
    expectedPackageVersion: expectedPackageVersion,
  );
}

final _LoadedNativeLibrary _packageNativeLibrary = _loadNativeEntry(
  native.merman_get_native_api,
  expectedPackageVersion: mermanPackageVersion,
);

String? _oneShotRequestOptionsJson(String? optionsJson) {
  if (optionsJson == null || optionsJson.trim().isEmpty) {
    return null;
  }
  final options = _asObject(jsonDecode(optionsJson), 'options_json');
  options.remove('runtime_policy');
  for (final wrapperName in const ['analysis', 'merman']) {
    final wrapper = options[wrapperName];
    if (wrapper is Map) {
      final normalized = _asObject(wrapper, 'options_json.$wrapperName');
      normalized.remove('runtime_policy');
      options[wrapperName] = normalized;
    }
  }
  return jsonEncode(options);
}

/// Discovery and one-shot facade for Flutter and standalone Dart hosts.
///
/// This object owns no native engine token. Every execution uses a fresh
/// deterministic engine and closes it before returning. Use [MermanEngine]
/// when options or constructor services should be reused across calls.
class Merman {
  Merman._(this._native, this.runtimeCatalog);

  factory Merman.fromDynamicLibrary(
    ffi.DynamicLibrary library, {
    String? expectedPackageVersion,
  }) {
    final loaded = _loadDynamicLibrary(
      library,
      expectedPackageVersion: expectedPackageVersion,
    );
    return Merman._(loaded.native, loaded.runtimeCatalog);
  }

  /// Opens the package-owned library and requires its exact package contract.
  factory Merman.open() {
    final loaded = _packageNativeLibrary;
    return Merman._(loaded.native, loaded.runtimeCatalog);
  }

  /// Opens a host-owned library that implements the current ABI contract.
  factory Merman.openPath(String path) =>
      Merman.fromDynamicLibrary(openMermanLibraryFromPath(path));

  final _NativeApi _native;
  final MermanRuntimeCatalog runtimeCatalog;
  List<String>? _supportedDiagramsCache;
  List<MermanAsciiCapability>? _asciiCapabilitiesCache;
  List<MermanDiagramFamilyCapability>? _diagramFamilyCapabilitiesCache;
  List<MermanLintRuleCatalogEntry>? _lintRuleCatalogCache;
  List<String>? _supportedThemesCache;
  MermanPresentationCatalog? _presentationCatalogCache;

  /// Native package version reported by the discovered table.
  String get packageVersion => _native.packageVersion;

  /// Creates a control with an optional relative monotonic timeout.
  MermanOperationControl createOperationControl({Duration? timeout}) =>
      _native.createOperationControl(timeout: timeout);

  MermanOperationResult execute(
    MermanOperation operation,
    String source, {
    String? uri,
    String? optionsJson,
    MermanOperationControl? control,
  }) {
    final engine = _native.createEngine(
      runtimeCatalog: runtimeCatalog,
      optionsJson: optionsJson,
      services: const MermanEngineServices(),
    );
    try {
      return engine.execute(
        operation,
        source,
        uri: uri,
        optionsJson: _oneShotRequestOptionsJson(optionsJson),
        control: control,
      );
    } finally {
      engine.close();
    }
  }

  String renderSvg(String source, {String? optionsJson}) =>
      execute(MermanOperation.svg, source, optionsJson: optionsJson).utf8Text;

  Uint8List renderPng(String source, {String? optionsJson}) =>
      renderPngResult(source, optionsJson: optionsJson).bytes;

  MermanOperationResult renderPngResult(String source, {String? optionsJson}) =>
      execute(MermanOperation.png, source, optionsJson: optionsJson);

  Uint8List renderJpeg(String source, {String? optionsJson}) =>
      renderJpegResult(source, optionsJson: optionsJson).bytes;

  MermanOperationResult renderJpegResult(
    String source, {
    String? optionsJson,
  }) => execute(MermanOperation.jpeg, source, optionsJson: optionsJson);

  Uint8List renderPdf(String source, {String? optionsJson}) =>
      renderPdfResult(source, optionsJson: optionsJson).bytes;

  MermanOperationResult renderPdfResult(String source, {String? optionsJson}) =>
      execute(MermanOperation.pdf, source, optionsJson: optionsJson);

  String renderAscii(String source, {String? optionsJson}) =>
      execute(MermanOperation.ascii, source, optionsJson: optionsJson).utf8Text;

  Map<String, Object?> parseJson(String source, {String? optionsJson}) =>
      _json(MermanOperation.semanticJson, source, optionsJson: optionsJson);

  Map<String, Object?> layoutJson(String source, {String? optionsJson}) =>
      _json(MermanOperation.layoutJson, source, optionsJson: optionsJson);

  Map<String, Object?> analyzeJson(String source, {String? optionsJson}) =>
      _json(MermanOperation.analysisJson, source, optionsJson: optionsJson);

  Map<String, Object?> analysisFactsJson(
    String source, {
    String? optionsJson,
  }) => _json(
    MermanOperation.analysisFactsJson,
    source,
    optionsJson: optionsJson,
  );

  Map<String, Object?> svgPlanJson(String source, {String? optionsJson}) =>
      _json(MermanOperation.svgPlanJson, source, optionsJson: optionsJson);

  Map<String, Object?> analyzeDocumentJson(
    String source, {
    required String uri,
    String? optionsJson,
  }) => _json(
    MermanOperation.documentAnalysisJson,
    source,
    uri: uri,
    optionsJson: optionsJson,
  );

  Map<String, Object?> analyzeDocumentFactsJson(
    String source, {
    required String uri,
    String? optionsJson,
  }) => _json(
    MermanOperation.documentAnalysisFactsJson,
    source,
    uri: uri,
    optionsJson: optionsJson,
  );

  MermanValidationResult validate(String source, {String? optionsJson}) =>
      MermanValidationResult._(
        _json(MermanOperation.validationJson, source, optionsJson: optionsJson),
      );

  /// Returns the exact JSON payload for any callable metadata ID.
  String metadataJson(String id) => utf8.decode(_native.collectMetadata(id));

  /// Returns diagram types exposed by the loaded native artifact.
  List<String> supportedDiagrams() {
    return _supportedDiagramsCache ??= List.unmodifiable(
      _decodeJsonStringList(
        _native.collectMetadata(MermanBindingMetadataId.supportedDiagrams),
        'supported diagrams',
      ),
    );
  }

  /// Returns ASCII rendering capability records for the loaded artifact.
  List<MermanAsciiCapability> asciiCapabilities() {
    return _asciiCapabilitiesCache ??= List.unmodifiable(
      _decodeJsonObjectList(
        _native.collectMetadata(MermanBindingMetadataId.asciiCapabilities),
        'ASCII capabilities',
        MermanAsciiCapability.fromJson,
      ),
    );
  }

  /// Returns parser/render capability records for the loaded artifact.
  List<MermanDiagramFamilyCapability> diagramFamilyCapabilities() {
    return _diagramFamilyCapabilitiesCache ??= List.unmodifiable(
      _decodeJsonObjectList(
        _native.collectMetadata(
          MermanBindingMetadataId.diagramFamilyCapabilities,
        ),
        'diagram family capabilities',
        MermanDiagramFamilyCapability.fromJson,
      ),
    );
  }

  /// Returns governed lint rule metadata for the loaded artifact.
  List<MermanLintRuleCatalogEntry> lintRuleCatalog() {
    return _lintRuleCatalogCache ??= List.unmodifiable(
      _decodeJsonObjectListFromField(
        _native.collectMetadata(MermanBindingMetadataId.lintRuleCatalog),
        'lint rule catalog',
        'rules',
        MermanLintRuleCatalogEntry.fromJson,
      ),
    );
  }

  /// Returns built-in Mermaid theme names.
  List<String> supportedThemes() {
    return _supportedThemesCache ??= List.unmodifiable(
      _decodeJsonStringList(
        _native.collectMetadata(MermanBindingMetadataId.supportedThemes),
        'supported themes',
      ),
    );
  }

  /// Returns artifact-owned presentation theme and profile metadata.
  MermanPresentationCatalog presentationCatalog() {
    return _presentationCatalogCache ??= MermanPresentationCatalog.fromJson(
      _decodeJsonObject(
        _native.collectMetadata(MermanBindingMetadataId.presentationCatalog),
        'presentation catalog',
      ),
    );
  }

  Map<String, Object?> _json(
    MermanOperation operation,
    String source, {
    String? uri,
    String? optionsJson,
  }) =>
      execute(operation, source, uri: uri, optionsJson: optionsJson).jsonObject;
}

enum _MermanEngineState { open, closing, closed }

/// A reusable native engine with immutable constructor-owned services.
class MermanEngine {
  MermanEngine._(
    this._native,
    this.runtimeCatalog,
    this._token,
    this._textMeasurement,
  );

  /// Opens the package-owned library and constructs one reusable engine.
  factory MermanEngine({
    String? optionsJson,
    MermanEngineServices services = const MermanEngineServices(),
  }) {
    final loaded = _packageNativeLibrary;
    return loaded.native.createEngine(
      runtimeCatalog: loaded.runtimeCatalog,
      optionsJson: optionsJson,
      services: services,
    );
  }

  /// Constructs a reusable engine from an already opened native library.
  factory MermanEngine.fromDynamicLibrary(
    ffi.DynamicLibrary library, {
    String? optionsJson,
    MermanEngineServices services = const MermanEngineServices(),
    String? expectedPackageVersion,
  }) {
    final loaded = _loadDynamicLibrary(
      library,
      expectedPackageVersion: expectedPackageVersion,
    );
    return loaded.native.createEngine(
      runtimeCatalog: loaded.runtimeCatalog,
      optionsJson: optionsJson,
      services: services,
    );
  }

  /// Opens a host-owned current-contract library and constructs a reusable engine.
  factory MermanEngine.openPath(
    String path, {
    String? optionsJson,
    MermanEngineServices services = const MermanEngineServices(),
  }) => MermanEngine.fromDynamicLibrary(
    openMermanLibraryFromPath(path),
    optionsJson: optionsJson,
    services: services,
  );

  final _NativeApi _native;
  final MermanRuntimeCatalog runtimeCatalog;
  int? _token;
  final _TextMeasurementRegistration? _textMeasurement;
  bool _activeCall = false;
  _MermanEngineState _state = _MermanEngineState.open;

  String get packageVersion => _native.packageVersion;
  bool get isClosed => _state == _MermanEngineState.closed;

  /// Creates a control that may be attached to this engine's operations.
  MermanOperationControl createOperationControl({Duration? timeout}) =>
      _native.createOperationControl(timeout: timeout);

  MermanOperationResult execute(
    MermanOperation operation,
    String source, {
    String? uri,
    String? optionsJson,
    MermanOperationControl? control,
  }) {
    final token = _requireToken();
    final requestUri = uri == null || uri.isEmpty ? null : uri;
    if (operation.requiresUri != (requestUri != null)) {
      throw MermanException.contract(
        'operation `${operation.operationId}` ${operation.requiresUri ? 'requires' : 'does not accept'} a URI',
      );
    }
    return _withNativeCall(
      () => _native.execute(
        token,
        operation,
        source,
        uri: requestUri,
        optionsJson: optionsJson,
        operationControl: control?._borrowFor(_native) ?? 0,
      ),
    );
  }

  String renderSvg(String source, {String? optionsJson}) =>
      execute(MermanOperation.svg, source, optionsJson: optionsJson).utf8Text;

  Uint8List renderPng(String source, {String? optionsJson}) =>
      renderPngResult(source, optionsJson: optionsJson).bytes;

  MermanOperationResult renderPngResult(String source, {String? optionsJson}) =>
      execute(MermanOperation.png, source, optionsJson: optionsJson);

  Uint8List renderJpeg(String source, {String? optionsJson}) =>
      renderJpegResult(source, optionsJson: optionsJson).bytes;

  MermanOperationResult renderJpegResult(
    String source, {
    String? optionsJson,
  }) => execute(MermanOperation.jpeg, source, optionsJson: optionsJson);

  Uint8List renderPdf(String source, {String? optionsJson}) =>
      renderPdfResult(source, optionsJson: optionsJson).bytes;

  MermanOperationResult renderPdfResult(String source, {String? optionsJson}) =>
      execute(MermanOperation.pdf, source, optionsJson: optionsJson);

  String renderAscii(String source, {String? optionsJson}) =>
      execute(MermanOperation.ascii, source, optionsJson: optionsJson).utf8Text;

  Map<String, Object?> parseJson(String source, {String? optionsJson}) =>
      _json(MermanOperation.semanticJson, source, optionsJson: optionsJson);

  Map<String, Object?> layoutJson(String source, {String? optionsJson}) =>
      _json(MermanOperation.layoutJson, source, optionsJson: optionsJson);

  Map<String, Object?> analyzeJson(String source, {String? optionsJson}) =>
      _json(MermanOperation.analysisJson, source, optionsJson: optionsJson);

  Map<String, Object?> analysisFactsJson(
    String source, {
    String? optionsJson,
  }) => _json(
    MermanOperation.analysisFactsJson,
    source,
    optionsJson: optionsJson,
  );

  Map<String, Object?> svgPlanJson(String source, {String? optionsJson}) =>
      _json(MermanOperation.svgPlanJson, source, optionsJson: optionsJson);

  Map<String, Object?> analyzeDocumentJson(
    String source, {
    required String uri,
    String? optionsJson,
  }) => _json(
    MermanOperation.documentAnalysisJson,
    source,
    uri: uri,
    optionsJson: optionsJson,
  );

  Map<String, Object?> analyzeDocumentFactsJson(
    String source, {
    required String uri,
    String? optionsJson,
  }) => _json(
    MermanOperation.documentAnalysisFactsJson,
    source,
    uri: uri,
    optionsJson: optionsJson,
  );

  MermanValidationResult validate(String source, {String? optionsJson}) =>
      MermanValidationResult._(
        _json(MermanOperation.validationJson, source, optionsJson: optionsJson),
      );

  /// Tries to close this engine without waiting. Safe to call more than once.
  ///
  /// A BUSY or REENTRANT failure retains the native token and the immutable
  /// callback registration so [close] can be retried.
  void close() {
    if (_state == _MermanEngineState.closed ||
        _state == _MermanEngineState.closing) {
      return;
    }
    final token = _token;
    if (token == null) {
      _state = _MermanEngineState.closed;
      return;
    }
    if (_activeCall) {
      throw const MermanReentrantCallException(
        code: native.MERMAN_NATIVE_STATUS_REENTRANT_CALL,
        codeName: 'reentrant-call',
        message: 'Merman engine cannot be closed from a native callback',
      );
    }
    _state = _MermanEngineState.closing;
    try {
      _native.tryCloseEngine(token);
    } catch (_) {
      _state = _MermanEngineState.open;
      rethrow;
    }
    _token = null;
    _state = _MermanEngineState.closed;
    _textMeasurement?.dispose();
  }

  Map<String, Object?> _json(
    MermanOperation operation,
    String source, {
    String? uri,
    String? optionsJson,
  }) =>
      execute(operation, source, uri: uri, optionsJson: optionsJson).jsonObject;

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
    if (_state != _MermanEngineState.open) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_ENGINE_CLOSED',
        message: 'Merman engine is closed',
      );
    }
    final token = _token;
    if (token == null) {
      throw const MermanException(
        code: -1,
        codeName: 'DART_ENGINE_CLOSED',
        message: 'Merman engine is closed',
      );
    }
    return token;
  }
}

class _NativeApi {
  _NativeApi._({
    required this.packageVersion,
    required native.DartMermanNativeRuntimeCatalogFnFunction runtimeCatalog,
    required _NativeEngineCloser engineCloser,
    required native.DartMermanNativeExecuteCollectFnFunction executeCollect,
    required native.DartMermanNativeResultFreeFnFunction resultFree,
    required native.DartMermanNativeMetadataCollectFnFunction metadataCollect,
    required native.DartMermanNativeEngineNewWithServicesFnFunction
    engineNewWithServices,
    required native.DartMermanNativeOperationControlNewFnFunction
    operationControlNew,
    required native.DartMermanNativeOperationControlCancelFnFunction
    operationControlCancel,
    required native.DartMermanNativeOperationControlReleaseFnFunction
    operationControlRelease,
  }) : _runtimeCatalog = runtimeCatalog,
       _engineCloser = engineCloser,
       _executeCollect = executeCollect,
       _resultFree = resultFree,
       _metadataCollect = metadataCollect,
       _engineNewWithServices = engineNewWithServices,
       _operationControlNew = operationControlNew,
       _operationControlCancel = operationControlCancel,
       _operationControlRelease = operationControlRelease;

  final String packageVersion;
  final native.DartMermanNativeRuntimeCatalogFnFunction _runtimeCatalog;
  final _NativeEngineCloser _engineCloser;
  final native.DartMermanNativeExecuteCollectFnFunction _executeCollect;
  final native.DartMermanNativeResultFreeFnFunction _resultFree;
  final native.DartMermanNativeMetadataCollectFnFunction _metadataCollect;
  final native.DartMermanNativeEngineNewWithServicesFnFunction
  _engineNewWithServices;
  final native.DartMermanNativeOperationControlNewFnFunction
  _operationControlNew;
  final native.DartMermanNativeOperationControlCancelFnFunction
  _operationControlCancel;
  final native.DartMermanNativeOperationControlReleaseFnFunction
  _operationControlRelease;

  factory _NativeApi.discover(_MermanGetNativeApiDart getNativeApi) {
    final request = calloc<native.MermanNativeApiRequest>();
    final api = calloc<native.MermanNativeApi>();
    final allocations = _NativeAllocationScope();
    try {
      _writeSlice(
        request.ref.expected_minimum_prefix_layout_digest,
        utf8.encode(native.MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST),
        allocations,
      );
      request.ref.struct_size = ffi.sizeOf<native.MermanNativeApiRequest>();
      request.ref.expected_abi_version = native.MERMAN_NATIVE_ABI_VERSION;
      final consumerTableSize = ffi.sizeOf<native.MermanNativeApi>();
      if (consumerTableSize <
          native.MERMAN_NATIVE_API_OPERATION_CONTROL_RELEASE_PREFIX_SIZE) {
        throw MermanException.contract(
          'generated native API table is smaller than the current ABI 3 table',
        );
      }
      api.ref.struct_size = consumerTableSize;

      final status = getNativeApi(request, api);
      if (status != native.MERMAN_NATIVE_STATUS_OK) {
        throw MermanException(
          code: status,
          codeName: 'DART_ABI_DISCOVERY_FAILED',
          message: 'merman_get_native_api rejected the ABI 3 request',
        );
      }

      final table = api.ref;
      if (!_nativeApiHasCurrentTable(table.struct_size)) {
        throw MermanException.contract(
          'native API table is smaller than the current ABI 3 table',
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
      _requireFunctionPointer(table.metadata_collect, 'metadata_collect');
      _requireFunctionPointer(
        table.engine_new_with_services,
        'engine_new_with_services',
      );
      _requireFunctionPointer(
        table.operation_control_new,
        'operation_control_new',
      );
      _requireFunctionPointer(
        table.operation_control_cancel,
        'operation_control_cancel',
      );
      _requireFunctionPointer(
        table.operation_control_release,
        'operation_control_release',
      );

      return _NativeApi._(
        packageVersion: packageVersion,
        runtimeCatalog: table.runtime_catalog
            .asFunction<native.DartMermanNativeRuntimeCatalogFnFunction>(),
        engineCloser: _NativeEngineCloser(
          identity: table.engine_try_close.address,
          close: table.engine_try_close
              .asFunction<native.DartMermanNativeEngineTryCloseFnFunction>(),
        ),
        executeCollect: table.execute_collect
            .asFunction<native.DartMermanNativeExecuteCollectFnFunction>(),
        resultFree: table.result_free
            .asFunction<native.DartMermanNativeResultFreeFnFunction>(),
        metadataCollect: table.metadata_collect
            .asFunction<native.DartMermanNativeMetadataCollectFnFunction>(),
        engineNewWithServices: table.engine_new_with_services
            .asFunction<
              native.DartMermanNativeEngineNewWithServicesFnFunction
            >(),
        operationControlNew: table.operation_control_new
            .asFunction<
              native.DartMermanNativeOperationControlNewFnFunction
            >(),
        operationControlCancel: table.operation_control_cancel
            .asFunction<
              native.DartMermanNativeOperationControlCancelFnFunction
            >(),
        operationControlRelease: table.operation_control_release
            .asFunction<
              native.DartMermanNativeOperationControlReleaseFnFunction
            >(),
      );
    } finally {
      allocations.dispose();
      calloc.free(request);
      calloc.free(api);
    }
  }

  Uint8List collectMetadata(String metadataId) {
    final id = calloc<native.MermanNativeSlice>();
    final allocations = _NativeAllocationScope();
    final result = _NativeResult.allocate(_resultFree);
    try {
      _writeSlice(id.ref, utf8.encode(metadataId), allocations);
      final status = _metadataCollect(id.ref, result.pointer);
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

  MermanEngine createEngine({
    required MermanRuntimeCatalog runtimeCatalog,
    required String? optionsJson,
    required MermanEngineServices services,
  }) {
    final quarantineBlocker = _unpublishedEngineQuarantine.sweepAndBlockerFor(
      _engineCloser.identity,
    );
    if (quarantineBlocker != null) {
      throw quarantineBlocker.toException();
    }
    runtimeCatalog.requireEngineServices(services);
    final iconPacks =
        services.iconPackSet?._encodedPacks ?? const <_EncodedMermanIconPack>[];
    ffi.Pointer<native.MermanNativeEngineServicesConfig>? servicesConfig;
    ffi.Pointer<native.MermanNativeEngineToken>? token;
    _NativeResult? result;
    _NativeAllocationScope? allocations;
    var nativeIconPacks = ffi.nullptr.cast<native.MermanNativeIconPack>();
    _TextMeasurementRegistration? registration;
    var unownedToken = 0;
    try {
      servicesConfig = calloc<native.MermanNativeEngineServicesConfig>();
      token = calloc<native.MermanNativeEngineToken>();
      result = _NativeResult.allocate(_resultFree);
      allocations = _NativeAllocationScope();
      if (iconPacks.isNotEmpty) {
        nativeIconPacks = calloc<native.MermanNativeIconPack>(iconPacks.length);
      }
      registration = services.textMeasurer == null
          ? null
          : _TextMeasurementRegistration.create(services.textMeasurer!);
      servicesConfig.ref.struct_size = ffi
          .sizeOf<native.MermanNativeEngineServicesConfig>();
      _initializeEngineConfig(
        servicesConfig.ref.engine_config,
        optionsJson,
        registration,
        allocations,
      );
      for (var index = 0; index < iconPacks.length; index += 1) {
        final pack = (nativeIconPacks + index).ref;
        pack.struct_size = ffi.sizeOf<native.MermanNativeIconPack>();
        _writeSlice(pack.json, iconPacks[index].json, allocations);
        _writeSlice(
          pack.registration_name,
          iconPacks[index].registrationName,
          allocations,
        );
      }
      servicesConfig.ref.icon_packs = nativeIconPacks;
      servicesConfig.ref.icon_pack_count = iconPacks.length;
      final status = _engineNewWithServices(
        servicesConfig,
        token,
        result.pointer,
      );
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
      final engine = MermanEngine._(
        this,
        runtimeCatalog,
        token.value,
        registration,
      );
      unownedToken = 0;
      registration = null;
      return engine;
    } catch (_) {
      if (unownedToken != 0) {
        final closeStatus = _engineCloser.close(unownedToken);
        if (closeStatus != native.MERMAN_NATIVE_STATUS_OK) {
          _unpublishedEngineQuarantine.retain(
            closer: _engineCloser,
            token: unownedToken,
            releaseCallbackState: registration?.dispose,
            closeStatus: closeStatus,
          );
          registration = null;
          rethrow;
        }
      }
      registration?.dispose();
      rethrow;
    } finally {
      allocations?.dispose();
      result?.dispose();
      if (servicesConfig != null) calloc.free(servicesConfig);
      if (nativeIconPacks.address != 0) {
        calloc.free(nativeIconPacks);
      }
      if (token != null) calloc.free(token);
    }
  }

  void tryCloseEngine(int token) {
    final status = _engineCloser.close(token);
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

  MermanOperationControl createOperationControl({Duration? timeout}) {
    final timeoutMs = timeout?.inMilliseconds ?? 0;
    if (timeoutMs < 0) {
      throw RangeError.value(timeout, 'timeout', 'must not be negative');
    }
    final control = calloc<native.MermanNativeOperationControlToken>();
    final result = _NativeResult.allocate(_resultFree);
    var unownedControl = 0;
    try {
      final status = _operationControlNew(
        timeoutMs,
        timeout == null ? 0 : 1,
        control,
        result.pointer,
      );
      unownedControl = control.value;
      result.requireWritten(status);
      final record = result.pointer.ref;
      final metadata = _copyBuffer(record.metadata_or_error_json);
      _ensureResultStatus(status, record.status, metadata);
      if (record.operation != native.MERMAN_NATIVE_OPERATION_NONE ||
          record.data.len != 0 ||
          unownedControl == 0) {
        throw MermanException.contract(
          'native operation-control creation returned an invalid result',
        );
      }
      final operationControl = MermanOperationControl._(this, unownedControl);
      unownedControl = 0;
      return operationControl;
    } catch (_) {
      if (unownedControl != 0) {
        _operationControlRelease(unownedControl);
      }
      rethrow;
    } finally {
      result.dispose();
      calloc.free(control);
    }
  }

  void cancelOperationControl(int control) {
    final status = _operationControlCancel(control);
    if (status != native.MERMAN_NATIVE_STATUS_OK) {
      throw MermanException.fromNative(status, Uint8List(0));
    }
  }

  void releaseOperationControl(int control) {
    final status = _operationControlRelease(control);
    if (status != native.MERMAN_NATIVE_STATUS_OK) {
      throw MermanException.fromNative(status, Uint8List(0));
    }
  }

  MermanOperationResult execute(
    int engine,
    MermanOperation operation,
    String source, {
    String? uri,
    String? optionsJson,
    int operationControl = 0,
  }) {
    final request = _newRequest(
      operation,
      source,
      uri: uri,
      optionsJson: optionsJson,
      operationControl: operationControl,
    );
    final result = _NativeResult.allocate(_resultFree);
    try {
      final status = _executeCollect(engine, request.pointer, result.pointer);
      result.requireWritten(status);
      final record = result.pointer.ref;
      final metadata = _copyBuffer(record.metadata_or_error_json);
      _ensureResultStatus(status, record.status, metadata);
      if (record.operation != operation.nativeCode) {
        throw MermanException.contract(
          'native operation does not match the requested `${operation.operationId}`',
        );
      }
      final mediaType = _utf8FromSlice(record.media_type, 'result media type');
      final bytes = _copyBuffer(record.data);
      final operationMetadata = _decodeOperationMetadata(metadata);
      final expectation = _operationExpectationById[operation.operationId]!;
      if (mediaType != expectation.mediaType ||
          operationMetadata.version != expectation.metadataSchemaVersion) {
        throw MermanException.contract(
          'native result contract does not match generated operation '
          '`${operation.operationId}`',
        );
      }
      if (operationMetadata.operationId != operation.operationId) {
        throw MermanException.contract(
          'operation metadata `${operationMetadata.operationId}` does not '
          'match the requested `${operation.operationId}`',
        );
      }
      if (operationMetadata.mediaType != mediaType) {
        throw MermanException.contract(
          'operation metadata media type `${operationMetadata.mediaType}` '
          'does not match the native result `$mediaType`',
        );
      }
      if (operationMetadata.byteLength != bytes.length) {
        throw MermanException.contract(
          'operation metadata byte length `${operationMetadata.byteLength}` '
          'does not match the native result `${bytes.length}`',
        );
      }
      return MermanOperationResult(
        operation: operation,
        mediaType: mediaType,
        bytes: bytes,
        metadata: operationMetadata,
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
    required int operationControl,
  }) {
    final request = calloc<native.MermanNativeOperationRequest>();
    final allocations = _NativeAllocationScope();
    request.ref.struct_size = ffi.sizeOf<native.MermanNativeOperationRequest>();
    request.ref.operation = operation.nativeCode;
    request.ref.operation_control = operationControl;
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

/// Reports whether a producer-written ABI table includes the complete current
/// table without reading beyond that prefix.
bool nativeApiHasCurrentTableForTesting(int producerTableSize) =>
    _nativeApiHasCurrentTable(producerTableSize);

bool _nativeApiHasCurrentTable(int producerTableSize) =>
    producerTableSize >=
    native.MERMAN_NATIVE_API_OPERATION_CONTROL_RELEASE_PREFIX_SIZE;

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
    _bytes.add(pointer);
    pointer.asTypedList(bytes.length).setAll(0, bytes);
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
  _TextMeasurementRegistration._(this._key, this._callback);

  static final Map<int, MermanTextMeasurer> _measurers = {};

  final ffi.Pointer<ffi.Uint8> _key;
  final ffi.NativeCallable<native.MermanNativeTextMeasureCallbackFunction>
  _callback;
  bool _disposed = false;

  ffi.Pointer<ffi.Void> get userData => _key.cast<ffi.Void>();

  ffi.Pointer<
    ffi.NativeFunction<native.MermanNativeTextMeasureCallbackFunction>
  >
  get nativeFunction => _callback.nativeFunction;

  static _TextMeasurementRegistration create(MermanTextMeasurer measurer) {
    final key = calloc<ffi.Uint8>();
    _measurers[key.address] = measurer;
    try {
      final callback =
          ffi.NativeCallable<
            native.MermanNativeTextMeasureCallbackFunction
          >.isolateLocal(
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

typedef _EngineTryClose = int Function(int token);

final class _NativeEngineCloser {
  const _NativeEngineCloser({required this.identity, required this.close});

  final int identity;
  final _EngineTryClose close;
}

enum _QuarantinedEngineState { retryable, poisoned }

final class _QuarantinedUnpublishedEngine {
  _QuarantinedUnpublishedEngine({
    required this.closer,
    required this.token,
    required void Function()? releaseCallbackState,
    required int closeStatus,
  }) : _releaseCallbackState = releaseCallbackState,
       lastStatus = closeStatus,
       state = _isRetryableEngineCloseStatus(closeStatus)
           ? _QuarantinedEngineState.retryable
           : _QuarantinedEngineState.poisoned;

  final _NativeEngineCloser closer;
  final int token;
  void Function()? _releaseCallbackState;
  int lastStatus;
  _QuarantinedEngineState state;

  int retryClose() {
    if (state == _QuarantinedEngineState.poisoned) {
      return lastStatus;
    }
    final status = closer.close(token);
    lastStatus = status;
    if (status != native.MERMAN_NATIVE_STATUS_OK &&
        !_isRetryableEngineCloseStatus(status)) {
      state = _QuarantinedEngineState.poisoned;
    }
    return status;
  }

  void releaseCallbackState() {
    final release = _releaseCallbackState;
    _releaseCallbackState = null;
    release?.call();
  }
}

bool _isRetryableEngineCloseStatus(int status) =>
    status == native.MERMAN_NATIVE_STATUS_BUSY ||
    status == native.MERMAN_NATIVE_STATUS_REENTRANT_CALL;

final class _UnpublishedEngineBlocker {
  const _UnpublishedEngineBlocker({
    required this.status,
    required this.poisoned,
  });

  final int status;
  final bool poisoned;

  MermanException toException() => MermanException(
    code: status,
    codeName: poisoned
        ? 'DART_UNPUBLISHED_ENGINE_QUARANTINE_POISONED'
        : 'DART_UNPUBLISHED_ENGINE_QUARANTINED',
    message: poisoned
        ? 'native engine construction is disabled for this native '
              'producer because a previously published token could not '
              'establish callback quiescence; callback state remains retained'
        : 'native engine construction is disabled for this native '
              'producer because a previously published token has not yet '
              'been rolled back',
  );
}

/// Retains callback state when a non-conforming producer publishes an engine
/// token that cannot be rolled back. Every entry owns the exact close function
/// from the table that produced its token, so equal token values from another
/// loaded library can never authorize its retirement.
final class _UnpublishedEngineQuarantine {
  final Map<int, List<_QuarantinedUnpublishedEngine>> _enginesByProducer = {};

  void retain({
    required _NativeEngineCloser closer,
    required int token,
    required void Function()? releaseCallbackState,
    required int closeStatus,
  }) {
    if (closeStatus == native.MERMAN_NATIVE_STATUS_OK) {
      throw ArgumentError.value(
        closeStatus,
        'closeStatus',
        'a successfully closed engine must not enter quarantine',
      );
    }
    _enginesByProducer
        .putIfAbsent(closer.identity, () => [])
        .add(
          _QuarantinedUnpublishedEngine(
            closer: closer,
            token: token,
            releaseCallbackState: releaseCallbackState,
            closeStatus: closeStatus,
          ),
        );
  }

  _UnpublishedEngineBlocker? sweepAndBlockerFor(int producerIdentity) {
    _sweep();
    final engines = _enginesByProducer[producerIdentity];
    if (engines == null || engines.isEmpty) {
      return null;
    }
    var blocker = engines.first;
    for (final engine in engines) {
      if (engine.state == _QuarantinedEngineState.poisoned) {
        blocker = engine;
        break;
      }
    }
    return _UnpublishedEngineBlocker(
      status: blocker.lastStatus,
      poisoned: blocker.state == _QuarantinedEngineState.poisoned,
    );
  }

  void _sweep() {
    for (final producerIdentity in _enginesByProducer.keys.toList(
      growable: false,
    )) {
      final engines = _enginesByProducer[producerIdentity]!;
      var index = 0;
      while (index < engines.length) {
        final engine = engines[index];
        if (engine.retryClose() != native.MERMAN_NATIVE_STATUS_OK) {
          index += 1;
          continue;
        }
        engines.removeAt(index);
        engine.releaseCallbackState();
      }
      if (engines.isEmpty) {
        _enginesByProducer.remove(producerIdentity);
      }
    }
  }

  int entryCountFor(int producerIdentity) =>
      _enginesByProducer[producerIdentity]?.length ?? 0;

  int get totalEntryCount => _enginesByProducer.values.fold(
    0,
    (total, engines) => total + engines.length,
  );
}

final _unpublishedEngineQuarantine = _UnpublishedEngineQuarantine();

/// Package-internal snapshot used by the deterministic Dart contract tests.
final class UnpublishedEngineQuarantineStateForTesting {
  const UnpublishedEngineQuarantineStateForTesting({
    required this.blocked,
    required this.status,
    required this.poisoned,
    required this.producerEntryCount,
    required this.totalEntryCount,
  });

  final bool blocked;
  final int? status;
  final bool poisoned;
  final int producerEntryCount;
  final int totalEntryCount;
}

/// Package-internal harness that exercises quarantine ownership without a
/// malformed native library fixture.
final class UnpublishedEngineQuarantineTestHarness {
  final _UnpublishedEngineQuarantine _quarantine =
      _UnpublishedEngineQuarantine();

  void retain({
    required int producerIdentity,
    required int token,
    required int closeStatus,
    required int Function(int token) close,
    void Function()? releaseCallbackState,
  }) {
    _quarantine.retain(
      closer: _NativeEngineCloser(identity: producerIdentity, close: close),
      token: token,
      releaseCallbackState: releaseCallbackState,
      closeStatus: closeStatus,
    );
  }

  UnpublishedEngineQuarantineStateForTesting sweepFor(int producerIdentity) {
    final blocker = _quarantine.sweepAndBlockerFor(producerIdentity);
    return UnpublishedEngineQuarantineStateForTesting(
      blocked: blocker != null,
      status: blocker?.status,
      poisoned: blocker?.poisoned ?? false,
      producerEntryCount: _quarantine.entryCountFor(producerIdentity),
      totalEntryCount: _quarantine.totalEntryCount,
    );
  }
}

void _initializeEngineConfig(
  native.MermanNativeEngineConfig config,
  String? optionsJson,
  _TextMeasurementRegistration? registration,
  _NativeAllocationScope allocations,
) {
  config.struct_size = ffi.sizeOf<native.MermanNativeEngineConfig>();
  _writeSlice(
    config.options_json,
    optionsJson == null ? const <int>[] : utf8.encode(optionsJson),
    allocations,
  );
  config.text_measure =
      registration?.nativeFunction ??
      ffi.nullptr
          .cast<
            ffi.NativeFunction<native.MermanNativeTextMeasureCallbackFunction>
          >();
  config.text_measure_user_data =
      registration?.userData ?? ffi.nullptr.cast<ffi.Void>();
}

void _writeSlice(
  native.MermanNativeSlice slice,
  List<int> bytes,
  _NativeAllocationScope allocations,
) {
  final owned = bytes is Uint8List ? bytes : Uint8List.fromList(bytes);
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
  if (!_isRuntimeIdentifier(generalBindingDefaultProfile)) {
    throw MermanException.contract(
      'runtime resources.general_binding_default_profile must be a stable identifier',
    );
  }
  final cliDefaultProfile = _requiredNonEmptyString(
    resources,
    'cli_default_profile',
    'runtime resources',
  );
  if (!_isRuntimeIdentifier(cliDefaultProfile)) {
    throw MermanException.contract(
      'runtime resources.cli_default_profile must be a stable identifier',
    );
  }

  final rawLimits = resources['limits'];
  if (rawLimits is! List || rawLimits.isEmpty) {
    throw MermanException.contract(
      'runtime resources.limits must be a non-empty array',
    );
  }
  final limits = <MermanResourceLimitDescriptor>[];
  final limitIds = <String, MermanResourceLimitId>{};
  for (var index = 0; index < rawLimits.length; index += 1) {
    final label = 'runtime resources.limits[$index]';
    final limit = _asObject(rawLimits[index], label);
    _requireRequiredKeys(limit, const {
      'id',
      'phase',
      'description',
      'overridable',
      'hard_cap',
      'minimum_value',
    }, label);
    final idText = _requiredNonEmptyString(limit, 'id', label);
    if (!_isRuntimeFieldIdentifier(idText)) {
      throw MermanException.contract(
        '$label.id must be a stable field identifier',
      );
    }
    if (limitIds.containsKey(idText)) {
      throw MermanException.contract(
        'runtime resource limit ID `$idText` is duplicated',
      );
    }
    final id = MermanResourceLimitId.fromId(idText);
    limitIds[idText] = id;
    final overridable = _requiredBool(limit, 'overridable', label);
    final hardCap = _requiredBool(limit, 'hard_cap', label);
    if (hardCap && overridable) {
      throw MermanException.contract(
        'runtime resource limit `$idText` cannot be both a hard cap and overridable',
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
    limits.add(
      MermanResourceLimitDescriptor(
        id: id,
        phase: _requiredNonEmptyString(limit, 'phase', label),
        description: _requiredNonEmptyString(limit, 'description', label),
        overridable: overridable,
        hardCap: hardCap,
        minimumValue: _requiredNonNegativeInt(limit, 'minimum_value', label),
        operationIds: limitOperationIds,
        additionalFields: _additionalJsonFields(limit, const {
          'id',
          'phase',
          'description',
          'overridable',
          'hard_cap',
          'minimum_value',
          'operation_ids',
        }),
      ),
    );
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
    _requireRequiredKeys(profile, const {
      'id',
      'purpose',
      'trust_assumption',
      'recommended_binding_default',
      'limits',
    }, label);
    final id = _requiredNonEmptyString(profile, 'id', label);
    if (!_isRuntimeIdentifier(id)) {
      throw MermanException.contract(
        '$label.id must be a stable runtime identifier',
      );
    }
    if (!profileIds.add(id)) {
      throw MermanException.contract(
        'runtime resource profile ID `$id` is duplicated',
      );
    }

    final rawProfileLimits = _asObject(profile['limits'], '$label.limits');
    final profileLimitIds = rawProfileLimits.keys.toSet();
    final declaredLimitIds = limitIds.keys.toSet();
    final missingLimitIds = declaredLimitIds.difference(profileLimitIds);
    final unknownLimitIds = profileLimitIds.difference(declaredLimitIds);
    if (missingLimitIds.isNotEmpty || unknownLimitIds.isNotEmpty) {
      throw MermanException.contract(
        '$label.limits must contain exactly the declared resource limit IDs; '
        'missing: ${missingLimitIds.toList()..sort()}, '
        'unknown: ${unknownLimitIds.toList()..sort()}',
      );
    }

    final profileLimits = <MermanResourceLimitId, int?>{};
    for (final limit in limits) {
      final value = rawProfileLimits[limit.id.id];
      if (value != null && (value is! int || value < limit.minimumValue)) {
        throw MermanException.contract(
          '$label.limits[`${limit.id.id}`] must be null or at least ${limit.minimumValue}',
        );
      }
      if (limit.hardCap && value == null) {
        throw MermanException.contract(
          '$label.limits[`${limit.id.id}`] must retain its finite hard cap',
        );
      }
      profileLimits[limit.id] = value as int?;
    }
    profiles.add(
      MermanResourceProfileDescriptor(
        id: id,
        purpose: _requiredNonEmptyString(profile, 'purpose', label),
        trustAssumption: _requiredNonEmptyString(
          profile,
          'trust_assumption',
          label,
        ),
        recommendedBindingDefault: _requiredBool(
          profile,
          'recommended_binding_default',
          label,
        ),
        limits: profileLimits,
        additionalFields: _additionalJsonFields(profile, const {
          'id',
          'purpose',
          'trust_assumption',
          'recommended_binding_default',
          'limits',
        }),
      ),
    );
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
  final recommendedProfiles = profiles
      .where((profile) => profile.recommendedBindingDefault)
      .toList();
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
    _requireRequiredKeys(contract, const {
      'id',
      'media_type',
      'system_fonts',
      'embedded_images',
    }, 'runtime output contract');
    final id = _requiredNonEmptyString(
      contract,
      'id',
      'runtime output contract',
    );
    if (!_isRuntimeIdentifier(id)) {
      throw MermanException.contract(
        'runtime output contract ID must be a stable runtime identifier',
      );
    }
    contracts.add(
      MermanRuntimeOutputContract(
        id: id,
        mediaType: _requiredNonEmptyString(
          contract,
          'media_type',
          'runtime output contract',
        ),
        systemFonts: _parseRuntimeSystemFontContract(contract['system_fonts']),
        embeddedImages: _parseRuntimeEmbeddedImageContract(
          contract['embedded_images'],
        ),
      ),
    );
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
  _requireRequiredKeys(fonts, const {
    'source_id',
    'discovery',
    'cache_scope',
    'host_dependent',
    'caller_configurable',
    'resource_bounded',
  }, 'runtime system font contract');
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
  _requireRequiredKeys(images, const {
    'source_ids',
    'filesystem_access',
    'network_access',
    'caller_configurable',
    'limits',
  }, 'runtime embedded image contract');
  final limits = _asObject(images['limits'], 'runtime embedded image limits');
  _requireRequiredKeys(limits, const {
    'max_bytes_per_image',
    'max_total_bytes',
    'max_pixels_per_image',
    'max_total_pixels',
  }, 'runtime embedded image limits');
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

MermanOperationMetadata _decodeOperationMetadata(Uint8List bytes) {
  try {
    return decodeMermanOperationMetadata(utf8.decode(bytes));
  } on FormatException catch (error) {
    throw MermanException.contract(error.message);
  }
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
      .map((entry) => decode(_asObject(entry.$2, '$label.$field[${entry.$1}]')))
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

Map<String, Object?> _additionalJsonFields(
  Map<String, Object?> source,
  Set<String> knownKeys,
) => <String, Object?>{
  for (final entry in source.entries)
    if (!knownKeys.contains(entry.key)) entry.key: entry.value,
};

String _encodePreservedJsonObject(Map<String, Object?> source, String label) {
  try {
    return jsonEncode(source);
  } on JsonUnsupportedObjectError {
    throw MermanException.contract('$label contains a non-JSON value');
  }
}

Map<String, Object?> _deeplyUnmodifiableJsonObject(
  Map<String, Object?> source,
  String label,
) => Map.unmodifiable(<String, Object?>{
  for (final entry in source.entries)
    entry.key: _deeplyUnmodifiableJsonValue(entry.value, '$label.${entry.key}'),
});

Object? _deeplyUnmodifiableJsonValue(Object? value, String label) {
  if (value == null || value is String || value is num || value is bool) {
    return value;
  }
  if (value is List) {
    return List<Object?>.unmodifiable(
      value.indexed.map(
        (entry) =>
            _deeplyUnmodifiableJsonValue(entry.$2, '$label[${entry.$1}]'),
      ),
    );
  }
  if (value is Map) {
    return _deeplyUnmodifiableJsonObject(_asObject(value, label), label);
  }
  throw MermanException.contract('$label must be a JSON value');
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
    throw MermanException.contract('$label.$key must be a non-empty string');
  }
  return value;
}

String _requiredString(Map<String, Object?> source, String key, String label) {
  final value = source[key];
  if (value is! String) {
    throw MermanException.contract('$label.$key must be a string');
  }
  return value;
}

bool _requiredBool(Map<String, Object?> source, String key, String label) {
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
    if (item is! String || !_isRuntimeIdentifier(item)) {
      throw MermanException.contract(
        '$label must contain stable runtime identifiers',
      );
    }
    if (previous != null && previous.compareTo(item) >= 0) {
      throw MermanException.contract('$label must be sorted and unique');
    }
    previous = item;
    values.add(item);
  }
  return values;
}

List<String> _requiredSortedUniqueFieldIdentifiers(
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
    if (item is! String || !_isRuntimeFieldIdentifier(item)) {
      throw MermanException.contract(
        '$label must contain stable field identifiers',
      );
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

List<int> _requiredSortedUniquePositiveInts(
  Map<String, Object?> source,
  String key,
  String label,
) {
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

void _validateRuntimeOptionGroups(
  List<String> ids,
  Set<String> capabilityIds, {
  required bool usesSvgPipeline,
}) {
  for (final id in ids) {
    if (!_isRuntimeFieldIdentifier(id)) {
      throw MermanException.contract(
        'runtime option group ID `$id` is not a field identifier',
      );
    }
  }
  final expectedKnownIds =
      mermanBindingOptionGroupSpecs.values
          .where(
            (spec) =>
                spec.alwaysAvailable ||
                (spec.requiresSvgPipeline && usesSvgPipeline) ||
                spec.anyCapabilityIds.any(capabilityIds.contains),
          )
          .map((spec) => spec.id)
          .toList()
        ..sort();
  final actualKnownIds = ids
      .where(mermanBindingOptionGroupSpecs.containsKey)
      .toList(growable: false);
  if (!_sameStrings(actualKnownIds, expectedKnownIds)) {
    throw MermanException.contract(
      'runtime option group IDs do not match the artifact capability closure',
    );
  }
}

void _validateRuntimeOperationRelations(
  List<String> operationIds,
  Set<String> capabilityIds,
  Set<String> outputIds,
) {
  for (final operationId in operationIds) {
    final expectation = _operationExpectationById[operationId];
    final requiredCapabilityId = expectation?.availabilityCapabilityId;
    if (requiredCapabilityId != null &&
        !capabilityIds.contains(requiredCapabilityId)) {
      throw MermanException.contract(
        'runtime operation `$operationId` requires capability '
        '`$requiredCapabilityId`',
      );
    }
    final outputId = expectation?.outputId;
    if (outputId != null && !outputIds.contains(outputId)) {
      throw MermanException.contract(
        'runtime operation `$operationId` requires output `$outputId`',
      );
    }
  }
}

void _validateRuntimeCapabilityRelations(
  List<String> capabilityIds,
  Set<String> capabilitySet,
) {
  for (final capabilityId in capabilityIds) {
    final spec = mermanBindingCapabilitySpecs[capabilityId];
    if (spec == null) continue;
    for (final implicationId in spec.implicationIds) {
      if (!capabilitySet.contains(implicationId)) {
        throw MermanException.contract(
          'runtime capability `$capabilityId` requires implied capability '
          '`$implicationId`',
        );
      }
    }
  }
}

void _validateRuntimeMetadataRelations(
  List<String> metadataIds,
  Set<String> capabilityIds,
) {
  for (final metadataId in metadataIds) {
    final requiredCapabilityId =
        mermanBindingMetadataSpecs[metadataId]?.requiredCapabilityId;
    if (requiredCapabilityId != null &&
        !capabilityIds.contains(requiredCapabilityId)) {
      throw MermanException.contract(
        'runtime metadata `$metadataId` requires capability '
        '`$requiredCapabilityId`',
      );
    }
  }
}

void _validateRuntimeConstructorServiceIds(
  List<String> ids, {
  required bool usesSvgPipeline,
}) {
  final candidates = mermanBindingTransportExposureSpecs['native-c']!
      .constructorServiceCandidateIds;
  final expectedKnownIds =
      mermanBindingConstructorServiceSpecs.values
          .where(
            (spec) =>
                candidates.contains(spec.id) &&
                (!spec.requiresSvgPipeline || usesSvgPipeline),
          )
          .map((spec) => spec.id)
          .toList()
        ..sort();
  final actualKnownIds = ids.where(candidates.contains).toList(growable: false);
  if (!_sameStrings(actualKnownIds, expectedKnownIds)) {
    throw MermanException.contract(
      'runtime constructor service IDs do not match the native C transport exposure',
    );
  }
}

const String _hostTextMeasurementServiceId =
    mermanHostTextMeasurementConstructorServiceId;

const String _iconRegistryServiceId = mermanIconRegistryConstructorServiceId;

MermanBindingConstructorResourceLimitSpec _iconRegistryResourceLimit(
  String id,
) {
  final limits = mermanBindingConstructorServiceSpecs[_iconRegistryServiceId]!
      .resourceLimits;
  for (final limit in limits) {
    if (limit.id == id) return limit;
  }
  throw StateError('generated icon-registry limit `$id` is missing');
}

Never _throwIconRegistryResourceLimit(
  MermanBindingConstructorResourceLimitSpec limit, {
  required int actual,
  required String message,
  int? packIndex,
  String? registrationName,
}) {
  throw MermanException(
    code: native.MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED,
    codeName: 'resource-limit-exceeded',
    message: message,
    resourceDetails: MermanResourceErrorDetails(
      cause: 'ceiling',
      limitId: MermanResourceLimitId.fromId(limit.id),
      phase: limit.phase,
      actual: actual,
      max: limit.value,
      profile: 'constructor-fixed',
    ),
    iconRegistryDetails: MermanIconRegistryErrorDetails(
      kindId: 'resource_limit_exceeded',
      packIndex: packIndex,
      registrationName: registrationName,
    ),
  );
}

int _boundedIconRegistryUtf8Length(
  String value, {
  required int byteCeiling,
  required int packIndex,
  required String field,
  String? registrationName,
}) {
  var byteLength = 0;
  var hasInvalidSurrogate = false;
  for (var index = 0; index < value.length; index += 1) {
    final codeUnit = value.codeUnitAt(index);
    if (codeUnit <= 0x7f) {
      byteLength += 1;
    } else if (codeUnit <= 0x7ff) {
      byteLength += 2;
    } else if (codeUnit >= 0xd800 && codeUnit <= 0xdbff) {
      if (index + 1 < value.length) {
        final next = value.codeUnitAt(index + 1);
        if (next >= 0xdc00 && next <= 0xdfff) {
          index += 1;
          byteLength += 4;
        } else {
          hasInvalidSurrogate = true;
          byteLength += 3;
        }
      } else {
        hasInvalidSurrogate = true;
        byteLength += 3;
      }
    } else if (codeUnit >= 0xdc00 && codeUnit <= 0xdfff) {
      hasInvalidSurrogate = true;
      byteLength += 3;
    } else {
      byteLength += 3;
    }
    if (byteLength > byteCeiling) {
      return byteLength;
    }
  }
  if (hasInvalidSurrogate) {
    _throwIconRegistryInvalidUtf16(
      packIndex: packIndex,
      field: field,
      registrationName: registrationName,
    );
  }
  return byteLength;
}

Never _throwIconRegistryInvalidUtf16({
  required int packIndex,
  required String field,
  required String? registrationName,
}) {
  throw MermanException(
    code: native.MERMAN_NATIVE_STATUS_UTF8_ERROR,
    codeName: 'utf8-error',
    message:
        'icon registry $field contains an isolated UTF-16 surrogate and cannot be encoded as valid UTF-8',
    iconRegistryDetails: MermanIconRegistryErrorDetails(
      kindId: 'invalid_utf8',
      packIndex: packIndex,
      registrationName: registrationName,
    ),
  );
}

final Map<String, MermanBindingOperationExpectation> _operationExpectationById =
    Map.unmodifiable({
      for (final expectation in mermanBindingOperationExpectations)
        expectation.operationId: expectation,
    });

final Map<String, String> _generatedConstructorResourceLimitOwners =
    Map.unmodifiable({
      for (final service in mermanBindingConstructorServiceSpecs.values)
        for (final limit in service.resourceLimits) limit.id: service.id,
    });

List<MermanRuntimeConstructorServiceContract>
_parseRuntimeConstructorServiceContracts(
  Object? value,
  List<String> serviceIds,
  List<String> availableProviderIds,
) {
  if (value is! List) {
    throw MermanException.contract(
      'runtime constructor service contracts must be an array',
    );
  }
  final contracts = <MermanRuntimeConstructorServiceContract>[];
  final providerOwners = <String, String>{};
  final generatedProviderOwners = <String, String>{
    for (final spec in mermanBindingConstructorServiceSpecs.values)
      for (final providerId in spec.providedTextMeasurementProviderIds)
        providerId: spec.id,
  };
  String? previousServiceId;
  for (var index = 0; index < value.length; index += 1) {
    final label = 'runtime constructor service contracts[$index]';
    final contract = _asObject(value[index], label);
    _requireRequiredKeys(contract, const {
      'id',
      'provided_text_measurement_provider_ids',
      'resource_limits',
    }, label);
    final id = _requiredNonEmptyString(contract, 'id', label);
    if (!_isRuntimeIdentifier(id)) {
      throw MermanException.contract('$label.id must be a stable identifier');
    }
    if (previousServiceId != null && previousServiceId.compareTo(id) >= 0) {
      throw MermanException.contract(
        'runtime constructor service contracts must be sorted and unique by ID',
      );
    }
    final providerIds = _requiredSortedUniqueStrings(
      contract,
      'provided_text_measurement_provider_ids',
      '$label text measurement provider IDs',
    );
    final generatedServiceSpec = mermanBindingConstructorServiceSpecs[id];
    if (generatedServiceSpec != null) {
      final expectedKnownProviderIds =
          generatedServiceSpec.providedTextMeasurementProviderIds.toList()
            ..sort();
      final actualKnownProviderIds = providerIds
          .where(generatedProviderOwners.containsKey)
          .toList(growable: false);
      if (!_sameStrings(actualKnownProviderIds, expectedKnownProviderIds)) {
        throw MermanException.contract(
          'runtime constructor service `$id` does not match its generated '
          'text measurement provider contract',
        );
      }
    }
    for (final providerId in providerIds) {
      if (!availableProviderIds.contains(providerId)) {
        throw MermanException.contract(
          'runtime constructor service `$id` names unavailable text '
          'measurement provider `$providerId`',
        );
      }
      final previousOwner = providerOwners[providerId];
      if (previousOwner != null) {
        throw MermanException.contract(
          'runtime text measurement provider `$providerId` has multiple '
          'constructor service owners: `$previousOwner` and `$id`',
        );
      }
      final generatedOwner = generatedProviderOwners[providerId];
      if (generatedOwner != null && generatedOwner != id) {
        throw MermanException.contract(
          'runtime text measurement provider `$providerId` belongs to '
          'constructor service `$generatedOwner`, not `$id`',
        );
      }
      providerOwners[providerId] = id;
    }
    final resourceLimits = _parseRuntimeConstructorResourceLimits(
      contract['resource_limits'],
      id,
    );
    contracts.add(
      MermanRuntimeConstructorServiceContract(
        id: id,
        providedTextMeasurementProviderIds: providerIds,
        resourceLimits: resourceLimits,
      ),
    );
    previousServiceId = id;
  }
  final contractIds = contracts.map((contract) => contract.id).toList();
  if (!_sameStrings(contractIds, serviceIds)) {
    throw MermanException.contract(
      'runtime constructor service contracts must exactly match service IDs',
    );
  }
  return contracts;
}

List<MermanRuntimeConstructorResourceLimit>
_parseRuntimeConstructorResourceLimits(Object? value, String serviceId) {
  if (value is! List) {
    throw MermanException.contract(
      'runtime constructor service `$serviceId` resource limits must be an array',
    );
  }
  final limits = <MermanRuntimeConstructorResourceLimit>[];
  String? previousId;
  for (var index = 0; index < value.length; index += 1) {
    final label =
        'runtime constructor service `$serviceId` resource limits[$index]';
    final limit = _asObject(value[index], label);
    _requireRequiredKeys(limit, const {
      'id',
      'phase',
      'unit',
      'description',
      'value',
    }, label);
    final id = _requiredNonEmptyString(limit, 'id', label);
    if (!_isRuntimeFieldIdentifier(id)) {
      throw MermanException.contract('$label.id must be a field identifier');
    }
    if (previousId != null && previousId.compareTo(id) >= 0) {
      throw MermanException.contract(
        'runtime constructor service `$serviceId` resource limits must be '
        'sorted and unique by ID',
      );
    }
    limits.add(
      MermanRuntimeConstructorResourceLimit(
        id: id,
        phase: _requiredNonEmptyString(limit, 'phase', label),
        unit: _requiredNonEmptyString(limit, 'unit', label),
        description: _requiredNonEmptyString(limit, 'description', label),
        value: _requiredNonNegativeInt(limit, 'value', label),
      ),
    );
    previousId = id;
  }
  for (final limit in limits) {
    final generatedOwner = _generatedConstructorResourceLimitOwners[limit.id];
    if (generatedOwner != null && generatedOwner != serviceId) {
      throw MermanException.contract(
        'runtime constructor resource limit `${limit.id}` belongs to '
        'generated service `$generatedOwner`, not `$serviceId`',
      );
    }
  }
  final generatedService = mermanBindingConstructorServiceSpecs[serviceId];
  if (generatedService != null) {
    final expected = generatedService.resourceLimits;
    final actualById = <String, MermanRuntimeConstructorResourceLimit>{
      for (final limit in limits) limit.id: limit,
    };
    final matches = expected.every((generated) {
      final actual = actualById[generated.id];
      return actual != null &&
          actual.phase == generated.phase &&
          actual.unit == generated.unit &&
          actual.description == generated.description &&
          actual.value == generated.value;
    });
    if (!matches) {
      throw MermanException.contract(
        'runtime constructor service `$serviceId` resource limits do not '
        'match the generated contract',
      );
    }
  }
  return limits;
}

bool _isRuntimeFieldIdentifier(String value) =>
    RegExp(mermanRuntimeCatalogFieldIdentifierPattern).hasMatch(value);

bool _isRuntimeIdentifier(String value) =>
    RegExp(mermanRuntimeCatalogIdentifierPattern).hasMatch(value);

bool _sameStrings(List<String> left, List<String> right) {
  if (left.length != right.length) {
    return false;
  }
  for (var index = 0; index < left.length; index += 1) {
    if (left[index] != right[index]) {
      return false;
    }
  }
  return true;
}

List<MermanRuntimePayloadSchema> _parseRuntimePayloadSchemas(Object? value) {
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
    if (!_isRuntimeIdentifier(id)) {
      throw MermanException.contract(
        '$label.id must be a stable runtime identifier',
      );
    }
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
