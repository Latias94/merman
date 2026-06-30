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
    engine.setTextMeasurer(null);
  } finally {
    engine.close();
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
