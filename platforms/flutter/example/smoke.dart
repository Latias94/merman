import 'package:merman/merman.dart';

void _expect(bool condition, String message) {
  if (!condition) {
    throw StateError(message);
  }
}

void main(List<String> args) {
  if (args.length > 1) {
    throw ArgumentError('expected at most one native library path');
  }

  final iconPackSet = MermanIconPackSet.fromPacks([
    MermanIconPack(
      json:
          '{"icons":{"rocket":{"body":"<path data-icon=\\"flutter-smoke\\" d=\\"M0 0H16V16H0z\\"/>"}}}',
      registrationName: 'smoke',
    ),
  ]);
  var measureCalls = 0;
  final services = MermanEngineServices(
    iconPackSet: iconPackSet,
    textMeasurer: (_) {
      measureCalls += 1;
      return null;
    },
  );
  final engine = args.isEmpty
      ? MermanEngine(services: services)
      : MermanEngine.openPath(args.single, services: services);

  try {
    final svg = engine.renderSvg(
      'flowchart TD\nA@{ icon: "smoke:rocket", label: "Hello" } --> B[World]',
    );
    _expect(svg.contains('<svg'), 'SVG smoke failed');
    _expect(svg.contains('Hello'), 'SVG label smoke failed');
    _expect(svg.contains('flutter-smoke'), 'icon service smoke failed');
    _expect(measureCalls > 0, 'host text measurer was not called');
  } finally {
    engine.close();
  }

  print('merman Flutter smoke passed');
}
