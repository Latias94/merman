import assert from "node:assert/strict";
import test from "node:test";

import {
  BENCHMARK_PROTOCOL_VERSION,
  type BenchmarkSampleRequest,
} from "../protocol.ts";
import {
  BENCHMARK_TRACE_SCHEMA_VERSION,
  type BenchmarkRawTrace,
  type BenchmarkTraceMark,
} from "../trace.ts";
import {
  createBenchmarkRealmSession,
  type BenchmarkRealmSessionDependencies,
} from "./controller.ts";
import {
  REALM_BUDGETS,
  REALM_PROTOCOL_VERSION,
} from "../../runtime/realm/channel-protocol.ts";

const IDENTITY = {
  kind: "benchmark" as const,
  realmId: "test-realm",
  realmToken: "t".repeat(43),
};
const RUN_TOKEN = "r".repeat(43);

test("parent session validates SVG then projects immutable evidence without SVG", async () => {
  const harness = createControllerHarness();
  const session = await createBenchmarkRealmSession(
    { width: 800, height: 600 },
    new AbortController().signal,
    harness.dependencies
  );

  const pending = session.sample(sampleInput("realm-cold", "cold-1"));
  const request = await harness.nextRequest();
  let sequence = sendProgress(harness, request, COLD_MERMAN_PROGRESS, 0);
  sequence += 1;
  harness.respond(successResponse(request, coldTrace(), sequence));
  const result = await pending;

  assert.equal(result.type, "benchmark-sample-success");
  assert.equal("svg" in result, false);
  assert.equal(result.svgBytes, 46);
  assert.equal(harness.validatedSvgCount, 1);
  assert.equal(harness.pendingTimers.size, 1);
  assert(Object.isFrozen(result));

  const warm = session.sample(sampleInput("warm", "warm-1"));
  const warmRequest = await harness.nextRequest();
  sequence = sendProgress(harness, warmRequest, WARM_PROGRESS, sequence);
  sequence += 1;
  harness.respond(successResponse(warmRequest, warmTrace(), sequence));
  assert.equal((await warm).type, "benchmark-sample-success");
  assert.equal(harness.pendingTimers.size, 1);

  session.dispose();
  assert.equal(harness.disposeCount, 1);
  assert.equal(harness.pendingTimers.size, 0);
  harness.close();
});

test("failure destroys the realm before resolving and rejects immediate reuse", async () => {
  const harness = createControllerHarness();
  const session = await createBenchmarkRealmSession(
    { width: 800, height: 600 },
    new AbortController().signal,
    harness.dependencies
  );

  const pending = session.sample(sampleInput("realm-cold", "cold-1"));
  const request = await harness.nextRequest();
  let sequence = sendProgress(
    harness,
    request,
    COLD_MERMAN_PROGRESS.slice(0, 11),
    0
  );
  sequence += 1;
  harness.respond(failureResponse(request, sequence));
  const failure = await pending;

  assert.equal(failure.type, "benchmark-sample-failure");
  assert.equal(harness.disposeCount, 1);
  assert.equal(harness.pendingTimers.size, 0);
  await assert.rejects(
    session.sample(sampleInput("warm", "late")),
    /not ready/
  );
  harness.close();
});

test("unsafe parent-side SVG poisons transport and rejects the active sample", async () => {
  const harness = createControllerHarness(() => {
    throw new Error("unsafe SVG");
  });
  const session = await createBenchmarkRealmSession(
    { width: 800, height: 600 },
    new AbortController().signal,
    harness.dependencies
  );

  const pending = session.sample(sampleInput("realm-cold", "cold-1"));
  const request = await harness.nextRequest();
  let sequence = sendProgress(harness, request, COLD_MERMAN_PROGRESS, 0);
  sequence += 1;
  harness.respond(successResponse(request, coldTrace(), sequence));

  await assert.rejects(pending, /unsafe SVG/);
  assert.equal(harness.poisonCount, 1);
  assert.equal(harness.pendingTimers.size, 0);
  await assert.rejects(
    session.sample(sampleInput("warm", "late")),
    /not ready/
  );
  session.dispose();
  harness.close();
});

test("transport failure clears the cumulative timer and rejects active work", async () => {
  const harness = createControllerHarness();
  const session = await createBenchmarkRealmSession(
    { width: 800, height: 600 },
    new AbortController().signal,
    harness.dependencies
  );

  const pending = session.sample(sampleInput("realm-cold", "cold-1"));
  await harness.nextRequest();
  assert.equal(harness.pendingTimers.size, 2);
  harness.failTransport(new Error("transport lost"));

  await assert.rejects(pending, /transport lost/);
  assert.equal(harness.pendingTimers.size, 0);
  await assert.rejects(
    session.sample(sampleInput("warm", "late")),
    /not ready/
  );
  harness.close();
});

test("parent watchdog rejects a sample that sends no progress", async () => {
  const harness = createControllerHarness();
  const session = await createBenchmarkRealmSession(
    { width: 800, height: 600 },
    new AbortController().signal,
    harness.dependencies
  );

  const pending = session.sample(sampleInput("realm-cold", "cold-1"));
  await harness.nextRequest();
  assert.equal(harness.pendingTimers.size, 2);
  harness.fireTimeout(REALM_BUDGETS.stageTimeoutMs);

  await assert.rejects(pending, /progress.*timed out/i);
  assert.equal(harness.poisonCount, 1);
  assert.equal(harness.pendingTimers.size, 0);
  harness.close();
});

test("duplicate progress poisons the realm without extending any deadline", async () => {
  const harness = createControllerHarness();
  const session = await createBenchmarkRealmSession(
    { width: 800, height: 600 },
    new AbortController().signal,
    harness.dependencies
  );

  const pending = session.sample(sampleInput("realm-cold", "cold-1"));
  const request = await harness.nextRequest();
  harness.respond(progressResponse(request, "fonts_wait_start", 1));
  await harness.waitForTimerSetCount(4);
  const timerSetCount = harness.timerSetCount;
  harness.respond(progressResponse(request, "fonts_wait_start", 2));

  await assert.rejects(pending, /twice/);
  assert.equal(harness.timerSetCount, timerSetCount);
  assert.equal(harness.poisonCount, 1);
  assert.equal(harness.pendingTimers.size, 0);
  harness.close();
});

test("progress in one stage cannot extend an overlapping stage deadline", async () => {
  const harness = createControllerHarness();
  const session = await createBenchmarkRealmSession(
    { width: 800, height: 600 },
    new AbortController().signal,
    harness.dependencies
  );

  const pending = session.sample(sampleInput("realm-cold", "cold-1"));
  const request = await harness.nextRequest();
  harness.respond(progressResponse(request, "fonts_wait_start", 1));
  harness.respond(progressResponse(request, "adapter_import_start", 2));
  harness.respond(progressResponse(request, "adapter_import_end", 3));
  await harness.waitForTimerSetCount(7);
  harness.fireTimeout(REALM_BUDGETS.stageTimeoutMs);

  await assert.rejects(pending, /during fonts/);
  assert.equal(harness.poisonCount, 1);
  assert.equal(harness.pendingTimers.size, 0);
  harness.close();
});

test("a complete raw trace cannot substitute for live progress", async () => {
  const harness = createControllerHarness();
  const session = await createBenchmarkRealmSession(
    { width: 800, height: 600 },
    new AbortController().signal,
    harness.dependencies
  );

  const pending = session.sample(sampleInput("realm-cold", "cold-1"));
  const request = await harness.nextRequest();
  harness.respond(successResponse(request, coldTrace(), 1));

  await assert.rejects(pending, /progress.*incomplete/i);
  assert.equal(harness.poisonCount, 1);
  assert.equal(harness.pendingTimers.size, 0);
  harness.close();
});

const COLD_MERMAN_PROGRESS = Object.freeze([
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
  "safe_svg_ready",
  "dom_inserted",
  "layout_box_ready",
  "presentation_ready",
] as const satisfies readonly BenchmarkTraceMark[]);

const WARM_PROGRESS = Object.freeze([
  "fonts_wait_start",
  "fonts_wait_end",
  "render_start",
  "safe_svg_ready",
  "dom_inserted",
  "layout_box_ready",
  "presentation_ready",
] as const satisfies readonly BenchmarkTraceMark[]);

function sampleInput(mode: "realm-cold" | "warm", requestId: string) {
  return {
    runId: "run-1",
    runToken: RUN_TOKEN,
    requestId,
    engine: "merman" as const,
    mode,
    role: "measured" as const,
    payload: {
      source: "flowchart TD\nA-->B",
      configJson: "{}",
      theme: "default",
      diagramFont: "trebuchet" as const,
      externalRequirements: { elkLayouts: false, zenuml: false },
      viewport: { width: 800, height: 600 },
    },
  };
}

function successResponse(
  request: BenchmarkSampleRequest,
  trace: BenchmarkRawTrace,
  sequence: number
) {
  return {
    type: "benchmark-sample-success",
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence,
    runId: request.runId,
    runToken: request.runToken,
    requestId: request.requestId,
    engine: request.engine,
    mode: request.mode,
    role: request.role,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace,
    resources: [],
    resourceError: null,
    svg: '<svg xmlns="http://www.w3.org/2000/svg"></svg>',
    version: "test-version",
  };
}

function failureResponse(request: BenchmarkSampleRequest, sequence: number) {
  return {
    type: "benchmark-sample-failure",
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence,
    runId: request.runId,
    runToken: request.runToken,
    requestId: request.requestId,
    engine: request.engine,
    mode: request.mode,
    role: request.role,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    trace: {
      ...coldTrace(),
      safe_svg_ready: null,
      dom_inserted: null,
      layout_box_ready: null,
      presentation_ready: null,
      sample_end: 9,
    },
    resources: [],
    resourceError: null,
    stage: "render",
    message: "render failed",
    version: "test-version",
  };
}

function progressResponse(
  request: BenchmarkSampleRequest,
  event: BenchmarkTraceMark,
  sequence: number
) {
  return {
    type: "benchmark-progress",
    protocol: REALM_PROTOCOL_VERSION,
    benchmarkProtocol: BENCHMARK_PROTOCOL_VERSION,
    ...IDENTITY,
    sequence,
    runId: request.runId,
    runToken: request.runToken,
    requestId: request.requestId,
    engine: request.engine,
    mode: request.mode,
    role: request.role,
    traceSchema: BENCHMARK_TRACE_SCHEMA_VERSION,
    event,
  };
}

function sendProgress(
  harness: Readonly<{ respond(message: unknown): void }>,
  request: BenchmarkSampleRequest,
  events: readonly BenchmarkTraceMark[],
  initialSequence: number
): number {
  let sequence = initialSequence;
  for (const event of events) {
    sequence += 1;
    harness.respond(progressResponse(request, event, sequence));
  }
  return sequence;
}

function coldTrace(): BenchmarkRawTrace {
  return {
    sample_start: 0 as const,
    fonts_wait_start: 0,
    fonts_wait_end: 7,
    adapter_import_start: 0,
    adapter_import_end: 2,
    engine_import_start: 2,
    engine_import_end: 5,
    resource_acquire_start: 2.5,
    resource_acquire_end: 6,
    register_start: null,
    register_end: null,
    initialize_start: 6,
    initialize_end: 7,
    render_start: 8,
    safe_svg_ready: 10,
    dom_inserted: 10.5,
    layout_box_ready: 11,
    presentation_ready: 12,
    sample_end: 12.5,
  };
}

function warmTrace(): BenchmarkRawTrace {
  return {
    sample_start: 0,
    fonts_wait_start: 0,
    fonts_wait_end: 1,
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
    render_start: 1,
    safe_svg_ready: 3,
    dom_inserted: 3,
    layout_box_ready: 4,
    presentation_ready: 5,
    sample_end: 5,
  };
}

function createControllerHarness(validateSvg: (svg: string) => void = () => {}) {
  const channel = new MessageChannel();
  const pendingTimers = new Map<
    number,
    Readonly<{ callback: () => void; timeoutMs: number }>
  >();
  let nextTimer = 0;
  let timerSetCount = 0;
  let disposeCount = 0;
  let poisonCount = 0;
  let validatedSvgCount = 0;
  let onFailure: ((error: Error) => void) | null = null;
  const requests: BenchmarkSampleRequest[] = [];
  const requestWaiters: Array<(request: BenchmarkSampleRequest) => void> = [];

  channel.port2.onmessage = (event) => {
    const request = event.data as BenchmarkSampleRequest;
    const waiter = requestWaiters.shift();
    if (waiter) {
      waiter(request);
    } else {
      requests.push(request);
    }
  };
  channel.port2.start();
  const dependencies: BenchmarkRealmSessionDependencies = {
    clearTimer(handle) {
      pendingTimers.delete(handle as number);
    },
    async createChannel(options) {
      onFailure = options.onFailure;
      channel.port1.start();
      return {
        identity: IDENTITY,
        port: channel.port1,
        dispose() {
          disposeCount += 1;
          channel.port1.close();
        },
        poison(error) {
          poisonCount += 1;
          const failure = error instanceof Error ? error : new Error(String(error));
          onFailure?.(failure);
        },
        setViewport: async () => undefined,
      };
    },
    getVisibilityState: () => "visible",
    now: () => 0,
    realmUrl: new URL("https://play.test/benchmark.html"),
    setTimer(callback, timeoutMs) {
      nextTimer += 1;
      timerSetCount += 1;
      pendingTimers.set(nextTimer, { callback, timeoutMs });
      return nextTimer;
    },
    validateSvg(svg) {
      validatedSvgCount += 1;
      validateSvg(svg);
    },
  };

  return {
    dependencies,
    pendingTimers,
    get disposeCount() {
      return disposeCount;
    },
    get poisonCount() {
      return poisonCount;
    },
    get validatedSvgCount() {
      return validatedSvgCount;
    },
    get timerSetCount() {
      return timerSetCount;
    },
    nextRequest() {
      const request = requests.shift();
      return request
        ? Promise.resolve(request)
        : new Promise<BenchmarkSampleRequest>((resolve) => {
            requestWaiters.push(resolve);
          });
    },
    respond(message: unknown) {
      channel.port2.postMessage(message);
    },
    failTransport(error: Error) {
      onFailure?.(error);
    },
    fireTimeout(timeoutMs: number) {
      const match = [...pendingTimers].find(
        ([, timer]) => timer.timeoutMs === timeoutMs
      );
      assert(match, `No ${timeoutMs}ms timer is pending.`);
      const [handle, timer] = match;
      pendingTimers.delete(handle);
      timer.callback();
    },
    async waitForTimerSetCount(expected: number) {
      for (let attempt = 0; attempt < 20; attempt += 1) {
        if (timerSetCount === expected) return;
        await new Promise<void>((resolve) => setImmediate(resolve));
      }
      assert.equal(timerSetCount, expected);
    },
    close() {
      channel.port1.close();
      channel.port2.close();
    },
  };
}
