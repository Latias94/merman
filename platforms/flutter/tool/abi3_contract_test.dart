import 'dart:convert';
import 'dart:ffi' as ffi;
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';
import 'package:merman/merman.dart';
import 'package:merman/src/generated/binding_contract.dart' as binding;
import 'package:merman/src/generated/native_abi.dart' as native;
import 'package:merman/src/merman_ffi.dart' as ffi_transport;

void main() {
  projectsCurrentAbi3TableBoundaries();
  decodesTypedMetadataCatalogs();
  acceptsAdditiveTypedMetadataFields();
  matchesThePubPackageVersionProjection();
  acceptsAFlatAbi3Catalog();
  acceptsAdditiveConstructorResourceLimits();
  rejectsMalformedAdditiveTransportSections();
  acceptsSchemaOneCatalogWithoutAdditiveTransportSections();
  normalizesEmptyIconRegistryToNoService();
  rejectsUnboundedIconRegistryStaging();
  rejectsMalformedIconRegistryUtf16();
  rejectsCatalogsMissingCurrentBindingSchemas();
  decodesTypedOperationMetadata();
  projectsSvgPlanOperationFromGeneratedAbi();
  requiresSdkUpgradeForUnknownCatalogOperation();
  preservesTypedRuntimeOutputContracts();
  preservesCompleteRuntimeResourceContract();
  acceptsInvariantOnlyCatalog();
  acceptsAdditiveRuntimeCatalogFields();
  acceptsAdditiveRuntimeResourceIds();
  rejectsMissingRuntimeOutputContracts();
  rejectsOutputContractIdDrift();
  rejectsMalformedOutputContracts();
  rejectsMissingOriginalSchemaOneFields();
  rejectsInconsistentGeneratedCatalogRelations();
  enforcesRuntimeIdentifierGrammarWithoutClosingFutureVocabularies();
  rejectsDuplicateCapabilityIds();
  rejectsUncallableOutputIds();
  rejectsInconsistentAdapters();
  rejectsCoercedRuntimeCatalogVersionFields();
  rejectsInconsistentTextMeasurement();
  rejectsTextMeasurementWithoutVendoredProvider();
  rejectsMalformedResourceDescriptors();
  textMeasurementFactoriesRejectMalformedValues();
  decodesMachineReadableNativeErrors();
  rejectsInconsistentNativeErrorRelations();
  rejectsMalformedNativeDiagnosticDetails();
  rejectsMismatchedNativeErrorSchema();
  preservesAllocationTokenExhaustionStatus();
  preservesUnpublishedEngineProducerProvenance();
  poisonsUnpublishedEnginesWithoutReleasingCallbacks();
  retainsEveryUnpublishedEngineEntry();
  fuzzesNativeErrorPayloadDecoding();
  print('ABI 3 Dart contract tests passed');
}

void preservesUnpublishedEngineProducerProvenance() {
  const producerA = 0xa1;
  const producerB = 0xb1;
  const collidingToken = 5;
  final harness = ffi_transport.UnpublishedEngineQuarantineTestHarness();
  final retryStatuses = <int>[
    native.MERMAN_NATIVE_STATUS_BUSY,
    native.MERMAN_NATIVE_STATUS_OK,
  ];
  final closedTokens = <int>[];
  var releasedCallbacks = 0;
  harness.retain(
    producerIdentity: producerA,
    token: collidingToken,
    closeStatus: native.MERMAN_NATIVE_STATUS_BUSY,
    close: (token) {
      closedTokens.add(token);
      return retryStatuses.removeAt(0);
    },
    releaseCallbackState: () => releasedCallbacks += 1,
  );

  final otherProducer = harness.sweepFor(producerB);
  _expect(
    !otherProducer.blocked &&
        otherProducer.producerEntryCount == 0 &&
        otherProducer.totalEntryCount == 1 &&
        closedTokens.length == 1 &&
        closedTokens.single == collidingToken &&
        releasedCallbacks == 0,
    'another producer must retry a quarantined token only through its '
    'originating close function and must remain unblocked while that close is busy',
  );

  final originatingProducer = harness.sweepFor(producerA);
  _expect(
    !originatingProducer.blocked &&
        originatingProducer.totalEntryCount == 0 &&
        closedTokens.length == 2 &&
        closedTokens.every((token) => token == collidingToken) &&
        releasedCallbacks == 1,
    'the originating producer must release callback state exactly after its '
    'own close function confirms quiescence',
  );

  final repeatedSweep = harness.sweepFor(producerA);
  _expect(
    !repeatedSweep.blocked &&
        closedTokens.length == 2 &&
        releasedCallbacks == 1,
    'a retired quarantine entry must not be closed or released twice',
  );
}

void poisonsUnpublishedEnginesWithoutReleasingCallbacks() {
  for (final terminalStatus in const [
    native.MERMAN_NATIVE_STATUS_INVALID_ENGINE,
    native.MERMAN_NATIVE_STATUS_INTERNAL_ERROR,
  ]) {
    final harness = ffi_transport.UnpublishedEngineQuarantineTestHarness();
    final producer = 0xc0 + terminalStatus;
    final otherProducer = producer + 0x100;
    var closeCalls = 0;
    var releasedCallbacks = 0;
    harness.retain(
      producerIdentity: producer,
      token: 9,
      closeStatus: native.MERMAN_NATIVE_STATUS_REENTRANT_CALL,
      close: (token) {
        closeCalls += 1;
        _expect(token == 9, 'quarantine retried an unexpected engine token');
        return terminalStatus;
      },
      releaseCallbackState: () => releasedCallbacks += 1,
    );

    final unrelated = harness.sweepFor(otherProducer);
    _expect(
      !unrelated.blocked &&
          unrelated.totalEntryCount == 1 &&
          closeCalls == 1 &&
          releasedCallbacks == 0,
      'a terminal rollback status must retain callback state without blocking '
      'an unrelated native producer',
    );

    final owner = harness.sweepFor(producer);
    _expect(
      owner.blocked &&
          owner.poisoned &&
          owner.status == terminalStatus &&
          owner.producerEntryCount == 1 &&
          closeCalls == 1 &&
          releasedCallbacks == 0,
      'INVALID_ENGINE and unexpected close statuses must poison only the '
      'originating producer and must never establish callback quiescence',
    );

    final repeatedOwnerSweep = harness.sweepFor(producer);
    _expect(
      repeatedOwnerSweep.blocked &&
          repeatedOwnerSweep.poisoned &&
          closeCalls == 1 &&
          releasedCallbacks == 0,
      'a poisoned quarantine entry must not retry close or release callback state',
    );

    final directlyPoisoned =
        ffi_transport.UnpublishedEngineQuarantineTestHarness();
    var directCloseCalls = 0;
    var directReleases = 0;
    directlyPoisoned.retain(
      producerIdentity: producer,
      token: 21,
      closeStatus: terminalStatus,
      close: (token) {
        directCloseCalls += 1;
        return native.MERMAN_NATIVE_STATUS_OK;
      },
      releaseCallbackState: () => directReleases += 1,
    );
    final directOwner = directlyPoisoned.sweepFor(producer);
    _expect(
      directOwner.blocked &&
          directOwner.poisoned &&
          directOwner.status == terminalStatus &&
          directCloseCalls == 0 &&
          directReleases == 0,
      'a terminal initial rollback status must enter poison state without '
      'being retried or releasing callback ownership',
    );
  }
}

void retainsEveryUnpublishedEngineEntry() {
  const producer = 0xd1;
  final harness = ffi_transport.UnpublishedEngineQuarantineTestHarness();
  final closedTokens = <int>[];
  var releasedCallbacks = 0;
  for (final entry in const [
    (token: 13, status: native.MERMAN_NATIVE_STATUS_BUSY),
    (token: 17, status: native.MERMAN_NATIVE_STATUS_REENTRANT_CALL),
  ]) {
    harness.retain(
      producerIdentity: producer,
      token: entry.token,
      closeStatus: entry.status,
      close: (token) {
        closedTokens.add(token);
        return native.MERMAN_NATIVE_STATUS_OK;
      },
      releaseCallbackState: () => releasedCallbacks += 1,
    );
  }

  final state = harness.sweepFor(producer);
  _expect(
    !state.blocked &&
        state.totalEntryCount == 0 &&
        closedTokens.length == 2 &&
        closedTokens.toSet().containsAll(const [13, 17]) &&
        releasedCallbacks == 2,
    'quarantine must retain every published token and treat BUSY and '
    'REENTRANT close failures as independently retryable',
  );
}

void normalizesEmptyIconRegistryToNoService() {
  final packSet = MermanIconPackSet.fromPacks(const []);
  final services = MermanEngineServices(iconPackSet: packSet);
  _expect(
    packSet.isEmpty && services.isEmpty && !services.hasIconPacks,
    'an empty icon pack set must normalize to an empty constructor service',
  );
}

void projectsCurrentAbi3TableBoundaries() {
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
    'ABI 3 base slots must retain their descriptor-owned order',
  );
  _expect(
    native.MERMAN_NATIVE_FUNCTION_METADATA_COLLECT == 5,
    'metadata_collect must occupy the first appended function slot',
  );
  _expect(
    native.MERMAN_NATIVE_FUNCTION_ENGINE_NEW_WITH_SERVICES == 6,
    'engine_new_with_services must append after metadata_collect',
  );
  _expect(
    native.MERMAN_NATIVE_FUNCTION_OPERATION_CONTROL_NEW == 7 &&
        native.MERMAN_NATIVE_FUNCTION_OPERATION_CONTROL_CANCEL == 8 &&
        native.MERMAN_NATIVE_FUNCTION_OPERATION_CONTROL_RELEASE == 9 &&
        native.MERMAN_NATIVE_FUNCTION_EXECUTE_COLLECT_CONTROLLED == 10,
    'operation-control functions and controlled execution must retain their appended ABI 3 slots',
  );
  _expect(
    native.MERMAN_NATIVE_STATUS_BUSY == 16 &&
        native.MERMAN_NATIVE_STATUS_CANCELLED == 17,
    'cancelled must append after every pre-existing ABI 3 status',
  );
  _expect(
    !ffi_transport.nativeApiHasCurrentTableForTesting(
          native.MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE - 1,
        ) &&
        ffi_transport.nativeApiHasCurrentTableForTesting(
          native.MERMAN_NATIVE_API_EXECUTE_COLLECT_CONTROLLED_PREFIX_SIZE,
        ),
    'consumers must require the complete controlled-execution table prefix',
  );
  _expect(
    native.MERMAN_NATIVE_API_MINIMUM_PREFIX_SIZE <
            native.MERMAN_NATIVE_API_OPERATION_CONTROL_NEW_PREFIX_SIZE &&
        native.MERMAN_NATIVE_API_OPERATION_CONTROL_NEW_PREFIX_SIZE <
            native.MERMAN_NATIVE_API_OPERATION_CONTROL_CANCEL_PREFIX_SIZE &&
        native.MERMAN_NATIVE_API_OPERATION_CONTROL_CANCEL_PREFIX_SIZE <
            native.MERMAN_NATIVE_API_OPERATION_CONTROL_RELEASE_PREFIX_SIZE &&
        native.MERMAN_NATIVE_API_OPERATION_CONTROL_RELEASE_PREFIX_SIZE <
            native.MERMAN_NATIVE_API_EXECUTE_COLLECT_CONTROLLED_PREFIX_SIZE &&
        native.MERMAN_NATIVE_API_EXECUTE_COLLECT_CONTROLLED_PREFIX_SIZE ==
            ffi.sizeOf<native.MermanNativeApi>(),
    'each appended control or execution slot must end at one complete table prefix',
  );

  final request = calloc<native.MermanNativeApiRequest>();
  final api = calloc<native.MermanNativeApi>();
  final result = calloc<native.MermanNativeResult>();
  try {
    request.ref.expected_minimum_prefix_layout_digest.struct_size = ffi
        .sizeOf<native.MermanNativeSlice>();
    _expect(
      api.ref.engine_try_close.address == 0,
      'zeroed ABI table should expose the engine_try_close slot',
    );
    _expect(
      api.ref.metadata_collect.address == 0,
      'zeroed ABI table should expose the appended metadata_collect slot',
    );
    _expect(
      api.ref.engine_new_with_services.address == 0,
      'zeroed ABI table should expose the appended service constructor slot',
    );
    final iconPack = calloc<native.MermanNativeIconPack>();
    final servicesConfig = calloc<native.MermanNativeEngineServicesConfig>();
    try {
      iconPack.ref.struct_size = ffi.sizeOf<native.MermanNativeIconPack>();
      servicesConfig.ref.struct_size = ffi
          .sizeOf<native.MermanNativeEngineServicesConfig>();
      _expect(
        iconPack.ref.struct_size > 0 && servicesConfig.ref.struct_size > 0,
        'generated Dart ABI must project both service-constructor records',
      );
    } finally {
      calloc.free(iconPack);
      calloc.free(servicesConfig);
    }
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
    'semantic_coverage': 'partial',
    'primary_projection': 'diagrammatic',
    'structured_text_fallback': false,
    'support_level': 'partial',
    'supported_semantics': ['nodes', 'edges'],
    'limits': ['html-labels'],
    'evidence': [
      {'kind': evidence.kind, 'source': evidence.source, 'note': evidence.note},
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
    'tags': ['deprecated'],
    'default_enabled': true,
    'default_profile': 'recommended',
    'origin': 'merman',
    'configurable': false,
    'fixable': false,
  });
  final legacyRuleWithoutTags = MermanLintRuleCatalogEntry.fromJson({
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
  final presentation = MermanPresentationCatalog.fromJson({
    'schema_version': 1,
    'theme_presets': [
      {
        'id': 'one-dark',
        'appearance': 'dark',
        'fully_available': true,
        'missing_capability_ids': <String>[],
      },
    ],
    'profiles': [
      {
        'id': 'merman-modern',
        'fully_available': false,
        'missing_capability_ids': ['layout-elk'],
        'aspects': [
          {
            'id': 'flowchart-routing',
            'applicability': {'kind': 'family', 'family_id': 'flowchart'},
            'required_capability_id': 'layout-elk',
            'available': false,
            'missing_capability_ids': ['layout-elk'],
          },
        ],
      },
    ],
  });

  _expect(
    ascii.evidence.single.note == 'typed evidence' &&
        ascii.supportedSemantics.length == 2 &&
        ascii.semanticCoverage == 'partial' &&
        ascii.primaryProjection == 'diagrammatic' &&
        !ascii.structuredTextFallback &&
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
        rule.evidence.single == 'ADR-0070' &&
        rule.tags.single == 'deprecated' &&
        legacyRuleWithoutTags.tags.isEmpty &&
        presentation.themePresets.single.id == 'one-dark' &&
        presentation.profiles.single.aspects.single.applicability.familyId ==
            'flowchart' &&
        presentation.profiles.single.missingCapabilityIds.single ==
            'layout-elk',
    'typed metadata records must preserve their public contract',
  );
}

void acceptsAdditiveTypedMetadataFields() {
  final ascii = MermanAsciiCapability.fromJson({
    'diagram_type': 'flowchart-v2',
    'display_name': 'Flowchart',
    'semantic_coverage': 'partial',
    'primary_projection': 'diagrammatic',
    'structured_text_fallback': false,
    'support_level': 'partial',
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
  final presentation = MermanPresentationCatalog.fromJson({
    'schema_version': 1,
    'theme_presets': [
      {
        'id': 'future-theme',
        'appearance': 'adaptive',
        'fully_available': true,
        'missing_capability_ids': <String>[],
        'future_field': true,
      },
    ],
    'profiles': [
      {
        'id': 'future-profile',
        'fully_available': true,
        'missing_capability_ids': <String>[],
        'aspects': [
          {
            'id': 'future-aspect',
            'applicability': {
              'kind': 'future-scope',
              'family_id': null,
              'future_field': 1,
            },
            'required_capability_id': null,
            'available': true,
            'missing_capability_ids': <String>[],
            'future_field': 1,
          },
        ],
        'future_field': 1,
      },
    ],
    'future_field': 1,
  });

  _expect(
    ascii.diagramType == 'flowchart-v2' &&
        family.metadataId == null &&
        presentation.themePresets.single.appearance == 'adaptive' &&
        presentation.profiles.single.aspects.single.applicability.kind ==
            'future-scope',
    'typed metadata decoders must ignore additive JSON fields',
  );
}

void matchesThePubPackageVersionProjection() {
  final pubspec = File('pubspec.yaml').readAsStringSync();
  final match = RegExp(
    r'^version:\s*([^\s#]+)\s*$',
    multiLine: true,
  ).firstMatch(pubspec);
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
  _expect(
    MermanOperation.knownValues.every(
      (known) =>
          identical(MermanOperation.fromNativeCode(known.nativeCode), known) &&
          identical(MermanOperation.fromOperationId(known.operationId), known),
    ),
    'every generated operation must round-trip by numeric code and public ID',
  );
  final maxKnownCode = MermanOperation.knownValues
      .map((known) => known.nativeCode)
      .reduce(max);
  _expectThrows<ArgumentError>(
    () => MermanOperation.fromNativeCode(native.MERMAN_NATIVE_OPERATION_NONE),
  );
  _expectThrows<ArgumentError>(
    () => MermanOperation.fromNativeCode(maxKnownCode + 1),
  );
}

void requiresSdkUpgradeForUnknownCatalogOperation() {
  final catalog = _catalog(
    operationIds: const ['future-operation', 'semantic-json', 'svg'],
  );
  final validated = MermanRuntimeCatalog.fromJson(catalog);
  _expect(
    validated.supportsOperation('future-operation'),
    'runtime discovery must preserve unknown future operation IDs',
  );
  try {
    MermanOperation.fromOperationId('future-operation');
  } on UnsupportedError catch (error) {
    _expect(
      error.message.toString().contains('updated Merman SDK/header'),
      'unknown operation invocation must require an SDK/header upgrade',
    );
    return;
  }
  throw StateError('unknown catalog operation unexpectedly became callable');
}

void acceptsAFlatAbi3Catalog() {
  final catalog = MermanRuntimeCatalog.fromJson(_catalog());
  _expect(
    catalog.packageVersion == 'test',
    'package version should be preserved',
  );
  _expect(
    catalog.supportsCapability('svg'),
    'SVG capability should be present',
  );
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
        catalog
            .resourceLimitById('max_source_bytes')!
            .operationIds
            .contains('semantic-json'),
    'runtime discovery additions should retain their typed values',
  );
}

void acceptsAdditiveConstructorResourceLimits() {
  final catalog = _catalog();
  final iconRegistry = _constructorServiceContract(catalog, 'icon-registry');
  final limits = iconRegistry['resource_limits'] as List<Object?>;
  limits.insert(0, <String, Object?>{
    'id': 'future_constructor_limit',
    'phase': 'future_constructor_phase',
    'unit': 'items',
    'description': 'Future additive constructor resource limit',
    'value': 7,
    'future_limit_metadata': <String, Object?>{'version': 2},
  });

  final validated = MermanRuntimeCatalog.fromJson(catalog);
  final contract = validated.constructorServiceContracts.firstWhere(
    (candidate) => candidate.id == 'icon-registry',
  );
  final future = contract.resourceLimits.firstWhere(
    (candidate) => candidate.id == 'future_constructor_limit',
  );
  final rawContracts =
      validated.jsonObject['constructor_service_contracts'] as List<Object?>;
  final rawIconRegistry = rawContracts.cast<Map<String, Object?>>().firstWhere(
    (candidate) => candidate['id'] == 'icon-registry',
  );
  final rawFuture = (rawIconRegistry['resource_limits'] as List<Object?>)
      .cast<Map<String, Object?>>()
      .firstWhere((candidate) => candidate['id'] == 'future_constructor_limit');
  _expect(
    future.phase == 'future_constructor_phase' &&
        future.unit == 'items' &&
        future.value == 7 &&
        (rawFuture['future_limit_metadata']
                as Map<String, Object?>)['version'] ==
            2,
    'unknown future constructor limits must remain discoverable and preserved',
  );
}

void rejectsMalformedAdditiveTransportSections() {
  for (final mutation in <void Function(Map<String, Object?>)>[
    (catalog) => (catalog['option_group_ids'] as List<Object?>).remove('svg'),
    (catalog) =>
        (catalog['option_group_ids'] as List<Object?>).add('not-valid!'),
    (catalog) => catalog.remove('constructor_service_contracts'),
    (catalog) => (catalog['constructor_service_contracts'] as List<Object?>)
        .removeLast(),
    (catalog) =>
        (((catalog['constructor_service_contracts'] as List<Object?>).first
                    as Map<
                      String,
                      Object?
                    >)['provided_text_measurement_provider_ids']
                as List<Object?>)
            .add('unavailable-provider'),
    (catalog) =>
        ((((catalog['constructor_service_contracts'] as List<Object?>).last
                            as Map<String, Object?>)['resource_limits']
                        as List<Object?>)
                    .first
                as Map<String, Object?>)['value'] =
            -1,
    (catalog) {
      final iconRegistry = _constructorServiceContract(
        catalog,
        'icon-registry',
      );
      final first =
          (iconRegistry['resource_limits'] as List<Object?>).first
              as Map<String, Object?>;
      first['value'] = (first['value'] as int) + 1;
    },
    (catalog) {
      final hostText = _constructorServiceContract(
        catalog,
        'host-text-measurement',
      );
      final iconRegistry = _constructorServiceContract(
        catalog,
        'icon-registry',
      );
      final knownIconLimit = Map<String, Object?>.from(
        (iconRegistry['resource_limits'] as List<Object?>).first
            as Map<String, Object?>,
      );
      (hostText['resource_limits'] as List<Object?>).add(knownIconLimit);
    },
  ]) {
    final catalog = _catalog();
    mutation(catalog);
    _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
  }

  final noSvg = _catalog(
    capabilityIds: const [],
    outputIds: const [],
    operationIds: const ['semantic-json'],
  );
  noSvg['constructor_service_ids'] = const ['icon-registry'];
  noSvg['constructor_service_contracts'] = const [
    {
      'id': 'icon-registry',
      'provided_text_measurement_provider_ids': <String>[],
      'resource_limits': <Object?>[],
    },
  ];
  _expectContractFailure(() => MermanRuntimeCatalog.fromJson(noSvg));
}

void acceptsSchemaOneCatalogWithoutAdditiveTransportSections() {
  final catalog = _clonedCatalog()
    ..remove('option_group_ids')
    ..remove('constructor_service_ids')
    ..remove('constructor_service_contracts');
  final parsed = MermanRuntimeCatalog.fromJson(catalog);
  _expect(
    parsed.optionGroupIds.isEmpty && parsed.constructorServiceIds.isEmpty,
    'schema-1 catalogs without additive transport sections must normalize to empty exposure',
  );
  _expectContractFailure(parsed.requireCurrentBindingSchemas);
}

void rejectsUnboundedIconRegistryStaging() {
  var iterated = 0;
  Iterable<MermanIconPack> tooManyPacks() sync* {
    for (var index = 0; index < 100; index += 1) {
      iterated += 1;
      yield MermanIconPack(json: '{"prefix":"p$index","icons":{}}');
    }
  }

  final countError = _expectMermanException(
    () => MermanIconPackSet.fromPacks(tooManyPacks()),
  );
  _expect(
    iterated == 17 &&
        countError.code ==
            native.MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED &&
        countError.resourceDetails?.limitId.id == 'max_icon_registry_packs' &&
        countError.resourceDetails?.actual == 17 &&
        countError.iconRegistryDetails?.kindId == 'resource_limit_exceeded',
    'icon registry factory must stop at limit plus one before unbounded staging',
  );

  final prefixError = _expectMermanException(
    () => MermanIconPackSet.fromPacks([
      MermanIconPack(
        json: '{"prefix":"test","icons":{}}',
        registrationName: 'a' * 65,
      ),
    ]),
  );
  _expect(
    prefixError.resourceDetails?.limitId.id ==
            'max_icon_registry_prefix_bytes' &&
        prefixError.resourceDetails?.actual == 65 &&
        prefixError.iconRegistryDetails?.packIndex == 0 &&
        prefixError.iconRegistryDetails?.registrationName == null,
    'icon registry factory must enforce exact UTF-8 registration-name bytes',
  );

  final overLimitMalformedPrefix = _expectMermanException(
    () => MermanIconPackSet.fromPacks([
      MermanIconPack(
        json: '{"prefix":"test","icons":{}}',
        registrationName: '${'a' * 64}${String.fromCharCode(0xd800)}',
      ),
    ]),
  );
  _expect(
    overLimitMalformedPrefix.code ==
            native.MERMAN_NATIVE_STATUS_RESOURCE_LIMIT_EXCEEDED &&
        overLimitMalformedPrefix.resourceDetails?.limitId.id ==
            'max_icon_registry_prefix_bytes' &&
        overLimitMalformedPrefix.resourceDetails?.actual == 67 &&
        overLimitMalformedPrefix.iconRegistryDetails?.registrationName == null,
    'registration-name byte limits must win before malformed UTF-16 decoding',
  );
}

void rejectsMalformedIconRegistryUtf16() {
  final isolatedHighSurrogate = String.fromCharCode(0xd800);
  final isolatedLowSurrogate = String.fromCharCode(0xdc00);

  final jsonError = _expectMermanException(
    () => MermanIconPackSet.fromPacks([
      MermanIconPack(
        json:
            '{"prefix":"test","icons":{"broken":{"body":"$isolatedHighSurrogate"}}}',
        registrationName: 'smoke',
      ),
    ]),
  );
  _expect(
    jsonError.code == native.MERMAN_NATIVE_STATUS_UTF8_ERROR &&
        jsonError.codeName == 'utf8-error' &&
        jsonError.resourceDetails == null &&
        jsonError.iconRegistryDetails?.kindId == 'invalid_utf8' &&
        jsonError.iconRegistryDetails?.packIndex == 0 &&
        jsonError.iconRegistryDetails?.registrationName == 'smoke',
    'isolated JSON surrogates must fail with the native invalid-UTF-8 contract',
  );

  final registrationNameError = _expectMermanException(
    () => MermanIconPackSet.fromPacks([
      MermanIconPack(
        json: '{"prefix":"test","icons":{}}',
        registrationName: 'bad$isolatedLowSurrogate',
      ),
    ]),
  );
  _expect(
    registrationNameError.code == native.MERMAN_NATIVE_STATUS_UTF8_ERROR &&
        registrationNameError.codeName == 'utf8-error' &&
        registrationNameError.iconRegistryDetails?.kindId == 'invalid_utf8' &&
        registrationNameError.iconRegistryDetails?.packIndex == 0 &&
        registrationNameError.iconRegistryDetails?.registrationName == null,
    'isolated registration-name surrogates must not be echoed in error details',
  );

  final validPair = String.fromCharCodes(const [0xd83d, 0xde00]);
  final packSet = MermanIconPackSet.fromPacks([
    MermanIconPack(
      json:
          '{"prefix":"test","icons":{"valid":{"body":"<text>$validPair</text>"}}}',
    ),
  ]);
  _expect(
    packSet.length == 1,
    'valid UTF-16 surrogate pairs must remain admissible UTF-8 input',
  );
}

void rejectsCatalogsMissingCurrentBindingSchemas() {
  final legacyOptions = _catalog();
  legacyOptions['options_schema_versions'] = [1];
  _expectContractFailure(
    () => MermanRuntimeCatalog.fromJson(
      legacyOptions,
    ).requireCurrentBindingSchemas(),
  );

  final missingResult = _catalog();
  missingResult['payload_schemas'] = const [
    {'id': 'operation-metadata', 'version': 1},
  ];
  _expectContractFailure(
    () => MermanRuntimeCatalog.fromJson(
      missingResult,
    ).requireCurrentBindingSchemas(),
  );

  final missingMetadata = _catalog();
  missingMetadata['payload_schemas'] = const [
    {'id': 'binding-result', 'version': 1},
  ];
  _expectContractFailure(
    () => MermanRuntimeCatalog.fromJson(
      missingMetadata,
    ).requireCurrentBindingSchemas(),
  );
}

void decodesTypedOperationMetadata() {
  final raster = binding.decodeMermanOperationMetadata(
    jsonEncode({
      'version': 1,
      'operation_id': 'png',
      'media_type': 'image/png',
      'runtime_policy': 'deterministic',
      'byte_length': 128,
      'output_plan': {
        'kind': 'raster',
        'requested_width_px': 100.5,
        'requested_height_px': 50.25,
        'width_px': 100,
        'height_px': 50,
        'requested_scale': 1.0,
        'effective_scale': 0.5,
        'limited': true,
      },
      'future_metadata': {'preserved': true},
    }),
  );
  final rasterPlan = raster.outputPlan;
  _expect(
    rasterPlan is MermanRasterOutputPlan &&
        rasterPlan.requestedWidthPx == 100.5 &&
        rasterPlan.widthPx == 100 &&
        rasterPlan.limited &&
        raster.jsonObject['future_metadata'] is Map,
    'generated metadata decoder must project raster plans and preserve raw JSON',
  );

  final pdf = binding.decodeMermanOperationMetadata(
    jsonEncode({
      'version': 1,
      'operation_id': 'pdf',
      'media_type': 'application/pdf',
      'runtime_policy': 'deterministic',
      'byte_length': 256,
      'output_plan': {
        'kind': 'pdf-filter-images',
        'filtered_groups': 2,
        'requested_scale': 1.0,
        'effective_scale': 0.75,
        'requested_image_pixels': 1000,
        'effective_image_pixels': 750,
        'limited': true,
      },
    }),
  );
  _expect(
    pdf.outputPlan is MermanPdfFilterImagesOutputPlan &&
        (pdf.outputPlan! as MermanPdfFilterImagesOutputPlan).filteredGroups ==
            2,
    'generated metadata decoder must project PDF filter-image plans',
  );

  final unknown = binding.decodeMermanOperationMetadata(
    jsonEncode({
      'version': 1,
      'operation_id': 'future',
      'media_type': 'application/x-future',
      'runtime_policy': 'future-policy',
      'byte_length': 7,
      'output_plan': {
        'kind': 'future-plan',
        'nested': {'answer': 42},
      },
    }),
  );
  _expect(
    unknown.outputPlan is MermanUnknownOutputPlan &&
        (unknown.outputPlan! as MermanUnknownOutputPlan).jsonObject['nested']
            is Map,
    'unknown future output plans must preserve their complete JSON object',
  );
  _expectThrows<UnsupportedError>(
    () =>
        ((unknown.outputPlan! as MermanUnknownOutputPlan).jsonObject['nested']!
                as Map<String, Object?>)['answer'] =
            0,
  );

  _expectThrows<FormatException>(
    () => binding.decodeMermanOperationMetadata(
      '{"version":1,"operation_id":"png"}',
    ),
  );
  _expectThrows<FormatException>(
    () => binding.decodeMermanOperationMetadata(
      jsonEncode({
        'version': 1,
        'operation_id': 'png',
        'media_type': 'image/png',
        'runtime_policy': 'deterministic',
        'byte_length': 1,
        'output_plan': {
          'kind': 'raster',
          'requested_width_px': 1.0,
          'requested_height_px': 1.0,
          'width_px': 0x100000000,
          'height_px': 1,
          'requested_scale': 1.0,
          'effective_scale': 1.0,
          'limited': false,
        },
      }),
    ),
  );
}

void preservesCompleteRuntimeResourceContract() {
  final catalog = MermanRuntimeCatalog.fromJson(_catalog());
  _expect(
    catalog.resourceLimits.length == MermanResourceLimitId.knownValues.length &&
        catalog.resourceProfiles.length == MermanResourceProfile.values.length,
    'runtime resource descriptors must be retained',
  );

  final sourceBytes =
      catalog.resourceLimitsById[MermanResourceLimitId.maxSourceBytes];
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
        catalog.resourceLimits.every(
          (limit) => limit.hardCap
              ? unbounded.limits[limit.id] != null
              : unbounded.limits[limit.id] == null,
        ),
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
  final preserved = validated.jsonObject;
  _expect(
    identical(preserved, validated.jsonObject),
    'runtime catalog JSON must be decoded and frozen once',
  );
  _expect(
    validated.supportsCapability('svg') &&
        preserved['future_root'] == true &&
        ((preserved['capabilities']
                    as Map<String, Object?>)['future_capability_metadata']
                as Map<String, Object?>)
            .isEmpty &&
        ((preserved['output_contracts'] as List<Object?>)
                .cast<Map<String, Object?>>()
                .firstWhere(
                  (output) => output['id'] == 'png',
                ))['future_output_metadata'] ==
            true,
    'schema 1 consumers must preserve additive catalog fields',
  );
  _expectThrows<UnsupportedError>(
    () => (preserved['capabilities'] as Map<String, Object?>)['future'] = true,
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
    'minimum_value': 0,
    'future_limit_metadata': <String, Object?>{
      'version': 2,
      'tags': <Object?>[
        'future',
        <String, Object?>{'stable': true},
      ],
    },
  });
  for (final rawProfile in resources['profiles'] as List<Object?>) {
    final profile = rawProfile as Map<String, Object?>;
    final profileLimits = profile['limits'] as Map<String, Object?>;
    profileLimits['future_limit'] = 4096;
    profile['future_profile_metadata'] = <String, Object?>{
      'source': 'future-runtime',
      'flags': <Object?>[true, null],
    };
  }

  final validated = MermanRuntimeCatalog.fromJson(catalog);
  final futureLimitId = MermanResourceLimitId.fromId('future_limit');
  final futureLimit = validated.resourceLimitsById[futureLimitId];
  _expect(futureLimit != null, 'future resource limit must be discoverable');
  final futureLimitMetadata =
      futureLimit!.additionalFields['future_limit_metadata']
          as Map<String, Object?>;
  final futureLimitTags = futureLimitMetadata['tags'] as List<Object?>;
  final futureLimitStability = futureLimitTags[1] as Map<String, Object?>;
  final futureProfile = validated.resourceProfiles.first;
  final futureProfileMetadata =
      futureProfile.additionalFields['future_profile_metadata']
          as Map<String, Object?>;
  final futureProfileFlags = futureProfileMetadata['flags'] as List<Object?>;

  final rawFutureLimit = limits.last as Map<String, Object?>;
  final rawFutureLimitMetadata =
      rawFutureLimit['future_limit_metadata'] as Map<String, Object?>;
  rawFutureLimitMetadata['version'] = 3;
  ((rawFutureLimitMetadata['tags'] as List<Object?>)[1]
          as Map<String, Object?>)['stable'] =
      false;
  final rawFirstProfile =
      (resources['profiles'] as List<Object?>).first as Map<String, Object?>;
  final rawFutureProfileMetadata =
      rawFirstProfile['future_profile_metadata'] as Map<String, Object?>;
  rawFutureProfileMetadata['source'] = 'mutated-source';

  _expect(
    futureLimit.phase == 'future_phase' &&
        futureLimit.hardCap &&
        !futureLimit.overridable &&
        futureLimit.minimumValue == 0 &&
        identical(validated.resourceLimitById('future_limit'), futureLimit) &&
        futureLimitMetadata['version'] == 2 &&
        futureLimitStability['stable'] == true &&
        futureProfileMetadata['source'] == 'future-runtime' &&
        futureProfileFlags.last == null &&
        validated.resourceProfiles.every(
          (profile) =>
              profile.limits.containsKey(futureLimitId) &&
              profile.limits[futureLimitId] == 4096,
        ),
    'ABI 3 consumers must retain additive resource IDs and metadata',
  );
  _expectThrows<UnsupportedError>(
    () => futureLimit.additionalFields['mutated'] = true,
  );
  _expectThrows<UnsupportedError>(() => futureLimitTags.add('mutated'));
  _expectThrows<UnsupportedError>(() => futureLimitStability['stable'] = false);
  _expectThrows<UnsupportedError>(
    () => futureProfileMetadata['source'] = 'mutated',
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
    (catalog) =>
        (_outputContractAt(catalog, 'png')['system_fonts']
                as Map<String, Object?>)['source_id'] =
            1,
    (catalog) =>
        (_outputContractAt(catalog, 'png')['system_fonts']
                as Map<String, Object?>)['host_dependent'] =
            'true',
    (catalog) =>
        (_outputContractAt(catalog, 'png')['embedded_images']
                as Map<String, Object?>)['source_ids'] =
            'data-url',
    (catalog) =>
        (_outputContractAt(catalog, 'png')['embedded_images']
                as Map<String, Object?>)['filesystem_access'] =
            1,
    (catalog) =>
        ((_outputContractAt(catalog, 'png')['embedded_images']
                    as Map<String, Object?>)['limits']
                as Map<String, Object?>)['max_bytes_per_image'] =
            0,
    (catalog) =>
        ((_outputContractAt(catalog, 'png')['embedded_images']
                    as Map<String, Object?>)['limits']
                as Map<String, Object?>)['max_total_bytes'] =
            1.5,
    (catalog) =>
        ((_outputContractAt(catalog, 'png')['embedded_images']
                    as Map<String, Object?>)['limits']
                as Map<String, Object?>)['max_total_pixels'] =
            '4096',
  ]) {
    final catalog = _catalogWithExportOutput();
    mutation(catalog);
    _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
  }
}

void rejectsMissingOriginalSchemaOneFields() {
  for (final field in const [
    'options_schema_versions',
    'payload_schemas',
    'metadata_ids',
  ]) {
    for (final validator in <void Function(Map<String, Object?>)>[
      (catalog) => MermanRuntimeCatalog.fromJson(catalog),
      (catalog) =>
          MermanRuntimeCatalog.fromJson(catalog).requireCurrentBindingSchemas(),
    ]) {
      final catalog = _clonedCatalog()..remove(field);
      _expectContractFailure(() => validator(catalog));
    }
  }
}

void rejectsInconsistentGeneratedCatalogRelations() {
  for (final catalog in <Map<String, Object?>>[
    _catalog(
      capabilityIds: const ['math'],
      outputIds: const [],
      operationIds: const ['semantic-json'],
    ),
    _catalog(
      capabilityIds: const [],
      outputIds: const [],
      operationIds: const ['semantic-json'],
      metadataIds: const ['lint-rule-catalog'],
    ),
    _catalog(
      capabilityIds: const ['png'],
      outputIds: const [],
      operationIds: const ['png', 'semantic-json'],
    ),
  ]) {
    _expectContractFailure(() => MermanRuntimeCatalog.fromJson(catalog));
  }
}

void enforcesRuntimeIdentifierGrammarWithoutClosingFutureVocabularies() {
  final valid = _catalog();
  final capabilities = _runtimeCapabilities(valid);
  capabilities['capability_ids'] = <String>['future-capability', 'svg'];
  capabilities['operation_ids'] = <String>[
    'future-operation',
    'semantic-json',
    'svg',
  ];
  (valid['metadata_ids'] as List<String>)
    ..add('future-metadata')
    ..sort();
  (valid['option_group_ids'] as List<String>)
    ..add('future_group')
    ..sort();
  valid['payload_schemas'] =
      <Object?>[
        ...valid['payload_schemas'] as List<Object?>,
        <String, Object>{'id': 'future-schema', 'version': 7},
      ]..sort(
        (left, right) => (left as Map<String, Object?>)['id']
            .toString()
            .compareTo((right as Map<String, Object?>)['id'].toString()),
      );
  final validated = MermanRuntimeCatalog.fromJson(valid);
  _expect(
    validated.supportsCapability('future-capability') &&
        validated.supportsOperation('future-operation') &&
        validated.metadataIds.contains('future-metadata') &&
        validated.optionGroupIds.contains('future_group') &&
        validated.supportsPayloadSchema('future-schema', 7),
    'valid future IDs must remain discoverable across open runtime vocabularies',
  );

  for (final mutation in <void Function(Map<String, Object?>)>[
    (catalog) =>
        _runtimeCapabilities(catalog)['capability_ids'] = const ['SVG'],
    (catalog) => catalog['metadata_ids'] = const ['future metadata'],
    (catalog) => catalog['option_group_ids'] = const ['future.option'],
    (catalog) => catalog['payload_schemas'] = const [
      {'id': 'future_schema', 'version': 1},
    ],
    (catalog) => _limitAt(catalog, 0)['id'] = 'Future_limit',
    (catalog) => _profileAt(catalog, 0)['id'] = 'future_profile',
  ]) {
    final catalog = _clonedCatalog();
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
    (catalog) =>
        (_runtimeCapabilities(catalog)['text_measurement']
                as Map<String, Object?>)['protocol_version'] =
            '1',
    (catalog) =>
        (_runtimeCapabilities(catalog)['text_measurement']
                as Map<String, Object?>)['protocol_version'] =
            1.0,
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
    final textMeasurement =
        _runtimeCapabilities(catalog)['text_measurement']
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
    (catalog) => (_resources(catalog)['limits'] as List<Object?>).add(
      Map<String, Object?>.from(_limitAt(catalog, 0)),
    ),
    (catalog) => _profileAt(catalog, 0).remove('purpose'),
    (catalog) => _profileAt(catalog, 0)['recommended_binding_default'] = 'true',
    (catalog) =>
        (_profileAt(catalog, 0)['limits']
                as Map<String, Object?>)['max_source_bytes'] =
            '1',
    (catalog) =>
        (_profileAt(catalog, 0)['limits']
                as Map<String, Object?>)['max_source_bytes'] =
            1.5,
    (catalog) =>
        (_profileAt(catalog, 0)['limits']
                as Map<String, Object?>)['max_source_bytes'] =
            0,
    (catalog) => (_profileAt(catalog, 0)['limits'] as Map<String, Object?>)
        .remove('max_source_bytes'),
    (catalog) =>
        (_profileAt(catalog, 0)['limits']
                as Map<String, Object?>)['undeclared_limit'] =
            null,
    (catalog) => (_resources(catalog)['profiles'] as List<Object?>).add(
      Map<String, Object?>.from(_profileAt(catalog, 0)),
    ),
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
  const expectedDiagnostic = MermanDiagnosticErrorDetails(
    code: 'merman.flowchart.edge.invalid',
    span: MermanDiagnosticSpan(start: 3, end: 8, kind: 'exact'),
    field: 'edge',
    diagramType: 'flowchart-v2',
  );
  Map<String, Object?> diagnosticJson() => {
    'code': expectedDiagnostic.code,
    'span': {
      'start': expectedDiagnostic.span!.start,
      'end': expectedDiagnostic.span!.end,
      'kind': expectedDiagnostic.span!.kind,
      'future_span_field': true,
    },
    'field': expectedDiagnostic.field,
    'diagram_type': expectedDiagnostic.diagramType,
    'future_diagnostic_field': true,
  };
  bool preservesExpectedDiagnostic(MermanException error) =>
      error.diagnosticDetails?.code == expectedDiagnostic.code &&
      error.diagnosticDetails?.span?.start == expectedDiagnostic.span?.start &&
      error.diagnosticDetails?.span?.end == expectedDiagnostic.span?.end &&
      error.diagnosticDetails?.span?.kind == expectedDiagnostic.span?.kind &&
      error.diagnosticDetails?.field == expectedDiagnostic.field &&
      error.diagnosticDetails?.diagramType == expectedDiagnostic.diagramType;

  final diagnostic = MermanException.fromNative(
    native.MERMAN_NATIVE_STATUS_PARSE_ERROR,
    Uint8List.fromList(
      utf8.encode(
        jsonEncode({
          'version': 1,
          'ok': false,
          'status': native.MERMAN_NATIVE_STATUS_PARSE_ERROR,
          'status_name': 'parse-error',
          'kind': 'generic',
          'capability_id': null,
          'details': {'diagnostic': diagnosticJson()},
          'message': 'invalid flowchart edge',
        }),
      ),
    ),
  );
  _expect(
    diagnostic.runtimeType == MermanException &&
        preservesExpectedDiagnostic(diagnostic),
    'structured diagnostic metadata should survive the Dart boundary',
  );

  final specializedCases =
      <
        ({
          String label,
          int status,
          String statusName,
          String kind,
          String? capabilityId,
          Map<String, Object?> additionalDetails,
          bool Function(MermanException) matches,
        })
      >[
        (
          label: 'unknown operation',
          status: native.MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
          statusName: 'unsupported-operation',
          kind: 'unknown-operation',
          capabilityId: null,
          additionalDetails: const {},
          matches: (error) =>
              error is MermanUnknownOperationException &&
              error.kind == MermanErrorKind.unknownOperation,
        ),
        (
          label: 'missing capability',
          status: native.MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
          statusName: 'unsupported-operation',
          kind: 'missing-capability',
          capabilityId: 'svg',
          additionalDetails: const {},
          matches: (error) =>
              error is MermanMissingCapabilityException &&
              error.capabilityId == 'svg',
        ),
        (
          label: 'generic unsupported operation',
          status: native.MERMAN_NATIVE_STATUS_UNSUPPORTED_OPERATION,
          statusName: 'unsupported-operation',
          kind: 'generic',
          capabilityId: null,
          additionalDetails: const {},
          matches: (error) =>
              error.runtimeType == MermanUnsupportedOperationException &&
              error.kind == MermanErrorKind.generic,
        ),
        (
          label: 'reentrant call',
          status: native.MERMAN_NATIVE_STATUS_REENTRANT_CALL,
          statusName: 'reentrant-call',
          kind: 'reentrant-call',
          capabilityId: null,
          additionalDetails: const {},
          matches: (error) =>
              error is MermanReentrantCallException &&
              error.kind == MermanErrorKind.reentrantCall,
        ),
        (
          label: 'busy engine',
          status: native.MERMAN_NATIVE_STATUS_BUSY,
          statusName: 'busy',
          kind: 'busy',
          capabilityId: null,
          additionalDetails: const {},
          matches: (error) =>
              error is MermanBusyException &&
              error.kind == MermanErrorKind.busy,
        ),
        (
          label: 'cancelled operation',
          status: native.MERMAN_NATIVE_STATUS_CANCELLED,
          statusName: 'cancelled',
          kind: 'generic',
          capabilityId: null,
          additionalDetails: const {
            'cancellation': {'reason': 'deadline_exceeded', 'phase': 'layout'},
          },
          matches: (error) =>
              error is MermanCancelledException &&
              error.cancellationDetails?.reason == 'deadline_exceeded' &&
              error.cancellationDetails?.phase == 'layout',
        ),
      ];
  for (final testCase in specializedCases) {
    final preservesDiagnostic =
        testCase.status != native.MERMAN_NATIVE_STATUS_CANCELLED;
    final error = MermanException.fromNative(
      testCase.status,
      Uint8List.fromList(
        utf8.encode(
          jsonEncode({
            'version': 1,
            'ok': false,
            'status': testCase.status,
            'status_name': testCase.statusName,
            'kind': testCase.kind,
            'capability_id': testCase.capabilityId,
            'details': {
              if (preservesDiagnostic) 'diagnostic': diagnosticJson(),
              ...testCase.additionalDetails,
            },
            'message': '${testCase.label} failed',
          }),
        ),
      ),
    );
    _expect(
      testCase.matches(error) &&
          (preservesDiagnostic
              ? preservesExpectedDiagnostic(error)
              : error.diagnosticDetails == null),
      '${testCase.label} should preserve its valid terminal classification',
    );
  }

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
              'cause': 'arithmetic_overflow',
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
    resource.resourceDetails?.cause == 'arithmetic_overflow' &&
        resource.resourceDetails?.limitId.id == 'max_embedded_image_bytes' &&
        resource.resourceDetails?.phase == 'embedded_image_decode' &&
        resource.resourceDetails?.actual == 5 &&
        resource.resourceDetails?.max == 4 &&
        resource.resourceDetails?.profile == 'constrained',
    'resource metadata should survive the Dart boundary',
  );
  _expect(
    resource.exactResourceDetails?.actual == '5' &&
        resource.exactResourceDetails?.max == '4',
    'resource metadata should expose an exact decimal projection',
  );

  final wideResource = MermanException.fromNative(
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
              'cause': 'arithmetic_overflow',
              'limit_id': 'max_ascii_layout_work_units',
              'phase': 'ascii_layout_work',
              'actual': '18446744073709551615',
              'max': '9223372036854775808',
              'profile': 'interactive',
            },
          },
          'message': 'ASCII layout work accounting overflowed',
        }),
      ),
    ),
  );
  _expect(
    wideResource.exactResourceDetails?.actual == '18446744073709551615' &&
        wideResource.exactResourceDetails?.max == '9223372036854775808' &&
        wideResource.resourceDetails == null,
    'wide resource counts should survive exactly without a signed compatibility view',
  );

  final iconRegistry = MermanException.fromNative(
    native.MERMAN_NATIVE_STATUS_RENDER_ERROR,
    Uint8List.fromList(
      utf8.encode(
        jsonEncode({
          'version': 1,
          'ok': false,
          'status': native.MERMAN_NATIVE_STATUS_RENDER_ERROR,
          'status_name': 'render-error',
          'kind': 'generic',
          'capability_id': null,
          'details': {
            'icon_registry': {
              'kind_id': 'invalid_xml',
              'pack_index': 2,
              'registration_name': 'smoke',
            },
          },
          'message': 'icon body is invalid',
        }),
      ),
    ),
  );
  _expect(
    iconRegistry.iconRegistryDetails?.kindId == 'invalid_xml' &&
        iconRegistry.iconRegistryDetails?.packIndex == 2 &&
        iconRegistry.iconRegistryDetails?.registrationName == 'smoke',
    'icon-registry metadata should survive the Dart boundary',
  );
}

void rejectsInconsistentNativeErrorRelations() {
  Map<String, Object?> cancellationEnvelope({
    int? status,
    String statusName = 'cancelled',
    String kind = 'generic',
    String? capabilityId,
    String reason = 'requested',
    String phase = 'layout',
    Map<String, Object?> additionalDetails = const {},
  }) => {
    'version': native.MERMAN_NATIVE_RESULT_SCHEMA_VERSION,
    'ok': false,
    'status': status ?? native.MERMAN_NATIVE_STATUS_CANCELLED,
    'status_name': statusName,
    'kind': kind,
    'capability_id': capabilityId,
    'details': {
      'cancellation': {'reason': reason, 'phase': phase},
      ...additionalDetails,
    },
    'message': 'cancelled',
  };

  final inconsistentPayloads = <Map<String, Object?>>[
    cancellationEnvelope(
      additionalDetails: {
        'resource': {
          'cause': 'ceiling',
          'limit_id': 'max_source_bytes',
          'phase': 'source_input',
          'actual': 2,
          'max': 1,
          'profile': 'interactive',
        },
      },
    ),
    cancellationEnvelope(
      additionalDetails: {
        'diagnostic': {
          'code': 'merman.test',
          'span': null,
          'field': null,
          'diagram_type': null,
        },
      },
    ),
    cancellationEnvelope(
      additionalDetails: {
        'icon_registry': {
          'kind_id': 'invalid_xml',
          'pack_index': null,
          'registration_name': null,
        },
      },
    ),
    cancellationEnvelope(status: native.MERMAN_NATIVE_STATUS_PARSE_ERROR),
    cancellationEnvelope(statusName: 'parse-error'),
    cancellationEnvelope(reason: 'bogus'),
    cancellationEnvelope(phase: 'future-phase'),
    cancellationEnvelope(kind: 'missing-capability', capabilityId: 'svg'),
    cancellationEnvelope(kind: 'future-kind'),
  ];

  for (final payload in inconsistentPayloads) {
    final error = MermanException.fromNative(
      native.MERMAN_NATIVE_STATUS_CANCELLED,
      Uint8List.fromList(utf8.encode(jsonEncode(payload))),
    );
    _expect(
      error.codeName == 'DART_NATIVE_CONTRACT_ERROR' &&
          error.kind == MermanErrorKind.generic &&
          error.capabilityId == null &&
          error.cancellationDetails == null,
      'inconsistent native terminal fields must fail closed as a contract error',
    );
  }
}

void rejectsMalformedNativeDiagnosticDetails() {
  for (final diagnostic in <Map<String, Object?>>[
    {'code': '', 'span': null, 'field': null, 'diagram_type': null},
    {
      'code': 'merman.test',
      'span': 'not-an-object',
      'field': null,
      'diagram_type': null,
    },
    {
      'code': 'merman.test',
      'span': {'start': -1, 'end': 3, 'kind': 'exact'},
      'field': null,
      'diagram_type': null,
    },
    {
      'code': 'merman.test',
      'span': {'start': 4, 'end': 3, 'kind': 'exact'},
      'field': null,
      'diagram_type': null,
    },
    {
      'code': 'merman.test',
      'span': {'start': 3, 'end': 4, 'kind': 'future-kind'},
      'field': null,
      'diagram_type': null,
    },
    {
      'code': 'merman.test',
      'span': {'start': 3, 'end': 4, 'kind': 'insertion-point'},
      'field': null,
      'diagram_type': null,
    },
    {'code': 'merman.test', 'span': null, 'field': 7, 'diagram_type': null},
    {'code': 'merman.test', 'span': null, 'field': null, 'diagram_type': false},
  ]) {
    final error = MermanException.fromNative(
      native.MERMAN_NATIVE_STATUS_PARSE_ERROR,
      Uint8List.fromList(
        utf8.encode(
          jsonEncode({
            'version': 1,
            'ok': false,
            'status': native.MERMAN_NATIVE_STATUS_PARSE_ERROR,
            'status_name': 'parse-error',
            'kind': 'generic',
            'capability_id': null,
            'details': {'diagnostic': diagnostic},
            'message': 'invalid diagram',
          }),
        ),
      ),
    );
    _expect(
      error.codeName == 'DART_NATIVE_CONTRACT_ERROR' &&
          error.diagnosticDetails == null,
      'malformed diagnostic metadata must fail closed as a contract error',
    );
  }
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
      error.code == (payload.isEmpty ? status : -1),
      'malformed native error payload escaped fail-closed decoding',
    );
  }
}

Map<String, Object?> _catalog({
  List<String> capabilityIds = const ['svg'],
  List<String> outputIds = const ['svg'],
  List<String> operationIds = const ['semantic-json', 'svg'],
  List<String> systemAdapterIds = const [],
  List<String>? metadataIds,
}) {
  final usesSvgPipeline = capabilityIds.contains('svg');
  final optionGroupIds =
      binding.mermanBindingOptionGroupSpecs.values
          .where(
            (spec) =>
                spec.alwaysAvailable ||
                (spec.requiresSvgPipeline && usesSvgPipeline) ||
                spec.anyCapabilityIds.any(capabilityIds.contains),
          )
          .map((spec) => spec.id)
          .toList()
        ..sort();
  final transport = binding.mermanBindingTransportExposureSpecs['native-c']!;
  final serviceSpecs =
      binding.mermanBindingConstructorServiceSpecs.values
          .where(
            (spec) =>
                transport.constructorServiceCandidateIds.contains(spec.id) &&
                (!spec.requiresSvgPipeline || usesSvgPipeline),
          )
          .toList()
        ..sort((left, right) => left.id.compareTo(right.id));
  final providers = <String>{
    if (usesSvgPipeline) 'vendored',
    for (final spec in serviceSpecs) ...spec.providedTextMeasurementProviderIds,
  }.toList()..sort();
  final effectiveMetadataIds =
      metadataIds ??
      (binding.mermanBindingMetadataSpecs.values
          .where(
            (spec) =>
                spec.requiredCapabilityId == null ||
                capabilityIds.contains(spec.requiredCapabilityId),
          )
          .map((spec) => spec.id)
          .toList()
        ..sort());
  return {
    'schema_version': 1,
    'transport_api_version': 3,
    'package_version': 'test',
    'options_schema_versions': const [2],
    'payload_schemas': const [
      {'id': 'binding-result', 'version': 1},
      {'id': 'operation-metadata', 'version': 1},
    ],
    'metadata_ids': effectiveMetadataIds,
    'capabilities': {
      'capability_ids': capabilityIds,
      'output_ids': outputIds,
      'operation_ids': operationIds,
      'system_adapter_ids': systemAdapterIds,
      'text_measurement': usesSvgPipeline
          ? {'protocol_version': 1, 'provider_ids': providers}
          : null,
    },
    'option_group_ids': optionGroupIds,
    'constructor_service_ids': serviceSpecs.map((spec) => spec.id).toList(),
    'constructor_service_contracts': <Object?>[
      for (final spec in serviceSpecs)
        <String, Object?>{
          'id': spec.id,
          'provided_text_measurement_provider_ids':
              spec.providedTextMeasurementProviderIds.toList()..sort(),
          'resource_limits': <Object?>[
            for (final limit in spec.resourceLimits)
              <String, Object?>{
                'id': limit.id,
                'phase': limit.phase,
                'unit': limit.unit,
                'description': limit.description,
                'value': limit.value,
              },
          ],
        },
    ],
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
        for (final limit in MermanResourceLimitId.knownValues)
          <String, Object?>{
            'id': limit.id,
            'phase': limit.phase,
            'description': 'Test descriptor for ${limit.id}',
            'overridable': limit.overridable,
            'hard_cap': !limit.overridable!,
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
              for (final limit in MermanResourceLimitId.knownValues)
                limit.id:
                    profile == MermanResourceProfile.unboundedForTrustedInput &&
                        limit.overridable!
                    ? null
                    : 1,
            },
          },
      ],
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

Map<String, Object?> _constructorServiceContract(
  Map<String, Object?> catalog,
  String serviceId,
) => (catalog['constructor_service_contracts'] as List<Object?>)
    .cast<Map<String, Object?>>()
    .firstWhere((contract) => contract['id'] == serviceId);

List<Object?> _outputContracts(Map<String, Object?> catalog) =>
    catalog['output_contracts']! as List<Object?>;

Map<String, Object?> _outputContractAt(
  Map<String, Object?> catalog,
  String outputId,
) => _outputContracts(catalog).cast<Map<String, Object?>>().firstWhere(
  (contract) => contract['id'] == outputId,
);

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
