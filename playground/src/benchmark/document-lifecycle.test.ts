import assert from "node:assert/strict";
import test from "node:test";

import {
  createBenchmarkDocumentLifecycle,
  type BenchmarkDocumentLifecycleSignal,
  type BenchmarkLifecycleEventTarget,
} from "./document-lifecycle.ts";

test("projects browser lifecycle events into closed immutable signals", () => {
  const documentTarget = fakeTarget();
  const windowTarget = fakeTarget();
  let visibilityState = "visible";
  const lifecycle = createBenchmarkDocumentLifecycle({
    documentTarget,
    getVisibilityState: () => visibilityState,
    windowTarget,
  });
  const signals: BenchmarkDocumentLifecycleSignal[] = [];
  const unsubscribe = lifecycle.subscribe((signal) => signals.push(signal));

  documentTarget.dispatch("visibilitychange", {});
  visibilityState = "hidden";
  documentTarget.dispatch("visibilitychange", {});
  documentTarget.dispatch("freeze", {});
  documentTarget.dispatch("resume", {});
  windowTarget.dispatch("pagehide", { persisted: true });
  windowTarget.dispatch("pageshow", { persisted: false });

  assert.deepEqual(signals, [
    { kind: "visibility-hidden", visibilityState: "hidden" },
    { kind: "freeze", visibilityState: "hidden" },
    { kind: "resume", visibilityState: "hidden" },
    { kind: "pagehide", persisted: true, visibilityState: "hidden" },
    { kind: "pageshow", persisted: false, visibilityState: "hidden" },
  ]);
  assert.ok(signals.every(Object.isFrozen));
  assert.equal(lifecycle.getVisibilityState(), "hidden");

  unsubscribe();
  unsubscribe();
  assert.equal(documentTarget.listenerCount(), 0);
  assert.equal(windowTarget.listenerCount(), 0);
});

function fakeTarget(): BenchmarkLifecycleEventTarget & {
  dispatch(type: string, event: unknown): void;
  listenerCount(): number;
} {
  const listeners = new Map<string, Set<(event: unknown) => void>>();
  return {
    addEventListener(type, listener) {
      const bucket = listeners.get(type) ?? new Set();
      bucket.add(listener);
      listeners.set(type, bucket);
    },
    removeEventListener(type, listener) {
      listeners.get(type)?.delete(listener);
    },
    dispatch(type, event) {
      for (const listener of listeners.get(type) ?? []) listener(event);
    },
    listenerCount() {
      return [...listeners.values()].reduce(
        (total, bucket) => total + bucket.size,
        0
      );
    },
  };
}
