import assert from "node:assert/strict";
import test from "node:test";

import {
  BenchmarkStageTimeoutError,
  createBenchmarkProgressGate,
  createBenchmarkStageWatchdog,
  type BenchmarkStageTimer,
} from "./stage-watchdog.ts";

test("progress gate accepts complete cold and warm execution paths", () => {
  const merman = createBenchmarkProgressGate({
    engine: "merman",
    intentKind: "cold-measured",
  });
  for (const event of [
    "fonts_wait_start",
    "adapter_import_start",
    "adapter_import_end",
    "fonts_wait_end",
    "engine_import_start",
    "resource_acquire_start",
    "resource_acquire_end",
    "engine_import_end",
    "initialize_start",
    "initialize_end",
    "render_start",
    "budgeted_svg_ready",
    "isolated_dom_inserted",
    "isolated_layout_box_ready",
    "isolated_presentation_ready",
  ] as const) {
    merman.observe(event);
  }
  assert.doesNotThrow(() => merman.assertComplete());

  const mermaid = createBenchmarkProgressGate({
    engine: "mermaid",
    intentKind: "cold-measured",
  });
  for (const event of [
    "fonts_wait_start",
    "adapter_import_start",
    "fonts_wait_end",
    "adapter_import_end",
    "engine_import_start",
    "engine_import_end",
    "register_start",
    "register_end",
    "initialize_start",
    "initialize_end",
    "render_start",
    "budgeted_svg_ready",
    "isolated_dom_inserted",
    "isolated_layout_box_ready",
    "isolated_presentation_ready",
  ] as const) {
    mermaid.observe(event);
  }
  assert.doesNotThrow(() => mermaid.assertComplete());

  const warm = createBenchmarkProgressGate({
    engine: "mermaid",
    intentKind: "warm-measured",
  });
  for (const event of [
    "fonts_wait_start",
    "fonts_wait_end",
    "render_start",
    "budgeted_svg_ready",
    "isolated_dom_inserted",
    "isolated_layout_box_ready",
    "isolated_presentation_ready",
  ] as const) {
    warm.observe(event);
  }
  assert.doesNotThrow(() => warm.assertComplete());
});

test("progress gate rejects duplicate, out-of-order, and inapplicable events", () => {
  const duplicate = createBenchmarkProgressGate({
    engine: "merman",
    intentKind: "cold-measured",
  });
  duplicate.observe("fonts_wait_start");
  assert.throws(() => duplicate.observe("fonts_wait_start"), /twice/);

  const outOfOrder = createBenchmarkProgressGate({
    engine: "merman",
    intentKind: "cold-measured",
  });
  assert.throws(() => outOfOrder.observe("render_start"), /requires/);

  const wrongEngine = createBenchmarkProgressGate({
    engine: "mermaid",
    intentKind: "cold-measured",
  });
  wrongEngine.observe("fonts_wait_start");
  wrongEngine.observe("adapter_import_start");
  assert.throws(
    () => wrongEngine.observe("resource_acquire_start"),
    /forbidden/
  );

  const incomplete = createBenchmarkProgressGate({
    engine: "merman",
    intentKind: "cold-measured",
  });
  incomplete.observe("fonts_wait_start");
  assert.throws(() => incomplete.assertComplete(), /incomplete/);
});

test("overlapping engine and resource stages own independent deadlines", () => {
  const timer = fakeTimer();
  const timedOut: string[] = [];
  const watchdog = createBenchmarkStageWatchdog(
    { engine: "merman", intentKind: "cold-measured" },
    (stage) => timedOut.push(stage),
    timer,
    30
  );

  watchdog.observe("engine_import_start");
  watchdog.observe("resource_acquire_start");
  assert.equal(timer.pending.size, 2);
  watchdog.observe("engine_import_end");
  assert.equal(timer.pending.size, 1);
  timer.fireAll();
  assert.deepEqual(timedOut, ["resource-acquire"]);
});

test("budgeted SVG completes render and starts the isolated presentation deadline", () => {
  const timer = fakeTimer();
  const timedOut: string[] = [];
  const watchdog = createBenchmarkStageWatchdog(
    { engine: "merman", intentKind: "cold-measured" },
    (stage) => timedOut.push(stage),
    timer
  );

  watchdog.observe("render_start");
  watchdog.observe("budgeted_svg_ready");
  assert.equal(timer.pending.size, 1);
  timer.fireAll();
  assert.deepEqual(timedOut, ["presentation"]);
});

test("stage completion and disposal clear every timer idempotently", () => {
  const timer = fakeTimer();
  const watchdog = createBenchmarkStageWatchdog(
    { engine: "merman", intentKind: "cold-measured" },
    () => undefined,
    timer
  );

  watchdog.observe("fonts_wait_start");
  watchdog.observe("adapter_import_start");
  watchdog.observe("fonts_wait_end");
  assert.equal(timer.pending.size, 1);
  watchdog.dispose();
  watchdog.dispose();
  assert.equal(timer.pending.size, 0);
});

test("synchronous work cannot clear an already exceeded deadline", () => {
  const timer = fakeTimer();
  const watchdog = createBenchmarkStageWatchdog(
    { engine: "merman", intentKind: "cold-measured" },
    () => undefined,
    timer,
    30
  );

  watchdog.observe("render_start");
  timer.advance(31);
  assert.throws(
    () => watchdog.observe("budgeted_svg_ready"),
    (error: unknown) =>
      error instanceof BenchmarkStageTimeoutError && error.stage === "render"
  );
  assert.equal(timer.pending.size, 0);
});

function fakeTimer(): BenchmarkStageTimer & {
  readonly pending: Map<number, () => void>;
  advance(milliseconds: number): void;
  fireAll(): void;
} {
  let nextId = 0;
  let now = 0;
  const pending = new Map<number, () => void>();
  return {
    pending,
    now: () => now,
    set(callback) {
      nextId += 1;
      pending.set(nextId, callback);
      return nextId;
    },
    clear(handle) {
      pending.delete(handle as number);
    },
    advance(milliseconds) {
      now += milliseconds;
    },
    fireAll() {
      const callbacks = [...pending.values()];
      pending.clear();
      for (const callback of callbacks) callback();
    },
  };
}
