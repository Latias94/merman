import 'dart:convert';
import 'dart:io';

import 'package:merman/merman.dart';
import 'package:merman/src/generated/binding_contract.dart' as binding;

final class _SemanticOperationFixture {
  const _SemanticOperationFixture({
    required this.operationId,
    required this.source,
    required this.uri,
    required this.options,
    required this.expectedMediaType,
    required this.expectedErrorKind,
    required this.payloadInvariants,
  });

  final String operationId;
  final String source;
  final String? uri;
  final Map<String, Object?>? options;
  final String? expectedMediaType;
  final String? expectedErrorKind;
  final List<String> payloadInvariants;
}

void main(List<String> args) {
  if (args.length > 1) {
    throw ArgumentError('expected at most one native library path');
  }
  final fixtures = _loadFixtures();
  final merman = args.isEmpty ? Merman.open() : Merman.openPath(args.single);
  for (var index = 0; index < fixtures.length; index += 1) {
    _runFixture(merman, fixtures[index], index);
  }
  _runGeneratedOperationMatrix(merman);
  print('Shared semantic operation fixture tests passed');
}

void _runGeneratedOperationMatrix(Merman merman) {
  final generatedIds = binding.mermanBindingOperationExpectations
      .map((expectation) => expectation.operationId)
      .toSet();
  final sdkIds = MermanOperation.knownValues
      .map((operation) => operation.operationId)
      .toSet();
  _expect(
    generatedIds.length == 13 &&
        generatedIds.length == sdkIds.length &&
        generatedIds.containsAll(sdkIds) &&
        sdkIds.containsAll(generatedIds),
    'generated Dart invocation mapping must cover the shared 13-operation matrix',
  );

  const diagramSource = 'flowchart TD\nA --> B';
  const documentSource = 'Intro\n```mermaid\nflowchart TD\nA --> B\n```\n';
  for (final expectation in binding.mermanBindingOperationExpectations) {
    final operation = MermanOperation.fromOperationId(expectation.operationId);
    try {
      final result = merman.execute(
        operation,
        expectation.requiresUri ? documentSource : diagramSource,
        uri: expectation.requiresUri ? 'file:///tmp/matrix.md' : null,
      );
      _expect(
        merman.runtimeCatalog.supportsOperation(expectation.operationId),
        '`${expectation.operationId}` succeeded without runtime advertisement',
      );
      _expect(
        result.mediaType == expectation.mediaType &&
            result.metadata.operationId == expectation.operationId &&
            result.metadata.version == expectation.metadataSchemaVersion,
        '`${expectation.operationId}` violated the generated operation contract',
      );
    } on MermanMissingCapabilityException catch (error) {
      _expect(
        !merman.runtimeCatalog.supportsOperation(expectation.operationId) &&
            expectation.availabilityCapabilityId != null &&
            error.capabilityId == expectation.availabilityCapabilityId,
        '`${expectation.operationId}` returned the wrong unavailable contract',
      );
    }
  }
}

void _runFixture(
  Merman merman,
  _SemanticOperationFixture fixture,
  int index,
) {
  final label = 'fixture $index operation `${fixture.operationId}`';
  try {
    final operation = _operationForId(fixture.operationId);
    final result = merman.execute(
      operation,
      fixture.source,
      uri: fixture.uri,
      optionsJson: fixture.options == null ? null : jsonEncode(fixture.options),
    );
    _expect(
      fixture.expectedMediaType != null,
      '$label unexpectedly succeeded',
    );
    _expect(
      result.operation.operationId == fixture.operationId,
      '$label returned the wrong operation',
    );
    _expect(
      result.mediaType == fixture.expectedMediaType,
      '$label returned the wrong media type',
    );
    _assertSuccessInvariants(fixture, result, label);
  } on MermanException catch (error) {
    _expect(
      fixture.expectedErrorKind != null,
      '$label unexpectedly failed: $error',
    );
    _expect(
      error.kind.wireName == fixture.expectedErrorKind,
      '$label returned error kind `${error.kind.wireName}`',
    );
    _expect(error.capabilityId == null, '$label returned a capability ID');
    for (final invariant in fixture.payloadInvariants) {
      switch (invariant) {
        case 'error-message-nonempty':
          _expect(error.message.isNotEmpty, '$label returned an empty message');
        default:
          throw StateError(
              '$label has unsupported error invariant `$invariant`');
      }
    }
  }
}

void _assertSuccessInvariants(
  _SemanticOperationFixture fixture,
  MermanOperationResult result,
  String label,
) {
  for (final invariant in fixture.payloadInvariants) {
    switch (invariant) {
      case 'nonempty':
        _expect(result.bytes.isNotEmpty, '$label returned an empty payload');
      case 'utf8':
        result.utf8Text;
      case 'json-object':
        result.jsonObject;
      case 'svg-root':
        _expect(
          result.utf8Text.trimLeft().startsWith('<svg'),
          '$label did not return an SVG root',
        );
      case 'metadata-operation-id':
        _expect(
          result.metadata.operationId == fixture.operationId,
          '$label metadata returned the wrong operation',
        );
      default:
        throw StateError(
            '$label has unsupported success invariant `$invariant`');
    }
  }
}

MermanOperation _operationForId(String operationId) {
  return MermanOperation.fromOperationId(operationId);
}

List<_SemanticOperationFixture> _loadFixtures() {
  final fixtureUri = Platform.script.resolve(
    '../../../fixtures/bindings/assets/semantic-operations-v1.json',
  );
  final root = _object(
    jsonDecode(File.fromUri(fixtureUri).readAsStringSync()),
    'fixture root',
  );
  _expect(
    root['schema_version'] is int && root['schema_version'] == 1,
    'unsupported semantic operation fixture schema',
  );
  final rawCases = root['cases'];
  _expect(rawCases is List<Object?>, 'cases must be an array');
  final cases = rawCases! as List<Object?>;

  return [
    for (var index = 0; index < cases.length; index += 1)
      _fixtureCase(cases[index], index),
  ];
}

_SemanticOperationFixture _fixtureCase(Object? raw, int index) {
  final label = 'fixture case $index';
  final value = _object(raw, label);
  final options = value.containsKey('options')
      ? _object(value['options'], '$label.options')
      : null;

  return _SemanticOperationFixture(
    operationId: value['operation_id']! as String,
    source: value['source']! as String,
    uri: value['uri'] as String?,
    options: options,
    expectedMediaType: value['expected_media_type'] as String?,
    expectedErrorKind: value['expected_error_kind'] as String?,
    payloadInvariants: _stringArray(value['payload_invariants']),
  );
}

Map<String, Object?> _object(Object? value, String label) {
  _expect(value is Map<Object?, Object?>, '$label must be an object');
  final result = <String, Object?>{};
  for (final entry in (value! as Map<Object?, Object?>).entries) {
    _expect(entry.key is String, '$label keys must be strings');
    result[entry.key! as String] = entry.value;
  }
  return result;
}

List<String> _stringArray(Object? value) =>
    (value! as List<Object?>).cast<String>();

void _expect(bool condition, String message) {
  if (!condition) {
    throw StateError(message);
  }
}
