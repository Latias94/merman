import 'dart:convert';
import 'dart:typed_data';

import 'package:merman/merman.dart';

MermanTextMeasureResult? _measure(MermanTextMeasureRequest request) {
  return switch (request.operation) {
    MermanTextMeasurementOperation.measure ||
    MermanTextMeasurementOperation.wrapped ||
    MermanTextMeasurementOperation.mermaidCalculateTextDimensions =>
      MermanTextMeasureResult.metrics(width: 42, height: 24, lineCount: 1),
    MermanTextMeasurementOperation.computedLength ||
    MermanTextMeasurementOperation.simpleBBoxWidth ||
    MermanTextMeasurementOperation.rawBBoxWidth ||
    MermanTextMeasurementOperation.tspanBBoxWidth ||
    MermanTextMeasurementOperation.tspanBBoxHeight ||
    MermanTextMeasurementOperation.wrapProbeBBoxWidth ||
    MermanTextMeasurementOperation.simpleBBoxHeight ||
    MermanTextMeasurementOperation.boundingClientRectWidth ||
    MermanTextMeasurementOperation.canvasMeasureTextWidth ||
    MermanTextMeasurementOperation.rawBBoxHeight =>
      MermanTextMeasureResult.length(length: 42),
    MermanTextMeasurementOperation.bboxX ||
    MermanTextMeasurementOperation.bboxXWithAsciiOverhang ||
    MermanTextMeasurementOperation.titleBBoxX =>
      MermanTextMeasureResult.horizontalExtents(left: 21, right: 21),
    MermanTextMeasurementOperation.wrappedWithRawWidth =>
      MermanTextMeasureResult.wrappedWithRawWidth(
        width: 42,
        height: 24,
        lineCount: 1,
        rawWidth: 42,
      ),
    MermanTextMeasurementOperation.createTextBBoxYOffset ||
    MermanTextMeasurementOperation.createTextMiddleBBoxYOffset =>
      MermanTextMeasureResult.length(length: -1),
    null => null,
  };
}

void _expect(bool condition, String message) {
  if (!condition) {
    throw StateError(message);
  }
}

void _expectPrefix(Uint8List bytes, List<int> prefix, String label) {
  _expect(bytes.length >= prefix.length, '$label output is too short');
  for (var index = 0; index < prefix.length; index += 1) {
    _expect(bytes[index] == prefix[index], '$label signature mismatch');
  }
}

bool _sameBytes(Uint8List left, Uint8List right) {
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

MermanEngine _openEngine(
  List<String> args, {
  String? optionsJson,
  MermanEngineServices services = const MermanEngineServices(),
}) =>
    args.isEmpty
        ? MermanEngine(optionsJson: optionsJson, services: services)
        : MermanEngine.openPath(
            args.single,
            optionsJson: optionsJson,
            services: services,
          );

void main(List<String> args) {
  if (args.length > 1) {
    throw ArgumentError('expected at most one native library path');
  }
  final merman = args.isEmpty ? Merman.open() : Merman.openPath(args.single);
  const source = 'flowchart TD\nA[Hello] --> B[World]';
  final catalog = merman.runtimeCatalog;
  _expect(
    merman.packageVersion == catalog.packageVersion,
    'table and runtime catalog package versions must agree',
  );
  const expectedCapabilities = {
    'analysis',
    'ascii',
    'jpeg',
    'layout-cytoscape',
    'layout-elk',
    'math',
    'pdf',
    'png',
    'svg',
    'system-clock',
    'system-random',
    'system-timezone',
  };
  const expectedOutputs = {'ascii', 'jpeg', 'pdf', 'png', 'svg'};
  const expectedOperations = {
    'analysis-facts-json',
    'analysis-json',
    'ascii',
    'document-analysis-facts-json',
    'document-analysis-json',
    'jpeg',
    'layout-json',
    'pdf',
    'png',
    'semantic-json',
    'svg',
    'svg-plan-json',
    'validation-json',
  };
  const expectedSystemAdapters = {
    'system-clock',
    'system-random',
    'system-timezone',
  };
  _expect(
    catalog.capabilityIds.toSet().containsAll(expectedCapabilities) &&
        expectedCapabilities.containsAll(catalog.capabilityIds),
    'native SDK capability catalog drifted',
  );
  _expect(
    catalog.outputIds.toSet().containsAll(expectedOutputs) &&
        expectedOutputs.containsAll(catalog.outputIds),
    'native SDK output catalog drifted',
  );
  _expect(
    catalog.operationIds.toSet().containsAll(expectedOperations) &&
        expectedOperations.containsAll(catalog.operationIds),
    'native SDK operation catalog drifted',
  );
  _expect(
    catalog.systemAdapterIds.toSet().containsAll(expectedSystemAdapters) &&
        expectedSystemAdapters.containsAll(catalog.systemAdapterIds),
    'native SDK system adapter catalog drifted',
  );
  _expect(
    catalog.hasDeclaredOptionGroups &&
        catalog.hasDeclaredConstructorServices &&
        catalog.constructorServiceIds.length == 2,
    'native SDK transport exposure catalog drifted',
  );

  final supportedDiagrams = merman.supportedDiagrams();
  final asciiCapabilities = merman.asciiCapabilities();
  final diagramFamilies = merman.diagramFamilyCapabilities();
  final lintRules = merman.lintRuleCatalog();
  final supportedThemes = merman.supportedThemes();
  final presentationCatalog = merman.presentationCatalog();
  _expect(
    supportedDiagrams.isNotEmpty &&
        asciiCapabilities.isNotEmpty &&
        diagramFamilies.isNotEmpty &&
        lintRules.isNotEmpty &&
        supportedThemes.isNotEmpty,
    'typed native metadata catalogs must be available',
  );
  _expect(
    (jsonDecode(merman.metadataJson('supported-diagrams')) as List).isNotEmpty,
    'generic metadata dispatch failed',
  );
  _expect(
    presentationCatalog.themePresets.any((preset) => preset.id == 'one-dark') &&
        presentationCatalog.profiles
            .any((profile) => profile.id == 'merman-modern'),
    'presentation catalog must expose bundled themes and profiles',
  );
  _expect(
    identical(supportedDiagrams, merman.supportedDiagrams()) &&
        identical(asciiCapabilities, merman.asciiCapabilities()) &&
        identical(diagramFamilies, merman.diagramFamilyCapabilities()) &&
        identical(lintRules, merman.lintRuleCatalog()) &&
        identical(supportedThemes, merman.supportedThemes()) &&
        identical(presentationCatalog, merman.presentationCatalog()),
    'typed native metadata catalogs must be cached',
  );

  final genericSvg = merman.execute(
    MermanOperation.svg,
    source,
    optionsJson: '{"svg":{"diagram_id":"flutter-request"}}',
  );
  _expect(
    genericSvg.utf8Text.contains('id="flutter-request"') &&
        genericSvg.metadata.operationId == 'svg' &&
        genericSvg.metadata.runtimePolicy == 'deterministic' &&
        genericSvg.metadata.byteLength == genericSvg.bytes.length,
    'one-shot execution and typed metadata failed',
  );
  _expect(
    merman
            .execute(
              MermanOperation.semanticJson,
              source,
              optionsJson: '{"runtime_policy":"deterministic"}',
            )
            .metadata
            .runtimePolicy ==
        'deterministic',
    'one-shot runtime policy must remain constructor-owned',
  );
  _expect(
    merman.analysisFactsJson(source).isNotEmpty &&
        merman.svgPlanJson(source).isNotEmpty,
    'named analysis-facts/SVG-plan helpers failed',
  );

  if (args.isNotEmpty) {
    final pinned = MermanEngine.fromDynamicLibrary(
      openMermanLibraryFromPath(args.single),
      expectedPackageVersion: mermanPackageVersion,
      services: MermanEngineServices(textMeasurer: _measure),
    );
    try {
      _expect(
        pinned.renderSvg(source).contains('<svg'),
        'exact-version reusable callback engine failed',
      );
    } finally {
      pinned.close();
    }
    try {
      Merman.fromDynamicLibrary(
        openMermanLibraryFromPath(args.single),
        expectedPackageVersion: 'not-${merman.packageVersion}',
      );
      throw StateError('mismatched exact package version was accepted');
    } on MermanException catch (error) {
      _expect(
        error.codeName == 'DART_NATIVE_CONTRACT_ERROR',
        'exact package version mismatch returned the wrong error',
      );
    }
  }

  final configured = _openEngine(
    args,
    optionsJson: '''
      {
        "version": 2,
        "resources": {"profile": "constrained"},
        "svg": {"diagram_id": "engine-base", "pipeline": "readable"}
      }
    ''',
  );
  try {
    final configuredSvg = configured.renderSvg(
      source,
      optionsJson: '{"svg":{"diagram_id":"request-override"}}',
    );
    _expect(
      configuredSvg.contains('id="request-override"') &&
          configuredSvg.contains('data-merman-foreignobject'),
      'request options must deeply override the engine baseline',
    );
  } finally {
    configured.close();
  }

  final pngResult = merman.renderPngResult(source);
  _expectPrefix(pngResult.bytes, [0x89, 0x50, 0x4e, 0x47], 'PNG');
  _expect(
    pngResult.metadata.outputPlan is MermanRasterOutputPlan &&
        _sameBytes(pngResult.bytes, merman.renderPng(source)),
    'PNG result helper must preserve bytes and typed output planning',
  );
  final jpegResult = merman.renderJpegResult(source);
  _expectPrefix(jpegResult.bytes, [0xff, 0xd8, 0xff], 'JPEG');
  _expect(
    jpegResult.metadata.outputPlan is MermanRasterOutputPlan,
    'JPEG result helper must expose a raster plan',
  );
  final pdfResult = merman.renderPdfResult(source);
  _expectPrefix(pdfResult.bytes, [0x25, 0x50, 0x44, 0x46], 'PDF');
  _expect(
    pdfResult.metadata.rawJson.isNotEmpty,
    'PDF result helper must preserve raw metadata',
  );

  final registry = MermanIconRegistry.fromPacks([
    MermanIconPack(
      json: jsonEncode({
        'icons': {
          'rocket': {
            'body': '<path data-icon="flutter-registry" d="M0 0H16V16H0z"/>',
          },
        },
      }),
      registrationName: 'smoke',
    ),
  ]);
  _expect(
    registry.length == 1 && !registry.isEmpty,
    'immutable icon registry construction failed',
  );
  const iconSource = 'flowchart TD\nA@{ icon: "smoke:rocket", label: "A" }';
  for (var index = 0; index < 2; index += 1) {
    final iconEngine = _openEngine(
      args,
      services: MermanEngineServices(iconRegistry: registry),
    );
    try {
      _expect(
        iconEngine.renderSvg(iconSource).contains('flutter-registry'),
        'icon registry reuse failed for engine $index',
      );
    } finally {
      iconEngine.close();
    }
  }
  try {
    _openEngine(
      args,
      services: MermanEngineServices(
        iconRegistry: MermanIconRegistry.fromPacks([
          MermanIconPack(
            json: '{"prefix":"bad","icons":{"broken":{"body":"<path>"}}}',
          ),
        ]),
      ),
    );
    throw StateError('invalid icon registry unexpectedly published an engine');
  } on MermanException catch (error) {
    _expect(
      error.iconRegistryDetails?.kindId == 'invalid_xml' &&
          error.iconRegistryDetails?.packIndex == 0,
      'invalid icon registry must preserve structured transactional failure',
    );
  }

  var conflictCallbackCalls = 0;
  try {
    _openEngine(
      args,
      optionsJson: '{"environment":{"text_measurement":"deterministic"}}',
      services: MermanEngineServices(
        textMeasurer: (request) {
          conflictCallbackCalls += 1;
          return _measure(request);
        },
      ),
    );
    throw StateError('conflicting text measurement services were accepted');
  } on MermanException catch (error) {
    _expect(
      error.message.contains('environment.text_measurement') &&
          conflictCallbackCalls == 0,
      'constructor conflicts must fail without invoking the host callback',
    );
  }

  var requestConflictCallbackCalls = 0;
  final requestConflictEngine = _openEngine(
    args,
    services: MermanEngineServices(
      textMeasurer: (request) {
        requestConflictCallbackCalls += 1;
        return _measure(request);
      },
    ),
  );
  try {
    try {
      requestConflictEngine.renderSvg(
        source,
        optionsJson: '{"environment":{"text_measurement":"deterministic"}}',
      );
      throw StateError('request-local text measurement conflict was accepted');
    } on MermanException catch (error) {
      _expect(
        error.message.contains('environment.text_measurement') &&
            requestConflictCallbackCalls == 0,
        'request conflicts must fail without invoking the host callback',
      );
    }
  } finally {
    requestConflictEngine.close();
  }

  final semantic = merman.parseJson(source);
  _expect(semantic['type'] == 'flowchart-v2', 'semantic JSON smoke failed');
  final layout = merman.layoutJson(source);
  _expect(layout.containsKey('layout'), 'layout JSON smoke failed');
  _expect(merman.validate(source).valid, 'validation smoke failed');
  const documentSource = 'Intro\n```mermaid\nflowchart TD\nA --> B\n```\n';
  _expect(
    merman.analyzeDocumentJson(
          documentSource,
          uri: 'file:///tmp/example.md',
        )['valid'] ==
        true,
    'document analysis smoke failed',
  );

  late final MermanEngine measured;
  MermanException? callbackCloseError;
  var callbackCount = 0;
  measured = _openEngine(
    args,
    services: MermanEngineServices(
      textMeasurer: (request) {
        callbackCount += 1;
        if (callbackCloseError == null) {
          try {
            measured.close();
          } on MermanException catch (error) {
            callbackCloseError = error;
          }
        }
        return _measure(request);
      },
    ),
  );
  try {
    _expect(
      measured
          .renderSvg(
            source,
            optionsJson: '{"svg":{"diagram_id":"measured-request"}}',
          )
          .contains('id="measured-request"'),
      'request options must preserve host text measurement',
    );
    _expect(callbackCount > 0, 'host text measurement callback was not used');
    _expect(
      callbackCloseError is MermanReentrantCallException && !measured.isClosed,
      'reentrant close must preserve the engine and callback for retry',
    );
  } finally {
    measured.close();
    measured.close();
  }
  _expect(measured.isClosed, 'successful close must clear the Dart handle');
  try {
    measured.renderSvg(source);
    throw StateError('closed engine accepted a new operation');
  } on MermanException catch (error) {
    _expect(
      error.codeName == 'DART_ENGINE_CLOSED',
      'post-close operation returned the wrong error',
    );
  }
}
