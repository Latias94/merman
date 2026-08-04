import assert from "node:assert/strict";
import test from "node:test";

import {
  createBenchmarkController,
  validateBenchmarkRunRequest,
  type BenchmarkControllerDependencies,
  type BenchmarkRunRequest,
  type BenchmarkLifecycleTarget,
} from "./controller.ts";
import {
  BENCHMARK_PROTOCOL_VERSION,
  type BenchmarkSampleFailure,
} from "./protocol.ts";
import {
  BENCHMARK_TRACE_SCHEMA_VERSION,
  type BenchmarkEngine,
  type BenchmarkRawTrace,
} from "./trace.ts";
import type {
  BenchmarkRealmSampleResult,
  BenchmarkSampleInput,
  BrowserBenchmarkRealmSession,
} from "./realm/controller.ts";
import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
} from "../runtime/realm/channel-protocol.ts";
import { BENCHMARK_PUBLICATION_CLOCK_BOUNDARY } from "./publication.ts";

test("realm-cold runs deterministic balanced blocks with one fresh realm per sample", async () => {
  const harness = createHarness();
  const controller = createBenchmarkController(harness.dependencies);
  const report = await controller.start(runRequest("realm-cold", 4, 0)).completion;

  assert.equal(report.terminalStatus, "success");
  assert.equal(harness.createCount, 8);
  assert.equal(harness.disposeCount, 8);
  assert.equal(harness.maxLiveRealms, 1);
  assert.equal(harness.pauseCount, 1);
  assert.equal(harness.releaseCount, 1);
  assert.equal(harness.listenerCount, 0);
  assert.deepEqual(
    harness.sampleInputs.map((sample) => sample.engine),
    report.schedule.blocks.flatMap((block) => block.order)
  );
  assert(harness.sampleInputs.every((sample) => sample.mode === "realm-cold"));
  assert(
    harness.sampleInputs.every(
      (sample) =>
        JSON.stringify(sample.payload) ===
        JSON.stringify(harness.sampleInputs[0].payload)
    )
  );
  assert(report.aggregates);
  assert.equal(report.aggregates.ratios.firstIsolatedPresentationMs, 1.5);
  assert.equal(report.aggregates.ratios.firstPublishableSvgMs, 1.5);
  assert.equal(report.aggregates.ratios.resourceAcquisitionMs, null);
  assert(
    report.samples.every(
      (sample) =>
        sample.realmCreation?.clockBoundary === "parent-before-sample" &&
        sample.realmCreation.artifact.id ===
          (sample.engine === "merman" ? "benchmark-merman" : "mermaid")
    )
  );
});

test("warm mode creates two engine realms, applies equal warmups, and reuses each realm", async () => {
  const harness = createHarness();
  const controller = createBenchmarkController(harness.dependencies);
  const report = await controller.start(runRequest("warm", 4, 2)).completion;

  assert.equal(report.terminalStatus, "success");
  assert.equal(harness.createCount, 2);
  assert.equal(harness.disposeCount, 2);
  assert.equal(harness.maxLiveRealms, 2);
  for (const engine of ["merman", "mermaid"] as const) {
    const inputs = harness.sampleInputs.filter((sample) => sample.engine === engine);
    assert.equal(inputs.length, 7);
    assert.equal(inputs[0].mode, "realm-cold");
    assert.equal(inputs[0].role, "warmup");
    assert.deepEqual(
      inputs.slice(1).map((sample) => sample.mode),
      Array(6).fill("warm")
    );
    assert.equal(inputs.filter((sample) => sample.role === "warmup").length, 3);
    assert.equal(inputs.filter((sample) => sample.role === "measured").length, 4);
  }
  assert(report.aggregates);
  assert.equal(report.aggregates.ratios.warmIsolatedPresentationMs, 1.5);
  assert.equal(report.aggregates.ratios.warmPublishableSvgMs, 1.5);
  assert.equal(report.aggregates.ratios.firstIsolatedPresentationMs, null);
  assert.equal(
    report.samples.filter((sample) => sample.realmCreation !== null).length,
    2
  );
  assert(
    report.samples
      .filter((sample) => sample.mode === "warm")
      .every((sample) => sample.realmCreation === null)
  );
});

test("realm failure remains raw evidence and fails every ratio closed", async () => {
  let failed = false;
  const harness = createHarness((input) => {
    if (!failed && input.engine === "mermaid" && input.role === "measured") {
      failed = true;
      return realmFailure(input);
    }
    return realmSuccess(input);
  });
  const controller = createBenchmarkController(harness.dependencies);
  const report = await controller.start(runRequest("realm-cold", 4, 0)).completion;

  assert.equal(report.terminalStatus, "complete-with-errors");
  assert.equal(report.samples.filter((sample) => sample.outcome === "failure").length, 1);
  assert(report.aggregates);
  assert(
    Object.values(report.aggregates.ratios).every((ratio) => ratio === null)
  );
  assert.equal(harness.disposeCount, harness.createCount);
});

test("observed package version drift becomes retained protocol failure", async () => {
  const harness = createHarness((input) => {
    const result = realmSuccess(input);
    return input.engine === "mermaid"
      ? { ...result, version: "11.15.0" }
      : result;
  });
  const controller = createBenchmarkController(harness.dependencies);
  const report = await controller.start(runRequest("realm-cold", 2, 0)).completion;

  assert.equal(report.terminalStatus, "complete-with-errors");
  const drift = report.samples.find(
    (sample) => sample.failure?.stage === "version"
  );
  assert(drift);
  assert.equal(drift.trace?.isolated_presentation_ready !== null, true);
  assert.equal(report.aggregates?.ratios.firstIsolatedPresentationMs, null);
});

test("visibility invalidation is atomic, retains the transition, and releases all ownership", async () => {
  const pending = Promise.withResolvers<BenchmarkRealmSampleResult>();
  const harness = createHarness(() => pending.promise);
  const controller = createBenchmarkController(harness.dependencies);
  const running = controller.start(runRequest("realm-cold", 4, 0)).completion;
  await waitFor(() => harness.sampleInputs.length === 1);

  harness.visibilityState = "hidden";
  harness.documentTarget.dispatch("visibilitychange", {});
  const report = await running;

  assert.equal(report.terminalStatus, "invalidated");
  assert.equal(report.aggregates, null);
  assert.equal(report.transitions.at(-1)?.kind, "visibility-hidden");
  assert.equal(harness.liveRealms, 0);
  assert.equal(harness.listenerCount, 0);
  assert.equal(harness.releaseCount, 1);
  pending.reject(new Error("disposed"));
});

test("pagehide and freeze each invalidate without publishing a partial aggregate", async (t) => {
  for (const kind of ["pagehide", "freeze"] as const) {
    await t.test(kind, async () => {
      const pending = Promise.withResolvers<BenchmarkRealmSampleResult>();
      const harness = createHarness(() => pending.promise);
      const controller = createBenchmarkController(harness.dependencies);
      const running = controller.start(runRequest("realm-cold", 4, 0)).completion;
      await waitFor(() => harness.sampleInputs.length === 1);

      if (kind === "pagehide") {
        harness.windowTarget.dispatch("pagehide", { persisted: true });
      } else {
        harness.documentTarget.dispatch("freeze", {});
      }
      const report = await running;
      assert.equal(report.terminalStatus, "invalidated");
      assert.equal(report.transitions.at(-1)?.kind, kind);
      assert.equal(report.aggregates, null);
      assert.equal(harness.liveRealms, 0);
      pending.reject(new Error("disposed"));
    });
  }
});

test("cancel closes active resources, rejects overlapping work, and permits a clean rerun", async () => {
  const pending = Promise.withResolvers<BenchmarkRealmSampleResult>();
  let block = true;
  const harness = createHarness((input) =>
    block ? pending.promise : realmSuccess(input)
  );
  const controller = createBenchmarkController(harness.dependencies);
  const firstRun = controller.start(runRequest("realm-cold", 4, 0));
  const first = firstRun.completion;
  await waitFor(() => harness.sampleInputs.length === 1);
  const activeState = controller.store.getState();
  assert.equal(activeState.status, "running");
  assert.equal(activeState.activeRunId, firstRun.runId);

  assert.throws(
    () => controller.start(runRequest("realm-cold", 4, 0)),
    /already active/,
  );
  controller.cancel("dialog-closed");
  const cancelled = await first;
  assert.equal(cancelled.terminalStatus, "cancelled");
  assert.equal(cancelled.aggregates, null);
  const cancelledState = controller.store.getState();
  assert.equal(cancelledState.status, "cancelled");
  assert.equal(cancelledState.retained.report, cancelled);
  assert.equal(cancelledState.cancellation, null);
  assert.equal(harness.liveRealms, 0);
  pending.reject(new Error("disposed"));

  block = false;
  const rerun = await controller.start(runRequest("realm-cold", 4, 0)).completion;
  assert.equal(rerun.terminalStatus, "success");
  assert.equal(harness.liveRealms, 0);
});

test("cancelling a rerun restores the retained report and its stale state", async () => {
  const pending = Promise.withResolvers<BenchmarkRealmSampleResult>();
  let block = false;
  const harness = createHarness((input) =>
    block ? pending.promise : realmSuccess(input)
  );
  const controller = createBenchmarkController(harness.dependencies);
  const retained = await controller.start(runRequest("realm-cold", 2, 0)).completion;
  controller.markStale();
  const beforeRejectedStart = controller.store.getState();
  const oversized = runRequest("realm-cold", 2, 0);
  assert.throws(
    () =>
      controller.start({
        ...oversized,
        payload: {
          ...oversized.payload,
          source: "x".repeat(REALM_BUDGETS.sourceBytes + 1),
        },
      }),
    /byte budget/,
  );
  assert.strictEqual(controller.store.getState(), beforeRejectedStart);

  block = true;
  const rerun = controller.start(runRequest("realm-cold", 4, 0)).completion;
  await waitFor(() => harness.sampleInputs.length === 5);
  const running = controller.store.getState();
  assert.equal(running.status, "running");
  assert.equal(running.retained?.report, retained);
  assert.equal(running.retained?.stale, true);
  assert.equal(running.stale, false);

  controller.cancel("dialog-closed");
  const cancelled = await rerun;
  const restored = controller.store.getState();
  assert.equal(restored.status, "cancelled");
  assert.equal(restored.retained.report, retained);
  assert.equal(restored.retained.stale, true);
  assert.equal(restored.cancellation?.report, cancelled);

  pending.reject(new Error("disposed"));
  await Promise.resolve();
  assert.strictEqual(controller.store.getState(), restored);
});

test("whole-run watchdog terminates cross-realm work and releases the coordinator", async () => {
  const pending = Promise.withResolvers<BenchmarkRealmSampleResult>();
  const harness = createHarness(() => pending.promise);
  const controller = createBenchmarkController(harness.dependencies);
  const running = controller.start(runRequest("realm-cold", 4, 0)).completion;
  await waitFor(() => harness.sampleInputs.length === 1);

  harness.fireRunTimeout();
  const report = await running;
  assert.equal(report.terminalStatus, "failed");
  assert.equal(report.terminalError?.stage, "timeout");
  assert.equal(harness.liveRealms, 0);
  assert.equal(harness.releaseCount, 1);
  assert.equal(harness.pendingTimerCount, 0);
  pending.reject(new Error("disposed"));
});

test("validation accepts exact cold limits and rejects overflow before pause or realm creation", async () => {
  const exact = runRequest("realm-cold", 1_000, 0);
  const exactPayload = {
    ...exact.payload,
    source: "s".repeat(REALM_BUDGETS.sourceBytes),
    configJson: "c".repeat(REALM_BUDGETS.configBytes),
  };
  const exactRequest = { ...exact, payload: exactPayload };
  assert.equal(
    validateBenchmarkRunRequest(exactRequest, 1).schedule.blocks.length,
    1_000
  );

  const harness = createHarness();
  const controller = createBenchmarkController(harness.dependencies);
  assert.throws(
    () =>
      controller.start({
        ...exactRequest,
        payload: { ...exactPayload, source: `${exactPayload.source}x` },
      }),
    /byte budget/,
  );
  assert.throws(
    () => controller.start(runRequest("warm", 1_000, 0)),
    /retained sample budget/,
  );
  assert.equal(harness.pauseCount, 0);
  assert.equal(harness.createCount, 0);
});

test("stale state is external to the immutable retained report", async () => {
  const harness = createHarness();
  const controller = createBenchmarkController(harness.dependencies);
  const report = await controller.start(runRequest("realm-cold", 2, 0)).completion;
  controller.markStale();

  const state = controller.store.getState();
  assert.equal(state.status, "success");
  assert.equal(state.stale, true);
  assert.equal(state.report, report);
  assert.equal("stale" in report, false);
});

function runRequest(
  mode: "realm-cold" | "warm",
  iterations: number,
  warmups: number
): BenchmarkRunRequest & { payload: BenchmarkRunRequest["payload"] } {
  return {
    mode,
    iterations,
    warmups,
    seed: 0x1234_5678,
    payload: {
      source: "flowchart TD\nA-->B",
      configJson: "{}",
      theme: "default",
      diagramFont: "trebuchet",
      externalRequirements: { externalDiagrams: [], layoutModules: [] },
      viewport: { width: 800, height: 600 },
    },
    detection: {
      status: "available",
      validity: "valid",
      diagramType: "flowchart",
      syntaxId: "flowchart",
      effectiveLayoutId: "dagre",
    },
    versions: { merman: "test-merman", mermaid: "test-mermaid" },
  };
}

function createHarness(
  sample: (
    input: BenchmarkSampleInput
  ) => BenchmarkRealmSampleResult | Promise<BenchmarkRealmSampleResult> = realmSuccess
) {
  const documentTarget = new FakeLifecycleTarget();
  const windowTarget = new FakeLifecycleTarget();
  const sampleInputs: BenchmarkSampleInput[] = [];
  let createCount = 0;
  let disposeCount = 0;
  let liveRealms = 0;
  let maxLiveRealms = 0;
  let pauseCount = 0;
  let releaseCount = 0;
  let visibilityState = "visible";
  let now = 100;
  let nextTimer = 0;
  const timers = new Map<number, Readonly<{ callback: () => void; timeoutMs: number }>>();

  const dependencies: BenchmarkControllerDependencies = {
    clearTimer(handle) {
      timers.delete(handle as number);
    },
    createRealm: async (engine) => {
      createCount += 1;
      liveRealms += 1;
      maxLiveRealms = Math.max(maxLiveRealms, liveRealms);
      let disposed = false;
      const session: BrowserBenchmarkRealmSession = {
        creationEvidence: {
          artifact: {
            bytes: 17,
            id: engine === "merman" ? "benchmark-merman" : "mermaid",
            schemaVersion: 1,
            sha256: "a".repeat(64),
          },
          artifactAcquisitionMs: 2,
          clockBoundary: "parent-before-sample",
          realmBootstrapMs: 3,
          totalMs: 5,
        },
        dispose() {
          if (disposed) return;
          disposed = true;
          disposeCount += 1;
          liveRealms -= 1;
        },
        async sample(input) {
          if (disposed) throw new Error("realm disposed");
          sampleInputs.push(input);
          now += 1;
          return sample(input);
        },
      };
      return session;
    },
    pauseCoordinator: async () => {
      pauseCount += 1;
      let released = false;
      return () => {
        if (released) return;
        released = true;
        releaseCount += 1;
      };
    },
    setTimer(callback, timeoutMs) {
      nextTimer += 1;
      timers.set(nextTimer, { callback, timeoutMs });
      return nextTimer;
    },
    createSeed: () => 7,
    createToken: () => "r".repeat(43),
    dateNow: () => 1_753_000_000_000 + now,
    now: () => now,
    getVisibilityState: () => visibilityState,
    documentTarget,
    windowTarget,
    getEnvironment: () => ({
      userAgent: "test",
      language: "en-US",
      platform: "test",
      hardwareConcurrency: 8,
      devicePixelRatio: 1,
      crossOriginIsolated: false,
    }),
  };

  return {
    dependencies,
    documentTarget,
    windowTarget,
    sampleInputs,
    get createCount() {
      return createCount;
    },
    get disposeCount() {
      return disposeCount;
    },
    get liveRealms() {
      return liveRealms;
    },
    get maxLiveRealms() {
      return maxLiveRealms;
    },
    get pauseCount() {
      return pauseCount;
    },
    get releaseCount() {
      return releaseCount;
    },
    get listenerCount() {
      return documentTarget.listenerCount + windowTarget.listenerCount;
    },
    get pendingTimerCount() {
      return timers.size;
    },
    fireRunTimeout() {
      const match = [...timers].find(
        ([, timer]) => timer.timeoutMs === REALM_BUDGETS.runTimeoutMs
      );
      assert(match, "No controller run timer is pending.");
      const [handle, timer] = match;
      timers.delete(handle);
      timer.callback();
    },
    get visibilityState() {
      return visibilityState;
    },
    set visibilityState(value: string) {
      visibilityState = value;
    },
  };
}

class FakeLifecycleTarget implements BenchmarkLifecycleTarget {
  readonly listeners = new Map<string, Set<(event: unknown) => void>>();

  get listenerCount(): number {
    return [...this.listeners.values()].reduce(
      (total, listeners) => total + listeners.size,
      0
    );
  }

  addEventListener(type: string, listener: (event: unknown) => void): void {
    const listeners = this.listeners.get(type) ?? new Set();
    listeners.add(listener);
    this.listeners.set(type, listeners);
  }

  removeEventListener(type: string, listener: (event: unknown) => void): void {
    this.listeners.get(type)?.delete(listener);
  }

  dispatch(type: string, event: unknown): void {
    for (const listener of this.listeners.get(type) ?? []) listener(event);
  }
}

function realmSuccess(input: BenchmarkSampleInput): BenchmarkRealmSampleResult {
  const trace =
    input.mode === "warm"
      ? warmTrace(input.engine)
      : coldTrace(input.engine);
  const scale = input.engine === "merman" ? 1 : 1.5;
  return {
    type: "benchmark-sample-success",
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    kind: "benchmark",
    realmId: `realm-${input.engine}`,
    realmToken: "t".repeat(43),
    sequence: 1,
    ...input,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace,
    resources: [],
    resourceError: null,
    version: `test-${input.engine}`,
    parentPublication: Object.freeze({
      clockBoundary: BENCHMARK_PUBLICATION_CLOCK_BOUNDARY,
      isolatedPresentationReceiptMs: 10 * scale,
      responseDeliveryMs: 0.25 * scale,
      responseEnvelopeValidationMs: 0.25 * scale,
      strictSvgValidationMs: 0.5 * scale,
      totalMs: 11 * scale,
    }),
    svgBytes: 100,
  };
}

function realmFailure(input: BenchmarkSampleInput): BenchmarkSampleFailure {
  const trace = coldTrace(input.engine);
  return {
    type: "benchmark-sample-failure",
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    kind: "benchmark",
    realmId: `realm-${input.engine}`,
    realmToken: "t".repeat(43),
    sequence: 1,
    ...input,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace: {
      ...trace,
      budgeted_svg_ready: null,
      isolated_dom_inserted: null,
      isolated_layout_box_ready: null,
      isolated_presentation_ready: null,
    },
    resources: [],
    resourceError: null,
    version: `test-${input.engine}`,
    stage: "render",
    message: "render failed",
    detail: null,
  };
}

function coldTrace(engine: BenchmarkEngine): BenchmarkRawTrace {
  const scale = engine === "merman" ? 1 : 1.5;
  return {
    sample_start: 0,
    fonts_wait_start: 0,
    fonts_wait_end: 1 * scale,
    adapter_import_start: 0,
    adapter_import_end: 2 * scale,
    engine_import_start: 2 * scale,
    engine_import_end: 4 * scale,
    resource_acquire_start: engine === "merman" ? 2.5 * scale : null,
    resource_acquire_end: engine === "merman" ? 5 * scale : null,
    register_start: engine === "mermaid" ? 4 * scale : null,
    register_end: engine === "mermaid" ? 5 * scale : null,
    initialize_start: 5 * scale,
    initialize_end: 6 * scale,
    render_start: 6 * scale,
    budgeted_svg_ready: 8 * scale,
    isolated_dom_inserted: 8 * scale,
    isolated_layout_box_ready: 9 * scale,
    isolated_presentation_ready: 10 * scale,
    sample_end: 10.5 * scale,
  };
}

function warmTrace(engine: BenchmarkEngine): BenchmarkRawTrace {
  const scale = engine === "merman" ? 1 : 1.5;
  return {
    sample_start: 0,
    fonts_wait_start: 0,
    fonts_wait_end: 1 * scale,
    adapter_import_start: null,
    adapter_import_end: null,
    engine_import_start: null,
    engine_import_end: null,
    resource_acquire_start: null,
    resource_acquire_end: null,
    register_start: null,
    register_end: null,
    initialize_start: null,
    initialize_end: null,
    render_start: 1 * scale,
    budgeted_svg_ready: 3 * scale,
    isolated_dom_inserted: 3 * scale,
    isolated_layout_box_ready: 4 * scale,
    isolated_presentation_ready: 5 * scale,
    sample_end: 5.5 * scale,
  };
}

async function waitFor(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (predicate()) return;
    await new Promise<void>((resolve) => setImmediate(resolve));
  }
  assert.fail("Timed out waiting for benchmark controller state.");
}
