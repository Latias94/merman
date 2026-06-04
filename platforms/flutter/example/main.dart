import 'package:merman/merman.dart';

void main(List<String> args) {
  final engine = args.isEmpty ? Merman.open() : Merman.openPath(args.single);
  const source = 'flowchart TD\nA[Hello] --> B[World]';

  final svg = engine.renderSvg(
    source,
    optionsJson: '{"svg":{"pipeline":"readable"}}',
  );
  final ascii = engine.renderAscii(source);
  final validation = engine.validate(source);

  print('Merman ${engine.packageVersion}');
  print('SVG bytes: ${svg.length}');
  print(ascii);
  print('Valid: ${validation.valid}');
}
