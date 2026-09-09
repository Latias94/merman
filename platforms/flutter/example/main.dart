import 'package:merman/merman.dart';

void main(List<String> args) {
  final merman = args.isEmpty ? Merman.open() : Merman.openPath(args.single);
  const source = 'flowchart TD\nA[Hello] --> B[World]';
  final svg = merman.renderSvg(source);
  final ascii = merman.renderAscii(source);
  final validation = merman.validate(source);
  final drawingListResult = merman.execute(
    MermanOperation.drawingListJson,
    'info',
  );
  final drawingList = drawingListResult.jsonObject;
  final commands = drawingList['commands'];
  if (drawingListResult.mediaType !=
          'application/vnd.merman.drawing-list+json;version=1' ||
      drawingList['version'] != 1 ||
      drawingList['coordinate_system'] != 'logical_pixels_y_down' ||
      commands is! List<Object?> ||
      commands.isEmpty) {
    throw StateError('DrawingList smoke failed');
  }

  print('Merman ${merman.packageVersion}');
  print('SVG bytes: ${svg.length}');
  print('DrawingList commands: ${commands.length}');
  print(ascii);
  print('Valid: ${validation.valid}');
}
