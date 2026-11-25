import 'dart:convert';

import 'package:merman/merman.dart';

void _require(bool condition, String message) {
  if (!condition) throw StateError(message);
}

void main(List<String> args) {
  if (args.length > 1) {
    throw ArgumentError(
      'usage: theme_authoring_smoke.dart [native-library-path]',
    );
  }
  final oneShot = args.isEmpty ? Merman.open() : Merman.openPath(args.single);
  final catalog = oneShot.themeCatalog();
  _require(
    catalog.schemaVersion == 2 &&
        catalog.presets.length == 13 &&
        catalog.mermaidThemes.length == 11 &&
        catalog.families.any((family) => family.id == 'gitGraph') &&
        catalog.families.any(
          (family) => family.id == 'flowchart' && family.capabilities > 0,
        ),
    'unexpected theme v2 catalog',
  );
  final studio = catalog.presets.singleWhere((preset) => preset.id == 'studio');
  _require(
    studio.appearances.join(',') == 'light,dark' &&
        studio.defaultAppearance == 'light' &&
        studio.swatches.length == 6,
    'Studio lost its appearance and swatch data',
  );

  const baseOptions = '{"theme":"studio"}';
  final engine = args.isEmpty
      ? MermanEngine(optionsJson: baseOptions)
      : MermanEngine.openPath(args.single, optionsJson: baseOptions);
  const source =
      '---\nconfig:\n  layout: dagre\n---\n'
      'flowchart LR\nA[Review] --> B[Publish]';
  try {
    for (final (name, execute) in [
      ('one-shot', oneShot.execute),
      ('reusable', engine.execute),
    ]) {
      final exported = execute(MermanOperation.exportThemePreset, studio.id);
      final document = exported.jsonObject;
      _require(
        exported.mediaType == 'application/json' &&
            document['schema_version'] == 2 &&
            document['id'] == studio.id &&
            (document['appearances'] as Map).keys.toSet().containsAll([
              'light',
              'dark',
            ]),
        '$name did not export the editable preset',
      );
      for (final input in [document, exported.utf8Text]) {
        final resolved = execute(
          MermanOperation.resolveTheme,
          jsonEncode({'document': input, 'appearance': 'dark'}),
          optionsJson: '{}',
        ).jsonObject;
        _require(
          resolved['ok'] == true &&
              (resolved['resolved'] as Map)['appearance'] == 'dark' &&
              resolved['notices'] is List,
          '$name could not resolve Studio Dark',
        );
      }

      for (final (text, column, path, message) in [
        (
          '{\n  "schema_version": 2,\n  "extends": ]\n}',
          14,
          'extends',
          'expected value',
        ),
        (
          '{\n  "schema_version": 2,\n  "schema_version": 2\n}',
          18,
          '.',
          'duplicate key',
        ),
      ]) {
        final diagnostic = execute(
          MermanOperation.resolveTheme,
          jsonEncode({'document': text}),
        ).jsonObject;
        _require(
          diagnostic['ok'] == false &&
              diagnostic['line'] == 3 &&
              diagnostic['column'] == column &&
              diagnostic['path'] == path &&
              (diagnostic['message'] as String).contains(message),
          '$name lost document-relative diagnostics: $diagnostic',
        );
      }

      try {
        execute(
          MermanOperation.exportThemePreset,
          studio.id,
          optionsJson: '{"runtime_policy":"deterministic"}',
        );
        throw StateError(
          '$name accepted a nonempty theme operation options object',
        );
      } on MermanException catch (error) {
        _require(
          error.message.contains('does not accept options_json fields'),
          '$name: $error',
        );
      }

      final rendered = execute(
        MermanOperation.svg,
        source,
        optionsJson: jsonEncode({
          'theme': {'document': document, 'appearance': 'dark'},
          'background': '#123456',
        }),
      );
      final outcome = rendered.metadata.themeOutcome;
      _require(
        outcome?['mode'] == 'merman_native' &&
            outcome?['appearance'] == 'dark' &&
            outcome?['notices'] is List,
        '$name lost the SVG theme outcome',
      );
      _require(
        RegExp(
          r'<g class="merman-theme-canvas"[^>]*><rect [^>]*\bfill="#123456"',
        ).hasMatch(rendered.utf8Text),
        '$name background did not replace the theme canvas',
      );
    }

    final inherited = engine
        .execute(MermanOperation.svg, source)
        .metadata
        .themeOutcome;
    _require(
      inherited?['preset_id'] == 'studio' &&
          inherited?['appearance'] == 'light',
      'an inline theme changed the reusable baseline',
    );
    final cleared = engine.execute(
      MermanOperation.svg,
      source,
      optionsJson: '{"theme":"none"}',
    );
    _require(
      cleared.metadata.themeOutcome?['mode'] == 'mermaid_parity',
      'theme none did not clear the preset',
    );
  } finally {
    engine.close();
  }
  print('Flutter Theme v2 authoring smoke passed');
}
