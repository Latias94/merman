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

void main(List<String> args) {
  if (args.length > 1) {
    throw ArgumentError('expected at most one native library path');
  }
  final merman = args.isEmpty ? Merman.open() : Merman.openPath(args.single);
  try {
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
    _expect(catalog.supportsCapability('svg'), 'native SDK must expose SVG');
    _expect(catalog.supportsOutput('svg'), 'native SDK must expose SVG output');
    _expect(
      catalog.textMeasurementProviderIds.isNotEmpty,
      'SVG artifact must report a text measurement provider',
    );

    final svg = merman.renderSvg(source);
    _expect(svg.contains('<svg'), 'SVG smoke failed');
    _expect(svg.contains('Hello'), 'SVG text smoke failed');
    final genericSvg = merman.execute(
      MermanOperation.svg,
      source,
      optionsJson: '{"svg":{"diagram_id":"flutter-request"}}',
    );
    _expect(
      genericSvg.metadata['runtime_policy'] == 'deterministic',
      'request options must preserve the engine runtime policy',
    );
    _expect(
      genericSvg.utf8Text.contains('id="flutter-request"'),
      'generic request options smoke failed',
    );
    for (var iteration = 0; iteration < 32; iteration += 1) {
      final semanticResult = merman.execute(
        MermanOperation.semanticJson,
        source,
      );
      _expect(
        semanticResult.jsonObject['type'] == 'flowchart-v2',
        'repeated generic execution failed at iteration $iteration',
      );
    }

    if (args.isNotEmpty) {
      final pinned = Merman.fromDynamicLibrary(
        openMermanLibraryFromPath(args.single),
        expectedPackageVersion: mermanPackageVersion,
        textMeasurer: _measure,
      );
      try {
        _expect(
          pinned.renderSvg(source).contains('<svg'),
          'exact-version constructor callback smoke failed',
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

    final configured = merman.reusableEngine(
      optionsJson: '''
        {
          "version": 1,
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
      configured.dispose();
    }

    final ascii = merman.renderAscii(source);
    _expect(ascii.contains('Hello'), 'ASCII smoke failed');

    _expectPrefix(merman.renderPng(source), [0x89, 0x50, 0x4e, 0x47], 'PNG');
    _expectPrefix(merman.renderJpeg(source), [0xff, 0xd8, 0xff], 'JPEG');
    _expectPrefix(merman.renderPdf(source), [0x25, 0x50, 0x44, 0x46], 'PDF');

    final semantic = merman.parseJson(source);
    _expect(semantic['type'] == 'flowchart-v2', 'semantic JSON smoke failed');
    final layout = merman.layoutJson(source);
    _expect(layout.containsKey('layout'), 'layout JSON smoke failed');
    final validation = merman.validate(source);
    _expect(validation.valid, 'validation smoke failed');

    const documentSource = 'Intro\n```mermaid\nflowchart TD\nA --> B\n```\n';
    final document = merman.analyzeDocumentJson(
      documentSource,
      uri: 'file:///tmp/example.md',
    );
    _expect(document['valid'] == true, 'document analysis smoke failed');

    late final MermanReusableEngine measured;
    MermanException? callbackCloseError;
    var callbackCount = 0;
    measured = merman.reusableEngine(
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
    );
    try {
      _expect(
        measured
            .renderSvg(
              source,
              optionsJson: '{"svg":{"diagram_id":"measured-request"}}',
            )
            .contains('id="measured-request"'),
        'request options must preserve the host text measurement engine',
      );
      _expect(callbackCount > 0, 'host text measurement callback was not used');
      _expect(
        callbackCloseError is MermanReentrantCallException &&
            !measured.isDisposed,
        'reentrant close must retain the native token and callback',
      );
    } finally {
      measured.close();
      measured.close();
    }
    _expect(measured.isDisposed, 'successful close must clear the Dart handle');
  } finally {
    merman.close();
  }
}
