import 'package:merman/merman.dart';

MermanTextMeasureResult? _smokeTextMeasurementResult(
  MermanTextMeasurementOperation? operation, {
  required double width,
  required double height,
}) {
  return switch (operation) {
    MermanTextMeasurementOperation.measure ||
    MermanTextMeasurementOperation.wrapped ||
    MermanTextMeasurementOperation.mermaidCalculateTextDimensions =>
      MermanTextMeasureResult.metrics(
        width: width,
        height: height,
        lineCount: 1,
      ),
    MermanTextMeasurementOperation.computedLength ||
    MermanTextMeasurementOperation.simpleBBoxWidth ||
    MermanTextMeasurementOperation.rawBBoxWidth ||
    MermanTextMeasurementOperation.boundingClientRectWidth ||
    MermanTextMeasurementOperation.tspanBBoxWidth ||
    MermanTextMeasurementOperation.wrapProbeBBoxWidth ||
    MermanTextMeasurementOperation.canvasMeasureTextWidth =>
      MermanTextMeasureResult.length(length: width),
    MermanTextMeasurementOperation.tspanBBoxHeight ||
    MermanTextMeasurementOperation.simpleBBoxHeight ||
    MermanTextMeasurementOperation.rawBBoxHeight =>
      MermanTextMeasureResult.length(length: height),
    MermanTextMeasurementOperation.createTextBBoxYOffset ||
    MermanTextMeasurementOperation.createTextMiddleBBoxYOffset =>
      MermanTextMeasureResult.length(length: -1),
    MermanTextMeasurementOperation.bboxX ||
    MermanTextMeasurementOperation.bboxXWithAsciiOverhang ||
    MermanTextMeasurementOperation.titleBBoxX =>
      MermanTextMeasureResult.horizontalExtents(
        left: width / 2,
        right: width / 2,
      ),
    MermanTextMeasurementOperation.wrappedWithRawWidth =>
      MermanTextMeasureResult.wrappedWithRawWidth(
        width: width,
        height: height,
        lineCount: 1,
        rawWidth: width,
      ),
    null => null,
  };
}

void _assertTextMeasurementContract() {
  final operationCodes = MermanTextMeasurementOperation.values
      .map((operation) => operation.code)
      .toList(growable: false);
  if (operationCodes.length != 19 ||
      operationCodes.asMap().entries.any((entry) => entry.key != entry.value)) {
    throw StateError(
      'text measurement operation codes are not the contiguous ABI range 0..18',
    );
  }

  final dimensions = _smokeTextMeasurementResult(
    MermanTextMeasurementOperation.mermaidCalculateTextDimensions,
    width: 42,
    height: 24,
  );
  if (dimensions?.resultKind != MermanTextMeasurementResultKind.metrics) {
    throw StateError('MermaidCalculateTextDimensions must return metrics');
  }

  final canvasWidth = _smokeTextMeasurementResult(
    MermanTextMeasurementOperation.canvasMeasureTextWidth,
    width: 42,
    height: 24,
  );
  if (canvasWidth?.resultKind != MermanTextMeasurementResultKind.length) {
    throw StateError('CanvasMeasureTextWidth must return length');
  }

  final rawBBoxHeight = _smokeTextMeasurementResult(
    MermanTextMeasurementOperation.rawBBoxHeight,
    width: 42,
    height: 24,
  );
  if (rawBBoxHeight?.resultKind != MermanTextMeasurementResultKind.length ||
      rawBBoxHeight?.length != 24) {
    throw StateError('RawBBoxHeight must return the raw bbox height as length');
  }

  final yOffset = _smokeTextMeasurementResult(
    MermanTextMeasurementOperation.createTextBBoxYOffset,
    width: 42,
    height: 24,
  );
  if (yOffset == null || yOffset.length >= 0) {
    throw StateError('CreateTextBBoxYOffset must preserve signed lengths');
  }
  final middleYOffset = _smokeTextMeasurementResult(
    MermanTextMeasurementOperation.createTextMiddleBBoxYOffset,
    width: 42,
    height: 24,
  );
  if (middleYOffset == null || middleYOffset.length >= 0) {
    throw StateError(
        'CreateTextMiddleBBoxYOffset must preserve signed lengths');
  }
}

void main(List<String> args) {
  _assertTextMeasurementContract();

  final merman = args.isEmpty ? Merman.open() : Merman.openPath(args.single);
  final source = 'flowchart TD\nA[Hello] --> B[World]';

  final svg = merman.renderSvg(source);
  if (!svg.contains('<svg') ||
      !svg.contains('Hello') ||
      !svg.contains('World')) {
    throw StateError('SVG smoke failed');
  }

  final ascii = merman.renderAscii(source);
  if (!ascii.contains('Hello') || !ascii.contains('World')) {
    throw StateError('ASCII smoke failed');
  }

  final semantic = merman.parseJson(source);
  if (semantic['type'] != 'flowchart-v2') {
    throw StateError('parseJson smoke failed');
  }

  final layout = merman.layoutJson(source);
  if (!layout.containsKey('meta') || !layout.containsKey('layout')) {
    throw StateError('layoutJson smoke failed');
  }

  final validation = merman.validate(source);
  if (!validation.valid || validation.codeName != 'MERMAN_OK') {
    throw StateError('validate smoke failed');
  }

  final documentSource = 'Intro\n```mermaid\n$source\n```\n';
  final document = merman.analyzeDocumentJson(
    documentSource,
    uri: 'file:///tmp/example.md',
  );
  if ((document['source'] as Map<String, Object?>?)?['kind'] != 'markdown' ||
      document['valid'] != true) {
    throw StateError('analyzeDocumentJson smoke failed');
  }
  final documentFacts = merman.analyzeDocumentFactsJson(
    documentSource,
    uri: 'file:///tmp/example.md',
  );
  final diagrams = documentFacts['diagrams'] as List<Object?>? ?? const [];
  if (diagrams.isEmpty ||
      (diagrams.first as Map<String, Object?>)['source_id'] !=
          'mermaid-fence-1') {
    throw StateError('analyzeDocumentFactsJson smoke failed');
  }

  if (!merman.supportedDiagrams().contains('flowchart')) {
    throw StateError('supportedDiagrams smoke failed');
  }
  final ganttAsciiCapability = merman.asciiCapabilities().any(
        (capability) =>
            capability.diagramType == 'gantt' &&
            capability.supportLevel == 'summary' &&
            !capability.summaryFallback,
      );
  if (!ganttAsciiCapability) {
    throw StateError('asciiCapabilities smoke failed');
  }
  final flowchartCapability = merman.diagramFamilyCapabilities().any(
        (capability) =>
            capability.diagramType == 'flowchart' &&
            capability.logicalFamilyKind == 'flowchart' &&
            capability.metadataId == 'flowchart' &&
            capability.renderModelKind == 'flowchart' &&
            capability.hasDetector &&
            capability.hasSemanticParser &&
            capability.hasEditorParser &&
            capability.hasCombinedParser &&
            capability.hasRenderParser &&
            !capability.hasHeader &&
            capability.configNamespace == 'flowchart',
      );
  if (!flowchartCapability) {
    throw StateError('diagramFamilyCapabilities smoke failed');
  }
  if (!merman.lintRuleCatalog().any(
        (rule) =>
            rule.id == 'merman.authoring.flowchart.explicit_direction' &&
            rule.evidence.contains('docs/adr/0072-lint-rule-governance.md'),
      )) {
    throw StateError('lintRuleCatalog smoke failed');
  }
  if (!merman.supportedThemes().contains('default')) {
    throw StateError('themes smoke failed');
  }
  if (!merman.supportedHostThemePresets().contains('one-dark')) {
    throw StateError('host theme presets smoke failed');
  }

  final engine = merman.reusableEngine();
  try {
    final measurementPhases = <MermanTextMeasurementPhase>{};
    final measurementOperations = <MermanTextMeasurementOperation>{};
    engine.setTextMeasurer((request) {
      if (request.phase != null) {
        measurementPhases.add(request.phase!);
      }
      if (request.operation != null) {
        measurementOperations.add(request.operation!);
      }
      if (request.text == 'Hello' &&
          request.wrapMode == MermanTextWrapMode.htmlLike) {
        return _smokeTextMeasurementResult(
          request.operation,
          width: 42,
          height: 24,
        );
      }
      return null;
    });
    final measuredSvg = engine.renderSvg(source);
    if (!measuredSvg.contains('<svg') || !measuredSvg.contains('Hello')) {
      throw StateError('reusable engine SVG smoke failed');
    }
    if (!measurementPhases.contains(MermanTextMeasurementPhase.wrap)) {
      throw StateError(
        'text measurement wrap phase was not observable: $measurementPhases',
      );
    }
    if (!measurementOperations
        .contains(MermanTextMeasurementOperation.wrapped)) {
      throw StateError(
        'wrapped text measurement operation was not observable: '
        '$measurementOperations',
      );
    }
    final reusableDocument = engine.analyzeDocumentJson(
      documentSource,
      uri: 'file:///tmp/example.md',
    );
    if ((reusableDocument['source'] as Map<String, Object?>?)?['kind'] !=
        'markdown') {
      throw StateError('reusable analyzeDocumentJson smoke failed');
    }
    final reusableDocumentFacts = engine.analyzeDocumentFactsJson(
      documentSource,
      uri: 'file:///tmp/example.md',
    );
    final reusableDiagrams =
        reusableDocumentFacts['diagrams'] as List<Object?>? ?? const [];
    if (reusableDiagrams.isEmpty ||
        (reusableDiagrams.first as Map<String, Object?>)['source_id'] !=
            'mermaid-fence-1') {
      throw StateError('reusable analyzeDocumentFactsJson smoke failed');
    }
    engine.setTextMeasurer(null);
  } finally {
    engine.close();
  }

  final reentrantEngine = merman.reusableEngine();
  var sawReentrantCallback = false;
  String? reentrantFailure;
  try {
    reentrantEngine.setTextMeasurer((request) {
      if (!sawReentrantCallback && request.text == 'Hello') {
        sawReentrantCallback = true;
        try {
          reentrantEngine.renderSvg(source);
          reentrantFailure = 'expected DART_ENGINE_REENTERED to be thrown';
        } on MermanException catch (error) {
          if (error.codeName != 'DART_ENGINE_REENTERED') {
            reentrantFailure =
                'expected DART_ENGINE_REENTERED, got ${error.codeName}';
          }
        } catch (error) {
          reentrantFailure = 'expected DART_ENGINE_REENTERED, got $error';
        }
      }
      return null;
    });
    final svgAfterReentry = reentrantEngine.renderSvg(source);
    final reentrantFailureMessage = reentrantFailure;
    if (reentrantFailureMessage != null) {
      throw StateError(reentrantFailureMessage);
    }
    if (!sawReentrantCallback || !svgAfterReentry.contains('<svg')) {
      throw StateError('reusable engine reentry smoke failed');
    }
  } finally {
    reentrantEngine.close();
  }

  final closingEngine = merman.reusableEngine();
  var sawCloseCallback = false;
  try {
    closingEngine.setTextMeasurer((request) {
      if (!sawCloseCallback && request.text == 'Hello') {
        sawCloseCallback = true;
        closingEngine.close();
      }
      return null;
    });
    final svgAfterCallbackClose = closingEngine.renderSvg(source);
    if (!sawCloseCallback || !svgAfterCallbackClose.contains('<svg')) {
      throw StateError('reusable engine callback close smoke failed');
    }
    expectMermanException('DART_ENGINE_CLOSED', () {
      closingEngine.renderSvg(source);
    });
  } finally {
    closingEngine.close();
  }

  try {
    merman.renderSvg(source, optionsJson: '{');
  } on MermanException catch (error) {
    if (error.codeName != 'MERMAN_OPTIONS_JSON_ERROR') {
      throw StateError('unexpected error code: ${error.codeName}');
    }
  }

  print('merman Dart FFI smoke passed (${merman.packageVersion})');
}

void expectMermanException(String codeName, void Function() body) {
  try {
    body();
  } catch (error) {
    if (error is MermanException && error.codeName == codeName) {
      return;
    }
    throw StateError('expected $codeName, got $error');
  }
  throw StateError('expected $codeName to be thrown');
}
