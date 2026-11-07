import 'package:merman/merman.dart';

Map<String, Object?> _appearance(String mode) => {
  'appearance': mode,
  'concrete_preset_id': 'editor-$mode',
  'available': true,
  'availability_reason_ids': <String>[],
  'family_designs': <Object?>[],
  'preview_colors': <String>[],
  'application_notes': <Object?>[],
  'qualified_cells': <Object?>[],
  'future_field': {
    'scope': [mode],
  },
};

Map<String, Object?> _preset() => {
  'id': 'editor',
  'display_name': 'Editor',
  'supported_appearances': ['light', 'dark'],
  'default_appearance': 'light',
  'appearances': [_appearance('light'), _appearance('dark')],
  'maturity': 'alpha',
  'available': true,
  'availability_reason_ids': <String>[],
  'license_expression': 'MIT OR Apache-2.0',
  'required_attribution': null,
  'export_kind': 'design',
  'future_field': {'shared': true},
};

Map<String, Object?> _catalog() => {
  'schema_version': 1,
  'structured_spec_available': true,
  'recipe_schema_version': 1,
  'authoring_request_schema_version': 1,
  'authoring_result_schema_version': 1,
  'supported_output_ids': ['svg'],
  'presets': [_preset()],
  'mermaid_theme_designs': [
    {
      'id': 'default',
      'display_name': 'Default',
      'default_appearance': 'light',
      'appearances': [
        {'appearance': 'light', 'theme_id': 'default'},
      ],
    },
    {
      'id': 'dark',
      'display_name': 'Dark',
      'default_appearance': 'dark',
      'appearances': [
        {'appearance': 'dark', 'theme_id': 'dark'},
      ],
    },
    {
      'id': 'neo',
      'display_name': 'Neo',
      'default_appearance': 'light',
      'appearances': [
        {'appearance': 'light', 'theme_id': 'neo'},
        {'appearance': 'dark', 'theme_id': 'neo-dark'},
      ],
    },
  ],
  'known_capability_ids': ['semantic-rules'],
  'known_text_capability_ids': <String>[],
  'known_font_container_ids': <String>[],
  'known_font_source_ids': <String>[],
  'known_semantic_target_ids': ['node'],
  'known_variant_ids': <String>[],
  'resource_limits': <Object?>[],
  'future_field': {'enabled': true},
};

void _expect(bool condition, String message) {
  if (!condition) throw StateError(message);
}

void _reject(void Function() action) {
  try {
    action();
  } on MermanException catch (error) {
    _expect(
      error.codeName == 'DART_NATIVE_CONTRACT_ERROR',
      'unexpected error: $error',
    );
    return;
  }
  throw StateError('malformed paired catalog was accepted');
}

void runPairedThemeCatalogContractTests() {
  final source = _catalog();
  final catalog = MermanThemeCatalog.fromJson(source);
  final preset = catalog.presets.single;
  _expect(
    catalog.recipeSchemaVersion == 1 &&
        catalog.authoringRequestSchemaVersion == 1 &&
        catalog.authoringResultSchemaVersion == 1 &&
        preset.supportedAppearances.join(',') == 'light,dark' &&
        preset.appearances.last.concretePresetId == 'editor-dark',
    'one theme must retain its concrete appearances and schema discovery',
  );
  _expect(
    catalog.mermaidThemeDesigns[0].id == 'default' &&
        catalog.mermaidThemeDesigns[1].id == 'dark' &&
        catalog.mermaidThemeDesigns[2].appearances.last.themeId == 'neo-dark',
    'upstream groups must retain literal IDs and independent Default/Dark designs',
  );
  final rawScope =
      (preset.appearances.last.jsonObject['future_field'] as Map)['scope']
          as List;
  (source['future_field'] as Map)['enabled'] = false;
  _expect(
    (catalog.jsonObject['future_field'] as Map)['enabled'] == true &&
        (preset.jsonObject['future_field'] as Map)['shared'] == true &&
        rawScope.single == 'dark',
    'typed metadata must retain independent immutable raw extension fields',
  );
  try {
    rawScope.add('changed');
    throw StateError('raw extension data remained mutable');
  } on UnsupportedError {
    _expect(rawScope.single == 'dark', 'extension data changed');
  }
  for (final mutate in <void Function(Map<String, Object?>)>[
    (value) => value['supported_appearances'] = <String>[],
    (value) => value['supported_appearances'] = ['light', 'light'],
    (value) => value['supported_appearances'] = ['dark', 'light'],
    (value) => value['default_appearance'] = 'auto',
    (value) => value['default_appearance'] = null,
    (value) => value['appearances'] = <Object?>[],
    (value) => (value['appearances'] as List).last['appearance'] = 'light',
    (value) => (value['appearances'] as List).last['concrete_preset_id'] =
        'editor-light',
    (value) => (value['appearances'] as List).last['available'] = false,
    (value) => value['available'] = false,
    (value) => value['export_kind'] = 'complete_spec',
  ]) {
    final value = _preset();
    mutate(value);
    _reject(() => MermanThemePreset.fromJson(value));
  }
  final rejectedPair = _preset();
  rejectedPair['available'] = false;
  rejectedPair['availability_reason_ids'] = [
    'theme-preset.resource-policy-rejected',
  ];
  for (final mode in rejectedPair['appearances'] as List) {
    mode['available'] = false;
    mode['availability_reason_ids'] = ['theme-preset.resource-policy-rejected'];
  }
  final rejected = MermanThemePreset.fromJson(rejectedPair);
  _expect(
    !rejected.available &&
        rejected.appearances.every((mode) => !mode.available),
    'availability reasons must preserve namespaced IDs and the complete-design gate',
  );
  for (final field in [
    'recipe_schema_version',
    'authoring_request_schema_version',
    'authoring_result_schema_version',
  ]) {
    _reject(() => MermanThemeCatalog.fromJson({..._catalog(), field: null}));
  }
  final empty = MermanThemeCatalog.fromJson({
    ..._catalog(),
    'structured_spec_available': false,
    'recipe_schema_version': null,
    'authoring_request_schema_version': null,
    'authoring_result_schema_version': null,
    'presets': <Object?>[],
    'mermaid_theme_designs': <Object?>[],
    'supported_output_ids': <String>[],
    'known_capability_ids': <String>[],
    'known_semantic_target_ids': <String>[],
  });
  _expect(
    !empty.structuredSpecAvailable && empty.recipeSchemaVersion == null,
    'capability-reduced artifacts must not promise theme authoring schemas',
  );
  final duplicate = _catalog();
  (duplicate['presets'] as List).add(_preset());
  _reject(() => MermanThemeCatalog.fromJson(duplicate));
  final badMermaid = _catalog();
  (badMermaid['mermaid_theme_designs'] as List).last['appearances'] = [
    {'appearance': 'dark', 'theme_id': 'neo-dark'},
  ];
  _reject(() => MermanThemeCatalog.fromJson(badMermaid));
}

void main() {
  runPairedThemeCatalogContractTests();
  print('Paired theme catalog Dart contract tests passed');
}
