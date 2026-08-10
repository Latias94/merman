import 'package:merman/merman.dart';

void _expect(bool condition, String message) {
  if (!condition) {
    throw StateError(message);
  }
}

void _expectMissingCapability(
  String capabilityId,
  void Function() operation,
) {
  try {
    operation();
  } on MermanMissingCapabilityException catch (error) {
    _expect(
      error.capabilityId == capabilityId,
      'missing capability was `${error.capabilityId}`, expected `$capabilityId`',
    );
    return;
  }
  throw StateError('expected missing capability `$capabilityId`');
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
    for (final capability in const [
      'analysis',
      'ascii',
      'layout-cytoscape',
      'layout-elk',
      'svg',
    ]) {
      _expect(
        engine.runtimeCatalog.supportsCapability(capability),
        'bundled native library is missing `$capability`',
      );
    }
    for (final capability in const [
      'jpeg',
      'math',
      'pdf',
      'png',
      'system-clock',
      'system-random',
      'system-timezone',
    ]) {
      _expect(
        !engine.runtimeCatalog.supportsCapability(capability),
        'bundled native library unexpectedly includes `$capability`',
      );
    }

    final ascii = engine.renderAscii('flowchart TD\nA --> B');
    _expect(ascii.contains('A'), 'ASCII smoke failed');
    final analysis = engine.analyzeJson('flowchart TD\nA --> B');
    _expect(analysis.isNotEmpty, 'analysis smoke failed');

    _expectMissingCapability(
      'png',
      () => engine.renderPng('flowchart TD\nA --> B'),
    );
    _expectMissingCapability(
      'jpeg',
      () => engine.renderJpeg('flowchart TD\nA --> B'),
    );
    _expectMissingCapability(
      'pdf',
      () => engine.renderPdf('flowchart TD\nA --> B'),
    );
    _expectMissingCapability(
      'math',
      () => engine.renderSvg('flowchart TD\nA["\$\$x^2\$\$"] --> B'),
    );
  } finally {
    engine.close();
  }

  print('merman Flutter smoke passed');
}
