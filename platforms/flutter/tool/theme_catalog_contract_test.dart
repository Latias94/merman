import 'package:merman/merman.dart';

final _studio = <String, Object?>{
  'id': 'studio',
  'name': 'Studio',
  'description': 'A reading theme.',
  'category': 'reading',
  'appearances': ['light', 'dark'],
  'default_appearance': 'light',
  'swatches': [
    '#F8FAFC',
    '#FFFFFF',
    '#EFF4FA',
    '#2563EB',
    '#0F172A',
    '#BFDBFE',
  ],
  'future_field': {'label': 'retained'},
};

final _flowchart = <String, Object?>{
  'id': 'flowchart',
  'capabilities': 33,
  'future_field': true,
};

Map<String, Object?> _catalog() => {
  'schema_version': 2,
  'presets': [_studio],
  'mermaid_themes': ['default', 'dark'],
  'families': [
    _flowchart,
    {'id': 'gitGraph', 'capabilities': 0},
  ],
  'future_field': [42],
};

void main() => runThemeCatalogContractTests();

void runThemeCatalogContractTests() {
  final catalog = MermanThemeCatalog.fromJson(_catalog());
  final studio = catalog.presets.single;
  final flowchart = catalog.families.first;
  if (catalog.schemaVersion != 2 ||
      studio.id != 'studio' ||
      studio.name != 'Studio' ||
      studio.description != 'A reading theme.' ||
      studio.category != 'reading' ||
      studio.defaultAppearance != 'light' ||
      studio.appearances.join(',') != 'light,dark' ||
      studio.swatches.last != '#BFDBFE' ||
      catalog.mermaidThemes.join(',') != 'default,dark' ||
      flowchart.id != 'flowchart' ||
      flowchart.capabilities != 33 ||
      catalog.families.last.id != 'gitGraph' ||
      (studio.jsonObject['future_field'] as Map)['label'] != 'retained' ||
      flowchart.jsonObject['future_field'] != true ||
      (catalog.jsonObject['future_field'] as List).single != 42) {
    throw StateError('theme catalog lost fields or changed their order');
  }

  for (final mutate in <void Function()>[
    () => catalog.presets.clear(),
    () => catalog.families.clear(),
    () => catalog.mermaidThemes.clear(),
    () => studio.appearances.clear(),
    () => studio.swatches.clear(),
    () => (studio.jsonObject['future_field'] as Map).clear(),
    () => (catalog.jsonObject['future_field'] as List).clear(),
  ]) {
    try {
      mutate();
    } on UnsupportedError {
      continue;
    }
    throw StateError('theme catalog value was mutable');
  }

  final nonRendering = MermanThemeCatalog.fromJson({
    ..._catalog(),
    'presets': [],
    'families': [],
  });
  if (nonRendering.presets.isNotEmpty ||
      nonRendering.families.isNotEmpty ||
      !nonRendering.mermaidThemes.contains('default')) {
    throw StateError('non-rendering theme catalog did not retain Mermaid IDs');
  }

  for (final invalid in [
    {..._catalog(), 'schema_version': 1},
    _catalog()..remove('families'),
    {
      ..._catalog(),
      'presets': [_studio, _studio],
    },
    {
      ..._catalog(),
      'mermaid_themes': ['dark', 'dark'],
    },
    {
      ..._catalog(),
      'families': [_flowchart, _flowchart],
    },
    {
      ..._catalog(),
      'families': [
        {..._flowchart, 'capabilities': -1},
      ],
    },
    for (final fields in [
      {'appearances': []},
      {
        'appearances': ['dark'],
        'default_appearance': 'light',
      },
      {
        'appearances': ['light', 'light'],
      },
      {
        'swatches': ['#123456'],
      },
      {'swatches': List.filled(6, 'invalid')},
    ])
      {
        ..._catalog(),
        'presets': [
          {..._studio, ...fields},
        ],
      },
  ]) {
    try {
      MermanThemeCatalog.fromJson(invalid);
    } on MermanException {
      continue;
    }
    throw StateError('accepted malformed theme catalog: $invalid');
  }
  print('Theme v2 catalog Dart contract tests passed');
}
