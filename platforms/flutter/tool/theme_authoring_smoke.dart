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

void _checkError(Execute execute, Map<String, dynamic> vector, String mode) {
  final context = '$mode/${vector['id']}';
  try {
    execute(
      MermanOperation.materializeThemeJson,
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
    }
  } finally {
    engine.close();
  }
  print(
    'Flutter Native Assets theme authoring passed: '
    '2 materializations, ${support.length} support queries, ${errors.length} errors per consumer',
  );
}
