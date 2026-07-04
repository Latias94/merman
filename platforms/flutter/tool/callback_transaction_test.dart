import 'package:merman/src/merman_ffi.dart';

void main() {
  replaceFailureKeepsPreviousCallbackAlive();
  clearFailureKeepsPreviousCallbackAlive();
  replaceSuccessClosesPreviousCallback();
  takeCallbackClearsStateWithoutClosing();
  lifecycleReentrantCallThrowsStableError();
  lifecycleMutationDuringNativeCallThrowsStableError();
  lifecycleCloseDuringNativeCallDefersFreeUntilFinish();
  lifecycleCallsAfterCloseThrowStableError();

  print('callback transaction tests passed');
}

void replaceFailureKeepsPreviousCallbackAlive() {
  final registration = testRegistration();
  final first = FakeCallback('first');
  final second = FakeCallback('second');
  registration.replace(
    callback: first,
    measurer: ignoreMeasure,
    installNative: (_) {},
  );

  expectThrows<StateError>(() {
    registration.replace(
      callback: second,
      measurer: ignoreMeasure,
      installNative: (_) {
        throw StateError('native install failed');
      },
    );
  });

  expectSame(
      registration.callback, first, 'previous callback should remain active');
  expectFalse(
      first.closed, 'previous callback must stay open after replace failure');
  expectTrue(second.closed, 'failed replacement callback must be closed');
  expectNotNull(
      registration.measurer, 'previous measurer should remain active');
}

void clearFailureKeepsPreviousCallbackAlive() {
  final registration = testRegistration();
  final first = FakeCallback('first');
  registration.replace(
    callback: first,
    measurer: ignoreMeasure,
    installNative: (_) {},
  );

  expectThrows<StateError>(() {
    registration.clear(clearNative: () {
      throw StateError('native clear failed');
    });
  });

  expectSame(registration.callback, first,
      'callback should remain active after clear failure');
  expectFalse(first.closed, 'callback must stay open after clear failure');
  expectNotNull(registration.measurer,
      'measurer should remain active after clear failure');
}

void replaceSuccessClosesPreviousCallback() {
  final registration = testRegistration();
  final first = FakeCallback('first');
  final second = FakeCallback('second');
  registration.replace(
    callback: first,
    measurer: ignoreMeasure,
    installNative: (_) {},
  );
  registration.replace(
    callback: second,
    measurer: ignoreMeasure,
    installNative: (_) {},
  );

  expectSame(
      registration.callback, second, 'new callback should become active');
  expectTrue(first.closed,
      'previous callback should close after successful replacement');
  expectFalse(second.closed, 'active callback must remain open');
}

void takeCallbackClearsStateWithoutClosing() {
  final registration = testRegistration();
  final first = FakeCallback('first');
  registration.replace(
    callback: first,
    measurer: ignoreMeasure,
    installNative: (_) {},
  );

  final taken = registration.takeCallback();

  expectSame(taken, first, 'takeCallback should return the active callback');
  expectNull(registration.callback, 'takeCallback should clear callback state');
  expectNull(registration.measurer, 'takeCallback should clear measurer state');
  expectFalse(first.closed, 'takeCallback must not close before native free');

  registration.closeDetached(taken);
  expectTrue(first.closed, 'detached callback should close explicitly');
}

void lifecycleReentrantCallThrowsStableError() {
  final lifecycle = testLifecycle();

  final result = lifecycle.withNativeCall((handle) {
    expectEquals(handle, 'engine', 'active native call should use handle');
    expectMermanException('DART_ENGINE_REENTERED', () {
      lifecycle.withNativeCall((_) {});
    });
    return 42;
  });

  expectEquals(result, 42, 'outer native call should complete');
  expectFalse(lifecycle.isClosed, 'reentrant failure must not close engine');
}

void lifecycleMutationDuringNativeCallThrowsStableError() {
  final lifecycle = testLifecycle();

  lifecycle.withNativeCall((_) {
    expectMermanException('DART_ENGINE_REENTERED', () {
      lifecycle.openHandle;
    });
  });

  expectFalse(lifecycle.isClosed, 'mutation failure must not close engine');
}

void lifecycleCloseDuringNativeCallDefersFreeUntilFinish() {
  final closedHandles = <String>[];
  final lifecycle = testLifecycle(onClose: closedHandles.add);

  final result = lifecycle.withNativeCall((handle) {
    expectEquals(handle, 'engine', 'native call should receive live handle');
    lifecycle.close();
    expectTrue(lifecycle.closeRequested,
        'close inside a native callback should be deferred');
    expectFalse(lifecycle.isClosed,
        'engine must stay open until the outer native call exits');
    expectEquals(closedHandles.length, 0, 'native handle freed too early');
    return 'rendered';
  });

  expectEquals(result, 'rendered', 'outer native call result should survive');
  expectTrue(lifecycle.isClosed, 'engine should close after native call exit');
  expectListEquals(
      closedHandles, ['engine'], 'native handle should be freed exactly once');
}

void lifecycleCallsAfterCloseThrowStableError() {
  final closedHandles = <String>[];
  final lifecycle = testLifecycle(onClose: closedHandles.add);

  lifecycle.close();
  lifecycle.close();

  expectTrue(lifecycle.isClosed, 'close should mark engine closed');
  expectListEquals(closedHandles, ['engine'], 'close should be idempotent');
  expectMermanException('DART_ENGINE_CLOSED', () {
    lifecycle.withNativeCall((_) {});
  });
  expectMermanException('DART_ENGINE_CLOSED', () {
    lifecycle.openHandle;
  });
}

MermanTextMeasureCallbackRegistration<FakeCallback> testRegistration() {
  return MermanTextMeasureCallbackRegistration(
    closeCallback: (callback) {
      callback.closed = true;
    },
  );
}

MermanReusableEngineLifecycle<String> testLifecycle({
  void Function(String handle)? onClose,
}) {
  return MermanReusableEngineLifecycle(
    initialHandle: 'engine',
    closedHandle: 'closed',
    isClosed: (handle) => handle == 'closed',
    closeHandle: onClose ?? (_) {},
  );
}

MermanTextMeasureResult? ignoreMeasure(MermanTextMeasureRequest request) =>
    null;

class FakeCallback {
  FakeCallback(this.name);

  final String name;
  bool closed = false;
}

void expectTrue(bool value, String message) {
  if (!value) {
    throw StateError(message);
  }
}

void expectFalse(bool value, String message) {
  expectTrue(!value, message);
}

void expectNull(Object? value, String message) {
  if (value != null) {
    throw StateError('$message: got $value');
  }
}

void expectNotNull(Object? value, String message) {
  if (value == null) {
    throw StateError(message);
  }
}

void expectSame(Object? actual, Object? expected, String message) {
  if (!identical(actual, expected)) {
    throw StateError('$message: got $actual, expected $expected');
  }
}

void expectEquals(Object? actual, Object? expected, String message) {
  if (actual != expected) {
    throw StateError('$message: got $actual, expected $expected');
  }
}

void expectListEquals<T>(List<T> actual, List<T> expected, String message) {
  if (actual.length != expected.length) {
    throw StateError('$message: got $actual, expected $expected');
  }
  for (var index = 0; index < actual.length; index += 1) {
    if (actual[index] != expected[index]) {
      throw StateError('$message: got $actual, expected $expected');
    }
  }
}

void expectThrows<T extends Object>(void Function() body) {
  try {
    body();
  } catch (error) {
    if (error is T) {
      return;
    }
    throw StateError('expected $T, got $error');
  }
  throw StateError('expected $T to be thrown');
}

void expectMermanException(String codeName, void Function() body) {
  try {
    body();
  } catch (error) {
    if (error is MermanException && error.codeName == codeName) {
      return;
    }
    throw StateError('expected $codeName, got $error');
  }
  throw StateError('expected $codeName to be thrown');
}
