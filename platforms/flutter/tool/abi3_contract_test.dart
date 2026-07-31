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
  preservesTypedRuntimeOutputContracts();
  preservesCompleteRuntimeResourceContract();
  acceptsInvariantOnlyCatalog();
  acceptsAdditiveRuntimeCatalogFields();
  acceptsAdditiveRuntimeResourceIds();
  rejectsMissingRuntimeOutputContracts();
  rejectsOutputContractIdDrift();
  rejectsMalformedOutputContracts();
  rejectsDuplicateCapabilityIds();
  rejectsUncallableOutputIds();
  rejectsInconsistentAdapters();
  rejectsCoercedRuntimeCatalogVersionFields();
  rejectsInconsistentTextMeasurement();
  rejectsTextMeasurementWithoutVendoredProvider();
  rejectsMalformedResourceDescriptors();
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

void preservesCompleteRuntimeResourceContract() {
  final catalog = MermanRuntimeCatalog.fromJson(_catalog());
  _expect(
    catalog.resourceLimits.length == MermanResourceLimitId.values.length &&
        catalog.resourceProfiles.length == MermanResourceProfile.values.length,
    'runtime resource descriptors must be retained',
  );

  final sourceBytes = catalog.resourceLimitsById['max_source_bytes'];
  final interactive = catalog.resourceProfilesById['interactive'];
  final unbounded = catalog.resourceProfilesById['unbounded-for-trusted-input'];
  _expect(
    sourceBytes != null &&
        sourceBytes.phase == 'source' &&
        sourceBytes.description.isNotEmpty &&
        sourceBytes.overridable &&
        !sourceBytes.hardCap,
    'runtime limit phase, description, override, and hard-cap facts must survive',
  );
  _expect(
    interactive != null &&
        interactive.purpose.isNotEmpty &&
        interactive.trustAssumption.isNotEmpty &&
        interactive.recommendedBindingDefault &&
        interactive.limits.length == catalog.resourceLimits.length,
    'runtime profile purpose, trust, recommendation, and limits must survive',
  );
  _expect(
    unbounded != null &&
        unbounded.limits.values.every((value) => value == null),
    'runtime nullable limit facts must survive',
  );
  _expect(
    catalog.generalBindingDefaultResourceProfile.id == 'interactive' &&
        catalog.cliDefaultResourceProfile.id == 'trusted-native',
    'runtime default profile references must resolve to typed profiles',
  );
}

void preservesTypedRuntimeOutputContracts() {
  final catalog = MermanRuntimeCatalog.fromJson(_catalogWithExportOutput());
  final png = catalog.outputContractsById['png'];
  _expect(
    catalog.outputContracts.map((contract) => contract.id).toList().join(',') ==
            'png,svg' &&
        png != null &&
        png.mediaType == 'image/png' &&
        png.systemFonts != null &&
        png.systemFonts!.sourceId == 'host-system' &&
        png.systemFonts!.hostDependent &&
        png.embeddedImages != null &&
        png.embeddedImages!.sourceIds.single == 'data-url' &&
        png.embeddedImages!.limits.maxBytesPerImage == 1024 &&
        png.embeddedImages!.limits.maxTotalBytes == null &&
        png.embeddedImages!.limits.maxPixelsPerImage == 2048 &&
        png.embeddedImages!.limits.maxTotalPixels == 4096,
    'runtime output contracts must preserve typed native output behavior',
  );
}

void acceptsAdditiveRuntimeCatalogFields() {
  final catalog = _catalogWithExportOutput();
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
  final png = _outputContractAt(catalog, 'png');
  png['future_output_metadata'] = true;
  (png['system_fonts'] as Map<String, Object?>)['future_font_metadata'] = true;
  final embeddedImages = png['embedded_images'] as Map<String, Object?>;
  embeddedImages['future_image_metadata'] = true;
  (embeddedImages['limits'] as Map<String, Object?>)['future_limit_metadata'] =
      true;

  final validated = MermanRuntimeCatalog.fromJson(catalog);
  _expect(
    validated.supportsCapability('svg'),
    'schema 1 consumers must tolerate additive catalog fields',
  );
}

void acceptsAdditiveRuntimeResourceIds() {
  final catalog = _catalog();
  final resources = catalog['resources'] as Map<String, Object?>;
  final limits = resources['limits'] as List<Object?>;
  limits.add(<String, Object?>{
    'id': 'future_limit',
    'phase': 'future_phase',
    'description': 'Future additive resource limit',
    'overridable': false,
    'hard_cap': true,
    'future_limit_metadata': true,
  });
  for (final rawProfile in resources['profiles'] as List<Object?>) {
    final profile = rawProfile as Map<String, Object?>;
    final profileLimits = profile['limits'] as Map<String, Object?>;
    profileLimits['future_limit'] = 4096;
    profile['future_profile_metadata'] = true;
  }

  final validated = MermanRuntimeCatalog.fromJson(catalog);
  final futureLimit = validated.resourceLimitsById['future_limit'];
  _expect(
    futureLimit != null &&
        futureLimit.phase == 'future_phase' &&
        futureLimit.hardCap &&
        !futureLimit.overridable &&
        validated.resourceProfiles.every(
          (profile) =>
              profile.limits.containsKey('future_limit') &&
              profile.limits['future_limit'] == 4096,
        ),
    'ABI 3 consumers must retain additive declared resource IDs',
  );
}

void acceptsInvariantOnlyCatalog() {
  final catalog = _catalog();
  final capabilities = _runtimeCapabilities(catalog);
  capabilities['capability_ids'] = <String>[];
  capabilities['output_ids'] = <String>[];
  catalog['output_contracts'] = <Object?>[];
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

void rejectsMissingRuntimeOutputContracts() {
  final catalog = _catalog();
  catalog.remove('output_contracts');
  _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
}

void rejectsOutputContractIdDrift() {
  for (final mutation in <void Function(Map<String, Object?>)>[
    (catalog) => _outputContracts(catalog).removeLast(),
    (catalog) => _outputContractAt(catalog, 'png')['id'] = 'svg',
    (catalog) => _outputContracts(catalog).add(_outputContract('svg')),
  ]) {
    final catalog = _catalogWithExportOutput();
    mutation(catalog);
    _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
  }
}

void rejectsMalformedOutputContracts() {
  for (final mutation in <void Function(Map<String, Object?>)>[
    (catalog) => _outputContractAt(catalog, 'png')['media_type'] = 1,
    (catalog) =>
        _outputContractAt(catalog, 'png')['system_fonts'] = <String, Object?>{},
    (catalog) => (_outputContractAt(catalog, 'png')['system_fonts']
        as Map<String, Object?>)['source_id'] = 1,
    (catalog) => (_outputContractAt(catalog, 'png')['system_fonts']
        as Map<String, Object?>)['host_dependent'] = 'true',
    (catalog) => (_outputContractAt(catalog, 'png')['embedded_images']
        as Map<String, Object?>)['source_ids'] = 'data-url',
    (catalog) => (_outputContractAt(catalog, 'png')['embedded_images']
        as Map<String, Object?>)['filesystem_access'] = 1,
    (catalog) => ((_outputContractAt(catalog, 'png')['embedded_images']
            as Map<String, Object?>)['limits']
        as Map<String, Object?>)['max_bytes_per_image'] = 0,
    (catalog) => ((_outputContractAt(catalog, 'png')['embedded_images']
            as Map<String, Object?>)['limits']
        as Map<String, Object?>)['max_total_bytes'] = 1.5,
    (catalog) => ((_outputContractAt(catalog, 'png')['embedded_images']
            as Map<String, Object?>)['limits']
        as Map<String, Object?>)['max_total_pixels'] = '4096',
  ]) {
    final catalog = _catalogWithExportOutput();
    mutation(catalog);
    _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
  }
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

void rejectsMalformedResourceDescriptors() {
  for (final mutation in <void Function(Map<String, Object?>)>[
    (catalog) => _resources(catalog).remove('profiles'),
    (catalog) => _limitAt(catalog, 0).remove('description'),
    (catalog) => _limitAt(catalog, 0)['hard_cap'] = 'false',
    (catalog) => _limitAt(catalog, 0)['overridable'] = 1,
    (catalog) => _limitAt(catalog, 0)
      ..['hard_cap'] = true
      ..['overridable'] = true,
    (catalog) => _limitAt(catalog, 0)
      ..['hard_cap'] = true
      ..['overridable'] = false,
    (catalog) => (_resources(catalog)['limits'] as List<Object?>)
        .add(Map<String, Object?>.from(_limitAt(catalog, 0))),
    (catalog) => _profileAt(catalog, 0).remove('purpose'),
    (catalog) => _profileAt(catalog, 0)['recommended_binding_default'] = 'true',
    (catalog) => (_profileAt(catalog, 0)['limits']
        as Map<String, Object?>)['max_source_bytes'] = '1',
    (catalog) => (_profileAt(catalog, 0)['limits']
        as Map<String, Object?>)['max_source_bytes'] = 1.5,
    (catalog) => (_profileAt(catalog, 0)['limits']
        as Map<String, Object?>)['max_source_bytes'] = 0,
    (catalog) => (_profileAt(catalog, 0)['limits'] as Map<String, Object?>)
        .remove('max_source_bytes'),
    (catalog) => (_profileAt(catalog, 0)['limits']
        as Map<String, Object?>)['undeclared_limit'] = null,
    (catalog) => (_resources(catalog)['profiles'] as List<Object?>)
        .add(Map<String, Object?>.from(_profileAt(catalog, 0))),
    (catalog) =>
        _resources(catalog)['general_binding_default_profile'] = 'missing',
    (catalog) => _resources(catalog)['cli_default_profile'] = 'missing',
    (catalog) {
      for (final rawProfile
          in _resources(catalog)['profiles'] as List<Object?>) {
        (rawProfile as Map<String, Object?>)['recommended_binding_default'] =
            false;
      }
    },
  ]) {
    final catalog = _clonedCatalog();
    mutation(catalog);
    _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
  }
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
    'output_contracts': <Object?>[
      for (final outputId in outputIds) _outputContract(outputId),
    ],
    'registry': {'diagram_family_count': 35},
    'resources': _resourceContract(),
  };
}

Map<String, Object?> _catalogWithExportOutput() => _catalog(
      capabilityIds: const ['png', 'svg'],
      outputIds: const ['png', 'svg'],
      operationIds: const ['png', 'semantic-json', 'svg'],
    );

Map<String, Object?> _outputContract(String outputId) {
  if (outputId == 'png') {
    return <String, Object?>{
      'id': 'png',
      'media_type': 'image/png',
      'system_fonts': <String, Object?>{
        'source_id': 'host-system',
        'discovery': 'first-use',
        'cache_scope': 'process-global',
        'host_dependent': true,
        'caller_configurable': false,
        'resource_bounded': false,
      },
      'embedded_images': <String, Object?>{
        'source_ids': <String>['data-url'],
        'filesystem_access': false,
        'network_access': false,
        'caller_configurable': false,
        'limits': <String, Object?>{
          'max_bytes_per_image': 1024,
          'max_total_bytes': null,
          'max_pixels_per_image': 2048,
          'max_total_pixels': 4096,
        },
      },
    };
  }
  return <String, Object?>{
    'id': outputId,
    'media_type': 'image/svg+xml',
    'system_fonts': null,
    'embedded_images': null,
  };
}

Map<String, Object?> _resourceContract() => <String, Object?>{
      'general_binding_default_profile': MermanResourceProfile.interactive.id,
      'cli_default_profile': MermanResourceProfile.trustedNative.id,
      'limits': <Object?>[
        for (final limit in MermanResourceLimitId.values)
          <String, Object?>{
            'id': limit.id,
            'phase': _resourceLimitPhase(limit),
            'description': 'Test descriptor for ${limit.id}',
            'overridable': limit.overridable,
            'hard_cap': false,
          },
      ],
      'profiles': <Object?>[
        for (final profile in MermanResourceProfile.values)
          <String, Object?>{
            'id': profile.id,
            'purpose': 'Test purpose for ${profile.id}',
            'trust_assumption': 'Test trust assumption for ${profile.id}',
            'recommended_binding_default':
                profile == MermanResourceProfile.interactive,
            'limits': <String, Object?>{
              for (final limit in MermanResourceLimitId.values)
                limit.id:
                    profile == MermanResourceProfile.unboundedForTrustedInput
                        ? null
                        : 1,
            },
          },
      ],
    };

String _resourceLimitPhase(MermanResourceLimitId limit) => switch (limit) {
      MermanResourceLimitId.maxSourceBytes => 'source',
      MermanResourceLimitId.maxModelItems ||
      MermanResourceLimitId.maxModelTextBytes ||
      MermanResourceLimitId.maxModelNestingDepth =>
        'model',
      MermanResourceLimitId.maxLayoutWorkUnits => 'layout',
      MermanResourceLimitId.maxSvgBytes ||
      MermanResourceLimitId.maxSvgElements =>
        'svg',
    };

Map<String, Object?> _clonedCatalog() =>
    jsonDecode(jsonEncode(_catalog())) as Map<String, Object?>;

Map<String, Object?> _resources(Map<String, Object?> catalog) =>
    catalog['resources']! as Map<String, Object?>;

Map<String, Object?> _limitAt(Map<String, Object?> catalog, int index) =>
    (_resources(catalog)['limits'] as List<Object?>)[index]
        as Map<String, Object?>;

Map<String, Object?> _profileAt(Map<String, Object?> catalog, int index) =>
    (_resources(catalog)['profiles'] as List<Object?>)[index]
        as Map<String, Object?>;

Map<String, Object?> _runtimeCapabilities(Map<String, Object?> catalog) =>
    catalog['capabilities']! as Map<String, Object?>;

List<Object?> _outputContracts(Map<String, Object?> catalog) =>
    catalog['output_contracts']! as List<Object?>;

Map<String, Object?> _outputContractAt(
  Map<String, Object?> catalog,
  String outputId,
) =>
    _outputContracts(catalog)
        .cast<Map<String, Object?>>()
        .firstWhere((contract) => contract['id'] == outputId);

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
