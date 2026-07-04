import 'package:merman/merman.dart';

void main(List<String> args) {
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
            capability.metadataId == 'flowchart' &&
            capability.hasSemanticParser &&
            capability.hasRenderParser,
      );
  if (!flowchartCapability) {
    throw StateError('diagramFamilyCapabilities smoke failed');
  }
  if (!merman.lintRuleCatalog().any((rule) =>
      rule.id == 'merman.authoring.flowchart.explicit_direction' &&
      rule.evidence.contains('docs/adr/0072-lint-rule-governance.md'))) {
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
    engine.setTextMeasurer((request) {
      if (request.text == 'Hello' &&
          request.wrapMode == MermanTextWrapMode.htmlLike) {
        return const MermanTextMeasureResult(
          width: 42,
          height: 24,
          lineCount: 1,
        );
      }
      return null;
    });
    final measuredSvg = engine.renderSvg(source);
    if (!measuredSvg.contains('<svg') || !measuredSvg.contains('Hello')) {
      throw StateError('reusable engine SVG smoke failed');
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
