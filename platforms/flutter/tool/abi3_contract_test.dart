import 'dart:convert';
import 'dart:typed_data';

import 'package:merman/merman.dart';

void main() {
  acceptsAFlatAbi3Catalog();
  acceptsInvariantOnlyCatalog();
  rejectsDuplicateCapabilityIds();
  rejectsUncallableOutputIds();
  rejectsInconsistentAdapters();
  rejectsInconsistentTextMeasurement();
  rejectsMalformedResources();
  textMeasurementFactoriesRejectMalformedValues();
  decodesMachineReadableNativeErrors();
  rejectsMismatchedNativeErrorSchema();
  print('ABI 3 Dart contract tests passed');
}

void acceptsAFlatAbi3Catalog() {
  final catalog = MermanRuntimeCatalog.fromJson(_catalog());
  _expect(
      catalog.packageVersion == 'test', 'package version should be preserved');
  _expect(
      catalog.supportsCapability('svg'), 'SVG capability should be present');
  _expect(catalog.supportsOutput('svg'), 'SVG output should be present');
  _expect(
    catalog.supportsOperation('semantic-json'),
    'invariant semantic operation should be present',
  );
  _expect(
    catalog.diagramFamilyCount == 35 &&
        catalog.generalBindingDefaultProfile == 'interactive' &&
        catalog.cliDefaultProfile == 'trusted-native',
    'flat registry/resource facts should be preserved',
  );
}

void acceptsInvariantOnlyCatalog() {
  final catalog = _catalog();
  final capabilities = _runtimeCapabilities(catalog);
  capabilities['capability_ids'] = <String>[];
  capabilities['output_ids'] = <String>[];
  capabilities['operation_ids'] = ['semantic-json'];
  capabilities['system_adapter_ids'] = <String>[];
  capabilities['text_measurement'] = null;

  final validated = MermanRuntimeCatalog.fromJson(catalog);
  _expect(
    validated.operationIds.length == 1 &&
        validated.supportsOperation('semantic-json'),
    'base artifacts should preserve invariant semantic operations',
  );
}

void rejectsDuplicateCapabilityIds() {
  final catalog = _catalog();
  _runtimeCapabilities(catalog)['capability_ids'] = ['svg', 'svg'];
  _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
}

void rejectsUncallableOutputIds() {
  final catalog = _catalog();
  _runtimeCapabilities(catalog)['operation_ids'] = ['semantic-json'];
  _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
}

void rejectsInconsistentAdapters() {
  final catalog = _catalog(
    capabilityIds: ['svg'],
    outputIds: ['svg'],
    operationIds: ['semantic-json', 'svg'],
    systemAdapterIds: ['system-clock'],
  );
  _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
}

void rejectsInconsistentTextMeasurement() {
  final catalog = _catalog();
  _runtimeCapabilities(catalog)['text_measurement'] = null;
  _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
}

void rejectsMalformedResources() {
  final catalog = _catalog();
  final resources = catalog['resources'] as Map<String, Object?>;
  resources.remove('profiles');
  _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
}

void textMeasurementFactoriesRejectMalformedValues() {
  _expectThrows<RangeError>(() {
    MermanTextMeasureResult.metrics(width: 1, height: 1, lineCount: 0);
  });
  _expectThrows<ArgumentError>(() {
    MermanTextMeasureResult.length(length: double.nan);
  });
  _expectThrows<RangeError>(() {
    MermanTextMeasureResult.horizontalExtents(left: -1, right: 1);
  });
}

void decodesMachineReadableNativeErrors() {
  final unknown = MermanException.fromNative(
    7,
    Uint8List.fromList(
      utf8.encode(
        jsonEncode({
          'version': 1,
          'ok': false,
          'status': 7,
          'status_name': 'unsupported-operation',
          'kind': 'unknown-operation',
          'capability_id': null,
          'message': 'unknown operation',
        }),
      ),
    ),
  );
  _expect(
    unknown is MermanUnknownOperationException &&
        unknown.kind == MermanErrorKind.unknownOperation &&
        unknown.capabilityId == null,
    'unknown operation classification should survive the Dart boundary',
  );

  final reentrant = MermanException.fromNative(
    14,
    Uint8List.fromList(
      utf8.encode(
        jsonEncode({
          'version': 1,
          'ok': false,
          'status': 14,
          'status_name': 'reentrant-call',
          'kind': 'reentrant-call',
          'capability_id': null,
          'message': 'same engine was re-entered',
        }),
      ),
    ),
  );
  _expect(
    reentrant.kind == MermanErrorKind.reentrantCall,
    'reentrant classification should survive the Dart boundary',
  );
}

void rejectsMismatchedNativeErrorSchema() {
  final error = MermanException.fromNative(
    7,
    Uint8List.fromList(
      utf8.encode(
        jsonEncode({
          'version': 2,
          'ok': false,
          'status': 7,
          'status_name': 'unsupported-operation',
          'kind': 'missing-capability',
          'capability_id': 'svg',
          'message': 'stale payload',
        }),
      ),
    ),
  );
  _expect(
    error.codeName == 'DART_NATIVE_CONTRACT_ERROR' &&
        error.kind == MermanErrorKind.generic &&
        error.capabilityId == null,
    'a stale native error schema should fail closed as a contract error',
  );
}

Map<String, Object?> _catalog({
  List<String> capabilityIds = const ['svg'],
  List<String> outputIds = const ['svg'],
  List<String> operationIds = const ['semantic-json', 'svg'],
  List<String> systemAdapterIds = const [],
}) {
  return {
    'schema_version': 1,
    'transport_api_version': 3,
    'package_version': 'test',
    'capabilities': {
      'capability_ids': capabilityIds,
      'output_ids': outputIds,
      'operation_ids': operationIds,
      'system_adapter_ids': systemAdapterIds,
      'text_measurement': capabilityIds.contains('svg')
          ? {
              'protocol_version': 1,
              'provider_ids': ['vendored'],
            }
          : null,
    },
    'registry': {'diagram_family_count': 35},
    'resources': {
      'schema_version': 1,
      'general_binding_default_profile': 'interactive',
      'cli_default_profile': 'trusted-native',
      'limits': <Object?>[],
      'profiles': <Object?>[],
    },
  };
}

Map<String, Object?> _runtimeCapabilities(Map<String, Object?> catalog) =>
    catalog['capabilities']! as Map<String, Object?>;

void _expectContractFailure(void Function() action) {
  _expectThrows<MermanException>(action);
}

void _expect(bool condition, String message) {
  if (!condition) {
    throw StateError(message);
  }
}

void _expectThrows<T extends Object>(void Function() action) {
  try {
    action();
  } on T {
    return;
  }
  throw StateError('expected $T');
}
