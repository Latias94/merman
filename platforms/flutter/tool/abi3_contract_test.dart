import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:merman/merman.dart';
import 'package:merman/src/generated/native_abi.dart' as native;
import 'package:merman/src/merman_ffi.dart' as ffi_transport;

void main() {
  projectsFrozenAbi3MinimumPrefix();
  matchesThePubPackageVersionProjection();
  acceptsAFlatAbi3Catalog();
  projectsSvgPlanOperationFromGeneratedAbi();
  acceptsInvariantOnlyCatalog();
  acceptsAdditiveRuntimeCatalogFields();
  rejectsDuplicateCapabilityIds();
  rejectsUncallableOutputIds();
  rejectsInconsistentAdapters();
  rejectsCoercedRuntimeCatalogVersionFields();
  rejectsInconsistentTextMeasurement();
  rejectsTextMeasurementWithoutVendoredProvider();
  rejectsMalformedResources();
  textMeasurementFactoriesRejectMalformedValues();
  decodesMachineReadableNativeErrors();
  rejectsMismatchedNativeErrorSchema();
  preservesAllocationTokenExhaustionStatus();
  fuzzesNativeErrorPayloadDecoding();
  print('ABI 3 Dart contract tests passed');
}

void projectsFrozenAbi3MinimumPrefix() {
  _expect(
    native.MERMAN_NATIVE_ABI_MINIMUM_PREFIX_LAYOUT_DIGEST.startsWith('sha256:'),
    'ABI discovery must use the generated minimum-prefix digest',
  );
  _expect(
    native.MERMAN_NATIVE_FUNCTION_RUNTIME_CATALOG == 0 &&
        native.MERMAN_NATIVE_FUNCTION_ENGINE_NEW == 1 &&
        native.MERMAN_NATIVE_FUNCTION_ENGINE_TRY_CLOSE == 2 &&
        native.MERMAN_NATIVE_FUNCTION_EXECUTE_COLLECT == 3 &&
        native.MERMAN_NATIVE_FUNCTION_RESULT_FREE == 4,
    'ABI 3 consumers must preserve the frozen five-slot function prefix',
  );

  final request = calloc<native.MermanNativeApiRequest>();
  final api = calloc<native.MermanNativeApi>();
  final result = calloc<native.MermanNativeResult>();
  try {
    request.ref.expected_minimum_prefix_layout_digest.struct_size =
        ffi.sizeOf<native.MermanNativeSlice>();
    _expect(
      api.ref.engine_try_close.address == 0,
      'zeroed ABI table should expose the engine_try_close slot',
    );
    result.ref.allocation_token = 1;
    _expect(
      result.ref.allocation_token == 1,
      'native result ownership must be represented by an opaque token',
    );
  } finally {
    calloc.free(request);
    calloc.free(api);
    calloc.free(result);
  }
}

void matchesThePubPackageVersionProjection() {
  final pubspec = File('pubspec.yaml').readAsStringSync();
  final match =
      RegExp(r'^version:\s*([^\s#]+)\s*$', multiLine: true).firstMatch(pubspec);
  _expect(match != null, 'pubspec.yaml must declare one package version');
  _expect(
    match!.group(1) == mermanPackageVersion,
    'the bundled native version pin must match pubspec.yaml',
  );
}

void projectsSvgPlanOperationFromGeneratedAbi() {
  final operation = MermanOperation.svgPlanJson;
  _expect(
    operation.nativeCode == native.MERMAN_NATIVE_OPERATION_SVG_PLAN_JSON,
    'SVG plan operation code must come from the generated ABI projection',
  );
  _expect(
    operation.operationId == native.MERMAN_NATIVE_OPERATION_ID_SVG_PLAN_JSON,
    'SVG plan operation ID must come from the generated ABI projection',
  );
  _expect(
    operation.requiresUri ==
        (native.MERMAN_NATIVE_OPERATION_REQUIRES_URI_SVG_PLAN_JSON != 0),
    'SVG plan URI contract must come from the generated ABI projection',
  );
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

void acceptsAdditiveRuntimeCatalogFields() {
  final catalog = _catalog();
  catalog['future_root'] = true;
  _runtimeCapabilities(catalog)['future_capability_metadata'] =
      <String, Object?>{};
  catalog['registry'] = <String, Object?>{
    ...catalog['registry'] as Map,
    'future_registry_metadata': true,
  };
  catalog['resources'] = <String, Object?>{
    ...catalog['resources'] as Map,
    'future_resource_metadata': true,
  };
  final textMeasurement =
      _runtimeCapabilities(catalog)['text_measurement'] as Map<String, Object?>;
  textMeasurement['future_text_measurement_metadata'] = true;

  final validated = MermanRuntimeCatalog.fromJson(catalog);
  _expect(
    validated.supportsCapability('svg'),
    'schema 1 consumers must tolerate additive catalog fields',
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

void rejectsCoercedRuntimeCatalogVersionFields() {
  for (final mutation in <void Function(Map<String, Object?>)>[
    (catalog) => catalog['schema_version'] = '1',
    (catalog) => catalog['schema_version'] = 1.0,
    (catalog) => catalog['transport_api_version'] = '3',
    (catalog) => catalog['transport_api_version'] = 3.0,
    (catalog) => catalog['package_version'] = 1,
    (catalog) => (_runtimeCapabilities(catalog)['text_measurement']
        as Map<String, Object?>)['protocol_version'] = '1',
    (catalog) => (_runtimeCapabilities(catalog)['text_measurement']
        as Map<String, Object?>)['protocol_version'] = 1.0,
  ]) {
    final catalog = _catalog();
    mutation(catalog);
    _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
  }
}

void rejectsInconsistentTextMeasurement() {
  final catalog = _catalog();
  _runtimeCapabilities(catalog)['text_measurement'] = null;
  _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
}

void rejectsTextMeasurementWithoutVendoredProvider() {
  for (final providers in <List<String>>[
    <String>[],
    <String>['host-callback'],
  ]) {
    final catalog = _catalog();
    final textMeasurement = _runtimeCapabilities(catalog)['text_measurement']
        as Map<String, Object?>;
    textMeasurement['provider_ids'] = providers;
    _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
  }
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
    reentrant is MermanReentrantCallException &&
        reentrant.kind == MermanErrorKind.reentrantCall,
    'reentrant classification should survive the Dart boundary',
  );

  final busy = MermanException.fromNative(
    native.MERMAN_NATIVE_STATUS_BUSY,
    Uint8List.fromList(
      utf8.encode(
        jsonEncode({
          'version': 1,
          'ok': false,
          'status': native.MERMAN_NATIVE_STATUS_BUSY,
          'status_name': 'busy',
          'kind': 'busy',
          'capability_id': null,
          'message': 'engine has an active operation',
        }),
      ),
    ),
  );
  _expect(
    busy is MermanBusyException && busy.kind == MermanErrorKind.busy,
    'busy classification should survive the Dart boundary',
  );

  final busyWithoutResult = MermanException.fromNative(
    native.MERMAN_NATIVE_STATUS_BUSY,
    Uint8List(0),
  );
  _expect(
    busyWithoutResult is MermanBusyException,
    'result-free engine close status must preserve busy classification',
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

void preservesAllocationTokenExhaustionStatus() {
  final result = calloc<native.MermanNativeResult>();
  try {
    result.ref.struct_size = ffi.sizeOf<native.MermanNativeResult>();
    final exhausted = _expectMermanException(
      () => ffi_transport.validateNativeResultForTesting(
        result,
        native.MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
      ),
    );
    _expect(
      exhausted.code == native.MERMAN_NATIVE_STATUS_INTERNAL_ERROR &&
          exhausted.codeName != 'DART_NATIVE_CONTRACT_ERROR',
      'token exhaustion must preserve the native internal-error status',
    );

    result.ref.operation = native.MERMAN_NATIVE_OPERATION_SEMANTIC_JSON;
    final corrupted = _expectMermanException(
      () => ffi_transport.validateNativeResultForTesting(
        result,
        native.MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
      ),
    );
    _expect(
      corrupted.codeName == 'DART_NATIVE_CONTRACT_ERROR',
      'a partially written zero-token result must fail closed',
    );

    result.ref.operation = 0;
    final missingToken = _expectMermanException(
      () => ffi_transport.validateNativeResultForTesting(
        result,
        native.MERMAN_NATIVE_STATUS_OK,
      ),
    );
    _expect(
      missingToken.codeName == 'DART_NATIVE_CONTRACT_ERROR',
      'a successful producing call must return a nonzero allocation token',
    );

    result.ref.allocation_token = 1;
    ffi_transport.validateNativeResultForTesting(
      result,
      native.MERMAN_NATIVE_STATUS_OK,
    );
  } finally {
    calloc.free(result);
  }
}

void fuzzesNativeErrorPayloadDecoding() {
  final random = Random(0x4d45524d);
  for (var iteration = 0; iteration < 512; iteration += 1) {
    final payload = Uint8List.fromList(
      List.generate(random.nextInt(128), (_) => random.nextInt(256)),
    );
    final status = random.nextBool()
        ? native.MERMAN_NATIVE_STATUS_BUSY
        : native.MERMAN_NATIVE_STATUS_REENTRANT_CALL;
    final error = MermanException.fromNative(status, payload);
    _expect(
      error.code == status || error.code == -1,
      'malformed native error payload escaped fail-closed decoding',
    );
  }
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

MermanException _expectMermanException(void Function() action) {
  try {
    action();
  } on MermanException catch (error) {
    return error;
  }
  throw StateError('expected MermanException');
}
