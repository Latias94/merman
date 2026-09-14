import 'dart:convert';
import 'dart:io';

import 'package:merman/merman.dart';

typedef Execute =
    MermanOperationResult Function(
      MermanOperation operation,
      String source, {
      String? optionsJson,
    });

bool _sameJson(Object? actual, Object? expected) {
  if (actual is Map && expected is Map) {
    return actual.length == expected.length &&
        expected.keys.every(
          (key) =>
              actual.containsKey(key) && _sameJson(actual[key], expected[key]),
        );
  }
  if (actual is List && expected is List) {
    if (actual.length != expected.length) return false;
    for (var i = 0; i < actual.length; i++) {
      if (!_sameJson(actual[i], expected[i])) return false;
    }
    return true;
  }
  return actual == expected;
}

void _expectJson(Object? actual, Object? expected, String context) {
  if (!_sameJson(actual, expected)) {
    throw StateError(
      '$context: expected ${jsonEncode(expected)}, got ${jsonEncode(actual)}',
    );
  }
}

void _checkError(
  Execute execute,
  Map<String, dynamic> vector,
  String mode, {
  MermanOperation operation = MermanOperation.materializeThemeJson,
}) {
  final context = '$mode/${vector['id']}';
  try {
    execute(
      operation,
      vector['source'] as String,
      optionsJson: vector['options_json'] as String?,
    );
  } on MermanException catch (error) {
    _expectJson(
      error.codeName,
      vector['native_status_name'],
      '$context status',
    );
    _expectJson(
      error.details?['resource'],
      vector['resource'],
      '$context resource',
    );
    if (vector['theme_authoring'] == null) {
      _expectJson(
        error.details?['theme_authoring'],
        null,
        '$context authoring',
      );
      return;
    }
    // Copy the immutable native envelope before removing explanatory messages.
    final authoring = jsonDecode(jsonEncode(error.details?['theme_authoring']));
    if (authoring is! Map || authoring['diagnostics'] is! List) {
      throw StateError('$context: missing authoring diagnostics');
    }
    for (final diagnostic in authoring['diagnostics'] as List) {
      final message = (diagnostic as Map).remove('message');
      if (message is! String || message.trim().isEmpty) {
        throw StateError('$context: missing diagnostic message');
      }
    }
    _expectJson(authoring, vector['theme_authoring'], '$context authoring');
    return;
  }
  throw StateError('$context: expected an authoring error');
}

void main() {
  // Repository fixtures are deliberately excluded from the published Dart package.
  final fixtures = Platform.script.resolve(
    '../../../crates/merman-theme-authoring-fixtures/fixtures/authoring-v1/',
  );
  String read(String name) =>
      File.fromUri(fixtures.resolve(name)).readAsStringSync();
  final errors = jsonDecode(read('errors.json')) as List;
  final support = jsonDecode(read('support.json')) as List;
  final oneShot = Merman.open();
  final engine = MermanEngine();
  try {
    final consumers = <String, Execute>{
      'one-shot': oneShot.execute,
      'reusable': engine.execute,
    };
    for (final consumer in consumers.entries) {
      final execute = consumer.value;
      for (final name in ['light', 'dark']) {
        final result = execute(
          MermanOperation.materializeThemeJson,
          read('$name.definition.json'),
        );
        _expectJson(
          result.mediaType,
          'application/json',
          '${consumer.key}/$name media',
        );
        _expectJson(result.jsonObject, {
          'schema_version': 1,
          'authoring_schema_version': 1,
          'expansion_version': 1,
          'spec_schema_version': 1,
          'spec': jsonDecode(read('$name.spec.canonical.json')),
        }, '${consumer.key}/$name materialization');
      }
      for (final vector in support) {
        final result = execute(
          MermanOperation.describeThemeSupportJson,
          jsonEncode(vector['query']),
        );
        _expectJson(
          result.mediaType,
          'application/json',
          '${consumer.key}/${vector['id']} media',
        );
        _expectJson(
          result.jsonObject,
          vector['expected'],
          '${consumer.key}/${vector['id']}',
        );
      }
      for (final vector in errors) {
        _checkError(execute, vector as Map<String, dynamic>, consumer.key);
      }
      // All authoring operations must reach theme admission before generic engine limits.
      final encodedLimit =
          (errors.singleWhere((vector) => vector['id'] == 'encoded-byte-limit')
                  as Map)
              .cast<String, dynamic>();
      for (final operation in [
        MermanOperation.describeThemeSupportJson,
        MermanOperation.exportThemePresetJson,
      ]) {
        _checkError(
          execute,
          {...encodedLimit, 'theme_authoring': null},
          '${consumer.key}/${operation.operationId}',
          operation: operation,
        );
      }
      final budgetedInputs = <MermanOperation, String>{
        MermanOperation.materializeThemeJson: read('light.definition.json'),
        MermanOperation.describeThemeSupportJson: jsonEncode(
          support.first['query'],
        ),
        MermanOperation.exportThemePresetJson: 'editor-light',
      };
      for (final request in budgetedInputs.entries) {
        final baseline = execute(request.key, request.value);
        // Preset export also admits the expanded recipe, so its budget must cover that input.
        final encodedBudget =
            request.key == MermanOperation.exportThemePresetJson
            ? utf8.encode(jsonEncode(baseline.jsonObject)).length
            : utf8.encode(request.value).length;
        MermanOperationResult bounded;
        try {
          bounded = execute(
            request.key,
            request.value,
            optionsJson: jsonEncode({
              'resources': {
                'profile': 'constrained',
                'limits': {'max_theme_encoded_bytes': encodedBudget},
              },
            }),
          );
        } on MermanException catch (error) {
          throw StateError(
            '${consumer.key}/${request.key.operationId}: ${error.details}',
          );
        }
        _expectJson(
          bounded.mediaType,
          baseline.mediaType,
          '${consumer.key}/${request.key.operationId} bounded media',
        );
        _expectJson(
          bounded.jsonObject,
          baseline.jsonObject,
          '${consumer.key}/${request.key.operationId} encoded byte budget',
        );
      }
    }
  } finally {
    engine.close();
  }
  print(
    'Flutter Native Assets theme authoring passed: '
    '2 materializations, ${support.length} support queries, '
    '${errors.length + 2} errors, 3 budgeted operations per consumer',
  );
}
