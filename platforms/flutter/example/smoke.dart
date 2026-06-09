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
  if (!merman.asciiSupportedDiagrams().contains('sequence')) {
    throw StateError('asciiSupportedDiagrams smoke failed');
  }
  if (!merman.supportedThemes().contains('default')) {
    throw StateError('themes smoke failed');
  }
  if (!merman.supportedHostThemePresets().contains('one-dark')) {
    throw StateError('host theme presets smoke failed');
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
