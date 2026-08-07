import 'dart:convert';

/// Typed schema-1 metadata returned with every native operation result.
///
/// [rawJson] preserves the complete metadata document so callers can inspect
/// additive fields introduced by a newer compatible native producer.
final class MermanOperationMetadata {
  const MermanOperationMetadata({
    required this.version,
    required this.operationId,
    required this.mediaType,
    required this.runtimePolicy,
    required this.byteLength,
    required this.outputPlan,
    required this.rawJson,
  });

  final int version;
  final String operationId;
  final String mediaType;
  final String runtimePolicy;
  final int byteLength;
  final MermanOutputPlan? outputPlan;
  final String rawJson;

  /// Decodes the preserved document, including fields unknown to this SDK.
  Map<String, Object?> get jsonObject {
    final value = jsonDecode(rawJson);
    if (value is! Map<Object?, Object?>) {
      throw const FormatException('operation metadata must be a JSON object');
    }
    return _immutableJsonObject(value, 'operation metadata');
  }
}

/// Open output-plan vocabulary carried by operation metadata.
abstract base class MermanOutputPlan {
  const MermanOutputPlan._();

  String get kind;
}

/// Effective raster dimensions after resource-limit planning.
final class MermanRasterOutputPlan extends MermanOutputPlan {
  const MermanRasterOutputPlan({
    required this.requestedWidthPx,
    required this.requestedHeightPx,
    required this.widthPx,
    required this.heightPx,
    required this.requestedScale,
    required this.effectiveScale,
    required this.limited,
  }) : super._();

  @override
  String get kind => 'raster';

  final double requestedWidthPx;
  final double requestedHeightPx;
  final int widthPx;
  final int heightPx;
  final double requestedScale;
  final double effectiveScale;
  final bool limited;
}

/// Effective rasterization budget for SVG filter groups embedded in a PDF.
final class MermanPdfFilterImagesOutputPlan extends MermanOutputPlan {
  const MermanPdfFilterImagesOutputPlan({
    required this.filteredGroups,
    required this.requestedScale,
    required this.effectiveScale,
    required this.requestedImagePixels,
    required this.effectiveImagePixels,
    required this.limited,
  }) : super._();

  @override
  String get kind => 'pdf-filter-images';

  final int filteredGroups;
  final double requestedScale;
  final double effectiveScale;
  final int requestedImagePixels;
  final int effectiveImagePixels;
  final bool limited;
}

/// A future output plan not understood by this SDK version.
///
/// [rawJson] preserves the complete plan object instead of discarding fields
/// or rejecting an additive plan kind.
final class MermanUnknownOutputPlan extends MermanOutputPlan {
  const MermanUnknownOutputPlan({
    required this.kind,
    required this.rawJson,
  }) : super._();

  @override
  final String kind;
  final String rawJson;

  Map<String, Object?> get jsonObject {
    final value = jsonDecode(rawJson);
    if (value is! Map<Object?, Object?>) {
      throw const FormatException('unknown output plan must be a JSON object');
    }
    return _immutableJsonObject(value, 'unknown output plan');
  }
}

Map<String, Object?> _immutableJsonObject(
  Map<Object?, Object?> value,
  String label,
) {
  final result = <String, Object?>{};
  for (final entry in value.entries) {
    final key = entry.key;
    if (key is! String) {
      throw FormatException('$label contains a non-string key');
    }
    result[key] = _immutableJsonValue(entry.value, '$label.$key');
  }
  return Map.unmodifiable(result);
}

Object? _immutableJsonValue(Object? value, String label) {
  if (value == null || value is String || value is num || value is bool) {
    return value;
  }
  if (value is List<Object?>) {
    return List<Object?>.unmodifiable(
      value.indexed.map(
        (entry) => _immutableJsonValue(entry.$2, '$label[${entry.$1}]'),
      ),
    );
  }
  if (value is Map<Object?, Object?>) {
    return _immutableJsonObject(value, label);
  }
  throw FormatException('$label is not a JSON value');
}
