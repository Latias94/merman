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

  final semantic = merman.parseJson(source);
  if (semantic['type'] != 'flowchart-v2') {
    throw StateError('parseJson smoke failed');
  }

  final layout = merman.layoutJson(source);
  if (!layout.containsKey('meta') || !layout.containsKey('layout')) {
    throw StateError('layoutJson smoke failed');
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
