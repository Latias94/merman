import 'package:merman/merman.dart';

void main(List<String> args) {
  final merman = args.isEmpty ? Merman.open() : Merman.openPath(args.single);
  const source = 'flowchart TD\nA[Hello] --> B[World]';
  final svg = merman.renderSvg(source);
  final ascii = merman.renderAscii(source);
  final validation = merman.validate(source);

  print('Merman ${merman.packageVersion}');
  print('SVG bytes: ${svg.length}');
  print(ascii);
  print('Valid: ${validation.valid}');
}
