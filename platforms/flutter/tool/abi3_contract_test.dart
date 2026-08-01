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
  decodesTypedMetadataCatalogs();
  acceptsAdditiveTypedMetadataFields();
  matchesThePubPackageVersionProjection();
  acceptsAFlatAbi3Catalog();
  rejectsCatalogsMissingCurrentBindingSchemas();
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
  _expect(
    native.MERMAN_NATIVE_FUNCTION_METADATA_COLLECT == 5,
    'metadata_collect must occupy the first appended function slot',
  );
  _expect(
    ffi.sizeOf<native.MermanNativeApi>() >
        native.MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE,
    'metadata_collect must remain an appended slot outside the frozen prefix',
  );
  _expect(
    !ffi_transport.nativeApiHasMetadataCollectForTesting(
          native.MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE,
        ) &&
        ffi_transport.nativeApiHasMetadataCollectForTesting(
          native.MERMAN_NATIVE_API_METADATA_COLLECT_PREFIX_SIZE,
        ),
    'consumers must not inspect metadata_collect in a five-slot producer',
  );
  _expect(
    native.MERMAN_NATIVE_API_METADATA_COLLECT_PREFIX_SIZE <=
        ffi.sizeOf<native.MermanNativeApi>(),
    'metadata_collect availability must depend on its own field boundary',
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
    _expect(
      api.ref.metadata_collect.address == 0,
      'zeroed ABI table should expose the appended metadata_collect slot',
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

void decodesTypedMetadataCatalogs() {
  final evidence = MermanAsciiCapabilityEvidence.fromJson({
    'kind': 'local_advantage',
    'source': 'fixture',
    'note': 'typed evidence',
  });
  final ascii = MermanAsciiCapability.fromJson({
    'diagram_type': 'flowchart-v2',
    'display_name': 'Flowchart',
    'support_level': 'full',
    'summary_fallback': false,
    'supported_semantics': ['nodes', 'edges'],
    'limits': ['html-labels'],
    'evidence': [
      {
        'kind': evidence.kind,
        'source': evidence.source,
        'note': evidence.note,
      },
    ],
  });
  final family = MermanDiagramFamilyCapability.fromJson({
    'diagram_type': 'flowchart-v2',
    'logical_family_kind': 'flowchart',
    'metadata_id': 'flowchart',
    'render_model_kind': 'flowchart',
    'has_detector': true,
    'has_semantic_parser': true,
    'has_editor_parser': true,
    'has_combined_parser': true,
    'has_render_parser': true,
    'has_header': true,
    'config_namespace': 'flowchart',
  });
  final rule = MermanLintRuleCatalogEntry.fromJson({
    'id': 'parse-error',
    'description': 'Reports parser failures.',
    'evidence': ['ADR-0070'],
    'default_severity': 'error',
    'category': 'parse',
    'default_enabled': true,
    'default_profile': 'recommended',
    'origin': 'merman',
    'configurable': false,
    'fixable': false,
  });

  _expect(
    ascii.evidence.single.note == 'typed evidence' &&
        ascii.supportedSemantics.length == 2 &&
        family.metadataId == 'flowchart' &&
        family.logicalFamilyKind == 'flowchart' &&
        family.renderModelKind == 'flowchart' &&
        family.hasDetector &&
        family.hasSemanticParser &&
        family.hasEditorParser &&
        family.hasCombinedParser &&
        family.hasHeader &&
        family.configNamespace == 'flowchart' &&
        rule.id == 'parse-error' &&
        rule.evidence.single == 'ADR-0070',
    'typed metadata records must preserve their public contract',
  );
}

void acceptsAdditiveTypedMetadataFields() {
  final ascii = MermanAsciiCapability.fromJson({
    'diagram_type': 'flowchart-v2',
    'display_name': 'Flowchart',
    'support_level': 'full',
    'summary_fallback': false,
    'supported_semantics': <String>[],
    'limits': <String>[],
    'evidence': <Object?>[],
    'future_field': {'nested': true},
  });
  final family = MermanDiagramFamilyCapability.fromJson({
    'diagram_type': 'flowchart-v2',
    'logical_family_kind': 'flowchart',
    'metadata_id': null,
    'render_model_kind': null,
    'has_detector': true,
    'has_semantic_parser': true,
    'has_editor_parser': true,
    'has_combined_parser': true,
    'has_render_parser': false,
    'has_header': true,
    'config_namespace': null,
    'future_field': 1,
  });

  _expect(
    ascii.diagramType == 'flowchart-v2' && family.metadataId == null,
    'typed metadata decoders must ignore additive JSON fields',
  );
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
  _expect(
    catalog.optionsSchemaVersions.single == 2 &&
        catalog.payloadSchemas.any(
          (schema) => schema.id == 'binding-result' && schema.version == 1,
        ) &&
        catalog.metadataIds.contains('diagram-family-capabilities') &&
        catalog.resourceLimitsById['max_source_bytes']!.operationIds
            .contains('semantic-json'),
    'runtime discovery additions should retain their typed values',
  );
}

void rejectsCatalogsMissingCurrentBindingSchemas() {
  final legacyOptions = _catalog();
  legacyOptions['options_schema_versions'] = [1];
  _expectContractFailure(
    () => MermanRuntimeCatalog.fromJson(legacyOptions)
        .requireCurrentBindingSchemas(),
  );

  final missingResult = _catalog();
  missingResult['payload_schemas'] = const [
    {'id': 'operation-metadata', 'version': 1},
  ];
  _expectContractFailure(
    () => MermanRuntimeCatalog.fromJson(missingResult)
        .requireCurrentBindingSchemas(),
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
        catalog.resourceLimits.every((limit) => limit.hardCap
            ? unbounded.limits[limit.id] != null
            : unbounded.limits[limit.id] == null),
    'runtime nullable policy limits and finite hard caps must survive',
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
        futureLimit.minimumValue == 1 &&
        validated.resourceProfiles.every(
          (profile) =>
              profile.limits.containsKey('future_limit') &&
              profile.limits['future_limit'] == 4096,
        ),
    'ABI 3 consumers must retain additive declared resource IDs',
  );
}

void acceptsInvariantOnlyCatalog() {
  final catalog = _catalog(
    capabilityIds: const [],
    outputIds: const [],
    operationIds: const ['semantic-json'],
  );

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

  final resource = MermanException.fromNative(
    native.MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED,
    Uint8List.fromList(
      utf8.encode(
        jsonEncode({
          'version': 1,
          'ok': false,
          'status': native.MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED,
          'status_name': 'resource-limit-exceeded',
          'kind': 'generic',
          'capability_id': null,
          'details': {
            'resource': {
              'limit_id': 'max_embedded_image_bytes',
              'phase': 'embedded_image_decode',
              'actual': 5,
              'max': 4,
              'profile': 'constrained',
            },
          },
          'message': 'embedded image is too large',
        }),
      ),
    ),
  );
  _expect(
    resource.resourceDetails?.limitId == 'max_embedded_image_bytes' &&
        resource.resourceDetails?.phase == 'embedded_image_decode' &&
        resource.resourceDetails?.actual == 5 &&
        resource.resourceDetails?.max == 4 &&
        resource.resourceDetails?.profile == 'constrained',
    'resource metadata should survive the Dart boundary',
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
    'options_schema_versions': const [2],
    'payload_schemas': const [
      {'id': 'binding-result', 'version': 1},
    ],
    'metadata_ids': const [
      'ascii-capabilities',
      'diagram-family-capabilities',
      'lint-rule-catalog',
      'supported-diagrams',
      'supported-host-theme-presets',
      'supported-themes',
    ],
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
    'resources': _resourceContract(operationIds),
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
        'caller_configurable': true,
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

Map<String, Object?> _resourceContract(List<String> operationIds) =>
    <String, Object?>{
      'general_binding_default_profile': MermanResourceProfile.interactive.id,
      'cli_default_profile': MermanResourceProfile.trustedNative.id,
      'limits': <Object?>[
        for (final limit in MermanResourceLimitId.values)
          <String, Object?>{
            'id': limit.id,
            'phase': _resourceLimitPhase(limit),
            'description': 'Test descriptor for ${limit.id}',
            'overridable': limit.overridable,
            'hard_cap': !limit.overridable,
            'minimum_value': limit.minimumValue,
            'operation_ids': operationIds,
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
                    profile == MermanResourceProfile.unboundedForTrustedInput &&
                            limit.overridable
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
      MermanResourceLimitId.maxDocumentDiagrams => 'document_scan',
      MermanResourceLimitId.maxAsciiGridCells => 'ascii_layout',
      MermanResourceLimitId.maxRasterWidth ||
      MermanResourceLimitId.maxRasterHeight ||
      MermanResourceLimitId.maxRasterPixels =>
        'raster_allocation',
      MermanResourceLimitId.maxEmbeddedImageBytes ||
      MermanResourceLimitId.maxTotalEmbeddedImageBytes ||
      MermanResourceLimitId.maxEmbeddedImagePixels ||
      MermanResourceLimitId.maxTotalEmbeddedImagePixels =>
        'embedded_image_decode',
      MermanResourceLimitId.maxPdfFilterImagePixels =>
        'pdf_filter_rasterization',
      MermanResourceLimitId.maxSvgConversionIsolationDepth ||
      MermanResourceLimitId.maxSvgConversionFilterPrimitivesPerFilter ||
      MermanResourceLimitId.maxTotalSvgConversionFilterPrimitives ||
      MermanResourceLimitId.maxSvgConversionSubroots ||
      MermanResourceLimitId.maxNestedSvgImages ||
      MermanResourceLimitId.svgBackendTreeNodes =>
        'svg_conversion',
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
